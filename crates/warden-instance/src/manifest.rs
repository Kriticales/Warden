//! Manifesto de estado da instância: `instances/<pack-id>/state/manifest.json`
//! (ARCHITECTURE §8.1 e §8.2 item 8; ADR-0011).
//!
//! Para cada arquivo que o Warden colocou na instância: caminho → SHA-256, tamanho, data de
//! modificação, origem e o hash do item no índice do pack. Serve para:
//!
//! - remover da instância só o que o Warden instalou e saiu do pack (§8.2 item 6);
//! - saber se um arquivo gerenciado foi alterado na instância (pelo jogo ou pela pessoa) antes
//!   de sobrescrevê-lo (§8.2 item 7);
//! - pular a releitura de arquivos que não mudaram (tamanho e data iguais), para a segunda
//!   materialização sem mudanças não ler nem escrever nada;
//! - a captura (T15): "alterado e presente no manifesto" é "veio do pack".
//!
//! O manifesto só é regravado quando muda. Um arquivo ilegível vira manifesto vazio (com aviso
//! no registro): o pior caso é a próxima materialização tratar os arquivos existentes como
//! desconhecidos e pedir revisão antes de sobrescrevê-los.

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::Path;
use std::time::UNIX_EPOCH;

use serde::{Deserialize, Serialize};
use warden_core::atomic_write;

use crate::error::{Error, Result};

/// Nome do arquivo na pasta `state/`.
pub const MANIFEST_FILE: &str = "manifest.json";

/// Versão do formato.
pub const MANIFEST_SCHEMA: u32 = 1;

/// De onde veio um arquivo da instância.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum EntryOrigin {
    /// Arquivo do próprio pack (config, resource pack embutido…).
    Pack,
    /// Download por link (`.pw.toml` com `url`); do Modrinth quando tem os IDs.
    Url {
        /// Endereço.
        url: String,
        /// IDs do Modrinth (`[update.modrinth]`), quando houver.
        modrinth: Option<ModrinthIds>,
    },
    /// CurseForge (`metadata:curseforge`): só os IDs, nunca o endereço.
    Curseforge {
        /// ID do projeto.
        project: u64,
        /// ID do arquivo.
        file: u64,
    },
}

/// IDs do Modrinth de um item.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ModrinthIds {
    /// ID do projeto.
    pub project: String,
    /// ID da versão.
    pub version: String,
}

/// Um arquivo que o Warden colocou na instância.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManifestEntry {
    /// SHA-256 do conteúdo gravado.
    pub sha256: String,
    /// Tamanho em bytes.
    pub size: u64,
    /// Data de modificação do arquivo logo depois de gravado (nanossegundos desde 1970).
    pub modified_ns: u64,
    /// Caminho do item no índice do pack (`mods/sodium.pw.toml`).
    pub index_path: String,
    /// Hash do item no índice, para saber se o item mudou.
    pub index_hash: String,
    /// Hash esperado do conteúdo (`formato:valor`, o do metafile ou o do arquivo do pack).
    pub expected: String,
    /// Origem.
    pub origin: EntryOrigin,
}

/// O manifesto inteiro.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    /// Versão do formato.
    pub schema_version: u32,
    /// Lado instalado (`client` ou `server`).
    pub side: String,
    /// Arquivos, pelo caminho relativo à pasta do jogo (com `/`).
    pub files: BTreeMap<String, ManifestEntry>,
}

impl Default for Manifest {
    fn default() -> Self {
        Self {
            schema_version: MANIFEST_SCHEMA,
            side: "client".to_owned(),
            files: BTreeMap::new(),
        }
    }
}

impl Manifest {
    /// Lê o manifesto de `state_dir`. Sem arquivo: vazio. Arquivo ilegível ou de versão mais
    /// nova: vazio, com aviso.
    pub fn load(state_dir: &Path) -> Result<Self> {
        let path = state_dir.join(MANIFEST_FILE);
        let bytes = match fs::read(&path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(error) => return Err(Error::io("ler", path, error)),
        };
        match serde_json::from_slice::<Self>(&bytes) {
            Ok(manifest) if manifest.schema_version <= MANIFEST_SCHEMA => Ok(manifest),
            Ok(manifest) => {
                tracing::warn!(
                    versao = manifest.schema_version,
                    "manifesto da instância de versão mais nova; ignorado"
                );
                Ok(Self::default())
            }
            Err(error) => {
                tracing::warn!(%error, caminho = %path.display(), "manifesto da instância ilegível; ignorado");
                Ok(Self::default())
            }
        }
    }

    /// Texto JSON gravado (estável: chaves em ordem).
    pub fn to_json(&self) -> Result<Vec<u8>> {
        let mut bytes = serde_json::to_vec_pretty(self)
            .map_err(|error| Error::Internal(format!("manifesto em JSON: {error}")))?;
        bytes.push(b'\n');
        Ok(bytes)
    }

    /// Grava em `state_dir` se o conteúdo for diferente do que está no disco. Devolve se
    /// gravou.
    pub fn save_if_changed(&self, state_dir: &Path) -> Result<bool> {
        let path = state_dir.join(MANIFEST_FILE);
        let bytes = self.to_json()?;
        match fs::read(&path) {
            Ok(current) if current == bytes => return Ok(false),
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(Error::io("ler", path, error)),
        }
        fs::create_dir_all(state_dir).map_err(|e| Error::io("criar a pasta", state_dir, e))?;
        atomic_write(&path, &bytes)?;
        Ok(true)
    }
}

/// Data de modificação em nanossegundos desde 1970 (0 se o sistema não informar).
#[must_use]
pub fn modified_ns(metadata: &fs::Metadata) -> u64 {
    metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .and_then(|elapsed| u64::try_from(elapsed.as_nanos()).ok())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry() -> ManifestEntry {
        ManifestEntry {
            sha256: "ab".repeat(32),
            size: 3,
            modified_ns: 1,
            index_path: "mods/a.pw.toml".to_owned(),
            index_hash: "cd".repeat(32),
            expected: "sha1:00".to_owned(),
            origin: EntryOrigin::Curseforge {
                project: 1,
                file: 2,
            },
        }
    }

    #[test]
    fn grava_so_quando_muda() {
        let dir = crate::test_support::temp_dir();
        let mut manifest = Manifest::default();
        manifest.files.insert("mods/a.jar".to_owned(), entry());
        assert!(manifest.save_if_changed(dir.path()).unwrap());
        assert!(!manifest.save_if_changed(dir.path()).unwrap());
        assert_eq!(Manifest::load(dir.path()).unwrap(), manifest);
        manifest.files.clear();
        assert!(manifest.save_if_changed(dir.path()).unwrap());
    }

    #[test]
    fn ilegivel_ou_mais_novo_vira_vazio() {
        let dir = crate::test_support::temp_dir();
        assert_eq!(Manifest::load(dir.path()).unwrap(), Manifest::default());
        fs::write(dir.path().join(MANIFEST_FILE), b"{ quebrado").unwrap();
        assert_eq!(Manifest::load(dir.path()).unwrap(), Manifest::default());
        fs::write(
            dir.path().join(MANIFEST_FILE),
            br#"{"schemaVersion": 99, "side": "client", "files": {}}"#,
        )
        .unwrap();
        assert_eq!(Manifest::load(dir.path()).unwrap(), Manifest::default());
    }

    #[test]
    fn json_em_camel_case_sem_endereco_da_curseforge() {
        let mut manifest = Manifest::default();
        manifest.files.insert("mods/a.jar".to_owned(), entry());
        let text = String::from_utf8(manifest.to_json().unwrap()).unwrap();
        assert!(text.contains("\"schemaVersion\": 1"));
        assert!(text.contains("\"indexHash\""));
        assert!(text.contains("\"kind\": \"curseforge\""));
        assert!(!text.contains("url"));
    }
}
