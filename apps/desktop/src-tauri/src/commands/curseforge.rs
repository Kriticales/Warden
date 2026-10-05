//! Domínio `curseforge` no app (ARCHITECTURE §4.1 e §14; P1-04).
//!
//! Por enquanto só o testador da chave: o "Testar" de Configurações → Chaves e contas
//! (`secrets_test`) confere a chave da CurseForge na API (`GET /v1/games/432`). Chave recusada
//! → `curseforge.CURSEFORGE_KEY_INVALID` ("A CurseForge recusou a chave.", CA-T21-02). Os
//! comandos de busca e de arquivos chegam com as tarefas da busca combinada (P1-09/P1-10).

use secrecy::SecretString;
use warden_curseforge::CurseforgeClient;
use warden_http::{HttpClient, HttpConfig};

use crate::commands::secrets::SecretTester;
use crate::error::AppError;

/// Confere a chave da CurseForge na API.
#[derive(Debug, Default)]
pub(crate) struct CurseforgeKeyTester {
    /// Endereço da API (`None`: o oficial, ou o de `WARDEN_API_BASE_CURSEFORGE` em debug).
    base_url: Option<String>,
}

impl CurseforgeKeyTester {
    #[cfg(test)]
    fn with_base_url(base_url: &str) -> Self {
        Self {
            base_url: Some(base_url.to_owned()),
        }
    }

    fn client(&self, key: SecretString) -> Result<CurseforgeClient, AppError> {
        let http = HttpClient::new(HttpConfig::for_version(env!("CARGO_PKG_VERSION")))
            .map_err(|error| AppError::from_domain(&error))?;
        let client = match &self.base_url {
            Some(base) => CurseforgeClient::with_base_url(http, base, Some(key)),
            None => CurseforgeClient::new(http, Some(key)),
        };
        client.map_err(|error| AppError::from_domain(&error))
    }
}

#[async_trait::async_trait]
impl SecretTester for CurseforgeKeyTester {
    async fn test(&self, value: SecretString) -> Result<(), AppError> {
        self.client(value)?
            .check_key(None)
            .await
            .map_err(|error| AppError::from_domain(&error))
    }
}

#[cfg(test)]
mod tests {
    use std::io::{Read as _, Write as _};
    use std::net::TcpListener;

    use warden_curseforge::CurseforgeErrorCode;

    use super::*;
    use crate::error::ErrorCode;

    /// Servidor HTTP mínimo que responde `status` com `body` a um pedido e devolve o pedido
    /// recebido.
    fn one_shot(status: &str, body: &str) -> (String, std::thread::JoinHandle<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}/v1", listener.local_addr().unwrap());
        let response = format!(
            "HTTP/1.1 {status}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
            body.len()
        );
        let handle = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = Vec::new();
            let mut buffer = [0_u8; 4096];
            while !request.windows(4).any(|w| w == b"\r\n\r\n") {
                let read = stream.read(&mut buffer).unwrap();
                if read == 0 {
                    break;
                }
                request.extend_from_slice(&buffer[..read]);
            }
            stream.write_all(response.as_bytes()).unwrap();
            String::from_utf8_lossy(&request).into_owned()
        });
        (base, handle)
    }

    const KEY: &str = "$2a$10$chaveFalsaDoTesteDoApp000000000000000000000000";

    /// CA-T21-02 (backend): "Testar" com chave recusada devolve `CURSEFORGE_KEY_INVALID`.
    #[tokio::test]
    async fn ca_t21_02_chave_recusada() {
        let (base, server) = one_shot("403 Forbidden", "");
        let tester = CurseforgeKeyTester::with_base_url(&base);
        let error = tester
            .test(SecretString::from(KEY.to_owned()))
            .await
            .unwrap_err();
        assert_eq!(
            error.code,
            ErrorCode::Curseforge(CurseforgeErrorCode::KeyInvalid)
        );
        assert!(!error.detail.unwrap_or_default().contains(KEY));
        let request = server.join().unwrap();
        assert!(request.starts_with("GET /v1/games/432 "), "{request}");
        assert!(request.contains(&format!("x-api-key: {KEY}")), "{request}");
    }

    #[tokio::test]
    async fn chave_aceita() {
        let (base, server) = one_shot("200 OK", r#"{"data":{"id":432,"name":"Minecraft"}}"#);
        let tester = CurseforgeKeyTester::with_base_url(&base);
        tester
            .test(SecretString::from(KEY.to_owned()))
            .await
            .unwrap();
        server.join().unwrap();
    }

    #[tokio::test]
    async fn chave_vazia_e_a_mesma_que_sem_chave() {
        let tester = CurseforgeKeyTester::with_base_url("http://127.0.0.1:9/v1");
        let error = tester
            .test(SecretString::from(" ".to_owned()))
            .await
            .unwrap_err();
        assert_eq!(
            error.code,
            ErrorCode::Curseforge(CurseforgeErrorCode::KeyMissing)
        );
        assert!(format!("{:?}", CurseforgeKeyTester::default()).contains("base_url"));
    }
}
