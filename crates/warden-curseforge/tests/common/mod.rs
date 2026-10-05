//! Auxiliares dos testes com servidor simulado.
//!
//! As respostas são montadas aqui, no formato das respostas reais conferido em 05/10/2026
//! (IDs reais, campos reduzidos), em vez de gravadas: os termos da CurseForge proíbem guardar
//! dados obtidos pela API (§3.1(e)), então o repositório não versiona respostas dela. O formato
//! é conferido contra a API real pelos testes `rede_*` (`tests/rede_curseforge.rs`), que
//! decodificam respostas reais com os mesmos tipos.

// Nos auxiliares dos testes, `unwrap`, `expect` e `panic!` são a forma idiomática de falhar
// (o `clippy.toml` só os libera dentro das funções `#[test]`).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    dead_code,
    unreachable_pub
)]

use std::sync::Arc;

use serde_json::{Value, json};
use warden_curseforge::{CurseforgeClient, MemoryCache, SecretString};
use warden_http::{HttpClient, HttpConfig, ManualTimer};
use wiremock::{MockServer, ResponseTemplate};

/// Chave falsa, no formato das chaves da CurseForge (`$2a$…`).
pub const KEY: &str = "$2a$10$chaveFalsaDoWardenParaTestes0123456789abcdefghijkl";

/// JEI.
pub const JEI: u64 = 238_222;
/// Entity Culling (distribuição bloqueada).
pub const ENTITY_CULLING: u64 = 448_233;

pub struct Setup {
    pub server: MockServer,
    pub client: CurseforgeClient,
    pub timer: ManualTimer,
}

/// Servidor simulado e cliente com a chave falsa, relógio manual e cache próprio.
pub async fn setup() -> Setup {
    setup_with_key(Some(KEY)).await
}

pub async fn setup_with_key(key: Option<&str>) -> Setup {
    let server = MockServer::start().await;
    let timer = ManualTimer::new();
    let http =
        HttpClient::with_timer(HttpConfig::for_version("1"), Arc::new(timer.clone())).unwrap();
    let client = CurseforgeClient::with_base_url(
        http,
        &format!("{}/v1", server.uri()),
        key.map(|key| SecretString::from(key.to_owned())),
    )
    .unwrap()
    .with_cache(MemoryCache::new(Arc::new(timer.clone())));
    Setup {
        server,
        client,
        timer,
    }
}

/// `{"data": …}` com 200.
pub fn data(value: Value) -> ResponseTemplate {
    ResponseTemplate::new(200)
        .insert_header("content-type", "application/json; charset=utf-8")
        .set_body_json(json!({ "data": value }))
}

/// Resposta paginada.
pub fn paged(items: Vec<Value>, total: u64) -> ResponseTemplate {
    let count = items.len();
    ResponseTemplate::new(200)
        .insert_header("content-type", "application/json; charset=utf-8")
        .set_body_json(json!({
            "data": items,
            "pagination": {"index": 0, "pageSize": count, "resultCount": count, "totalCount": total}
        }))
}

/// 403 vazio, como a CloudFront da CurseForge responde a uma chave recusada.
pub fn forbidden() -> ResponseTemplate {
    ResponseTemplate::new(403)
}

/// Um arquivo no formato da API.
pub fn file(mod_id: u64, file_id: u64, name: &str, download_url: Option<&str>) -> Value {
    json!({
        "id": file_id,
        "gameId": 432,
        "modId": mod_id,
        "isAvailable": true,
        "displayName": name,
        "fileName": name,
        "releaseType": 1,
        "fileStatus": 4,
        "hashes": [
            {"value": "130494fd5e5e8eb5ea8008f479a34672e658e653", "algo": 1},
            {"value": "1b82bf047b7a1b2e19224d979a6135ea", "algo": 2}
        ],
        "fileDate": "2026-09-19T11:47:57.347Z",
        "fileLength": 1_036_704,
        "downloadCount": 7,
        "downloadUrl": download_url,
        "gameVersions": ["Client", "1.20.1", "Forge", "Server"],
        "sortableGameVersions": [
            {"gameVersionName": "Client", "gameVersionPadded": "0", "gameVersion": "", "gameVersionReleaseDate": "2022-12-08T00:00:00Z", "gameVersionTypeId": 75208},
            {"gameVersionName": "1.20.1", "gameVersionPadded": "0000000001.0000000020.0000000001", "gameVersion": "1.20.1", "gameVersionReleaseDate": "2023-06-12T14:26:38.477Z", "gameVersionTypeId": 75125},
            {"gameVersionName": "Forge", "gameVersionPadded": "0", "gameVersion": "", "gameVersionReleaseDate": "2022-10-01T00:00:00Z", "gameVersionTypeId": 68441}
        ],
        "dependencies": [{"modId": 1_689_768, "relationType": 3}, {"modId": 1_700_987, "relationType": 2}],
        "alternateFileId": 0,
        "isServerPack": false,
        "fileFingerprint": file_id as u32,
        "modules": [{"name": "META-INF", "fingerprint": 1_214_030_310}]
    })
}

/// Um projeto no formato da API.
pub fn project(
    id: u64,
    slug: &str,
    name: &str,
    allow_distribution: bool,
    files: Vec<Value>,
) -> Value {
    json!({
        "id": id,
        "gameId": 432,
        "name": name,
        "slug": slug,
        "links": {
            "websiteUrl": format!("https://www.curseforge.com/minecraft/mc-mods/{slug}"),
            "wikiUrl": null, "issuesUrl": null, "sourceUrl": null
        },
        "summary": format!("Resumo de teste de {name}"),
        "status": 4,
        "downloadCount": 450_000_000,
        "isFeatured": false,
        "primaryCategoryId": 421,
        "categories": [{"id": 421, "gameId": 432, "name": "API and Library", "slug": "library-api", "classId": 6, "parentCategoryId": 6}],
        "classId": 6,
        "authors": [{"id": 1, "name": "mezz", "url": "https://www.curseforge.com/members/mezz"}],
        "logo": {"id": 1, "modId": id, "title": "logo", "description": "", "thumbnailUrl": "https://media.forgecdn.net/x.png", "url": "https://media.forgecdn.net/x.png"},
        "screenshots": [],
        "mainFileId": files.first().and_then(|f| f["id"].as_u64()).unwrap_or(0),
        "latestFiles": files,
        "latestFilesIndexes": [{"gameVersion": "1.20.1", "fileId": 1, "filename": "a.jar", "releaseType": 1, "gameVersionTypeId": 75125, "modLoader": 1}],
        "latestEarlyAccessFilesIndexes": [],
        "dateCreated": "2016-01-01T00:00:00Z",
        "dateModified": "2026-10-01T00:00:00Z",
        "dateReleased": "2026-10-01T00:00:00Z",
        "allowModDistribution": allow_distribution,
        "gamePopularityRank": 1,
        "isAvailable": true,
        "hasCommentsEnabled": false,
        "thumbsUpCount": 0
    })
}
