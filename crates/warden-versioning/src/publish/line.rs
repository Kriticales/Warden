//! A linha de publicação local (ARCHITECTURE §11.1 itens 2, 4 e 5).
//!
//! Cada publicação é um commit em `refs/warden/publish/main` cujo pai é a publicação
//! anterior e cuja árvore é **só** o conteúdo do pack daquela versão (a árvore da exportação,
//! E-01), mais `CHANGELOG.md` (notas de todas as versões publicadas, a mais nova no topo) e
//! `.gitattributes` com `* -text`. A tag anotada `refs/warden/publish-tags/vX.Y.Z` guarda as
//! notas da versão. Nada da linha de trabalho (`main` local, `.warden/`, versões não
//! publicadas) entra nela.
//!
//! As referências só são gravadas depois que o envio deu certo
//! ([`PackRepo::record_publication`]); antes disso, o commit e a tag são objetos soltos, e uma
//! falha não deixa nada no remoto nem muda o estado "publicada".

use std::collections::BTreeMap;

use git2::{ObjectType, Oid};
use semver::Version;
use serde::{Deserialize, Serialize};

use super::PublishTree;
use crate::changelog::prepend_entry;
use crate::error::{Error, GitContext as _, Result};
use crate::moment::Moment;
use crate::repo::{Identity, PackRepo};
use crate::versions::{FINAL_REF_PREFIX, PUBLISH_TAG_PREFIX, parse_version, tag_name};
use crate::{Snapshot, VersionNameResolver};

/// Ponta local da linha de publicação (a `main` do GitHub depois de cada publicação).
pub const PUBLISH_LINE_REF: &str = "refs/warden/publish/main";

/// Versões enviadas cuja Release ainda não foi criada ("publicada, Release pendente").
pub const RELEASE_PENDING_PREFIX: &str = "refs/warden/publish-pending/";

/// Conteúdo do `.gitattributes` publicado: o git nunca muda finais de linha, e os hashes do
/// `index.toml` continuam valendo para os jogadores (CA-T18-07).
pub const GITATTRIBUTES: &str = "* -text\n";

/// Arquivos que a publicação acrescenta à árvore da exportação.
pub const ADDED_FILES: [&str; 2] = ["CHANGELOG.md", ".gitattributes"];

/// Uma versão publicada, como o Histórico mostra.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct PublishedVersion {
    /// Número da versão.
    pub version: String,
    /// Data da publicação, RFC 3339.
    pub date: String,
    /// Enviada, mas a Release não foi criada.
    pub release_pending: bool,
}

/// O commit e a tag de uma publicação, prontos para enviar.
#[derive(Debug, Clone)]
pub struct PreparedPublication {
    /// Versão.
    pub version: Version,
    /// Commit novo da linha de publicação.
    pub commit: Oid,
    /// Tag anotada (objeto) da versão.
    pub tag: Oid,
    /// Publicação anterior (a ponta atual da linha), se houver.
    pub parent: Option<Oid>,
    /// Nome da Release: "<Nome do pack> X.Y.Z".
    pub release_name: String,
    /// Notas da versão (corpo da Release e mensagem da tag).
    pub notes: String,
}

/// Pedido de publicação de uma versão final.
#[derive(Debug, Clone, Copy)]
pub struct PublishRequest<'a> {
    /// Versão final (`1.2.0`).
    pub version: &'a str,
    /// Árvore da versão montada pela exportação: `pack.toml`, `index.toml` e os arquivos do
    /// índice.
    pub tree: &'a PublishTree,
    /// Notas da versão (já com as notas para os jogadores), em Markdown.
    pub notes: &'a str,
    /// Nome do pack (título da Release e mensagem do commit).
    pub pack_name: &'a str,
    /// Quem assina.
    pub identity: &'a Identity,
    /// Quando.
    pub when: Moment,
}

impl PackRepo {
    /// Ponta da linha de publicação.
    ///
    /// Erros: leitura do histórico.
    pub fn publish_line_tip(&self) -> Result<Option<Oid>> {
        self.reference_commit(PUBLISH_LINE_REF)
    }

    /// Versões publicadas, da maior para a menor.
    ///
    /// Erros: leitura do histórico.
    pub fn published_versions(&self) -> Result<Vec<PublishedVersion>> {
        let mut found = Vec::new();
        let references = self
            .repo
            .references_glob(&format!("{PUBLISH_TAG_PREFIX}v*"))
            .ctx("listar as versões publicadas")?;
        for reference in references {
            let reference = reference.ctx("ler uma versão publicada")?;
            let Ok(name) = reference.name() else {
                continue;
            };
            let Some(text) = name
                .strip_prefix(PUBLISH_TAG_PREFIX)
                .and_then(|n| n.strip_prefix('v'))
            else {
                continue;
            };
            let Ok(version) = Version::parse(text) else {
                continue;
            };
            let date = match reference.peel(ObjectType::Tag) {
                Ok(object) => object
                    .as_tag()
                    .and_then(git2::Tag::tagger)
                    .map(|who| Moment::from_git(who.when())),
                Err(_) => None,
            };
            let date = match date {
                Some(date) => date,
                None => {
                    Moment::from_git(reference.peel_to_commit().ctx("ler a publicação")?.time())
                }
            };
            let pending = self
                .repo
                .find_reference(&format!("{RELEASE_PENDING_PREFIX}{}", tag_name(&version)))
                .is_ok();
            found.push((version, date, pending));
        }
        found.sort_by(|a, b| b.0.cmp(&a.0));
        Ok(found
            .into_iter()
            .map(|(version, date, release_pending)| PublishedVersion {
                version: version.to_string(),
                date: date.rfc3339(),
                release_pending,
            })
            .collect())
    }

    /// Maior versão publicada.
    ///
    /// Erros: leitura do histórico.
    pub fn last_published(&self) -> Result<Option<Version>> {
        Ok(self
            .published_versions()?
            .first()
            .and_then(|published| Version::parse(&published.version).ok()))
    }

    /// Base das notas de uma publicação: a última versão publicada (não a última salva,
    /// CA-T18-05), ou o vazio na primeira publicação.
    ///
    /// Erros: leitura do histórico.
    pub fn publication_base(&self) -> Result<Snapshot> {
        Ok(self.last_published()?.map_or(Snapshot::Empty, |version| {
            Snapshot::Version(version.to_string())
        }))
    }

    /// Changelog estruturado de uma publicação: da última versão publicada até `version`.
    ///
    /// Erros: os de [`PackRepo::changes`].
    pub fn publication_changes(
        &self,
        version: &str,
        resolver: &dyn VersionNameResolver,
    ) -> Result<crate::ChangeSet> {
        self.changes(
            &self.publication_base()?,
            &Snapshot::Version(version.to_owned()),
            resolver,
        )
    }

    /// Confere se `version` pode ser publicada: existe, é versão final, não foi publicada e é
    /// maior que a última publicada (publicar uma menor faria os jogadores voltarem).
    ///
    /// Erros: [`Error::InvalidVersion`], [`Error::VersionNotFound`], [`Error::NotFinal`],
    /// [`Error::AlreadyPublished`], [`Error::PublishNotNewer`].
    pub fn check_publishable(&self, version: &str) -> Result<Version> {
        let parsed = parse_version(version)?;
        self.version_commit(version)?;
        let name = tag_name(&parsed);
        if self
            .reference_commit(&format!("{FINAL_REF_PREFIX}{name}"))?
            .is_none()
        {
            return Err(Error::NotFinal {
                version: parsed.to_string(),
            });
        }
        if self
            .reference_commit(&format!("{PUBLISH_TAG_PREFIX}{name}"))?
            .is_some()
        {
            return Err(Error::AlreadyPublished {
                version: parsed.to_string(),
            });
        }
        if let Some(last) = self.last_published()?
            && parsed <= last
        {
            return Err(Error::PublishNotNewer {
                version: parsed.to_string(),
                last: last.to_string(),
            });
        }
        Ok(parsed)
    }

    /// Monta o commit e a tag da publicação, sem gravar referências.
    ///
    /// Erros: os de [`PackRepo::check_publishable`]; [`Error::Internal`] se a árvore estiver
    /// vazia, sem `pack.toml`/`index.toml` ou trouxer `CHANGELOG.md`, `.gitattributes`,
    /// `.warden/` ou `.git/` (a exportação nunca entrega isso); falhas do git.
    pub fn prepare_publication(&self, request: &PublishRequest<'_>) -> Result<PreparedPublication> {
        let version = self.check_publishable(request.version)?;
        for required in ["pack.toml", "index.toml"] {
            if !request.tree.contains_key(required) {
                return Err(Error::Internal(format!("árvore a publicar sem {required}")));
            }
        }
        if let Some(path) = request.tree.keys().find(|path| {
            ADDED_FILES.contains(&path.as_str())
                || path.starts_with(".warden/")
                || path.starts_with(".git/")
                || *path == ".git"
                || *path == ".warden"
        }) {
            return Err(Error::Internal(format!(
                "arquivo que não pode ir para o GitHub: {path}"
            )));
        }
        let parent = self.publish_line_tip()?;
        let previous_changelog = match parent {
            Some(parent) => self.blob_at(parent, "CHANGELOG.md")?,
            None => None,
        };
        let notes = request.notes.trim_end().to_owned();
        let entry = if notes.is_empty() {
            format!("## {version} — {}\n", request.when.date())
        } else {
            format!("## {version} — {}\n\n{notes}\n", request.when.date())
        };
        let changelog = prepend_entry(
            previous_changelog
                .as_deref()
                .map(String::from_utf8_lossy)
                .as_deref(),
            &entry,
        );
        let mut files: BTreeMap<&str, &[u8]> = request
            .tree
            .iter()
            .map(|(path, bytes)| (path.as_str(), bytes.as_slice()))
            .collect();
        files.insert("CHANGELOG.md", changelog.as_bytes());
        files.insert(".gitattributes", GITATTRIBUTES.as_bytes());
        let tree_id = self.write_tree(&files)?;
        let tree = self.repo.find_tree(tree_id).ctx("ler a árvore publicada")?;
        let signature = request.identity.signature(request.when)?;
        let release_name = format!("{} {version}", request.pack_name.trim());
        let parent_commit = parent
            .map(|id| self.repo.find_commit(id))
            .transpose()
            .ctx("ler a publicação anterior")?;
        let parents: Vec<&git2::Commit<'_>> = parent_commit.iter().collect();
        let commit = self
            .repo
            .commit(None, &signature, &signature, &release_name, &tree, &parents)
            .ctx("gravar a publicação")?;
        let object = self
            .repo
            .find_object(commit, Some(ObjectType::Commit))
            .ctx("ler a publicação")?;
        let tag_message = if notes.is_empty() {
            release_name.clone()
        } else {
            notes.clone()
        };
        let tag = self
            .repo
            .tag_annotation_create(&tag_name(&version), &object, &signature, &tag_message)
            .ctx("gravar a tag da publicação")?;
        Ok(PreparedPublication {
            version,
            commit,
            tag,
            parent,
            release_name,
            notes,
        })
    }

    /// Pacote de objetos para o servidor: os commits novos da linha (sem o que o servidor já
    /// tem, quando o commit dele existe aqui) e a tag.
    ///
    /// Erros: falhas do git.
    pub(crate) fn publication_pack(
        &self,
        prepared: &PreparedPublication,
        remote_has: Option<Oid>,
    ) -> Result<Vec<u8>> {
        let mut walk = self.repo.revwalk().ctx("montar o envio")?;
        walk.push(prepared.commit).ctx("montar o envio")?;
        if let Some(remote) = remote_has
            && self.repo.find_commit(remote).is_ok()
        {
            walk.hide(remote).ctx("montar o envio")?;
        }
        let mut builder = self.repo.packbuilder().ctx("montar o envio")?;
        builder.insert_walk(&mut walk).ctx("montar o envio")?;
        builder
            .insert_object(prepared.tag, None)
            .ctx("montar o envio")?;
        let mut buffer = git2::Buf::new();
        builder.write_buf(&mut buffer).ctx("montar o envio")?;
        Ok(buffer.to_vec())
    }

    /// Grava as referências de uma publicação já enviada: a ponta da linha, a tag de
    /// publicação e a marca de Release pendente (apagada por
    /// [`PackRepo::record_release`]).
    ///
    /// Erros: falhas do git.
    pub fn record_publication(&self, prepared: &PreparedPublication) -> Result<()> {
        let name = tag_name(&prepared.version);
        self.repo
            .reference(
                PUBLISH_LINE_REF,
                prepared.commit,
                true,
                "warden: publicação",
            )
            .ctx("registrar a publicação")?;
        self.repo
            .reference(
                &format!("{PUBLISH_TAG_PREFIX}{name}"),
                prepared.tag,
                true,
                "warden: versão publicada",
            )
            .ctx("registrar a publicação")?;
        self.repo
            .reference(
                &format!("{RELEASE_PENDING_PREFIX}{name}"),
                prepared.tag,
                true,
                "warden: Release pendente",
            )
            .ctx("registrar a publicação")?;
        Ok(())
    }

    /// Marca a Release da versão como criada.
    ///
    /// Erros: falhas do git.
    pub fn record_release(&self, version: &Version) -> Result<()> {
        match self
            .repo
            .find_reference(&format!("{RELEASE_PENDING_PREFIX}{}", tag_name(version)))
        {
            Ok(mut reference) => reference.delete().ctx("registrar a Release"),
            Err(error) if error.code() == git2::ErrorCode::NotFound => Ok(()),
            Err(error) => Err(Error::git("registrar a Release", error)),
        }
    }

    /// Nome e notas da Release de uma versão publicada (para "Tentar de novo").
    ///
    /// Erros: [`Error::VersionNotFound`] se a versão não foi publicada; falhas do git.
    pub fn published_release_text(&self, version: &Version) -> Result<(String, String)> {
        let reference = self
            .repo
            .find_reference(&format!("{PUBLISH_TAG_PREFIX}{}", tag_name(version)))
            .map_err(|_| Error::VersionNotFound {
                version: version.to_string(),
            })?;
        let tag = reference
            .peel(ObjectType::Tag)
            .ctx("ler a publicação")?
            .into_tag()
            .map_err(|_| Error::Internal("publicação sem tag anotada".into()))?;
        let name = tag
            .target()
            .ctx("ler a publicação")?
            .peel_to_commit()
            .ctx("ler a publicação")?
            .summary()
            .ok()
            .flatten()
            .unwrap_or_default()
            .to_owned();
        let message = String::from_utf8_lossy(tag.message_bytes().unwrap_or_default())
            .trim_end()
            .to_owned();
        let body = if message == name {
            String::new()
        } else {
            message
        };
        Ok((name, body))
    }

    /// Conteúdo de um arquivo num commit.
    fn blob_at(&self, commit: Oid, path: &str) -> Result<Option<Vec<u8>>> {
        let tree = self
            .repo
            .find_commit(commit)
            .ctx("ler a publicação anterior")?
            .tree()
            .ctx("ler a publicação anterior")?;
        match tree.get_path(std::path::Path::new(path)) {
            Ok(entry) => Ok(Some(
                self.repo
                    .find_blob(entry.id())
                    .ctx("ler a publicação anterior")?
                    .content()
                    .to_vec(),
            )),
            Err(error) if error.code() == git2::ErrorCode::NotFound => Ok(None),
            Err(error) => Err(Error::git("ler a publicação anterior", error)),
        }
    }

    /// Grava os blobs e as árvores de `files` (caminhos com `/`) e devolve a árvore raiz.
    fn write_tree(&self, files: &BTreeMap<&str, &[u8]>) -> Result<Oid> {
        #[derive(Default)]
        struct Dir<'a> {
            files: BTreeMap<&'a str, Oid>,
            dirs: BTreeMap<&'a str, Dir<'a>>,
        }
        fn build(repo: &git2::Repository, dir: &Dir<'_>) -> Result<Oid> {
            let mut builder = repo.treebuilder(None).ctx("montar a árvore publicada")?;
            for (name, id) in &dir.files {
                builder
                    .insert(name, *id, 0o100_644)
                    .ctx("montar a árvore publicada")?;
            }
            for (name, sub) in &dir.dirs {
                let id = build(repo, sub)?;
                builder
                    .insert(name, id, 0o040_000)
                    .ctx("montar a árvore publicada")?;
            }
            builder.write().ctx("montar a árvore publicada")
        }
        let mut root = Dir::default();
        for (path, bytes) in files {
            let blob = self.repo.blob(bytes).ctx("gravar um arquivo publicado")?;
            let mut segments: Vec<&str> = path.split('/').collect();
            let Some(file) = segments.pop() else { continue };
            if file.is_empty()
                || segments
                    .iter()
                    .any(|s| s.is_empty() || *s == "." || *s == "..")
            {
                return Err(Error::Internal(format!("caminho inválido: {path}")));
            }
            let mut dir = &mut root;
            for segment in segments {
                dir = dir.dirs.entry(segment).or_default();
            }
            dir.files.insert(file, blob);
        }
        build(&self.repo, &root)
    }
}
