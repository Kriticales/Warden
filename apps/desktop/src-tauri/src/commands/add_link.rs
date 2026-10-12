//! Links do Modrinth e links diretos colados no campo único de "Adicionar" (SPEC T08 "Link";
//! P1-11; CA-T08-07). O link da CurseForge é da P1-10 (`add_curseforge.rs`).
//!
//! - `add_link_modrinth`: o projeto (e a versão, no link de versão) pela API do Modrinth.
//! - `add_link_download`: o link direto `https://` baixado uma vez numa pasta temporária, com
//!   limite de tamanho e de tempo e cancelável (`operation_cancel` com o ID que chega pelo
//!   canal). O arquivo vira um token, como o arquivo do computador, e segue por
//!   `add_local_inspect` e `add_local_apply` (que grava a referência com o link e o `sha256`).
//!
//! **`http://` é recusado** antes de qualquer requisição (erro `INVALID_INPUT` com
//! `reason = insecure`).
#![allow(clippy::needless_pass_by_value)] // argumentos pertencem ao contrato do Tauri

use tauri::State;
use tauri::ipc::Channel;
use warden_core::PackId;
use warden_http::{HttpClient, HttpConfig};
use warden_project::ProjectErrorCode;
use warden_project::add::link::{
    DirectError, ModrinthLinkTarget, download_direct, resolve_modrinth_link,
};
use warden_project::add::url::{DirectLink, LinkClass, ModrinthLink, classify_link};

use crate::commands::add_local::{LocalFileRef, remember};
use crate::commands::inventory::pack_root;
use crate::error::AppError;
use crate::events::OperationEvent;
use crate::operations::OperationKind;
use crate::state::AppState;

/// "Ler o link do Modrinth".
const MODRINTH: OperationKind = OperationKind::new("add.modrinthLink");
/// "Baixar o link direto".
const DOWNLOAD: OperationKind = OperationKind::new("add.directLink");

/// Os E2E não têm `https`: em build de debug, com esta variável, o download do link direto vai
/// para o mesmo caminho neste servidor (a validação continua sendo a do link original).
#[cfg(debug_assertions)]
const E2E_DIRECT_VIA_ENV: &str = "WARDEN_E2E_DIRECT_VIA";

fn link_error(reason: &str) -> AppError {
    AppError::new(ProjectErrorCode::InvalidInput)
        .with_param("field", "link")
        .with_param("reason", reason)
}

/// O link, se for do tipo pedido; `http://` e o resto são recusados sem requisição.
fn classify<T>(url: &str, pick: impl FnOnce(LinkClass) -> Option<T>) -> Result<T, AppError> {
    match classify_link(url) {
        LinkClass::Insecure => Err(link_error("insecure")),
        LinkClass::NotLink | LinkClass::Unsupported => Err(link_error("unsupported")),
        other => pick(other).ok_or_else(|| link_error("unsupported")),
    }
}

fn modrinth_link(url: &str) -> Result<ModrinthLink, AppError> {
    classify(url, |class| match class {
        LinkClass::Modrinth(link) => Some(link),
        _ => None,
    })
}

fn direct_link(url: &str) -> Result<DirectLink, AppError> {
    classify(url, |class| match class {
        LinkClass::Direct(link) => Some(link),
        _ => None,
    })
}

/// Lê o link do Modrinth colado no campo único: o projeto (a tela abre a pré-visualização) ou a
/// versão exata (a tela abre o diálogo de dependências com ela).
#[tauri::command]
#[specta::specta]
pub(crate) async fn add_link_modrinth(
    state: State<'_, AppState>,
    pack_id: PackId,
    url: String,
) -> Result<ModrinthLinkTarget, AppError> {
    let link = modrinth_link(&url)?;
    pack_root(&state, pack_id)?;
    let operation = state.operations.start(MODRINTH, Some(pack_id), true);
    let result = resolve_modrinth_link(&state.modrinth, &link, operation.token())
        .await
        .map_err(|e| AppError::from_domain(&e));
    let result = if operation.is_cancelled() {
        Err(AppError::cancelled())
    } else {
        result
    };
    operation.finish(result)
}

fn via() -> Option<warden_http::Url> {
    #[cfg(debug_assertions)]
    {
        std::env::var(E2E_DIRECT_VIA_ENV)
            .ok()
            .filter(|base| !base.trim().is_empty())
            .and_then(|base| warden_http::Url::parse(&base).ok())
    }
    #[cfg(not(debug_assertions))]
    {
        None
    }
}

/// Baixa o link direto `https://` uma vez (até 256 MiB e 10 minutos; cancelável pelo ID que
/// chega no canal). O arquivo vira um token para `add_local_inspect` e `add_local_apply`. Nada é
/// gravado no pack; falhou ou foi cancelado, o download é apagado.
#[tauri::command]
#[specta::specta]
pub(crate) async fn add_link_download(
    state: State<'_, AppState>,
    pack_id: PackId,
    url: String,
    on_event: Channel<OperationEvent>,
) -> Result<LocalFileRef, AppError> {
    let link = direct_link(&url)?;
    pack_root(&state, pack_id)?;
    let operation = state
        .operations
        .start(DOWNLOAD, Some(pack_id), true)
        .with_events(move |event| {
            // A tela pode ter saído; o download continua até o fim ou o cancelamento.
            let _ = on_event.send(event);
        });
    let staging = state.paths.staging_dir(operation.id());
    let result = async {
        let http = HttpClient::new(HttpConfig::for_version(env!("CARGO_PKG_VERSION")))
            .map_err(|e| AppError::from_domain(&e))?;
        let via = via();
        let downloaded = download_direct(&http, &link, &staging, via.as_ref(), operation.token())
            .await
            .map_err(|error| match error {
                DirectError::Project(error) => AppError::from_domain(&error),
                DirectError::Http(error) => AppError::from_domain(&error),
            })?;
        tracing::info!(size = downloaded.size, "link direto baixado");
        remember(
            downloaded.path,
            Some(link.url().to_string()),
            Some(staging.clone()),
        )
        .ok_or_else(|| AppError::internal("nome do arquivo baixado inválido"))
    }
    .await;
    let result = if operation.is_cancelled() {
        Err(AppError::cancelled())
    } else {
        result
    };
    if result.is_err() {
        // `download_direct` já apaga a pasta; o token não chegou a existir.
        let _ = std::fs::remove_dir_all(&staging);
    }
    operation.finish(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reason(error: &AppError) -> Option<&str> {
        error.params.get("reason").map(String::as_str)
    }

    #[test]
    fn http_e_recusado_sem_requisicao() {
        let error = direct_link("http://example.com/mod.jar").unwrap_err();
        assert_eq!(reason(&error), Some("insecure"));
        let error = modrinth_link("http://modrinth.com/mod/sodium").unwrap_err();
        assert_eq!(reason(&error), Some("insecure"));
        let error = direct_link("https://example.com/pagina.html").unwrap_err();
        assert_eq!(reason(&error), Some("unsupported"));
        // Link do Modrinth não é link direto, e vice-versa.
        assert!(direct_link("https://modrinth.com/mod/sodium").is_err());
        assert!(modrinth_link("https://example.com/mod.jar").is_err());
        assert!(direct_link("https://example.com/mod.jar").is_ok());
        assert!(modrinth_link("https://modrinth.com/mod/sodium").is_ok());
    }
}
