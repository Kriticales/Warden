//! Protocolo `warden-img://` (SPEC T08; ARCHITECTURE §17.1; P1-16).
//!
//! Entrega ao WebView as imagens da CurseForge baixadas pelo Rust e guardadas **só em
//! memória**, com `Cache-Control: no-store`, para que o cache de disco do WebView não guarde
//! dados da API (termos da CurseForge). O download, a lista de servidores permitidos, o tipo e
//! o tamanho máximo são da `warden-discovery::images`; aqui só está a ligação com o Tauri.

use std::borrow::Cow;
use std::sync::OnceLock;

use tauri::http::{Request, Response, StatusCode, header};
use warden_core::CancellationToken;
use warden_discovery::images::{ImageError, ImageProxy, SCHEME};
use warden_http::{HttpClient, HttpConfig};

/// O proxy do processo (um cache de memória só para todas as janelas).
fn proxy() -> Option<&'static ImageProxy> {
    static PROXY: OnceLock<Option<ImageProxy>> = OnceLock::new();
    PROXY
        .get_or_init(|| {
            HttpClient::new(HttpConfig::for_version(env!("CARGO_PKG_VERSION")))
                .map(ImageProxy::new)
                .map_err(|error| tracing::warn!(%error, "imagens da CurseForge desligadas"))
                .ok()
        })
        .as_ref()
}

/// O código HTTP de cada motivo de recusa.
fn status_of(error: ImageError) -> StatusCode {
    match error {
        ImageError::Forbidden => StatusCode::FORBIDDEN,
        ImageError::TooLarge => StatusCode::PAYLOAD_TOO_LARGE,
        ImageError::NotAnImage => StatusCode::UNSUPPORTED_MEDIA_TYPE,
        ImageError::Unavailable => StatusCode::BAD_GATEWAY,
    }
}

fn build(
    status: StatusCode,
    content_type: &str,
    body: Cow<'static, [u8]>,
) -> Response<Cow<'static, [u8]>> {
    Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, content_type)
        // Nada disto fica no cache de disco do WebView.
        .header(header::CACHE_CONTROL, "no-store")
        .header("X-Content-Type-Options", "nosniff")
        .body(body)
        .unwrap_or_else(|_| Response::new(Cow::Borrowed(&[][..])))
}

async fn respond(request: Request<Vec<u8>>) -> Response<Cow<'static, [u8]>> {
    let Some(proxy) = proxy() else {
        return build(StatusCode::BAD_GATEWAY, "text/plain", Cow::Borrowed(b""));
    };
    let token = request.uri().path().trim_start_matches('/');
    match proxy.fetch(token, &CancellationToken::new()).await {
        Ok(image) => build(
            StatusCode::OK,
            image.content_type,
            Cow::Owned(image.bytes.to_vec()),
        ),
        Err(error) => build(status_of(error), "text/plain", Cow::Borrowed(b"")),
    }
}

/// Registra o protocolo no builder do app.
pub(crate) fn register<R: tauri::Runtime>(builder: tauri::Builder<R>) -> tauri::Builder<R> {
    builder.register_asynchronous_uri_scheme_protocol(SCHEME, |_context, request, responder| {
        tauri::async_runtime::spawn(async move {
            responder.respond(respond(request).await);
        });
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cada_recusa_tem_o_seu_codigo() {
        assert_eq!(status_of(ImageError::Forbidden), StatusCode::FORBIDDEN);
        assert_eq!(
            status_of(ImageError::TooLarge),
            StatusCode::PAYLOAD_TOO_LARGE
        );
        assert_eq!(
            status_of(ImageError::NotAnImage),
            StatusCode::UNSUPPORTED_MEDIA_TYPE
        );
        assert_eq!(status_of(ImageError::Unavailable), StatusCode::BAD_GATEWAY);
    }

    #[test]
    fn toda_resposta_vai_sem_cache() {
        for status in [StatusCode::OK, StatusCode::FORBIDDEN] {
            let response = build(status, "image/png", Cow::Borrowed(b"x"));
            assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
            assert_eq!(response.headers()["X-Content-Type-Options"], "nosniff");
        }
    }

    #[tokio::test]
    async fn token_invalido_vira_403_sem_cache() {
        let request = Request::builder()
            .uri("warden-img://localhost/n%C3%A3o-%C3%A9-token")
            .body(Vec::new())
            .unwrap();
        let response = respond(request).await;
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
    }
}
