//! Registros do app (ARCHITECTURE §16).
//!
//! - `tracing` com arquivo diário `logs/warden.AAAA-MM-DD.log` nos dados locais, 14 dias de
//!   retenção, texto legível com horário, nível, alvo e os campos dos spans (`operation_id`,
//!   `pack_id`).
//! - Nível `info`; "detalhado" (Configurações) liga `debug` para o código do Warden, sem
//!   reiniciar.
//! - Os registros do frontend chegam pelo `tauri-plugin-log` (alvo `webview`): o plugin usa a
//!   fachada `log`, que o `tracing-log` encaminha para o mesmo arquivo.
//! - Pânico: o gancho registra a mensagem, o local e a pilha, e mostra um aviso amigável.
//!
//! Proibido registrar segredos (os valores são `SecretString`, cujo `Debug` não mostra nada),
//! cabeçalhos de autorização, ambiente de processos filhos e corpo de respostas da CurseForge.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use tracing_appender::non_blocking::WorkerGuard;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::layer::SubscriberExt as _;
use tracing_subscriber::util::SubscriberInitExt as _;
use tracing_subscriber::{EnvFilter, Registry, fmt, reload};

use crate::error::AppError;
use crate::settings::LogLevel;

/// Dias de registros guardados.
const RETENTION_DAYS: usize = 14;

/// Filtro de cada nível. `warden` cobre todas as crates `warden_*`.
fn filter_for(level: LogLevel) -> EnvFilter {
    let directives = match level {
        LogLevel::Normal => "info",
        LogLevel::Detailed => "info,warden=debug",
    };
    EnvFilter::new(directives)
}

/// Registros em funcionamento: troca de nível e a garantia de que o arquivo recebe tudo antes
/// de o app fechar.
pub struct Logging {
    reload: reload::Handle<EnvFilter, Registry>,
    logs_dir: PathBuf,
    _guard: WorkerGuard,
}

impl std::fmt::Debug for Logging {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Logging")
            .field("logs_dir", &self.logs_dir)
            .finish_non_exhaustive()
    }
}

impl Logging {
    /// Troca o nível na hora.
    pub fn set_level(&self, level: LogLevel) {
        if let Err(error) = self.reload.reload(filter_for(level)) {
            tracing::warn!(%error, "não foi possível trocar o nível dos registros");
        }
    }
}

/// Monta o assinante: filtro recarregável, arquivo diário e, em debug, o terminal.
fn build(
    logs_dir: &Path,
    level: LogLevel,
) -> Result<(impl tracing::Subscriber + Send + Sync + 'static, Logging), AppError> {
    let appender = RollingFileAppender::builder()
        .rotation(Rotation::DAILY)
        .filename_prefix("warden")
        .filename_suffix("log")
        .max_log_files(RETENTION_DAYS)
        .build(logs_dir)
        .map_err(|error| {
            AppError::new(warden_core::CoreErrorCode::Io)
                .with_param("path", logs_dir.display().to_string())
                .with_detail(format!("registros: {error}"))
        })?;
    let (writer, guard) = tracing_appender::non_blocking(appender);
    let (filter, reload) = reload::Layer::new(filter_for(level));
    let file = fmt::layer()
        .with_writer(writer)
        .with_ansi(false)
        .with_target(true);
    let terminal = cfg!(debug_assertions).then(|| fmt::layer().with_writer(std::io::stderr));
    let subscriber = tracing_subscriber::registry()
        .with(filter)
        .with(file)
        .with(terminal);
    Ok((
        subscriber,
        Logging {
            reload,
            logs_dir: logs_dir.to_path_buf(),
            _guard: guard,
        },
    ))
}

/// Liga os registros para o processo inteiro (uma vez, ao abrir o app).
pub fn init(logs_dir: &Path, level: LogLevel) -> Result<Logging, AppError> {
    let (subscriber, logging) = build(logs_dir, level)?;
    // `try_init` também liga o `tracing-log` (registros da fachada `log`, como os do
    // `tauri-plugin-log` e do Tauri).
    subscriber
        .try_init()
        .map_err(|error| AppError::internal(format!("registros já ligados: {error}")))?;
    Ok(logging)
}

/// Registros ligados só durante `work` (testes).
#[cfg(test)]
pub(crate) fn with_file_logging<T>(
    logs_dir: &Path,
    level: LogLevel,
    work: impl FnOnce() -> T,
) -> T {
    let (subscriber, logging) = build(logs_dir, level).unwrap();
    let result = tracing::subscriber::with_default(subscriber, work);
    // Soltar a guarda grava o que ainda está na fila.
    drop(logging);
    result
}

/// Handle do app para o aviso de pânico.
static PANIC_APP: OnceLock<tauri::AppHandle> = OnceLock::new();

thread_local! {
    /// Evita pânico dentro do gancho de pânico (ex.: o diálogo falhar).
    static IN_PANIC_HOOK: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Instala o gancho de pânico: registra a mensagem, o local e a pilha; com o app aberto,
/// mostra um aviso amigável.
pub fn install_panic_hook() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        if IN_PANIC_HOOK.with(|flag| flag.replace(true)) {
            return;
        }
        let message = info
            .payload()
            .downcast_ref::<&str>()
            .map(|text| (*text).to_owned())
            .or_else(|| info.payload().downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "(sem mensagem)".to_owned());
        let location = info
            .location()
            .map_or_else(String::new, ToString::to_string);
        let backtrace = std::backtrace::Backtrace::force_capture();
        tracing::error!(target: "warden_app::panic", %location, %message, %backtrace, "pânico");
        if cfg!(debug_assertions) {
            previous(info);
        }
        if let Some(app) = PANIC_APP.get() {
            show_panic_dialog(app);
        }
        IN_PANIC_HOOK.with(|flag| flag.set(false));
    }));
}

/// Liga o aviso de pânico ao app aberto.
pub fn set_panic_app(app: tauri::AppHandle) {
    // Só a primeira chamada vale (o app é um só).
    let _ = PANIC_APP.set(app);
}

fn show_panic_dialog(app: &tauri::AppHandle) {
    use tauri_plugin_dialog::{DialogExt as _, MessageDialogKind};
    app.dialog()
        .message(
            "O Warden encontrou um erro interno e registrou os detalhes. \
             Se algo parar de responder, feche e abra o Warden de novo.",
        )
        .title("Warden")
        .kind(MessageDialogKind::Error)
        .show(|_| {});
}

#[cfg(test)]
mod tests {
    use super::*;

    fn log_text(dir: &Path) -> String {
        let mut text = String::new();
        for entry in std::fs::read_dir(dir).unwrap() {
            let entry = entry.unwrap();
            let name = entry.file_name().to_string_lossy().into_owned();
            assert!(
                name.starts_with("warden.")
                    && Path::new(&name).extension().is_some_and(|ext| ext == "log"),
                "arquivo inesperado: {name}"
            );
            text.push_str(&std::fs::read_to_string(entry.path()).unwrap());
        }
        text
    }

    #[test]
    fn arquivo_diario_com_nivel_alvo_e_campos_dos_spans() {
        let dir = tempfile::tempdir().unwrap();
        with_file_logging(dir.path(), LogLevel::Normal, || {
            let span = tracing::info_span!("operacao", operation_id = "01J9", pack_id = "01JA");
            let _entered = span.enter();
            tracing::info!(target: "warden_app::teste", "mensagem informativa");
            tracing::debug!(target: "warden_app::teste", "mensagem de depuração");
        });
        let text = log_text(dir.path());
        assert!(text.contains("INFO"), "{text}");
        assert!(text.contains("warden_app::teste"), "{text}");
        assert!(text.contains("mensagem informativa"), "{text}");
        assert!(text.contains("operation_id=\"01J9\""), "{text}");
        assert!(text.contains("pack_id=\"01JA\""), "{text}");
        assert!(!text.contains("mensagem de depuração"), "{text}");
        assert!(!text.contains('\u{1b}'), "sem códigos ANSI no arquivo");
        let name = std::fs::read_dir(dir.path())
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .file_name();
        let name = name.to_string_lossy();
        // warden.AAAA-MM-DD.log
        assert_eq!(name.len(), "warden.2026-10-04.log".len(), "{name}");
    }

    #[test]
    fn nivel_detalhado_liga_debug_do_warden_e_troca_na_hora() {
        let dir = tempfile::tempdir().unwrap();
        let (subscriber, logging) = build(dir.path(), LogLevel::Detailed).unwrap();
        tracing::subscriber::with_default(subscriber, || {
            tracing::debug!(target: "warden_secrets::x", "depuração do warden");
            tracing::debug!(target: "hyper::x", "depuração de terceiros");
            logging.set_level(LogLevel::Normal);
            tracing::debug!(target: "warden_core::x", "depois de voltar ao normal");
        });
        drop(logging);
        let text = log_text(dir.path());
        assert!(text.contains("depuração do warden"), "{text}");
        assert!(!text.contains("depuração de terceiros"), "{text}");
        assert!(!text.contains("depois de voltar ao normal"), "{text}");
    }

    #[test]
    fn pasta_invalida_e_erro_de_io() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("arquivo");
        std::fs::write(&file, b"").unwrap();
        let error = build(&file.join("logs"), LogLevel::Normal).err().unwrap();
        assert_eq!(
            error.code,
            crate::error::ErrorCode::Core(warden_core::CoreErrorCode::Io)
        );
    }
}
