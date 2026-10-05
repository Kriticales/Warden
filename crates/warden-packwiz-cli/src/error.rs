//! Erros da crate (ARCHITECTURE §5).
//!
//! Cada falha do packwiz cita o comando (`refresh`, `curseforge add`…) e guarda as últimas
//! linhas da saída já limpa (ARCHITECTURE §6.3: as últimas 200 viram o detalhe técnico). A
//! chave da CurseForge nunca chega aqui: ela não vai no argv e é apagada das linhas antes de
//! qualquer registro.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::io;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use warden_core::{CoreErrorCode, DomainCode, DomainError, error_chain};

/// Quantas linhas finais da saída do packwiz entram no detalhe técnico.
pub const DETAIL_LINES: usize = 200;

/// Códigos do domínio `packwizCli`. O código é contrato: renomear é mudança de contrato;
/// acrescentar é permitido (só acréscimo, ROADMAP §1).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, specta::Type,
)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PackwizCliErrorCode {
    /// Bug: invariante quebrada sem código específico.
    Internal,
    /// O executável do packwiz não foi encontrado.
    BinaryNotFound,
    /// O sistema não deixou iniciar o packwiz.
    SpawnFailed,
    /// A pasta não tem `pack.toml`.
    PackFileMissing,
    /// O packwiz terminou com erro (código de saída diferente de zero).
    CommandFailed,
    /// O packwiz disse que deu certo, mas os arquivos relidos não conferem.
    PostconditionFailed,
    /// O comando precisava da chave da CurseForge e ela não foi informada (mesmo texto do
    /// código da `warden-curseforge`, sem o `_` que o `CurseForge` geraria).
    #[serde(rename = "CURSEFORGE_KEY_MISSING")]
    CurseForgeKeyMissing,
    /// Há mods da CurseForge que só podem ser baixados à mão.
    ManualDownloadsRequired,
    /// Um arquivo não pôde ser baixado e a exportação ficaria incompleta.
    DownloadFailed,
    /// A pasta do pack tem um link simbólico ou junção, que o Warden não segue.
    LinkInPack,
}

/// Um arquivo que a CurseForge não deixa apps de terceiros baixarem.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ManualDownload {
    /// Nome do mod.
    pub name: String,
    /// Nome do arquivo esperado.
    pub file_name: String,
    /// Página onde baixar.
    pub url: String,
}

/// Erro das funções desta crate.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// O executável não existe no caminho esperado.
    #[error("packwiz não encontrado em {}", .path.display())]
    BinaryNotFound {
        /// Onde ele foi procurado.
        path: PathBuf,
    },
    /// Falha ao iniciar o processo.
    #[error("não foi possível iniciar o packwiz ({})", .binary.display())]
    Spawn {
        /// O executável.
        binary: PathBuf,
        /// O erro do sistema.
        #[source]
        source: io::Error,
    },
    /// A pasta indicada não tem `pack.toml`.
    #[error("a pasta {} não tem pack.toml", .dir.display())]
    PackFileMissing {
        /// A pasta do pack.
        dir: PathBuf,
    },
    /// O packwiz saiu com código diferente de zero.
    #[error("packwiz {command} terminou com o código {exit_code}")]
    CommandFailed {
        /// O comando (`refresh`, `curseforge add`…).
        command: &'static str,
        /// O código de saída (`-1` se o processo foi encerrado por sinal).
        exit_code: i32,
        /// Últimas linhas da saída limpa.
        tail: Vec<String>,
    },
    /// Código 0, mas o resultado relido não confere (o packwiz às vezes sai com 0 em falhas,
    /// R3 §1.4).
    #[error("packwiz {command} não produziu o resultado esperado: {reason}")]
    Postcondition {
        /// O comando.
        command: &'static str,
        /// O que não conferiu.
        reason: String,
        /// Últimas linhas da saída limpa.
        tail: Vec<String>,
    },
    /// A saída do sidecar começa com a mensagem do patch 0001 (`WARDEN_CURSEFORGE_API_KEY
    /// ausente`).
    #[error("packwiz {command} precisava da chave da CurseForge, que não foi informada")]
    CurseForgeKeyMissing {
        /// O comando.
        command: &'static str,
    },
    /// O packwiz listou downloads manuais e parou (`cmdshared.ListManualDownloads`).
    #[error("packwiz {command} exige {} download(s) manual(is)", .files.len())]
    ManualDownloads {
        /// O comando.
        command: &'static str,
        /// Os arquivos que precisam ser baixados à mão.
        files: Vec<ManualDownload>,
    },
    /// Linhas `Download of <nome> (<arquivo>) failed`: a exportação seguiria incompleta com
    /// código 0 (R3 §1.4).
    #[error("packwiz {command}: falha ao baixar {}", .files.join(", "))]
    DownloadFailed {
        /// O comando.
        command: &'static str,
        /// Os arquivos que falharam (`nome (arquivo)`).
        files: Vec<String>,
        /// Últimas linhas da saída limpa.
        tail: Vec<String>,
    },
    /// Link simbólico ou junção na pasta do pack (ARCHITECTURE §6.2).
    #[error("o pack tem um link simbólico ou junção em {path:?}")]
    LinkInPack {
        /// Caminho relativo à pasta do pack, com `/`.
        path: String,
    },
    /// Cancelado pelo usuário; o processo e os filhos já foram encerrados.
    #[error("packwiz {command} cancelado")]
    Cancelled {
        /// O comando.
        command: &'static str,
    },
    /// Passou do tempo-limite; o processo e os filhos já foram encerrados.
    #[error("packwiz {command} passou do tempo-limite de {seconds} s")]
    Timeout {
        /// O comando.
        command: &'static str,
        /// O limite, em segundos.
        seconds: u64,
    },
    /// Falha de disco.
    #[error("falha ao {action} {}", .path.display())]
    Io {
        /// O que se tentava fazer ("copiar", "ler"…).
        action: &'static str,
        /// O caminho envolvido.
        path: PathBuf,
        /// O erro do sistema.
        #[source]
        source: io::Error,
    },
    /// Invariante quebrada.
    #[error("erro interno: {0}")]
    Internal(String),
}

impl Error {
    /// Atalho para [`Error::Io`].
    pub(crate) fn io(action: &'static str, path: impl Into<PathBuf>, source: io::Error) -> Self {
        Self::Io {
            action,
            path: path.into(),
            source,
        }
    }

    /// As últimas linhas da saída do packwiz guardadas no erro, se houver.
    #[must_use]
    pub fn output_tail(&self) -> &[String] {
        match self {
            Self::CommandFailed { tail, .. }
            | Self::Postcondition { tail, .. }
            | Self::DownloadFailed { tail, .. } => tail,
            _ => &[],
        }
    }

    /// O comando do packwiz envolvido, se houver.
    #[must_use]
    pub fn command(&self) -> Option<&'static str> {
        match self {
            Self::CommandFailed { command, .. }
            | Self::Postcondition { command, .. }
            | Self::CurseForgeKeyMissing { command }
            | Self::ManualDownloads { command, .. }
            | Self::DownloadFailed { command, .. }
            | Self::Cancelled { command }
            | Self::Timeout { command, .. } => Some(command),
            _ => None,
        }
    }
}

impl DomainError for Error {
    type Code = PackwizCliErrorCode;

    /// Cancelamento, tempo esgotado e falha de disco usam os códigos comuns (ARCHITECTURE §5).
    fn code(&self) -> DomainCode<PackwizCliErrorCode> {
        DomainCode::Domain(match self {
            Self::BinaryNotFound { .. } => PackwizCliErrorCode::BinaryNotFound,
            Self::Spawn { .. } => PackwizCliErrorCode::SpawnFailed,
            Self::PackFileMissing { .. } => PackwizCliErrorCode::PackFileMissing,
            Self::CommandFailed { .. } => PackwizCliErrorCode::CommandFailed,
            Self::Postcondition { .. } => PackwizCliErrorCode::PostconditionFailed,
            Self::CurseForgeKeyMissing { .. } => PackwizCliErrorCode::CurseForgeKeyMissing,
            Self::ManualDownloads { .. } => PackwizCliErrorCode::ManualDownloadsRequired,
            Self::DownloadFailed { .. } => PackwizCliErrorCode::DownloadFailed,
            Self::LinkInPack { .. } => PackwizCliErrorCode::LinkInPack,
            Self::Internal(_) => PackwizCliErrorCode::Internal,
            Self::Cancelled { .. } => return DomainCode::Core(CoreErrorCode::Cancelled),
            Self::Timeout { .. } => return DomainCode::Core(CoreErrorCode::Timeout),
            Self::Io { .. } => return DomainCode::Core(CoreErrorCode::Io),
        })
    }

    /// Valores para a frase traduzida: `command`, `path`, `exitCode`, `files`, `count` ou
    /// `seconds`, conforme o erro.
    fn params(&self) -> BTreeMap<String, String> {
        let mut params = BTreeMap::new();
        if let Some(command) = self.command() {
            params.insert("command".to_owned(), command.to_owned());
        }
        let mut put = |name: &str, value: String| {
            params.insert(name.to_owned(), value);
        };
        match self {
            Self::BinaryNotFound { path } | Self::Spawn { binary: path, .. } => {
                put("path", path.display().to_string());
            }
            Self::PackFileMissing { dir } => put("path", dir.display().to_string()),
            Self::Io { path, .. } => put("path", path.display().to_string()),
            Self::LinkInPack { path } => put("path", path.clone()),
            Self::CommandFailed { exit_code, .. } => put("exitCode", exit_code.to_string()),
            Self::ManualDownloads { files, .. } => {
                put("count", files.len().to_string());
                let names: Vec<&str> = files.iter().map(|file| file.name.as_str()).collect();
                put("files", names.join(", "));
            }
            Self::DownloadFailed { files, .. } => {
                put("count", files.len().to_string());
                put("files", files.join(", "));
            }
            Self::Timeout { seconds, .. } => put("seconds", seconds.to_string()),
            Self::Postcondition { .. }
            | Self::CurseForgeKeyMissing { .. }
            | Self::Cancelled { .. }
            | Self::Internal(_) => {}
        }
        params
    }

    /// A mensagem com as causas e, depois, as últimas linhas da saída do packwiz (ou a lista
    /// de downloads manuais).
    fn detail(&self) -> Option<String> {
        let mut text = error_chain(self);
        if let Self::ManualDownloads { files, .. } = self {
            for file in files {
                let _ = write!(text, "\n{} ({}) — {}", file.name, file.file_name, file.url);
            }
        }
        let tail = self.output_tail();
        if !tail.is_empty() {
            text.push_str("\n\nSaída do packwiz (últimas linhas):");
            for line in tail {
                text.push('\n');
                text.push_str(line);
            }
        }
        Some(text)
    }

    /// Tentar de novo só adianta em falhas passageiras: disco travado (antivírus), tempo
    /// esgotado, falha ao iniciar o processo ou download que falhou.
    fn retryable(&self) -> bool {
        matches!(
            self,
            Self::Io { .. }
                | Self::Timeout { .. }
                | Self::Spawn { .. }
                | Self::DownloadFailed { .. }
        )
    }
}

/// Resultado das funções desta crate.
pub type Result<T, E = Error> = std::result::Result<T, E>;

#[cfg(test)]
mod tests {
    use super::*;

    fn tail() -> Vec<String> {
        vec!["Loading modpack...".to_owned(), "erro final".to_owned()]
    }

    #[test]
    fn codigos_de_cada_variante() {
        let cases: Vec<(Error, DomainCode<PackwizCliErrorCode>)> = vec![
            (
                Error::BinaryNotFound {
                    path: PathBuf::from("x"),
                },
                DomainCode::Domain(PackwizCliErrorCode::BinaryNotFound),
            ),
            (
                Error::Spawn {
                    binary: PathBuf::from("x"),
                    source: io::Error::other("negado"),
                },
                DomainCode::Domain(PackwizCliErrorCode::SpawnFailed),
            ),
            (
                Error::PackFileMissing {
                    dir: PathBuf::from("p"),
                },
                DomainCode::Domain(PackwizCliErrorCode::PackFileMissing),
            ),
            (
                Error::CommandFailed {
                    command: "refresh",
                    exit_code: 1,
                    tail: tail(),
                },
                DomainCode::Domain(PackwizCliErrorCode::CommandFailed),
            ),
            (
                Error::Postcondition {
                    command: "refresh",
                    reason: "r".to_owned(),
                    tail: tail(),
                },
                DomainCode::Domain(PackwizCliErrorCode::PostconditionFailed),
            ),
            (
                Error::CurseForgeKeyMissing {
                    command: "curseforge add",
                },
                DomainCode::Domain(PackwizCliErrorCode::CurseForgeKeyMissing),
            ),
            (
                Error::ManualDownloads {
                    command: "modrinth export",
                    files: vec![],
                },
                DomainCode::Domain(PackwizCliErrorCode::ManualDownloadsRequired),
            ),
            (
                Error::DownloadFailed {
                    command: "modrinth export",
                    files: vec!["A (a.jar)".to_owned()],
                    tail: tail(),
                },
                DomainCode::Domain(PackwizCliErrorCode::DownloadFailed),
            ),
            (
                Error::LinkInPack {
                    path: "mods/x".to_owned(),
                },
                DomainCode::Domain(PackwizCliErrorCode::LinkInPack),
            ),
            (
                Error::Internal("x".to_owned()),
                DomainCode::Domain(PackwizCliErrorCode::Internal),
            ),
            (
                Error::Cancelled { command: "refresh" },
                DomainCode::Core(CoreErrorCode::Cancelled),
            ),
            (
                Error::Timeout {
                    command: "refresh",
                    seconds: 120,
                },
                DomainCode::Core(CoreErrorCode::Timeout),
            ),
            (
                Error::io("copiar", "a", io::Error::other("x")),
                DomainCode::Core(CoreErrorCode::Io),
            ),
        ];
        for (error, code) in cases {
            assert_eq!(error.code(), code, "{error}");
            assert!(!error.to_string().is_empty());
            assert!(error.detail().is_some());
        }
    }

    #[test]
    fn detalhe_traz_as_ultimas_linhas() {
        let error = Error::CommandFailed {
            command: "refresh",
            exit_code: 1,
            tail: tail(),
        };
        let detail = error.detail().unwrap();
        assert!(detail.starts_with("packwiz refresh terminou com o código 1"));
        assert!(detail.ends_with("Loading modpack...\nerro final"));
        assert_eq!(error.params()["exitCode"], "1");
        assert_eq!(error.params()["command"], "refresh");
        assert!(!error.retryable());
    }

    #[test]
    fn parametros_dos_downloads_manuais() {
        let error = Error::ManualDownloads {
            command: "modrinth export",
            files: vec![ManualDownload {
                name: "OptiFine".to_owned(),
                file_name: "OptiFine.jar".to_owned(),
                url: "https://www.curseforge.com/x".to_owned(),
            }],
        };
        let params = error.params();
        assert_eq!(params["count"], "1");
        assert_eq!(params["files"], "OptiFine");
        assert!(
            error
                .detail()
                .unwrap()
                .contains("OptiFine (OptiFine.jar) — https://www.curseforge.com/x")
        );
    }

    #[test]
    fn tentar_de_novo_so_em_falhas_passageiras() {
        assert!(
            Error::Timeout {
                command: "refresh",
                seconds: 1
            }
            .retryable()
        );
        assert!(Error::io("ler", "a", io::Error::other("x")).retryable());
        assert!(
            !Error::CurseForgeKeyMissing {
                command: "curseforge add"
            }
            .retryable()
        );
        assert!(!Error::Cancelled { command: "refresh" }.retryable());
    }

    #[test]
    fn codigo_serializa_em_maiusculas() {
        let json = serde_json::to_string(&PackwizCliErrorCode::CurseForgeKeyMissing).unwrap();
        assert_eq!(json, "\"CURSEFORGE_KEY_MISSING\"");
    }
}
