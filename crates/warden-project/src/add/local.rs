//! Arquivo do computador (e o arquivo baixado de um link direto) em "Adicionar" (SPEC T08
//! "Arquivo"; P1-11; CA-T08-04 e CA-T08-05).
//!
//! Para cada arquivo:
//!
//! 1. [`read_file`] lê o arquivo inteiro (até [`MAX_FILE_BYTES`]) e calcula sha1, sha256,
//!    sha512 e a impressão da CurseForge ([`FileFacts`]).
//! 2. [`inspect`] lê os metadados sem extrair nada (`warden-jarmeta`: `fabric.mod.json`,
//!    `quilt.mod.json`, `META-INF/mods.toml`, `META-INF/neoforge.mods.toml`, `mcmod.info`), adivinha
//!    o tipo pelo conteúdo (descritor de mod, `pack.mcmeta`, pasta `shaders/`), confere o loader
//!    e a versão do Minecraft com o pack e procura o mesmo arquivo no pack. O conteúdo não é
//!    confiável: o zip é lido com os limites da `warden-jarmeta` e só os nomes das entradas são
//!    olhados; JSON inválido vira "sem metadados", nunca pânico.
//! 3. [`identify`] procura o arquivo pelo hash no Modrinth (`/version_files`, sha512) e, com a
//!    chave, na CurseForge (`/fingerprints`). Achado, a tela oferece **Adicionar como
//!    referência** (`.pw.toml` pelo caminho normal, `add_plan`/`add_apply`, sem copiar o jar) ou
//!    **Adicionar como arquivo local**.
//! 4. [`add_local_apply`] grava: o arquivo do computador é **copiado** para a pasta do tipo; o
//!    do link direto vira um `.pw.toml` com `[download]` (link e `sha256`, como o
//!    `packwiz url add`). Tudo numa transação com um `packwiz refresh`: ou todos entram, ou
//!    nenhum. Incompatível só entra com `accept_incompatible` ("Adicionar mesmo assim"), e o
//!    diagnóstico passa a mostrar o erro de loader (`E_LOADER`).
//!
//! Privacidade: nenhuma mensagem de erro leva o caminho do arquivo no computador (só o nome).

use std::collections::{HashMap, HashSet};
use std::io::Read as _;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use warden_core::CancellationToken;
use warden_curseforge::CurseforgeClient;
use warden_jarmeta::{DescriptorKind, JarMetadata, Limits};
use warden_modrinth::{HashAlgorithm, ModrinthClient};
use warden_packwiz::hash::{curseforge_fingerprint, hash_bytes, hash_matches};
use warden_packwiz::{
    HashFormat, METAFILE_SUFFIX, Metafile, MetafileSource, slugify_name, unique_metafile_stem,
};
use warden_packwiz_cli::Packwiz;

use super::url::safe_file_name;
use super::{AddResult, AddedItem, SideNote};
use crate::inventory::scan;
use crate::search::{PackTarget, ProjectKind, SourceId};
use crate::side::SideChoice;
use crate::transaction::PackTransaction;
use crate::{Error, ProjectErrorCode as Code, Result};

/// Maior arquivo aceito: 256 MiB (o mesmo limite do link direto).
pub const MAX_FILE_BYTES: u64 = super::link::MAX_DIRECT_BYTES;

/// O que se sabe de um arquivo sem abrir o conteúdo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileFacts {
    /// Nome do arquivo (sem a pasta).
    pub file_name: String,
    /// Tamanho em bytes.
    pub size: u64,
    /// SHA-1.
    pub sha1: String,
    /// SHA-256 (o hash do link direto e do índice do packwiz).
    pub sha256: String,
    /// SHA-512 (identificação no Modrinth).
    pub sha512: String,
    /// Impressão digital da CurseForge (murmur2 sem espaços).
    pub murmur2: u32,
}

impl FileFacts {
    /// Hashes dos bytes.
    #[must_use]
    pub fn of(file_name: &str, bytes: &[u8]) -> Self {
        Self {
            file_name: file_name.to_owned(),
            size: u64::try_from(bytes.len()).unwrap_or(u64::MAX),
            sha1: hash_bytes(HashFormat::Sha1, bytes),
            sha256: hash_bytes(HashFormat::Sha256, bytes),
            sha512: hash_bytes(HashFormat::Sha512, bytes),
            murmur2: curseforge_fingerprint(bytes),
        }
    }

    /// O hash no formato dado (md5 não é calculado).
    fn hash(&self, format: HashFormat) -> Option<String> {
        match format {
            HashFormat::Sha1 => Some(self.sha1.clone()),
            HashFormat::Sha256 => Some(self.sha256.clone()),
            HashFormat::Sha512 => Some(self.sha512.clone()),
            HashFormat::Murmur2 => Some(self.murmur2.to_string()),
            HashFormat::Md5 => None,
        }
    }
}

/// Erro de entrada sobre um arquivo, com o nome (nunca o caminho) e o motivo para a frase.
fn file_error(name: &str, reason: &str, detail: impl Into<String>) -> Error {
    Error::new(Code::InvalidInput, detail)
        .param("field", "file")
        .param("reason", reason)
        .param("name", name)
}

/// O nome de um caminho, se servir de nome de arquivo no pack.
fn file_name_of(path: &Path) -> Result<String> {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default()
        .to_owned();
    if safe_file_name(&name) {
        Ok(name)
    } else {
        Err(file_error(
            &name,
            "badName",
            "o nome do arquivo não pode ser usado no pack",
        ))
    }
}

/// Lê o arquivo inteiro (até [`MAX_FILE_BYTES`]) e calcula os hashes.
///
/// Erros: [`Code::InvalidInput`] com `reason` `notFound`, `notFile`, `tooLarge`, `badName` ou
/// `unreadable` (a mensagem leva só o nome do arquivo).
pub fn read_file(path: &Path) -> Result<(Vec<u8>, FileFacts)> {
    let name = file_name_of(path)?;
    let metadata = std::fs::metadata(path).map_err(|e| {
        let reason = if e.kind() == std::io::ErrorKind::NotFound {
            "notFound"
        } else {
            "unreadable"
        };
        file_error(&name, reason, format!("{name}: {}", e.kind()))
    })?;
    if !metadata.is_file() {
        return Err(file_error(
            &name,
            "notFile",
            format!("{name} não é um arquivo"),
        ));
    }
    if metadata.len() > MAX_FILE_BYTES {
        return Err(file_error(
            &name,
            "tooLarge",
            format!(
                "{name} tem {} bytes; o máximo é {MAX_FILE_BYTES}",
                metadata.len()
            ),
        ));
    }
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .and_then(|file| file.take(MAX_FILE_BYTES + 1).read_to_end(&mut bytes))
        .map_err(|e| file_error(&name, "unreadable", format!("{name}: {}", e.kind())))?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > MAX_FILE_BYTES {
        return Err(file_error(
            &name,
            "tooLarge",
            format!("{name} cresceu durante a leitura"),
        ));
    }
    let facts = FileFacts::of(&name, &bytes);
    Ok((bytes, facts))
}

/// Metadados do mod lidos do jar.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct JarModInfo {
    /// ID do mod (`sodium`).
    pub id: String,
    /// Nome legível, se o jar informa.
    pub name: Option<String>,
    /// Versão do mod, se o jar informa.
    pub version: Option<String>,
    /// Loaders para os quais o jar traz descritor (`fabric`, `quilt`, `forge`, `neoforge`).
    pub loaders: Vec<String>,
    /// Versões do Minecraft aceitas, como o jar escreve (`>=1.21 <1.22`, `[1.20.1,1.21)`).
    pub minecraft: Option<String>,
}

/// Por que o arquivo não serve para o pack (SPEC T08: "Loader ou versão do Minecraft
/// incompatíveis").
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Incompatibility {
    /// O jar é de outro loader.
    #[serde(rename_all = "camelCase")]
    Loader {
        /// Loaders do jar.
        found: Vec<String>,
        /// Loader do pack.
        pack: String,
    },
    /// O jar não aceita a versão do Minecraft do pack.
    #[serde(rename_all = "camelCase")]
    Minecraft {
        /// Versões aceitas, como o jar escreve.
        accepts: String,
        /// Versão do pack.
        pack: String,
    },
}

/// O arquivo achado pelo hash numa fonte ("Este arquivo é o Sodium 0.6.0 do Modrinth.").
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct HashMatch {
    /// A fonte.
    pub source: SourceId,
    /// ID do projeto.
    pub project_id: String,
    /// ID da versão (Modrinth) ou do arquivo (CurseForge).
    pub version_id: String,
    /// Nome do projeto.
    pub title: String,
    /// Versão legível.
    pub version_number: String,
    /// Tipo do projeto (nenhum: tipo que a página Adicionar não aceita).
    pub kind: Option<ProjectKind>,
    /// A versão serve para o Minecraft e o loader do pack, pelos dados da fonte.
    pub compatible: bool,
}

/// Um item do pack que já tem o mesmo arquivo, ou o mesmo nome de arquivo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct InPack {
    /// Caminho do item no pack.
    pub path: String,
    /// Nome no pack.
    pub name: String,
    /// É o mesmo arquivo (pelo hash); senão, só o nome do arquivo coincide.
    pub same_file: bool,
}

/// O que a tela mostra de um arquivo antes de adicionar.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct FileReport {
    /// Nome do arquivo.
    pub file_name: String,
    /// Tamanho em bytes.
    pub size: u64,
    /// SHA-256 (a tela devolve na gravação: o arquivo não pode ter mudado).
    pub sha256: String,
    /// Tipo pelo conteúdo (nenhum: não deu para saber; a tela pede).
    pub kind: Option<ProjectKind>,
    /// O arquivo é um zip legível.
    pub archive: bool,
    /// Metadados do mod, se o jar tem descritor.
    pub mod_info: Option<JarModInfo>,
    /// Por que não serve para o pack (vazio: serve, ou não deu para saber).
    pub incompatible: Vec<Incompatibility>,
    /// Lado sugerido pelos metadados.
    pub side: SideChoice,
    /// Observação sobre o lado.
    pub side_note: Option<SideNote>,
    /// O mesmo arquivo nas fontes, o Modrinth primeiro.
    pub matches: Vec<HashMatch>,
    /// Fontes em que a procura pelo hash falhou (fora do ar, tempo esgotado).
    pub lookup_failed: Vec<SourceId>,
    /// O pack já tem este arquivo, ou outro com o mesmo nome na mesma pasta.
    pub in_pack: Option<InPack>,
}

fn loader_key(loader: warden_jarmeta::Loader) -> &'static str {
    match loader {
        warden_jarmeta::Loader::Fabric => "fabric",
        warden_jarmeta::Loader::Quilt => "quilt",
        warden_jarmeta::Loader::Forge => "forge",
        warden_jarmeta::Loader::NeoForge => "neoforge",
    }
}

fn jar_loader(loader: warden_packwiz::Loader) -> Option<warden_jarmeta::Loader> {
    match loader {
        warden_packwiz::Loader::Fabric => Some(warden_jarmeta::Loader::Fabric),
        warden_packwiz::Loader::Quilt => Some(warden_jarmeta::Loader::Quilt),
        warden_packwiz::Loader::Forge => Some(warden_jarmeta::Loader::Forge),
        warden_packwiz::Loader::NeoForge => Some(warden_jarmeta::Loader::NeoForge),
        warden_packwiz::Loader::LiteLoader => None,
    }
}

/// Tipo pelo conteúdo do zip: só os nomes das entradas são olhados (nada é extraído).
fn kind_by_entries(bytes: &[u8], limits: &Limits) -> Option<ProjectKind> {
    let archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).ok()?;
    if archive.len() > limits.max_entries {
        return None;
    }
    let mut mcmeta = false;
    let mut shaders = false;
    for name in archive.file_names() {
        let name = name.trim_start_matches('/');
        mcmeta |= name == "pack.mcmeta";
        shaders |= name.starts_with("shaders/");
    }
    if shaders {
        Some(ProjectKind::Shader)
    } else if mcmeta {
        Some(ProjectKind::ResourcePack)
    } else {
        None
    }
}

/// O loader do jar serve para o pack? Mesma regra do diagnóstico (`E_LOADER`): nenhum mod que
/// o loader do pack leia, ou só `fabric.mod.json` num pack Forge/NeoForge.
fn loader_problem(jar: &JarMetadata, target: &PackTarget) -> Option<Incompatibility> {
    let pack_loader = *target.loaders.first()?;
    let loader = jar_loader(pack_loader)?;
    if jar.mods.is_empty() {
        // Biblioteca, coremod ou jar sem descritor: não dá para saber.
        return None;
    }
    let mods = jar.effective_mods_for_loader(loader);
    let fabric_only = matches!(
        loader,
        warden_jarmeta::Loader::Forge | warden_jarmeta::Loader::NeoForge
    ) && !mods.is_empty()
        && mods
            .iter()
            .all(|module| module.source == DescriptorKind::FabricModJson);
    if !mods.is_empty() && !fabric_only {
        return None;
    }
    let mut found: Vec<String> = jar
        .mods
        .iter()
        .map(|module| loader_key(module.loader).to_owned())
        .collect();
    found.sort();
    found.dedup();
    Some(Incompatibility::Loader {
        found,
        pack: pack_loader.key().to_owned(),
    })
}

/// O mod principal do jar para o pack: o do loader do pack ou o primeiro.
fn main_mod<'a>(
    jar: &'a JarMetadata,
    target: &PackTarget,
) -> Option<&'a warden_jarmeta::ModMetadata> {
    target
        .loaders
        .first()
        .and_then(|loader| jar_loader(*loader))
        .and_then(|loader| jar.effective_mods_for_loader(loader).into_iter().next())
        .or_else(|| jar.mods.first())
}

fn side_of(module: Option<&warden_jarmeta::ModMetadata>) -> (SideChoice, Option<SideNote>) {
    let Some(module) = module else {
        return (SideChoice::Both, Some(SideNote::Unknown));
    };
    let declares = matches!(
        module.source,
        DescriptorKind::FabricModJson | DescriptorKind::QuiltModJson
    );
    match module.environment {
        warden_jarmeta::Environment::Client => (SideChoice::Client, None),
        warden_jarmeta::Environment::Server => (SideChoice::Server, None),
        warden_jarmeta::Environment::Both if declares => (SideChoice::Both, None),
        warden_jarmeta::Environment::Both => (SideChoice::Both, Some(SideNote::Unknown)),
    }
}

/// O que se lê do conteúdo, sem rede e sem o pack.
struct Content {
    kind: Option<ProjectKind>,
    archive: bool,
    mod_info: Option<JarModInfo>,
    incompatible: Vec<Incompatibility>,
    side: (SideChoice, Option<SideNote>),
}

fn read_content(bytes: &[u8], file_name: &str, target: &PackTarget) -> Content {
    let limits = Limits {
        max_file_bytes: MAX_FILE_BYTES,
        ..Limits::default()
    };
    let jar = warden_jarmeta::read_jar_bytes(bytes, &limits).ok();
    let archive = jar.is_some() || zip::ZipArchive::new(std::io::Cursor::new(bytes)).is_ok();
    let module = jar.as_ref().and_then(|jar| main_mod(jar, target));
    let mod_info = jar.as_ref().zip(module).map(|(jar, module)| {
        let mut loaders: Vec<String> = jar
            .mods
            .iter()
            .map(|m| loader_key(m.loader).to_owned())
            .collect();
        loaders.sort();
        loaders.dedup();
        JarModInfo {
            id: module.id.clone(),
            name: module.name.clone().filter(|name| !name.trim().is_empty()),
            version: module
                .version
                .clone()
                .filter(|version| !version.trim().is_empty()),
            loaders,
            minecraft: module.minecraft.as_ref().map(range_text),
        }
    });
    let is_jar = Path::new(file_name)
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("jar"));
    let kind = if mod_info.is_some() {
        Some(ProjectKind::Mod)
    } else {
        kind_by_entries(bytes, &limits).or(is_jar.then_some(ProjectKind::Mod))
    };
    let mut incompatible = Vec::new();
    if let Some(jar) = &jar {
        incompatible.extend(loader_problem(jar, target));
        if incompatible.is_empty()
            && let Some(range) = module.and_then(|module| module.minecraft.as_ref())
            // Faixa ilegível não bloqueia: o diagnóstico cuida dela.
            && !target
                .game_versions
                .iter()
                .any(|version| range.matches(version).unwrap_or(true))
        {
            incompatible.push(Incompatibility::Minecraft {
                accepts: range_text(range),
                pack: target.minecraft.clone(),
            });
        }
    }
    Content {
        kind,
        archive,
        mod_info,
        incompatible,
        side: side_of(module),
    }
}

fn range_text(range: &warden_jarmeta::VersionRange) -> String {
    match range {
        warden_jarmeta::VersionRange::Any => "*".to_owned(),
        warden_jarmeta::VersionRange::Fabric { predicates } => predicates.join(" || "),
        warden_jarmeta::VersionRange::Maven { spec } => spec.clone(),
        warden_jarmeta::VersionRange::Exact { version } => version.clone(),
    }
}

/// Lê o conteúdo do arquivo e o compara com o pack em `root` (tipo, metadados, loader e versão
/// do Minecraft, lado e se o pack já tem o arquivo). Sem rede: `matches` fica vazio (veja
/// [`identify`]).
///
/// Erro [`Code::InvalidPack`] se o pack não puder ser lido.
pub fn inspect(root: &Path, bytes: &[u8], facts: &FileFacts) -> Result<FileReport> {
    let target = PackTarget::read(root)?;
    let content = read_content(bytes, &facts.file_name, &target);
    let in_pack = find_in_pack(root, facts, content.kind)?;
    Ok(FileReport {
        file_name: facts.file_name.clone(),
        size: facts.size,
        sha256: facts.sha256.clone(),
        kind: content.kind,
        archive: content.archive,
        mod_info: content.mod_info,
        incompatible: content.incompatible,
        side: content.side.0,
        side_note: content.side.1,
        matches: Vec::new(),
        lookup_failed: Vec::new(),
        in_pack,
    })
}

/// O item do pack com o mesmo arquivo (pelo hash do metafile ou do índice) ou, na pasta do
/// tipo, com o mesmo nome de arquivo.
fn find_in_pack(
    root: &Path,
    facts: &FileFacts,
    kind: Option<ProjectKind>,
) -> Result<Option<InPack>> {
    let scan = scan(root)?;
    let mut same_name = None;
    for item in &scan.inventory.items {
        let hash = scan
            .metafiles
            .get(&item.path)
            .map(|metafile| {
                (
                    metafile.download.hash_format.clone(),
                    metafile.download.hash.clone(),
                )
            })
            .or_else(|| scan.hashes.get(&item.path).cloned());
        let same_file = hash.is_some_and(|(format, hash)| {
            HashFormat::parse(&format).ok().is_some_and(|format| {
                facts
                    .hash(format)
                    .is_some_and(|ours| !hash.is_empty() && hash_matches(format, &hash, &ours))
            })
        });
        if same_file {
            return Ok(Some(InPack {
                path: item.path.clone(),
                name: item.name.clone(),
                same_file: true,
            }));
        }
        let folder = kind.map_or("mods", ProjectKind::folder);
        if same_name.is_none()
            && item.path.starts_with(&format!("{folder}/"))
            && item
                .file_name
                .as_deref()
                .is_some_and(|name| name.eq_ignore_ascii_case(&facts.file_name))
        {
            same_name = Some(InPack {
                path: item.path.clone(),
                name: item.name.clone(),
                same_file: false,
            });
        }
    }
    Ok(same_name)
}

/// Os clientes para a procura pelo hash. Sem a CurseForge (sem chave ou chave recusada), só o
/// Modrinth é consultado: nenhuma requisição vai para a CurseForge.
#[derive(Clone, Copy)]
pub struct HashLookup<'a> {
    /// Modrinth.
    pub modrinth: Option<&'a ModrinthClient>,
    /// CurseForge (com a chave do usuário).
    pub curseforge: Option<&'a CurseforgeClient>,
}

/// O resultado da procura de vários arquivos, na ordem pedida.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Identified {
    /// Para cada arquivo, os achados (Modrinth primeiro).
    pub matches: Vec<Vec<HashMatch>>,
    /// Fontes em que a procura falhou.
    pub failed: Vec<SourceId>,
}

/// Procura os arquivos pelo hash no Modrinth e na CurseForge (uma requisição em lote por fonte,
/// mais uma para os nomes dos projetos). Falha de uma fonte não é erro: fica em `failed` e a
/// tela oferece o arquivo local.
pub async fn identify(
    lookup: HashLookup<'_>,
    files: &[FileFacts],
    target: &PackTarget,
    cancel: &CancellationToken,
) -> Identified {
    let mut result = Identified {
        matches: vec![Vec::new(); files.len()],
        failed: Vec::new(),
    };
    if files.is_empty() {
        return result;
    }
    if let Some(client) = lookup.modrinth {
        match modrinth_matches(client, files, target, cancel).await {
            Ok(found) => {
                for (slot, found) in result.matches.iter_mut().zip(found) {
                    slot.extend(found);
                }
            }
            Err(error) => {
                tracing::warn!(%error, "procura pelo hash no Modrinth falhou");
                result.failed.push(SourceId::Modrinth);
            }
        }
    }
    if let Some(client) = lookup.curseforge {
        match curseforge_matches(client, files, target, cancel).await {
            Ok(found) => {
                for (slot, found) in result.matches.iter_mut().zip(found) {
                    slot.extend(found);
                }
            }
            Err(error) => {
                tracing::warn!(%error, "procura pela impressão digital na CurseForge falhou");
                result.failed.push(SourceId::Curseforge);
            }
        }
    }
    result
}

async fn modrinth_matches(
    client: &ModrinthClient,
    files: &[FileFacts],
    target: &PackTarget,
    cancel: &CancellationToken,
) -> std::result::Result<Vec<Option<HashMatch>>, warden_modrinth::Error> {
    let hashes: Vec<String> = files.iter().map(|file| file.sha512.clone()).collect();
    let found = client
        .version_files(&hashes, HashAlgorithm::Sha512, Some(cancel))
        .await?;
    if found.is_empty() {
        return Ok(vec![None; files.len()]);
    }
    let mut ids: Vec<String> = found.values().map(|v| v.project_id.clone()).collect();
    ids.sort();
    ids.dedup();
    let projects: HashMap<String, warden_modrinth::Project> = client
        .projects(&ids, Some(cancel))
        .await?
        .into_iter()
        .map(|project| (project.id.clone(), project))
        .collect();
    Ok(files
        .iter()
        .map(|file| {
            let version = found.get(&file.sha512)?;
            let project = projects.get(&version.project_id);
            let kind = project.and_then(|p| super::modrinth::kind_of(p.project_type));
            let common = super::modrinth::source_version(version);
            Some(HashMatch {
                source: SourceId::Modrinth,
                project_id: version.project_id.clone(),
                version_id: version.id.clone(),
                title: project.map_or_else(|| version.name.clone(), |p| p.title.clone()),
                version_number: version.version_number.clone(),
                kind,
                compatible: common.compatible(kind, target),
            })
        })
        .collect())
}

async fn curseforge_matches(
    client: &CurseforgeClient,
    files: &[FileFacts],
    target: &PackTarget,
    cancel: &CancellationToken,
) -> std::result::Result<Vec<Option<HashMatch>>, warden_curseforge::Error> {
    let fingerprints: Vec<u32> = files.iter().map(|file| file.murmur2).collect();
    let found = client.fingerprints(&fingerprints, Some(cancel)).await?;
    if found.matches.is_empty() {
        return Ok(vec![None; files.len()]);
    }
    let ids: Vec<u64> = found
        .matches
        .values()
        .map(|m| m.id)
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    let projects: HashMap<u64, warden_curseforge::Mod> = client
        .projects(&ids, Some(cancel))
        .await?
        .into_iter()
        .map(|project| (project.id, project))
        .collect();
    Ok(files
        .iter()
        .map(|file| {
            let found = found.matches.get(&file.murmur2)?;
            let project = projects.get(&found.id);
            let kind = super::curseforge::kind_of(project.and_then(|p| p.class_id));
            let common = super::curseforge::source_version(&found.file);
            Some(HashMatch {
                source: SourceId::Curseforge,
                project_id: found.id.to_string(),
                version_id: found.file.id.to_string(),
                title: project.map_or_else(|| common.number.clone(), |p| p.name.clone()),
                version_number: common.number.clone(),
                kind,
                compatible: common.compatible(kind, target),
            })
        })
        .collect())
}

/// Um arquivo confirmado na tela.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalAdd {
    /// Onde estão os bytes (o arquivo do computador, ou o baixado do link na pasta temporária).
    pub path: PathBuf,
    /// O SHA-256 que a tela viu (o arquivo não pode ter mudado desde a leitura).
    pub sha256: String,
    /// Tipo escolhido (pasta do pack).
    pub kind: ProjectKind,
    /// Link direto de onde o arquivo veio: grava a referência (`.pw.toml`) no lugar da cópia.
    pub link: Option<String>,
    /// Nome do item (o `name` do `.pw.toml` do link; vazio: o do jar ou o do arquivo).
    pub name: String,
    /// Lado (só no `.pw.toml` do link; o arquivo copiado vale para os dois lados).
    pub side: SideChoice,
    /// "Adicionar mesmo assim": aceita o arquivo de outro loader ou de outra versão.
    pub accept_incompatible: bool,
}

/// Como a tela confirma um arquivo (a tela não conhece caminhos: o app troca o `token` pelo
/// caminho que ele mesmo guardou).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct LocalApplyItem {
    /// O arquivo lido antes (`add_local_inspect` ou `add_link_download`).
    pub token: String,
    /// O SHA-256 do relatório.
    pub sha256: String,
    /// Tipo escolhido.
    pub kind: ProjectKind,
    /// Lado escolhido; nenhum: o sugerido.
    #[serde(default)]
    pub side: Option<SideChoice>,
    /// "Adicionar mesmo assim".
    #[serde(default)]
    pub accept_incompatible: bool,
}

/// O nome do item: o do jar, senão o nome do arquivo sem a extensão.
fn display_name(content: &Content, file_name: &str) -> String {
    content
        .mod_info
        .as_ref()
        .and_then(|info| info.name.clone())
        .unwrap_or_else(|| {
            Path::new(file_name)
                .file_stem()
                .and_then(|stem| stem.to_str())
                .unwrap_or(file_name)
                .to_owned()
        })
}

/// Grava os arquivos confirmados numa transação só (um `packwiz refresh`): o arquivo do
/// computador é copiado para a pasta do tipo; o do link direto vira um `.pw.toml` com
/// `[download]`. Ou todos entram, ou nenhum.
///
/// Itens cujo arquivo o pack já tem (pelo hash) vão em `skipped`, sem mudança.
///
/// Erros: [`Code::InvalidInput`] (`reason`: `changed` se o arquivo mudou desde a leitura,
/// `incompatible` sem "Adicionar mesmo assim", `nameTaken` se outro arquivo do pack já usa o
/// nome, `badName`, `tooLarge`, `notFound`); [`Code::PackChangedExternally`]; os da transação.
pub async fn add_local_apply(
    root: &Path,
    items: &[LocalAdd],
    packwiz: &Packwiz,
    cancel: &CancellationToken,
) -> Result<AddResult> {
    if items.is_empty() {
        return Err(
            Error::new(Code::InvalidInput, "nenhum arquivo para gravar").param("field", "items")
        );
    }
    let target = PackTarget::read(root)?;
    let pack = super::plan::PackState::read(root)?;
    let mut taken: Vec<String> = pack.stems.clone();
    taken.extend(super::stems_on_disk(root));
    let mut used_names: HashSet<String> = HashSet::new();
    let mut tx = PackTransaction::new(root.to_path_buf());
    let mut added = Vec::new();
    let mut skipped = Vec::new();
    for item in items {
        let (bytes, facts) = read_file(&item.path)?;
        let name = facts.file_name.clone();
        if !facts.sha256.eq_ignore_ascii_case(item.sha256.trim()) {
            return Err(file_error(
                &name,
                "changed",
                format!("{name} mudou desde que foi lido"),
            ));
        }
        let content = read_content(&bytes, &name, &target);
        if item.kind == ProjectKind::Mod
            && !content.incompatible.is_empty()
            && !item.accept_incompatible
        {
            return Err(file_error(
                &name,
                "incompatible",
                format!("{name} não serve para o pack"),
            ));
        }
        if let Some(found) = find_in_pack(root, &facts, Some(item.kind))? {
            if found.same_file {
                skipped.push(found.path);
                continue;
            }
            if item.link.is_none() {
                return Err(file_error(
                    &name,
                    "nameTaken",
                    format!("{} já usa o nome {name}", found.path),
                )
                .param("path", found.path));
            }
        }
        let title = if item.name.trim().is_empty() {
            display_name(&content, &name)
        } else {
            item.name.trim().to_owned()
        };
        let path = match &item.link {
            None => {
                let path = format!("{}/{name}", item.kind.folder());
                if !used_names.insert(path.to_lowercase()) || root.join(&path).exists() {
                    return Err(file_error(&name, "nameTaken", format!("{path} já existe"))
                        .param("path", &path));
                }
                tx.write(&path, bytes)?;
                path
            }
            Some(link) => {
                let mut metafile = Metafile::url(&title, link, &facts.sha256).map_err(|e| {
                    Error::new(Code::InvalidInput, e.to_string()).param("field", "link")
                })?;
                metafile.side = item.side.side();
                let slug = Some(slugify_name(&title))
                    .filter(|slug| !slug.is_empty())
                    .unwrap_or_else(|| slugify_name(&name));
                let stem = unique_metafile_stem(
                    &slug,
                    MetafileSource::Url,
                    taken.iter().map(String::as_str),
                );
                taken.push(stem.clone());
                let path = format!("{}/{stem}{METAFILE_SUFFIX}", item.kind.folder());
                tx.write(&path, metafile.to_toml_string().into_bytes())?;
                path
            }
        };
        added.push(AddedItem {
            key: format!(
                "{}:{path}",
                if item.link.is_some() { "url" } else { "local" }
            ),
            title,
            path,
        });
    }
    if !added.is_empty() {
        tx.commit(packwiz, cancel).await?;
    }
    Ok(AddResult {
        added,
        skipped,
        removed: Vec::new(),
    })
}

#[cfg(test)]
mod tests {
    use std::io::Write as _;

    use warden_packwiz::{IndexEntry, Loader, PackIndex, PackManifest};
    use zip::write::{SimpleFileOptions, ZipWriter};

    use super::*;

    const CORPUS: &str = "../warden-jarmeta/tests/corpus/jars";

    /// Descritor real copiado de um jar do corpus da `warden-jarmeta`.
    fn corpus(path: &str) -> String {
        std::fs::read_to_string(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join(CORPUS)
                .join(path),
        )
        .unwrap()
    }

    fn zip_of(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let mut writer = ZipWriter::new(std::io::Cursor::new(Vec::new()));
        for (name, content) in entries {
            writer
                .start_file(*name, SimpleFileOptions::default())
                .unwrap();
            writer.write_all(content).unwrap();
        }
        writer.finish().unwrap().into_inner()
    }

    fn target(minecraft: &str, loader: Loader) -> PackTarget {
        let mut manifest = PackManifest::new("Teste", minecraft);
        manifest.set_loader_version(loader, "1.0.0");
        PackTarget::from_manifest(&manifest).unwrap()
    }

    fn pack(root: &Path, minecraft: &str, loader: Loader) {
        let mut manifest = PackManifest::new("Teste", minecraft);
        manifest.set_loader_version(loader, "1.0.0");
        std::fs::write(root.join("pack.toml"), manifest.to_toml_string()).unwrap();
        std::fs::write(
            root.join("index.toml"),
            PackIndex::default().to_toml_string(),
        )
        .unwrap();
    }

    fn sodium_jar() -> Vec<u8> {
        zip_of(&[(
            "fabric.mod.json",
            corpus("fabric-1.20.1-sodium/fabric.mod.json").as_bytes(),
        )])
    }

    #[test]
    fn hashes_do_arquivo() {
        let facts = FileFacts::of("a.jar", b"abc");
        assert_eq!(facts.sha1, "a9993e364706816aba3e25717850c26c9cd0d89d");
        assert_eq!(
            facts.sha256,
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(facts.sha512.len(), 128);
        assert_eq!(facts.murmur2, curseforge_fingerprint(b"abc"));
        assert_eq!(facts.size, 3);
    }

    #[test]
    fn caminho_do_windows_com_espacos_e_acentos() {
        let dir = tempfile::tempdir().unwrap();
        let folder = dir.path().join("Área de Trabalho").join("Mods baixados ç");
        std::fs::create_dir_all(&folder).unwrap();
        let file = folder.join("Meu Mod (versão 1).jar");
        std::fs::write(&file, sodium_jar()).unwrap();
        let (bytes, facts) = read_file(&file).unwrap();
        assert_eq!(facts.file_name, "Meu Mod (versão 1).jar");
        assert_eq!(bytes.len() as u64, facts.size);
    }

    #[test]
    fn erros_de_leitura_nao_levam_o_caminho() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("pessoal").join("sumiu.jar");
        let error = read_file(&missing).unwrap_err();
        assert_eq!(
            error.params.get("reason").map(String::as_str),
            Some("notFound")
        );
        assert!(!error.detail.contains("pessoal"), "{}", error.detail);
        assert_eq!(
            error.params.get("name").map(String::as_str),
            Some("sumiu.jar")
        );
        let error = read_file(dir.path()).unwrap_err();
        assert!(!error.detail.contains(&*dir.path().to_string_lossy()));
        // Grande demais: recusado antes de ler (o arquivo é esparso, nada é escrito).
        let big = dir.path().join("grande.jar");
        std::fs::File::create(&big)
            .unwrap()
            .set_len(MAX_FILE_BYTES + 1)
            .unwrap();
        let error = read_file(&big).unwrap_err();
        assert_eq!(
            error.params.get("reason").map(String::as_str),
            Some("tooLarge")
        );
    }

    #[test]
    fn jar_fabric_num_pack_forge_e_incompativel() {
        let jar = sodium_jar();
        let report = read_content(&jar, "sodium.jar", &target("1.20.1", Loader::Forge));
        assert_eq!(report.kind, Some(ProjectKind::Mod));
        let info = report.mod_info.unwrap();
        assert_eq!(info.id, "sodium");
        assert_eq!(info.loaders, ["fabric"]);
        assert_eq!(
            report.incompatible,
            [Incompatibility::Loader {
                found: vec!["fabric".into()],
                pack: "forge".into()
            }]
        );
        // O lado vem do `environment` do jar.
        assert_eq!(report.side, (SideChoice::Client, None));
    }

    #[test]
    fn versao_do_minecraft_conferida_pela_faixa_do_jar() {
        let jar = sodium_jar();
        let fabric = read_content(&jar, "sodium.jar", &target("1.20.1", Loader::Fabric));
        assert!(fabric.incompatible.is_empty(), "{:?}", fabric.incompatible);
        let newer = read_content(&jar, "sodium.jar", &target("1.21.1", Loader::Fabric));
        assert_eq!(
            newer.incompatible,
            [Incompatibility::Minecraft {
                accepts: "1.20.1".into(),
                pack: "1.21.1".into()
            }]
        );
        // Quilt lê `fabric.mod.json`.
        let quilt = read_content(&jar, "sodium.jar", &target("1.20.1", Loader::Quilt));
        assert!(quilt.incompatible.is_empty());
    }

    #[test]
    fn descritores_de_cada_loader() {
        let forge = zip_of(&[(
            "META-INF/mods.toml",
            corpus("forge-1.20.1-jei/META-INF/mods.toml").as_bytes(),
        )]);
        let report = read_content(&forge, "jei.jar", &target("1.20.1", Loader::Forge));
        assert_eq!(report.mod_info.as_ref().unwrap().id, "jei");
        assert!(report.incompatible.is_empty(), "{:?}", report.incompatible);
        assert_eq!(report.side, (SideChoice::Both, Some(SideNote::Unknown)));
        let on_fabric = read_content(&forge, "jei.jar", &target("1.20.1", Loader::Fabric));
        assert!(matches!(
            on_fabric.incompatible[..],
            [Incompatibility::Loader { .. }]
        ));

        let neoforge = zip_of(&[(
            "META-INF/neoforge.mods.toml",
            corpus("neoforge-1.21.1-jei/META-INF/neoforge.mods.toml").as_bytes(),
        )]);
        // O JEI real para NeoForge "1.21.1" declara `[1.21, 1.21.1)`: o mesmo que o diagnóstico
        // apontaria (`E_MCVERSION`). Num pack 1.21 ele serve.
        let report = read_content(&neoforge, "jei.jar", &target("1.21", Loader::NeoForge));
        assert_eq!(report.mod_info.as_ref().unwrap().loaders, ["neoforge"]);
        assert!(report.incompatible.is_empty(), "{:?}", report.incompatible);
        let report = read_content(&neoforge, "jei.jar", &target("1.21.1", Loader::NeoForge));
        assert!(matches!(
            report.incompatible[..],
            [Incompatibility::Minecraft { .. }]
        ));

        let quilt = zip_of(&[(
            "quilt.mod.json",
            corpus("quilt-1.20.1-qkl/quilt.mod.json").as_bytes(),
        )]);
        let report = read_content(&quilt, "qkl.jar", &target("1.20.1", Loader::Quilt));
        assert!(report.mod_info.is_some());
        assert!(report.incompatible.is_empty(), "{:?}", report.incompatible);

        // `mcmod.info` do 1.7.10 (o formato tolerante, com a lista solta).
        let legacy = zip_of(&[(
            "mcmod.info",
            br#"[{"modid":"NotEnoughItems","name":"Not Enough Items","version":"1.0.5","mcversion":"1.7.10"}]"#,
        )]);
        let report = read_content(&legacy, "nei.jar", &target("1.7.10", Loader::Forge));
        let info = report.mod_info.unwrap();
        assert_eq!(info.name.as_deref(), Some("Not Enough Items"));
        assert_eq!(info.minecraft.as_deref(), Some("1.7.10"));
        assert!(report.incompatible.is_empty(), "{:?}", report.incompatible);
        let report = read_content(&legacy, "nei.jar", &target("1.12.2", Loader::Forge));
        assert!(matches!(
            report.incompatible[..],
            [Incompatibility::Minecraft { .. }]
        ));
    }

    #[test]
    fn tipo_pelo_conteudo_do_zip() {
        let resource = zip_of(&[("pack.mcmeta", b"{}"), ("assets/x.png", b"")]);
        let report = read_content(&resource, "Faithful.zip", &target("1.21.1", Loader::Fabric));
        assert_eq!(report.kind, Some(ProjectKind::ResourcePack));
        assert!(report.incompatible.is_empty());
        let shader = zip_of(&[("shaders/final.fsh", b"")]);
        let report = read_content(&shader, "BSL.zip", &target("1.21.1", Loader::Fabric));
        assert_eq!(report.kind, Some(ProjectKind::Shader));
        // Zip sem nada reconhecível: a tela pede o tipo.
        let other = zip_of(&[("leia.txt", b"oi")]);
        let report = read_content(&other, "algo.zip", &target("1.21.1", Loader::Fabric));
        assert_eq!(report.kind, None);
        assert!(report.archive);
    }

    #[test]
    fn conteudo_malicioso_ou_quebrado_nao_derruba() {
        let t = target("1.21.1", Loader::Fabric);
        // Não é zip.
        let report = read_content(b"isto nao e um zip", "mod.jar", &t);
        assert!(!report.archive);
        assert_eq!(report.kind, Some(ProjectKind::Mod));
        assert!(report.mod_info.is_none());
        assert!(report.incompatible.is_empty());
        // JSON inválido e caminhos que tentam sair da pasta (nada é extraído).
        let evil = zip_of(&[
            ("fabric.mod.json", b"{ isto: nao e json"),
            ("../../../../Windows/System32/x.dll", b"MZ"),
            ("/etc/passwd", b"root"),
        ]);
        let report = read_content(&evil, "mod.jar", &t);
        assert!(report.archive);
        assert!(report.mod_info.is_none());
        assert!(report.incompatible.is_empty());
        // Truncado no meio.
        let jar = sodium_jar();
        let report = read_content(&jar[..jar.len() / 2], "mod.jar", &t);
        assert!(report.mod_info.is_none());
    }

    #[test]
    fn acha_no_pack_o_mesmo_arquivo_e_o_mesmo_nome() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        pack(root, "1.20.1", Loader::Fabric);
        let jar = sodium_jar();
        std::fs::create_dir_all(root.join("mods")).unwrap();
        std::fs::write(root.join("mods/sodium.jar"), &jar).unwrap();
        let facts = FileFacts::of("sodium.jar", &jar);
        let index = PackIndex {
            hash_format: "sha256".into(),
            files: vec![IndexEntry::new("mods/sodium.jar", &facts.sha256)],
        };
        std::fs::write(root.join("index.toml"), index.to_toml_string()).unwrap();
        let report = inspect(root, &jar, &FileFacts::of("outro-nome.jar", &jar)).unwrap();
        assert_eq!(
            report.in_pack,
            Some(InPack {
                path: "mods/sodium.jar".into(),
                name: "sodium.jar".into(),
                same_file: true
            })
        );
        // Outro arquivo com o mesmo nome.
        let other = zip_of(&[("leia.txt", b"x")]);
        let report = inspect(root, &other, &FileFacts::of("SODIUM.jar", &other)).unwrap();
        assert!(matches!(
            report.in_pack,
            Some(InPack {
                same_file: false,
                ..
            })
        ));
        assert_eq!(report.kind, Some(ProjectKind::Mod));
        assert!(report.matches.is_empty());
    }

    #[tokio::test]
    async fn gravar_confere_o_hash_e_a_compatibilidade() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("pack");
        std::fs::create_dir_all(&root).unwrap();
        pack(&root, "1.20.1", Loader::Forge);
        let file = dir.path().join("sodium.jar");
        std::fs::write(&file, sodium_jar()).unwrap();
        let facts = FileFacts::of("sodium.jar", &sodium_jar());
        // O sidecar não chega a rodar: tudo falha antes da transação.
        let packwiz = Packwiz::new(
            dir.path().join("sem-packwiz"),
            dir.path().join("cache"),
            dir.path().join("config.toml"),
        );
        let item = LocalAdd {
            path: file.clone(),
            sha256: facts.sha256.clone(),
            kind: ProjectKind::Mod,
            link: None,
            name: String::new(),
            side: SideChoice::Both,
            accept_incompatible: false,
        };
        let cancel = CancellationToken::new();
        let error = add_local_apply(&root, std::slice::from_ref(&item), &packwiz, &cancel)
            .await
            .unwrap_err();
        assert_eq!(
            error.params.get("reason").map(String::as_str),
            Some("incompatible")
        );
        let changed = LocalAdd {
            sha256: "0".repeat(64),
            ..item.clone()
        };
        let error = add_local_apply(&root, &[changed], &packwiz, &cancel)
            .await
            .unwrap_err();
        assert_eq!(
            error.params.get("reason").map(String::as_str),
            Some("changed")
        );
        assert!(!root.join("mods").exists(), "nada foi gravado");
        let error = add_local_apply(&root, &[], &packwiz, &cancel)
            .await
            .unwrap_err();
        assert_eq!(error.code, Code::InvalidInput);
    }
}
