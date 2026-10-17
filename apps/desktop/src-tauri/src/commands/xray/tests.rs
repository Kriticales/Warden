//! Testes do raio-x de mixins no app (D-10): o aviso `W_MIXIN_OVERLAP` na seção Problemas pelo
//! mesmo "Verificar agora" da D-03 (CA-T14-09), o relatório de um mod nos detalhes e o elevador
//! "o último travamento citou a config", com o packwiz real e jars sintéticos de verdade (classes
//! escritas pelo mesmo gerador dos testes da `warden-mixin`).

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use warden_core::PackId;
use warden_diagnostics::{Evidence, ItemRef, Severity};
use warden_launcher::outcome::{CrashArtifacts, Outcome};
use warden_launcher::session::{SessionDir, SessionRecord};
use warden_launcher::{GameSpec, LoaderSpec};
use warden_mixin::overlap::{Elevator, Risk};
use warden_packwiz_cli::Packwiz;
use warden_project::registry::PackRecord;
use warden_project::transaction::PackTransaction;

use super::*;
use crate::commands::diagnostics::check;
use crate::commands::diagnostics::report::CheckPass;
use crate::commands::inventory::tests::{packwiz_binary, test_packwiz};
use crate::error::ErrorCode;
use crate::state::tests::test_state;

#[allow(dead_code, unreachable_pub, clippy::expect_used, clippy::panic)]
mod classgen {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../crates/warden-mixin/tests/common/classgen.rs"
    ));
}

use classgen::{JarBuilder, MixinClass, at, fabric_mod_json, injector, mixin_config};

const TARGET: &str = "net/minecraft/class_1";
const CALL: &str = "Lnet/minecraft/class_2;method_3()V";

/// Um mod Fabric com uma classe de mixin que faz `@Redirect` (ou `@Inject` no começo) no mesmo
/// método do jogo.
fn mixin_jar(id: &str, redirect: bool) -> Vec<u8> {
    let annotation = if redirect {
        injector("Redirect", "method_9", at("INVOKE", CALL))
    } else {
        injector("Inject", "method_9", at("HEAD", ""))
    };
    let class = MixinClass::new(&format!("x/{id}/mixin/M"), TARGET).method("m", "()V", annotation);
    JarBuilder::new()
        .file(
            "fabric.mod.json",
            fabric_mod_json(id, "1.0", &[&format!("{id}.mixins.json")], &[], &[]),
        )
        .file(
            &format!("{id}.mixins.json"),
            mixin_config(&format!("x.{id}.mixin"), &["M"], None),
        )
        .class(&class)
        .build()
}

/// Pack Fabric 1.20.1 com os arquivos dados; o packwiz real grava o índice.
async fn fabric_pack(
    state: &AppState,
    dir: &Path,
    cli: &Packwiz,
    files: &[(&str, &[u8])],
) -> (PackId, PathBuf) {
    let root = dir.join("pack");
    fs::create_dir_all(&root).unwrap();
    let mut manifest = warden_packwiz::PackManifest::new("Raio-x", "1.20.1");
    manifest
        .versions
        .get_or_insert_with(BTreeMap::new)
        .insert("fabric".into(), "0.16.10".into());
    fs::write(root.join("pack.toml"), manifest.to_toml_string()).unwrap();
    fs::write(
        root.join("index.toml"),
        warden_packwiz::PackIndex::default().to_toml_string(),
    )
    .unwrap();
    for (path, bytes) in files {
        let file = root.join(path);
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(file, bytes).unwrap();
    }
    PackTransaction::new(root.clone())
        .commit(cli, &warden_core::CancellationToken::new())
        .await
        .unwrap();
    let record = PackRecord {
        id: PackId::new(),
        name: "Raio-x".into(),
        path: root.clone(),
        last_test: None,
        extra: BTreeMap::new(),
    };
    let id = record.id;
    state.packs.insert(record).unwrap();
    (id, root)
}

fn setup() -> Option<(tempfile::TempDir, AppState, Packwiz)> {
    let binary = packwiz_binary()?;
    let dir = tempfile::tempdir().unwrap();
    let state = test_state(dir.path());
    let cli = test_packwiz(&state, binary);
    Some((dir, state, cli))
}

/// Uma sessão do Testar que travou, com o `output.log` dado.
fn crashed_session(state: &AppState, pack: PackId, log: &str) {
    let sessions = InstanceDirs::for_pack(&state.paths, pack)
        .state_dir
        .join("sessions");
    let dir = SessionDir::create(&sessions, std::time::SystemTime::now()).unwrap();
    fs::write(dir.output_log(), log).unwrap();
    dir.write_record(&SessionRecord {
        schema: 1,
        started_at: "2026-10-13T10:00:00Z".into(),
        ended_at: "2026-10-13T10:01:00Z".into(),
        duration_ms: 60_000,
        outcome: Outcome::Crashed,
        exit_code: Some(1),
        pack_version: None,
        game: GameSpec::new(
            "1.20.1",
            LoaderSpec::Fabric {
                version: "0.16.10".into(),
            },
        )
        .unwrap(),
        java_major: 17,
        artifacts: CrashArtifacts::default(),
        extra: serde_json::Map::new(),
    })
    .unwrap();
}

fn mixin_findings(report: &crate::commands::diagnostics::report::DiagnosticsReport) -> Vec<&crate::commands::diagnostics::report::ReportFinding> {
    report
        .findings
        .iter()
        .filter(|entry| entry.finding.rule().as_str() == "W_MIXIN_OVERLAP")
        .collect()
}

/// CA-T14-09: dois mods com `@Redirect` no mesmo ponto geram o aviso "alteram o mesmo ponto do
/// jogo" na passagem completa, com a config de mixin e o ponto de cada um; o aviso nunca
/// bloqueia o teste.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn dois_redirect_no_mesmo_ponto_viram_aviso_que_nao_bloqueia() {
    let Some((dir, state, cli)) = setup() else {
        return;
    };
    let (a, b) = (mixin_jar("aaa", true), mixin_jar("bbb", true));
    let (pack, _root) = fabric_pack(
        &state,
        dir.path(),
        &cli,
        &[("mods/aaa.jar", &a), ("mods/bbb.jar", &b)],
    )
    .await;

    let quick = check(&state, pack, CheckPass::Quick, None).await.unwrap();
    assert!(mixin_findings(&quick).is_empty(), "a rápida não lê jars");

    let report = check(&state, pack, CheckPass::Complete, None).await.unwrap();
    let found = mixin_findings(&report);
    assert_eq!(found.len(), 1, "{report:#?}");
    let warning = found[0];
    assert_eq!(warning.finding.severity(), Severity::Warning);
    assert_eq!(report.blocking_errors(), 0, "nunca bloqueia o Testar");
    let paths: Vec<_> = warning.items.iter().filter_map(|i| i.path.clone()).collect();
    assert_eq!(paths, vec!["mods/aaa.jar", "mods/bbb.jar"]);
    assert_eq!(warning.finding.param("risco"), Some("alto"));
    assert_eq!(warning.finding.param("ponto"), Some("class_1.method_9"));
    for config in ["aaa.mixins.json", "bbb.mixins.json"] {
        assert!(
            warning.finding.evidence().iter().any(|e| matches!(e,
                Evidence::JarMetadata { file_in_jar, excerpt, .. }
                    if file_in_jar == config && excerpt.starts_with("@Redirect em class_1.method_9 (INVOKE"))),
            "{config}: {:?}",
            warning.finding.evidence()
        );
    }
    // O cache do raio-x fica num arquivo só, pelo id do pack.
    assert!(
        state
            .paths
            .cache_dir()
            .join("xray")
            .join(format!("{pack}.bin"))
            .is_file()
    );
}

/// CA-T14-09 (fim): um travamento cuja pilha cita a config de um deles sobe o risco para alto e
/// cita a linha do log; o relatório do mod nos detalhes mostra o mesmo.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn travamento_que_cita_a_config_sobe_para_alto_com_a_linha() {
    let Some((dir, state, cli)) = setup() else {
        return;
    };
    let (a, b) = (mixin_jar("aaa", false), mixin_jar("bbb", false));
    let (pack, _root) = fabric_pack(
        &state,
        dir.path(),
        &cli,
        &[("mods/aaa.jar", &a), ("mods/bbb.jar", &b)],
    )
    .await;
    let before = check(&state, pack, CheckPass::Complete, None).await.unwrap();
    assert!(mixin_findings(&before).is_empty(), "só @Inject: baixo, sem aviso");

    crashed_session(
        &state,
        pack,
        "[10:00:01] [main/INFO]: Loading\n[10:00:02] [main/ERROR]: Mixin apply failed bbb.mixins.json:M -> net.minecraft.class_1\n",
    );
    let after = check(&state, pack, CheckPass::Complete, None).await.unwrap();
    let found = mixin_findings(&after);
    assert_eq!(found.len(), 1, "{after:#?}");
    assert_eq!(found[0].finding.param("risco"), Some("alto"));
    assert_eq!(found[0].finding.param("travamento"), Some("sim"));
    assert!(found[0].finding.evidence().iter().any(|e| matches!(e,
        Evidence::Log { file, line: 2, excerpt } if file == "output.log" && excerpt.contains("bbb.mixins.json"))));

    let view = xray_mod_impl(&state, pack, "mods/aaa.jar").await.unwrap();
    assert_eq!(view.state, XrayState::Ready);
    let report = view.report.unwrap();
    assert_eq!(report.overlaps.len(), 1);
    assert_eq!(report.overlaps[0].risk, Risk::High);
    assert!(matches!(&report.overlaps[0].elevators[..], [Elevator::Crash(c)] if c.line == 2));
    assert_eq!(view.names.get("mods/bbb.jar").map(String::as_str), Some("bbb.jar"));
}

/// O relatório de um mod: resumo, sobreposições e partes alteradas; item que não está no pack ou
/// sem jar.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn relatorio_do_mod_nos_detalhes() {
    let Some((dir, state, cli)) = setup() else {
        return;
    };
    let (a, b) = (mixin_jar("aaa", true), mixin_jar("bbb", true));
    let plain = JarBuilder::new()
        .file("fabric.mod.json", fabric_mod_json("simples", "1", &[], &[], &[]))
        .build();
    let (pack, _root) = fabric_pack(
        &state,
        dir.path(),
        &cli,
        &[
            ("mods/aaa.jar", &a),
            ("mods/bbb.jar", &b),
            ("mods/simples.jar", &plain),
        ],
    )
    .await;

    let view = xray_mod_impl(&state, pack, "mods/aaa.jar").await.unwrap();
    assert_eq!(view.state, XrayState::Ready);
    assert!(!view.native_loader);
    let report = view.report.unwrap();
    assert_eq!(report.configs, vec!["aaa.mixins.json"]);
    assert_eq!((report.change_count, report.point_count, report.shared_point_count), (1, 1, 1));
    assert_eq!(report.overlaps[0].risk, Risk::High);
    assert_eq!(report.points[0].others, vec!["mods/bbb.jar"]);

    let simple = xray_mod_impl(&state, pack, "mods/simples.jar").await.unwrap();
    assert_eq!(simple.report.unwrap().change_count, 0);

    let error = xray_mod_impl(&state, pack, "mods/nao-existe.jar").await.unwrap_err();
    assert_eq!(
        error.code,
        ErrorCode::Project(ProjectErrorCode::ItemNotFound)
    );
}

/// O trecho do log vai sem dados pessoais (o nome do jogador vira marcador).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn citacao_do_travamento_vai_redigida() {
    let dir = tempfile::tempdir().unwrap();
    let state = test_state(dir.path());
    state
        .settings
        .update(crate::settings::SettingsPatch {
            player_name: Some("Steve_Teste".into()),
            ..Default::default()
        })
        .unwrap();
    let pack = PackId::new();
    crashed_session(
        &state,
        pack,
        "Steve_Teste caiu: Mixin apply failed a.mixins.json:X\n",
    );
    let citations = last_crash_citations(&state, pack);
    assert_eq!(citations.len(), 1);
    assert_eq!(citations[0].config, "a.mixins.json");
    assert!(!citations[0].excerpt.contains("Steve_Teste"), "{}", citations[0].excerpt);
    let _ = ItemRef::ModId { id: String::new() };
}
