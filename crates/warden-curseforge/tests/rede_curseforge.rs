//! Testes contra a API real da CurseForge (`cargo xtask test-network`; QUALITY §4.1). Marcados
//! `#[ignore = "rede"]`, com nome `rede_*`. A chave vem de `CURSEFORGE_API_KEY` (o `xtask`
//! carrega o `.env` do repositório principal sem imprimir valores). Nada é gravado, exceto o jar
//! baixado numa pasta temporária.
//!
//! Também servem de teste de contrato: os tipos da crate decodificam as respostas reais, que
//! não são versionadas (termos da CurseForge, §3.1(e)).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use warden_core::NoProgress;
use warden_curseforge::{
    CurseforgeClient, Distribution, Error, FileRef, FilesQuery, ModLoaderType, ProjectClass,
    SearchQuery, SecretString,
};
use warden_http::{HttpClient, HttpConfig};

/// JEI (Just Enough Items).
const JEI: u64 = 238_222;

/// Entity Culling (tr7zw), o mod com distribuição bloqueada mais baixado da CurseForge na
/// conferência de 05/10/2026 (231 milhões de downloads, `allowModDistribution: false`, arquivos
/// sem `downloadUrl` e `download-url` com 403). O autor desliga a distribuição por terceiros em
/// todos os seus mods (3D Skin Layers, Not Enough Animations, Wavey Capes…), que aparecem na
/// lista de mods bloqueados do Prism; é um caso conhecido e estável.
const ENTITY_CULLING: u64 = 448_233;

fn key() -> SecretString {
    let key = std::env::var("CURSEFORGE_API_KEY")
        .expect("CURSEFORGE_API_KEY ausente: rode pelo `cargo xtask test-network`");
    SecretString::from(key)
}

fn client() -> CurseforgeClient {
    let http = HttpClient::new(HttpConfig::default()).unwrap();
    CurseforgeClient::with_base_url(http, warden_curseforge::DEFAULT_BASE_URL, Some(key())).unwrap()
}

/// CA-1 da P1-04: buscar "jei" para Forge 1.20.1 retorna o JEI.
#[tokio::test]
#[ignore = "rede"]
async fn rede_p1_04_ca1_busca_jei_forge_1_20_1() {
    let client = client();
    let query = SearchQuery::new("jei")
        .class(ProjectClass::Mod)
        .game_version("1.20.1")
        .loader(ModLoaderType::Forge);
    let results = client.search(&query, None).await.unwrap();
    assert!(results.pagination.total_count > 0);
    let jei = results
        .mods
        .iter()
        .find(|project| project.id == JEI)
        .expect("o JEI não veio na busca");
    assert_eq!(jei.slug, "jei");
    assert_eq!(jei.class_id, Some(ProjectClass::Mod));
    assert!(!jei.authors.is_empty());
    assert!(
        jei.latest_files_indexes
            .iter()
            .any(|index| index.game_version == "1.20.1"
                && index.mod_loader == Some(ModLoaderType::Forge)),
        "o JEI não tem arquivo para Forge 1.20.1"
    );
    // Por popularidade, o JEI é o primeiro.
    assert_eq!(results.mods[0].id, JEI);
}

/// CA-1 da P1-04: um mod com distribuição bloqueada conhecido é identificado como bloqueado,
/// pelo projeto, pela situação do arquivo (com a página do download manual) e pelo
/// `download-url`.
#[tokio::test]
#[ignore = "rede"]
async fn rede_p1_04_ca1_mod_bloqueado_identificado() {
    let client = client();
    let project = client.project(ENTITY_CULLING, None).await.unwrap();
    assert!(project.is_distribution_blocked(), "{}", project.name);
    let file_id = project.main_file_id;
    let report = client
        .distribution(
            &[
                FileRef::new(ENTITY_CULLING, file_id),
                FileRef::new(JEI, client.project(JEI, None).await.unwrap().main_file_id),
            ],
            None,
        )
        .await
        .unwrap();
    let Distribution::Blocked { page_url } = &report[0].distribution else {
        panic!("Entity Culling não veio bloqueado: {:?}", report[0]);
    };
    let page_url = page_url.as_deref().expect("sem página do download manual");
    assert!(
        page_url.starts_with("https://www.curseforge.com/minecraft/mc-mods/")
            && page_url.ends_with(&format!("/files/{file_id}")),
        "{page_url}"
    );
    assert!(
        matches!(report[1].distribution, Distribution::Allowed { .. }),
        "{:?}",
        report[1]
    );
    let error = client
        .download_url(ENTITY_CULLING, file_id, None)
        .await
        .unwrap_err();
    assert!(
        matches!(
            error,
            Error::DistributionBlocked {
                mod_id: ENTITY_CULLING,
                ..
            }
        ),
        "{error}"
    );
}

/// O `classId` de cada classe, confirmado pela API (o de shaders era inferência na R3 §3.2).
#[tokio::test]
#[ignore = "rede"]
async fn rede_classes_confirmadas_pela_api() {
    let client = client();
    let classes = client.classes(None).await.unwrap();
    let id_of = |slug: &str| {
        classes
            .iter()
            .find(|class| class.slug == slug)
            .map(|class| class.id)
    };
    assert_eq!(id_of("mc-mods"), Some(ProjectClass::Mod.id()));
    assert_eq!(
        id_of("texture-packs"),
        Some(ProjectClass::ResourcePack.id())
    );
    assert_eq!(id_of("shaders"), Some(ProjectClass::Shader.id()));
    assert_eq!(ProjectClass::Shader.id(), 6552);
    assert_eq!(id_of("modpacks"), Some(ProjectClass::Modpack.id()));
    assert_eq!(id_of("data-packs"), Some(ProjectClass::DataPack.id()));
    assert!(classes.iter().all(|class| class.is_class == Some(true)));

    // A busca por classe devolve só aquela classe.
    let results = client
        .search(
            &SearchQuery::new("complementary").class(ProjectClass::Shader),
            None,
        )
        .await
        .unwrap();
    assert!(!results.mods.is_empty());
    assert!(
        results
            .mods
            .iter()
            .all(|project| project.class_id == Some(ProjectClass::Shader))
    );
    let results = client
        .search(
            &SearchQuery::new("faithful")
                .class(ProjectClass::ResourcePack)
                .game_version("1.20.1"),
            None,
        )
        .await
        .unwrap();
    assert!(
        results
            .mods
            .iter()
            .all(|project| project.class_id == Some(ProjectClass::ResourcePack))
    );
    let categories = client
        .categories(Some(ProjectClass::Mod), None)
        .await
        .unwrap();
    assert!(
        categories
            .iter()
            .any(|category| category.class_id == Some(6))
    );
}

/// CA-2 da P1-04 contra a API real: a chave verdadeira é aceita; uma inventada vira
/// `CURSEFORGE_KEY_INVALID`.
#[tokio::test]
#[ignore = "rede"]
async fn rede_p1_04_ca2_chave_real_e_chave_inventada() {
    client().check_key(None).await.unwrap();
    let http = HttpClient::new(HttpConfig::default()).unwrap();
    let invalid = CurseforgeClient::with_base_url(
        http,
        warden_curseforge::DEFAULT_BASE_URL,
        Some(SecretString::from(
            "$2a$10$chaveInventadaPeloTesteDoWarden000000000000000000000".to_owned(),
        )),
    )
    .unwrap();
    let error = invalid.check_key(None).await.unwrap_err();
    assert!(
        matches!(error, Error::KeyInvalid { status: Some(403) }),
        "{error}"
    );
    let error = invalid
        .search(&SearchQuery::new("jei"), None)
        .await
        .unwrap_err();
    assert!(error.is_key_problem(), "{error}");
}

/// Contrato e CDN real: arquivos do JEI para Forge 1.20.1, o arquivo pelo ID e em lote, a
/// descrição, o download pela CDN com a chave (o `edge.forgecdn.net` redireciona para o
/// `mediafilez.forgecdn.net`) com o SHA-1 conferido, e a identificação do jar baixado pela
/// impressão digital calculada no download.
#[tokio::test]
#[ignore = "rede"]
async fn rede_arquivos_download_e_impressao_digital_do_jei() {
    let client = client();
    let page = client
        .files(
            JEI,
            &FilesQuery::default()
                .game_version("1.20.1")
                .loader(ModLoaderType::Forge)
                .page(0, 5),
            None,
        )
        .await
        .unwrap();
    assert!(!page.files.is_empty());
    let file = page.files[0].clone();
    assert_eq!(file.mod_id, JEI);
    assert!(file.minecraft_versions().contains(&"1.20.1".to_owned()));
    assert!(file.loaders().contains(&ModLoaderType::Forge));
    assert!(file.sha1().is_some());
    assert!(!file.is_distribution_blocked());

    assert_eq!(client.file(JEI, file.id, None).await.unwrap().id, file.id);
    let by_id = client.files_by_id(&[file.id, 1], None).await.unwrap();
    assert_eq!(by_id.len(), 1);
    assert!(!client.description(JEI, None).await.unwrap().is_empty());
    let url = client.download_url(JEI, file.id, None).await.unwrap();
    assert_eq!(url.host_str(), Some("edge.forgecdn.net"));

    let dir = tempfile::tempdir().unwrap();
    let request = client
        .download_request(&file, dir.path().join("jei.jar"), None)
        .await
        .unwrap();
    assert!(request.headers["x-api-key"].is_sensitive());
    let http = HttpClient::new(HttpConfig::default()).unwrap();
    let downloaded = http.download(&request, &NoProgress, None).await.unwrap();
    assert_eq!(downloaded.size, file.file_length);
    assert_eq!(
        downloaded.final_url.host_str(),
        Some("mediafilez.forgecdn.net")
    );
    assert_eq!(downloaded.hashes.murmur2, file.file_fingerprint);

    let matches = client
        .fingerprints(&[downloaded.hashes.murmur2, 1], None)
        .await
        .unwrap();
    assert_eq!(matches.matches[&downloaded.hashes.murmur2].file.id, file.id);
    assert_eq!(matches.matches[&downloaded.hashes.murmur2].id, JEI);
    assert_eq!(matches.unmatched, vec![1]);
}
