//! Inventário do pack: a lista única de mods, resource packs e shaders, guiada pelo índice e
//! tolerante a erros (SPEC T06; CA-T06-01 e CA-T06-02).
//!
//! O índice manda: cada metafile listado vira um item (válido ou com o erro só dele), cada
//! arquivo `.jar`/`.zip` indexado direto numa pasta de conteúdo vira um arquivo local, e os
//! arquivos dessas pastas que o índice não lista (nem o `.packwizignore` exclui) aparecem como
//! "Fora do índice". A versão mostrada é sempre legível: o número da versão do Modrinth (cache
//! primeiro, rede só para o que falta) ou o nome do arquivo; nunca um ID interno.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::path::Path;

use serde::Serialize;
use warden_modrinth::{ModrinthClient, Project, Version};
use warden_packwiz::{
    METAFILE_SUFFIX, Metafile, PackwizIgnore, Side, check_relative_path, clean_path, read_pack,
};

use crate::{Error, ProjectErrorCode as Code, Result};

/// Tipo do item, pela pasta em que está.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum ItemKind {
    /// `mods/`.
    Mod,
    /// `resourcepacks/`.
    ResourcePack,
    /// `shaderpacks/`.
    Shader,
    /// Qualquer outra pasta.
    Other,
}

/// De onde o arquivo vem.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum ItemSource {
    /// Metafile com `[update.modrinth]`.
    Modrinth,
    /// Metafile com `[update.curseforge]` ou `mode = "metadata:curseforge"`.
    Curseforge,
    /// Metafile só com o link do download.
    Url,
    /// Arquivo guardado no próprio pack (indexado direto ou fora do índice).
    Local,
}

/// Lado em que o item é instalado.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum ItemSide {
    /// Cliente e servidor (inclusive `side` ausente).
    Both,
    /// Só cliente.
    Client,
    /// Só servidor.
    Server,
    /// Valor que o packwiz não conhece, ou metafile ilegível.
    Unknown,
}

impl From<&Side> for ItemSide {
    fn from(side: &Side) -> Self {
        match side {
            Side::Unset | Side::Both => Self::Both,
            Side::Client => Self::Client,
            Side::Server => Self::Server,
            Side::Other(_) => Self::Unknown,
        }
    }
}

/// Situação do arquivo do item.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum ItemState {
    /// Lido sem problemas.
    Ok,
    /// O metafile não pôde ser lido; o detalhe está em [`InventoryItem::error`].
    Invalid,
    /// Arquivo na pasta que o índice não lista.
    OutsideIndex,
}

/// Uma linha do inventário.
#[derive(Debug, Clone, PartialEq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct InventoryItem {
    /// Chave estável (gancho 1.1, ADR-0039): `modrinth:<projeto>`, `curseforge:<projeto>` ou
    /// `path:<caminho>`.
    pub key: String,
    /// Caminho relativo à pasta do pack, com `/`: o `.pw.toml`, ou o próprio arquivo.
    pub path: String,
    /// Tipo, pela pasta.
    pub kind: ItemKind,
    /// Situação do arquivo.
    pub state: ItemState,
    /// Nome: o `name` do metafile; o nome do `.pw.toml` se ele for inválido; o nome do
    /// arquivo se for local.
    pub name: String,
    /// Nome do arquivo baixado ou guardado (`None` num metafile inválido).
    pub file_name: Option<String>,
    /// Versão legível: o número da versão do Modrinth ou o nome do arquivo sem `.jar`/`.zip`.
    /// Nunca o ID do Modrinth nem o file-id da CurseForge (CA-T06-02).
    pub version: Option<String>,
    /// Fonte.
    pub source: ItemSource,
    /// Lado.
    pub side: ItemSide,
    /// Se o lado pode ser mudado (só metafile válido).
    pub side_editable: bool,
    /// Versão fixada (`pin = true`).
    pub pinned: bool,
    /// Opcional para o jogador (`[option] optional = true`).
    pub optional: bool,
    /// ID do projeto no Modrinth ou na CurseForge.
    pub project_id: Option<String>,
    /// ID da versão do Modrinth ou do arquivo da CurseForge instalado. Uso interno (dependências,
    /// troca de versão); **nunca** deve ser mostrado como versão.
    pub source_version_id: Option<String>,
    /// Resumo do projeto (Modrinth), quando houver.
    pub summary: Option<String>,
    /// Ícone do projeto (Modrinth), quando houver.
    pub icon_url: Option<String>,
    /// Detalhe técnico do arquivo inválido.
    pub error: Option<String>,
}

/// O inventário do pack.
#[derive(Debug, Clone, PartialEq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct Inventory {
    /// Itens, por tipo (mods, resource packs, shaders, outros) e nome, sem diferenciar
    /// maiúsculas.
    pub items: Vec<InventoryItem>,
    /// Erro do índice ilegível (a lista fica vazia).
    pub index_error: Option<String>,
}

/// Pastas de conteúdo varridas atrás de arquivos fora do índice.
const CONTENT_FOLDERS: [&str; 3] = ["mods", "resourcepacks", "shaderpacks"];

/// Leitura síncrona do pack, sem metadados de rede, com o que as outras operações precisam.
#[derive(Debug)]
pub(crate) struct Scan {
    /// Itens sem os metadados do Modrinth, já ordenados.
    pub(crate) inventory: Inventory,
    /// Metafiles válidos pelo caminho relativo à pasta do pack.
    pub(crate) metafiles: HashMap<String, Metafile>,
    /// Hash do índice dos arquivos indexados direto: caminho → (formato, hash).
    pub(crate) hashes: HashMap<String, (String, String)>,
}

impl Scan {
    /// O item de um caminho relativo à pasta do pack.
    pub(crate) fn item(&self, path: &str) -> Option<&InventoryItem> {
        let wanted = clean_path(&path.replace('\\', "/"));
        self.inventory.items.iter().find(|item| item.path == wanted)
    }

    /// O item de um caminho, ou [`Code::ItemNotFound`].
    pub(crate) fn require(&self, path: &str) -> Result<&InventoryItem> {
        self.item(path).ok_or_else(|| item_not_found(path))
    }
}

/// Erro de item ausente do inventário.
pub(crate) fn item_not_found(path: &str) -> Error {
    Error::new(Code::ItemNotFound, format!("{path} não está no inventário")).param("path", path)
}

/// Monta o inventário. Só falha se o `pack.toml` não puder ser lido ([`Code::InvalidPack`]);
/// metafile inválido, índice ilegível e falta de internet nunca derrubam a lista.
pub async fn inventory(root: &Path, modrinth: Option<&ModrinthClient>) -> Result<Inventory> {
    let mut scan = scan(root)?;
    if let Some(client) = modrinth {
        enrich(&mut scan.inventory.items, client).await;
    }
    for item in &mut scan.inventory.items {
        if item.version.is_none() {
            item.version = item.file_name.as_deref().map(readable_file_version);
        }
    }
    Ok(scan.inventory)
}

/// Lê o pack do disco: índice, metafiles e arquivos fora do índice.
pub(crate) fn scan(root: &Path) -> Result<Scan> {
    let pack = read_pack(root).map_err(|e| Error::new(Code::InvalidPack, e.to_string()))?;
    let mut result = Scan {
        inventory: Inventory {
            items: Vec::new(),
            index_error: None,
        },
        metafiles: HashMap::new(),
        hashes: HashMap::new(),
    };
    let index = match &pack.index {
        Ok(index) => &index.value,
        Err(error) => {
            result.inventory.index_error = Some(error.to_string());
            return Ok(result);
        }
    };
    let index_dir = index_dir(&pack.pack.value.index.file);
    let mut items = Vec::new();
    for read in &pack.metafiles {
        let path = join(&index_dir, &read.path);
        let kind = kind_of(&read.path);
        match &read.result {
            Ok(parsed) => {
                let item = metafile_item(&path, kind, &parsed.value);
                result.metafiles.insert(path, parsed.value.clone());
                items.push(item);
            }
            Err(error) => items.push(invalid_item(root, &path, kind, &error.to_string())),
        }
    }
    let mut indexed: HashSet<String> = HashSet::new();
    for entry in index.normalized_entries() {
        indexed.insert(entry.file.clone());
        if entry.metafile
            || !is_content_file(&entry.file)
            || check_relative_path(&entry.file).is_err()
        {
            continue;
        }
        let path = join(&index_dir, &entry.file);
        let format = if entry.hash_format.is_empty() {
            index.hash_format.clone()
        } else {
            entry.hash_format.clone()
        };
        result
            .hashes
            .insert(path.clone(), (format, entry.hash.clone()));
        items.push(local_item(&path, kind_of(&entry.file), ItemState::Ok));
    }
    let ignore = PackwizIgnore::for_pack(pack.packwizignore.as_deref());
    let base = if index_dir.is_empty() {
        root.to_path_buf()
    } else {
        root.join(&index_dir)
    };
    for folder in CONTENT_FOLDERS {
        let Ok(entries) = fs::read_dir(base.join(folder)) else {
            continue;
        };
        let mut names: Vec<String> = entries
            .filter_map(std::result::Result::ok)
            .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_file()))
            .filter_map(|entry| entry.file_name().into_string().ok())
            .collect();
        names.sort();
        for name in names {
            let relative = format!("{folder}/{name}");
            if !is_content_file(&relative)
                || indexed.contains(&relative)
                || ignore.is_excluded(&relative)
            {
                continue;
            }
            let path = join(&index_dir, &relative);
            items.push(local_item(
                &path,
                kind_of(&relative),
                ItemState::OutsideIndex,
            ));
        }
    }
    sort_items(&mut items);
    result.inventory.items = items;
    Ok(result)
}

/// Ordem da lista: tipo, nome sem diferenciar maiúsculas e caminho (desempate estável).
fn sort_items(items: &mut [InventoryItem]) {
    items.sort_by(|a, b| {
        a.kind
            .cmp(&b.kind)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
            .then_with(|| a.path.cmp(&b.path))
    });
}

/// Pasta do índice relativa à pasta do pack, com `/` (vazia na raiz).
pub(crate) fn index_dir(index_file: &str) -> String {
    let cleaned = clean_path(index_file);
    match cleaned.rsplit_once('/') {
        Some((dir, _)) => dir.to_owned(),
        None => String::new(),
    }
}

/// Caminho do índice (relativo à pasta do índice) para relativo à pasta do pack.
pub(crate) fn join(index_dir: &str, path: &str) -> String {
    if index_dir.is_empty() {
        clean_path(path)
    } else {
        clean_path(&format!("{index_dir}/{path}"))
    }
}

/// Tipo pela primeira pasta do caminho relativo à pasta do índice.
fn kind_of(path: &str) -> ItemKind {
    match path.split('/').next().unwrap_or_default() {
        "mods" => ItemKind::Mod,
        "resourcepacks" => ItemKind::ResourcePack,
        "shaderpacks" => ItemKind::Shader,
        _ => ItemKind::Other,
    }
}

/// Arquivo `.jar`/`.zip` dentro de uma pasta de conteúdo (não metafile).
fn is_content_file(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    let in_folder = CONTENT_FOLDERS
        .iter()
        .any(|folder| lower.starts_with(&format!("{folder}/")));
    in_folder
        && !lower.ends_with(METAFILE_SUFFIX)
        && Path::new(&lower)
            .extension()
            .is_some_and(|extension| extension == "jar" || extension == "zip")
}

fn file_name(path: &str) -> String {
    path.rsplit('/').next().unwrap_or(path).to_owned()
}

/// Nome do arquivo sem `.jar`/`.zip`: a versão legível quando não há outra.
pub(crate) fn readable_file_version(file_name: &str) -> String {
    let lower = file_name.to_ascii_lowercase();
    for extension in [".jar", ".zip"] {
        if lower.ends_with(extension) && file_name.len() > extension.len() {
            return file_name[..file_name.len() - extension.len()].to_owned();
        }
    }
    file_name.to_owned()
}

fn source_of(metafile: &Metafile) -> ItemSource {
    if metafile.modrinth_ids().is_some() {
        ItemSource::Modrinth
    } else if metafile.curseforge_ids().is_some() || metafile.is_curseforge_mode() {
        ItemSource::Curseforge
    } else {
        ItemSource::Url
    }
}

fn metafile_item(path: &str, kind: ItemKind, metafile: &Metafile) -> InventoryItem {
    let source = source_of(metafile);
    let (key, project_id, source_version_id) =
        if let Some((project, version)) = metafile.modrinth_ids() {
            (
                format!("modrinth:{project}"),
                Some(project.to_owned()),
                Some(version.to_owned()),
            )
        } else if let Some((project, file)) = metafile.curseforge_ids() {
            (
                format!("curseforge:{project}"),
                Some(project.to_string()),
                Some(file.to_string()),
            )
        } else {
            (format!("path:{path}"), None, None)
        };
    let name = if metafile.name.trim().is_empty() {
        file_name(path)
    } else {
        metafile.name.clone()
    };
    InventoryItem {
        key,
        path: path.to_owned(),
        kind,
        state: ItemState::Ok,
        name,
        file_name: (!metafile.filename.is_empty()).then(|| metafile.filename.clone()),
        version: None,
        source,
        side: ItemSide::from(&metafile.side),
        side_editable: true,
        pinned: metafile.pin,
        optional: metafile
            .option
            .as_ref()
            .is_some_and(|option| option.optional),
        project_id,
        source_version_id,
        summary: None,
        icon_url: None,
        error: None,
    }
}

fn invalid_item(root: &Path, path: &str, kind: ItemKind, error: &str) -> InventoryItem {
    // Palpite da fonte pelo texto cru, só para a coluna Fonte; o arquivo continua inválido.
    let text = fs::read_to_string(root.join(path)).unwrap_or_default();
    let source = if text.contains("[update.modrinth]") {
        ItemSource::Modrinth
    } else if text.contains("[update.curseforge]") || text.contains("metadata:curseforge") {
        ItemSource::Curseforge
    } else {
        ItemSource::Url
    };
    InventoryItem {
        key: format!("path:{path}"),
        path: path.to_owned(),
        kind,
        state: ItemState::Invalid,
        name: file_name(path),
        file_name: None,
        version: None,
        source,
        side: ItemSide::Unknown,
        side_editable: false,
        pinned: false,
        optional: false,
        project_id: None,
        source_version_id: None,
        summary: None,
        icon_url: None,
        error: Some(error.to_owned()),
    }
}

fn local_item(path: &str, kind: ItemKind, state: ItemState) -> InventoryItem {
    let name = file_name(path);
    InventoryItem {
        key: format!("path:{path}"),
        path: path.to_owned(),
        kind,
        state,
        name: name.clone(),
        file_name: Some(name),
        version: None,
        source: ItemSource::Local,
        side: ItemSide::Both,
        side_editable: false,
        pinned: false,
        optional: false,
        project_id: None,
        source_version_id: None,
        summary: None,
        icon_url: None,
        error: None,
    }
}

/// Acrescenta versão legível, resumo e ícone aos itens do Modrinth. Nunca falha.
async fn enrich(items: &mut [InventoryItem], client: &ModrinthClient) {
    let is_modrinth =
        |item: &InventoryItem| item.source == ItemSource::Modrinth && item.state == ItemState::Ok;
    let project_ids: Vec<String> = items
        .iter()
        .filter(|item| is_modrinth(item))
        .filter_map(|item| item.project_id.clone())
        .collect();
    let version_ids: Vec<String> = items
        .iter()
        .filter(|item| is_modrinth(item))
        .filter_map(|item| item.source_version_id.clone())
        .collect();
    if project_ids.is_empty() {
        return;
    }
    let projects = modrinth_projects(client, &project_ids).await;
    let versions = modrinth_versions(client, &version_ids).await;
    for item in items.iter_mut().filter(|item| is_modrinth(item)) {
        if let Some(project) = item.project_id.as_ref().and_then(|id| projects.get(id)) {
            item.summary = (!project.description.is_empty()).then(|| project.description.clone());
            item.icon_url.clone_from(&project.icon_url);
        }
        if let Some(version) = item
            .source_version_id
            .as_ref()
            .and_then(|id| versions.get(id))
            .filter(|version| !version.version_number.trim().is_empty())
        {
            item.version = Some(version.version_number.clone());
        }
    }
}

/// Projetos do Modrinth pelo ID: cliente (cache primeiro); sem rede, só o cache.
pub(crate) async fn modrinth_projects(
    client: &ModrinthClient,
    ids: &[String],
) -> HashMap<String, Project> {
    if ids.is_empty() {
        return HashMap::new();
    }
    let projects = match client.projects(ids, None).await {
        Ok(projects) => projects,
        Err(_) => match client.cache() {
            Some(cache) => cache
                .projects(ids)
                .await
                .map(|found| found.into_iter().map(|cached| cached.value).collect())
                .unwrap_or_default(),
            None => Vec::new(),
        },
    };
    projects
        .into_iter()
        .map(|project| (project.id.clone(), project))
        .collect()
}

/// Versões do Modrinth pelo ID: cliente (cache primeiro); sem rede, só o cache.
pub(crate) async fn modrinth_versions(
    client: &ModrinthClient,
    ids: &[String],
) -> HashMap<String, Version> {
    if ids.is_empty() {
        return HashMap::new();
    }
    let versions = match client.versions(ids, None).await {
        Ok(versions) => versions,
        Err(_) => match client.cache() {
            Some(cache) => cache.versions(ids, false).await.unwrap_or_default(),
            None => Vec::new(),
        },
    };
    versions
        .into_iter()
        .map(|version| (version.id.clone(), version))
        .collect()
}

/// Agrupa itens por uma chave, preservando a ordem: os grupos aparecem na ordem do primeiro
/// item de cada um, e os itens de cada grupo na ordem recebida (gancho 1.1: a W-08 acrescenta
/// "por grupo").
pub fn group_by<K: Ord + Clone>(
    items: &[InventoryItem],
    key: impl Fn(&InventoryItem) -> K,
) -> Vec<(K, Vec<&InventoryItem>)> {
    let mut positions: BTreeMap<K, usize> = BTreeMap::new();
    let mut groups: Vec<(K, Vec<&InventoryItem>)> = Vec::new();
    for item in items {
        let group = key(item);
        if let Some(&position) = positions.get(&group) {
            if let Some((_, members)) = groups.get_mut(position) {
                members.push(item);
            }
        } else {
            positions.insert(group.clone(), groups.len());
            groups.push((group, vec![item]));
        }
    }
    groups
}

/// Agrupa por tipo (mods, resource packs, shaders, outros).
#[must_use]
pub fn group_by_kind(items: &[InventoryItem]) -> Vec<(ItemKind, Vec<&InventoryItem>)> {
    group_by(items, |item| item.kind)
}

#[cfg(test)]
mod tests {
    use super::*;
    use warden_packwiz::{IndexEntry, ModOption, PackIndex, PackManifest};

    fn write(root: &Path, path: &str, text: &str) {
        let full = root.join(path);
        fs::create_dir_all(full.parent().unwrap()).unwrap();
        fs::write(full, text).unwrap();
    }

    fn url_metafile(name: &str, filename: &str) -> Metafile {
        Metafile::url(name, &format!("https://example.org/{filename}"), "00").unwrap()
    }

    fn pack(root: &Path, index_file: &str, files: &[&str]) {
        let mut manifest = PackManifest::new("Teste", "1.20.1");
        manifest.index.file = index_file.to_owned();
        write(root, "pack.toml", &manifest.to_toml_string());
        let index = PackIndex {
            hash_format: "sha256".into(),
            files: files
                .iter()
                .map(|file| IndexEntry::new(file, "ab"))
                .collect(),
        };
        write(root, index_file, &index.to_toml_string());
    }

    /// Pack com uma fonte de cada tipo, em pastas variadas.
    fn mixed_pack(root: &Path) {
        pack(
            root,
            "index.toml",
            &[
                "mods/z.pw.toml",
                "mods/a.pw.toml",
                "mods/cf.pw.toml",
                "resourcepacks/r.pw.toml",
                "shaderpacks/s.zip",
                "mods/local.jar",
                "config/x.toml",
                "datapacks/d.pw.toml",
            ],
        );
        let mut z = Metafile::modrinth(
            &warden_packwiz::ModrinthFile {
                title: "Zeta".into(),
                project_id: "AANobbMI".into(),
                version_id: "SMxNOGZ6".into(),
                filename: "zeta-1.0.jar".into(),
                url: "https://cdn.modrinth.com/z.jar".into(),
                hash_format: warden_packwiz::HashFormat::Sha512,
                hash: "00".into(),
            },
            Side::Client,
        );
        z.pin = true;
        write(root, "mods/z.pw.toml", &z.to_toml_string());
        let mut a = url_metafile("alfa", "alfa-2.zip");
        a.side = Side::Other("weird".into());
        a.option = Some(ModOption {
            optional: true,
            description: String::new(),
            default: false,
        });
        write(root, "mods/a.pw.toml", &a.to_toml_string());
        let cf = Metafile::curseforge(
            &warden_packwiz::CurseForgeFile {
                name: "Beta".into(),
                project_id: 238_222,
                file_id: 5_101_366,
                filename: "beta-15.2.jar".into(),
                hash_format: warden_packwiz::HashFormat::Sha1,
                hash: "00".into(),
            },
            Side::Server,
        );
        write(root, "mods/cf.pw.toml", &cf.to_toml_string());
        write(
            root,
            "resourcepacks/r.pw.toml",
            &url_metafile("Recurso", "r.zip").to_toml_string(),
        );
        write(
            root,
            "datapacks/d.pw.toml",
            &url_metafile("Dados", "d.zip").to_toml_string(),
        );
    }

    #[test]
    fn fontes_lados_chaves_e_ordem() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        mixed_pack(root);
        let scan = scan(root).unwrap();
        let items = &scan.inventory.items;
        let names: Vec<&str> = items.iter().map(|i| i.name.as_str()).collect();
        assert_eq!(
            names,
            [
                "alfa",
                "Beta",
                "local.jar",
                "Zeta",
                "Recurso",
                "s.zip",
                "Dados"
            ]
        );
        let zeta = scan.item("mods/z.pw.toml").unwrap();
        assert_eq!(zeta.key, "modrinth:AANobbMI");
        assert_eq!(zeta.source, ItemSource::Modrinth);
        assert_eq!(zeta.side, ItemSide::Client);
        assert!(zeta.pinned && zeta.side_editable);
        assert_eq!(zeta.source_version_id.as_deref(), Some("SMxNOGZ6"));
        let alfa = scan.item("mods/a.pw.toml").unwrap();
        assert_eq!(alfa.source, ItemSource::Url);
        assert_eq!(alfa.side, ItemSide::Unknown);
        assert!(alfa.optional);
        assert_eq!(alfa.key, "path:mods/a.pw.toml");
        let beta = scan.item("mods/cf.pw.toml").unwrap();
        assert_eq!(beta.key, "curseforge:238222");
        assert_eq!(beta.side, ItemSide::Server);
        let local = scan.item("mods/local.jar").unwrap();
        assert_eq!(local.source, ItemSource::Local);
        assert_eq!(local.side, ItemSide::Both);
        assert!(!local.side_editable);
        assert_eq!(
            scan.item("shaderpacks/s.zip").unwrap().kind,
            ItemKind::Shader
        );
        assert_eq!(
            scan.item("datapacks/d.pw.toml").unwrap().kind,
            ItemKind::Other
        );
        assert!(scan.item("config/x.toml").is_none());
        assert_eq!(
            scan.hashes["mods/local.jar"],
            ("sha256".into(), "ab".into())
        );
        let groups = group_by_kind(items);
        let kinds: Vec<ItemKind> = groups.iter().map(|(kind, _)| *kind).collect();
        assert_eq!(
            kinds,
            [
                ItemKind::Mod,
                ItemKind::ResourcePack,
                ItemKind::Shader,
                ItemKind::Other
            ]
        );
        assert_eq!(groups[0].1.len(), 4);
    }

    #[test]
    fn indice_em_subpasta_e_fora_do_indice_respeitando_packwizignore() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        pack(root, "meta/index.toml", &["mods/a.pw.toml"]);
        write(
            root,
            "meta/mods/a.pw.toml",
            &url_metafile("A", "a.jar").to_toml_string(),
        );
        write(root, "meta/mods/solto.jar", "x");
        write(root, "meta/mods/ignorado.jar", "x");
        write(root, "meta/mods/notas.txt", "x");
        write(root, "meta/resourcepacks/rp.zip", "x");
        write(root, "meta/mods/sub/fundo.jar", "x");
        write(root, ".packwizignore", "mods/ignorado.jar\n");
        let scan = scan(root).unwrap();
        let paths: Vec<(&str, ItemState)> = scan
            .inventory
            .items
            .iter()
            .map(|i| (i.path.as_str(), i.state))
            .collect();
        assert_eq!(
            paths,
            [
                ("meta/mods/a.pw.toml", ItemState::Ok),
                ("meta/mods/solto.jar", ItemState::OutsideIndex),
                ("meta/resourcepacks/rp.zip", ItemState::OutsideIndex),
            ]
        );
    }

    #[test]
    fn indice_ilegivel_da_lista_vazia_e_pack_toml_ilegivel_falha() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        pack(root, "index.toml", &[]);
        write(root, "index.toml", "files = 1\n");
        let scan = scan(root).unwrap();
        assert!(scan.inventory.items.is_empty());
        assert!(scan.inventory.index_error.is_some());
        write(root, "pack.toml", "name = 1\n");
        assert_eq!(super::scan(root).unwrap_err().code, Code::InvalidPack);
    }

    #[test]
    fn metafile_invalido_adivinha_a_fonte_e_mostra_o_erro() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        pack(root, "index.toml", &["mods/ruim.pw.toml"]);
        write(root, "mods/ruim.pw.toml", "name = \n[update.modrinth]\n");
        let scan = scan(root).unwrap();
        let item = &scan.inventory.items[0];
        assert_eq!(item.state, ItemState::Invalid);
        assert_eq!(item.name, "ruim.pw.toml");
        assert_eq!(item.source, ItemSource::Modrinth);
        assert!(!item.side_editable);
        assert!(item.error.as_deref().unwrap().contains("ruim.pw.toml"));
    }

    #[test]
    fn versao_legivel_do_nome_do_arquivo() {
        assert_eq!(
            readable_file_version("jei-1.20.1-15.2.jar"),
            "jei-1.20.1-15.2"
        );
        assert_eq!(readable_file_version("Shader.ZIP"), "Shader");
        assert_eq!(readable_file_version("x.txt"), "x.txt");
        assert_eq!(readable_file_version(".jar"), ".jar");
    }

    #[test]
    fn agrupamento_generico_preserva_a_ordem() {
        let item = |name: &str, kind| {
            let mut item = local_item(&format!("mods/{name}"), kind, ItemState::Ok);
            item.name = name.into();
            item
        };
        let items = vec![
            item("b", ItemKind::Shader),
            item("a", ItemKind::Mod),
            item("c", ItemKind::Shader),
        ];
        let groups = group_by(&items, |i| i.kind);
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].0, ItemKind::Shader);
        let names: Vec<&str> = groups[0].1.iter().map(|i| i.name.as_str()).collect();
        assert_eq!(names, ["b", "c"]);
        assert!(group_by(&[], |i| i.kind).is_empty());
    }
}
