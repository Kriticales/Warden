//! Comando de "Navegar por modpacks" (SPEC T08; ARCHITECTURE §4.1 e §17.1; P1-17).
//!
//! - `modpack_contents`: a lista **Mods deste modpack**, com o estado de cada mod para o pack
//!   aberto. A busca de modpacks é a busca combinada com `kind = modpack` (`search_projects`),
//!   e "Adicionar N selecionados" é o `add_plan`/`add_apply` de vários itens.
//!
//! A leitura é uma operação cancelável (`add.modpackContents`, ARCHITECTURE §15): o ID chega
//! pelo canal, e o painel chama `operation_cancel` ao fechar ou trocar de modpack. Cancelada, a
//! leitura por partes para na hora e fecha a conexão.
//!
//! Fino: lê o pack (com a trava de leitura) e chama a `warden-discovery`. Não grava nada: nem no
//! pack, nem na pasta de dados (o arquivo do modpack é lido por partes, só em memória).
#![allow(clippy::needless_pass_by_value)] // argumentos pertencem ao contrato do Tauri

use std::sync::OnceLock;

use tauri::State;
use tauri::ipc::Channel;
use warden_core::{CancellationToken, PackId};
use warden_discovery::modpacks::{
    ModpackContents, ModpackFailure, ModpackSources, PackView, modpack_contents as contents,
};
use warden_http::{HttpClient, HttpConfig};
use warden_project::add::plan::PackState;
use warden_project::search::SourceId;

use crate::commands::add_curseforge::ready_client;
use crate::commands::inventory::pack_root;
use crate::error::AppError;
use crate::events::OperationEvent;
use crate::operations::OperationKind;
use crate::state::AppState;

/// "Ler os mods do modpack".
const CONTENTS: OperationKind = OperationKind::new("add.modpackContents");

/// O cliente HTTP da leitura por partes, um só para o app (divide conexões e limites).
fn http() -> Result<HttpClient, AppError> {
    static CLIENT: OnceLock<HttpClient> = OnceLock::new();
    if let Some(client) = CLIENT.get() {
        return Ok(client.clone());
    }
    let client = HttpClient::new(HttpConfig::for_version(env!("CARGO_PKG_VERSION")))
        .map_err(|error| AppError::from_domain(&error))?;
    Ok(CLIENT.get_or_init(|| client).clone())
}

fn failure(error: ModpackFailure) -> AppError {
    match error {
        ModpackFailure::Source(error) => AppError::from_domain(&error),
        ModpackFailure::Modpack(error) => AppError::from_domain(&error),
    }
}

/// Os mods de um modpack, com "Já no seu pack", "Compatível", "Sem versão para o seu pack" e
/// "Arquivo fora das lojas". Cancelável pelo ID que chega no canal.
#[tauri::command]
#[specta::specta]
pub(crate) async fn modpack_contents(
    state: State<'_, AppState>,
    pack_id: PackId,
    source: SourceId,
    project_id: String,
    on_event: Channel<OperationEvent>,
) -> Result<ModpackContents, AppError> {
    contents_operation(&state, pack_id, source, &project_id, move |event| {
        // O painel pode ter fechado; a leitura para pelo cancelamento.
        let _ = on_event.send(event);
    })
    .await
}

async fn contents_operation(
    state: &AppState,
    pack_id: PackId,
    source: SourceId,
    project_id: &str,
    forward: impl Fn(OperationEvent) + Send + Sync + 'static,
) -> Result<ModpackContents, AppError> {
    let project_id = project_id.trim();
    if project_id.is_empty() {
        return Err(
            AppError::new(warden_project::ProjectErrorCode::InvalidInput)
                .with_param("field", "projectId"),
        );
    }
    let operation = state
        .operations
        .start(CONTENTS, Some(pack_id), true)
        .with_events(forward);
    let result = read_contents(state, pack_id, source, project_id, operation.token()).await;
    let result = if operation.is_cancelled() {
        Err(AppError::cancelled())
    } else {
        result
    };
    operation.finish(result)
}

async fn read_contents(
    state: &AppState,
    pack_id: PackId,
    source: SourceId,
    project_id: &str,
    cancel: &CancellationToken,
) -> Result<ModpackContents, AppError> {
    let pack = {
        let _lock = state.locks.read(pack_id).await;
        let root = pack_root(state, pack_id)?;
        PackState::read(&root).map_err(|error| AppError::from_domain(&error))?
    };
    let sources = ModpackSources {
        modrinth: state.modrinth.clone(),
        curseforge: ready_client(state),
        http: http()?,
    };
    let view = PackView {
        target: &pack.target,
        items: &pack.items,
    };
    contents(&sources, &view, source, project_id, cancel)
        .await
        .map_err(failure)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::sync::{Arc, Mutex};
    use std::time::{Duration, Instant};

    use serde_json::json;
    use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
    use warden_core::OperationId;
    use warden_modrinth::ModrinthClient;
    use warden_project::registry::PackRecord;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;
    use crate::error::ErrorCode;
    use crate::operations::OperationState;
    use crate::state::tests::test_state;

    /// Servidor do arquivo do modpack que manda o cabeçalho e 1 KiB e para; avisa quando o
    /// cliente fecha a conexão.
    async fn stalled_file() -> (String, tokio::sync::oneshot::Receiver<()>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let (closed_tx, closed_rx) = tokio::sync::oneshot::channel();
        tokio::spawn(async move {
            let mut closed_tx = Some(closed_tx);
            while let Ok((mut socket, _)) = listener.accept().await {
                let mut buffer = [0_u8; 4096];
                loop {
                    let mut request = Vec::new();
                    while !request.windows(4).any(|w| w == b"\r\n\r\n") {
                        match socket.read(&mut buffer).await {
                            Ok(0) | Err(_) => break,
                            Ok(n) => request.extend_from_slice(&buffer[..n]),
                        }
                    }
                    if request.is_empty() {
                        break;
                    }
                    let head = "HTTP/1.1 200 OK\r\ncontent-length: 1000000\r\n\r\n";
                    if socket.write_all(head.as_bytes()).await.is_err() {
                        break;
                    }
                    if request.starts_with(b"HEAD") {
                        continue;
                    }
                    let _ = socket.write_all(&[7_u8; 1024]).await;
                    while matches!(socket.read(&mut buffer).await, Ok(n) if n > 0) {}
                    if let Some(tx) = closed_tx.take() {
                        let _ = tx.send(());
                    }
                    break;
                }
            }
        });
        (format!("http://{address}/parado.mrpack"), closed_rx)
    }

    /// Um pack 1.20.1 vazio registrado no estado.
    fn register_pack(state: &AppState, dir: &std::path::Path) -> PackId {
        let root = dir.join("pack");
        std::fs::create_dir_all(&root).unwrap();
        let manifest = warden_packwiz::PackManifest::new("Vale", "1.20.1");
        std::fs::write(root.join("pack.toml"), manifest.to_toml_string()).unwrap();
        std::fs::write(
            root.join("index.toml"),
            warden_packwiz::PackIndex::default().to_toml_string(),
        )
        .unwrap();
        let record = PackRecord {
            id: PackId::new(),
            name: "Vale".into(),
            path: root,
            last_test: None,
            extra: BTreeMap::new(),
        };
        let id = record.id;
        state.packs.insert(record).unwrap();
        id
    }

    /// Revisão: o comando usava um `CancellationToken` próprio, e fechar o painel não parava a
    /// leitura. Agora é uma operação cancelável (`add.modpackContents`): o ID chega pelo canal,
    /// `operation_cancel` interrompe a leitura na hora, a conexão fecha e a operação termina
    /// como cancelada.
    #[tokio::test]
    async fn cancelar_a_operacao_interrompe_a_leitura_do_modpack() {
        let dir = tempfile::tempdir().unwrap();
        let mut state = test_state(dir.path());
        let pack_id = register_pack(&state, dir.path());
        let (file_url, closed) = stalled_file().await;
        let api = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v2/project/MODPACK1/version"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([{
                "id": "VERSAO01",
                "project_id": "MODPACK1",
                "version_number": "1.0.0",
                "version_type": "release",
                "game_versions": ["1.20.1"],
                "loaders": ["forge"],
                "files": [{
                    "hashes": { "sha1": "00", "sha512": "00" },
                    "url": file_url,
                    "filename": "parado.mrpack",
                    "primary": true,
                    "size": 1_000_000
                }],
                "dependencies": []
            }])))
            .mount(&api)
            .await;
        let http = HttpClient::new(HttpConfig::for_version("teste")).unwrap();
        state.modrinth =
            ModrinthClient::with_base_url(http, &format!("{}/v2", api.uri()), None).unwrap();
        let state = Arc::new(state);

        let started: Arc<Mutex<Option<OperationId>>> = Arc::default();
        let seen = Arc::clone(&started);
        let running = Arc::clone(&state);
        let task = tokio::spawn(async move {
            contents_operation(
                &running,
                pack_id,
                SourceId::Modrinth,
                "MODPACK1",
                move |event| {
                    if let OperationEvent::Started { operation_id, .. } = event {
                        *seen.lock().unwrap() = Some(operation_id);
                    }
                },
            )
            .await
        });
        let id = loop {
            if let Some(id) = *started.lock().unwrap() {
                break id;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        };
        let snapshot = state.operations.get(id).unwrap();
        assert_eq!(snapshot.kind, CONTENTS);
        assert!(snapshot.cancellable);
        // Espera a leitura chegar ao servidor parado e cancela.
        tokio::time::sleep(Duration::from_millis(300)).await;
        assert!(!task.is_finished(), "a leitura deveria estar esperando");
        let cancelled_at = Instant::now();
        state.operations.cancel(id).unwrap();
        let result = tokio::time::timeout(Duration::from_secs(5), task)
            .await
            .expect("o comando termina logo depois de cancelar")
            .unwrap();
        assert!(cancelled_at.elapsed() < Duration::from_secs(5));
        let error = result.unwrap_err();
        assert_eq!(
            error.code,
            ErrorCode::Core(warden_core::CoreErrorCode::Cancelled)
        );
        assert_eq!(
            state.operations.get(id).unwrap().state,
            OperationState::Cancelled
        );
        tokio::time::timeout(Duration::from_secs(5), closed)
            .await
            .expect("a conexão com o servidor do arquivo fechou")
            .unwrap();
    }
}
