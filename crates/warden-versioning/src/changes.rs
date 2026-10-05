//! Changelog estruturado: o que mudou nos itens do pack entre dois estados (ARCHITECTURE §11;
//! SPEC T16 e T17).
//!
//! Compara os metafiles `.pw.toml` de um lado (lidos dos blobs da versão) com os do outro
//! (blobs de outra versão ou a pasta do pack):
//!
//! - o mesmo projeto é reconhecido por `[update.modrinth].mod-id` ou
//!   `[update.curseforge].project-id`; sem fonte de atualização (link ou arquivo local), pelo
//!   caminho;
//! - mesmo projeto com outra versão (`version` do Modrinth, `file-id` da CurseForge, hash ou
//!   nome do arquivo) → **atualizado**; só nos novos → **adicionado**; só nos antigos →
//!   **removido**; mesma versão com o metafile diferente (lado, opcional…) → **ajustado**;
//! - arquivos soltos em `mods/`, `resourcepacks/` e `shaderpacks/` (sem metafile) são itens
//!   locais, reconhecidos pelo caminho;
//! - **configs**: os demais arquivos alterados, fora os de controle (`pack.toml`, índice,
//!   `CHANGELOG.md`, `README.md`, `.gitignore`, `.gitattributes`, `.packwizignore`,
//!   `.warden/`);
//! - **Minecraft e loader**: `[versions]` do `pack.toml` dos dois lados.
//!
//! Nomes de versão legíveis vêm do [`VersionNameResolver`] (implementado pela `warden-app` com
//! o cache do Modrinth e a API da CurseForge); o recuo é o nome do arquivo.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs;
use std::io;

use git2::ObjectType;
use serde::{Deserialize, Serialize};
use warden_packwiz::{METAFILE_SUFFIX, Metafile, PackManifest, Side as PackSide};

use crate::error::{Error, GitContext as _, Result};
use crate::repo::PackRepo;
use crate::versions::parse_version;
use crate::worktree::{FileChange, FileChangeKind, Side};

/// Tipo de item, pela pasta.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, specta::Type,
)]
#[serde(rename_all = "camelCase")]
pub enum ItemCategory {
    /// Mod (qualquer item fora de `resourcepacks/` e `shaderpacks/`).
    Mod,
    /// Resource pack (`resourcepacks/`).
    ResourcePack,
    /// Shader (`shaderpacks/`).
    Shader,
}

/// De onde o item vem.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum ItemSource {
    /// Modrinth (`[update.modrinth]`).
    Modrinth,
    /// CurseForge (`[update.curseforge]`).
    CurseForge,
    /// Link direto (metafile sem fonte de atualização).
    Url,
    /// Arquivo local dentro do pack (sem metafile).
    Local,
    /// Metafile que não pôde ser lido.
    Unknown,
}

/// O que aconteceu com o item.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, specta::Type,
)]
#[serde(rename_all = "camelCase")]
pub enum ItemChangeKind {
    /// Novo no pack.
    Added,
    /// Saiu do pack.
    Removed,
    /// Outra versão do mesmo projeto.
    Updated,
    /// Mesma versão, com o metafile alterado (lado, opcional, fixar versão…).
    Adjusted,
}

/// Identificação de uma versão de item, para o [`VersionNameResolver`].
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum VersionRef {
    /// Versão do Modrinth.
    #[serde(rename_all = "camelCase")]
    Modrinth {
        /// ID do projeto.
        project_id: String,
        /// ID da versão.
        version_id: String,
    },
    /// Arquivo da CurseForge.
    #[serde(rename_all = "camelCase")]
    CurseForge {
        /// ID do projeto.
        project_id: u32,
        /// ID do arquivo.
        file_id: u32,
    },
    /// Só o arquivo (link direto ou arquivo local).
    #[serde(rename_all = "camelCase")]
    File {
        /// Nome do arquivo.
        filename: String,
    },
}

/// Uma versão de item com o nome legível.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ItemVersion {
    /// Identificação da versão.
    pub reference: VersionRef,
    /// Nome do arquivo.
    pub filename: String,
    /// Nome legível ("0.6.0"); o nome do arquivo quando a fonte não informa.
    pub label: String,
}

/// Um item que mudou.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ItemChange {
    /// O que aconteceu.
    pub kind: ItemChangeKind,
    /// Tipo, pela pasta.
    pub category: ItemCategory,
    /// De onde vem.
    pub source: ItemSource,
    /// Nome exibido (título do projeto; nome do arquivo para itens locais).
    pub name: String,
    /// Caminho do metafile ou do arquivo (o novo; o antigo para removidos).
    pub path: String,
    /// ID do projeto no Modrinth ou na CurseForge.
    pub project_id: Option<String>,
    /// Lado do item (`both`, `client`, `server`; vazio = `both`), do lado novo ou, para
    /// removidos, do antigo.
    pub side: String,
    /// Versão antes (removidos, atualizados e ajustados).
    pub old: Option<ItemVersion>,
    /// Versão depois (adicionados, atualizados e ajustados).
    pub new: Option<ItemVersion>,
}

impl ItemChange {
    /// Só cliente (`side = "client"`): removê-lo não apaga nada dos mundos.
    #[must_use]
    pub fn is_client_only(&self) -> bool {
        self.side == "client"
    }
}

/// Uma mudança de valor (antes e depois; `None` = ausente).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ValueChange {
    /// Antes.
    pub old: Option<String>,
    /// Depois.
    pub new: Option<String>,
}

/// Mudança de um loader em `[versions]`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct LoaderChange {
    /// Chave em `[versions]` (`fabric`, `neoforge`…).
    pub loader: String,
    /// Antes e depois.
    pub change: ValueChange,
}

/// Tudo o que mudou entre dois estados do pack.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ChangeSet {
    /// Versão do Minecraft, se mudou.
    pub minecraft: Option<ValueChange>,
    /// Loaders que mudaram.
    pub loaders: Vec<LoaderChange>,
    /// Itens que mudaram, por tipo, mudança e nome.
    pub items: Vec<ItemChange>,
    /// Configs alteradas (caminhos).
    pub configs: Vec<String>,
    /// Todos os arquivos alterados.
    pub files: Vec<FileChange>,
}

impl ChangeSet {
    /// Nada mudou.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }

    /// Itens de um tipo de mudança.
    pub fn items_of(&self, kind: ItemChangeKind) -> impl Iterator<Item = &ItemChange> {
        self.items.iter().filter(move |item| item.kind == kind)
    }

    /// Mods removidos que não são "só cliente" (podem apagar blocos ou itens dos mundos;
    /// SPEC T16).
    pub fn removed_world_mods(&self) -> impl Iterator<Item = &ItemChange> {
        self.items_of(ItemChangeKind::Removed)
            .filter(|item| item.category == ItemCategory::Mod && !item.is_client_only())
    }
}

/// Nomes legíveis das versões, implementado pela `warden-app` (cache do Modrinth e API da
/// CurseForge no momento de salvar; ARCHITECTURE §3 e §11).
pub trait VersionNameResolver {
    /// Nome legível de cada versão pedida, na mesma ordem (`None` = desconhecido; o
    /// changelog usa o nome do arquivo).
    fn version_names(&self, versions: &[VersionRef]) -> Vec<Option<String>>;
}

/// Resolver que não conhece nenhum nome: o changelog usa o nome do arquivo.
#[derive(Debug, Clone, Copy, Default)]
pub struct FileNames;

impl VersionNameResolver for FileNames {
    fn version_names(&self, versions: &[VersionRef]) -> Vec<Option<String>> {
        vec![None; versions.len()]
    }
}

/// Um lado da comparação para quem chama.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Snapshot {
    /// Antes de existir qualquer arquivo (base da primeira versão).
    Empty,
    /// Uma versão salva (`1.2.0`).
    Version(String),
    /// Um ponto de segurança, pelo nome.
    SafetyPoint(String),
    /// A pasta do pack como está agora.
    WorkingTree,
}

/// Arquivos de controle, que não entram na lista de configs.
fn is_control_file(path: &str, index_file: &str) -> bool {
    matches!(
        path,
        "pack.toml"
            | "CHANGELOG.md"
            | "README.md"
            | ".gitignore"
            | ".gitattributes"
            | ".packwizignore"
    ) || path == index_file
        || path.starts_with(".warden/")
}

fn category_of(path: &str) -> Option<ItemCategory> {
    let first = path.split('/').next().unwrap_or_default();
    match first {
        "mods" => Some(ItemCategory::Mod),
        "resourcepacks" => Some(ItemCategory::ResourcePack),
        "shaderpacks" => Some(ItemCategory::Shader),
        _ => None,
    }
}

fn file_name(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

/// Um item num lado.
#[derive(Debug, Clone)]
struct Item {
    key: String,
    path: String,
    category: ItemCategory,
    source: ItemSource,
    name: String,
    project_id: Option<String>,
    side: String,
    reference: VersionRef,
    /// Identidade da versão (muda quando a versão muda).
    version_identity: String,
    filename: String,
    bytes: Vec<u8>,
}

fn metafile_item(path: &str, bytes: Vec<u8>) -> Item {
    let category = category_of(path).unwrap_or(ItemCategory::Mod);
    let parsed = std::str::from_utf8(&bytes)
        .ok()
        .and_then(|text| Metafile::parse(text).ok())
        .map(|parsed| parsed.value);
    let Some(meta) = parsed else {
        let stem = file_name(path).trim_end_matches(METAFILE_SUFFIX).to_owned();
        return Item {
            key: format!("path:{path}"),
            path: path.to_owned(),
            category,
            source: ItemSource::Unknown,
            name: stem.clone(),
            project_id: None,
            side: String::new(),
            reference: VersionRef::File {
                filename: stem.clone(),
            },
            version_identity: format!("bytes:{}", content_id(&bytes)),
            filename: stem,
            bytes,
        };
    };
    let side = match &meta.side {
        PackSide::Unset => String::new(),
        other => other.as_str().to_owned(),
    };
    let name = if meta.name.trim().is_empty() {
        meta.filename.clone()
    } else {
        meta.name.clone()
    };
    let (key, source, project_id, reference, version_identity) =
        if let Some((project, version)) = meta.modrinth_ids() {
            (
                format!("modrinth:{project}"),
                ItemSource::Modrinth,
                Some(project.to_owned()),
                VersionRef::Modrinth {
                    project_id: project.to_owned(),
                    version_id: version.to_owned(),
                },
                format!("modrinth:{version}"),
            )
        } else if let Some((project, file)) = meta.curseforge_ids() {
            (
                format!("curseforge:{project}"),
                ItemSource::CurseForge,
                Some(project.to_string()),
                VersionRef::CurseForge {
                    project_id: project,
                    file_id: file,
                },
                format!("curseforge:{file}"),
            )
        } else {
            (
                format!("path:{path}"),
                ItemSource::Url,
                None,
                VersionRef::File {
                    filename: meta.filename.clone(),
                },
                format!("file:{}:{}", meta.download.hash, meta.filename),
            )
        };
    Item {
        key,
        path: path.to_owned(),
        category,
        source,
        name,
        project_id,
        side,
        reference,
        version_identity,
        filename: meta.filename,
        bytes,
    }
}

fn local_item(path: &str, category: ItemCategory, bytes: Vec<u8>) -> Item {
    let filename = file_name(path).to_owned();
    Item {
        key: format!("path:{path}"),
        path: path.to_owned(),
        category,
        source: ItemSource::Local,
        name: filename.clone(),
        project_id: None,
        side: String::new(),
        reference: VersionRef::File {
            filename: filename.clone(),
        },
        version_identity: format!("bytes:{}", content_id(&bytes)),
        filename,
        bytes,
    }
}

/// Id do conteúdo (o mesmo hash que o git daria ao arquivo), para comparar versões de itens
/// locais.
fn content_id(bytes: &[u8]) -> String {
    git2::Oid::hash_object(ObjectType::Blob, bytes)
        .map_or_else(|_| format!("{}-bytes", bytes.len()), |oid| oid.to_string())
}

/// Maior arquivo lido inteiro para montar o changelog (metafiles e `pack.toml` são
/// pequenos; o limite protege contra arquivos estranhos com o mesmo nome).
const MAX_TEXT_FILE: u64 = 4 * 1024 * 1024;

/// Lado de uma comparação já resolvido.
struct Loaded {
    pack: Option<PackManifest>,
    items: BTreeMap<String, Item>,
}

impl PackRepo {
    /// Changelog estruturado de `old` para `new` (SPEC T16: alterações para salvar; T17: "Ver
    /// diferenças para o estado atual").
    ///
    /// Erros: [`crate::Error::VersionNotFound`], [`crate::Error::SafetyPointNotFound`],
    /// falhas de leitura.
    pub fn changes(
        &self,
        old: &Snapshot,
        new: &Snapshot,
        resolver: &dyn VersionNameResolver,
    ) -> Result<ChangeSet> {
        let old = self.resolve_snapshot(old)?;
        let new = self.resolve_snapshot(new)?;
        let files = self.diff(old, new)?;
        // Arquivos adicionados não existem do lado antigo, nem os apagados do lado novo.
        let old_loaded = self.load_side(old, &files, FileChangeKind::Added)?;
        let new_loaded = self.load_side(new, &files, FileChangeKind::Deleted)?;

        let index_file = new_loaded
            .pack
            .as_ref()
            .or(old_loaded.pack.as_ref())
            .map_or_else(|| "index.toml".to_owned(), |pack| pack.index.file.clone());
        let configs = files
            .iter()
            .filter(|change| {
                category_of(&change.path).is_none()
                    && !change.path.ends_with(METAFILE_SUFFIX)
                    && !is_control_file(&change.path, &index_file)
            })
            .map(|change| change.path.clone())
            .collect();

        let (minecraft, loaders) =
            versions_changes(old_loaded.pack.as_ref(), new_loaded.pack.as_ref());
        let mut items = match_items(old_loaded.items, new_loaded.items);
        resolve_names(&mut items, resolver);
        items.sort_by(|a, b| {
            (a.category, a.kind, a.name.to_lowercase(), &a.path).cmp(&(
                b.category,
                b.kind,
                b.name.to_lowercase(),
                &b.path,
            ))
        });
        Ok(ChangeSet {
            minecraft,
            loaders,
            items,
            configs,
            files,
        })
    }

    /// Changelog das alterações não salvas, contra a última versão salva (ou contra o vazio,
    /// antes da primeira versão: tudo aparece como adicionado).
    pub fn changes_since_last_version(
        &self,
        resolver: &dyn VersionNameResolver,
    ) -> Result<ChangeSet> {
        let base = match self.last_version()? {
            Some(version) => Snapshot::Version(version.to_string()),
            None => Snapshot::Empty,
        };
        self.changes(&base, &Snapshot::WorkingTree, resolver)
    }

    /// Conteúdo de um arquivo num lado (`None` se o arquivo não existe nele). Arquivos
    /// maiores que [`MAX_TEXT_FILE`] são tratados como ausentes do changelog.
    fn read_side_file(&self, side: Side, path: &str) -> Result<Option<Vec<u8>>> {
        match side {
            Side::Empty => Ok(None),
            Side::Tree(id) => {
                let tree = self.repo.find_tree(id).ctx("ler uma árvore")?;
                let entry = match tree.get_path(std::path::Path::new(path)) {
                    Ok(entry) => entry,
                    Err(error) if error.code() == git2::ErrorCode::NotFound => return Ok(None),
                    Err(error) => return Err(Error::git("ler um arquivo da versão", error)),
                };
                if entry.kind() != Some(ObjectType::Blob) {
                    return Ok(None);
                }
                let blob = self
                    .repo
                    .find_blob(entry.id())
                    .ctx("ler um arquivo da versão")?;
                if blob.size() as u64 > MAX_TEXT_FILE {
                    return Ok(None);
                }
                Ok(Some(blob.content().to_vec()))
            }
            Side::WorkingTree => {
                let absolute = warden_core::resolve_inside(&self.root, path)?;
                match fs::metadata(&absolute) {
                    Ok(meta) if meta.is_file() && meta.len() <= MAX_TEXT_FILE => {}
                    Ok(_) => return Ok(None),
                    Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
                    Err(error) => return Err(Error::io("ler", absolute, error)),
                }
                match fs::read(&absolute) {
                    Ok(bytes) => Ok(Some(bytes)),
                    Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
                    Err(error) => Err(Error::io("ler", absolute, error)),
                }
            }
        }
    }

    fn resolve_snapshot(&self, snapshot: &Snapshot) -> Result<Side> {
        Ok(match snapshot {
            Snapshot::Empty => Side::Empty,
            Snapshot::WorkingTree => Side::WorkingTree,
            Snapshot::Version(version) => {
                parse_version(version)?;
                let commit = self.version_commit(version)?;
                Side::Tree(
                    self.repo
                        .find_commit(commit)
                        .map_err(|e| crate::Error::git("ler uma versão", e))?
                        .tree_id(),
                )
            }
            Snapshot::SafetyPoint(name) => {
                let point = self.safety_point(name)?;
                let commit = git2::Oid::from_str(&point.commit)
                    .map_err(|e| crate::Error::git("ler o ponto de segurança", e))?;
                Side::Tree(
                    self.repo
                        .find_commit(commit)
                        .map_err(|e| crate::Error::git("ler o ponto de segurança", e))?
                        .tree_id(),
                )
            }
        })
    }

    /// Lê o `pack.toml` e os itens alterados de um lado.
    fn load_side(
        &self,
        side: Side,
        files: &[FileChange],
        absent: FileChangeKind,
    ) -> Result<Loaded> {
        let pack = self
            .read_side_file(side, "pack.toml")?
            .and_then(|bytes| String::from_utf8(bytes).ok())
            .and_then(|text| PackManifest::parse(&text).ok())
            .map(|parsed| parsed.value);
        let mut items = BTreeMap::new();
        for change in files {
            if change.kind == absent {
                continue;
            }
            let path = change.path.as_str();
            let is_metafile = path.ends_with(METAFILE_SUFFIX);
            let category = category_of(path);
            if !is_metafile && category.is_none() {
                continue;
            }
            let Some(bytes) = self.read_side_file(side, path)? else {
                continue;
            };
            let item = match (is_metafile, category) {
                (true, _) => metafile_item(path, bytes),
                (false, Some(category)) => local_item(path, category, bytes),
                (false, None) => continue,
            };
            let mut key = item.key.clone();
            if items.contains_key(&key) {
                // Dois metafiles do mesmo projeto: cada um vira um item próprio, pelo caminho.
                key = format!("{key}#{path}");
            }
            items.insert(key, item);
        }
        Ok(Loaded { pack, items })
    }
}

fn versions_changes(
    old: Option<&PackManifest>,
    new: Option<&PackManifest>,
) -> (Option<ValueChange>, Vec<LoaderChange>) {
    let empty = BTreeMap::new();
    let old_versions = old
        .and_then(|pack| pack.versions.as_ref())
        .unwrap_or(&empty);
    let new_versions = new
        .and_then(|pack| pack.versions.as_ref())
        .unwrap_or(&empty);
    // Sem `pack.toml` de um dos lados (primeira versão), Minecraft e loader não "mudam".
    if old.is_none() || new.is_none() {
        return (None, Vec::new());
    }
    let mut minecraft = None;
    let mut loaders = Vec::new();
    let keys: BTreeSet<&String> = old_versions.keys().chain(new_versions.keys()).collect();
    for key in keys {
        let before = old_versions.get(key).cloned();
        let after = new_versions.get(key).cloned();
        if before == after {
            continue;
        }
        let change = ValueChange {
            old: before,
            new: after,
        };
        if key == "minecraft" {
            minecraft = Some(change);
        } else {
            loaders.push(LoaderChange {
                loader: key.clone(),
                change,
            });
        }
    }
    (minecraft, loaders)
}

fn match_items(old: BTreeMap<String, Item>, mut new: BTreeMap<String, Item>) -> Vec<ItemChange> {
    let mut changes = Vec::new();
    for (key, before) in old {
        match new.remove(&key) {
            Some(after) => {
                let kind = if before.version_identity != after.version_identity {
                    ItemChangeKind::Updated
                } else if before.bytes != after.bytes {
                    ItemChangeKind::Adjusted
                } else {
                    continue;
                };
                changes.push(ItemChange {
                    kind,
                    category: after.category,
                    source: after.source,
                    name: after.name.clone(),
                    path: after.path.clone(),
                    project_id: after.project_id.clone(),
                    side: after.side.clone(),
                    old: Some(item_version(&before)),
                    new: Some(item_version(&after)),
                });
            }
            None => changes.push(ItemChange {
                kind: ItemChangeKind::Removed,
                category: before.category,
                source: before.source,
                name: before.name.clone(),
                path: before.path.clone(),
                project_id: before.project_id.clone(),
                side: before.side.clone(),
                old: Some(item_version(&before)),
                new: None,
            }),
        }
    }
    for after in new.into_values() {
        changes.push(ItemChange {
            kind: ItemChangeKind::Added,
            category: after.category,
            source: after.source,
            name: after.name.clone(),
            path: after.path.clone(),
            project_id: after.project_id.clone(),
            side: after.side.clone(),
            old: None,
            new: Some(item_version(&after)),
        });
    }
    changes
}

fn item_version(item: &Item) -> ItemVersion {
    ItemVersion {
        reference: item.reference.clone(),
        filename: item.filename.clone(),
        label: item.filename.clone(),
    }
}

/// Pede os nomes ao resolver de uma vez só (uma consulta em lote por fonte).
fn resolve_names(items: &mut [ItemChange], resolver: &dyn VersionNameResolver) {
    let mut wanted: Vec<VersionRef> = Vec::new();
    for item in items.iter() {
        for version in [&item.old, &item.new].into_iter().flatten() {
            if !matches!(version.reference, VersionRef::File { .. })
                && !wanted.contains(&version.reference)
            {
                wanted.push(version.reference.clone());
            }
        }
    }
    if wanted.is_empty() {
        return;
    }
    let names = resolver.version_names(&wanted);
    let lookup: HashMap<&VersionRef, String> = wanted
        .iter()
        .zip(names)
        .filter_map(|(reference, name)| {
            let name = name?.trim().to_owned();
            (!name.is_empty()).then_some((reference, name))
        })
        .collect();
    for item in items.iter_mut() {
        for version in [&mut item.old, &mut item.new].into_iter().flatten() {
            if let Some(name) = lookup.get(&version.reference) {
                version.label.clone_from(name);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn categorias_e_arquivos_de_controle() {
        assert_eq!(category_of("mods/a.jar"), Some(ItemCategory::Mod));
        assert_eq!(
            category_of("resourcepacks/x.pw.toml"),
            Some(ItemCategory::ResourcePack)
        );
        assert_eq!(category_of("shaderpacks/s.zip"), Some(ItemCategory::Shader));
        assert_eq!(category_of("config/a.toml"), None);
        assert!(is_control_file("pack.toml", "index.toml"));
        assert!(is_control_file("meta/index.toml", "meta/index.toml"));
        assert!(is_control_file(".warden/notes.toml", "index.toml"));
        assert!(!is_control_file("config/pack.toml", "index.toml"));
    }

    #[test]
    fn metafile_ilegivel_vira_item_desconhecido() {
        let item = metafile_item("mods/quebrado.pw.toml", b"name = [".to_vec());
        assert_eq!(item.source, ItemSource::Unknown);
        assert_eq!(item.name, "quebrado");
        assert_eq!(item.key, "path:mods/quebrado.pw.toml");
    }

    #[test]
    fn resolver_vazio_mantem_o_nome_do_arquivo() {
        let mut items = vec![ItemChange {
            kind: ItemChangeKind::Added,
            category: ItemCategory::Mod,
            source: ItemSource::Modrinth,
            name: "Sodium".into(),
            path: "mods/sodium.pw.toml".into(),
            project_id: Some("AANobbMI".into()),
            side: "client".into(),
            old: None,
            new: Some(ItemVersion {
                reference: VersionRef::Modrinth {
                    project_id: "AANobbMI".into(),
                    version_id: "abc".into(),
                },
                filename: "sodium-0.6.0.jar".into(),
                label: "sodium-0.6.0.jar".into(),
            }),
        }];
        resolve_names(&mut items, &FileNames);
        assert_eq!(items[0].new.as_ref().unwrap().label, "sodium-0.6.0.jar");
        assert!(items[0].is_client_only());
    }

    #[test]
    fn versoes_sem_pack_toml_de_um_lado_nao_mudam() {
        let pack = PackManifest::new("P", "1.21.1");
        assert_eq!(versions_changes(None, Some(&pack)), (None, Vec::new()));
        let mut other = pack.clone();
        other.set_loader_version(warden_packwiz::Loader::Fabric, "0.16.0");
        other
            .versions
            .as_mut()
            .unwrap()
            .insert("minecraft".into(), "1.21.4".into());
        let (minecraft, loaders) = versions_changes(Some(&pack), Some(&other));
        assert_eq!(
            minecraft,
            Some(ValueChange {
                old: Some("1.21.1".into()),
                new: Some("1.21.4".into())
            })
        );
        assert_eq!(loaders.len(), 1);
        assert_eq!(loaders[0].loader, "fabric");
        assert_eq!(loaders[0].change.old, None);
    }
}
