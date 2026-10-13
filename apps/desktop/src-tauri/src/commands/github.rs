//! Comandos do domínio `github` (SPEC T18; ARCHITECTURE §11.1 item 3): o repositório do pack
//! no GitHub e o "Testar" do token em Configurações.
//!
//! - `github_status`: repositório, visibilidade, versões publicadas e link do pack, sem rede
//!   (lido de `packs.json` e do histórico local).
//! - `github_setup`: primeira publicação: criar o repositório (público por padrão, privado
//!   como "só backup") ou vincular um que já existe (`dono/nome`).
//!
//! O token é lido do cofre na hora do uso (ADR-0025) e nunca chega à interface.
#![allow(clippy::needless_pass_by_value)] // argumentos pertencem ao contrato do Tauri

use std::path::Path;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State};
use warden_core::PackId;
use warden_http::{HttpClient, HttpConfig};
use warden_secrets::{SecretKind, SecretString};
use warden_versioning::PackRepo;
use warden_versioning::github::{
    GithubClient, GithubEndpoints, RepoRef, default_repo_name, validate_repo_name,
};
use warden_versioning::publish::PublishedVersion;

use crate::commands::inventory::{notify_changed, pack_root};
use crate::commands::secrets::SecretTester;
use crate::error::AppError;
use crate::events::PackArea;
use crate::state::AppState;

/// Chave do repositório do pack em `packs.json` (campo extra do registro, P1-07).
const REGISTRY_KEY: &str = "github";

/// O repositório do pack, como fica em `packs.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StoredLink {
    pub(crate) owner: String,
    pub(crate) name: String,
    pub(crate) private: bool,
}

impl StoredLink {
    pub(crate) fn repo(&self) -> RepoRef {
        RepoRef {
            owner: self.owner.clone(),
            name: self.name.clone(),
        }
    }
}

/// O repositório como a interface mostra.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GithubLinkView {
    /// `dono/nome`.
    pub(crate) repo: RepoRef,
    /// Privado ("só backup": o link não funciona para os jogadores).
    pub(crate) private: bool,
    /// Página do repositório.
    pub(crate) repo_url: String,
    /// Link do pack (`…/main/pack.toml`).
    pub(crate) pack_link: String,
}

/// A área "Publicação para os jogadores" do Histórico.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GithubStatus {
    /// O repositório do pack (`None` antes da primeira publicação).
    pub(crate) link: Option<GithubLinkView>,
    /// Versões publicadas, da mais nova para a mais antiga.
    pub(crate) published: Vec<PublishedVersion>,
    /// O token do GitHub está salvo em Configurações.
    pub(crate) token_configured: bool,
    /// Nome sugerido para o repositório (nome do pack em minúsculas com hífens).
    pub(crate) suggested_name: String,
    /// Minecraft do pack (passo a passo dos jogadores).
    pub(crate) minecraft: Option<String>,
    /// Loader e versão do pack (`Fabric 0.16.9`).
    pub(crate) loader: Option<String>,
}

/// O que fazer na primeira publicação.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, specta::Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub(crate) enum GithubSetup {
    /// Criar um repositório na conta do token.
    Create {
        /// Nome do repositório.
        name: String,
        /// Privado ("só backup").
        private: bool,
    },
    /// Usar um repositório que já existe (`dono/nome`).
    Link {
        /// `dono/nome` ou o endereço do repositório.
        repo: String,
    },
}

fn domain(error: warden_versioning::Error) -> AppError {
    AppError::from_domain(&error)
}

fn http() -> Result<HttpClient, AppError> {
    HttpClient::new(HttpConfig::for_version(env!("CARGO_PKG_VERSION")))
        .map_err(|error| AppError::from_domain(&error))
}

/// Cliente da API com um token dado.
pub(crate) fn client_with(token: SecretString) -> Result<GithubClient, AppError> {
    Ok(GithubClient::new(http()?, GithubEndpoints::from_env(), token))
}

/// Cliente da API com o token do cofre. Erro `secrets.*` se o token não estiver salvo.
pub(crate) async fn client(state: &AppState) -> Result<GithubClient, AppError> {
    let secrets = std::sync::Arc::clone(&state.secrets);
    let token = tauri::async_runtime::spawn_blocking(move || secrets.require(SecretKind::Github))
        .await
        .map_err(|error| AppError::internal(format!("leitura do token: {error}")))??;
    client_with(token)
}

/// O repositório gravado para o pack.
pub(crate) fn stored_link(state: &AppState, pack_id: PackId) -> Result<Option<StoredLink>, AppError> {
    let record = state
        .packs
        .get(pack_id)
        .map_err(|error| AppError::from_domain(&error))?;
    Ok(record
        .extra
        .get(REGISTRY_KEY)
        .and_then(|value| serde_json::from_value(value.clone()).ok()))
}

pub(crate) fn view(link: &StoredLink) -> GithubLinkView {
    let endpoints = GithubEndpoints::from_env();
    let repo = link.repo();
    GithubLinkView {
        repo_url: endpoints.repo_page(&repo),
        pack_link: endpoints.pack_link(&repo),
        repo,
        private: link.private,
    }
}

fn manifest(root: &Path) -> Option<warden_packwiz::PackManifest> {
    let text = std::fs::read_to_string(root.join("pack.toml")).ok()?;
    warden_packwiz::PackManifest::parse(&text)
        .ok()
        .map(|parsed| parsed.value)
}

/// Repositório, visibilidade, versões publicadas e link do pack. Não usa a rede.
#[tauri::command]
#[specta::specta]
pub(crate) async fn github_status(
    state: State<'_, AppState>,
    pack_id: PackId,
) -> Result<GithubStatus, AppError> {
    status_impl(&state, pack_id).await
}

async fn status_impl(state: &AppState, pack_id: PackId) -> Result<GithubStatus, AppError> {
    let _lock = state.locks.read(pack_id).await;
    let root = pack_root(state, pack_id)?;
    let link = stored_link(state, pack_id)?;
    let secrets = std::sync::Arc::clone(&state.secrets);
    let (published, token_configured) = tauri::async_runtime::spawn_blocking({
        let root = root.clone();
        move || -> Result<_, AppError> {
            let published = PackRepo::open(&root)
                .and_then(|repo| repo.published_versions())
                .map_err(domain)?;
            Ok((published, secrets.status()?.github))
        }
    })
    .await
    .map_err(|error| AppError::internal(format!("estado da publicação: {error}")))??;
    let manifest = manifest(&root);
    let name = manifest
        .as_ref()
        .map_or_else(|| state.packs.get(pack_id).map(|r| r.name).unwrap_or_default(), |m| m.name.clone());
    let loader = manifest.as_ref().and_then(|m| {
        let versions = m.versions.as_ref()?;
        ["neoforge", "forge", "fabric", "quilt"]
            .into_iter()
            .find_map(|key| versions.get(key).map(|v| (key, v.clone())))
            .map(|(key, version)| {
                let label = match key {
                    "neoforge" => "NeoForge",
                    "forge" => "Forge",
                    "fabric" => "Fabric",
                    _ => "Quilt",
                };
                format!("{label} {version}")
            })
    });
    Ok(GithubStatus {
        link: link.as_ref().map(view),
        published,
        token_configured,
        suggested_name: default_repo_name(&name),
        minecraft: manifest.as_ref().and_then(|m| m.minecraft_version().map(ToOwned::to_owned)),
        loader,
    })
}

/// Primeira publicação: cria o repositório (público por padrão) ou vincula um existente, e
/// grava em `packs.json`.
///
/// Erros: `versioning.GITHUB_REPO_NAME_TAKEN`, `GITHUB_INVALID_REPO_NAME`,
/// `GITHUB_REPO_NOT_FOUND`, `GITHUB_PERMISSION_MISSING`, `GITHUB_TOKEN_REJECTED`,
/// `GITHUB_OFFLINE`; `secrets.*` sem token.
#[tauri::command]
#[specta::specta]
pub(crate) async fn github_setup(
    app: AppHandle,
    state: State<'_, AppState>,
    pack_id: PackId,
    setup: GithubSetup,
) -> Result<GithubLinkView, AppError> {
    let link = setup_impl(&state, &client(&state).await?, pack_id, &setup).await?;
    notify_changed(&app, pack_id, vec![PackArea::History]);
    Ok(link)
}

async fn setup_impl(
    state: &AppState,
    client: &GithubClient,
    pack_id: PackId,
    setup: &GithubSetup,
) -> Result<GithubLinkView, AppError> {
    let root = pack_root(state, pack_id)?;
    let info = match setup {
        GithubSetup::Create { name, private } => {
            let name = name.trim();
            validate_repo_name(name).map_err(domain)?;
            let login = client.user(None).await.map_err(domain)?;
            let description = manifest(&root).map(|m| m.name).unwrap_or_default();
            client
                .create_repo(&login, name, *private, &description)
                .await
                .map_err(domain)?
        }
        GithubSetup::Link { repo } => {
            let repo = RepoRef::parse(repo).map_err(domain)?;
            client.repo(&repo).await.map_err(domain)?
        }
    };
    let link = StoredLink {
        owner: info.repo.owner.clone(),
        name: info.repo.name.clone(),
        private: info.private,
    };
    let value = serde_json::to_value(&link)
        .map_err(|error| AppError::internal(format!("repositório do pack: {error}")))?;
    state
        .packs
        .set_extra(pack_id, REGISTRY_KEY, Some(value))
        .map_err(|error| AppError::from_domain(&error))?;
    tracing::info!(repo = %info.repo.full_name(), privado = info.private, "repositório do pack no GitHub");
    Ok(view(&link))
}

/// "Testar" do token do GitHub em Configurações: `GET /user` com o token digitado.
#[derive(Debug, Default)]
pub(crate) struct GithubTokenTester;

#[async_trait::async_trait]
impl SecretTester for GithubTokenTester {
    async fn test(&self, value: SecretString) -> Result<(), AppError> {
        client_with(value)?.user(None).await.map_err(domain)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use wiremock::matchers::{body_partial_json, header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;
    use crate::commands::inventory::tests::{packwiz_binary, registered_pack, test_packwiz};
    use crate::state::tests::test_state;

    const TOKEN: &str = "ghp_tokenDoTesteDoApp0000000000000000";

    async fn github() -> (MockServer, GithubClient) {
        let server = MockServer::start().await;
        let auth = format!("Bearer {TOKEN}");
        Mock::given(method("GET"))
            .and(path("/user"))
            .and(header("authorization", auth.as_str()))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"login": "dono"})))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/user/repos"))
            .and(body_partial_json(json!({"name": "vale", "private": false})))
            .respond_with(ResponseTemplate::new(201).set_body_json(json!({
                "name": "vale", "owner": {"login": "dono"}, "private": false,
                "html_url": "https://github.com/dono/vale"
            })))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/repos/outra/backup"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "name": "backup", "owner": {"login": "outra"}, "private": true,
                "html_url": "https://github.com/outra/backup"
            })))
            .mount(&server)
            .await;
        let base = server.uri();
        let client = GithubClient::new(
            http().unwrap(),
            GithubEndpoints::new(&base, &base, &base).unwrap(),
            SecretString::from(TOKEN.to_owned()),
        );
        (server, client)
    }

    #[tokio::test]
    async fn criar_publico_grava_em_packs_json_e_o_estado_mostra_o_link() {
        let dir = tempfile::tempdir().unwrap();
        let state = test_state(dir.path());
        let Some(binary) = packwiz_binary() else {
            return;
        };
        let cli = test_packwiz(&state, binary);
        let (pack_id, root) = registered_pack(&state, dir.path(), &cli, 0).await;
        PackRepo::init(
            &root,
            warden_versioning::InitialPoint::Created,
            &warden_versioning::Identity::for_pack("Autor", None),
            warden_versioning::Moment::new(1_790_866_800, 0),
        )
        .unwrap();

        // O "Testar" de Configurações tem testador para o token do GitHub.
        assert!(format!("{:?}", state.secret_testers).contains("Github"));
        let before = status_impl(&state, pack_id).await.unwrap();
        assert!(before.link.is_none());
        assert!(!before.token_configured);
        assert_eq!(before.suggested_name, "vale");
        assert_eq!(before.minecraft.as_deref(), Some("1.20.1"));
        assert!(before.published.is_empty());

        let (_server, client) = github().await;
        let created = setup_impl(
            &state,
            &client,
            pack_id,
            &GithubSetup::Create {
                name: "vale".into(),
                private: false,
            },
        )
        .await
        .unwrap();
        assert_eq!(created.repo.full_name(), "dono/vale");
        assert!(!created.private);
        let record = state.packs.get(pack_id).unwrap();
        assert_eq!(
            record.extra[REGISTRY_KEY],
            json!({"owner": "dono", "name": "vale", "private": false})
        );
        let after = status_impl(&state, pack_id).await.unwrap();
        let link = after.link.unwrap();
        assert!(link.pack_link.ends_with("/dono/vale/main/pack.toml"), "{}", link.pack_link);
        assert!(!serde_json::to_string(&record.extra).unwrap().contains(TOKEN));

        // Vincular um existente troca o repositório e mostra a visibilidade dele.
        let linked = setup_impl(
            &state,
            &client,
            pack_id,
            &GithubSetup::Link {
                repo: "outra/backup".into(),
            },
        )
        .await
        .unwrap();
        assert!(linked.private);
        // Nome inválido é recusado antes de qualquer pedido.
        let error = setup_impl(
            &state,
            &client,
            pack_id,
            &GithubSetup::Create {
                name: "nome com espaço".into(),
                private: false,
            },
        )
        .await
        .unwrap_err();
        assert_eq!(
            serde_json::to_value(error.code).unwrap(),
            json!({"domain": "versioning", "code": "GITHUB_INVALID_REPO_NAME"})
        );
    }
}
