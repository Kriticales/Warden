//! Tipos do contrato do launcher (ARCHITECTURE §7.1): o que instalar ([`GameSpec`]), o que o
//! motor devolveu ([`InstalledGame`]), como abrir ([`LaunchOptions`]) e a linha final
//! ([`LaunchCommand`]).

use std::fmt;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::offline::OfflineProfile;

/// Versão do Minecraft como o manifesto da Mojang a escreve (`1.20.1`, `26.3`, `23w14a`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, specta::Type)]
#[serde(transparent)]
pub struct MinecraftVersionId(#[specta(type = String)] String);

impl MinecraftVersionId {
    /// Valida o texto (não vazio, só letras, dígitos, `.`, `-`, `_` e `+`).
    ///
    /// Erros: [`Error::MinecraftVersionNotFound`] com texto vazio ou caracteres estranhos.
    pub fn new(id: &str) -> Result<Self> {
        if is_version_text(id) {
            Ok(Self(id.to_owned()))
        } else {
            Err(Error::MinecraftVersionNotFound {
                minecraft: id.to_owned(),
                engine: "texto de versão inválido".into(),
            })
        }
    }

    /// O texto.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for MinecraftVersionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Texto de versão aceitável (Minecraft ou loader).
fn is_version_text(text: &str) -> bool {
    !text.is_empty()
        && text.len() <= 64
        && text
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_' | b'+'))
}

/// O loader e a sua versão **exata**, a do `pack.toml` (ARCHITECTURE §7.1, D7).
///
/// Não existe "estável" nem "mais nova": sem versão fixa o portablemc consulta o meta do
/// loader na rede a cada abertura (S-R5-3 §13).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, specta::Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum LoaderSpec {
    /// Minecraft sem loader.
    Vanilla,
    /// Fabric Loader (`0.19.5`).
    Fabric {
        /// Versão do loader.
        version: String,
    },
    /// Quilt Loader. Só fumaça do motor na matriz L-05: o Quilt está fora da v1 (SPEC §9).
    Quilt {
        /// Versão do loader.
        version: String,
    },
    /// Forge, com a versão do `pack.toml` (`47.4.10`, `10.13.4.1614`).
    Forge {
        /// Versão do loader.
        version: String,
    },
    /// NeoForge, com a versão do `pack.toml` (`21.1.252`; na 1.20.1, `47.1.106`).
    NeoForge {
        /// Versão do loader.
        version: String,
    },
}

impl LoaderSpec {
    /// Nome do loader para as frases.
    #[must_use]
    pub const fn label(&self) -> &'static str {
        match self {
            Self::Vanilla => "Minecraft",
            Self::Fabric { .. } => "Fabric",
            Self::Quilt { .. } => "Quilt",
            Self::Forge { .. } => "Forge",
            Self::NeoForge { .. } => "NeoForge",
        }
    }

    /// A versão do loader (nenhuma no vanilla).
    #[must_use]
    pub fn version(&self) -> Option<&str> {
        match self {
            Self::Vanilla => None,
            Self::Fabric { version }
            | Self::Quilt { version }
            | Self::Forge { version }
            | Self::NeoForge { version } => Some(version),
        }
    }

    /// Confere que a versão é exata.
    ///
    /// Erros: [`Error::LoaderVersionMissing`] com versão vazia, apelido (`stable`, `latest`,
    /// `recommended`…) ou caracteres que não aparecem em versões.
    pub fn validate(&self) -> Result<()> {
        let Some(version) = self.version() else {
            return Ok(());
        };
        let alias = matches!(
            version.to_ascii_lowercase().as_str(),
            "stable" | "unstable" | "latest" | "recommended" | "newest" | "release" | "snapshot"
        );
        if alias || !is_version_text(version) {
            return Err(Error::LoaderVersionMissing {
                loader: self.label().to_owned(),
                version: version.to_owned(),
            });
        }
        Ok(())
    }
}

/// O que instalar: Minecraft + loader com a versão exata.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct GameSpec {
    /// Versão do Minecraft.
    pub minecraft: MinecraftVersionId,
    /// Loader e versão.
    pub loader: LoaderSpec,
}

impl GameSpec {
    /// Monta e valida.
    ///
    /// Erros: os de [`MinecraftVersionId::new`] e [`LoaderSpec::validate`].
    pub fn new(minecraft: &str, loader: LoaderSpec) -> Result<Self> {
        let spec = Self {
            minecraft: MinecraftVersionId::new(minecraft)?,
            loader,
        };
        spec.loader.validate()?;
        Ok(spec)
    }
}

impl fmt::Display for GameSpec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.loader.version() {
            Some(version) => write!(f, "{} {} {version}", self.minecraft, self.loader.label()),
            None => write!(f, "{}", self.minecraft),
        }
    }
}

/// O Java do teste, escolhido pela `warden-java`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JavaRuntime {
    /// `java` com console: o motor usa para instalar (processors do Forge/NeoForge).
    pub java: PathBuf,
    /// O programa que abre o jogo: `javaw.exe` no Windows, `java` no Linux.
    pub launcher: PathBuf,
    /// Major (8, 17, 21, 25).
    pub major: u32,
}

impl JavaRuntime {
    /// O Java instalado pela `warden-java`.
    #[must_use]
    pub fn from_installed(runtime: &warden_java::InstalledRuntime) -> Self {
        Self {
            java: runtime.java.clone(),
            launcher: runtime.launcher.clone(),
            major: runtime.version.major,
        }
    }
}

/// Entrar direto no jogo (ARCHITECTURE §7.8).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum QuickPlay {
    /// Abre um mundo de `saves/` (só 1.20+, a partir do 23w14a).
    Singleplayer {
        /// Nome da pasta do mundo.
        world: String,
    },
    /// Conecta a um servidor (1.20+: `--quickPlayMultiplayer`; antes: `--server`/`--port`).
    Multiplayer {
        /// Endereço.
        host: String,
        /// Porta.
        port: u16,
    },
}

/// Como abrir o jogo instalado.
#[derive(Debug, Clone)]
pub struct LaunchOptions {
    /// O `.minecraft` da instância (`instances/<id>/minecraft`, ou a da busca, ou a temporária).
    pub game_dir: PathBuf,
    /// Pasta do Warden para arquivos da abertura (o `@argfile`), fora do `gameDir` para não
    /// entrar na captura (`instances/<id>/state/`).
    pub state_dir: PathBuf,
    /// O Java.
    pub java: JavaRuntime,
    /// Memória máxima (`-Xmx`), em MB.
    pub memory_mb: u32,
    /// Argumentos JVM do usuário, já validados; vêm depois dos do Warden (o último vale).
    pub extra_jvm_args: Vec<String>,
    /// Propriedades de sistema pedidas por quem abre (`fml.queryResult`, `fabric.noGui`, log
    /// `TRACE` do perfil de desempenho).
    pub system_props: Vec<(String, String)>,
    /// Entrar direto num mundo ou servidor.
    pub quick_play: Option<QuickPlay>,
    /// `--quickPlayPath`: arquivo (relativo ao `gameDir`) que o jogo grava ao entrar no mundo
    /// (só 1.20+; ignorado antes).
    pub quick_play_path: Option<String>,
    /// O jogador offline.
    pub player: OfflineProfile,
}

/// De onde vem a configuração do log4j do jogo (ARCHITECTURE §7.4).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum LoggingConfig {
    /// O XML da Mojang do JSON da versão (`client-1.12.xml`), com as correções do `Log4Shell`.
    Mojang {
        /// Nome do arquivo.
        file: String,
    },
    /// A configuração do próprio loader (Forge com `"logging": {}`, NeoForge), verificada
    /// como segura ou numa versão com log4j corrigido.
    Loader,
    /// Nenhuma (versão sem `logging`).
    None,
}

/// O que o motor instalou: tudo o que é preciso para montar a linha de comando.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledGame {
    /// O que foi pedido.
    pub spec: GameSpec,
    /// Id da versão raiz (`fabric-loader-0.19.5-1.20.1`, `forge-1.20.1-47.4.10`).
    pub version_id: String,
    /// Hierarquia de versões, da raiz até o vanilla.
    pub hierarchy: Vec<String>,
    /// `releaseTime` do vanilla (RFC 3339), usado para decidir argumentos por faixa.
    pub release_time: String,
    /// Classe principal.
    pub main_class: String,
    /// Argumentos JVM do perfil (classpath, natives, log4j), com os caminhos já resolvidos.
    pub jvm_args: Vec<String>,
    /// Argumentos do jogo do perfil, com o jogador e o `gameDir` provisórios do motor.
    pub game_args: Vec<String>,
    /// Pasta de jogo provisória usada na instalação; trocada pelo `game_dir` da abertura.
    pub game_dir_placeholder: String,
    /// Configuração de log escolhida.
    pub logging: LoggingConfig,
}

/// Arquivo de argumentos (`@argfile`) usado quando a linha passa do limite do Windows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArgFile {
    /// Onde gravar.
    pub path: PathBuf,
    /// Conteúdo (um argumento por linha, entre aspas).
    pub contents: String,
}

/// A linha de comando final.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchCommand {
    /// O programa (`javaw.exe`/`java`).
    pub program: PathBuf,
    /// Os argumentos, na ordem (com `@argfile`, só ele).
    pub args: Vec<String>,
    /// Diretório de trabalho (o `gameDir`).
    pub cwd: PathBuf,
    /// Variáveis de ambiente acrescentadas.
    pub env: Vec<(String, String)>,
    /// Arquivo de argumentos a gravar antes de abrir.
    pub argfile: Option<ArgFile>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versao_do_loader_precisa_ser_exata() {
        assert!(
            GameSpec::new(
                "1.20.1",
                LoaderSpec::Fabric {
                    version: "0.19.5".into()
                }
            )
            .is_ok()
        );
        for bad in ["", "stable", "LATEST", "recommended", "0.19 .5", "*"] {
            let error = GameSpec::new(
                "1.20.1",
                LoaderSpec::Forge {
                    version: bad.into(),
                },
            )
            .unwrap_err();
            assert!(
                matches!(error, Error::LoaderVersionMissing { .. }),
                "{bad}: {error}"
            );
        }
        assert!(GameSpec::new("", LoaderSpec::Vanilla).is_err());
        assert!(GameSpec::new("26.3", LoaderSpec::Vanilla).is_ok());
    }

    #[test]
    fn formato_json_do_spec() {
        let spec = GameSpec::new(
            "1.21.1",
            LoaderSpec::NeoForge {
                version: "21.1.252".into(),
            },
        )
        .unwrap();
        let json = serde_json::to_string(&spec).unwrap();
        assert_eq!(
            json,
            r#"{"minecraft":"1.21.1","loader":{"kind":"neoForge","version":"21.1.252"}}"#
        );
        assert_eq!(spec.to_string(), "1.21.1 NeoForge 21.1.252");
    }
}
