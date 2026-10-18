//! Comandos do domínio `perf` (ARCHITECTURE §4.1 e §7.7; SPEC T13; ROADMAP L-12): a parte
//! "Desempenho" do resultado do "Testar com perfil de desempenho".
//!
//! - `perf_report_get`: o que o teste mediu, gravado no `session.json` (campo
//!   [`SESSION_FIELD`]) pelo gancho `perf_profile` ([`crate::test_hooks`]): o tempo por mod lido
//!   do log do loader e o resumo de cada perfil do spark salvo durante o teste;
//! - `perf_open_spark_viewer`: "Abrir no visualizador do spark". O visualizador do spark lê
//!   arquivos do próprio computador (o arquivo é aberto dentro da página, sem envio): o Warden
//!   abre a página no navegador padrão e mostra o arquivo na pasta para arrastar. Nada é
//!   enviado pelo Warden.
//!
//! As amostras ao vivo da faixa de desempenho vão pelo canal do teste (`OperationEvent::
//! PerfSample`, da L-10), não por aqui.
#![allow(clippy::needless_pass_by_value)] // argumentos pertencem ao contrato do Tauri

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State};
use tauri_plugin_opener::OpenerExt as _;
use warden_core::PackId;
use warden_launcher::session::SessionRecord;
use warden_perf::mod_timing::{PhaseTime, TimingSource, TimingStatus};
use warden_perf::spark::{SparkKind, SparkPlatform, TickSummary};

use crate::error::{AppError, AppErrorCode};
use crate::state::AppState;
use crate::test_session::session_dir;

/// Campo do `session.json` com o perfil de desempenho.
pub(crate) const SESSION_FIELD: &str = "perfProfile";

/// Pasta da sessão com as cópias dos arquivos do spark (os da instância podem ser apagados ou
/// trocados no próximo teste).
pub(crate) const SPARK_DIR: &str = "spark";

/// O visualizador do spark (abre arquivos locais; ADR-0038).
const SPARK_VIEWER_URL: &str = "https://spark.lucko.me/";

/// Em build de debug, o E2E troca o navegador pelo arquivo apontado por esta variável (a
/// mesma do "Relatar problema"): o endereço e o arquivo do spark são gravados nele em vez de
/// abrir.
#[cfg(debug_assertions)]
const E2E_OPEN_URL_ENV: &str = "WARDEN_E2E_OPEN_URL";

/// Onde o perfil foi medido.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) enum PerfSide {
    /// No jogo ("Testar com perfil de desempenho").
    Client,
    /// No servidor local ("Testar como servidor" com o perfil).
    Server,
}

/// O spark do servidor local, ligado pelo Warden.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) enum ServerSpark {
    /// Ligado pelo console quando o jogo abriu (e desligado no fim, se ainda estava ligado).
    Started,
    /// O spark não está entre os mods do servidor.
    NotInstalled,
    /// O comando não chegou ao servidor (ele fechou antes, por exemplo).
    CommandFailed,
}

/// Um mod no "O que mais demorou para carregar".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LoadRow {
    /// Como o log chama o mod (id, ou o nome de exibição no Forge 1.8 a 1.12.2).
    pub(crate) key: String,
    /// Nome para mostrar: o do item do pack, quando o índice liga o id a ele; senão a chave.
    pub(crate) name: String,
    /// O item do pack (caminho do metafile), quando se sabe.
    pub(crate) item: Option<String>,
    /// Duração, em ms.
    pub(crate) ms: u32,
    /// As fases, na ordem.
    pub(crate) phases: Vec<PhaseTime>,
}

/// O tempo de carregamento por mod.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LoadReport {
    /// O que a leitura conseguiu.
    pub(crate) status: TimingStatus,
    /// Como o log registra.
    pub(crate) source: Option<TimingSource>,
    /// O loader do teste (`forge`, `neoforge`, `fabric`, `quilt`, `vanilla`), para dizer por
    /// que não há dado.
    pub(crate) loader: String,
    /// O arquivo lido, relativo à instância.
    pub(crate) log: Option<String>,
    /// Os mods, do mais demorado ao menos.
    pub(crate) mods: Vec<LoadRow>,
    /// O jogo e o loader, somados, em ms.
    pub(crate) game_and_loader_ms: u32,
    /// Todos os mods, somados, em ms.
    pub(crate) mods_ms: u32,
}

/// Um mod (ou o jogo) no resumo de um perfil do spark.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SparkRow {
    /// Nome para mostrar (o do item do pack, o id do mod).
    pub(crate) name: String,
    /// O item do pack, quando se sabe.
    pub(crate) item: Option<String>,
    /// O id do mod, quando se sabe.
    pub(crate) mod_id: Option<String>,
    /// Porcentagem, com uma casa.
    #[specta(type = specta_typescript::Number)]
    pub(crate) percent: f64,
}

/// O resumo de um arquivo do spark salvo durante o teste.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SparkReport {
    /// Nome do arquivo (copiado para a pasta da sessão).
    pub(crate) file: String,
    /// Perfil de processamento ou memória.
    pub(crate) kind: SparkKind,
    /// Quando o arquivo foi salvo (ms desde 1970).
    #[specta(type = Option<specta_typescript::Number>)]
    pub(crate) saved_at_ms: Option<i64>,
    /// O arquivo foi lido (sem isso, o resto fica vazio e a interface diz que não deu para
    /// ler).
    pub(crate) readable: bool,
    /// Onde rodou.
    pub(crate) platform: Option<SparkPlatform>,
    /// Duração do perfil, em ms.
    #[specta(type = Option<specta_typescript::Number>)]
    pub(crate) duration_ms: Option<u64>,
    /// O tempo por tick.
    pub(crate) ticks: Option<TickSummary>,
    /// Os mods que mais usaram processamento (ou memória).
    pub(crate) top: Vec<SparkRow>,
    /// A parte do jogo, do loader e do Java, em %.
    #[specta(type = specta_typescript::Number)]
    pub(crate) game_percent: f64,
}

/// O perfil de desempenho de uma sessão.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PerfReport {
    /// Onde foi medido.
    pub(crate) side: PerfSide,
    /// O tempo por mod (no servidor local, sem valor).
    pub(crate) load: Option<LoadReport>,
    /// O spark do servidor local (só no servidor).
    pub(crate) server_spark: Option<ServerSpark>,
    /// Os arquivos do spark salvos durante o teste, do mais antigo ao mais novo.
    pub(crate) spark: Vec<SparkReport>,
    /// Arquivos do spark salvos além do limite, que ficaram de fora.
    pub(crate) spark_skipped: u32,
}

/// O perfil de desempenho gravado numa sessão (`None` nas sessões sem ele).
pub(crate) fn report_of(record: &SessionRecord) -> Option<PerfReport> {
    record
        .extra
        .get(SESSION_FIELD)
        .and_then(|value| serde_json::from_value(value.clone()).ok())
}

/// O perfil de desempenho de uma sessão (`null` nas sessões sem ele). Erros:
/// `app.TEST_SESSION_NOT_FOUND`.
#[tauri::command]
#[specta::specta]
pub(crate) async fn perf_report_get(
    state: State<'_, AppState>,
    pack_id: PackId,
    session_id: String,
) -> Result<Option<PerfReport>, AppError> {
    let paths = state.paths.clone();
    tokio::task::spawn_blocking(move || {
        let dir = session_dir(&paths, pack_id, &session_id)?;
        let record = dir.read_record().map_err(|_| {
            AppError::new(AppErrorCode::TestSessionNotFound).with_param("sessionId", &session_id)
        })?;
        Ok(report_of(&record))
    })
    .await
    .map_err(|error| AppError::internal(format!("ler o perfil de desempenho: {error}")))?
}

/// "Abrir no visualizador do spark": abre o visualizador no navegador padrão e mostra o arquivo
/// (a cópia na pasta da sessão) no gerenciador de arquivos. Erros: `app.TEST_SESSION_NOT_FOUND`,
/// `app.TEST_ARTIFACT_INVALID` (arquivo que não é desta sessão ou que sumiu).
#[tauri::command]
#[specta::specta]
pub(crate) async fn perf_open_spark_viewer(
    app: AppHandle,
    state: State<'_, AppState>,
    pack_id: PackId,
    session_id: String,
    file: String,
) -> Result<(), AppError> {
    let paths = state.paths.clone();
    let path = tokio::task::spawn_blocking(move || {
        let dir = session_dir(&paths, pack_id, &session_id)?;
        let record = dir.read_record().map_err(|_| {
            AppError::new(AppErrorCode::TestSessionNotFound).with_param("sessionId", &session_id)
        })?;
        let invalid = || AppError::new(AppErrorCode::TestArtifactInvalid).with_param("path", &file);
        let known = report_of(&record)
            .is_some_and(|report| report.spark.iter().any(|spark| spark.file == file));
        if !known {
            return Err(invalid());
        }
        let path = warden_core::resolve_inside(&dir.path().join(SPARK_DIR), &file)
            .map_err(|_| invalid())?;
        if !path.is_file() {
            return Err(invalid());
        }
        Ok(path)
    })
    .await
    .map_err(|error| AppError::internal(format!("abrir o visualizador do spark: {error}")))??;
    tracing::info!("visualizador do spark aberto no navegador");
    #[cfg(debug_assertions)]
    if let Some(target) = std::env::var_os(E2E_OPEN_URL_ENV).filter(|value| !value.is_empty()) {
        let name = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        return std::fs::write(&target, format!("{SPARK_VIEWER_URL}\n{name}\n"))
            .map_err(|error| AppError::internal(format!("E2E: gravar o endereço: {error}")));
    }
    app.opener()
        .open_url(SPARK_VIEWER_URL, None::<&str>)
        .map_err(|error| AppError::internal(format!("abrir o visualizador do spark: {error}")))?;
    app.opener()
        .reveal_item_in_dir(&path)
        .map_err(|error| AppError::internal(format!("mostrar o arquivo do spark: {error}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relatorio_em_camel_case_e_sessao_sem_o_campo() {
        let report = PerfReport {
            side: PerfSide::Client,
            load: Some(LoadReport {
                status: TimingStatus::Measured,
                source: Some(TimingSource::BarStep),
                loader: "forge".into(),
                log: Some("logs/debug.log".into()),
                mods: vec![LoadRow {
                    key: "Just Enough Items".into(),
                    name: "Just Enough Items".into(),
                    item: None,
                    ms: 138,
                    phases: Vec::new(),
                }],
                game_and_loader_ms: 1345,
                mods_ms: 220,
            }),
            server_spark: None,
            spark: Vec::new(),
            spark_skipped: 0,
        };
        let json = serde_json::to_value(&report).unwrap();
        assert_eq!(json["load"]["gameAndLoaderMs"], 1345);
        assert_eq!(json["load"]["source"], "barStep");
        assert_eq!(json["side"], "client");
        let mut record = SessionRecord {
            schema: warden_launcher::session::SESSION_SCHEMA,
            started_at: "2026-10-18T00:00:00Z".into(),
            ended_at: "2026-10-18T00:01:00Z".into(),
            duration_ms: 60_000,
            outcome: warden_launcher::outcome::Outcome::ClosedNormally,
            exit_code: Some(0),
            pack_version: None,
            game: warden_launcher::GameSpec::new(
                "1.12.2",
                warden_launcher::LoaderSpec::Forge {
                    version: "14.23.5.2860".into(),
                },
            )
            .unwrap(),
            java_major: 8,
            artifacts: warden_launcher::outcome::CrashArtifacts::default(),
            extra: serde_json::Map::new(),
        };
        assert_eq!(report_of(&record), None);
        record.extra.insert(SESSION_FIELD.into(), json);
        assert_eq!(report_of(&record), Some(report));
    }
}
