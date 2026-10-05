//! Semântica da materialização além do que o packwiz-installer cobre (ROADMAP L-03; SPEC T13,
//! T15, T20 e T22): contagem de operações, cache por hash, CurseForge na hora com a chave
//! também na CDN, mods bloqueados, pausa antes de sobrescrever arquivo alterado, opcionais com
//! o padrão do item, falhas parciais e cancelamento.

// Nos auxiliares dos testes, `unwrap`, `expect` e `panic!` são a forma idiomática de falhar
// (o `clippy.toml` só os libera dentro das funções `#[test]`).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::case_sensitive_file_extension_comparisons
)]

mod conformance_support;

use std::fs;
use std::path::Path;
use std::time::{Duration, Instant};

use conformance_support::{
    KEY, TestMod, TestPack, TestSide, cf_data, cf_file, cf_project, curseforge, dirs,
    materialize_ok, sources, temp_dir, tree, write,
};
use serde_json::json;
use warden_core::{CancellationToken, DomainError as _, NoProgress};
use warden_instance::{
    EntryOrigin, Error, FailureReason, MANIFEST_FILE, Manifest, MaterializeOptions, ModifiedAction,
    OnModified, OptionalChoices, OriginSource, Outcome, accept_manual_file, materialize,
};
use warden_packwiz::HashFormat;
use warden_packwiz::hash::hash_bytes;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn sample_pack() -> TestPack {
    TestPack::default()
        .with_mod(
            TestMod::new("sodium")
                .modrinth("AANobbMI", "v1")
                .hash(HashFormat::Sha512),
        )
        .with_mod(TestMod::new("lithium"))
        .with_mod(TestMod::new("servidor").side(TestSide::Server))
        .with_file("config/sodium-options.json", "{\"quality\": 1}\n")
        .with_file("options.txt", "renderDistance:8\n")
}

/// Materializa esperando itens que falharam: o resto é feito e o relatório lista as falhas.
async fn incomplete(
    sources: &warden_instance::Sources,
    pack_dir: &Path,
    instance: &warden_instance::InstanceDirs,
) -> Error {
    let report = materialize_ok(sources, pack_dir, instance, &MaterializeOptions::default()).await;
    assert!(!report.is_complete(), "{report:?}");
    report.incomplete_error().expect("deveria haver falhas")
}

async fn requests(server: &MockServer) -> usize {
    server.received_requests().await.unwrap_or_default().len()
}

/// Datas de modificação de todos os arquivos da pasta.
fn mtimes(root: &Path) -> Vec<(String, std::time::SystemTime)> {
    tree(root)
        .keys()
        .filter(|path| !path.ends_with('/'))
        .map(|path| {
            (
                path.clone(),
                fs::metadata(root.join(path)).unwrap().modified().unwrap(),
            )
        })
        .collect()
}

#[tokio::test]
async fn criterio_3_segunda_vez_nao_baixa_nem_escreve() {
    let dir = temp_dir();
    let server = MockServer::start().await;
    let pack = sample_pack();
    pack.serve(&server).await;
    pack.write_to(&dir.path().join("pack"), &server.uri());
    let sources = sources(&dir.path().join("cache"), None);
    let instance = dirs(&dir.path().join("instancia"));
    let options = MaterializeOptions::default();

    let first = materialize_ok(&sources, &dir.path().join("pack"), &instance, &options).await;
    assert_eq!(first.written, 4, "{first:?}");
    assert_eq!(first.downloaded, 2);
    assert_eq!(first.skipped_side, 1);
    assert!(first.manifest_written);
    assert_eq!(requests(&server).await, 2);
    let manifest_before = fs::read(instance.state_dir.join(MANIFEST_FILE)).unwrap();
    let files_before = mtimes(&instance.game_dir);

    let second = materialize_ok(&sources, &dir.path().join("pack"), &instance, &options).await;
    assert_eq!(second.written, 0, "{second:?}");
    assert_eq!(second.downloaded, 0);
    assert_eq!(second.removed, 0);
    assert_eq!(second.unchanged, 4);
    assert!(!second.manifest_written);
    assert_eq!(
        requests(&server).await,
        2,
        "a segunda vez pediu algo à rede"
    );
    assert_eq!(
        fs::read(instance.state_dir.join(MANIFEST_FILE)).unwrap(),
        manifest_before
    );
    assert_eq!(mtimes(&instance.game_dir), files_before);
    assert!(!instance.game_dir.join("mods/servidor-1.0.jar").exists());

    // Outra instância com o mesmo cache: nada vem da rede.
    let other = dirs(&dir.path().join("outra"));
    let third = materialize_ok(&sources, &dir.path().join("pack"), &other, &options).await;
    assert_eq!((third.written, third.downloaded), (4, 0));
    assert_eq!(requests(&server).await, 2);
    assert_eq!(tree(&other.game_dir), tree(&instance.game_dir));
}

#[tokio::test]
async fn cache_guarda_os_quatro_hashes_e_a_origem() {
    let dir = temp_dir();
    let server = MockServer::start().await;
    let pack = sample_pack();
    pack.serve(&server).await;
    pack.write_to(&dir.path().join("pack"), &server.uri());
    let sources = sources(&dir.path().join("cache"), None);
    let instance = dirs(&dir.path().join("instancia"));
    materialize_ok(
        &sources,
        &dir.path().join("pack"),
        &instance,
        &MaterializeOptions::default(),
    )
    .await;
    let sodium = &pack.mods[0];
    let expected = warden_http::ExpectedHash::new(
        HashFormat::Sha1,
        hash_bytes(HashFormat::Sha1, &sodium.data),
    );
    let cached = sources.cache.find(&expected).unwrap().unwrap();
    assert_eq!(
        cached.hashes.sha512,
        hash_bytes(HashFormat::Sha512, &sodium.data)
    );
    assert_eq!(
        cached.hashes.sha256,
        hash_bytes(HashFormat::Sha256, &sodium.data)
    );
    assert_eq!(
        cached.hashes.murmur2.to_string(),
        hash_bytes(HashFormat::Murmur2, &sodium.data)
    );
    let origins = sources.cache.origins(&cached.hashes.sha256).unwrap();
    assert_eq!(origins.len(), 1);
    assert_eq!(origins[0].source, OriginSource::Modrinth);
    assert_eq!(origins[0].project.as_deref(), Some("AANobbMI"));
    assert_eq!(origins[0].version.as_deref(), Some("v1"));
    assert!(
        origins[0]
            .url
            .as_deref()
            .unwrap()
            .ends_with("/files/sodium-1.0.jar")
    );
    // O manifesto registra a origem.
    let manifest = Manifest::load(&instance.state_dir).unwrap();
    assert!(matches!(
        &manifest.files["mods/sodium-1.0.jar"].origin,
        EntryOrigin::Url { modrinth: Some(ids), .. } if ids.project == "AANobbMI"
    ));
    assert_eq!(
        manifest.files["config/sodium-options.json"].origin,
        EntryOrigin::Pack
    );
}

#[tokio::test]
async fn ca_t13_04_remover_mod_tira_o_jar_e_mantem_o_mundo() {
    let dir = temp_dir();
    let server = MockServer::start().await;
    let pack = sample_pack();
    pack.serve(&server).await;
    let pack_dir = dir.path().join("pack");
    pack.write_to(&pack_dir, &server.uri());
    let sources = sources(&dir.path().join("cache"), None);
    let instance = dirs(&dir.path().join("instancia"));
    let options = MaterializeOptions::default();
    materialize_ok(&sources, &pack_dir, &instance, &options).await;
    // O teste criou um mundo, um log e uma captura de tela.
    write(&instance.game_dir, "saves/Mundo/level.dat", b"mundo");
    write(&instance.game_dir, "logs/latest.log", b"log");
    write(&instance.game_dir, "screenshots/a.png", b"png");

    pack.clone()
        .without_mod("mods/lithium.pw.toml")
        .write_to(&pack_dir, &server.uri());
    let report = materialize_ok(&sources, &pack_dir, &instance, &options).await;
    assert_eq!(report.removed, 1, "{report:?}");
    assert!(!instance.game_dir.join("mods/lithium-1.0.jar").exists());
    assert!(instance.game_dir.join("mods/sodium-1.0.jar").exists());
    assert_eq!(
        fs::read(instance.game_dir.join("saves/Mundo/level.dat")).unwrap(),
        b"mundo"
    );
    assert!(instance.game_dir.join("logs/latest.log").exists());
    assert!(instance.game_dir.join("screenshots/a.png").exists());
    let manifest = Manifest::load(&instance.state_dir).unwrap();
    assert!(!manifest.files.contains_key("mods/lithium-1.0.jar"));
}

#[tokio::test]
async fn atualizacao_troca_o_arquivo_e_apaga_o_antigo() {
    let dir = temp_dir();
    let server = MockServer::start().await;
    let pack = sample_pack();
    let updated = pack
        .clone()
        .replace_mod(TestMod::new("lithium").version("2.0"));
    pack.serve(&server).await;
    updated.serve(&server).await;
    let pack_dir = dir.path().join("pack");
    pack.write_to(&pack_dir, &server.uri());
    let sources = sources(&dir.path().join("cache"), None);
    let instance = dirs(&dir.path().join("instancia"));
    let options = MaterializeOptions::default();
    materialize_ok(&sources, &pack_dir, &instance, &options).await;
    updated.write_to(&pack_dir, &server.uri());
    let report = materialize_ok(&sources, &pack_dir, &instance, &options).await;
    assert_eq!(
        (report.written, report.removed, report.downloaded),
        (1, 1, 1)
    );
    assert!(!instance.game_dir.join("mods/lithium-1.0.jar").exists());
    assert!(instance.game_dir.join("mods/lithium-2.0.jar").exists());
}

#[tokio::test]
async fn arquivo_alterado_pausa_para_revisao_sem_mexer_em_nada() {
    let dir = temp_dir();
    let server = MockServer::start().await;
    let pack = sample_pack();
    pack.serve(&server).await;
    let pack_dir = dir.path().join("pack");
    pack.write_to(&pack_dir, &server.uri());
    let sources = sources(&dir.path().join("cache"), None);
    let instance = dirs(&dir.path().join("instancia"));
    materialize_ok(
        &sources,
        &pack_dir,
        &instance,
        &MaterializeOptions::default(),
    )
    .await;

    // O jogo mudou a config e o options.txt; o pack mudou a config e tirou o options.txt.
    write(
        &instance.game_dir,
        "config/sodium-options.json",
        b"{\"quality\": 3}\n",
    );
    write(&instance.game_dir, "options.txt", b"renderDistance:16\n");
    // Arquivo desconhecido no lugar de um item novo do pack.
    write(
        &instance.game_dir,
        "config/novo.toml",
        b"gerado pelo jogo\n",
    );
    let mut changed = pack
        .clone()
        .with_file("config/sodium-options.json", "{\"quality\": 2}\n");
    changed.files.remove("options.txt");
    let changed = changed.with_file("config/novo.toml", "do pack\n");
    changed.write_to(&pack_dir, &server.uri());
    let before = tree(&instance.game_dir);
    let manifest_before = fs::read(instance.state_dir.join(MANIFEST_FILE)).unwrap();

    let outcome = materialize(
        &sources,
        &pack_dir,
        &instance,
        &MaterializeOptions::default(),
        &NoProgress,
        &CancellationToken::new(),
    )
    .await
    .unwrap();
    let Outcome::NeedsReview(mut modified) = outcome else {
        panic!("deveria pausar: {outcome:?}");
    };
    modified.sort_by(|a, b| a.path.cmp(&b.path));
    let summary: Vec<(&str, ModifiedAction, bool)> = modified
        .iter()
        .map(|file| (file.path.as_str(), file.action, file.managed))
        .collect();
    assert_eq!(
        summary,
        [
            ("config/novo.toml", ModifiedAction::Overwrite, false),
            (
                "config/sodium-options.json",
                ModifiedAction::Overwrite,
                true
            ),
            ("options.txt", ModifiedAction::Remove, true),
        ]
    );
    assert_eq!(
        tree(&instance.game_dir),
        before,
        "a pausa mexeu na instância"
    );
    assert_eq!(
        fs::read(instance.state_dir.join(MANIFEST_FILE)).unwrap(),
        manifest_before
    );

    // Depois da revisão: sobrescreve.
    let options = MaterializeOptions {
        on_modified: OnModified::Overwrite,
        ..MaterializeOptions::default()
    };
    let report = materialize_ok(&sources, &pack_dir, &instance, &options).await;
    assert_eq!((report.written, report.removed), (2, 1), "{report:?}");
    assert_eq!(
        fs::read(instance.game_dir.join("config/sodium-options.json")).unwrap(),
        b"{\"quality\": 2}\n"
    );
    assert!(!instance.game_dir.join("options.txt").exists());
}

#[tokio::test]
async fn alteracao_sem_mudanca_no_pack_tambem_pede_revisao() {
    // O packwiz-installer mantém o arquivo que o jogo mudou enquanto o item não muda no pack;
    // o Warden devolve a instância ao estado do pack, depois da revisão (T15, "Descartar").
    let dir = temp_dir();
    let server = MockServer::start().await;
    let pack = sample_pack();
    pack.serve(&server).await;
    let pack_dir = dir.path().join("pack");
    pack.write_to(&pack_dir, &server.uri());
    let sources = sources(&dir.path().join("cache"), None);
    let instance = dirs(&dir.path().join("instancia"));
    materialize_ok(
        &sources,
        &pack_dir,
        &instance,
        &MaterializeOptions::default(),
    )
    .await;
    write(&instance.game_dir, "options.txt", b"renderDistance:12\n");
    let outcome = materialize(
        &sources,
        &pack_dir,
        &instance,
        &MaterializeOptions::default(),
        &NoProgress,
        &CancellationToken::new(),
    )
    .await
    .unwrap();
    assert!(matches!(&outcome, Outcome::NeedsReview(files) if files.len() == 1));
    let options = MaterializeOptions {
        on_modified: OnModified::Overwrite,
        ..MaterializeOptions::default()
    };
    let report = materialize_ok(&sources, &pack_dir, &instance, &options).await;
    assert_eq!(report.written, 1);
    assert_eq!(
        fs::read(instance.game_dir.join("options.txt")).unwrap(),
        b"renderDistance:8\n"
    );
}

#[tokio::test]
async fn opcionais_seguem_o_padrao_e_a_escolha_gravada() {
    let dir = temp_dir();
    let server = MockServer::start().await;
    let pack = TestPack::default()
        .with_mod(TestMod::new("desligado").optional(false))
        .with_mod(TestMod::new("ligado").optional(true));
    pack.serve(&server).await;
    let pack_dir = dir.path().join("pack");
    pack.write_to(&pack_dir, &server.uri());
    let sources = sources(&dir.path().join("cache"), None);
    let instance = dirs(&dir.path().join("instancia"));
    let options = MaterializeOptions::default();
    let report = materialize_ok(&sources, &pack_dir, &instance, &options).await;
    assert_eq!((report.written, report.skipped_disabled), (1, 1));
    assert!(instance.game_dir.join("mods/ligado-1.0.jar").exists());
    assert!(!instance.game_dir.join("mods/desligado-1.0.jar").exists());

    let mut choices = OptionalChoices::default();
    choices.set("mods/desligado.pw.toml", true);
    choices.set("mods/ligado.pw.toml", false);
    choices.save(&instance.state_dir).unwrap();
    let report = materialize_ok(&sources, &pack_dir, &instance, &options).await;
    assert_eq!((report.written, report.removed), (1, 1));
    assert!(!instance.game_dir.join("mods/ligado-1.0.jar").exists());
    assert!(instance.game_dir.join("mods/desligado-1.0.jar").exists());
}

#[tokio::test]
async fn indice_desatualizado_copia_o_arquivo_do_pack_e_avisa() {
    let dir = temp_dir();
    let server = MockServer::start().await;
    let pack = sample_pack();
    pack.serve(&server).await;
    let pack_dir = dir.path().join("pack");
    pack.write_to(&pack_dir, &server.uri());
    // Editado sem `packwiz refresh`.
    fs::write(pack_dir.join("options.txt"), b"renderDistance:20\n").unwrap();
    let sources = sources(&dir.path().join("cache"), None);
    let instance = dirs(&dir.path().join("instancia"));
    let options = MaterializeOptions::default();
    let report = materialize_ok(&sources, &pack_dir, &instance, &options).await;
    assert_eq!(report.stale_index, ["options.txt"]);
    assert_eq!(
        fs::read(instance.game_dir.join("options.txt")).unwrap(),
        b"renderDistance:20\n"
    );
    // Sem mudanças: nada de novo.
    let again = materialize_ok(&sources, &pack_dir, &instance, &options).await;
    assert_eq!(
        (again.written, again.manifest_written),
        (0, false),
        "{again:?}"
    );
}

#[tokio::test]
async fn falha_parcial_instala_o_resto_e_lista_o_que_falta() {
    let dir = temp_dir();
    let server = MockServer::start().await;
    let pack = sample_pack();
    // Só o sodium é servido; o lithium dá 404.
    Mock::given(method("GET"))
        .and(path(pack.mods[0].served_path()))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(pack.mods[0].data.clone()))
        .mount(&server)
        .await;
    let pack_dir = dir.path().join("pack");
    pack.write_to(&pack_dir, &server.uri());
    let sources = sources(&dir.path().join("cache"), None);
    let instance = dirs(&dir.path().join("instancia"));
    let error = incomplete(&sources, &pack_dir, &instance).await;
    let Error::Incomplete { items } = &error else {
        panic!("{error:?}");
    };
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].path, "mods/lithium.pw.toml");
    assert_eq!(items[0].name, "LITHIUM");
    assert!(matches!(
        items[0].reason,
        FailureReason::DownloadFailed {
            retryable: false,
            ..
        }
    ));
    assert_eq!(error.params()["names"], "LITHIUM");
    assert!(instance.game_dir.join("mods/sodium-1.0.jar").exists());
    assert!(!instance.game_dir.join("mods/lithium-1.0.jar").exists());
    // O manifesto registra o que foi instalado.
    let manifest = Manifest::load(&instance.state_dir).unwrap();
    assert!(manifest.files.contains_key("mods/sodium-1.0.jar"));
    assert!(!manifest.files.contains_key("mods/lithium-1.0.jar"));
}

#[tokio::test]
async fn hash_errado_do_servidor_nao_chega_a_instancia() {
    let dir = temp_dir();
    let server = MockServer::start().await;
    let pack = TestPack::default().with_mod(TestMod::new("lithium"));
    Mock::given(method("GET"))
        .and(path(pack.mods[0].served_path()))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"outro conteudo".to_vec()))
        .mount(&server)
        .await;
    let pack_dir = dir.path().join("pack");
    pack.write_to(&pack_dir, &server.uri());
    let sources = sources(&dir.path().join("cache"), None);
    let instance = dirs(&dir.path().join("instancia"));
    let error = incomplete(&sources, &pack_dir, &instance).await;
    assert!(
        matches!(&error, Error::Incomplete { items } if items.len() == 1),
        "{error:?}"
    );
    assert!(!instance.game_dir.join("mods/lithium-1.0.jar").exists());
    let leftovers: Vec<String> = tree(&dir.path().join("cache"))
        .into_keys()
        .filter(|path| !path.ends_with('/') && !path.starts_with("index.sqlite"))
        .collect();
    assert!(leftovers.is_empty(), "sobrou no cache: {leftovers:?}");
}

#[tokio::test]
async fn ca_t22_01_cancelar_para_em_ate_2_s_sem_arquivo_incompleto() {
    let dir = temp_dir();
    let server = MockServer::start().await;
    let pack = TestPack::default()
        .with_mod(TestMod::new("rapido"))
        .with_mod(TestMod::new("lento"));
    Mock::given(method("GET"))
        .and(path(pack.mods[0].served_path()))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(pack.mods[0].data.clone()))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path(pack.mods[1].served_path()))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_bytes(pack.mods[1].data.clone())
                .set_delay(Duration::from_secs(60)),
        )
        .mount(&server)
        .await;
    let pack_dir = dir.path().join("pack");
    pack.write_to(&pack_dir, &server.uri());
    let sources = sources(&dir.path().join("cache"), None);
    let instance = dirs(&dir.path().join("instancia"));
    let cancel = CancellationToken::new();
    let trigger = cancel.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(500)).await;
        trigger.cancel();
    });
    let started = Instant::now();
    let error = materialize(
        &sources,
        &pack_dir,
        &instance,
        &MaterializeOptions::default(),
        &NoProgress,
        &cancel,
    )
    .await
    .unwrap_err();
    let elapsed = started.elapsed();
    assert!(matches!(error, Error::Cancelled), "{error:?}");
    assert!(
        elapsed < Duration::from_millis(2500),
        "cancelar levou {elapsed:?}"
    );
    assert!(elapsed.saturating_sub(Duration::from_millis(500)) < Duration::from_secs(2));
    // Nenhum arquivo incompleto com nome final, nem temporário, na instância.
    let files: Vec<String> = tree(&instance.game_dir)
        .into_keys()
        .filter(|path| !path.ends_with('/'))
        .collect();
    assert!(
        files
            .iter()
            .all(|path| !path.contains("lento") && !path.ends_with(".warden-tmp")),
        "{files:?}"
    );
    // No cache, só `.part` (ou nada) do download interrompido.
    for path in tree(&dir.path().join("cache")).into_keys() {
        if path.contains("incoming/") && !path.ends_with('/') {
            assert!(path.ends_with(".part"), "{path}");
        }
    }
    // Retomar depois funciona.
    server.reset().await;
    pack.serve(&server).await;
    let report = materialize_ok(
        &sources,
        &pack_dir,
        &instance,
        &MaterializeOptions::default(),
    )
    .await;
    assert!(
        instance.game_dir.join("mods/lento-1.0.jar").exists(),
        "{report:?}"
    );
}

/// Pack com um mod da CurseForge (JEI, permitido) e um bloqueado (Entity Culling).
fn curseforge_pack() -> TestPack {
    TestPack::default()
        .with_mod(TestMod::new("jei").curseforge(238_222, 5_101_366))
        .with_mod(TestMod::new("entityculling").curseforge(448_233, 6_000_001))
        .with_mod(TestMod::new("sodium"))
}

async fn mount_curseforge(server: &MockServer, pack: &TestPack) {
    let jei = &pack.mods[0];
    let culling = &pack.mods[1];
    let cdn = format!("{}/cdn/files/5101/366/{}", server.uri(), jei.filename);
    Mock::given(method("POST"))
        .and(path("/v1/mods/files"))
        .and(header("x-api-key", KEY))
        .respond_with(cf_data(json!([
            cf_file(238_222, 5_101_366, &jei.filename, &jei.data, Some(&cdn)),
            cf_file(448_233, 6_000_001, &culling.filename, &culling.data, None),
        ])))
        .mount(server)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/mods"))
        .respond_with(cf_data(json!([cf_project(
            448_233,
            "entityculling",
            false
        )])))
        .mount(server)
        .await;
    Mock::given(method("GET"))
        .and(path(format!("/cdn/files/5101/366/{}", jei.filename)))
        .and(header("x-api-key", KEY))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(jei.data.clone()))
        .mount(server)
        .await;
}

#[tokio::test]
async fn curseforge_na_hora_com_chave_na_cdn_e_bloqueado_para_a_t20() {
    let dir = temp_dir();
    let server = MockServer::start().await;
    let pack = curseforge_pack();
    mount_curseforge(&server, &pack).await;
    pack.serve(&server).await;
    let pack_dir = dir.path().join("pack");
    pack.write_to(&pack_dir, &server.uri());
    let sources = sources(
        &dir.path().join("cache"),
        Some(curseforge(&server, Some(KEY))),
    );
    let instance = dirs(&dir.path().join("instancia"));
    let options = MaterializeOptions::default();
    let report = materialize_ok(&sources, &pack_dir, &instance, &options).await;
    assert_eq!(report.written, 2, "{report:?}");
    assert!(instance.game_dir.join("mods/jei-1.0.jar").exists());
    assert!(
        !instance
            .game_dir
            .join("mods/entityculling-1.0.jar")
            .exists()
    );
    // O download da CDN levou a chave (o mock só responde com ela).
    let cdn_requests: Vec<_> = server
        .received_requests()
        .await
        .unwrap()
        .into_iter()
        .filter(|request| request.url.path().starts_with("/cdn/"))
        .collect();
    assert_eq!(cdn_requests.len(), 1);
    assert!(cdn_requests[0].headers.get("x-api-key").is_some());
    // A chave não foi para o servidor dos mods por link? (mesmo servidor de teste aqui: o
    // `warden-curseforge` só manda para a CurseForge; ver os testes dela.)

    // T20: a lista do bloqueado, com a página do download manual.
    assert_eq!(report.blocked.len(), 1);
    let blocked = &report.blocked[0];
    assert_eq!(blocked.metafile, "mods/entityculling.pw.toml");
    assert_eq!(blocked.file_name, "entityculling-1.0.jar");
    assert_eq!(
        blocked.page_url.as_deref(),
        Some("https://www.curseforge.com/minecraft/mc-mods/entityculling/files/6000001")
    );

    // O manifesto e o índice do cache nunca guardam o endereço da CurseForge.
    let manifest_text = fs::read_to_string(instance.state_dir.join(MANIFEST_FILE)).unwrap();
    assert!(!manifest_text.contains("/cdn/"), "{manifest_text}");
    let jei = &pack.mods[0];
    let cached = sources
        .cache
        .find(&warden_http::ExpectedHash::new(
            HashFormat::Sha1,
            hash_bytes(HashFormat::Sha1, &jei.data),
        ))
        .unwrap()
        .unwrap();
    let origins = sources.cache.origins(&cached.hashes.sha256).unwrap();
    assert_eq!(origins[0].source, OriginSource::Curseforge);
    assert_eq!(origins[0].url, None);
    assert_eq!(origins[0].project.as_deref(), Some("238222"));

    // CA-T20-01/02: o arquivo certo escolhido à mão entra pelo cache, nunca pelo pack.
    let downloaded = dir.path().join("Downloads").join("entityculling-1.0.jar");
    write(
        &dir.path().join("Downloads"),
        "entityculling-1.0.jar",
        &pack.mods[1].data,
    );
    accept_manual_file(&sources.cache, blocked, &downloaded).unwrap();
    let before = server.received_requests().await.unwrap().len();
    let report = materialize_ok(&sources, &pack_dir, &instance, &options).await;
    assert_eq!((report.written, report.downloaded), (1, 0), "{report:?}");
    assert!(report.blocked.is_empty());
    assert!(
        instance
            .game_dir
            .join("mods/entityculling-1.0.jar")
            .exists()
    );
    assert_eq!(
        server.received_requests().await.unwrap().len(),
        before,
        "tudo veio do cache"
    );
    let in_pack: Vec<String> = tree(&pack_dir)
        .into_keys()
        .filter(|path| path.ends_with(".jar"))
        .collect();
    assert!(in_pack.is_empty(), "{in_pack:?}");
}

#[tokio::test]
async fn curseforge_sem_chave_instala_o_resto_e_avisa() {
    let dir = temp_dir();
    let server = MockServer::start().await;
    let pack = curseforge_pack();
    pack.serve(&server).await;
    let pack_dir = dir.path().join("pack");
    pack.write_to(&pack_dir, &server.uri());
    let sources = sources(&dir.path().join("cache"), Some(curseforge(&server, None)));
    let instance = dirs(&dir.path().join("instancia"));
    let error = incomplete(&sources, &pack_dir, &instance).await;
    let Error::Incomplete { items } = &error else {
        panic!("{error:?}");
    };
    assert_eq!(items.len(), 2);
    assert!(
        items
            .iter()
            .all(|item| item.reason == FailureReason::CurseforgeKeyMissing)
    );
    assert!(!error.retryable());
    assert!(instance.game_dir.join("mods/sodium-1.0.jar").exists());
    // Nenhuma requisição à API sem chave.
    assert!(
        server
            .received_requests()
            .await
            .unwrap()
            .iter()
            .all(|request| !request.url.path().starts_with("/v1/"))
    );
}

#[tokio::test]
async fn curseforge_no_cache_nao_consulta_a_api() {
    let dir = temp_dir();
    let server = MockServer::start().await;
    let pack = TestPack::default().with_mod(TestMod::new("jei").curseforge(238_222, 5_101_366));
    let pack_dir = dir.path().join("pack");
    pack.write_to(&pack_dir, &server.uri());
    let sources = sources(&dir.path().join("cache"), None);
    // O jar já está no cache (de outro pack, por exemplo).
    let source = dir.path().join("jei.jar");
    fs::write(&source, &pack.mods[0].data).unwrap();
    sources
        .cache
        .import_file(
            &source,
            &warden_http::ExpectedHash::new(
                HashFormat::Sha1,
                hash_bytes(HashFormat::Sha1, &pack.mods[0].data),
            ),
            &warden_instance::FileOrigin::curseforge(238_222, 5_101_366),
        )
        .unwrap()
        .unwrap();
    let instance = dirs(&dir.path().join("instancia"));
    // Sem cliente da CurseForge e sem rede: funciona (SPEC T13, sem internet).
    let report = materialize_ok(
        &sources,
        &pack_dir,
        &instance,
        &MaterializeOptions::default(),
    )
    .await;
    assert_eq!((report.written, report.downloaded), (1, 0));
    assert!(server.received_requests().await.unwrap().is_empty());
}

#[tokio::test]
async fn lado_desconhecido_e_metafile_quebrado_viram_falha_do_item() {
    let dir = temp_dir();
    let server = MockServer::start().await;
    let pack = TestPack::default().with_mod(TestMod::new("ok"));
    pack.serve(&server).await;
    let pack_dir = dir.path().join("pack");
    pack.write_to(&pack_dir, &server.uri());
    // Metafile com lado desconhecido e outro quebrado, entrando no índice.
    write(
        &pack_dir,
        "mods/estranho.pw.toml",
        b"name = \"Estranho\"\nfilename = \"e.jar\"\nside = \"ambos\"\n\n[download]\nurl = \"https://x/e.jar\"\nhash-format = \"sha1\"\nhash = \"00\"\n",
    );
    write(&pack_dir, "mods/quebrado.pw.toml", b"name = ");
    let mut index = fs::read_to_string(pack_dir.join("index.toml")).unwrap();
    index.push_str("\n[[files]]\nfile = \"mods/estranho.pw.toml\"\nhash = \"\"\nmetafile = true\n");
    index.push_str("\n[[files]]\nfile = \"mods/quebrado.pw.toml\"\nhash = \"\"\nmetafile = true\n");
    fs::write(pack_dir.join("index.toml"), index).unwrap();
    let sources = sources(&dir.path().join("cache"), None);
    let instance = dirs(&dir.path().join("instancia"));
    let error = incomplete(&sources, &pack_dir, &instance).await;
    let Error::Incomplete { items } = &error else {
        panic!("{error:?}");
    };
    let mut reasons: Vec<(&str, bool)> = items
        .iter()
        .map(|item| {
            (
                item.path.as_str(),
                matches!(item.reason, FailureReason::UnknownSide { .. }),
            )
        })
        .collect();
    reasons.sort_unstable();
    assert_eq!(
        reasons,
        [
            ("mods/estranho.pw.toml", true),
            ("mods/quebrado.pw.toml", false)
        ]
    );
    assert!(instance.game_dir.join("mods/ok-1.0.jar").exists());
}

#[tokio::test]
async fn pack_sem_pack_toml_e_erro_de_leitura() {
    let dir = temp_dir();
    let sources = sources(&dir.path().join("cache"), None);
    let error = materialize(
        &sources,
        &dir.path().join("nao-existe"),
        &dirs(&dir.path().join("instancia")),
        &MaterializeOptions::default(),
        &NoProgress,
        &CancellationToken::new(),
    )
    .await
    .unwrap_err();
    assert!(matches!(error, Error::Pack(_)), "{error:?}");
}

#[tokio::test(flavor = "multi_thread")]
async fn duas_instancias_ao_mesmo_tempo_baixam_cada_arquivo_uma_vez() {
    let dir = temp_dir();
    let server = MockServer::start().await;
    let pack = TestPack::default()
        .with_mod(TestMod::new("a"))
        .with_mod(TestMod::new("b"));
    for test_mod in &pack.mods {
        Mock::given(method("GET"))
            .and(path(test_mod.served_path()))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_bytes(test_mod.data.clone())
                    .set_delay(Duration::from_millis(300)),
            )
            .mount(&server)
            .await;
    }
    let pack_dir = dir.path().join("pack");
    pack.write_to(&pack_dir, &server.uri());
    let sources = sources(&dir.path().join("cache"), None);
    let one = dirs(&dir.path().join("um"));
    let two = dirs(&dir.path().join("dois"));
    let options = MaterializeOptions::default();
    let (first, second) = tokio::join!(
        materialize_ok(&sources, &pack_dir, &one, &options),
        materialize_ok(&sources, &pack_dir, &two, &options),
    );
    assert_eq!(
        first.downloaded + second.downloaded,
        2,
        "{first:?} {second:?}"
    );
    assert_eq!(requests(&server).await, 2);
    assert_eq!(tree(&one.game_dir), tree(&two.game_dir));
    assert_eq!(tree(&one.game_dir).len(), 3);
}
