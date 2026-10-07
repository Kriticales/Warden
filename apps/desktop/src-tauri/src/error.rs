//! Erro único que atravessa o IPC (ARCHITECTURE §5; ADR-0018).
//!
//! Todo comando devolve `Result<T, AppError>`. O `AppError` nasce do erro de uma crate de
//! domínio (que implementa [`warden_core::DomainError`]) por [`AppError::from_domain`] ou pelo
//! `?`, com o código estável, os parâmetros da frase traduzida, o detalhe técnico (truncado em
//! 16 KB e sem segredos) e, quando houver, a operação em que aconteceu.
//!
//! Registro acréscimo-apenas: cada domínio novo entra como variante de [`ErrorCode`] com o
//! seu `INTERNAL`; códigos novos entram só no enum da própria crate (ROADMAP §1).

use std::collections::BTreeMap;

use serde::Serialize;
use warden_core::{CoreErrorCode, DomainCode, DomainError, NoDomainCode, OperationId};

/// Maior `detail` aceito (ARCHITECTURE §5).
pub(crate) const MAX_DETAIL_BYTES: usize = 16 * 1024;

/// Erro devolvido por todo comando (`Result<T, AppError>`). A interface mostra a frase do
/// catálogo para `code`, com `params`, e `detail` e `operationId` em "Detalhes técnicos".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type, thiserror::Error)]
#[serde(rename_all = "camelCase")]
#[error("{code}")]
pub struct AppError {
    /// Código estável, por domínio.
    pub code: ErrorCode,
    /// Valores para a frase traduzida (nome do mod, versão…).
    pub params: BTreeMap<String, String>,
    /// Evidência técnica (saída de ferramenta, status HTTP, caminho), sem segredos e com até
    /// 16 KB.
    pub detail: Option<String>,
    /// Se repetir a mesma ação pode dar certo.
    pub retryable: bool,
    /// Operação em que o erro aconteceu, quando houver.
    pub operation_id: Option<OperationId>,
}

impl AppError {
    /// Erro com um código, sem parâmetros.
    #[must_use]
    pub fn new(code: impl Into<ErrorCode>) -> Self {
        Self {
            code: code.into(),
            params: BTreeMap::new(),
            detail: None,
            retryable: false,
            operation_id: None,
        }
    }

    /// Erro de bug: invariante quebrada, sem código específico ainda.
    #[must_use]
    pub fn internal(detail: impl Into<String>) -> Self {
        Self::new(AppErrorCode::Internal).with_detail(detail)
    }

    /// Operação cancelada pelo usuário (`core.CANCELLED`).
    #[must_use]
    pub fn cancelled() -> Self {
        Self::new(CoreErrorCode::Cancelled)
    }

    /// Converte o erro de uma crate de domínio.
    #[must_use]
    pub fn from_domain<E>(error: &E) -> Self
    where
        E: DomainError,
        E::Code: Into<ErrorCode>,
    {
        let code = match error.code() {
            DomainCode::Core(code) => ErrorCode::Core(code),
            DomainCode::Domain(code) => code.into(),
        };
        let mut app_error = Self::new(code);
        app_error.params = error.params();
        app_error.detail = error.detail().map(truncate_detail);
        app_error.retryable = error.retryable();
        app_error
    }

    /// Acrescenta o detalhe técnico (truncado em 16 KB).
    #[must_use]
    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(truncate_detail(detail.into()));
        self
    }

    /// Acrescenta um parâmetro da frase.
    #[must_use]
    pub fn with_param(mut self, name: &str, value: impl Into<String>) -> Self {
        self.params.insert(name.to_owned(), value.into());
        self
    }

    /// Marca a operação em que o erro aconteceu.
    #[must_use]
    pub fn in_operation(mut self, operation: OperationId) -> Self {
        self.operation_id = Some(operation);
        self
    }

    /// Se é o cancelamento pedido pelo usuário.
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.code == ErrorCode::Core(CoreErrorCode::Cancelled)
    }
}

impl From<warden_core::CoreError> for AppError {
    fn from(error: warden_core::CoreError) -> Self {
        Self::from_domain(&error)
    }
}

impl From<warden_secrets::SecretsError> for AppError {
    fn from(error: warden_secrets::SecretsError) -> Self {
        Self::from_domain(&error)
    }
}

/// Trunca em [`MAX_DETAIL_BYTES`], sem cortar um caractere ao meio.
fn truncate_detail(mut detail: String) -> String {
    const MARK: &str = "\n… (detalhe truncado)";
    if detail.len() <= MAX_DETAIL_BYTES {
        return detail;
    }
    let mut end = MAX_DETAIL_BYTES - MARK.len();
    while !detail.is_char_boundary(end) {
        end -= 1;
    }
    detail.truncate(end);
    detail.push_str(MARK);
    detail
}

macro_rules! error_domains {
    ($( $(#[$doc:meta])* $variant:ident($code:ty) = $name:literal ),+ $(,)?) => {
        /// Código de erro com o domínio: `{ "domain": "app", "code": "INTERNAL" }` no JSON.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, specta::Type)]
        #[serde(tag = "domain", content = "code", rename_all = "camelCase")]
        pub enum ErrorCode {
            $( $(#[$doc])* $variant($code), )+
        }

        impl ErrorCode {
            /// Nome do domínio no JSON (`packwizCli`, `core`…).
            #[must_use]
            pub const fn domain(&self) -> &'static str {
                match self {
                    $( Self::$variant(_) => $name, )+
                }
            }

            /// Todos os domínios, com o seu `INTERNAL` (testes de contrato).
            #[cfg(test)]
            pub(crate) fn all_internal() -> Vec<Self> {
                vec![$( Self::$variant(<$code>::Internal), )+]
            }
        }

        $(
            impl From<$code> for ErrorCode {
                fn from(code: $code) -> Self {
                    Self::$variant(code)
                }
            }
        )+
    };
}

error_domains! {
    /// Erros do próprio app (`warden-app`).
    App(AppErrorCode) = "app",
    /// Comuns a todos: cancelamento, tempo esgotado, disco, caminho fora da raiz, rede.
    Core(CoreErrorCode) = "core",
    /// Formato packwiz (`warden-packwiz`).
    Packwiz(warden_packwiz::PackwizErrorCode) = "packwiz",
    /// Sidecar do packwiz (`warden-packwiz-cli`).
    PackwizCli(warden_packwiz_cli::PackwizCliErrorCode) = "packwizCli",
    /// Serviço do pack (`warden-project`).
    Project(warden_project::ProjectErrorCode) = "project",
    /// Metadados de jars (`warden-jarmeta`).
    Jarmeta(warden_jarmeta::JarmetaErrorCode) = "jarmeta",
    /// API do Modrinth (`warden-modrinth`).
    Modrinth(warden_modrinth::ModrinthErrorCode) = "modrinth",
    /// API da CurseForge (`warden-curseforge`).
    Curseforge(warden_curseforge::CurseforgeErrorCode) = "curseforge",
    /// Catálogo de versões (`warden-catalog`).
    Catalog(warden_catalog::CatalogErrorCode) = "catalog",
    /// Configs dos mods (`warden-configs`).
    Configs(warden_configs::ConfigsErrorCode) = "configs",
    /// Java (`warden-java`).
    Java(warden_java::JavaErrorCode) = "java",
    /// Launcher (`warden-launcher`).
    Launcher(warden_launcher::LauncherErrorCode) = "launcher",
    /// Instância de teste (`warden-instance`).
    Instance(warden_instance::InstanceErrorCode) = "instance",
    /// Diagnóstico (`warden-diagnostics`).
    Diagnostics(warden_diagnostics::DiagnosticsErrorCode) = "diagnostics",
    /// IA (`warden-ai`).
    Ai(warden_ai::AiErrorCode) = "ai",
    /// Versionamento (`warden-versioning`).
    Versioning(warden_versioning::VersioningErrorCode) = "versioning",
    /// Exportação (`warden-export`).
    Export(warden_export::ExportErrorCode) = "export",
    /// Chaves e tokens (`warden-secrets`).
    Secrets(warden_secrets::SecretsErrorCode) = "secrets",
    /// Cliente HTTP (`warden-http`).
    Http(warden_http::HttpErrorCode) = "http",
    /// Raio-x de mixins (`warden-mixin`; D4).
    Mixin(warden_mixin::MixinErrorCode) = "mixin",
    /// Busca do culpado (`warden-bisect`; D4).
    Bisect(warden_bisect::BisectErrorCode) = "bisect",
    /// Servidor local (`warden-server`; D4).
    Server(warden_server::ServerErrorCode) = "server",
    /// Desempenho do jogo (`warden-perf`; D4).
    Perf(warden_perf::PerfErrorCode) = "perf",
    /// Página de descoberta (`warden-discovery`; D4).
    Discovery(warden_discovery::DiscoveryErrorCode) = "discovery",
    /// Importação (`warden-import`; D4).
    Import(warden_import::ImportErrorCode) = "import",
    /// Scripts KubeJS e CraftTweaker (`warden-scripts`; D4).
    Scripts(warden_scripts::ScriptsErrorCode) = "scripts",
}

impl From<NoDomainCode> for ErrorCode {
    fn from(never: NoDomainCode) -> Self {
        match never {}
    }
}

impl std::fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // O código é uma variante sem dados: o JSON é `{"domain": "...", "code": "..."}`.
        let code = serde_json::to_value(self)
            .ok()
            .and_then(|value| value.get("code")?.as_str().map(str::to_owned))
            .unwrap_or_default();
        write!(f, "{}.{code}", self.domain())
    }
}

/// Códigos do domínio `app`. Só acréscimo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, specta::Type)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AppErrorCode {
    /// Bug: todo `INTERNAL` visto em teste ou uso vira tarefa para um código específico.
    Internal,
    /// A operação pedida não existe (já terminou há muito tempo ou o id está errado).
    OperationNotFound,
    /// A operação não pode ser cancelada neste momento.
    OperationNotCancellable,
    /// Um valor das configurações foi recusado (`params.field` diz qual).
    SettingsInvalid,
    /// O `settings.json` foi gravado por uma versão mais nova do Warden; esta não o altera.
    SettingsNewerVersion,
    /// Já existe um jogo aberto (ou em preparação) no Warden; um por vez (L-04). `params`:
    /// `packId`, `packName`.
    GameAlreadyRunning,
    /// "Parar jogo" sem jogo aberto neste pack.
    NoGameRunning,
    /// O modo de teste pedido ainda não existe nesta versão (`params.mode`).
    TestModeUnavailable,
    /// Arquivos que o pack colocou na instância mudaram durante o teste anterior e seriam
    /// substituídos (`params.count`, `params.files`).
    TestInstanceChanged,
    /// Mods da CurseForge precisam de download manual (`params.count`, `params.names`).
    TestManualDownloads,
    /// O loader do pack não é aceito pelo Testar (`params.loader`).
    TestLoaderUnsupported,
    /// A sessão de teste pedida não existe mais (`params.sessionId`).
    TestSessionNotFound,
    /// A instância de teste ainda não existe (o pack nunca foi testado).
    TestInstanceMissing,
    /// Aviso: memória acima de 8 GB com Java 8 (`params.memoryMb`).
    TestMemoryHighJava8,
    /// O arquivo pedido não é um arquivo de travamento da sessão (`params.path`).
    TestArtifactInvalid,
}

#[cfg(test)]
mod tests {
    use super::*;
    use warden_secrets::{SecretKind, SecretsError, SecretsErrorCode};

    #[test]
    fn serializa_com_dominio_e_codigo() {
        let operation: OperationId = "01J9ZQ0000000000000000000A".parse().unwrap();
        let error = AppError::internal("falhou").in_operation(operation);
        let json = serde_json::to_value(&error).unwrap();
        assert_eq!(
            json,
            serde_json::json!({
                "code": { "domain": "app", "code": "INTERNAL" },
                "params": {},
                "detail": "falhou",
                "retryable": false,
                "operationId": "01J9ZQ0000000000000000000A",
            })
        );
        assert_eq!(error.to_string(), "app.INTERNAL");
    }

    #[test]
    fn todos_os_dominios_da_arquitetura_tem_internal() {
        let domains: Vec<_> = ErrorCode::all_internal()
            .iter()
            .map(|code| serde_json::to_value(code).unwrap())
            .collect();
        let names: Vec<_> = domains
            .iter()
            .map(|json| {
                assert_eq!(json["code"], "INTERNAL");
                json["domain"].as_str().unwrap().to_owned()
            })
            .collect();
        assert_eq!(
            names,
            [
                "app",
                "core",
                "packwiz",
                "packwizCli",
                "project",
                "jarmeta",
                "modrinth",
                "curseforge",
                "catalog",
                "configs",
                "java",
                "launcher",
                "instance",
                "diagnostics",
                "ai",
                "versioning",
                "export",
                "secrets",
                "http",
                "mixin",
                "bisect",
                "server",
                "perf",
                "discovery",
                "import",
                "scripts",
            ]
        );
        for code in ErrorCode::all_internal() {
            assert_eq!(code.to_string(), format!("{}.INTERNAL", code.domain()));
        }
    }

    #[test]
    fn converte_erro_de_dominio() {
        let error = AppError::from(SecretsError::InvalidValue {
            kind: SecretKind::Gemini,
            reason: "vazio",
        });
        assert_eq!(
            error.code,
            ErrorCode::Secrets(SecretsErrorCode::InvalidValue)
        );
        assert_eq!(error.params["secret"], "gemini");
        assert!(error.detail.unwrap().contains("vazio"));
        assert_eq!(error.operation_id, None);
    }

    #[test]
    fn converte_codigo_comum_de_outra_crate() {
        let error = AppError::from(warden_core::CoreError::Cancelled);
        assert!(error.is_cancelled());
        assert_eq!(error, AppError::cancelled());
        let io = AppError::from(SecretsError::Core(warden_core::CoreError::io(
            "gravar",
            "x",
            std::io::Error::other("disco"),
        )));
        assert_eq!(io.code, ErrorCode::Core(CoreErrorCode::Io));
        assert!(io.retryable);
    }

    #[test]
    fn detalhe_longo_e_truncado_sem_quebrar_caractere() {
        let long = "ção".repeat(MAX_DETAIL_BYTES);
        let error = AppError::internal(long);
        let detail = error.detail.unwrap();
        assert!(detail.len() <= MAX_DETAIL_BYTES);
        assert!(detail.ends_with("(detalhe truncado)"));
        let short = AppError::internal("curto");
        assert_eq!(short.detail.as_deref(), Some("curto"));
    }

    #[test]
    fn parametros_e_codigos_do_app() {
        let error = AppError::new(AppErrorCode::SettingsInvalid).with_param("field", "playerName");
        assert_eq!(error.params["field"], "playerName");
        assert_eq!(error.to_string(), "app.SETTINGS_INVALID");
        assert_eq!(
            AppError::new(AppErrorCode::OperationNotCancellable).to_string(),
            "app.OPERATION_NOT_CANCELLABLE"
        );
    }
}
