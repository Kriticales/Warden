//! Ferramentas do jogador na materialização (ROADMAP P1-18; SPEC T13; CA-T13-15; decisão D16).
//!
//! O pack tem o spark e o Crash Assistant marcados em `.warden/project.toml`. Num teste normal o
//! jar do Crash Assistant não vai para a instância e o spark vai; o `index.toml` do pack continua
//! com os dois. "Testar como o jogador recebe" leva os dois; a busca do culpado leva nenhum, a
//! menos que estejam entre os suspeitos.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod conformance_support;

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use conformance_support::{TestMod, TestPack, dirs, materialize_ok, sources, temp_dir};
use warden_instance::{MaterializeOptions, PlayerToolsMode};
use wiremock::MockServer;

const PROJECT: &str = "schemaVersion = 1\nid = \"01JABCDEFGHJKMNPQRSTVWXYZ0\"\n\n[player-tools]\nspark = \"mods/spark.pw.toml\"\ncrash-assistant = \"mods/crash-assistant.pw.toml\"\n";

async fn pack_on_disk(dir: &Path, server: &MockServer) {
    let pack = TestPack::default()
        .with_mod(TestMod::new("sodium"))
        .with_mod(TestMod::new("spark"))
        .with_mod(TestMod::new("crash-assistant"));
    pack.serve(server).await;
    pack.write_to(&dir.join("pack"), &server.uri());
    fs::create_dir_all(dir.join("pack/.warden")).unwrap();
    fs::write(dir.join("pack/.warden/project.toml"), PROJECT).unwrap();
}

fn mods(game_dir: &Path) -> Vec<String> {
    let mut found: Vec<String> = fs::read_dir(game_dir.join("mods"))
        .map(|entries| {
            entries
                .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default();
    found.sort();
    found
}

async fn run(mode: PlayerToolsMode) -> (Vec<String>, warden_instance::Report) {
    let dir = temp_dir();
    let server = MockServer::start().await;
    pack_on_disk(dir.path(), &server).await;
    let sources = sources(&dir.path().join("cache"), None);
    let instance = dirs(&dir.path().join("instancia"));
    let options = MaterializeOptions {
        player_tools: mode,
        ..MaterializeOptions::default()
    };
    let report = materialize_ok(&sources, &dir.path().join("pack"), &instance, &options).await;
    // O pack nunca muda: o índice continua com os três.
    let index = fs::read_to_string(dir.path().join("pack/index.toml")).unwrap();
    for stem in ["sodium", "spark", "crash-assistant"] {
        assert!(index.contains(&format!("mods/{stem}.pw.toml")), "{index}");
    }
    (mods(&instance.game_dir), report)
}

#[tokio::test]
async fn teste_normal_leva_o_spark_e_deixa_o_crash_assistant_de_fora() {
    // CA-T13-15.
    let (found, report) = run(PlayerToolsMode::Normal).await;
    assert_eq!(found, ["sodium-1.0.jar", "spark-1.0.jar"]);
    assert_eq!(report.skipped_player_tools, 1);
    assert!(report.is_complete(), "{report:?}");
}

#[tokio::test]
async fn testar_como_o_jogador_recebe_leva_os_dois() {
    let (found, report) = run(PlayerToolsMode::AsPlayer).await;
    assert_eq!(
        found,
        ["crash-assistant-1.0.jar", "sodium-1.0.jar", "spark-1.0.jar"]
    );
    assert_eq!(report.skipped_player_tools, 0);
}

#[tokio::test]
async fn busca_do_culpado_deixa_os_dois_de_fora_menos_os_suspeitos() {
    let (found, report) = run(PlayerToolsMode::Culprit {
        suspects: BTreeSet::new(),
    })
    .await;
    assert_eq!(found, ["sodium-1.0.jar"]);
    assert_eq!(report.skipped_player_tools, 2);

    let (found, _) = run(PlayerToolsMode::Culprit {
        suspects: BTreeSet::from(["mods/spark.pw.toml".to_owned()]),
    })
    .await;
    assert_eq!(found, ["sodium-1.0.jar", "spark-1.0.jar"]);
}

#[tokio::test]
async fn trocar_de_modo_na_mesma_instancia_coloca_e_tira_o_crash_assistant() {
    let dir = temp_dir();
    let server = MockServer::start().await;
    pack_on_disk(dir.path(), &server).await;
    let sources = sources(&dir.path().join("cache"), None);
    let instance = dirs(&dir.path().join("instancia"));
    let pack = dir.path().join("pack");
    let with = |mode| MaterializeOptions {
        player_tools: mode,
        ..MaterializeOptions::default()
    };
    materialize_ok(&sources, &pack, &instance, &with(PlayerToolsMode::AsPlayer)).await;
    assert_eq!(mods(&instance.game_dir).len(), 3);
    // Teste normal depois: o Crash Assistant sai, como qualquer item que saiu do pack.
    let report = materialize_ok(&sources, &pack, &instance, &with(PlayerToolsMode::Normal)).await;
    assert_eq!(report.removed, 1, "{report:?}");
    assert_eq!(
        mods(&instance.game_dir),
        ["sodium-1.0.jar", "spark-1.0.jar"]
    );
}
