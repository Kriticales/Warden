//! Comandos do domínio `catalog` (ARCHITECTURE §4.1; SPEC T03, etapas "Versão do Minecraft" e
//! "Loader").
//!
//! Finos: chamam a `warden-catalog` e convertem o erro. A lista vem com a data e o aviso de
//! que veio do cache sem rede ([`warden_catalog::Freshness`]), para a interface mostrar "Lista
//! de versões de <data>". `forceRefresh` é o "Tentar de novo": pede à fonte mesmo com o cache
//! válido.

use tauri::State;
use warden_catalog::{Catalog, CatalogCache, Loader, LoaderVersions, MinecraftVersions, Refresh};
use warden_core::AppPaths;
use warden_http::{HttpClient, HttpConfig};

use crate::error::AppError;
use crate::state::AppState;

/// Abre o catálogo do app: cliente HTTP com o User-Agent do Warden e o cache em
/// `cache/metadata.sqlite`. Se o cache não abrir (disco), o catálogo funciona com um cache em
/// memória e o problema fica registrado.
pub(crate) fn open_catalog(paths: &AppPaths) -> Result<Catalog, AppError> {
    let http = HttpClient::new(HttpConfig::for_version(env!("CARGO_PKG_VERSION")))
        .map_err(|error| AppError::from_domain(&error))?;
    let cache = match CatalogCache::open(&paths.metadata_db_file()) {
        Ok(cache) => cache,
        Err(error) => {
            tracing::warn!(%error, "cache do catálogo indisponível; usando cache em memória");
            CatalogCache::in_memory(None).map_err(|error| AppError::from_domain(&error))?
        }
    };
    Catalog::new(http, cache).map_err(|error| AppError::from_domain(&error))
}

fn refresh(force_refresh: bool) -> Refresh {
    if force_refresh {
        Refresh::Always
    } else {
        Refresh::IfStale
    }
}

/// As versões do Minecraft, na ordem oficial do manifesto da Mojang (da mais nova para a mais
/// antiga), com o tipo de cada uma e a etiqueta "melhor esforço" antes da 1.7.10.
#[tauri::command]
#[specta::specta]
#[allow(clippy::needless_pass_by_value)] // contrato do Tauri
pub(crate) async fn catalog_minecraft_versions(
    state: State<'_, AppState>,
    force_refresh: bool,
) -> Result<MinecraftVersions, AppError> {
    minecraft_versions_impl(&state.catalog, force_refresh).await
}

async fn minecraft_versions_impl(
    catalog: &Catalog,
    force_refresh: bool,
) -> Result<MinecraftVersions, AppError> {
    catalog
        .minecraft_versions(refresh(force_refresh), None)
        .await
        .map_err(|error| AppError::from_domain(&error))
}

/// As versões de um loader para uma versão do Minecraft, da mais nova para a mais antiga, com
/// a pré-selecionada. Lista vazia: o loader não existe para essa versão.
#[tauri::command]
#[specta::specta]
#[allow(clippy::needless_pass_by_value)] // contrato do Tauri
pub(crate) async fn catalog_loader_versions(
    state: State<'_, AppState>,
    loader: Loader,
    minecraft: String,
    force_refresh: bool,
) -> Result<LoaderVersions, AppError> {
    loader_versions_impl(&state.catalog, loader, &minecraft, force_refresh).await
}

async fn loader_versions_impl(
    catalog: &Catalog,
    loader: Loader,
    minecraft: &str,
    force_refresh: bool,
) -> Result<LoaderVersions, AppError> {
    if minecraft.trim().is_empty() || minecraft.len() > 64 {
        return Err(
            AppError::new(warden_catalog::CatalogErrorCode::VersionNotFound)
                .with_param("id", minecraft.chars().take(64).collect::<String>()),
        );
    }
    catalog
        .loader_versions(loader, minecraft, refresh(force_refresh), None)
        .await
        .map_err(|error| AppError::from_domain(&error))
}

#[cfg(test)]
mod tests {
    use warden_catalog::{CatalogErrorCode, Endpoints};
    use warden_core::CoreErrorCode;
    use wiremock::matchers::path;
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;
    use crate::error::ErrorCode;

    macro_rules! fixture {
        ($name:literal) => {
            include_bytes!(concat!(
                "../../../../../crates/warden-catalog/tests/fixtures/http/2026-10-05-",
                $name
            ))
        };
    }

    fn catalog(base: &str) -> Catalog {
        let http = HttpClient::new(HttpConfig::for_version("1")).unwrap();
        Catalog::with_endpoints(
            http,
            Endpoints::single(base).unwrap(),
            CatalogCache::in_memory(None).unwrap(),
        )
    }

    #[tokio::test]
    async fn listas_com_a_data_e_sem_aviso_de_cache() {
        let server = MockServer::start().await;
        for (route, body) in [
            (
                "/mc/game/version_manifest_v2.json",
                &fixture!("mojang-version_manifest_v2.json")[..],
            ),
            (
                "/net/minecraftforge/forge/maven-metadata.xml",
                &fixture!("forge-maven-metadata.xml")[..],
            ),
            (
                "/net/minecraftforge/forge/promotions_slim.json",
                &fixture!("forge-promotions_slim.json")[..],
            ),
        ] {
            Mock::given(path(route))
                .respond_with(ResponseTemplate::new(200).set_body_bytes(body))
                .mount(&server)
                .await;
        }
        let catalog = catalog(&server.uri());
        let minecraft = minecraft_versions_impl(&catalog, false).await.unwrap();
        assert_eq!(minecraft.latest_release.as_deref(), Some("26.3"));
        assert!(!minecraft.freshness.offline);
        let forge = loader_versions_impl(&catalog, Loader::Forge, "1.7.10", true)
            .await
            .unwrap();
        assert_eq!(forge.preselected.as_deref(), Some("10.13.4.1614"));
        // O formato que chega à interface.
        let json = serde_json::to_value(&forge).unwrap();
        assert_eq!(json["loader"], "forge");
        assert_eq!(json["versions"][0]["version"], "10.13.4.1614");
        assert_eq!(json["versions"][0]["recommended"], true);
        assert!(json["freshness"]["fetchedAtMs"].is_number());
        assert_eq!(json["freshness"]["offline"], false);
        let json = serde_json::to_value(&minecraft).unwrap();
        assert_eq!(json["latestRelease"], "26.3");
        assert_eq!(json["versions"][0]["kind"], "snapshot");
        assert!(json["versions"][0]["bestEffort"].is_boolean());
    }

    #[tokio::test]
    async fn sem_rede_e_sem_cache_e_erro_de_rede_com_a_fonte() {
        let catalog = catalog("http://127.0.0.1:1");
        let error = loader_versions_impl(&catalog, Loader::NeoForge, "1.21.1", false)
            .await
            .unwrap_err();
        assert_eq!(
            error.code,
            ErrorCode::Core(CoreErrorCode::NetworkUnavailable)
        );
        assert_eq!(error.params["source"], "NeoForge");
        assert!(error.retryable);
    }

    #[tokio::test]
    async fn versao_do_minecraft_vazia_e_recusada_sem_rede() {
        let catalog = catalog("http://127.0.0.1:1");
        for minecraft in ["", "  ", &"9".repeat(65)] {
            let error = loader_versions_impl(&catalog, Loader::Fabric, minecraft, false)
                .await
                .unwrap_err();
            assert_eq!(
                error.code,
                ErrorCode::Catalog(CatalogErrorCode::VersionNotFound)
            );
        }
        assert_eq!(catalog.request_count(), 0);
    }

    #[test]
    fn abre_o_catalogo_na_pasta_de_dados() {
        let dir = tempfile::tempdir().unwrap();
        let paths = AppPaths::from_dev_root(dir.path()).unwrap();
        let catalog = open_catalog(&paths).unwrap();
        assert_eq!(
            catalog.cache().path(),
            Some(paths.metadata_db_file().as_path())
        );
        assert_eq!(refresh(true), Refresh::Always);
        assert_eq!(refresh(false), Refresh::IfStale);
    }
}
