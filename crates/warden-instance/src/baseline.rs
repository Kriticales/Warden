//! Linha de base da instância de teste, tirada logo antes de abrir o jogo (ARCHITECTURE §8.3
//! item 1; SPEC T15; C-03).
//!
//! A "foto" tem, para cada arquivo da pasta do jogo fora da lista de ignorados
//! ([`CaptureIgnore`], `data/capture-ignore.txt`): tamanho, data de modificação e SHA-256. O
//! conteúdo dos arquivos de texto de até [`MAX_TEXT_BYTES`] em `config/`, `defaultconfigs/`,
//! `kubejs/`, `scripts/`, nas opções da raiz (`options.txt`…) e em `saves/*/serverconfig/` fica
//! guardado em `state/baseline/blobs/`, endereçado pelo SHA-256: é a versão "antes" das
//! diferenças por valor e a base do conflito de três vias.
//!
//! Também fica registrado o arquivo **do pack** de cada config que veio do pack (pelo
//! manifesto da instância), com o conteúdo: é o "pack no início do teste", para saber se o
//! mesmo arquivo foi editado no pack enquanto o jogo estava aberto (CA-T15-05).
//!
//! Para não ler tudo a cada teste, o SHA-256 de um arquivo com o mesmo tamanho e a mesma data
//! da linha de base anterior (ou do manifesto) é reaproveitado.
//!
//! Privacidade: os erros levam o caminho relativo à instância (`instância/…`) ou ao pack
//! (`pack/…`), nunca a pasta do usuário.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{self, Read as _};
use std::path::Path;
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use warden_core::atomic_write;
use warden_packwiz::{PACK_FILE, PackIndex, PackManifest, PackwizIgnore, clean_path};

use crate::error::{Error, Result};
use crate::manifest::{EntryOrigin, Manifest, modified_ns};
use crate::materialize::InstanceDirs;

/// Pasta da linha de base dentro de `state/`.
pub const BASELINE_DIR: &str = "baseline";

/// Arquivo da linha de base dentro de [`BASELINE_DIR`].
pub(crate) const BASELINE_FILE: &str = "baseline.json";

/// Pasta dos conteúdos guardados (por SHA-256) dentro de [`BASELINE_DIR`].
const BLOBS_DIR: &str = "blobs";

/// Versão do formato da linha de base.
pub(crate) const BASELINE_SCHEMA: u32 = 1;

/// Maior arquivo de texto cujo conteúdo é guardado (1 MB, ARCHITECTURE §8.3).
pub const MAX_TEXT_BYTES: u64 = 1024 * 1024;

/// A lista de sempre ignorados, embutida.
const CAPTURE_IGNORE: &str = include_str!("../data/capture-ignore.txt");

/// Pastas cujos arquivos de texto têm o conteúdo guardado.
const TEXT_DIRS: [&str; 4] = ["config/", "defaultconfigs/", "kubejs/", "scripts/"];

/// Arquivos soltos na raiz cujo conteúdo é guardado.
const TEXT_ROOT_FILES: [&str; 3] = ["options.txt", "optionsof.txt", "optionsshaders.txt"];

/// A lista "sempre ignorados" da captura (ARCHITECTURE §8.4).
#[derive(Debug, Clone)]
pub struct CaptureIgnore {
    patterns: PackwizIgnore,
}

impl CaptureIgnore {
    /// A lista de `data/capture-ignore.txt`.
    #[must_use]
    pub fn builtin() -> &'static Self {
        static BUILTIN: OnceLock<CaptureIgnore> = OnceLock::new();
        BUILTIN.get_or_init(|| Self::from_text(CAPTURE_IGNORE))
    }

    /// Uma lista no mesmo formato (testes).
    #[must_use]
    pub fn from_text(text: &str) -> Self {
        Self {
            patterns: PackwizIgnore::from_lines(text.lines().map(str::trim_end)),
        }
    }

    /// Se a pasta (relativa, sem `/` no fim) não deve ser percorrida.
    #[must_use]
    pub fn skips_dir(&self, dir: &str) -> bool {
        self.patterns.matches(dir)
    }

    /// Se o arquivo (relativo) fica de fora, olhando também as pastas acima dele.
    #[must_use]
    pub fn ignores(&self, file: &str) -> bool {
        let mut end = 0;
        while let Some(offset) = file[end..].find('/') {
            if self.skips_dir(&file[..end + offset]) {
                return true;
            }
            end += offset + 1;
        }
        self.patterns.matches(file)
    }
}

/// Um arquivo da instância na linha de base.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileState {
    /// Tamanho em bytes.
    pub size: u64,
    /// Data de modificação (nanossegundos desde 1970).
    pub modified_ns: u64,
    /// SHA-256 do conteúdo.
    pub sha256: String,
    /// Se o conteúdo está guardado em `blobs/`.
    #[serde(default)]
    pub stored: bool,
}

/// O arquivo do pack de uma config que veio do pack, no início do teste.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackStart {
    /// Caminho no pack (relativo à pasta do pack, com `/`).
    pub pack_path: String,
    /// SHA-256 do arquivo do pack (`None`: não existia).
    pub sha256: Option<String>,
    /// Se o conteúdo está guardado em `blobs/`.
    #[serde(default)]
    pub stored: bool,
}

/// A linha de base inteira (`state/baseline/baseline.json`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Baseline {
    /// Versão do formato.
    pub schema_version: u32,
    /// Quando foi tirada (milissegundos desde 1970).
    pub taken_at_ms: u64,
    /// Arquivos da instância, pelo caminho relativo à pasta do jogo.
    pub files: BTreeMap<String, FileState>,
    /// Configs que vieram do pack: caminho na instância → arquivo do pack no início.
    pub pack: BTreeMap<String, PackStart>,
}

impl Baseline {
    /// Lê a linha de base de `state_dir` (`None` se não existe ou é de outra versão).
    pub fn load(state_dir: &Path) -> Result<Option<Self>> {
        let path = state_dir.join(BASELINE_DIR).join(BASELINE_FILE);
        let bytes = match fs::read(&path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(Error::io("ler", state_path(BASELINE_FILE), error)),
        };
        match serde_json::from_slice::<Self>(&bytes) {
            Ok(baseline) if baseline.schema_version == BASELINE_SCHEMA => Ok(Some(baseline)),
            Ok(_) => Ok(None),
            Err(error) => {
                tracing::warn!(%error, "linha de base da captura ilegível; ignorada");
                Ok(None)
            }
        }
    }

    fn save(&self, state_dir: &Path) -> Result<()> {
        let dir = state_dir.join(BASELINE_DIR);
        fs::create_dir_all(&dir).map_err(|e| Error::io("criar a pasta", state_path(""), e))?;
        let mut bytes = serde_json::to_vec_pretty(self)
            .map_err(|error| Error::Internal(format!("linha de base em JSON: {error}")))?;
        bytes.push(b'\n');
        atomic_write(&dir.join(BASELINE_FILE), &bytes)?;
        Ok(())
    }
}

/// Os conteúdos guardados, por SHA-256 (`state/baseline/blobs/<sha256>`).
#[derive(Debug, Clone)]
pub(crate) struct BlobStore {
    dir: std::path::PathBuf,
}

impl BlobStore {
    /// O depósito da instância.
    #[must_use]
    pub(crate) fn of(state_dir: &Path) -> Self {
        Self {
            dir: state_dir.join(BASELINE_DIR).join(BLOBS_DIR),
        }
    }

    /// Guarda o conteúdo (se ainda não está) e devolve o SHA-256.
    pub(crate) fn put(&self, bytes: &[u8]) -> Result<String> {
        let sha256 = sha256_hex(bytes);
        let path = self.dir.join(&sha256);
        if !path.is_file() {
            fs::create_dir_all(&self.dir)
                .map_err(|e| Error::io("criar a pasta", state_path(BLOBS_DIR), e))?;
            atomic_write(&path, bytes)?;
        }
        Ok(sha256)
    }

    /// O conteúdo guardado com esse SHA-256 (`None` se não está guardado).
    pub(crate) fn get(&self, sha256: &str) -> Result<Option<Vec<u8>>> {
        if sha256.is_empty() || !sha256.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Ok(None);
        }
        match fs::read(self.dir.join(sha256)) {
            Ok(bytes) => Ok(Some(bytes)),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(Error::io("ler", state_path(BLOBS_DIR), error)),
        }
    }

    /// Apaga os conteúdos que não estão em `keep`.
    pub(crate) fn retain(&self, keep: &BTreeSet<String>) -> Result<()> {
        let entries = match fs::read_dir(&self.dir) {
            Ok(entries) => entries,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(Error::io("ler a pasta", state_path(BLOBS_DIR), error)),
        };
        for entry in entries.flatten() {
            let name = entry.file_name();
            if name.to_str().is_some_and(|name| keep.contains(name)) {
                continue;
            }
            if let Err(error) = fs::remove_file(entry.path()) {
                tracing::warn!(%error, "conteúdo antigo da linha de base não pôde ser apagado");
            }
        }
        Ok(())
    }
}

/// Caminho para mensagens: relativo ao estado da instância.
pub(crate) fn state_path(relative: &str) -> String {
    if relative.is_empty() {
        format!("instância/state/{BASELINE_DIR}")
    } else {
        format!("instância/state/{BASELINE_DIR}/{relative}")
    }
}

/// Caminho para mensagens: relativo à pasta do jogo da instância.
pub(crate) fn instance_path(relative: &str) -> String {
    format!("instância/{relative}")
}

/// Caminho para mensagens: relativo ao pack.
pub(crate) fn pack_path(relative: &str) -> String {
    format!("pack/{relative}")
}

/// SHA-256 em hexadecimal minúsculo.
#[must_use]
pub(crate) fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

/// Milissegundos desde 1970 (0 se o relógio estiver antes disso).
pub(crate) fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|elapsed| u64::try_from(elapsed.as_millis()).ok())
        .unwrap_or(0)
}

/// Se o conteúdo do arquivo é guardado (texto de config, opções, scripts, config de mundo).
#[must_use]
pub(crate) fn keeps_text(relative: &str) -> bool {
    TEXT_ROOT_FILES.contains(&relative)
        || TEXT_DIRS.iter().any(|dir| relative.starts_with(dir))
        || world_config_rest(relative).is_some()
}

/// `saves/<mundo>/serverconfig/<resto>` → `<resto>`.
#[must_use]
pub(crate) fn world_config_rest(relative: &str) -> Option<&str> {
    let rest = relative.strip_prefix("saves/")?;
    let (world, rest) = rest.split_once('/')?;
    if world.is_empty() {
        return None;
    }
    rest.strip_prefix("serverconfig/")
        .filter(|rest| !rest.is_empty())
}

/// Um arquivo encontrado na varredura.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Seen {
    pub(crate) size: u64,
    pub(crate) modified_ns: u64,
}

/// Varre a pasta do jogo: arquivos comuns (sem seguir atalhos) fora da lista de ignorados,
/// pelo caminho relativo com `/`. Nomes que não são UTF-8 ficam de fora (com aviso).
pub(crate) fn scan(game_dir: &Path, ignore: &CaptureIgnore) -> Result<BTreeMap<String, Seen>> {
    let mut found = BTreeMap::new();
    let mut pending = vec![String::new()];
    while let Some(relative_dir) = pending.pop() {
        let dir = if relative_dir.is_empty() {
            game_dir.to_path_buf()
        } else {
            game_dir.join(&relative_dir)
        };
        let entries = match fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(error) if error.kind() == io::ErrorKind::NotFound && relative_dir.is_empty() => {
                return Ok(found);
            }
            Err(error) => {
                return Err(Error::io(
                    "ler a pasta",
                    instance_path(&relative_dir),
                    error,
                ));
            }
        };
        for entry in entries {
            let entry =
                entry.map_err(|e| Error::io("ler a pasta", instance_path(&relative_dir), e))?;
            let Some(name) = entry.file_name().to_str().map(ToOwned::to_owned) else {
                tracing::warn!("arquivo com nome fora de UTF-8 ignorado na captura");
                continue;
            };
            let relative = if relative_dir.is_empty() {
                name
            } else {
                format!("{relative_dir}/{name}")
            };
            let metadata = match fs::symlink_metadata(entry.path()) {
                Ok(metadata) => metadata,
                // Apagado durante a varredura.
                Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
                Err(error) => return Err(Error::io("ler", instance_path(&relative), error)),
            };
            if metadata.is_dir() {
                if !ignore.skips_dir(&relative) {
                    pending.push(relative);
                }
            } else if metadata.is_file() && !ignore.ignores(&relative) {
                found.insert(
                    relative,
                    Seen {
                        size: metadata.len(),
                        modified_ns: modified_ns(&metadata),
                    },
                );
            }
        }
    }
    Ok(found)
}

/// SHA-256 de um arquivo, lido em pedaços.
pub(crate) fn hash_file(path: &Path, shown: &str) -> Result<String> {
    let mut file = fs::File::open(path).map_err(|e| Error::io("ler", instance_path(shown), e))?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0_u8; 64 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|e| Error::io("ler", instance_path(shown), e))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hex::encode(hasher.finalize()))
}

/// O que a captura precisa saber do pack: a pasta do índice e os arquivos com `preserve`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct PackLayout {
    /// A pasta do índice dentro do pack (`""` ou `"pasta/"`).
    pub(crate) index_dir: String,
    /// Destinos (caminho na instância) marcados com `preserve = true`: a materialização não
    /// os devolve à versão do pack, então a captura também não.
    pub(crate) preserved: BTreeSet<String>,
}

/// Lê o `pack.toml` e o índice. Pack ilegível: sem pasta de índice e sem `preserve`.
pub(crate) fn pack_layout(pack_root: &Path) -> PackLayout {
    let Ok(text) = fs::read_to_string(pack_root.join(PACK_FILE)) else {
        return PackLayout::default();
    };
    let Ok(pack) = PackManifest::parse(&text) else {
        return PackLayout::default();
    };
    let index_file = pack.value.index.file;
    let index_dir = match index_file.rsplit_once('/') {
        Some((dir, _)) => format!("{dir}/"),
        None => String::new(),
    };
    let preserved = warden_packwiz::check_relative_path(&index_file)
        .ok()
        .and_then(|()| fs::read_to_string(pack_root.join(&index_file)).ok())
        .and_then(|text| PackIndex::parse(&text).ok())
        .map(|index| {
            index
                .value
                .files
                .iter()
                .filter(|entry| entry.preserve && !entry.metafile)
                .map(|entry| {
                    let file = if entry.alias.is_empty() {
                        &entry.file
                    } else {
                        &entry.alias
                    };
                    clean_path(&format!("{index_dir}{file}"))
                })
                .collect()
        })
        .unwrap_or_default();
    PackLayout {
        index_dir,
        preserved,
    }
}

/// Lê um arquivo do pack (`None` se não existe).
pub(crate) fn read_pack_file(pack_root: &Path, relative: &str) -> Result<Option<Vec<u8>>> {
    let path = warden_core::resolve_inside(pack_root, relative)?;
    match fs::read(&path) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(error)
            if matches!(
                error.kind(),
                io::ErrorKind::NotFound | io::ErrorKind::IsADirectory
            ) =>
        {
            Ok(None)
        }
        Err(error) => Err(Error::io("ler", pack_path(relative), error)),
    }
}

/// Tira a linha de base da instância, logo antes de abrir o jogo, e a grava em
/// `state/baseline/`. Os conteúdos que não fazem mais parte dela são apagados.
///
/// Erros: disco.
pub fn take_baseline(dirs: &InstanceDirs, pack_root: &Path) -> Result<Baseline> {
    let ignore = CaptureIgnore::builtin();
    let manifest = Manifest::load(&dirs.state_dir)?;
    let previous = Baseline::load(&dirs.state_dir)?;
    let blobs = BlobStore::of(&dirs.state_dir);
    let seen = scan(&dirs.game_dir, ignore)?;

    let mut files = BTreeMap::new();
    let mut keep = BTreeSet::new();
    for (relative, now) in seen {
        let path = dirs.game_dir.join(&relative);
        let store = keeps_text(&relative) && now.size <= MAX_TEXT_BYTES;
        let state = if store {
            let bytes = match fs::read(&path) {
                Ok(bytes) => bytes,
                Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
                Err(error) => return Err(Error::io("ler", instance_path(&relative), error)),
            };
            let sha256 = blobs.put(&bytes)?;
            keep.insert(sha256.clone());
            FileState {
                size: u64::try_from(bytes.len()).unwrap_or(u64::MAX),
                modified_ns: now.modified_ns,
                sha256,
                stored: true,
            }
        } else {
            let known = previous
                .as_ref()
                .and_then(|base| base.files.get(&relative))
                .filter(|old| old.size == now.size && old.modified_ns == now.modified_ns)
                .map(|old| old.sha256.clone())
                .or_else(|| {
                    manifest
                        .files
                        .get(&relative)
                        .filter(|entry| {
                            entry.size == now.size && entry.modified_ns == now.modified_ns
                        })
                        .map(|entry| entry.sha256.to_ascii_lowercase())
                });
            let sha256 = match known {
                Some(sha256) => sha256,
                None => match hash_file(&path, &relative) {
                    Ok(sha256) => sha256,
                    Err(Error::Io { source, .. }) if source.kind() == io::ErrorKind::NotFound => {
                        continue;
                    }
                    Err(error) => return Err(error),
                },
            };
            FileState {
                size: now.size,
                modified_ns: now.modified_ns,
                sha256,
                stored: false,
            }
        };
        files.insert(relative, state);
    }

    let index_dir = pack_layout(pack_root).index_dir;
    let mut pack = BTreeMap::new();
    for (relative, entry) in &manifest.files {
        if entry.origin != EntryOrigin::Pack || !keeps_text(relative) {
            continue;
        }
        let pack_relative = format!("{index_dir}{}", entry.index_path);
        let start = match read_pack_file(pack_root, &pack_relative)? {
            Some(bytes) if u64::try_from(bytes.len()).unwrap_or(u64::MAX) <= MAX_TEXT_BYTES => {
                let sha256 = blobs.put(&bytes)?;
                keep.insert(sha256.clone());
                PackStart {
                    pack_path: pack_relative,
                    sha256: Some(sha256),
                    stored: true,
                }
            }
            Some(bytes) => PackStart {
                pack_path: pack_relative,
                sha256: Some(sha256_hex(&bytes)),
                stored: false,
            },
            None => PackStart {
                pack_path: pack_relative,
                sha256: None,
                stored: false,
            },
        };
        pack.insert(relative.clone(), start);
    }

    let baseline = Baseline {
        schema_version: BASELINE_SCHEMA,
        taken_at_ms: now_ms(),
        files,
        pack,
    };
    baseline.save(&dirs.state_dir)?;
    // A captura anterior (já resolvida) e os conteúdos dela deixam de valer.
    crate::capture::forget_capture(&dirs.state_dir)?;
    blobs.retain(&keep)?;
    Ok(baseline)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ignored(path: &str) -> bool {
        CaptureIgnore::builtin().ignores(path)
    }

    #[test]
    fn lista_de_ignorados_da_architecture() {
        for path in [
            "logs/latest.log",
            "logs/debug-1.log.gz",
            "crash-reports/crash-2026.txt",
            "debug/profiling.txt",
            "saves/Mundo de teste/level.dat",
            "saves/Mundo de teste/region/r.0.0.mca",
            "saves/Mundo/playerdata/x.dat",
            "screenshots/2026-10-13.png",
            "stats/x.json",
            "resources/sons.ogg",
            "natives/lwjgl.dll",
            "bin/x",
            "usercache.json",
            "usernamecache.json",
            "hotbar.nbt",
            "command_history.txt",
            "realms_persistence.json",
            "hs_err_pid1234.log",
            "replay_pid1234.log",
            "java_pid1.heapdump",
            ".fabric/remappedJars/a.jar",
            ".quilt/x",
            ".mixin.out/class/A.class",
            ".cache/x",
            "local/qualquer.txt",
            "mods/.connector/x.jar",
            "modernfix/cache.bin",
            "kubejs/exported/tags.json",
            "kubejs/probe/x.d.ts",
            "journeymap/data/sp/mundo/x.dat",
            "XaeroWaypoints/x.txt",
            "XaeroWorldMap/x",
            "xaero/minimap.txt",
            "packwiz.json",
            "packwiz-installer-bootstrap.jar",
            "packwiz-installer.jar",
            "config/create-common-1.toml.bak",
            "config/x.old",
            "config/x.tmp",
            "servers.dat_old",
            "config/x.toml.1-2.warden-tmp",
        ] {
            assert!(ignored(path), "{path} devia ser ignorado");
        }
    }

    #[test]
    fn spark_kubejs_e_probejs_ficam_de_fora() {
        // CA-T15-06.
        for path in [
            "config/spark/perfil.sparkprofile",
            "config/spark/heap.sparkheap",
            "config/spark/tmp",
            "config/spark/tmp123/a.txt",
            "config/spark/tmp-client/x",
            "kubejs/config/web_server.json",
            ".probe/types.d.ts",
            "kubejs/.probe/x.d.ts",
            ".vscode/settings.json",
            "kubejs/.vscode/settings.json",
            "local/kubejs/exported/x.json",
        ] {
            assert!(ignored(path), "{path} devia ser ignorado");
        }
        assert!(!ignored("config/spark/config.json"));
        assert!(!ignored("kubejs/config/client.properties"));
    }

    #[test]
    fn configs_mundos_mods_e_opcoes_entram() {
        for path in [
            "options.txt",
            "optionsof.txt",
            "servers.dat",
            "config/sodium-options.json",
            "config/Pasta com espaço/ação.toml",
            "defaultconfigs/create-server.toml",
            "kubejs/server_scripts/receitas.js",
            "scripts/receitas.zs",
            "mods/jei-1.20.1.jar",
            "resourcepacks/Faithful.zip",
            "shaderpacks/BSL.zip",
            "saves/Mundo de teste/serverconfig/create-server.toml",
            "saves/Mundo de teste/datapacks/meu.zip",
            "saves/Mundo de teste/datapacks/pasta/pack.mcmeta",
        ] {
            assert!(!ignored(path), "{path} não devia ser ignorado");
        }
        let ignore = CaptureIgnore::builtin();
        assert!(ignore.skips_dir("logs"));
        assert!(ignore.skips_dir("saves/Mundo/region"));
        assert!(!ignore.skips_dir("saves"));
        assert!(!ignore.skips_dir("saves/Mundo"));
        assert!(!ignore.skips_dir("saves/Mundo/serverconfig"));
        assert!(!ignore.skips_dir("config"));
    }

    #[test]
    fn texto_guardado_so_nas_pastas_de_config() {
        assert!(keeps_text("options.txt"));
        assert!(keeps_text("config/a.toml"));
        assert!(keeps_text("kubejs/server_scripts/a.js"));
        assert!(keeps_text("scripts/a.zs"));
        assert!(keeps_text("saves/Mundo/serverconfig/a.toml"));
        assert!(!keeps_text("mods/a.jar"));
        assert!(!keeps_text("servers.dat"));
        assert!(!keeps_text("saves/Mundo/datapacks/a.zip"));
        assert_eq!(
            world_config_rest("saves/Mundo de teste/serverconfig/create-server.toml"),
            Some("create-server.toml")
        );
        assert_eq!(world_config_rest("saves/Mundo/serverconfig/"), None);
        assert_eq!(world_config_rest("saves//serverconfig/a.toml"), None);
        assert_eq!(world_config_rest("config/a.toml"), None);
    }

    #[test]
    fn deposito_guarda_por_hash_e_limpa_o_resto() {
        let dir = crate::test_support::temp_dir();
        let blobs = BlobStore::of(dir.path());
        let a = blobs.put(b"a = 1\n").unwrap();
        let b = blobs.put(b"b = 2\n").unwrap();
        assert_eq!(a, sha256_hex(b"a = 1\n"));
        assert_eq!(blobs.get(&a).unwrap().unwrap(), b"a = 1\n");
        assert_eq!(blobs.get("../../x").unwrap(), None);
        blobs.retain(&BTreeSet::from([a.clone()])).unwrap();
        assert!(blobs.get(&a).unwrap().is_some());
        assert!(blobs.get(&b).unwrap().is_none());
    }
}
