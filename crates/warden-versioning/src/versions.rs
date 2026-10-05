//! Versões salvas: tags anotadas `vX.Y.Z` (ARCHITECTURE §11).
//!
//! - "Salvar versão" grava um commit "Versão X.Y.Z" na branch atual e a tag anotada
//!   `vX.Y.Z` com a mensagem que a V-02 monta (o changelog da versão).
//! - A versão nova precisa ser maior que **todas** as já salvas (CA-T16-04), mesmo as que não
//!   são alcançáveis a partir do HEAD.
//! - "Última versão salva" é a maior tag `v<SemVer>` alcançável a partir do HEAD (base do
//!   changelog e das comparações).
//! - Versão final: `refs/warden/final/vX.Y.Z`; publicada: `refs/warden/publish-tags/vX.Y.Z`
//!   (gravada pela V-03). Marcar e desmarcar não mexe na árvore de trabalho.

use git2::{ErrorCode, ObjectType, Oid};
use semver::Version;
use serde::{Deserialize, Serialize};

use crate::error::{Error, GitContext as _, Result};
use crate::moment::Moment;
use crate::repo::{Identity, PackRepo};

/// Prefixo das referências de versão final.
pub const FINAL_REF_PREFIX: &str = "refs/warden/final/";

/// Prefixo das tags de publicação (escritas pela V-03).
pub const PUBLISH_TAG_PREFIX: &str = "refs/warden/publish-tags/";

/// Uma versão salva, como o Histórico mostra (SPEC T17).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SavedVersion {
    /// Número da versão (`1.3.0`).
    pub version: String,
    /// Commit da versão, em hexadecimal.
    pub commit: String,
    /// Data da tag (ou do commit, em tags leves feitas por fora), RFC 3339.
    pub date: String,
    /// Mensagem da tag (o changelog da versão); vazia em tags leves.
    pub message: String,
    /// Marcada como versão final.
    pub is_final: bool,
    /// Já publicada para os jogadores (V-03).
    pub is_published: bool,
    /// Alcançável a partir do HEAD (falso para tags de outras branches de packs importados).
    pub reachable: bool,
}

/// Pedido de "Salvar versão".
#[derive(Debug, Clone, Copy)]
pub struct SaveVersion<'a> {
    /// Número da versão, `SemVer` sem o `v` (`1.3.0`).
    pub version: &'a str,
    /// Mensagem da tag anotada (o changelog da versão).
    pub tag_message: &'a str,
    /// Quem assina.
    pub identity: &'a Identity,
    /// Quando.
    pub when: Moment,
    /// Já marcar como versão final.
    pub mark_final: bool,
}

/// Lê um número de versão `SemVer` (`1.3.0`, `2.0.0-beta.1`), sem o `v`.
///
/// Erros: [`Error::InvalidVersion`].
pub fn parse_version(text: &str) -> Result<Version> {
    Version::parse(text).map_err(|error| Error::InvalidVersion {
        version: text.to_owned(),
        reason: error.to_string(),
    })
}

/// Nome da tag de uma versão.
#[must_use]
pub fn tag_name(version: &Version) -> String {
    format!("v{version}")
}

/// Uma tag de versão encontrada no repositório.
#[derive(Debug, Clone)]
pub(crate) struct VersionTag {
    pub(crate) version: Version,
    pub(crate) commit: Oid,
    pub(crate) date: Moment,
    pub(crate) message: String,
}

impl PackRepo {
    /// Todas as tags `v<SemVer>` (anotadas ou leves), da maior para a menor. Tags com outro
    /// formato são ignoradas.
    pub(crate) fn version_tags(&self) -> Result<Vec<VersionTag>> {
        let names = self.repo.tag_names(Some("v*")).ctx("listar as versões")?;
        let mut tags = Vec::new();
        for name in names
            .iter_bytes()
            .filter_map(|name| std::str::from_utf8(name).ok())
        {
            let Some(text) = name.strip_prefix('v') else {
                continue;
            };
            let Ok(version) = Version::parse(text) else {
                continue;
            };
            let object = self
                .repo
                .revparse_single(&format!("refs/tags/{name}"))
                .ctx("ler uma versão")?;
            let (date, message) = match object.as_tag() {
                Some(tag) => (
                    tag.tagger().map(|who| Moment::from_git(who.when())),
                    String::from_utf8_lossy(tag.message_bytes().unwrap_or_default()).into_owned(),
                ),
                None => (None, String::new()),
            };
            let Ok(commit) = object.peel_to_commit() else {
                continue;
            };
            tags.push(VersionTag {
                version,
                date: date.unwrap_or_else(|| Moment::from_git(commit.time())),
                commit: commit.id(),
                message,
            });
        }
        tags.sort_by(|a, b| b.version.cmp(&a.version));
        Ok(tags)
    }

    fn reachable_from_head(&self, commit: Oid) -> Result<bool> {
        let Some(head) = self.head_commit()? else {
            return Ok(false);
        };
        Ok(head.id() == commit
            || self
                .repo
                .graph_descendant_of(head.id(), commit)
                .ctx("comparar versões")?)
    }

    /// Versões salvas, da maior para a menor (SPEC T17).
    pub fn versions(&self) -> Result<Vec<SavedVersion>> {
        let mut versions = Vec::new();
        for tag in self.version_tags()? {
            let name = tag_name(&tag.version);
            versions.push(SavedVersion {
                version: tag.version.to_string(),
                commit: tag.commit.to_string(),
                date: tag.date.rfc3339(),
                message: tag.message,
                is_final: self
                    .reference_commit(&format!("{FINAL_REF_PREFIX}{name}"))?
                    .is_some(),
                is_published: self
                    .reference_commit(&format!("{PUBLISH_TAG_PREFIX}{name}"))?
                    .is_some(),
                reachable: self.reachable_from_head(tag.commit)?,
            });
        }
        Ok(versions)
    }

    /// Última versão salva: a maior tag de versão alcançável a partir do HEAD (`None` se não
    /// houver).
    pub fn last_version(&self) -> Result<Option<Version>> {
        Ok(self.last_version_tag()?.map(|tag| tag.version))
    }

    pub(crate) fn last_version_tag(&self) -> Result<Option<VersionTag>> {
        for tag in self.version_tags()? {
            if self.reachable_from_head(tag.commit)? {
                return Ok(Some(tag));
            }
        }
        Ok(None)
    }

    /// Maior versão já salva no repositório, alcançável ou não.
    pub fn highest_version(&self) -> Result<Option<Version>> {
        Ok(self
            .version_tags()?
            .into_iter()
            .next()
            .map(|tag| tag.version))
    }

    /// Confere se `version` pode ser salva agora (CA-T16-04): `SemVer` válido, maior que todas as
    /// versões já salvas e sem tag com o mesmo nome.
    ///
    /// Erros: [`Error::InvalidVersion`], [`Error::VersionExists`], [`Error::VersionNotGreater`].
    pub fn validate_new_version(&self, version: &str) -> Result<Version> {
        let parsed = parse_version(version)?;
        let tag = tag_name(&parsed);
        match self.repo.find_reference(&format!("refs/tags/{tag}")) {
            Ok(_) => {
                return Err(Error::VersionExists {
                    version: parsed.to_string(),
                });
            }
            Err(error) if error.code() == ErrorCode::NotFound => {}
            Err(error) => return Err(Error::git("ler uma versão", error)),
        }
        if let Some(highest) = self.highest_version()?
            && parsed <= highest
        {
            return Err(Error::VersionNotGreater {
                version: parsed.to_string(),
                last: highest.to_string(),
            });
        }
        Ok(parsed)
    }

    /// Salva a versão: põe tudo no índice, grava o commit "Versão X.Y.Z" na branch atual e a
    /// tag anotada `vX.Y.Z`; com `mark_final`, marca também como versão final. Depois disso
    /// não há alterações não salvas (CA-T16-03).
    ///
    /// Quem chama já gravou a versão no `pack.toml` e a entrada do `CHANGELOG.md` (V-02, pela
    /// `PackTransaction`). Se a tag falhar depois do commit, a branch volta para onde estava.
    ///
    /// Erros: os de [`PackRepo::check_writable`] e [`PackRepo::validate_new_version`];
    /// [`Error::NothingChanged`] se a árvore for igual à do último commit.
    pub fn save_version(&self, request: &SaveVersion<'_>) -> Result<SavedVersion> {
        self.check_writable()?;
        let version = self.validate_new_version(request.version)?;
        let parent = self.head_commit()?;
        let tree_id = self.stage_all()?;
        if parent.as_ref().map(git2::Commit::tree_id) == Some(tree_id) {
            return Err(Error::NothingChanged {
                last: self.last_version()?.map(|v| v.to_string()),
            });
        }
        let tree = self.repo.find_tree(tree_id).ctx("ler a árvore da versão")?;
        let signature = request.identity.signature(request.when)?;
        let parents: Vec<&git2::Commit<'_>> = parent.iter().collect();
        let commit_id = self
            .repo
            .commit(
                Some("HEAD"),
                &signature,
                &signature,
                &format!("Versão {version}"),
                &tree,
                &parents,
            )
            .ctx("gravar a versão")?;
        let name = tag_name(&version);
        let tagged = self
            .repo
            .find_object(commit_id, Some(ObjectType::Commit))
            .ctx("ler o commit da versão")
            .and_then(|object| {
                self.repo
                    .tag(&name, &object, &signature, request.tag_message, false)
                    .ctx("gravar a tag da versão")
            });
        if let Err(error) = tagged {
            self.undo_commit(parent.as_ref().map(git2::Commit::id));
            return Err(error);
        }
        if request.mark_final {
            self.set_final(&version.to_string(), true)?;
        }
        Ok(SavedVersion {
            version: version.to_string(),
            commit: commit_id.to_string(),
            date: request.when.rfc3339(),
            message: request.tag_message.to_owned(),
            is_final: request.mark_final,
            is_published: false,
            reachable: true,
        })
    }

    /// Devolve a branch ao commit anterior depois de uma falha (melhor esforço: se falhar, a
    /// versão fica sem tag e o próximo "Salvar versão" grava outra por cima).
    fn undo_commit(&self, previous: Option<Oid>) {
        let Ok(head) = self.repo.find_reference("HEAD") else {
            return;
        };
        let Some(branch) = head
            .symbolic_target_bytes()
            .and_then(|target| std::str::from_utf8(target).ok())
            .map(ToOwned::to_owned)
        else {
            return;
        };
        // Melhor esforço, como documentado acima: o erro original é o que importa.
        let _ = match previous {
            Some(id) => self
                .repo
                .reference(&branch, id, true, "warden: desfaz versão sem tag")
                .map(|_| ()),
            None => self
                .repo
                .find_reference(&branch)
                .and_then(|mut reference| reference.delete()),
        };
    }

    /// Commit de uma versão salva.
    ///
    /// Erros: [`Error::InvalidVersion`], [`Error::VersionNotFound`].
    pub(crate) fn version_commit(&self, version: &str) -> Result<Oid> {
        let parsed = parse_version(version)?;
        match self
            .repo
            .revparse_single(&format!("refs/tags/{}", tag_name(&parsed)))
        {
            Ok(object) => Ok(object.peel_to_commit().ctx("ler o commit da versão")?.id()),
            Err(error) if error.code() == ErrorCode::NotFound => Err(Error::VersionNotFound {
                version: parsed.to_string(),
            }),
            Err(error) => Err(Error::git("ler uma versão", error)),
        }
    }

    /// Marca (`true`) ou desmarca (`false`) uma versão salva como versão final. Não mexe em
    /// nenhum arquivo do pack (CA-T16-05).
    ///
    /// Erros: [`Error::VersionNotFound`]; [`Error::VersionPublished`] ao desmarcar uma versão
    /// já publicada.
    pub fn set_final(&self, version: &str, is_final: bool) -> Result<()> {
        let commit = self.version_commit(version)?;
        let name = tag_name(&parse_version(version)?);
        let final_ref = format!("{FINAL_REF_PREFIX}{name}");
        if is_final {
            self.repo
                .reference(&final_ref, commit, true, "warden: versão final")
                .ctx("marcar a versão final")?;
            return Ok(());
        }
        if self
            .reference_commit(&format!("{PUBLISH_TAG_PREFIX}{name}"))?
            .is_some()
        {
            return Err(Error::VersionPublished {
                version: version.to_owned(),
            });
        }
        match self.repo.find_reference(&final_ref) {
            Ok(mut reference) => reference.delete().ctx("desmarcar a versão final"),
            Err(error) if error.code() == ErrorCode::NotFound => Ok(()),
            Err(error) => Err(Error::git("desmarcar a versão final", error)),
        }
    }
}
