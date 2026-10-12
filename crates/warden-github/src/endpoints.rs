//! Endereços do GitHub, o repositório do Warden e o *client ID* do GitHub App (ADR-0059).
//!
//! Tudo aqui é constante: o envio do relato não recebe destino de fora. Só em build de debug
//! os testes trocam os endereços e o *client ID* por variáveis de ambiente, para falar com o
//! servidor simulado (ARCHITECTURE §17).

use url::Url;

use crate::error::{Error, Result};

/// *Client ID* do GitHub App do Warden. É público no *device flow* (o app instalado não tem
/// *client secret*). **Vazio** enquanto o dono não criar o GitHub App: com ele vazio, "Entrar
/// com o GitHub" fica indisponível e o relato usa "Abrir no GitHub" (UI-02).
pub const CLIENT_ID: &str = "";

/// Dono do repositório do Warden.
pub const REPOSITORY_OWNER: &str = "Kriticales";

/// Nome do repositório do Warden.
pub const REPOSITORY_NAME: &str = "Warden";

/// Endereço da API REST.
pub const API_BASE_URL: &str = "https://api.github.com/";

/// Endereço do site (login por dispositivo e páginas das issues e dos prints).
pub const LOGIN_BASE_URL: &str = "https://github.com/";

/// Branch que guarda os prints dos relatos (órfã: nunca se mistura com o `main`).
pub const ATTACHMENTS_BRANCH: &str = "relatos-anexos";

/// Pasta dos prints dentro da branch.
pub const ATTACHMENTS_DIR: &str = "anexos";

/// Troca o endereço da API (só em build de debug).
pub const API_BASE_ENV: &str = "WARDEN_API_BASE_GITHUB";

/// Troca o endereço do site (só em build de debug).
pub const LOGIN_BASE_ENV: &str = "WARDEN_LOGIN_BASE_GITHUB";

/// Troca o *client ID* (só em build de debug).
pub const CLIENT_ID_ENV: &str = "WARDEN_GITHUB_CLIENT_ID";

/// Os endereços e o *client ID* em uso.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Endpoints {
    api: Url,
    site: Url,
    client_id: String,
}

impl Endpoints {
    /// Os oficiais.
    pub fn official() -> Result<Self> {
        Self::custom(API_BASE_URL, LOGIN_BASE_URL, CLIENT_ID)
    }

    /// Os oficiais ou, em build de debug, os das variáveis de ambiente que estiverem definidas.
    pub fn from_env() -> Result<Self> {
        #[cfg(debug_assertions)]
        {
            let read = |name: &str| std::env::var(name).ok().filter(|value| !value.is_empty());
            let api = read(API_BASE_ENV).unwrap_or_else(|| API_BASE_URL.to_owned());
            let site = read(LOGIN_BASE_ENV).unwrap_or_else(|| LOGIN_BASE_URL.to_owned());
            let client_id = read(CLIENT_ID_ENV).unwrap_or_else(|| CLIENT_ID.to_owned());
            Self::custom(&api, &site, &client_id)
        }
        #[cfg(not(debug_assertions))]
        Self::official()
    }

    /// Outros endereços (testes). O repositório continua o do Warden.
    pub fn custom(api: &str, site: &str, client_id: &str) -> Result<Self> {
        Ok(Self {
            api: base(api)?,
            site: base(site)?,
            client_id: client_id.trim().to_owned(),
        })
    }

    /// Se o login pode ser feito (há *client ID*).
    #[must_use]
    pub fn login_available(&self) -> bool {
        !self.client_id.is_empty()
    }

    /// O *client ID* em uso (público).
    #[must_use]
    pub fn client_id(&self) -> &str {
        &self.client_id
    }

    /// Página em que a pessoa digita o código (`https://github.com/login/device`).
    pub fn device_page(&self) -> Result<Url> {
        join(&self.site, &["login", "device"])
    }

    /// `POST` que pede o código do dispositivo.
    pub(crate) fn device_code_url(&self) -> Result<Url> {
        join(&self.site, &["login", "device", "code"])
    }

    /// `POST` que troca o código (ou o *refresh token*) pelo token.
    pub(crate) fn access_token_url(&self) -> Result<Url> {
        join(&self.site, &["login", "oauth", "access_token"])
    }

    /// `GET /user`.
    pub(crate) fn user_url(&self) -> Result<Url> {
        join(&self.api, &["user"])
    }

    /// Endereço dentro de `/repos/Kriticales/Warden/` (cada pedaço é codificado).
    pub(crate) fn repo_url(&self, segments: &[&str]) -> Result<Url> {
        let mut all = vec!["repos", REPOSITORY_OWNER, REPOSITORY_NAME];
        all.extend_from_slice(segments);
        join(&self.api, &all)
    }

    /// Página da issue `number` do Warden.
    pub fn issue_page(&self, number: u64) -> Result<Url> {
        join(
            &self.site,
            &[
                REPOSITORY_OWNER,
                REPOSITORY_NAME,
                "issues",
                &number.to_string(),
            ],
        )
    }

    /// Endereço que mostra um arquivo da branch dos prints. Funciona com o repositório público
    /// ou privado (quem tem acesso vê); o `download_url` da API nunca é usado, porque no
    /// repositório privado ele leva um token temporário.
    pub fn attachment_page(&self, path: &str) -> Result<Url> {
        let mut segments = vec![
            REPOSITORY_OWNER,
            REPOSITORY_NAME,
            "blob",
            ATTACHMENTS_BRANCH,
        ];
        segments.extend(path.split('/'));
        let mut url = join(&self.site, &segments)?;
        url.set_query(Some("raw=true"));
        Ok(url)
    }

    /// Se `url` é uma página de issue do Warden (`…/Kriticales/Warden/issues/<número>`).
    #[must_use]
    pub fn is_issue_page(&self, url: &str) -> bool {
        let Ok(url) = Url::parse(url) else {
            return false;
        };
        let Some(number) = url
            .as_str()
            .strip_prefix(self.site.as_str())
            .and_then(|rest| {
                rest.strip_prefix(&format!("{REPOSITORY_OWNER}/{REPOSITORY_NAME}/issues/"))
            })
        else {
            return false;
        };
        !number.is_empty() && number.bytes().all(|byte| byte.is_ascii_digit())
    }
}

/// Endereço base válido, terminado em `/`.
fn base(text: &str) -> Result<Url> {
    let mut url = warden_http::parse_url(text)?;
    if !url.path().ends_with('/') {
        let path = format!("{}/", url.path());
        url.set_path(&path);
    }
    Ok(url)
}

/// `base` + os pedaços do caminho, cada um codificado.
fn join(base: &Url, segments: &[&str]) -> Result<Url> {
    let mut url = base.clone();
    {
        let mut path = url
            .path_segments_mut()
            .map_err(|()| Error::Internal(format!("endereço sem caminho: {base}")))?;
        path.pop_if_empty();
        path.extend(segments);
    }
    url.set_query(None);
    Ok(url)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn oficiais() {
        let endpoints = Endpoints::official().unwrap();
        assert_eq!(
            endpoints.device_page().unwrap().as_str(),
            "https://github.com/login/device"
        );
        assert_eq!(
            endpoints.device_code_url().unwrap().as_str(),
            "https://github.com/login/device/code"
        );
        assert_eq!(
            endpoints.access_token_url().unwrap().as_str(),
            "https://github.com/login/oauth/access_token"
        );
        assert_eq!(
            endpoints.user_url().unwrap().as_str(),
            "https://api.github.com/user"
        );
        assert_eq!(
            endpoints
                .repo_url(&["labels", "categoria: visual ou texto"])
                .unwrap()
                .as_str(),
            "https://api.github.com/repos/Kriticales/Warden/labels/categoria:%20visual%20ou%20texto"
        );
        assert_eq!(
            endpoints.issue_page(42).unwrap().as_str(),
            "https://github.com/Kriticales/Warden/issues/42"
        );
        assert_eq!(
            endpoints.attachment_page("anexos/a1.png").unwrap().as_str(),
            "https://github.com/Kriticales/Warden/blob/relatos-anexos/anexos/a1.png?raw=true"
        );
        // O client ID fica vazio até o dono criar o GitHub App (ADR-0059).
        assert_eq!(CLIENT_ID, "");
        assert!(!endpoints.login_available());
    }

    #[test]
    fn pedaco_com_barra_nao_sai_do_repositorio() {
        let endpoints = Endpoints::official().unwrap();
        let url = endpoints.repo_url(&["labels", "../../outro"]).unwrap();
        assert_eq!(
            url.as_str(),
            "https://api.github.com/repos/Kriticales/Warden/labels/..%2F..%2Foutro"
        );
    }

    #[test]
    fn enderecos_de_teste_com_caminho() {
        let endpoints = Endpoints::custom(
            "http://127.0.0.1:9/github/api",
            "http://127.0.0.1:9/github/site/",
            "abc",
        )
        .unwrap();
        assert!(endpoints.login_available());
        assert_eq!(endpoints.client_id(), "abc");
        assert_eq!(
            endpoints.repo_url(&["issues"]).unwrap().as_str(),
            "http://127.0.0.1:9/github/api/repos/Kriticales/Warden/issues"
        );
        assert_eq!(
            endpoints.device_page().unwrap().as_str(),
            "http://127.0.0.1:9/github/site/login/device"
        );
        assert!(
            endpoints.is_issue_page("http://127.0.0.1:9/github/site/Kriticales/Warden/issues/7")
        );
    }

    #[test]
    fn so_paginas_de_issue_do_warden() {
        let endpoints = Endpoints::official().unwrap();
        assert!(endpoints.is_issue_page("https://github.com/Kriticales/Warden/issues/12"));
        for url in [
            "https://github.com/Kriticales/Warden/issues/",
            "https://github.com/Kriticales/Warden/issues/12/x",
            "https://github.com/Kriticales/Warden/issues/12?x=1",
            "https://github.com/outro/Warden/issues/12",
            "http://github.com/Kriticales/Warden/issues/12",
            "https://github.com.evil.com/Kriticales/Warden/issues/12",
            "nada",
        ] {
            assert!(!endpoints.is_issue_page(url), "{url}");
        }
    }

    #[test]
    fn client_id_vazio_ou_com_espacos_e_indisponivel() {
        let endpoints = Endpoints::custom(API_BASE_URL, LOGIN_BASE_URL, "  ").unwrap();
        assert!(!endpoints.login_available());
        assert!(Endpoints::custom("ftp://x/", LOGIN_BASE_URL, "a").is_err());
    }
}
