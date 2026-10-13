//! Cache do packwiz-installer-bootstrap e do packwiz-installer ("Testar como o jogador
//! recebe", ROADMAP L-07; ARCHITECTURE §8.5 passo 1).
//!
//! Os dois jars ficam em `cache/installer/` com versão e SHA-256 fixados aqui. Faltando (ou
//! com outro hash), são baixados da URL de download do Release do GitHub, que **não** passa
//! pela API (a API sem login tem limite de 60 pedidos por hora e o bootstrap, sem plano B,
//! falha com 403; pesquisa R6 §4.3). O teste sempre roda o bootstrap com
//! `--bootstrap-no-update`, apontando o installer daqui.
//!
//! As versões e os hashes são os mesmos da `cargo xtask installer` (`xtask/src/installer.rs`),
//! que prepara os testes de conformidade da L-03; um teste confere que as duas listas não se
//! separam.

use std::fs;
use std::io::Read as _;
use std::path::{Path, PathBuf};

use sha2::{Digest as _, Sha256};
use warden_core::{CancellationToken, ProgressSink};
use warden_http::{DownloadRequest, ExpectedHash, HttpClient, Url};
use warden_packwiz::HashFormat;

use crate::error::{Error, Result};

/// Um jar fixado: endereço do Release, nome no cache e SHA-256.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PinnedJar {
    /// Endereço de download do Release (`https://github.com/…/releases/download/…`).
    pub url: &'static str,
    /// Nome do arquivo no cache (com a versão).
    pub file_name: &'static str,
    /// SHA-256 em hexadecimal minúsculo.
    pub sha256: &'static str,
    /// Tamanho máximo aceito no download.
    pub max_size: u64,
}

/// packwiz-installer-bootstrap v0.0.3 (12/07/2020, a última).
pub const BOOTSTRAP: PinnedJar = PinnedJar {
    url: "https://github.com/packwiz/packwiz-installer-bootstrap/releases/download/v0.0.3/packwiz-installer-bootstrap.jar",
    file_name: "packwiz-installer-bootstrap-0.0.3.jar",
    sha256: "a8fbb24dc604278e97f4688e82d3d91a318b98efc08d5dbfcbcbcab6443d116c",
    max_size: 1024 * 1024,
};

/// packwiz-installer v0.5.14 (21/04/2024, a última).
pub const INSTALLER: PinnedJar = PinnedJar {
    url: "https://github.com/packwiz/packwiz-installer/releases/download/v0.5.14/packwiz-installer.jar",
    file_name: "packwiz-installer-0.5.14.jar",
    sha256: "c9f646908d340d84773948a9a7d98bc1dae250d35e1016dc6e2b8459760b5598",
    max_size: 16 * 1024 * 1024,
};

/// Etapa "Baixando o instalador do jogador".
pub const STAGE_INSTALLER: &str = "test.player.installer";
/// Texto da etapa.
pub const STAGE_INSTALLER_LABEL: &str = "jogador:etapa.instalador";

/// Os dois jars, prontos no cache.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallerJars {
    /// `packwiz-installer-bootstrap.jar`.
    pub bootstrap: PathBuf,
    /// `packwiz-installer.jar` (passado com `--bootstrap-main-jar`).
    pub installer: PathBuf,
}

/// `cache/installer/`.
#[derive(Debug, Clone)]
pub struct InstallerCache {
    dir: PathBuf,
    /// Troca o começo do endereço de download (servidor local dos testes e do E2E): o jar
    /// vem de `<base>/<file_name>`. `None` no app.
    base: Option<Url>,
}

impl InstallerCache {
    /// O cache na pasta dada, baixando dos Releases do GitHub.
    #[must_use]
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self {
            dir: dir.into(),
            base: None,
        }
    }

    /// O mesmo, baixando de `<base>/<nome do arquivo>` (testes).
    #[must_use]
    pub fn with_base(mut self, base: Option<Url>) -> Self {
        self.base = base;
        self
    }

    /// A pasta do cache.
    #[must_use]
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Garante os dois jars com o hash certo, baixando o que faltar.
    ///
    /// Erros: [`Error::InstallerUnavailable`] (download ou hash), os de disco e
    /// [`Error::Cancelled`].
    pub async fn ensure(
        &self,
        http: &HttpClient,
        progress: &dyn ProgressSink,
        cancel: &CancellationToken,
    ) -> Result<InstallerJars> {
        Ok(InstallerJars {
            bootstrap: self.ensure_one(http, &BOOTSTRAP, progress, cancel).await?,
            installer: self.ensure_one(http, &INSTALLER, progress, cancel).await?,
        })
    }

    async fn ensure_one(
        &self,
        http: &HttpClient,
        jar: &PinnedJar,
        progress: &dyn ProgressSink,
        cancel: &CancellationToken,
    ) -> Result<PathBuf> {
        let path = self.dir.join(jar.file_name);
        if reuse_or_clear(&path, jar.sha256)? {
            return Ok(path);
        }
        if cancel.is_cancelled() {
            return Err(Error::Cancelled);
        }
        progress.stage(STAGE_INSTALLER, STAGE_INSTALLER_LABEL);
        let url = self.url_of(jar)?;
        let mut request = DownloadRequest::new(url, &path);
        request.expected_hashes = vec![ExpectedHash::new(HashFormat::Sha256, jar.sha256)];
        request.max_size = Some(jar.max_size);
        http.download(&request, progress, Some(cancel))
            .await
            .map_err(|source| {
                if cancel.is_cancelled() {
                    Error::Cancelled
                } else {
                    Error::InstallerUnavailable {
                        file: jar.file_name.to_owned(),
                        source: Box::new(source),
                    }
                }
            })?;
        tracing::info!(file = jar.file_name, "instalador do jogador baixado");
        Ok(path)
    }

    fn url_of(&self, jar: &PinnedJar) -> Result<Url> {
        let text = match &self.base {
            Some(base) => format!("{}/{}", base.as_str().trim_end_matches('/'), jar.file_name),
            None => jar.url.to_owned(),
        };
        Url::parse(&text).map_err(|error| Error::Internal(format!("endereço do jar: {error}")))
    }
}

/// O caminho como aparece nas mensagens: relativo à pasta de dados, sem a pasta do usuário.
fn shown(path: &Path) -> PathBuf {
    Path::new("cache/installer").join(path.file_name().unwrap_or_default())
}

/// SHA-256 de um arquivo, em hexadecimal minúsculo.
fn sha256_file(path: &Path) -> Result<String> {
    let mut file = fs::File::open(path).map_err(|e| Error::io("abrir", shown(path), e))?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0_u8; 64 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|e| Error::io("ler", shown(path), e))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hex::encode(hasher.finalize()))
}

/// Diz se `path` é o jar com o SHA-256 dado. Outra coisa no lugar (hash errado, pasta) é
/// apagada para o download ocupar o caminho.
fn reuse_or_clear(path: &Path, sha256: &str) -> Result<bool> {
    if path.is_file() {
        if sha256_file(path)? == sha256 {
            return Ok(true);
        }
        tracing::warn!(file = %path.file_name().unwrap_or_default().to_string_lossy(), "jar do instalador com hash errado; baixando de novo");
        fs::remove_file(path).map_err(|e| Error::io("apagar", shown(path), e))?;
    } else if path.is_dir() {
        fs::remove_dir_all(path).map_err(|e| Error::io("apagar", shown(path), e))?;
    }
    Ok(false)
}

#[cfg(test)]
mod tests {
    use warden_core::NoProgress;
    use warden_http::HttpConfig;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;
    use crate::test_support::temp_dir;

    #[test]
    fn fixados_sao_releases_sem_api_e_com_sha256() {
        for jar in [BOOTSTRAP, INSTALLER] {
            assert!(
                jar.url.starts_with("https://github.com/packwiz/"),
                "{}",
                jar.url
            );
            assert!(jar.url.contains("/releases/download/"), "{}", jar.url);
            assert!(!jar.url.contains("api.github.com"));
            assert_eq!(jar.sha256.len(), 64);
            assert!(
                jar.sha256
                    .chars()
                    .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
            );
        }
    }

    /// As versões e os hashes são os mesmos da `cargo xtask installer` (L-03): os testes de
    /// conformidade e o app usam os mesmos jars.
    #[test]
    fn mesmos_jars_da_xtask() {
        let xtask = include_str!("../../../xtask/src/installer.rs");
        for jar in [BOOTSTRAP, INSTALLER] {
            assert!(xtask.contains(jar.url), "{} fora da xtask", jar.url);
            assert!(xtask.contains(jar.file_name), "{}", jar.file_name);
            assert!(xtask.contains(jar.sha256), "{}", jar.sha256);
        }
    }

    fn http() -> HttpClient {
        HttpClient::new(HttpConfig::default()).unwrap()
    }

    async fn serve(server: &MockServer, jar: &PinnedJar, body: &[u8]) {
        Mock::given(method("GET"))
            .and(path(format!("/jars/{}", jar.file_name)))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(body.to_vec()))
            .mount(server)
            .await;
    }

    #[tokio::test]
    async fn jar_com_hash_errado_e_recusado_e_nada_fica_no_cache() {
        let server = MockServer::start().await;
        serve(&server, &BOOTSTRAP, b"nao e o bootstrap").await;
        let dir = temp_dir();
        let cache = InstallerCache::new(dir.path()).with_base(Some(
            Url::parse(&format!("{}/jars/", server.uri())).unwrap(),
        ));
        let error = cache
            .ensure(&http(), &NoProgress, &CancellationToken::new())
            .await
            .unwrap_err();
        assert!(
            matches!(&error, Error::InstallerUnavailable { file, .. } if file == BOOTSTRAP.file_name),
            "{error:?}"
        );
        assert!(!dir.path().join(BOOTSTRAP.file_name).exists());
    }

    #[tokio::test]
    async fn jar_certo_ja_no_cache_nao_e_baixado() {
        let server = MockServer::start().await;
        let dir = temp_dir();
        // Conteúdos de mentira com os hashes "fixados" substituídos: só a reutilização é
        // testada aqui (o download real é provado pelo teste com o bootstrap real).
        fs::write(dir.path().join("a.jar"), b"conteudo").unwrap();
        let sha = sha256_file(&dir.path().join("a.jar")).unwrap();
        assert!(reuse_or_clear(&dir.path().join("a.jar"), &sha).unwrap());
        assert!(!reuse_or_clear(&dir.path().join("a.jar"), &"0".repeat(64)).unwrap());
        assert!(!dir.path().join("a.jar").exists());
        fs::create_dir_all(dir.path().join("b.jar").join("sobra")).unwrap();
        assert!(!reuse_or_clear(&dir.path().join("b.jar"), &sha).unwrap());
        assert!(!dir.path().join("b.jar").exists());
        assert!(server.received_requests().await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn cancelado_antes_do_download() {
        let dir = temp_dir();
        let cancel = CancellationToken::new();
        cancel.cancel();
        let error = InstallerCache::new(dir.path())
            .ensure(&http(), &NoProgress, &cancel)
            .await
            .unwrap_err();
        assert!(matches!(error, Error::Cancelled), "{error:?}");
    }

    #[test]
    fn endereco_do_release_ou_da_base() {
        let cache = InstallerCache::new("x");
        assert_eq!(cache.url_of(&INSTALLER).unwrap().as_str(), INSTALLER.url);
        let local = cache.with_base(Some(Url::parse("http://127.0.0.1:9/jars").unwrap()));
        assert_eq!(
            local.url_of(&INSTALLER).unwrap().as_str(),
            "http://127.0.0.1:9/jars/packwiz-installer-0.5.14.jar"
        );
    }
}
