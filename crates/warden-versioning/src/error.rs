//! Erros do domínio `versioning` (ARCHITECTURE §5).
//!
//! [`Error`] implementa [`DomainError`]: cada variante tem um código estável de
//! [`VersioningErrorCode`] e os parâmetros da frase em
//! `apps/desktop/src/i18n/errors/versioning.ts`. Falhas de disco e caminhos recusados chegam
//! como os códigos comuns do domínio `core` ([`CoreError`]).
//!
//! O dono não vê git: as frases falam de "versão", "salvar versão" e "ponto de segurança"
//! (QUALITY §8.2). A mensagem técnica da libgit2 só aparece nos detalhes técnicos.

use std::collections::BTreeMap;
use std::io;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use warden_core::{CoreError, DomainCode, DomainError};

/// Resultado com [`Error`].
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Códigos do domínio `versioning`. O código é contrato: renomear é mudança de contrato;
/// acrescentar é permitido (só acréscimo, ROADMAP §1).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, specta::Type,
)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum VersioningErrorCode {
    /// Bug: invariante quebrada sem código específico.
    Internal,
    /// A pasta do pack ainda não tem histórico de versões.
    NotVersioned,
    /// A pasta do pack já tem histórico de versões.
    AlreadyVersioned,
    /// O histórico pertence a outro usuário do Windows (proteção da libgit2).
    RepoNotOwned,
    /// Outro programa está mexendo no histórico agora (arquivo de trava do git).
    RepoLocked,
    /// O histórico está no meio de uma operação feita por fora (merge, rebase…).
    RepoBusy,
    /// O histórico não está numa linha de versões (HEAD destacado).
    DetachedHead,
    /// O histórico tem conflitos não resolvidos.
    Conflicts,
    /// O texto não é uma versão no formato X.Y.Z.
    InvalidVersion,
    /// A versão pedida não é maior que a última salva.
    VersionNotGreater,
    /// Já existe uma versão salva com esse número.
    VersionExists,
    /// Nada mudou desde a última versão salva.
    NothingChanged,
    /// A versão pedida não existe no histórico.
    VersionNotFound,
    /// O ponto de segurança pedido não existe.
    SafetyPointNotFound,
    /// A versão já foi publicada e não pode deixar de ser versão final.
    VersionPublished,
    /// Um arquivo do pack está aberto em outro programa (o jogo, o antivírus…).
    FileInUse,
    /// A restauração falhou e o pack não pôde voltar sozinho ao estado anterior.
    RollbackFailed,
    /// Falha da biblioteca git ao ler ou gravar o histórico.
    GitFailed,
}

/// Erro das operações de versionamento.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// A pasta não é a raiz de um repositório git.
    #[error("a pasta {root} não tem histórico de versões (repositório git)")]
    NotVersioned {
        /// Pasta do pack.
        root: PathBuf,
    },

    /// A pasta já é a raiz de um repositório git.
    #[error("a pasta {root} já tem histórico de versões (repositório git)")]
    AlreadyVersioned {
        /// Pasta do pack.
        root: PathBuf,
    },

    /// A libgit2 recusou abrir um repositório de outro dono (`safe.directory`).
    #[error("o repositório em {root} pertence a outro usuário")]
    RepoNotOwned {
        /// Pasta do pack.
        root: PathBuf,
        /// Erro da libgit2.
        #[source]
        source: git2::Error,
    },

    /// Arquivo de trava do git presente (`index.lock`, `HEAD.lock`…).
    #[error("o repositório está travado por outro processo ao {action}")]
    RepoLocked {
        /// O que se tentava fazer.
        action: &'static str,
        /// Erro da libgit2.
        #[source]
        source: git2::Error,
    },

    /// Operação em andamento feita por fora do Warden.
    #[error("o repositório está no meio de uma operação: {state}")]
    RepoBusy {
        /// Nome da operação (`merge`, `rebase`, `cherry-pick`…).
        state: String,
    },

    /// `HEAD` não aponta para uma branch.
    #[error("o repositório está com HEAD destacado (fora de uma branch)")]
    DetachedHead,

    /// Arquivos em conflito no índice.
    #[error("o repositório tem conflitos não resolvidos: {}", paths.join(", "))]
    Conflicts {
        /// Arquivos em conflito.
        paths: Vec<String>,
    },

    /// O texto não é `SemVer`.
    #[error("versão inválida {version:?}: {reason}")]
    InvalidVersion {
        /// Texto recebido.
        version: String,
        /// Motivo (do leitor de `SemVer`).
        reason: String,
    },

    /// A versão nova precisa ser maior que todas as já salvas (CA-T16-04).
    #[error("a versão {version} não é maior que a última salva ({last})")]
    VersionNotGreater {
        /// Versão pedida.
        version: String,
        /// Maior versão já salva.
        last: String,
    },

    /// A tag `v<versão>` já existe.
    #[error("a versão {version} já existe")]
    VersionExists {
        /// Versão pedida.
        version: String,
    },

    /// A árvore de trabalho é igual à da última versão (ou ao ponto inicial).
    #[error("nada mudou desde a última versão salva")]
    NothingChanged {
        /// Última versão salva, se houver.
        last: Option<String>,
    },

    /// Nenhuma tag `v<versão>` com esse número.
    #[error("a versão {version} não existe no histórico")]
    VersionNotFound {
        /// Versão pedida.
        version: String,
    },

    /// Nenhuma referência `refs/warden/safety/<nome>`.
    #[error("o ponto de segurança {name} não existe")]
    SafetyPointNotFound {
        /// Nome pedido.
        name: String,
    },

    /// Desmarcar versão final de uma versão já publicada.
    #[error("a versão {version} já foi publicada")]
    VersionPublished {
        /// Versão pedida.
        version: String,
    },

    /// Arquivo aberto por outro programa durante a restauração. O pack ficou como estava.
    #[error("o arquivo {path} está em uso por outro programa")]
    FileInUse {
        /// Caminho relativo à raiz do pack.
        path: String,
        /// Erro do sistema.
        #[source]
        source: io::Error,
    },

    /// A restauração falhou e o desfazer também: o estado anterior está no ponto de
    /// segurança.
    #[error(
        "a restauração falhou e não foi possível desfazer em {}; o estado anterior está no ponto de segurança {safety_point}",
        paths.join(", ")
    )]
    RollbackFailed {
        /// Nome do ponto de segurança com o estado anterior.
        safety_point: String,
        /// Arquivos que não puderam voltar.
        paths: Vec<String>,
        /// A falha original da restauração.
        #[source]
        source: Box<Error>,
    },

    /// Falha da libgit2.
    #[error("falha do git ao {action}")]
    Git {
        /// O que se tentava fazer.
        action: &'static str,
        /// Erro da libgit2.
        #[source]
        source: git2::Error,
    },

    /// Falha de disco ou caminho recusado.
    #[error(transparent)]
    Core(#[from] CoreError),

    /// Invariante quebrada.
    #[error("erro interno: {0}")]
    Internal(String),
}

impl Error {
    /// Embrulha um erro da libgit2 com o que se tentava fazer. Trava de outro processo vira
    /// [`Error::RepoLocked`].
    pub(crate) fn git(action: &'static str, source: git2::Error) -> Self {
        if source.code() == git2::ErrorCode::Locked {
            Self::RepoLocked { action, source }
        } else {
            Self::Git { action, source }
        }
    }

    /// Atalho para [`CoreError::Io`].
    pub(crate) fn io(action: &'static str, path: impl Into<PathBuf>, source: io::Error) -> Self {
        Self::Core(CoreError::io(action, path, source))
    }
}

/// `map_err` curto para chamadas da libgit2.
pub(crate) trait GitContext<T> {
    /// Embrulha o erro com [`Error::git`].
    fn ctx(self, action: &'static str) -> Result<T>;
}

impl<T> GitContext<T> for std::result::Result<T, git2::Error> {
    fn ctx(self, action: &'static str) -> Result<T> {
        self.map_err(|source| Error::git(action, source))
    }
}

impl DomainError for Error {
    type Code = VersioningErrorCode;

    fn code(&self) -> DomainCode<VersioningErrorCode> {
        use VersioningErrorCode as Code;
        DomainCode::Domain(match self {
            Self::NotVersioned { .. } => Code::NotVersioned,
            Self::AlreadyVersioned { .. } => Code::AlreadyVersioned,
            Self::RepoNotOwned { .. } => Code::RepoNotOwned,
            Self::RepoLocked { .. } => Code::RepoLocked,
            Self::RepoBusy { .. } => Code::RepoBusy,
            Self::DetachedHead => Code::DetachedHead,
            Self::Conflicts { .. } => Code::Conflicts,
            Self::InvalidVersion { .. } => Code::InvalidVersion,
            Self::VersionNotGreater { .. } => Code::VersionNotGreater,
            Self::VersionExists { .. } => Code::VersionExists,
            Self::NothingChanged { .. } => Code::NothingChanged,
            Self::VersionNotFound { .. } => Code::VersionNotFound,
            Self::SafetyPointNotFound { .. } => Code::SafetyPointNotFound,
            Self::VersionPublished { .. } => Code::VersionPublished,
            Self::FileInUse { .. } => Code::FileInUse,
            Self::RollbackFailed { .. } => Code::RollbackFailed,
            Self::Git { .. } => Code::GitFailed,
            Self::Internal(_) => Code::Internal,
            Self::Core(core) => return core.code().map_core(),
        })
    }

    fn params(&self) -> BTreeMap<String, String> {
        let mut params = BTreeMap::new();
        let mut put = |name: &str, value: String| {
            params.insert(name.to_owned(), value);
        };
        match self {
            Self::NotVersioned { root }
            | Self::AlreadyVersioned { root }
            | Self::RepoNotOwned { root, .. } => put("path", root.display().to_string()),
            Self::RepoBusy { state } => put("state", state.clone()),
            Self::Conflicts { paths } => put("paths", paths.join(", ")),
            Self::InvalidVersion { version, .. }
            | Self::VersionExists { version }
            | Self::VersionNotFound { version }
            | Self::VersionPublished { version } => put("version", version.clone()),
            Self::VersionNotGreater { version, last } => {
                put("version", version.clone());
                put("last", last.clone());
            }
            Self::NothingChanged { last } => {
                if let Some(last) = last {
                    put("last", last.clone());
                }
            }
            Self::SafetyPointNotFound { name } => put("name", name.clone()),
            Self::FileInUse { path, .. } => put("path", path.clone()),
            Self::RollbackFailed {
                safety_point,
                paths,
                ..
            } => {
                put("safetyPoint", safety_point.clone());
                put("paths", paths.join(", "));
            }
            Self::Core(core) => return core.params(),
            Self::RepoLocked { .. } | Self::DetachedHead | Self::Git { .. } | Self::Internal(_) => {
            }
        }
        params
    }

    fn detail(&self) -> Option<String> {
        match self {
            Self::Core(core) => core.detail(),
            _ => Some(warden_core::error_chain(self)),
        }
    }

    fn retryable(&self) -> bool {
        match self {
            Self::RepoLocked { .. } | Self::FileInUse { .. } => true,
            Self::Core(core) => core.retryable(),
            _ => false,
        }
    }
}

/// Converte o código de um [`CoreError`] para o código deste domínio.
trait MapCore {
    fn map_core(self) -> DomainCode<VersioningErrorCode>;
}

impl MapCore for DomainCode<warden_core::NoDomainCode> {
    fn map_core(self) -> DomainCode<VersioningErrorCode> {
        match self {
            DomainCode::Core(code) => DomainCode::Core(code),
            DomainCode::Domain(never) => match never {},
        }
    }
}

#[cfg(test)]
mod tests {
    use warden_core::CoreErrorCode;

    use super::*;

    fn git_error(code: git2::ErrorCode) -> git2::Error {
        git2::Error::new(code, git2::ErrorClass::Index, "mensagem da libgit2")
    }

    #[test]
    fn codigos_em_maiusculas_com_sublinhado() {
        let json = serde_json::to_value([
            VersioningErrorCode::NotVersioned,
            VersioningErrorCode::VersionNotGreater,
            VersioningErrorCode::RollbackFailed,
            VersioningErrorCode::GitFailed,
        ])
        .unwrap();
        assert_eq!(
            json,
            serde_json::json!([
                "NOT_VERSIONED",
                "VERSION_NOT_GREATER",
                "ROLLBACK_FAILED",
                "GIT_FAILED"
            ])
        );
    }

    #[test]
    #[allow(clippy::too_many_lines)] // uma linha da tabela por variante
    fn cada_variante_tem_codigo_e_parametros() {
        use VersioningErrorCode as Code;
        let cases: Vec<(Error, Code, &[&str])> = vec![
            (
                Error::NotVersioned {
                    root: "C:/p".into(),
                },
                Code::NotVersioned,
                &["path"],
            ),
            (
                Error::AlreadyVersioned {
                    root: "C:/p".into(),
                },
                Code::AlreadyVersioned,
                &["path"],
            ),
            (
                Error::RepoNotOwned {
                    root: "C:/p".into(),
                    source: git_error(git2::ErrorCode::Owner),
                },
                Code::RepoNotOwned,
                &["path"],
            ),
            (
                Error::git("gravar o índice", git_error(git2::ErrorCode::Locked)),
                Code::RepoLocked,
                &[],
            ),
            (
                Error::RepoBusy {
                    state: "merge".into(),
                },
                Code::RepoBusy,
                &["state"],
            ),
            (Error::DetachedHead, Code::DetachedHead, &[]),
            (
                Error::Conflicts {
                    paths: vec!["a".into(), "b".into()],
                },
                Code::Conflicts,
                &["paths"],
            ),
            (
                Error::InvalidVersion {
                    version: "x".into(),
                    reason: "r".into(),
                },
                Code::InvalidVersion,
                &["version"],
            ),
            (
                Error::VersionNotGreater {
                    version: "1.0.0".into(),
                    last: "1.2.0".into(),
                },
                Code::VersionNotGreater,
                &["last", "version"],
            ),
            (
                Error::VersionExists {
                    version: "1.0.0".into(),
                },
                Code::VersionExists,
                &["version"],
            ),
            (
                Error::NothingChanged {
                    last: Some("1.0.0".into()),
                },
                Code::NothingChanged,
                &["last"],
            ),
            (
                Error::NothingChanged { last: None },
                Code::NothingChanged,
                &[],
            ),
            (
                Error::VersionNotFound {
                    version: "9.9.9".into(),
                },
                Code::VersionNotFound,
                &["version"],
            ),
            (
                Error::SafetyPointNotFound { name: "x".into() },
                Code::SafetyPointNotFound,
                &["name"],
            ),
            (
                Error::VersionPublished {
                    version: "1.0.0".into(),
                },
                Code::VersionPublished,
                &["version"],
            ),
            (
                Error::FileInUse {
                    path: "mods/a.jar".into(),
                    source: io::Error::other("em uso"),
                },
                Code::FileInUse,
                &["path"],
            ),
            (
                Error::RollbackFailed {
                    safety_point: "20261004T120000Z-voltar".into(),
                    paths: vec!["a".into()],
                    source: Box::new(Error::Internal("x".into())),
                },
                Code::RollbackFailed,
                &["paths", "safetyPoint"],
            ),
            (
                Error::git("ler", git_error(git2::ErrorCode::GenericError)),
                Code::GitFailed,
                &[],
            ),
            (Error::Internal("x".into()), Code::Internal, &[]),
        ];
        for (error, code, keys) in cases {
            assert_eq!(error.code(), DomainCode::Domain(code), "{error}");
            let params = error.params();
            let got: Vec<&str> = params.keys().map(String::as_str).collect();
            assert_eq!(got, keys, "{error}");
            assert!(error.detail().is_some());
        }
    }

    #[test]
    fn core_passa_codigo_parametros_e_nova_tentativa() {
        let error = Error::io("gravar", "C:/p/a.txt", io::Error::other("disco cheio"));
        assert_eq!(error.code(), DomainCode::Core(CoreErrorCode::Io));
        assert_eq!(error.params()["path"], "C:/p/a.txt");
        assert!(error.retryable());
        assert!(error.detail().unwrap().contains("disco cheio"));
    }

    #[test]
    fn trava_e_arquivo_em_uso_valem_nova_tentativa() {
        assert!(Error::git("x", git_error(git2::ErrorCode::Locked)).retryable());
        assert!(
            Error::FileInUse {
                path: "a".into(),
                source: io::Error::other("x")
            }
            .retryable()
        );
        assert!(!Error::DetachedHead.retryable());
    }

    #[test]
    fn detalhe_traz_a_mensagem_da_libgit2() {
        let error = Error::git("ler o histórico", git_error(git2::ErrorCode::NotFound));
        let detail = error.detail().unwrap();
        assert!(
            detail.starts_with("falha do git ao ler o histórico"),
            "{detail}"
        );
        assert!(detail.contains("mensagem da libgit2"), "{detail}");
    }
}
