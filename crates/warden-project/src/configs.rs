//! Editor de configs do pack e da instância de teste (SPEC T12; ARCHITECTURE §10; C-02).
//!
//! Três partes, todas sobre o disco:
//!
//! - [`list_files`]: os arquivos de texto que o editor mostra, por origem ([`ConfigOrigin`]).
//!   Só entram as pastas de configs e scripts; `mods/`, o `pack.toml` e o índice nunca passam
//!   por aqui.
//! - [`read_file`]: o texto de um arquivo com o `hash` do conteúdo (SHA-256), o fim de linha e o
//!   motivo de abrir só para leitura (binário, fora de UTF-8, acima de 2 MB).
//! - [`save_to_pack`] e [`save_to_instance`]: gravam o texto do usuário com **concorrência
//!   otimista**: o `hash` esperado é conferido contra o disco, e se o arquivo mudou fora do
//!   Warden nada é gravado (`FILE_CHANGED_ON_DISK`). No pack a gravação passa pela
//!   [`PackTransaction`] (escrita atômica, `packwiz refresh` e reversão em falha); na
//!   instância é uma gravação atômica, sem `refresh`.
//!
//! O texto do usuário é gravado como veio. O que o editor não mostra é devolvido pelo Warden:
//! o BOM do arquivo (o texto lido não o traz) é mantido, e o fim de linha é preservado pelo
//! editor (a interface usa o `lineEnding` devolvido na leitura). Abrir e salvar sem mudança não
//! toca no disco (CA-T12-01). Nenhum `.bak` é criado, e `.warden-tmp` que sobrou de uma
//! gravação interrompida é apagado antes da próxima gravação (CA-T12-05).

use std::fs;
use std::io::Read as _;
use std::path::Path;

use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use warden_core::{
    CancellationToken, TEMP_SUFFIX, atomic_write, remove_temp_files, resolve_inside,
};
use warden_packwiz_cli::Packwiz;

use crate::transaction::PackTransaction;
use crate::{Error, ProjectErrorCode as Code, Result};

/// Maior arquivo editado no editor (2 MB, SPEC T12); acima disso, só leitura com aviso.
pub const MAX_EDIT_BYTES: u64 = warden_configs::MAX_STRUCTURED_BYTES as u64;

/// Maior arquivo que o editor mostra mesmo em leitura (8 MB); acima disso, só o tamanho.
pub const MAX_VIEW_BYTES: u64 = 8 * 1024 * 1024;

/// Máximo de arquivos devolvidos por origem; passou disso, `truncated` fica ligado.
pub const MAX_LISTED_FILES: usize = 20_000;

/// Quantos bytes do começo do arquivo dizem se ele é binário.
const SNIFF_BYTES: usize = 8 * 1024;

/// Pastas do pack que o editor mostra.
const PACK_DIRS: [&str; 4] = ["config", "defaultconfigs", "kubejs", "scripts"];

/// Pastas da instância de teste que o editor mostra (e `saves/*/serverconfig/`).
const INSTANCE_DIRS: [&str; 2] = ["config", "defaultconfigs"];

/// Arquivos soltos na raiz (as opções do jogo e do `OptiFine`).
const ROOT_FILES: [&str; 3] = ["options.txt", "optionsof.txt", "optionsshaders.txt"];

/// De onde vêm os arquivos mostrados.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum ConfigOrigin {
    /// A pasta do pack (o que vai para os jogadores).
    Pack,
    /// A instância de teste (vale só para o teste; vira "O que mudou durante o teste").
    Instance,
}

/// Um arquivo da árvore.
#[derive(Debug, Clone, PartialEq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ConfigFile {
    /// Caminho relativo à origem, com `/`.
    pub path: String,
    /// Tamanho em bytes.
    pub size: u32,
    /// Última alteração, em milissegundos Unix.
    pub modified_at_ms: Option<f64>,
    /// Se o arquivo parece binário (aparece sem edição).
    pub binary: bool,
}

/// Os arquivos de uma origem.
#[derive(Debug, Clone, PartialEq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ConfigFileList {
    /// A pasta da origem existe (a instância só existe depois do primeiro teste).
    pub available: bool,
    /// Arquivos, em ordem de caminho.
    pub files: Vec<ConfigFile>,
    /// A lista foi cortada em [`MAX_LISTED_FILES`].
    pub truncated: bool,
}

/// Fim de linha do arquivo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum LineEnding {
    /// `\n` (ou nenhuma quebra).
    Lf,
    /// `\r\n` em todas as linhas.
    Crlf,
    /// Mistura de fins de linha (ou `\r` solto). O editor preserva cada byte.
    Mixed,
}

/// Por que um arquivo abre só para leitura.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum ReadOnlyReason {
    /// Binário (ex.: `servers.dat`): sem texto.
    Binary,
    /// Acima de [`MAX_EDIT_BYTES`]. O texto vem junto até [`MAX_VIEW_BYTES`].
    TooLarge,
    /// Não é UTF-8: o texto vem com os bytes inválidos trocados, só para olhar.
    NotUtf8,
}

/// Um arquivo lido.
#[derive(Debug, Clone, PartialEq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ConfigContent {
    /// Caminho relativo à origem.
    pub path: String,
    /// Texto sem o BOM; `None` quando não há como mostrar.
    pub text: Option<String>,
    /// SHA-256 do arquivo inteiro (hexadecimal), para a concorrência otimista.
    pub hash: String,
    /// Tamanho em bytes.
    pub size: u32,
    /// Fim de linha.
    pub line_ending: LineEnding,
    /// Só leitura, e por quê. `None` = editável.
    pub read_only: Option<ReadOnlyReason>,
    /// Última alteração, em milissegundos Unix.
    pub modified_at_ms: Option<f64>,
}

/// Resultado de uma gravação.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ConfigSaved {
    /// SHA-256 do arquivo depois da gravação (o `hash` esperado da próxima).
    pub hash: String,
    /// Tamanho em bytes.
    pub size: u32,
    /// `false` quando o texto era igual ao do disco e nada foi gravado.
    pub changed: bool,
}

/// SHA-256 em hexadecimal.
#[must_use]
pub fn hash_bytes(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn size_u32(size: u64) -> u32 {
    u32::try_from(size).unwrap_or(u32::MAX)
}

// ---------------------------------------------------------------------------------------------
// Caminhos permitidos
// ---------------------------------------------------------------------------------------------

/// Divide o caminho em componentes sem `.` e vazios; `..` é recusado.
fn components(relative: &str) -> Result<Vec<&str>> {
    let mut parts = Vec::new();
    for part in relative.split(['/', '\\']) {
        match part {
            "" | "." => {}
            ".." => return Err(invalid_path(relative)),
            _ => parts.push(part),
        }
    }
    if parts.is_empty() {
        return Err(invalid_path(relative));
    }
    Ok(parts)
}

fn invalid_path(relative: &str) -> Error {
    Error::new(
        Code::InvalidInput,
        format!("caminho de config inválido: {relative}"),
    )
    .param("field", "path")
}

fn is(name: &str, expected: &str) -> bool {
    name.eq_ignore_ascii_case(expected)
}

/// Arquivos e pastas que existem na origem mas o editor esconde: tokens, lixo do jogo e
/// temporários do Warden.
fn is_hidden(parts: &[&str]) -> bool {
    let Some(last) = parts.last() else {
        return true;
    };
    let lower = last.to_ascii_lowercase();
    if lower.ends_with(TEMP_SUFFIX) {
        return true;
    }
    if parts
        .iter()
        .any(|part| is(part, ".probe") || is(part, ".vscode"))
    {
        return true;
    }
    // O token do servidor web do KubeJS 7 (ARCHITECTURE §8.4): nunca vai para a tela.
    if parts.len() == 3
        && is(parts[0], "kubejs")
        && is(parts[1], "config")
        && is(last, "web_server.json")
    {
        return true;
    }
    // Perfis do spark ficam dentro de `config/spark/` e não são configs.
    parts.len() >= 3
        && is(parts[0], "config")
        && is(parts[1], "spark")
        && (lower.ends_with(".sparkprofile")
            || lower.ends_with(".sparkheap")
            || lower.starts_with("tmp"))
}

/// Se o arquivo (ou a pasta) pode aparecer na origem.
fn is_listed(origin: ConfigOrigin, parts: &[&str]) -> bool {
    let (Some(first), Some(last)) = (parts.first(), parts.last()) else {
        return false;
    };
    if is_hidden(parts) {
        return false;
    }
    if parts.len() == 1 {
        return ROOT_FILES.iter().any(|name| is(last, name));
    }
    match origin {
        ConfigOrigin::Pack => PACK_DIRS.iter().any(|dir| is(first, dir)),
        ConfigOrigin::Instance => {
            INSTANCE_DIRS.iter().any(|dir| is(first, dir))
                || (is(first, "saves") && parts.len() >= 4 && is(parts[2], "serverconfig"))
        }
    }
}

/// Se uma pasta pode conter arquivos listados (a varredura só desce nelas).
fn is_descendable(origin: ConfigOrigin, parts: &[&str]) -> bool {
    if parts
        .iter()
        .any(|part| is(part, ".probe") || is(part, ".vscode"))
    {
        return false;
    }
    let Some(first) = parts.first() else {
        return false;
    };
    match origin {
        ConfigOrigin::Pack => PACK_DIRS.iter().any(|dir| is(first, dir)),
        ConfigOrigin::Instance => {
            if INSTANCE_DIRS.iter().any(|dir| is(first, dir)) {
                return true;
            }
            if !is(first, "saves") {
                return false;
            }
            match parts.len() {
                1 | 2 => true,
                _ => is(parts[2], "serverconfig"),
            }
        }
    }
}

/// Valida o caminho pedido pela interface e devolve o caminho no disco e o normalizado.
fn resolve(
    root: &Path,
    origin: ConfigOrigin,
    relative: &str,
) -> Result<(std::path::PathBuf, String)> {
    let parts = components(relative)?;
    if !is_listed(origin, &parts) {
        return Err(invalid_path(relative));
    }
    let normalized = parts.join("/");
    let path = resolve_inside(root, &normalized).map_err(|error| {
        Error::new(Code::InvalidInput, error.to_string()).param("field", "path")
    })?;
    Ok((path, normalized))
}

// ---------------------------------------------------------------------------------------------
// Listagem
// ---------------------------------------------------------------------------------------------

fn modified_ms(metadata: &fs::Metadata) -> Option<f64> {
    let elapsed = metadata
        .modified()
        .ok()?
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?;
    #[allow(clippy::cast_precision_loss)] // milissegundos Unix cabem folgados em f64
    Some(elapsed.as_millis() as f64)
}

fn looks_binary(path: &Path) -> bool {
    let mut buffer = [0u8; SNIFF_BYTES];
    let Ok(mut file) = fs::File::open(path) else {
        return false;
    };
    let mut filled = 0;
    while filled < buffer.len() {
        match file.read(&mut buffer[filled..]) {
            Ok(0) | Err(_) => break,
            Ok(read) => filled += read,
        }
    }
    buffer[..filled].contains(&0)
}

/// Os arquivos da origem que o editor mostra.
///
/// Links simbólicos são ignorados (não dá para garantir que ficam dentro da origem). Uma pasta
/// ilegível é pulada; o resto da árvore continua. Uma `root` que não existe devolve a lista
/// vazia com `available = false`.
pub fn list_files(root: &Path, origin: ConfigOrigin) -> Result<ConfigFileList> {
    if !root.is_dir() {
        return Ok(ConfigFileList {
            available: false,
            files: Vec::new(),
            truncated: false,
        });
    }
    let mut files = Vec::new();
    let mut truncated = false;
    let mut pending: Vec<Vec<String>> = vec![Vec::new()];
    'walk: while let Some(dir) = pending.pop() {
        let mut path = root.to_path_buf();
        path.extend(&dir);
        let Ok(entries) = fs::read_dir(&path) else {
            continue;
        };
        for entry in entries.flatten() {
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            if kind.is_symlink() {
                continue;
            }
            let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
                continue;
            };
            let mut parts: Vec<&str> = dir.iter().map(String::as_str).collect();
            parts.push(&name);
            if kind.is_dir() {
                if is_descendable(origin, &parts) {
                    let mut next = dir.clone();
                    next.push(name);
                    pending.push(next);
                }
            } else if kind.is_file() && is_listed(origin, &parts) {
                let Ok(metadata) = entry.metadata() else {
                    continue;
                };
                if files.len() >= MAX_LISTED_FILES {
                    truncated = true;
                    break 'walk;
                }
                files.push(ConfigFile {
                    path: parts.join("/"),
                    size: size_u32(metadata.len()),
                    modified_at_ms: modified_ms(&metadata),
                    binary: looks_binary(&entry.path()),
                });
            }
        }
    }
    files.sort_by_cached_key(|file| file.path.to_lowercase());
    Ok(ConfigFileList {
        available: true,
        files,
        truncated,
    })
}

// ---------------------------------------------------------------------------------------------
// Leitura
// ---------------------------------------------------------------------------------------------

fn line_ending_of(text: &str) -> LineEnding {
    let bytes = text.as_bytes();
    let (mut lf, mut crlf, mut cr) = (0usize, 0usize, 0usize);
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'\r' if bytes.get(index + 1) == Some(&b'\n') => {
                crlf += 1;
                index += 1;
            }
            b'\r' => cr += 1,
            b'\n' => lf += 1,
            _ => {}
        }
        index += 1;
    }
    if cr > 0 || (lf > 0 && crlf > 0) {
        LineEnding::Mixed
    } else if crlf > 0 {
        LineEnding::Crlf
    } else {
        LineEnding::Lf
    }
}

fn not_found(relative: &str) -> Error {
    Error::new(
        Code::ConfigNotFound,
        format!("o arquivo {relative} não existe mais"),
    )
    .param("path", relative)
}

fn io_error(path: &Path, error: &std::io::Error) -> Error {
    if error.kind() == std::io::ErrorKind::NotFound {
        return not_found(&path.display().to_string());
    }
    Error::new(Code::Internal, format!("{}: {error}", path.display()))
}

/// Estado do arquivo no disco, para ler e para conferir antes de gravar.
struct OnDisk {
    bytes: Option<Vec<u8>>,
    hash: String,
    size: u64,
    modified_at_ms: Option<f64>,
}

fn load(path: &Path, relative: &str) -> Result<OnDisk> {
    let metadata = fs::metadata(path).map_err(|error| io_error(path, &error))?;
    if !metadata.is_file() {
        return Err(not_found(relative));
    }
    let size = metadata.len();
    let mut file = fs::File::open(path).map_err(|error| io_error(path, &error))?;
    if size <= MAX_VIEW_BYTES {
        let mut bytes = Vec::with_capacity(usize::try_from(size).unwrap_or(0));
        file.read_to_end(&mut bytes)
            .map_err(|error| io_error(path, &error))?;
        return Ok(OnDisk {
            hash: hash_bytes(&bytes),
            size: bytes.len() as u64,
            bytes: Some(bytes),
            modified_at_ms: modified_ms(&metadata),
        });
    }
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; 64 * 1024];
    let mut total = 0u64;
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|error| io_error(path, &error))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
        total += read as u64;
    }
    Ok(OnDisk {
        bytes: None,
        hash: hex::encode(hasher.finalize()),
        size: total,
        modified_at_ms: modified_ms(&metadata),
    })
}

/// O motivo de abrir só para leitura, olhando os bytes (`None` = editável).
fn read_only_reason(bytes: &[u8]) -> Option<ReadOnlyReason> {
    if bytes.len() as u64 > MAX_EDIT_BYTES {
        return Some(ReadOnlyReason::TooLarge);
    }
    if bytes[..bytes.len().min(SNIFF_BYTES)].contains(&0) {
        return Some(ReadOnlyReason::Binary);
    }
    if std::str::from_utf8(bytes).is_err() {
        return Some(ReadOnlyReason::NotUtf8);
    }
    None
}

/// Lê um arquivo da origem. Erros: `INVALID_INPUT` (caminho fora do que o editor mostra),
/// `CONFIG_NOT_FOUND` (o arquivo não existe mais).
pub fn read_file(root: &Path, origin: ConfigOrigin, relative: &str) -> Result<ConfigContent> {
    let (path, normalized) = resolve(root, origin, relative)?;
    let disk = load(&path, &normalized)?;
    let size = size_u32(disk.size);
    let Some(bytes) = disk.bytes else {
        return Ok(ConfigContent {
            path: normalized,
            text: None,
            hash: disk.hash,
            size,
            line_ending: LineEnding::Lf,
            read_only: Some(ReadOnlyReason::TooLarge),
            modified_at_ms: disk.modified_at_ms,
        });
    };
    let reason = read_only_reason(&bytes);
    let text = if reason == Some(ReadOnlyReason::Binary) {
        None
    } else {
        let decoded = String::from_utf8_lossy(&bytes);
        Some(
            decoded
                .strip_prefix('\u{feff}')
                .unwrap_or(&decoded)
                .to_owned(),
        )
    };
    let line_ending = text.as_deref().map_or(LineEnding::Lf, line_ending_of);
    Ok(ConfigContent {
        path: normalized,
        text,
        hash: disk.hash,
        size,
        line_ending,
        read_only: reason,
        modified_at_ms: disk.modified_at_ms,
    })
}

// ---------------------------------------------------------------------------------------------
// Gravação
// ---------------------------------------------------------------------------------------------

enum Plan {
    /// O texto é igual ao do disco.
    Unchanged { hash: String, size: u32 },
    /// Há o que gravar.
    Write {
        normalized: String,
        path: std::path::PathBuf,
        bytes: Vec<u8>,
    },
}

/// Confere o caminho, o tipo do arquivo e o `hash` esperado, e monta os bytes novos.
fn plan(
    root: &Path,
    origin: ConfigOrigin,
    relative: &str,
    text: &str,
    expected_hash: &str,
) -> Result<Plan> {
    if text.len() as u64 > MAX_EDIT_BYTES {
        return Err(not_editable(ReadOnlyReason::TooLarge));
    }
    let (path, normalized) = resolve(root, origin, relative)?;
    let disk = load(&path, &normalized)?;
    if disk.hash != expected_hash {
        return Err(Error::new(
            Code::FileChangedOnDisk,
            format!("{normalized} mudou no disco desde que foi aberto"),
        )
        .param("path", normalized));
    }
    let Some(current) = disk.bytes else {
        return Err(not_editable(ReadOnlyReason::TooLarge));
    };
    if let Some(reason) = read_only_reason(&current) {
        return Err(not_editable(reason));
    }
    let mut bytes = Vec::with_capacity(text.len() + 3);
    if current.starts_with(&[0xEF, 0xBB, 0xBF]) {
        bytes.extend_from_slice(&[0xEF, 0xBB, 0xBF]);
    }
    bytes.extend_from_slice(text.as_bytes());
    if bytes == current {
        return Ok(Plan::Unchanged {
            hash: disk.hash,
            size: size_u32(disk.size),
        });
    }
    Ok(Plan::Write {
        normalized,
        path,
        bytes,
    })
}

fn not_editable(reason: ReadOnlyReason) -> Error {
    let name = match reason {
        ReadOnlyReason::Binary => "binary",
        ReadOnlyReason::TooLarge => "tooLarge",
        ReadOnlyReason::NotUtf8 => "notUtf8",
    };
    Error::new(
        Code::ConfigNotEditable,
        format!("o arquivo só abre para leitura ({name})"),
    )
    .param("reason", name)
}

/// Apaga `.warden-tmp` que sobraram de uma gravação interrompida na pasta do arquivo. A falha
/// da limpeza é descartada de propósito: o temporário que sobrar é ignorado pelo índice
/// (`*.warden-tmp` no bloco obrigatório do `.packwizignore`) e a próxima gravação tenta de novo.
fn sweep_leftovers(path: &Path) {
    if let Some(parent) = path.parent() {
        let _ = remove_temp_files(parent);
    }
}

/// Grava o texto no **pack**, pela [`PackTransaction`] (escrita atômica, `packwiz refresh` e
/// reversão de tudo se algo falhar). O chamador segura a trava de escrita do pack.
///
/// Erros: `FILE_CHANGED_ON_DISK` (o arquivo mudou desde a leitura; nada é gravado),
/// `CONFIG_NOT_EDITABLE`, `CONFIG_NOT_FOUND`, `INVALID_INPUT`.
pub async fn save_to_pack(
    root: &Path,
    relative: &str,
    text: &str,
    expected_hash: &str,
    packwiz: &Packwiz,
    cancel: &CancellationToken,
) -> Result<ConfigSaved> {
    match plan(root, ConfigOrigin::Pack, relative, text, expected_hash)? {
        Plan::Unchanged { hash, size } => Ok(ConfigSaved {
            hash,
            size,
            changed: false,
        }),
        Plan::Write {
            normalized,
            path,
            bytes,
        } => {
            sweep_leftovers(&path);
            let saved = ConfigSaved {
                hash: hash_bytes(&bytes),
                size: size_u32(bytes.len() as u64),
                changed: true,
            };
            let mut transaction = PackTransaction::new(root.to_path_buf());
            transaction.write(&normalized, bytes)?;
            transaction.commit(packwiz, cancel).await?;
            Ok(saved)
        }
    }
}

/// Grava o texto na **instância de teste**: gravação atômica no arquivo, sem `refresh` (a
/// instância não tem índice). O chamador segura a trava do pack.
pub fn save_to_instance(
    root: &Path,
    relative: &str,
    text: &str,
    expected_hash: &str,
) -> Result<ConfigSaved> {
    match plan(root, ConfigOrigin::Instance, relative, text, expected_hash)? {
        Plan::Unchanged { hash, size } => Ok(ConfigSaved {
            hash,
            size,
            changed: false,
        }),
        Plan::Write { path, bytes, .. } => {
            sweep_leftovers(&path);
            atomic_write(&path, &bytes)
                .map_err(|error| Error::new(Code::Internal, error.to_string()))?;
            Ok(ConfigSaved {
                hash: hash_bytes(&bytes),
                size: size_u32(bytes.len() as u64),
                changed: true,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(root: &Path, relative: &str, bytes: &[u8]) {
        let path = root.join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, bytes).unwrap();
    }

    fn names(list: &ConfigFileList) -> Vec<&str> {
        list.files.iter().map(|file| file.path.as_str()).collect()
    }

    #[test]
    fn lista_so_as_pastas_de_config_do_pack() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        for path in [
            "config/b.toml",
            "config/A.json",
            "config/jei/jei-client.ini",
            "defaultconfigs/x.toml",
            "kubejs/server_scripts/receitas.js",
            "kubejs/config/web_server.json",
            "kubejs/.probe/types.d.ts",
            "scripts/a.zs",
            "options.txt",
            "mods/sodium.pw.toml",
            "pack.toml",
            "index.toml",
            "notas.txt",
            "config/x.toml.1-0.warden-tmp",
            "config/spark/perfil.sparkprofile",
            "config/spark/tmp123",
            "config/spark/spark.json",
        ] {
            write(root, path, b"x");
        }
        let list = list_files(root, ConfigOrigin::Pack).unwrap();
        assert!(list.available && !list.truncated);
        assert_eq!(
            names(&list),
            [
                "config/A.json",
                "config/b.toml",
                "config/jei/jei-client.ini",
                "config/spark/spark.json",
                "defaultconfigs/x.toml",
                "kubejs/server_scripts/receitas.js",
                "options.txt",
                "scripts/a.zs",
            ]
        );
    }

    #[test]
    fn lista_a_instancia_com_serverconfig_dos_mundos() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        for path in [
            "config/a.toml",
            "defaultconfigs/b.toml",
            "options.txt",
            "saves/Mundo/serverconfig/create-server.toml",
            "saves/Mundo/level.dat",
            "saves/Mundo/region/r.0.0.mca",
            "saves/Mundo/datapacks/x/pack.mcmeta",
            "kubejs/server_scripts/a.js",
            "mods/a.jar",
            "logs/latest.log",
        ] {
            write(root, path, b"x");
        }
        let list = list_files(root, ConfigOrigin::Instance).unwrap();
        assert_eq!(
            names(&list),
            [
                "config/a.toml",
                "defaultconfigs/b.toml",
                "options.txt",
                "saves/Mundo/serverconfig/create-server.toml",
            ]
        );
    }

    #[test]
    fn origem_inexistente_nao_e_erro() {
        let dir = tempfile::tempdir().unwrap();
        let list = list_files(&dir.path().join("minecraft"), ConfigOrigin::Instance).unwrap();
        assert!(!list.available);
        assert!(list.files.is_empty());
    }

    #[test]
    fn marca_binarios() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "config/servers.dat", &[1, 0, 2, 0]);
        write(dir.path(), "config/a.toml", b"a = 1\n");
        let list = list_files(dir.path(), ConfigOrigin::Pack).unwrap();
        let binary: Vec<_> = list
            .files
            .iter()
            .map(|f| (f.path.as_str(), f.binary))
            .collect();
        assert_eq!(
            binary,
            [("config/a.toml", false), ("config/servers.dat", true)]
        );
    }

    #[test]
    fn recusa_caminhos_fora_do_que_o_editor_mostra() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        write(root, "pack.toml", b"x");
        write(root, "mods/a.pw.toml", b"x");
        write(root, "kubejs/config/web_server.json", b"{\"token\":\"x\"}");
        for path in [
            "pack.toml",
            "../fora.toml",
            "config/../pack.toml",
            "mods/a.pw.toml",
            "kubejs/config/web_server.json",
            "",
            "/etc/passwd",
            "C:/Windows/win.ini",
            "config",
        ] {
            let error = read_file(root, ConfigOrigin::Pack, path).unwrap_err();
            assert_eq!(error.code, Code::InvalidInput, "{path}");
        }
        let error = read_file(root, ConfigOrigin::Pack, "config/nao-existe.toml").unwrap_err();
        assert_eq!(error.code, Code::ConfigNotFound);
    }

    #[test]
    fn le_texto_sem_bom_e_com_hash() {
        let dir = tempfile::tempdir().unwrap();
        let bytes = "\u{feff}a = \"é\"\r\nb = 2\r\n".as_bytes();
        write(dir.path(), "config/a.toml", bytes);
        let content = read_file(dir.path(), ConfigOrigin::Pack, "config\\a.toml").unwrap();
        assert_eq!(content.path, "config/a.toml");
        assert_eq!(content.text.as_deref(), Some("a = \"é\"\r\nb = 2\r\n"));
        assert_eq!(content.hash, hash_bytes(bytes));
        assert_eq!(content.line_ending, LineEnding::Crlf);
        assert_eq!(content.read_only, None);
    }

    #[test]
    fn detecta_fim_de_linha() {
        assert_eq!(line_ending_of(""), LineEnding::Lf);
        assert_eq!(line_ending_of("a\nb\n"), LineEnding::Lf);
        assert_eq!(line_ending_of("a\r\nb\r\n"), LineEnding::Crlf);
        assert_eq!(line_ending_of("a\r\nb\n"), LineEnding::Mixed);
        assert_eq!(line_ending_of("a\rb"), LineEnding::Mixed);
    }

    #[test]
    fn binario_nao_traz_texto_e_fora_de_utf8_traz_so_para_olhar() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "config/servers.dat", &[10, 0, 11]);
        write(dir.path(), "config/latin1.cfg", b"nome=Jos\xe9\n");
        let binary = read_file(dir.path(), ConfigOrigin::Pack, "config/servers.dat").unwrap();
        assert_eq!(binary.text, None);
        assert_eq!(binary.read_only, Some(ReadOnlyReason::Binary));
        let latin = read_file(dir.path(), ConfigOrigin::Pack, "config/latin1.cfg").unwrap();
        assert_eq!(latin.read_only, Some(ReadOnlyReason::NotUtf8));
        assert_eq!(latin.text.as_deref(), Some("nome=Jos\u{fffd}\n"));
    }

    #[test]
    fn acima_de_2_mb_so_leitura_e_acima_de_8_mb_so_tamanho() {
        let dir = tempfile::tempdir().unwrap();
        let big = "a = 1\n".repeat(400_000);
        write(dir.path(), "config/grande.toml", big.as_bytes());
        let content = read_file(dir.path(), ConfigOrigin::Pack, "config/grande.toml").unwrap();
        assert_eq!(content.read_only, Some(ReadOnlyReason::TooLarge));
        assert_eq!(content.text.as_deref(), Some(big.as_str()));
        let huge = vec![b'a'; usize::try_from(MAX_VIEW_BYTES).unwrap() + 1];
        write(dir.path(), "config/enorme.txt", &huge);
        let content = read_file(dir.path(), ConfigOrigin::Pack, "config/enorme.txt").unwrap();
        assert_eq!(content.read_only, Some(ReadOnlyReason::TooLarge));
        assert_eq!(content.text, None);
        assert_eq!(content.hash, hash_bytes(&huge));
    }

    #[test]
    fn instancia_grava_com_hash_e_preserva_o_bom() {
        let dir = tempfile::tempdir().unwrap();
        let original = "\u{feff}a = 1\n".as_bytes();
        write(dir.path(), "config/a.toml", original);
        let read = read_file(dir.path(), ConfigOrigin::Instance, "config/a.toml").unwrap();
        let text = read.text.unwrap().replace("a = 1", "a = 2");
        let saved = save_to_instance(dir.path(), "config/a.toml", &text, &read.hash).unwrap();
        assert!(saved.changed);
        let on_disk = fs::read(dir.path().join("config/a.toml")).unwrap();
        assert_eq!(on_disk, "\u{feff}a = 2\n".as_bytes());
        assert_eq!(saved.hash, hash_bytes(&on_disk));
    }

    #[test]
    fn salvar_sem_mudanca_nao_toca_no_arquivo() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "config/a.toml", b"a = 1\r\n");
        let path = dir.path().join("config/a.toml");
        let before = fs::metadata(&path).unwrap().modified().unwrap();
        let read = read_file(dir.path(), ConfigOrigin::Instance, "config/a.toml").unwrap();
        let saved = save_to_instance(
            dir.path(),
            "config/a.toml",
            read.text.as_deref().unwrap(),
            &read.hash,
        )
        .unwrap();
        assert!(!saved.changed);
        assert_eq!(saved.hash, read.hash);
        assert_eq!(fs::metadata(&path).unwrap().modified().unwrap(), before);
    }

    #[test]
    fn arquivo_alterado_por_fora_nao_e_gravado() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "config/a.toml", b"a = 1\n");
        let read = read_file(dir.path(), ConfigOrigin::Instance, "config/a.toml").unwrap();
        write(dir.path(), "config/a.toml", b"a = 99\n");
        let error =
            save_to_instance(dir.path(), "config/a.toml", "a = 2\n", &read.hash).unwrap_err();
        assert_eq!(error.code, Code::FileChangedOnDisk);
        assert_eq!(
            fs::read(dir.path().join("config/a.toml")).unwrap(),
            b"a = 99\n"
        );
        // O arquivo apagado por fora também é "mudou".
        fs::remove_file(dir.path().join("config/a.toml")).unwrap();
        let error =
            save_to_instance(dir.path(), "config/a.toml", "a = 2\n", &read.hash).unwrap_err();
        assert_eq!(error.code, Code::ConfigNotFound);
    }

    #[test]
    fn so_leitura_nao_grava() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "config/servers.dat", &[1, 0, 1]);
        let read = read_file(dir.path(), ConfigOrigin::Instance, "config/servers.dat").unwrap();
        let error =
            save_to_instance(dir.path(), "config/servers.dat", "x", &read.hash).unwrap_err();
        assert_eq!(error.code, Code::ConfigNotEditable);
        assert_eq!(error.params["reason"], "binary");
        let error = save_to_instance(
            dir.path(),
            "config/servers.dat",
            &"a".repeat(usize::try_from(MAX_EDIT_BYTES).unwrap() + 1),
            &read.hash,
        )
        .unwrap_err();
        assert_eq!(error.code, Code::ConfigNotEditable);
    }

    #[test]
    fn falha_na_gravacao_deixa_o_original_e_nenhum_temporario() {
        for point in ["atomic_write.before_write", "atomic_write.before_rename"] {
            let dir = tempfile::tempdir().unwrap();
            write(dir.path(), "config/a.toml", b"a = 1\n");
            let read = read_file(dir.path(), ConfigOrigin::Instance, "config/a.toml").unwrap();
            let _armed = warden_core::fault::arm(point);
            let error =
                save_to_instance(dir.path(), "config/a.toml", "a = 2\n", &read.hash).unwrap_err();
            assert_eq!(error.code, Code::Internal, "{point}");
            assert_eq!(
                fs::read(dir.path().join("config/a.toml")).unwrap(),
                b"a = 1\n"
            );
            let left: Vec<_> = fs::read_dir(dir.path().join("config"))
                .unwrap()
                .map(|entry| entry.unwrap().file_name())
                .collect();
            assert_eq!(left, ["a.toml"], "{point}");
        }
    }

    #[test]
    fn temporario_de_gravacao_interrompida_some_na_proxima_gravacao() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "config/a.toml", b"a = 1\n");
        // O que um encerramento à força entre gravar e renomear deixa para trás.
        write(dir.path(), "config/a.toml.4242-0.warden-tmp", b"a = ");
        let read = read_file(dir.path(), ConfigOrigin::Instance, "config/a.toml").unwrap();
        save_to_instance(dir.path(), "config/a.toml", "a = 2\n", &read.hash).unwrap();
        let left: Vec<_> = fs::read_dir(dir.path().join("config"))
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect();
        assert_eq!(left, ["a.toml"]);
    }
}
