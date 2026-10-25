//! "O que mudou durante o teste" com instâncias sintéticas (ROADMAP C-03; SPEC T15).
//!
//! Cada teste copia um pack para a instância com a materialização de verdade, tira a linha de
//! base, muda arquivos como o jogo mudaria (com a data de modificação adiantada, como depois de
//! uma sessão de jogo) e confere a captura, o que vai para o pack e se a próxima cópia do pack
//! passa sem pedir revisão.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::too_many_lines
)]

mod conformance_support;

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;
use std::time::{Duration, SystemTime};

use conformance_support::{TestPack, dirs, materialize_ok, sources, temp_dir};
use warden_instance::{
    CaptureApplyRequest, CaptureChange, CaptureDecision, CaptureError, CaptureGroup,
    CaptureResolution, CaptureStatus, InstanceDirs, MaterializeOptions, OnModified, Outcome,
    PersonalConfigs, capture_after_exit, capture_after_exit_with, capture_texts, capture_view,
    discard, finish_apply, load_capture, materialize, plan_apply, take_baseline,
};

const OPTIONS: &str = "version:3465\nrenderDistance:12\nguiScale:0\nlang:en_us\nfullscreen:false\nkey_key.jump:key.keyboard.space\n";
const SODIUM: &str = "{\n  \"quality\": {\n    \"weather_quality\": \"DEFAULT\"\n  },\n  \"advanced\": {\n    \"cpu_render_ahead_limit\": 3\n  }\n}\n";
const FORGE: &str = "# Configuração do cliente\n[client]\n\t#Mostrar o aviso\n\tshowWarning = true\n\tmaxItems = 64\n";

/// O pack padrão: options.txt, duas configs e um script.
fn base_pack() -> TestPack {
    TestPack::default()
        .with_file("options.txt", OPTIONS)
        .with_file("config/sodium-options.json", SODIUM)
        .with_file("config/Pasta com espaço/ação-client.toml", FORGE)
        .with_file(
            "kubejs/server_scripts/receitas.js",
            "ServerEvents.recipes(e => {})\n",
        )
}

struct Lab {
    _dir: tempfile::TempDir,
    pack: std::path::PathBuf,
    instance: InstanceDirs,
    cache: std::path::PathBuf,
}

impl Lab {
    async fn new(pack: &TestPack) -> Self {
        let dir = temp_dir();
        let lab = Self {
            pack: dir.path().join("Meu pack ç"),
            instance: dirs(&dir.path().join("instância de teste")),
            cache: dir.path().join("cache"),
            _dir: dir,
        };
        pack.write_to(&lab.pack, "http://127.0.0.1:1");
        lab.materialize().await;
        lab
    }

    async fn materialize(&self) {
        materialize_ok(
            &sources(&self.cache, None),
            &self.pack,
            &self.instance,
            &MaterializeOptions::default(),
        )
        .await;
    }

    /// Materializa e devolve se pediu revisão (algum arquivo seria sobrescrito).
    async fn materialize_needs_review(&self) -> bool {
        let outcome = materialize(
            &sources(&self.cache, None),
            &self.pack,
            &self.instance,
            &MaterializeOptions {
                on_modified: OnModified::Pause,
                ..MaterializeOptions::default()
            },
            &warden_core::NoProgress,
            &warden_core::CancellationToken::new(),
        )
        .await
        .unwrap();
        matches!(outcome, Outcome::NeedsReview(_))
    }

    fn game(&self, relative: &str) -> std::path::PathBuf {
        self.instance.game_dir.join(relative)
    }

    /// O jogo grava o arquivo (com a data de modificação depois da linha de base).
    fn play_write(&self, relative: &str, contents: &[u8]) {
        let path = self.game(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, contents).unwrap();
        let later = SystemTime::now() + Duration::from_secs(5);
        fs::File::options()
            .write(true)
            .open(&path)
            .unwrap()
            .set_modified(later)
            .unwrap();
    }

    fn baseline(&self) {
        take_baseline(&self.instance, &self.pack).unwrap();
    }

    fn capture(&self) -> warden_instance::Capture {
        capture_after_exit(&self.instance, &self.pack, "sessao-1")
            .unwrap()
            .expect("captura")
            .capture
    }

    /// Grava o plano no pack como a `PackTransaction` gravaria (índice com os hashes novos).
    fn write_plan(&self, pack: &TestPack, plan: &warden_instance::ApplyPlan) -> TestPack {
        let mut next = pack.clone();
        for (path, bytes) in &plan.writes {
            next.files.insert(
                path.clone(),
                (bytes.clone(), next.files.get(path).is_some_and(|f| f.1)),
            );
        }
        for path in &plan.deletes {
            next.files.remove(path);
        }
        next.write_to(&self.pack, "http://127.0.0.1:1");
        next
    }
}

fn paths(capture: &warden_instance::Capture) -> Vec<(&str, CaptureGroup)> {
    capture
        .files
        .iter()
        .map(|file| (file.path.as_str(), file.group))
        .collect()
}

fn pack_sha(lab: &Lab, relative: &str) -> Option<String> {
    use sha2::Digest as _;
    fs::read(lab.pack.join(relative))
        .ok()
        .map(|bytes| hex::encode(sha2::Sha256::digest(bytes)))
}

#[tokio::test]
async fn distancia_de_renderizacao_vira_uma_linha_no_options_do_pack() {
    // CA-T15-01.
    let pack = base_pack();
    let lab = Lab::new(&pack).await;
    lab.baseline();
    lab.play_write(
        "options.txt",
        b"version:3700\nrenderDistance:16\nguiScale:0\nlang:en_us\nfullscreen:true\nkey_key.jump:key.keyboard.space\ntutorialStep:none\n",
    );
    let capture = lab.capture();
    assert_eq!(paths(&capture), [("options.txt", CaptureGroup::Options)]);
    let options = &capture.files[0];
    assert_eq!(options.options.len(), 1);
    assert_eq!(options.options[0].key, "renderDistance");
    assert_eq!(options.options[0].before.as_deref(), Some("12"));
    assert_eq!(options.options[0].after, "16");
    assert!(options.options[0].selected);
    assert_eq!(options.hidden_volatile, 3);
    assert!(capture.needs_review());

    let request = CaptureApplyRequest {
        session_id: "sessao-1".into(),
        files: Vec::new(),
        option_keys: vec!["renderDistance".into()],
        expected_options_sha256: pack_sha(&lab, "options.txt"),
    };
    let plan = plan_apply(&lab.instance, &lab.pack, &request).unwrap();
    assert_eq!(plan.count, 1);
    let written = String::from_utf8(plan.writes["options.txt"].clone()).unwrap();
    let changed: Vec<(&str, &str)> = OPTIONS
        .lines()
        .zip(written.lines())
        .filter(|(a, b)| a != b)
        .collect();
    assert_eq!(changed, [("renderDistance:12", "renderDistance:16")]);
    assert_eq!(written.lines().count(), OPTIONS.lines().count());

    lab.write_plan(&pack, &plan);
    let restored = finish_apply(&lab.instance, &lab.pack, "sessao-1", &plan).unwrap();
    assert_eq!(restored, ["options.txt"]);
    assert_eq!(
        load_capture(&lab.instance.state_dir)
            .unwrap()
            .unwrap()
            .status,
        CaptureStatus::Applied { count: 1 }
    );
    // A próxima cópia do pack passa sem pedir revisão.
    assert!(!lab.materialize_needs_review().await);
    assert_eq!(
        fs::read_to_string(lab.game("options.txt")).unwrap(),
        written
    );
}

#[tokio::test]
async fn config_so_reformatada_nao_aparece_e_volta_ao_pack() {
    // CA-T15-02.
    let lab = Lab::new(&base_pack()).await;
    lab.baseline();
    // O Forge reescreve com outro recuo, sem o comentário e com a ordem trocada.
    lab.play_write(
        "config/Pasta com espaço/ação-client.toml",
        b"[client]\r\n    maxItems = 64\r\n    showWarning = true\r\n",
    );
    lab.play_write(
        "config/sodium-options.json",
        br#"{"advanced":{"cpu_render_ahead_limit":3},"quality":{"weather_quality":"DEFAULT"}}"#,
    );
    let outcome = capture_after_exit(&lab.instance, &lab.pack, "sessao-1")
        .unwrap()
        .unwrap();
    assert!(
        outcome.capture.files.is_empty(),
        "{:?}",
        outcome.capture.files
    );
    assert_eq!(outcome.capture.reformatted, 2);
    assert!(!outcome.capture.needs_review());
    assert_eq!(outcome.restored.len(), 2);
    assert!(!lab.materialize_needs_review().await);
}

#[tokio::test]
async fn sempre_ignorados_nunca_aparecem() {
    // CA-T15-03 e CA-T15-06.
    let lab = Lab::new(&base_pack()).await;
    lab.play_write("config/spark/tmp-antigo/x", b"antes");
    lab.baseline();
    for path in [
        "logs/latest.log",
        "crash-reports/crash.txt",
        "saves/Mundo de teste/level.dat",
        "screenshots/a.png",
        "usercache.json",
        "config/spark/perfil.sparkprofile",
        "config/spark/heap.sparkheap",
        "config/spark/tmp123",
        "config/spark/tmp-antigo/x",
        "kubejs/config/web_server.json",
        ".probe/types.d.ts",
        ".vscode/settings.json",
        "local/kubejs/exported/x.json",
        "kubejs/exported/tags.json",
        "journeymap/data/x.dat",
        "config/x-1.toml.bak",
        ".fabric/remappedJars/a.jar",
    ] {
        lab.play_write(path, b"depois");
    }
    let capture = lab.capture();
    assert!(capture.files.is_empty(), "{:?}", paths(&capture));
}

#[tokio::test]
async fn jar_na_pasta_de_mods_nunca_vai_para_o_pack() {
    // CA-T15-04.
    let lab = Lab::new(&base_pack()).await;
    lab.baseline();
    lab.play_write("mods/jei-1.20.1-forge 15.jar", b"PK\x03\x04jar");
    let capture = lab.capture();
    assert_eq!(
        paths(&capture),
        [("mods/jei-1.20.1-forge 15.jar", CaptureGroup::Mod)]
    );
    let jar = &capture.files[0];
    assert_eq!(jar.change, CaptureChange::Added);
    assert_eq!(jar.pack_path, None);
    assert!(!jar.selected);
    assert!(!capture.needs_review());
    let error = plan_apply(
        &lab.instance,
        &lab.pack,
        &CaptureApplyRequest {
            session_id: "sessao-1".into(),
            files: vec![CaptureDecision {
                path: "mods/jei-1.20.1-forge 15.jar".into(),
                resolution: CaptureResolution::Take,
                expected_pack_sha256: None,
            }],
            option_keys: Vec::new(),
            expected_options_sha256: None,
        },
    )
    .unwrap_err();
    assert!(
        matches!(error, CaptureError::InvalidItem { .. }),
        "{error:?}"
    );
    assert!(!lab.pack.join("mods/jei-1.20.1-forge 15.jar").exists());
}

#[tokio::test]
async fn conflito_de_tres_vias_quando_pack_e_jogo_mudam() {
    // CA-T15-05.
    let pack = base_pack();
    let lab = Lab::new(&pack).await;
    lab.baseline();
    // No jogo: muda a qualidade do tempo. No pack (pelo editor, com o jogo aberto): muda outra
    // chave.
    lab.play_write(
        "config/sodium-options.json",
        SODIUM.replace("DEFAULT", "FANCY").as_bytes(),
    );
    let pack_edit = SODIUM.replace(
        "\"cpu_render_ahead_limit\": 3",
        "\"cpu_render_ahead_limit\": 5",
    );
    fs::write(lab.pack.join("config/sodium-options.json"), &pack_edit).unwrap();

    let capture = lab.capture();
    assert_eq!(
        paths(&capture),
        [("config/sodium-options.json", CaptureGroup::PackConfig)]
    );
    assert_eq!(capture.files[0].values[0].key, "quality.weather_quality");
    let view = capture_view(&lab.instance, &lab.pack).unwrap().unwrap();
    let conflict = view.files[0].conflict.clone().expect("conflito");
    assert!(conflict.mergeable);
    assert_eq!(
        conflict.pack_values[0].key,
        "advanced.cpu_render_ahead_limit"
    );
    assert_eq!(conflict.pack_values[0].after.as_deref(), Some("5"));

    let texts = capture_texts(
        &lab.instance,
        &lab.pack,
        "sessao-1",
        "config/sodium-options.json",
    )
    .unwrap();
    assert_eq!(texts.base.as_deref(), Some(SODIUM));
    assert_eq!(texts.pack.as_deref(), Some(pack_edit.as_str()));
    assert!(texts.instance.unwrap().contains("FANCY"));

    let decision = |resolution, sha| CaptureApplyRequest {
        session_id: "sessao-1".into(),
        files: vec![CaptureDecision {
            path: "config/sodium-options.json".into(),
            resolution,
            expected_pack_sha256: sha,
        }],
        option_keys: Vec::new(),
        expected_options_sha256: None,
    };
    // O pack mudou de novo depois que a lista foi mostrada: nada é gravado.
    let stale = plan_apply(
        &lab.instance,
        &lab.pack,
        &decision(
            CaptureResolution::Merge,
            view.files[0].file.before_sha256.clone(),
        ),
    )
    .unwrap_err();
    assert!(matches!(stale, CaptureError::Stale { .. }), "{stale:?}");

    let plan = plan_apply(
        &lab.instance,
        &lab.pack,
        &decision(CaptureResolution::Merge, view.files[0].pack_sha256.clone()),
    )
    .unwrap();
    let merged = String::from_utf8(plan.writes["config/sodium-options.json"].clone()).unwrap();
    assert!(merged.contains("FANCY") && merged.contains("\"cpu_render_ahead_limit\": 5"));

    // "Manter a do pack" não grava nada.
    let keep = plan_apply(
        &lab.instance,
        &lab.pack,
        &decision(
            CaptureResolution::KeepPack,
            view.files[0].pack_sha256.clone(),
        ),
    )
    .unwrap();
    assert!(keep.writes.is_empty());
}

#[tokio::test]
async fn sem_conflito_quando_so_o_jogo_mudou() {
    let lab = Lab::new(&base_pack()).await;
    lab.baseline();
    lab.play_write(
        "config/sodium-options.json",
        SODIUM.replace("DEFAULT", "FANCY").as_bytes(),
    );
    let view = {
        lab.capture();
        capture_view(&lab.instance, &lab.pack).unwrap().unwrap()
    };
    assert!(view.files[0].conflict.is_none());
    assert!(view.files[0].file.selected);
    assert!(view.needs_review);
}

#[tokio::test]
async fn descartar_devolve_as_configs_do_pack_e_mantem_o_resto() {
    let lab = Lab::new(&base_pack()).await;
    lab.baseline();
    lab.play_write(
        "config/sodium-options.json",
        SODIUM.replace("DEFAULT", "FANCY").as_bytes(),
    );
    lab.play_write("config/jade/plugins.json", b"{\"a\": true}\n");
    let capture = lab.capture();
    assert_eq!(
        paths(&capture),
        [
            ("config/sodium-options.json", CaptureGroup::PackConfig),
            ("config/jade/plugins.json", CaptureGroup::Generated),
        ]
    );
    assert!(!capture.files[1].selected);
    // Antes de decidir, a próxima cópia do pack para e pede revisão.
    assert!(lab.materialize_needs_review().await);

    let restored = discard(&lab.instance, &lab.pack, "sessao-1").unwrap();
    assert_eq!(restored, ["config/sodium-options.json"]);
    assert!(lab.game("config/jade/plugins.json").is_file());
    assert_eq!(
        load_capture(&lab.instance.state_dir)
            .unwrap()
            .unwrap()
            .status,
        CaptureStatus::Discarded
    );
    assert!(!lab.materialize_needs_review().await);
    assert_eq!(
        fs::read_to_string(lab.game("config/sodium-options.json")).unwrap(),
        SODIUM
    );
    // Descartar de novo: já foi revisada.
    assert!(matches!(
        discard(&lab.instance, &lab.pack, "sessao-1"),
        Err(CaptureError::AlreadyResolved)
    ));
}

#[tokio::test]
async fn trazer_so_o_escolhido_e_o_resto_volta_ao_pack() {
    let pack = base_pack();
    let lab = Lab::new(&pack).await;
    lab.baseline();
    lab.play_write(
        "config/sodium-options.json",
        SODIUM.replace("DEFAULT", "FANCY").as_bytes(),
    );
    // CRLF e caminho com espaço e acento.
    lab.play_write(
        "config/Pasta com espaço/ação-client.toml",
        FORGE.replace("64", "128").replace('\n', "\r\n").as_bytes(),
    );
    lab.play_write(
        "saves/Mundo de teste/serverconfig/create-server.toml",
        b"[x]\na = 2\n",
    );
    lab.play_write(
        "kubejs/server_scripts/receitas.js",
        b"ServerEvents.recipes(e => { e.remove({}) })\n",
    );
    let capture = lab.capture();
    assert_eq!(
        paths(&capture),
        [
            (
                "config/Pasta com espaço/ação-client.toml",
                CaptureGroup::PackConfig
            ),
            ("config/sodium-options.json", CaptureGroup::PackConfig),
            (
                "kubejs/server_scripts/receitas.js",
                CaptureGroup::PackConfig
            ),
            (
                "saves/Mundo de teste/serverconfig/create-server.toml",
                CaptureGroup::WorldConfig
            ),
        ]
    );
    let toml = &capture.files[0];
    assert_eq!(toml.values.len(), 1);
    assert_eq!(toml.values[0].key, "client.maxItems");
    assert_eq!(toml.values[0].before.as_deref(), Some("64"));
    assert_eq!(toml.values[0].after.as_deref(), Some("128"));
    // Script alterado aparece como config, com diferença de texto.
    assert!(capture.files[2].text && capture.files[2].values.is_empty());
    let world = &capture.files[3];
    assert!(!world.selected);
    assert_eq!(
        world.pack_path.as_deref(),
        Some("defaultconfigs/create-server.toml")
    );

    let view = capture_view(&lab.instance, &lab.pack).unwrap().unwrap();
    let sha: BTreeMap<String, Option<String>> = view
        .files
        .iter()
        .map(|file| (file.file.path.clone(), file.pack_sha256.clone()))
        .collect();
    let take = |path: &str| CaptureDecision {
        path: path.into(),
        resolution: CaptureResolution::Take,
        expected_pack_sha256: sha[path].clone(),
    };
    let plan = plan_apply(
        &lab.instance,
        &lab.pack,
        &CaptureApplyRequest {
            session_id: "sessao-1".into(),
            files: vec![
                take("config/Pasta com espaço/ação-client.toml"),
                take("saves/Mundo de teste/serverconfig/create-server.toml"),
            ],
            option_keys: Vec::new(),
            expected_options_sha256: None,
        },
    )
    .unwrap();
    assert_eq!(plan.count, 2);
    assert_eq!(
        plan.writes.keys().collect::<Vec<_>>(),
        [
            "config/Pasta com espaço/ação-client.toml",
            "defaultconfigs/create-server.toml"
        ]
    );
    assert!(
        plan.writes["config/Pasta com espaço/ação-client.toml"].ends_with(b"maxItems = 128\r\n")
    );
    lab.write_plan(&pack, &plan);
    let mut restored = finish_apply(&lab.instance, &lab.pack, "sessao-1", &plan).unwrap();
    restored.sort();
    assert_eq!(
        restored,
        [
            "config/sodium-options.json",
            "kubejs/server_scripts/receitas.js"
        ]
    );
    assert!(!lab.materialize_needs_review().await);
    assert_eq!(
        fs::read(lab.game("config/Pasta com espaço/ação-client.toml")).unwrap(),
        plan.writes["config/Pasta com espaço/ação-client.toml"]
    );
    assert_eq!(
        fs::read_to_string(lab.game("config/sodium-options.json")).unwrap(),
        SODIUM
    );
}

#[tokio::test]
async fn opcoes_criadas_no_primeiro_teste_viram_arquivo_gerado() {
    // Sem options.txt no início, o arquivo inteiro aparece desmarcado (não chave a chave).
    let lab = Lab::new(&TestPack::default().with_file("config/a.toml", "a = 1\n")).await;
    lab.baseline();
    lab.play_write("options.txt", OPTIONS.as_bytes());
    lab.play_write("resourcepacks/Faithful/pack.mcmeta", b"{}");
    lab.play_write("resourcepacks/Faithful/assets/x.png", b"png");
    lab.play_write("saves/Novo mundo/datapacks/meu.zip", b"zip");
    let capture = lab.capture();
    assert_eq!(
        paths(&capture),
        [
            ("resourcepacks/Faithful", CaptureGroup::Resource),
            ("saves/Novo mundo/datapacks/meu.zip", CaptureGroup::Datapack),
            ("options.txt", CaptureGroup::Generated),
        ]
    );
    assert!(capture.files.iter().all(|file| !file.selected));
    assert!(!capture.needs_review());
}

#[tokio::test]
async fn opcoes_fora_do_pack_criam_options_so_com_as_chaves_escolhidas() {
    let lab = Lab::new(&TestPack::default().with_file("config/a.toml", "a = 1\n")).await;
    lab.play_write("options.txt", OPTIONS.replace('\n', "\r\n").as_bytes());
    lab.baseline();
    lab.play_write(
        "options.txt",
        OPTIONS
            .replace("renderDistance:12", "renderDistance:16")
            .replace("guiScale:0", "guiScale:3")
            .replace('\n', "\r\n")
            .as_bytes(),
    );
    let capture = lab.capture();
    assert_eq!(paths(&capture), [("options.txt", CaptureGroup::Options)]);
    assert!(!capture.files[0].managed);
    let plan = plan_apply(
        &lab.instance,
        &lab.pack,
        &CaptureApplyRequest {
            session_id: "sessao-1".into(),
            files: Vec::new(),
            option_keys: vec!["guiScale".into()],
            expected_options_sha256: None,
        },
    )
    .unwrap();
    assert_eq!(plan.writes["options.txt"], b"guiScale:3\r\n");
}

#[tokio::test]
async fn sem_linha_de_base_nao_ha_captura() {
    let lab = Lab::new(&base_pack()).await;
    assert!(
        capture_after_exit(&lab.instance, &lab.pack, "s")
            .unwrap()
            .is_none()
    );
    assert!(capture_view(&lab.instance, &lab.pack).unwrap().is_none());
    // Uma linha de base nova apaga a captura anterior.
    lab.baseline();
    lab.play_write("config/novo.toml", b"a = 1\n");
    lab.capture();
    lab.baseline();
    assert!(load_capture(&lab.instance.state_dir).unwrap().is_none());
}

#[test]
fn caminho_do_pack_nao_vaza_nos_erros() {
    let dir = temp_dir();
    let instance = dirs(&dir.path().join("x"));
    fs::create_dir_all(instance.state_dir.join("baseline")).unwrap();
    // A captura é uma pasta: a leitura falha.
    fs::create_dir_all(instance.state_dir.join("baseline/capture.json")).unwrap();
    let error = load_capture(&instance.state_dir).unwrap_err().to_string();
    let home = dir.path().to_string_lossy().into_owned();
    assert!(!error.contains(&home), "{error}");
    assert!(
        error.contains("instância/state/baseline/capture.json"),
        "{error}"
    );
    let _ = Path::new(&home);
}

/// ADR-0067: o `options.txt` (e o minimapa) são pessoais. O que o jogo muda neles não aparece
/// como candidato ao pack, a captura não pede revisão por eles, e a cópia seguinte do pack não os
/// sobrescreve nem pede revisão, mesmo com o `options.txt` ainda no pack.
#[tokio::test]
async fn configs_pessoais_nunca_viram_candidatas_ao_pack() {
    let pack = base_pack().with_file("config/xaerominimap.txt", "zoom:1\n");
    let lab = Lab::new(&pack).await;
    let store = lab._dir.path().join("pessoais");
    let personal = PersonalConfigs::load(&store).unwrap();
    let paths: BTreeSet<String> = personal.paths();
    lab.baseline();
    lab.play_write(
        "options.txt",
        OPTIONS
            .replace("renderDistance:12", "renderDistance:16")
            .as_bytes(),
    );
    lab.play_write("config/xaerominimap.txt", b"zoom:3\n");
    lab.play_write(
        "config/sodium-options.json",
        SODIUM.replace("3", "4").as_bytes(),
    );

    // Sem a lista pessoal (o comportamento antigo), o options.txt viria pré-marcado.
    let old = capture_after_exit(&lab.instance, &lab.pack, "antes")
        .unwrap()
        .unwrap();
    assert!(
        old.capture
            .files
            .iter()
            .any(|file| file.group == CaptureGroup::Options)
    );

    lab.baseline();
    lab.play_write(
        "options.txt",
        OPTIONS
            .replace("renderDistance:12", "renderDistance:20")
            .as_bytes(),
    );
    lab.play_write("config/xaerominimap.txt", b"zoom:4\n");
    let outcome = capture_after_exit_with(&lab.instance, &lab.pack, "s", &paths)
        .unwrap()
        .unwrap();
    let shown: Vec<&str> = outcome
        .capture
        .files
        .iter()
        .map(|f| f.path.as_str())
        .collect();
    assert!(!shown.contains(&"options.txt"), "{shown:?}");
    assert!(!shown.contains(&"config/xaerominimap.txt"), "{shown:?}");

    // A cópia guardada acompanha o que o jogo deixou.
    personal.save_from(&lab.instance.game_dir).unwrap();
    assert!(
        String::from_utf8(personal.stored("options.txt").unwrap().unwrap())
            .unwrap()
            .contains("renderDistance:20")
    );

    // A próxima cópia do pack (com o options.txt ainda nele) não toca nos pessoais nem pede
    // revisão por causa deles.
    let options = MaterializeOptions {
        on_modified: OnModified::Pause,
        personal: paths.clone(),
        ..MaterializeOptions::default()
    };
    lab.play_write("config/sodium-options.json", SODIUM.as_bytes());
    let outcome = materialize(
        &sources(&lab.cache, None),
        &lab.pack,
        &lab.instance,
        &options,
        &warden_core::NoProgress,
        &warden_core::CancellationToken::new(),
    )
    .await
    .unwrap();
    assert!(matches!(outcome, Outcome::Done(_)), "{outcome:?}");
    assert!(
        fs::read_to_string(lab.game("options.txt"))
            .unwrap()
            .contains("renderDistance:20")
    );
    assert_eq!(
        fs::read_to_string(lab.game("config/xaerominimap.txt")).unwrap(),
        "zoom:4\n"
    );
    // O manifesto esqueceu os pessoais: nem sem a lista eles são mais "do pack modificados".
    let manifest = warden_instance::Manifest::load(&lab.instance.state_dir).unwrap();
    assert!(!manifest.files.contains_key("options.txt"));
}
