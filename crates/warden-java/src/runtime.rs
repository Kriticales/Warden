//! Os Javas instalados pelo Warden e a plataforma em que rodam.
//!
//! Cada Java fica em `shared/runtimes/<id>/` (ARCHITECTURE §13), com o conteúdo do pacote
//! (`bin/`, `lib/`…) e o arquivo [`METADATA_FILE`], gravado por último, antes da renomeação que
//! torna a instalação visível ([`crate::store`]).

use std::fmt;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::version::JavaVersion;

/// Arquivo com os dados do Java instalado, na raiz da pasta dele.
pub const METADATA_FILE: &str = "warden-runtime.json";

/// Versão do formato de [`METADATA_FILE`].
pub(crate) const METADATA_SCHEMA: u32 = 1;

/// De onde veio o Java.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum RuntimeSource {
    /// Eclipse Temurin, pelo Adoptium (fonte padrão).
    Temurin,
    /// Runtime oficial da Mojang (alternativa).
    Mojang,
}

impl RuntimeSource {
    /// Prefixo do id e da pasta.
    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Self::Temurin => "temurin",
            Self::Mojang => "mojang",
        }
    }

    fn from_key(key: &str) -> Option<Self> {
        match key {
            "temurin" => Some(Self::Temurin),
            "mojang" => Some(Self::Mojang),
            _ => None,
        }
    }
}

/// Sistema operacional suportado.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Os {
    /// Windows.
    Windows,
    /// Linux.
    Linux,
}

/// Processador suportado (só 64 bits; ADR-0012).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Arch {
    /// x86-64.
    X64,
    /// ARM de 64 bits.
    Aarch64,
}

/// Sistema e processador para os quais o Java é baixado.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Platform {
    /// Sistema.
    pub os: Os,
    /// Processador.
    pub arch: Arch,
}

impl Platform {
    /// A plataforma em que o Warden está rodando.
    pub fn current() -> Result<Self> {
        Self::from_names(std::env::consts::OS, std::env::consts::ARCH)
    }

    /// Plataforma a partir dos nomes do Rust (`windows`/`linux`, `x86_64`/`aarch64`).
    pub fn from_names(os: &str, arch: &str) -> Result<Self> {
        let unsupported = || Error::UnsupportedPlatform {
            os: os.to_owned(),
            arch: arch.to_owned(),
        };
        let os = match os {
            "windows" => Os::Windows,
            "linux" => Os::Linux,
            _ => return Err(unsupported()),
        };
        let arch = match arch {
            "x86_64" => Arch::X64,
            "aarch64" => Arch::Aarch64,
            _ => return Err(unsupported()),
        };
        Ok(Self { os, arch })
    }

    /// `os` da API do Adoptium.
    #[must_use]
    pub const fn adoptium_os(self) -> &'static str {
        match self.os {
            Os::Windows => "windows",
            Os::Linux => "linux",
        }
    }

    /// `architecture` da API do Adoptium (e sufixo do id).
    #[must_use]
    pub const fn arch_key(self) -> &'static str {
        match self.arch {
            Arch::X64 => "x64",
            Arch::Aarch64 => "aarch64",
        }
    }

    /// Chave da plataforma no índice `java-runtime` da Mojang. `None` onde a Mojang não
    /// publica runtime (Linux ARM).
    #[must_use]
    pub const fn mojang_key(self) -> Option<&'static str> {
        match (self.os, self.arch) {
            (Os::Windows, Arch::X64) => Some("windows-x64"),
            (Os::Windows, Arch::Aarch64) => Some("windows-arm64"),
            (Os::Linux, Arch::X64) => Some("linux"),
            (Os::Linux, Arch::Aarch64) => None,
        }
    }

    /// `windows-x64`, para mensagens.
    #[must_use]
    pub fn label(self) -> String {
        format!("{}-{}", self.adoptium_os(), self.arch_key())
    }

    /// Caminho do programa `java` dentro da pasta do Java (com console: usado na validação).
    #[must_use]
    pub fn java_in(self, home: &Path) -> PathBuf {
        match self.os {
            Os::Windows => home.join("bin").join("java.exe"),
            Os::Linux => home.join("bin").join("java"),
        }
    }

    /// Caminho do programa que abre o jogo: `javaw.exe` no Windows (sem janela de console),
    /// `java` no Linux (ARCHITECTURE §7.4).
    #[must_use]
    pub fn javaw_in(self, home: &Path) -> PathBuf {
        match self.os {
            Os::Windows => home.join("bin").join("javaw.exe"),
            Os::Linux => home.join("bin").join("java"),
        }
    }
}

/// Identificador de um Java instalado: `<fonte>-<major>-<versão>-<processador>`, que também é
/// o nome da pasta (`temurin-21-21.0.12.1+1-x64`, `temurin-8-8u312-b07-x64`).
///
/// Ao ler (inclusive pelo IPC), o texto passa por [`RuntimeId::parse`]: um id nunca vira um
/// caminho fora de `shared/runtimes/`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, specta::Type)]
#[specta(transparent)]
pub struct RuntimeId(#[specta(type = String)] String);

impl Serialize for RuntimeId {
    fn serialize<S: serde::Serializer>(
        &self,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for RuntimeId {
    fn deserialize<D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> std::result::Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        Self::parse(&text).map_err(serde::de::Error::custom)
    }
}

impl RuntimeId {
    /// Monta o id. `release` é o texto da versão na fonte, sem caracteres de caminho.
    #[must_use]
    pub fn new(
        source: RuntimeSource,
        version: &JavaVersion,
        release: &str,
        platform: Platform,
    ) -> Self {
        let release: String = release
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || matches!(c, '.' | '+' | '_') {
                    c
                } else {
                    '-'
                }
            })
            .collect();
        let release = release.trim_matches('-');
        Self(format!(
            "{}-{}-{}-{}",
            source.key(),
            version.major,
            release,
            platform.arch_key()
        ))
    }

    /// Lê um id vindo da interface ou de um arquivo. Só aceita o formato que o Warden gera:
    /// letras, números, `.`, `+`, `_` e `-`, sem começar por ponto (as pastas temporárias e a
    /// lixeira começam por ponto).
    pub fn parse(text: &str) -> Result<Self> {
        let valid = !text.is_empty()
            && text.len() <= 128
            && !text.starts_with('.')
            && text
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '+' | '_' | '-'))
            && Self(text.to_owned()).source_and_major().is_some();
        if valid {
            Ok(Self(text.to_owned()))
        } else {
            Err(Error::RuntimeNotFound {
                id: text.chars().take(128).collect(),
            })
        }
    }

    /// O texto do id.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Fonte e major tirados do id (para achar o substituto de um Java removido).
    #[must_use]
    pub fn source_and_major(&self) -> Option<(RuntimeSource, u32)> {
        let mut parts = self.0.splitn(3, '-');
        let source = RuntimeSource::from_key(parts.next()?)?;
        let major = parts.next()?.parse().ok()?;
        parts.next()?;
        Some((source, major))
    }
}

impl fmt::Display for RuntimeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Um Java instalado pelo Warden.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct InstalledRuntime {
    /// Identificador (nome da pasta).
    pub id: RuntimeId,
    /// Fonte.
    pub source: RuntimeSource,
    /// Versão, conferida executando o Java.
    pub version: JavaVersion,
    /// `java.vendor` informado pelo próprio Java ("Eclipse Adoptium", "Oracle Corporation"…).
    pub vendor: String,
    /// Nome da versão na fonte (`jdk-21.0.12.1+1`, `8u51-cacert462b08`).
    pub release_name: String,
    /// Processador (`x64`).
    pub arch: String,
    /// Pasta do Java.
    pub home: PathBuf,
    /// Programa que abre o jogo (`javaw.exe` no Windows, `java` no Linux).
    pub launcher: PathBuf,
    /// Programa `java` com console (validação).
    pub java: PathBuf,
    /// Quando foi instalado (milissegundos desde 1970).
    #[specta(type = specta_typescript::Number)]
    pub installed_at_ms: u64,
    /// Teto de atualização do canal (o `312` de "8 ≤ u312"); as atualizações respeitam o teto.
    pub update_cap: Option<u32>,
    /// Componente da Mojang (`java-runtime-delta`), quando a fonte é a Mojang.
    pub mojang_component: Option<String>,
    /// SHA-256 do pacote baixado (Temurin).
    pub archive_sha256: Option<String>,
    /// Java mais novo que substituiu este numa atualização; ele é removido quando nenhum jogo
    /// o estiver usando.
    pub superseded_by: Option<RuntimeId>,
}

/// O que fica gravado em [`METADATA_FILE`].
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RuntimeMetadata {
    pub(crate) schema: u32,
    pub(crate) source: RuntimeSource,
    pub(crate) version: JavaVersion,
    pub(crate) vendor: String,
    pub(crate) release_name: String,
    pub(crate) arch: String,
    pub(crate) installed_at_ms: u64,
    #[serde(default)]
    pub(crate) update_cap: Option<u32>,
    #[serde(default)]
    pub(crate) mojang_component: Option<String>,
    #[serde(default)]
    pub(crate) archive_sha256: Option<String>,
    #[serde(default)]
    pub(crate) superseded_by: Option<RuntimeId>,
}

impl RuntimeMetadata {
    pub(crate) fn into_runtime(
        self,
        id: RuntimeId,
        home: PathBuf,
        platform: Platform,
    ) -> InstalledRuntime {
        InstalledRuntime {
            java: platform.java_in(&home),
            launcher: platform.javaw_in(&home),
            id,
            source: self.source,
            version: self.version,
            vendor: self.vendor,
            release_name: self.release_name,
            arch: self.arch,
            home,
            installed_at_ms: self.installed_at_ms,
            update_cap: self.update_cap,
            mojang_component: self.mojang_component,
            archive_sha256: self.archive_sha256,
            superseded_by: self.superseded_by,
        }
    }

    pub(crate) fn from_runtime(runtime: &InstalledRuntime) -> Self {
        Self {
            schema: METADATA_SCHEMA,
            source: runtime.source,
            version: runtime.version,
            vendor: runtime.vendor.clone(),
            release_name: runtime.release_name.clone(),
            arch: runtime.arch.clone(),
            installed_at_ms: runtime.installed_at_ms,
            update_cap: runtime.update_cap,
            mojang_component: runtime.mojang_component.clone(),
            archive_sha256: runtime.archive_sha256.clone(),
            superseded_by: runtime.superseded_by.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const WIN: Platform = Platform {
        os: Os::Windows,
        arch: Arch::X64,
    };

    #[test]
    fn ids_no_formato_da_pasta() {
        let version = JavaVersion::parse("21.0.12.1+1").unwrap();
        let id = RuntimeId::new(RuntimeSource::Temurin, &version, "jdk-21.0.12.1+1", WIN);
        assert_eq!(id.as_str(), "temurin-21-jdk-21.0.12.1+1-x64");
        assert_eq!(id.source_and_major(), Some((RuntimeSource::Temurin, 21)));
        let java8 = JavaVersion::parse("8u312-b07").unwrap();
        let id = RuntimeId::new(RuntimeSource::Temurin, &java8, "jdk8u312-b07", WIN);
        assert_eq!(id.as_str(), "temurin-8-jdk8u312-b07-x64");
        let mojang = RuntimeId::new(
            RuntimeSource::Mojang,
            &JavaVersion::parse("8u51").unwrap(),
            "8u51-cacert462b08",
            WIN,
        );
        assert_eq!(mojang.as_str(), "mojang-8-8u51-cacert462b08-x64");
        assert_eq!(RuntimeId::parse(mojang.as_str()).unwrap(), mojang);
        let json = serde_json::to_string(&mojang).unwrap();
        assert_eq!(serde_json::from_str::<RuntimeId>(&json).unwrap(), mojang);
        assert!(serde_json::from_str::<RuntimeId>("\"..\"").is_err());
        // Caracteres de caminho viram hífen.
        let odd = RuntimeId::new(RuntimeSource::Mojang, &java8, "../x\\y z", WIN);
        assert_eq!(odd.as_str(), "mojang-8-..-x-y-z-x64");
    }

    #[test]
    fn ids_invalidos() {
        for text in [
            "",
            ".tmp-01",
            "temurin",
            "temurin-x-1-x64",
            "zulu-21-1-x64",
            "temurin-21-a/b-x64",
            "temurin-21-..\\x-x64",
            "temurin-21",
        ] {
            assert!(RuntimeId::parse(text).is_err(), "{text:?}");
        }
    }

    #[test]
    fn plataformas() {
        assert_eq!(Platform::from_names("windows", "x86_64").unwrap(), WIN);
        let linux = Platform::from_names("linux", "aarch64").unwrap();
        assert_eq!(linux.mojang_key(), None);
        assert_eq!(linux.label(), "linux-aarch64");
        assert!(Platform::from_names("windows", "x86").is_err());
        assert!(Platform::from_names("macos", "aarch64").is_err());
        assert!(Platform::current().is_ok());
        let home = Path::new("r");
        assert_eq!(
            WIN.java_in(home),
            Path::new("r").join("bin").join("java.exe")
        );
        assert_eq!(
            WIN.javaw_in(home),
            Path::new("r").join("bin").join("javaw.exe")
        );
        assert_eq!(WIN.mojang_key(), Some("windows-x64"));
    }
}
