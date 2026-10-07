//! Imagens da CurseForge pelo protocolo `warden-img://` (SPEC T08; ARCHITECTURE §17.1).
//!
//! Os termos da CurseForge não deixam o app guardar dados da API em disco, e o cache do WebView
//! guarda tudo o que ele baixa. Por isso as imagens da CurseForge não vão direto para o
//! WebView: o Rust as baixa, mantém **só em memória** e as entrega com
//! `Cache-Control: no-store`. As do Modrinth continuam diretas (`img-src https:`).
//!
//! O endereço que a interface usa é `warden-img://localhost/<token>`, onde o token é o link
//! original em base64 seguro para URL. Antes de baixar, o link passa pela lista de servidores
//! permitidos (o proxy não é um navegador aberto), tempo-limite, tamanho máximo de
//! [`MAX_IMAGE_BYTES`] e só tipos de imagem.

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex, PoisonError};

use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use url::Url;
use warden_core::CancellationToken;
use warden_http::HttpClient;

/// Nome do protocolo registrado no Tauri.
pub const SCHEME: &str = "warden-img";

/// Maior imagem aceita (5 MB).
pub const MAX_IMAGE_BYTES: u64 = 5 * 1024 * 1024;

/// Quanto da memória o cache de imagens pode ocupar (48 MB).
const CACHE_BYTES: usize = 48 * 1024 * 1024;

/// Servidores que o proxy aceita (o próprio e qualquer subdomínio): as imagens da CurseForge.
const ALLOWED_HOSTS: [&str; 2] = ["forgecdn.net", "curseforge.com"];

/// Tipos de imagem entregues (SVG fica de fora de propósito).
const IMAGE_TYPES: [&str; 5] = [
    "image/png",
    "image/jpeg",
    "image/gif",
    "image/webp",
    "image/avif",
];

/// Se o texto é um link `https:`.
#[must_use]
pub fn is_https(url: &str) -> bool {
    Url::parse(url).is_ok_and(|url| url.scheme() == "https" && url.host_str().is_some())
}

/// O endereço `warden-img://` de uma imagem remota. `None` se o link não for `https:`.
#[must_use]
pub fn proxy_url(original: &str) -> Option<String> {
    is_https(original).then(|| format!("{SCHEME}://localhost/{}", URL_SAFE_NO_PAD.encode(original)))
}

/// O link original de um token do proxy.
fn decode(token: &str) -> Option<Url> {
    let bytes = URL_SAFE_NO_PAD.decode(token.trim_matches('/')).ok()?;
    Url::parse(std::str::from_utf8(&bytes).ok()?).ok()
}

/// Por que uma imagem não foi entregue.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ImageError {
    /// Token ilegível, link sem `https:` ou servidor fora da lista.
    #[error("endereço de imagem não permitido")]
    Forbidden,
    /// Maior que [`MAX_IMAGE_BYTES`].
    #[error("imagem grande demais")]
    TooLarge,
    /// A resposta não é uma imagem de um dos tipos aceitos.
    #[error("a resposta não é uma imagem aceita")]
    NotAnImage,
    /// O servidor não respondeu ou deu erro.
    #[error("imagem indisponível")]
    Unavailable,
}

/// Uma imagem pronta para entregar ao WebView.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Image {
    /// Tipo (`image/png`…).
    pub content_type: &'static str,
    /// Os bytes.
    pub bytes: Arc<[u8]>,
}

/// Cache só em memória, do mais antigo para o mais novo, com teto de bytes.
#[derive(Debug, Default)]
struct Memory {
    images: HashMap<String, Image>,
    order: VecDeque<String>,
    bytes: usize,
}

impl Memory {
    fn get(&self, key: &str) -> Option<Image> {
        self.images.get(key).cloned()
    }

    fn put(&mut self, key: String, image: Image) {
        let size = image.bytes.len();
        if size > CACHE_BYTES {
            return;
        }
        if let Some(old) = self.images.remove(&key) {
            self.bytes -= old.bytes.len();
            self.order.retain(|known| *known != key);
        }
        while self.bytes + size > CACHE_BYTES {
            let Some(oldest) = self.order.pop_front() else {
                break;
            };
            if let Some(old) = self.images.remove(&oldest) {
                self.bytes -= old.bytes.len();
            }
        }
        self.bytes += size;
        self.order.push_back(key.clone());
        self.images.insert(key, image);
    }
}

/// O proxy de imagens: baixa, confere e guarda na memória.
#[derive(Clone)]
pub struct ImageProxy {
    http: HttpClient,
    memory: Arc<Mutex<Memory>>,
    hosts: Arc<Vec<String>>,
    https_only: bool,
}

impl std::fmt::Debug for ImageProxy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ImageProxy").finish_non_exhaustive()
    }
}

impl ImageProxy {
    /// O proxy com a lista de servidores da CurseForge.
    #[must_use]
    pub fn new(http: HttpClient) -> Self {
        Self::with_policy(http, ALLOWED_HOSTS.map(str::to_owned).to_vec(), true)
    }

    /// Com outra lista de servidores e, nos testes, aceitando `http:`.
    #[must_use]
    pub(crate) fn with_policy(http: HttpClient, hosts: Vec<String>, https_only: bool) -> Self {
        Self {
            http,
            memory: Arc::new(Mutex::new(Memory::default())),
            hosts: Arc::new(hosts),
            https_only,
        }
    }

    fn host_allowed(&self, url: &Url) -> bool {
        let Some(host) = url.host_str().map(str::to_ascii_lowercase) else {
            return false;
        };
        (!self.https_only || url.scheme() == "https")
            && self
                .hosts
                .iter()
                .any(|allowed| host == *allowed || host.ends_with(&format!(".{allowed}")))
    }

    /// A imagem do token (o caminho do endereço `warden-img://`). Repetir o pedido usa a
    /// memória.
    pub async fn fetch(
        &self,
        token: &str,
        cancel: &CancellationToken,
    ) -> Result<Image, ImageError> {
        let url = decode(token).ok_or(ImageError::Forbidden)?;
        if !self.host_allowed(&url) {
            return Err(ImageError::Forbidden);
        }
        let key = url.as_str().to_owned();
        if let Some(image) = self
            .memory
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(&key)
        {
            return Ok(image);
        }
        let response = self
            .http
            .get(url)
            .max_body(MAX_IMAGE_BYTES)
            .cancel(cancel)
            .send()
            .await
            .map_err(|error| {
                tracing::debug!(%error, "imagem da CurseForge indisponível");
                ImageError::Unavailable
            })?;
        // Um redirecionamento não pode levar para fora da lista.
        if !self.host_allowed(response.final_url()) {
            return Err(ImageError::Forbidden);
        }
        let content_type = response
            .headers()
            .get(reqwest_content_type())
            .and_then(|value| value.to_str().ok())
            .map(|value| {
                value
                    .split(';')
                    .next()
                    .unwrap_or_default()
                    .trim()
                    .to_ascii_lowercase()
            })
            .and_then(|value| IMAGE_TYPES.into_iter().find(|known| *known == value))
            .ok_or(ImageError::NotAnImage)?;
        let bytes = response
            .bytes_limited(MAX_IMAGE_BYTES)
            .await
            .map_err(|error| match error {
                warden_http::Error::ResponseTooLarge { .. } => ImageError::TooLarge,
                other => {
                    tracing::debug!(error = %other, "imagem da CurseForge interrompida");
                    ImageError::Unavailable
                }
            })?;
        let image = Image {
            content_type,
            bytes: bytes.into(),
        };
        self.memory
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .put(key, image.clone());
        Ok(image)
    }
}

fn reqwest_content_type() -> &'static str {
    "content-type"
}

#[cfg(test)]
mod tests {
    use warden_http::HttpConfig;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;

    const PNG: &[u8] = b"\x89PNG\r\n\x1a\nfake";

    fn proxy() -> ImageProxy {
        let http = HttpClient::new(HttpConfig::for_version("0.0.0-teste")).unwrap();
        ImageProxy::with_policy(http, vec!["127.0.0.1".to_owned()], false)
    }

    fn token(server: &MockServer, file: &str) -> String {
        let url = format!("{}/{file}", server.uri());
        URL_SAFE_NO_PAD.encode(url)
    }

    #[test]
    fn endereco_do_proxy_so_para_https() {
        let url = proxy_url("https://media.forgecdn.net/a.png").unwrap();
        assert!(url.starts_with("warden-img://localhost/"));
        let token = url.rsplit('/').next().unwrap();
        assert_eq!(
            decode(token).unwrap().as_str(),
            "https://media.forgecdn.net/a.png"
        );
        assert!(proxy_url("http://media.forgecdn.net/a.png").is_none());
        assert!(proxy_url("javascript:alert(1)").is_none());
    }

    #[test]
    fn so_servidores_da_curseforge() {
        let http = HttpClient::new(HttpConfig::for_version("0.0.0-teste")).unwrap();
        let proxy = ImageProxy::new(http);
        let allowed = |text: &str| proxy.host_allowed(&Url::parse(text).unwrap());
        assert!(allowed("https://media.forgecdn.net/a.png"));
        assert!(allowed("https://forgecdn.net/a.png"));
        assert!(allowed("https://media.curseforge.com/a.png"));
        assert!(!allowed("http://media.forgecdn.net/a.png"));
        assert!(!allowed("https://evil-forgecdn.net/a.png"));
        assert!(!allowed("https://forgecdn.net.evil.com/a.png"));
        assert!(!allowed("https://example.com/a.png"));
    }

    #[tokio::test]
    async fn entrega_a_imagem_e_repete_pela_memoria() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/a.png"))
            .respond_with(ResponseTemplate::new(200).set_body_raw(PNG, "image/png"))
            .expect(1)
            .mount(&server)
            .await;
        let proxy = proxy();
        let cancel = CancellationToken::new();
        let first = proxy
            .fetch(&token(&server, "a.png"), &cancel)
            .await
            .unwrap();
        let second = proxy
            .fetch(&token(&server, "a.png"), &cancel)
            .await
            .unwrap();
        assert_eq!(first.content_type, "image/png");
        assert_eq!(&*first.bytes, PNG);
        assert_eq!(first, second);
        // `expect(1)` confere ao encerrar o servidor: a segunda veio da memória.
    }

    #[tokio::test]
    async fn recusa_o_que_nao_e_imagem_o_grande_e_o_de_fora() {
        let server = MockServer::start().await;
        Mock::given(path("/page.html"))
            .respond_with(ResponseTemplate::new(200).set_body_raw("<script>", "text/html"))
            .mount(&server)
            .await;
        Mock::given(path("/drawing.svg"))
            .respond_with(ResponseTemplate::new(200).set_body_raw("<svg/>", "image/svg+xml"))
            .mount(&server)
            .await;
        let huge = vec![0u8; usize::try_from(MAX_IMAGE_BYTES).unwrap() + 1];
        Mock::given(path("/huge.png"))
            .respond_with(ResponseTemplate::new(200).set_body_raw(huge, "image/png"))
            .mount(&server)
            .await;
        Mock::given(path("/gone.png"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;
        let proxy = proxy();
        let cancel = CancellationToken::new();
        let fetch = async |file: &str| proxy.fetch(&token(&server, file), &cancel).await;
        assert_eq!(
            fetch("page.html").await.unwrap_err(),
            ImageError::NotAnImage
        );
        assert_eq!(
            fetch("drawing.svg").await.unwrap_err(),
            ImageError::NotAnImage
        );
        assert_eq!(fetch("huge.png").await.unwrap_err(), ImageError::TooLarge);
        assert_eq!(
            fetch("gone.png").await.unwrap_err(),
            ImageError::Unavailable
        );
        let outside = URL_SAFE_NO_PAD.encode("http://example.com/a.png");
        assert_eq!(
            proxy.fetch(&outside, &cancel).await.unwrap_err(),
            ImageError::Forbidden
        );
        assert_eq!(
            proxy.fetch("não é base64!", &cancel).await.unwrap_err(),
            ImageError::Forbidden
        );
    }

    #[test]
    fn memoria_respeita_o_teto_e_descarta_o_mais_antigo() {
        let mut memory = Memory::default();
        let big = |n: u8| Image {
            content_type: "image/png",
            bytes: vec![n; CACHE_BYTES / 2 + 1].into(),
        };
        memory.put("a".into(), big(1));
        memory.put("b".into(), big(2));
        assert!(memory.get("a").is_none(), "a deveria ter saído");
        assert!(memory.get("b").is_some());
        assert!(memory.bytes <= CACHE_BYTES);
    }
}
