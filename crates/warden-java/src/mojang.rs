//! Runtime oficial da Mojang, a alternativa ao Temurin (R2 §2.2; ADR-0012).
//!
//! - Índice: `java-runtime/<hash>/all.json`, com os componentes por plataforma
//!   (`jre-legacy`, `java-runtime-delta`…), cada um com o manifesto e a versão.
//! - Manifesto do componente: cada arquivo (`type: file`, com `downloads.raw` e SHA-1), cada
//!   pasta (`type: directory`) e cada link (`type: link`). Não há pacote único: os arquivos são
//!   baixados um a um (8 por vez), conferidos pelo SHA-1 e pelo tamanho, e gravados pela
//!   [`warden_http::HttpClient::download`]. A versão `lzma` é ignorada (o `raw` basta e evita
//!   mais uma dependência).
//! - Caminhos passam por [`warden_core::resolve_inside`]; links só no Linux e só para dentro.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;

use serde::Deserialize;
use tokio::sync::Semaphore;
use tokio::task::JoinSet;
use warden_core::{CancellationToken, Progress, ProgressSink};
use warden_http::{DownloadRequest, HttpClient, Url};
use warden_packwiz::HashFormat;

use crate::error::{Error, JavaSource, Result};
use crate::runtime::Platform;
use crate::version::JavaVersion;

/// Endereço do índice `java-runtime` (o mesmo do launcher oficial).
pub const DEFAULT_INDEX_URL: &str = "https://piston-meta.mojang.com/v1/products/java-runtime/2ec0cc96c44e5a76b9c8b7c39df7210883d12871/all.json";

/// Variável que troca o endereço do índice, só em build de debug.
pub const INDEX_URL_ENV: &str = "WARDEN_API_BASE_MOJANG_JAVA";

/// Downloads simultâneos de arquivos do runtime.
const PARALLEL_DOWNLOADS: usize = 8;

/// Maior JSON aceito (o manifesto de um runtime tem ~150 KB).
const MAX_JSON: u64 = 8 * 1024 * 1024;

/// Maior arquivo aceito num runtime (o maior, `modules`/`rt.jar`, tem ~70 MB).
const MAX_FILE: u64 = 512 * 1024 * 1024;

/// Um componente da Mojang para a plataforma.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MojangComponent {
    /// Nome (`java-runtime-delta`).
    pub name: String,
    /// Nome da versão (`21.0.7`, `8u51-cacert462b08`).
    pub version_name: String,
    /// Versão.
    pub version: JavaVersion,
    /// Endereço do manifesto.
    pub manifest_url: Url,
    /// SHA-1 do manifesto.
    pub manifest_sha1: String,
}

/// Um arquivo do manifesto.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ManifestEntry {
    /// Arquivo para baixar.
    File {
        /// Endereço.
        url: Url,
        /// SHA-1.
        sha1: String,
        /// Tamanho.
        size: u64,
        /// Se é executável.
        executable: bool,
    },
    /// Pasta.
    Directory,
    /// Link simbólico.
    Link {
        /// Alvo, relativo à pasta do link.
        target: String,
    },
}

#[derive(Deserialize)]
struct RawManifestRef {
    sha1: String,
    url: String,
}

#[derive(Deserialize)]
struct RawVersionInfo {
    name: String,
}

#[derive(Deserialize)]
struct RawComponent {
    manifest: RawManifestRef,
    version: RawVersionInfo,
}

#[derive(Deserialize)]
struct RawDownload {
    sha1: String,
    size: u64,
    url: String,
}

#[derive(Deserialize)]
struct RawDownloads {
    raw: RawDownload,
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
enum RawEntry {
    File {
        downloads: RawDownloads,
        #[serde(default)]
        executable: bool,
    },
    Directory,
    Link {
        target: String,
    },
}

#[derive(Deserialize)]
struct RawManifest {
    files: BTreeMap<String, RawEntry>,
}

fn invalid(message: impl Into<String>) -> Error {
    Error::InvalidResponse {
        source_name: JavaSource::Mojang,
        message: message.into(),
    }
}

fn is_sha1(text: &str) -> bool {
    text.len() == 40 && text.bytes().all(|b| b.is_ascii_hexdigit())
}

/// Cliente do runtime da Mojang. Barato de clonar.
#[derive(Debug, Clone)]
pub struct MojangRuntimeClient {
    http: HttpClient,
    index: Url,
}

impl MojangRuntimeClient {
    /// Cliente com o índice oficial (ou o de `WARDEN_API_BASE_MOJANG_JAVA`, em build de debug).
    pub fn new(http: HttpClient) -> Result<Self> {
        let index = index_url_from_env().unwrap_or_else(|| DEFAULT_INDEX_URL.to_owned());
        Self::with_index_url(http, &index)
    }

    /// Cliente com outro índice (testes).
    pub fn with_index_url(http: HttpClient, index: &str) -> Result<Self> {
        let index = warden_http::parse_url(index).map_err(Error::http(JavaSource::Mojang))?;
        Ok(Self { http, index })
    }

    async fn get_json<T: serde::de::DeserializeOwned>(
        &self,
        url: Url,
        cancel: Option<&CancellationToken>,
    ) -> Result<T> {
        let mut request = self.http.get(url).max_body(MAX_JSON);
        if let Some(token) = cancel {
            request = request.cancel(token);
        }
        request
            .json()
            .await
            .map_err(Error::http(JavaSource::Mojang))
    }

    /// Componentes publicados para a plataforma (vazio onde a Mojang não publica).
    pub async fn components(
        &self,
        platform: Platform,
        cancel: Option<&CancellationToken>,
    ) -> Result<Vec<MojangComponent>> {
        let Some(key) = platform.mojang_key() else {
            return Ok(Vec::new());
        };
        let index: BTreeMap<String, BTreeMap<String, Vec<RawComponent>>> =
            self.get_json(self.index.clone(), cancel).await?;
        let Some(components) = index.get(key) else {
            return Ok(Vec::new());
        };
        let mut result = Vec::new();
        for (name, entries) in components {
            // `minecraft-java-exe` não é um Java.
            if !(name.starts_with("java-runtime") || name.starts_with("jre-")) {
                continue;
            }
            let Some(entry) = entries.first() else {
                continue;
            };
            let Some(version) = JavaVersion::parse(&entry.version.name) else {
                continue;
            };
            if !is_sha1(&entry.manifest.sha1) {
                return Err(invalid(format!("{name}: SHA-1 do manifesto inválido")));
            }
            let manifest_url = warden_http::parse_url(&entry.manifest.url)
                .map_err(|error| invalid(format!("{name}: endereço inválido: {error}")))?;
            result.push(MojangComponent {
                name: name.clone(),
                version_name: entry.version.name.clone(),
                version,
                manifest_url,
                manifest_sha1: entry.manifest.sha1.to_ascii_lowercase(),
            });
        }
        Ok(result)
    }

    /// O componente mais novo de um major, com o teto de atualização se houver. Em empate
    /// (beta e gamma apontam para o mesmo 17), o primeiro em ordem alfabética.
    #[must_use]
    pub fn pick(
        components: &[MojangComponent],
        major: u32,
        max_update: Option<u32>,
    ) -> Option<&MojangComponent> {
        components
            .iter()
            .filter(|component| component.version.major == major)
            .filter(|component| max_update.is_none_or(|cap| component.version.security <= cap))
            .filter(|component| !component.name.ends_with("-snapshot"))
            .fold(
                None,
                |best: Option<&MojangComponent>, candidate| match best {
                    Some(current) if current.version >= candidate.version => Some(current),
                    _ => Some(candidate),
                },
            )
    }

    /// Lê o manifesto de um componente, conferindo o SHA-1 do JSON.
    pub async fn manifest(
        &self,
        component: &MojangComponent,
        cancel: Option<&CancellationToken>,
    ) -> Result<BTreeMap<String, ManifestEntry>> {
        let mut request = self
            .http
            .get(component.manifest_url.clone())
            .max_body(MAX_JSON);
        if let Some(token) = cancel {
            request = request.cancel(token);
        }
        let bytes = request
            .bytes()
            .await
            .map_err(Error::http(JavaSource::Mojang))?;
        let actual = warden_packwiz::hash::hash_bytes(HashFormat::Sha1, &bytes);
        if actual != component.manifest_sha1 {
            return Err(Error::Http {
                source_name: JavaSource::Mojang,
                error: warden_http::Error::HashMismatch {
                    host: component
                        .manifest_url
                        .host_str()
                        .unwrap_or_default()
                        .to_owned(),
                    url: component.manifest_url.to_string(),
                    algorithm: "sha1".to_owned(),
                    expected: component.manifest_sha1.clone(),
                    actual,
                },
            });
        }
        parse_manifest(&bytes)
    }

    /// Baixa todos os arquivos do manifesto em `destination` (pasta vazia), conferindo cada
    /// SHA-1. Progresso em bytes. Erro no primeiro arquivo que falhar (os outros são
    /// cancelados); quem chama apaga a pasta.
    pub async fn download_all(
        &self,
        entries: &BTreeMap<String, ManifestEntry>,
        destination: &Path,
        progress: &dyn ProgressSink,
        cancel: &CancellationToken,
    ) -> Result<()> {
        let total: u64 = entries
            .values()
            .map(|entry| match entry {
                ManifestEntry::File { size, .. } => *size,
                _ => 0,
            })
            .sum();
        // Pastas primeiro.
        for (name, entry) in entries {
            if matches!(entry, ManifestEntry::Directory) {
                let path = warden_core::resolve_inside(destination, name)?;
                std::fs::create_dir_all(&path).map_err(|e| Error::io("criar a pasta", &path, e))?;
            }
        }
        let semaphore = Arc::new(Semaphore::new(PARALLEL_DOWNLOADS));
        let mut tasks = JoinSet::new();
        for (name, entry) in entries {
            let ManifestEntry::File {
                url,
                sha1,
                size,
                executable,
            } = entry
            else {
                continue;
            };
            if *size > MAX_FILE {
                return Err(invalid(format!(
                    "{name}: arquivo grande demais ({size} bytes)"
                )));
            }
            let path = warden_core::resolve_inside(destination, name)?;
            let request = DownloadRequest::new(url.clone(), &path)
                .expect_hash(HashFormat::Sha1, sha1.clone())
                .expect_size(*size)
                .max_size(*size);
            let http = self.http.clone();
            let semaphore = Arc::clone(&semaphore);
            let token = cancel.clone();
            let executable = *executable;
            tasks.spawn(async move {
                let _permit = semaphore
                    .acquire_owned()
                    .await
                    .map_err(|_| Error::Internal("semáforo fechado".into()))?;
                let downloaded = http
                    .download(&request, &warden_core::NoProgress, Some(&token))
                    .await
                    .map_err(Error::http(JavaSource::Mojang))?;
                if executable {
                    mark_executable(&downloaded.path)?;
                }
                Ok::<u64, Error>(downloaded.size)
            });
        }
        let mut done: u64 = 0;
        progress.progress(Progress::bytes(0, Some(total)));
        while let Some(joined) = tasks.join_next().await {
            let result =
                joined.map_err(|error| Error::Internal(format!("download interrompido: {error}")));
            match result.and_then(|inner| inner) {
                Ok(size) => {
                    done += size;
                    progress.progress(Progress::bytes(done, Some(total)));
                }
                Err(error) => {
                    tasks.abort_all();
                    return Err(error);
                }
            }
        }
        for (name, entry) in entries {
            if let ManifestEntry::Link { target } = entry {
                create_link(destination, name, target)?;
            }
        }
        Ok(())
    }
}

/// Lê o manifesto de um componente.
pub fn parse_manifest(bytes: &[u8]) -> Result<BTreeMap<String, ManifestEntry>> {
    let raw: RawManifest = serde_json::from_slice(bytes)
        .map_err(|error| invalid(format!("manifesto ilegível: {error}")))?;
    let mut entries = BTreeMap::new();
    for (name, entry) in raw.files {
        if name.is_empty() || name.starts_with('/') || name.contains('\\') {
            return Err(invalid(format!("caminho inválido no manifesto: {name:?}")));
        }
        let entry = match entry {
            RawEntry::Directory => ManifestEntry::Directory,
            RawEntry::Link { target } => ManifestEntry::Link { target },
            RawEntry::File {
                downloads,
                executable,
            } => {
                if !is_sha1(&downloads.raw.sha1) {
                    return Err(invalid(format!("{name}: SHA-1 inválido")));
                }
                let url = warden_http::parse_url(&downloads.raw.url)
                    .map_err(|error| invalid(format!("{name}: endereço inválido: {error}")))?;
                ManifestEntry::File {
                    url,
                    sha1: downloads.raw.sha1.to_ascii_lowercase(),
                    size: downloads.raw.size,
                    executable,
                }
            }
        };
        entries.insert(name, entry);
    }
    Ok(entries)
}

#[cfg(unix)]
fn mark_executable(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt as _;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))
        .map_err(|e| Error::io("ajustar a permissão de", path, e))
}

#[cfg(not(unix))]
#[allow(clippy::unnecessary_wraps)] // mesma assinatura da versão do Linux
fn mark_executable(_path: &Path) -> Result<()> {
    Ok(())
}

/// Cria o link `name` → `target` se o alvo, resolvido sem seguir nada, ficar dentro de
/// `destination`. No Windows os links são ignorados (os manifestos do Windows não têm).
fn create_link(destination: &Path, name: &str, target: &str) -> Result<()> {
    let mut resolved: Vec<&str> = name.split('/').collect();
    resolved.pop();
    if target.starts_with('/') || target.contains('\\') {
        return Err(invalid(format!(
            "link {name:?} com alvo absoluto {target:?}"
        )));
    }
    for part in target.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                if resolved.pop().is_none() {
                    return Err(invalid(format!("link {name:?} sai da pasta: {target:?}")));
                }
            }
            other => resolved.push(other),
        }
    }
    let path = warden_core::resolve_inside(destination, name)?;
    link(&path, target)
}

#[cfg(unix)]
fn link(path: &Path, target: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| Error::io("criar a pasta", parent, e))?;
    }
    std::os::unix::fs::symlink(target, path).map_err(|e| Error::io("criar o link", path, e))
}

#[cfg(not(unix))]
#[allow(clippy::unnecessary_wraps)] // mesma assinatura da versão do Linux
fn link(path: &Path, _target: &str) -> Result<()> {
    tracing::debug!(path = %path.display(), "link do runtime da Mojang ignorado no Windows");
    Ok(())
}

fn index_url_from_env() -> Option<String> {
    #[cfg(debug_assertions)]
    {
        std::env::var(INDEX_URL_ENV)
            .ok()
            .filter(|value| !value.trim().is_empty())
    }
    #[cfg(not(debug_assertions))]
    {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn component(name: &str, version: &str) -> MojangComponent {
        MojangComponent {
            name: name.into(),
            version_name: version.into(),
            version: JavaVersion::parse(version).unwrap(),
            manifest_url: warden_http::parse_url("https://example.invalid/m.json").unwrap(),
            manifest_sha1: "0".repeat(40),
        }
    }

    #[test]
    fn escolhe_o_componente_do_major() {
        let components = vec![
            component("java-runtime-beta", "17.0.15"),
            component("java-runtime-gamma", "17.0.15"),
            component("java-runtime-gamma-snapshot", "17.0.16"),
            component("java-runtime-delta", "21.0.7"),
            component("jre-legacy", "8u51-cacert462b08"),
        ];
        assert_eq!(
            MojangRuntimeClient::pick(&components, 17, None)
                .unwrap()
                .name,
            "java-runtime-beta"
        );
        assert_eq!(
            MojangRuntimeClient::pick(&components, 21, None)
                .unwrap()
                .name,
            "java-runtime-delta"
        );
        assert_eq!(
            MojangRuntimeClient::pick(&components, 8, Some(312))
                .unwrap()
                .name,
            "jre-legacy"
        );
        assert!(MojangRuntimeClient::pick(&components, 8, Some(50)).is_none());
        assert!(MojangRuntimeClient::pick(&components, 25, None).is_none());
    }

    #[test]
    fn manifesto_recusa_caminhos_e_hashes_invalidos() {
        let bad_path = br#"{"files":{"/etc/x":{"type":"directory"}}}"#;
        assert!(parse_manifest(bad_path).is_err());
        let bad_sha = br#"{"files":{"bin/java":{"type":"file","downloads":{"raw":{"sha1":"x","size":1,"url":"https://a/b"}}}}}"#;
        assert!(parse_manifest(bad_sha).is_err());
        let ok = br#"{"files":{"bin":{"type":"directory"},"bin/java":{"type":"file","executable":true,"downloads":{"raw":{"sha1":"0123456789abcdef0123456789abcdef01234567","size":3,"url":"https://a/b"}}},"bin/x":{"type":"link","target":"java"}}}"#;
        let entries = parse_manifest(ok).unwrap();
        assert_eq!(entries.len(), 3);
        assert!(matches!(
            entries["bin/java"],
            ManifestEntry::File {
                executable: true,
                size: 3,
                ..
            }
        ));
    }

    #[test]
    fn links_que_saem_da_pasta_sao_recusados() {
        let dir = tempfile::tempdir().unwrap();
        assert!(create_link(dir.path(), "bin/x", "../../fora").is_err());
        assert!(create_link(dir.path(), "bin/x", "/etc/passwd").is_err());
        assert!(create_link(dir.path(), "bin/x", "../lib/ok").is_ok());
    }
}
