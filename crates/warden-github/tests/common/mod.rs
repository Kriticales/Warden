//! Auxiliares dos testes contra o GitHub simulado (`wiremock`).

#![allow(dead_code, unreachable_pub)]

use std::io::Write;
use std::sync::{Arc, Mutex};

use warden_github::Endpoints;
use warden_http::{HttpClient, HttpConfig, ManualTimer};
use wiremock::MockServer;

/// Tokens falsos nos formatos do GitHub, montados em pedaços.
pub fn access_token(n: u32) -> String {
    format!("ghu_{}{n:04}", "AcessoFalsoDoTeste0123456789abcdef")
}
pub fn refresh_token(n: u32) -> String {
    format!(
        "ghr_{}{n:04}",
        "RenovacaoFalsaDoTeste0123456789abcdefghijklmnopqrstuvwxyz"
    )
}
pub const DEVICE_CODE: &str =
    "codigo-do-dispositivo-secreto-3584d83530557fdd1f46af8289938c8ef79f9dc5";
pub const CLIENT_ID: &str = "Iv1.cliente-de-teste";

pub fn client() -> (HttpClient, ManualTimer) {
    let timer = ManualTimer::new();
    let client =
        HttpClient::with_timer(HttpConfig::for_version("9.9.9"), Arc::new(timer.clone())).unwrap();
    (client, timer)
}

/// API em `<servidor>/api/` e site em `<servidor>/site/`.
pub fn endpoints(server: &MockServer) -> Endpoints {
    Endpoints::custom(
        &format!("{}/api/", server.uri()),
        &format!("{}/site/", server.uri()),
        CLIENT_ID,
    )
    .unwrap()
}

/// Registros capturados em memória (todos os níveis), enquanto o guarda viver.
#[derive(Clone, Default)]
pub struct Captured(Arc<Mutex<Vec<u8>>>);

impl Captured {
    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.0.lock().unwrap()).into_owned()
    }
}

impl Write for Captured {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

pub fn capture_logs() -> (Captured, tracing::subscriber::DefaultGuard) {
    let captured = Captured::default();
    let writer = captured.clone();
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::TRACE)
        .with_ansi(false)
        .with_writer(move || writer.clone())
        .finish();
    let guard = tracing::subscriber::set_default(subscriber);
    (captured, guard)
}
