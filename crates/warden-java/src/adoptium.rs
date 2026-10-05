//! Cliente da API v3 do Adoptium (Temurin JRE; R2 §2.3; ARCHITECTURE §7.3).
//!
//! - [`AdoptiumClient::latest`]: `GET assets/latest/{major}/hotspot`, a atualização mais nova
//!   de um major.
//! - [`AdoptiumClient::latest_up_to`]: `GET assets/version/[{major}.0.0,{major}.0.{teto+1})`,
//!   a mais nova até um teto (o "8 ≤ u312" do Forge 1.16.5 antigo). O limite superior é
//!   exclusivo porque a API compara pela versão semântica com o build (`8.0.312+7` é maior que
//!   `8.0.312`); a resposta também é filtrada aqui pelo número da atualização.
//! - [`AdoptiumClient::available_releases`]: `GET info/available_releases` (teste de rede da
//!   tabela de compatibilidade).
//!
//! Sempre o JRE (`image_type=jre`), `HotSpot`, `vendor=eclipse`, versões GA, heap normal, no
//! formato `.zip` (Windows) ou `.tar.gz` (Linux), com o SHA-256 que a API informa.

use serde::Deserialize;
use warden_core::CancellationToken;
use warden_http::{HttpClient, Url};

use crate::archive::ArchiveKind;
use crate::error::{Error, JavaSource, Result};
use crate::runtime::Platform;
use crate::version::JavaVersion;

/// Endereço da API.
pub const DEFAULT_BASE_URL: &str = "https://api.adoptium.net/v3/";

/// Variável que troca o endereço da API, só em build de debug (servidor de fixtures dos E2E).
pub const BASE_URL_ENV: &str = "WARDEN_API_BASE_ADOPTIUM";

/// Maior resposta JSON aceita.
const MAX_JSON: u64 = 4 * 1024 * 1024;

/// Um JRE publicado pelo Adoptium.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdoptiumBuild {
    /// Nome da versão (`jdk-21.0.12.1+1`, `jdk8u312-b07`).
    pub release_name: String,
    /// Versão.
    pub version: JavaVersion,
    /// Nome do pacote (`OpenJDK21U-jre_x64_windows_hotspot_21.0.12.1_1.zip`).
    pub package_name: String,
    /// Endereço do pacote.
    pub link: Url,
    /// Tamanho do pacote em bytes.
    pub size: u64,
    /// SHA-256 do pacote (hexadecimal minúsculo).
    pub sha256: String,
    /// Formato do pacote.
    pub kind: ArchiveKind,
}

/// Majors publicados.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct AvailableReleases {
    /// Majors com suporte longo (LTS).
    pub available_lts_releases: Vec<u32>,
    /// Todos os majors publicados.
    pub available_releases: Vec<u32>,
}

#[derive(Deserialize)]
struct RawPackage {
    name: String,
    link: String,
    size: u64,
    checksum: Option<String>,
}

#[derive(Deserialize)]
struct RawBinary {
    os: String,
    architecture: String,
    image_type: String,
    jvm_impl: String,
    #[serde(default)]
    heap_size: Option<String>,
    package: Option<RawPackage>,
}

#[derive(Deserialize)]
struct RawVersion {
    major: u32,
    #[serde(default)]
    minor: u32,
    #[serde(default)]
    security: u32,
    #[serde(default)]
    patch: Option<u32>,
    #[serde(default)]
    build: Option<u32>,
}

impl RawVersion {
    fn to_version(&self) -> JavaVersion {
        JavaVersion::new(
            self.major,
            self.minor,
            self.security,
            self.patch.unwrap_or(0),
            self.build.unwrap_or(0),
        )
    }
}

/// Item de `assets/latest`.
#[derive(Deserialize)]
struct RawLatest {
    binary: RawBinary,
    release_name: String,
    version: RawVersion,
}

/// Item de `assets/version`.
#[derive(Deserialize)]
struct RawRelease {
    binaries: Vec<RawBinary>,
    release_name: String,
    version_data: RawVersion,
}

/// Cliente do Adoptium. Barato de clonar.
#[derive(Debug, Clone)]
pub struct AdoptiumClient {
    http: HttpClient,
    base: Url,
}

fn invalid(message: impl Into<String>) -> Error {
    Error::InvalidResponse {
        source_name: JavaSource::Adoptium,
        message: message.into(),
    }
}

impl AdoptiumClient {
    /// Cliente com o endereço oficial (ou o de `WARDEN_API_BASE_ADOPTIUM`, em build de debug).
    pub fn new(http: HttpClient) -> Result<Self> {
        let base = base_url_from_env().unwrap_or_else(|| DEFAULT_BASE_URL.to_owned());
        Self::with_base_url(http, &base)
    }

    /// Cliente com outro endereço da API (testes).
    pub fn with_base_url(http: HttpClient, base: &str) -> Result<Self> {
        let base = warden_http::parse_url(base).map_err(Error::http(JavaSource::Adoptium))?;
        Ok(Self { http, base })
    }

    fn url(&self, path: &str, query: &[(&str, &str)]) -> Result<Url> {
        let mut url = self
            .base
            .join(path)
            .map_err(|error| invalid(format!("endereço inválido {path:?}: {error}")))?;
        if !query.is_empty() {
            url.query_pairs_mut().extend_pairs(query);
        }
        Ok(url)
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
            .map_err(Error::http(JavaSource::Adoptium))
    }

    /// A atualização mais nova do JRE de um major para a plataforma. `None` se o Adoptium não
    /// publica (404 ou lista vazia).
    pub async fn latest(
        &self,
        major: u32,
        platform: Platform,
        cancel: Option<&CancellationToken>,
    ) -> Result<Option<AdoptiumBuild>> {
        let url = self.url(
            &format!("assets/latest/{major}/hotspot"),
            &[
                ("architecture", platform.arch_key()),
                ("image_type", "jre"),
                ("os", platform.adoptium_os()),
                ("vendor", "eclipse"),
            ],
        )?;
        let items: Vec<RawLatest> = match self.get_json(url, cancel).await {
            Ok(items) => items,
            Err(Error::Http {
                error: warden_http::Error::NotFound { .. },
                ..
            }) => return Ok(None),
            Err(error) => return Err(error),
        };
        let mut builds = Vec::new();
        for item in items {
            if let Some(build) =
                build_from(&item.binary, &item.release_name, &item.version, platform)?
                && build.version.major == major
            {
                builds.push(build);
            }
        }
        Ok(builds.into_iter().max_by(|a, b| a.version.cmp(&b.version)))
    }

    /// A atualização mais nova de um major com `security` ≤ `max_update`. `None` se não há.
    pub async fn latest_up_to(
        &self,
        major: u32,
        max_update: u32,
        platform: Platform,
        cancel: Option<&CancellationToken>,
    ) -> Result<Option<AdoptiumBuild>> {
        // O intervalo vai no caminho, já codificado como a API documenta e como foi conferido
        // ao vivo: `[8.0.0,8.0.313)` → `%5B8.0.0%2C8.0.313%29`.
        let above = max_update.saturating_add(1);
        let mut url = self.url(
            &format!("assets/version/%5B{major}.0.0%2C{major}.0.{above}%29"),
            &[],
        )?;
        url.query_pairs_mut().extend_pairs([
            ("architecture", platform.arch_key()),
            ("heap_size", "normal"),
            ("image_type", "jre"),
            ("jvm_impl", "hotspot"),
            ("os", platform.adoptium_os()),
            ("page_size", "20"),
            ("project", "jdk"),
            ("release_type", "ga"),
            ("sort_method", "DEFAULT"),
            ("sort_order", "DESC"),
            ("vendor", "eclipse"),
        ]);
        let releases: Vec<RawRelease> = match self.get_json(url, cancel).await {
            Ok(items) => items,
            Err(Error::Http {
                error: warden_http::Error::NotFound { .. },
                ..
            }) => return Ok(None),
            Err(error) => return Err(error),
        };
        let mut builds = Vec::new();
        for release in releases {
            for binary in &release.binaries {
                if let Some(build) = build_from(
                    binary,
                    &release.release_name,
                    &release.version_data,
                    platform,
                )? && build.version.major == major
                    && build.version.security <= max_update
                {
                    builds.push(build);
                }
            }
        }
        Ok(builds.into_iter().max_by(|a, b| a.version.cmp(&b.version)))
    }

    /// Majors publicados pelo Adoptium.
    pub async fn available_releases(
        &self,
        cancel: Option<&CancellationToken>,
    ) -> Result<AvailableReleases> {
        let url = self.url("info/available_releases", &[])?;
        self.get_json(url, cancel).await
    }
}

/// Converte um binário da API, se for o JRE certo para a plataforma.
fn build_from(
    binary: &RawBinary,
    release_name: &str,
    version: &RawVersion,
    platform: Platform,
) -> Result<Option<AdoptiumBuild>> {
    let matches = binary.os == platform.adoptium_os()
        && binary.architecture == platform.arch_key()
        && binary.image_type == "jre"
        && binary.jvm_impl == "hotspot"
        && binary
            .heap_size
            .as_deref()
            .is_none_or(|heap| heap == "normal");
    if !matches {
        return Ok(None);
    }
    let Some(package) = &binary.package else {
        return Ok(None);
    };
    let Some(kind) = ArchiveKind::from_name(&package.name) else {
        return Ok(None);
    };
    let sha256 = package
        .checksum
        .as_deref()
        .map(str::to_ascii_lowercase)
        .filter(|sum| sum.len() == 64 && sum.bytes().all(|b| b.is_ascii_hexdigit()))
        .ok_or_else(|| invalid(format!("{} sem SHA-256 válido", package.name)))?;
    let link = warden_http::parse_url(&package.link)
        .map_err(|error| invalid(format!("link inválido de {}: {error}", package.name)))?;
    if link.scheme() != "https" && !cfg!(debug_assertions) {
        return Err(invalid(format!("link sem https: {}", package.link)));
    }
    if package.name.contains(['/', '\\']) || package.name.starts_with('.') {
        return Err(invalid(format!(
            "nome de pacote inválido: {:?}",
            package.name
        )));
    }
    Ok(Some(AdoptiumBuild {
        release_name: release_name.to_owned(),
        version: version.to_version(),
        package_name: package.name.clone(),
        link,
        size: package.size,
        sha256,
        kind,
    }))
}

fn base_url_from_env() -> Option<String> {
    #[cfg(debug_assertions)]
    {
        std::env::var(BASE_URL_ENV)
            .ok()
            .filter(|value| !value.trim().is_empty())
    }
    #[cfg(not(debug_assertions))]
    {
        None
    }
}
