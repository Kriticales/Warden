//! Materialização com mods reais (`cargo xtask test-network`; QUALITY §4.1). Marcados
//! `#[ignore = "rede"]`, com nome `rede_*`. A chave da CurseForge vem de `CURSEFORGE_API_KEY`
//! (o `xtask` carrega o `.env` do repositório principal sem imprimir valores). Nada da API da
//! CurseForge é gravado no repositório: os hashes dos arquivos da CurseForge são lidos na hora.

// Nos auxiliares dos testes, `unwrap`, `expect` e `panic!` são a forma idiomática de falhar
// (o `clippy.toml` só os libera dentro das funções `#[test]`).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod conformance_support;

use std::fs;

use conformance_support::{
    TestMod, TestPack, dirs, externals, materialize_ok, run_installer, temp_dir, tree,
};
use warden_curseforge::{CurseforgeClient, SecretString};
use warden_http::{HttpClient, HttpConfig};
use warden_instance::{
    DownloadCache, MaterializeOptions, OptionalChoices, OptionalSelection, OriginSource, Sources,
};
use warden_packwiz::HashFormat;

/// Mod Menu 7.2.2 (Fabric 1.20.1) no Modrinth: arquivo imutável, conferido em 05/10/2026.
const MODMENU_URL: &str =
    "https://cdn.modrinth.com/data/mOgUt4GM/versions/lEkperf6/modmenu-7.2.2.jar";
const MODMENU_SHA512: &str = "9a7837e04bb34376611b207a3b20e5fe1c82a4822b42929d5b410809ec4b88ff3cac8821c4568f880775bafa3c079dfc7800f8471356a4046248b12607e855eb";

/// JEI e Entity Culling (distribuição bloqueada; ver `warden-curseforge/tests/rede_curseforge.rs`).
const JEI: u64 = 238_222;
const ENTITY_CULLING: u64 = 448_233;

fn modmenu() -> TestMod {
    TestMod::real(
        "modmenu",
        "modmenu-7.2.2.jar",
        HashFormat::Sha512,
        MODMENU_SHA512,
        Some(MODMENU_URL),
    )
    .modrinth("mOgUt4GM", "lEkperf6")
}

fn curseforge_client() -> CurseforgeClient {
    let key = std::env::var("CURSEFORGE_API_KEY")
        .expect("CURSEFORGE_API_KEY ausente: rode pelo `cargo xtask test-network`");
    let http = HttpClient::new(HttpConfig::default()).unwrap();
    CurseforgeClient::with_base_url(
        http,
        warden_curseforge::DEFAULT_BASE_URL,
        Some(SecretString::from(key)),
    )
    .unwrap()
}

/// O arquivo principal de um projeto da CurseForge, como metafile `metadata:curseforge`.
async fn curseforge_mod(client: &CurseforgeClient, slug: &str, project: u64) -> TestMod {
    let found = client.project(project, None).await.unwrap();
    let file = client
        .file(project, found.main_file_id, None)
        .await
        .unwrap();
    let sha1 = file.sha1().expect("arquivo sem sha1");
    TestMod::real(slug, &file.file_name, HashFormat::Sha1, &sha1, None).curseforge(
        u32::try_from(project).unwrap(),
        u32::try_from(file.id).unwrap(),
    )
}

#[tokio::test]
#[ignore = "rede"]
async fn rede_materializa_modrinth_e_curseforge_com_bloqueado() {
    let dir = temp_dir();
    let client = curseforge_client();
    let jei = curseforge_mod(&client, "jei", JEI).await;
    let culling = curseforge_mod(&client, "entityculling", ENTITY_CULLING).await;
    let pack = TestPack::default()
        .with_mod(modmenu())
        .with_mod(jei.clone())
        .with_mod(culling.clone())
        .with_file("config/modmenu.json", "{}\n");
    let pack_dir = dir.path().join("pack");
    pack.write_to(&pack_dir, "https://nao-usado.invalid");
    let sources = Sources {
        http: HttpClient::new(HttpConfig::default()).unwrap(),
        curseforge: Some(client.clone()),
        cache: std::sync::Arc::new(DownloadCache::open(&dir.path().join("cache")).unwrap()),
    };
    let instance = dirs(&dir.path().join("instancia"));
    let options = MaterializeOptions::default();
    let report = materialize_ok(&sources, &pack_dir, &instance, &options).await;
    assert_eq!(report.downloaded, 2, "{report:?}");
    assert!(instance.game_dir.join("mods/modmenu-7.2.2.jar").is_file());
    assert!(instance.game_dir.join("mods").join(&jei.filename).is_file());
    assert_eq!(report.blocked.len(), 1, "{report:?}");
    assert_eq!(report.blocked[0].file_name, culling.filename);
    assert!(
        report.blocked[0]
            .page_url
            .as_deref()
            .is_some_and(|url| url.starts_with("https://www.curseforge.com/"))
    );
    let jei_cached = sources
        .cache
        .find(&warden_http::ExpectedHash::new(
            HashFormat::Sha1,
            jei.known_hash.clone().unwrap(),
        ))
        .unwrap()
        .unwrap();
    let origins = sources.cache.origins(&jei_cached.hashes.sha256).unwrap();
    assert_eq!(origins[0].source, OriginSource::Curseforge);
    assert_eq!(origins[0].url, None);

    // De novo: nada da rede, nem da API da CurseForge.
    let api_requests = client.request_count();
    let again = materialize_ok(&sources, &pack_dir, &instance, &options).await;
    assert_eq!((again.written, again.downloaded), (0, 0), "{again:?}");
    assert_eq!(again.blocked.len(), 1);
    assert!(
        client.request_count() <= api_requests + 2,
        "consultas demais"
    );
    let manifest = fs::read_to_string(instance.state_dir.join("manifest.json")).unwrap();
    assert!(!manifest.contains("forgecdn"), "{manifest}");
}

/// Conformidade com um mod real do Modrinth: o packwiz-installer real e o Warden baixam do CDN
/// do Modrinth e chegam à mesma árvore.
#[tokio::test(flavor = "multi_thread")]
#[ignore = "rede"]
async fn rede_conformidade_com_mod_real_do_modrinth() {
    let Some(externals) = externals() else {
        return;
    };
    let dir = temp_dir();
    let pack = TestPack::default()
        .with_mod(modmenu())
        .with_file("config/modmenu.json", "{\"count_libraries\": true}\n");
    let pack_dir = dir.path().join("pack");
    pack.write_to(&pack_dir, "https://nao-usado.invalid");
    let out = dir.path().join("instalador");
    let (ext, pack_toml, out_dir, cwd) = (
        externals,
        pack_dir.join("pack.toml"),
        out.clone(),
        dir.path().join("cwd"),
    );
    tokio::task::spawn_blocking(move || run_installer(&ext, &pack_toml, &out_dir, &cwd))
        .await
        .unwrap();
    let sources = Sources {
        http: HttpClient::new(HttpConfig::default()).unwrap(),
        curseforge: None,
        cache: std::sync::Arc::new(DownloadCache::open(&dir.path().join("cache")).unwrap()),
    };
    let instance = dirs(&dir.path().join("warden"));
    let options = MaterializeOptions {
        optionals: OptionalSelection::Explicit(OptionalChoices::default()),
        ..MaterializeOptions::default()
    };
    materialize_ok(&sources, &pack_dir, &instance, &options).await;
    assert_eq!(tree(&instance.game_dir), tree(&out));
}
