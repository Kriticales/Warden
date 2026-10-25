//! As ferramentas de desempenho no app, sobre um pack de verdade numa pasta temporária, com
//! `perf/<pack-id>.jsonl` e sessões gravadas nos dados do Warden e o Gemini simulado: os itens
//! `perf:*` e `perfprofile:*`, a comparação entre versões salvas, a redação e o laço completo
//! (a IA consulta, cita e a evidência é conferida contra o texto enviado).

// Os roteiros do simulado são `match` pelo número da requisição, como nos de `ai_tools/tests.rs`.
#![allow(clippy::unwrap_used, clippy::single_match_else)]

use std::fmt::Write as _;
use std::fs;
use std::time::{Duration, UNIX_EPOCH};

use serde_json::{Value, json};
use warden_ai::conversation::AiSymptom;
use warden_ai::evidence::ClaimStatus;
use warden_ai::tools::{AiPermissions, ToolOutput, ToolSource as _};
use warden_instance::InstanceDirs;
use warden_launcher::outcome::{CrashArtifacts, Outcome};
use warden_launcher::process::GameExit;
use warden_launcher::session::{SessionDir, SessionRecord};
use warden_launcher::{GameSpec, LoaderSpec};
use warden_perf::mod_timing::{TimingSource, TimingStatus};
use warden_perf::spark::{SparkKind, SparkPlatform, SparkSide, TickSummary};
use warden_versioning::{Identity, InitialPoint, Moment, PackRepo, SaveVersion};

use crate::ai_tools::PackTools;
use crate::ai_tools::tests::{Setup, calls, noop, setup};
use crate::commands::ai::{
    AiPreviewRequest, AiSource, get_impl, preview_impl, run_impl, start_impl,
};
use crate::commands::perf::{
    LoadReport, LoadRow, PerfReport, PerfSide, SESSION_FIELD, SparkReport, SparkRow,
};

const NEVER: &str = "unreachable";

fn tools(setup: &Setup) -> PackTools<'_> {
    PackTools::new(&setup.state, setup.pack, setup.root.clone(), None)
}

fn ids(output: &ToolOutput) -> Vec<&str> {
    output.items.iter().map(|item| item.id.as_str()).collect()
}

fn item<'a>(output: &'a ToolOutput, id: &str) -> &'a str {
    output
        .items
        .iter()
        .find(|item| item.id == id)
        .unwrap_or_else(|| panic!("sem o item {id}: {:?}", ids(output)))
        .text
        .as_str()
}

/// Uma linha do histórico como o gravador a escreve.
#[allow(clippy::too_many_arguments)]
fn line(
    session: &str,
    mode: &str,
    open_s: Option<u32>,
    ram: Option<u32>,
    tree: &str,
    profile_name: Option<&str>,
    machine: &str,
    first: bool,
) -> Value {
    let mut line = json!({
        "sessao": session,
        "data": format!("{}T21:10:00Z", &session[..10]),
        "modo": mode,
        "ram_pico_mb": ram,
        "heap_pico_mb": null,
        "perfil": { "id": "default", "assinatura": "sha256:ab12cd34ef" },
        "computador": machine,
        "arvore": tree,
        "primeira_abertura": first,
        "resultado": "closedNormally",
    });
    if let Some(open_s) = open_s {
        line["carregou_ms"] = json!(open_s * 1000);
        line["mundo_ms"] = json!(open_s * 1000 + 61_000);
    }
    if let Some(name) = profile_name {
        line["perfil"]["nome"] = json!(name);
    }
    line
}

fn write_history(setup: &Setup, lines: &[Value]) {
    let file = setup.state.paths.perf_file(setup.pack);
    fs::create_dir_all(file.parent().unwrap()).unwrap();
    let mut text = String::new();
    for (index, line) in lines.iter().enumerate() {
        writeln!(text, "{line}").unwrap();
        if index == 0 {
            // Uma linha ilegível no meio é ignorada.
            text.push_str("{isto não é json\n");
        }
    }
    fs::write(file, text).unwrap();
}

fn sessions_dir(setup: &Setup) -> std::path::PathBuf {
    InstanceDirs::for_pack(&setup.state.paths, setup.pack)
        .state_dir
        .join("sessions")
}

/// Grava uma sessão (`seconds` desde 1970 dá o nome da pasta) e devolve o id.
fn session(setup: &Setup, seconds: u64, report: Option<&PerfReport>) -> String {
    let started = UNIX_EPOCH + Duration::from_secs(seconds);
    let dir = SessionDir::create(&sessions_dir(setup), started).unwrap();
    let exit = GameExit {
        outcome: Outcome::ClosedNormally,
        exit_code: Some(0),
        stop_requested: false,
        duration_ms: 90_000,
        artifacts: CrashArtifacts::default(),
    };
    let spec = GameSpec::new(
        "1.20.1",
        LoaderSpec::Fabric {
            version: "0.16.9".into(),
        },
    )
    .unwrap();
    let mut record = SessionRecord::from_exit(
        started,
        &exit,
        &spec,
        17,
        None,
        std::path::Path::new("jogo"),
    );
    if let Some(report) = report {
        record
            .extra
            .insert(SESSION_FIELD.into(), serde_json::to_value(report).unwrap());
    }
    dir.write_record(&record).unwrap();
    dir.path()
        .file_name()
        .unwrap()
        .to_string_lossy()
        .into_owned()
}

fn report() -> PerfReport {
    let row = |name: &str, ms: u32| LoadRow {
        key: name.into(),
        name: name.into(),
        item: None,
        ms,
        phases: Vec::new(),
    };
    PerfReport {
        side: PerfSide::Client,
        load: Some(LoadReport {
            status: TimingStatus::Measured,
            source: Some(TimingSource::BarStep),
            loader: "forge".into(),
            log: Some("logs/debug.log".into()),
            // Fora de ordem: a ferramenta ordena do mais demorado ao menos.
            mods: vec![
                row("Create", 12_400),
                row("Mekanism", 20_100),
                row("C:\\Users\\Maria\\mods\\secreto", 3_000),
            ],
            game_and_loader_ms: 41_000,
            mods_ms: 35_500,
        }),
        server_spark: None,
        spark: vec![SparkReport {
            file: "profile-2026-10-18_00.42.48.sparkprofile".into(),
            kind: SparkKind::Profile,
            saved_at_ms: Some(1_790_000_000_000),
            readable: true,
            platform: Some(SparkPlatform {
                side: SparkSide::Client,
                name: "Forge".into(),
                version: "47.3.0".into(),
                minecraft: Some("1.20.1".into()),
            }),
            duration_ms: Some(125_000),
            ticks: Some(TickSummary {
                mspt_median: 18.4,
                mspt_max: 212.0,
                mspt_p95: Some(41.25),
                tps: Some(19.2),
                ticks: Some(2400),
            }),
            top: vec![
                SparkRow {
                    name: "Create".into(),
                    item: None,
                    mod_id: None,
                    percent: 31.4,
                },
                SparkRow {
                    name: "Mekanism".into(),
                    item: None,
                    mod_id: None,
                    percent: 12.0,
                },
            ],
            game_percent: 44.1,
        }],
        spark_skipped: 0,
    }
}

/// Versiona o pack (índice diferente a cada versão) e devolve a árvore de cada uma.
fn save_versions(setup: &Setup, versions: &[&str]) -> Vec<String> {
    let identity = Identity::for_pack("Autor", None);
    let repo = PackRepo::init(
        &setup.root,
        InitialPoint::Created,
        &identity,
        Moment::new(1_790_866_800, -180),
    )
    .unwrap();
    let mut trees = Vec::new();
    for (number, version) in versions.iter().enumerate() {
        let index = fs::read_to_string(setup.root.join("index.toml")).unwrap();
        fs::write(
            setup.root.join("index.toml"),
            format!("{index}# versão {number}\n"),
        )
        .unwrap();
        repo.save_version(&SaveVersion {
            version,
            tag_message: "",
            identity: &identity,
            when: Moment::new(1_790_866_800 + i64::try_from(number).unwrap(), -180),
            mark_final: false,
        })
        .unwrap();
        trees.push(crate::test_session::pack_tree_hash(&setup.root, "index.toml").unwrap());
    }
    trees
}

fn quiet() -> crate::ai_tools::tests::Script {
    Box::new(|_, _| unreachable!("{NEVER}: nada pode ir à IA"))
}

#[tokio::test]
async fn sem_historico_um_item_perf_nenhum() {
    let setup = setup(quiet()).await;
    let output = tools(&setup)
        .call("get_test_performance", &json!({}))
        .await
        .unwrap();
    assert_eq!(ids(&output), ["perf:nenhum"]);
    assert_eq!(
        output.items[0].text,
        "Nenhum teste com medidas de desempenho neste computador."
    );
    assert!(!output.game_data && !output.third_party && output.external.is_none());
    // Só linhas ilegíveis dá o mesmo.
    let file = setup.state.paths.perf_file(setup.pack);
    fs::create_dir_all(file.parent().unwrap()).unwrap();
    fs::write(file, "lixo\n{}\n").unwrap();
    let output = tools(&setup)
        .call("get_test_performance", &json!({}))
        .await
        .unwrap();
    assert_eq!(ids(&output), ["perf:nenhum"]);
}

#[tokio::test]
#[allow(clippy::too_many_lines)] // um cenário só, com muitas conferências
async fn testes_do_mais_novo_ao_mais_antigo_com_versao_comparacao_e_redacao() {
    let setup = setup(quiet()).await;
    let trees = save_versions(&setup, &["1.4.2", "1.5.0"]);
    // Alterações não salvas depois da 1.5.0.
    fs::write(setup.root.join("README.md"), "mudou\n").unwrap();
    let index = fs::read_to_string(setup.root.join("index.toml")).unwrap();
    fs::write(
        setup.root.join("index.toml"),
        format!("{index}# sem salvar\n"),
    )
    .unwrap();
    let unsaved = crate::test_session::pack_tree_hash(&setup.root, "index.toml").unwrap();

    let pc = "sha256:7f3a9c1d2e4b5a69";
    let with_profile = session(&setup, 1_790_900_000, Some(&report()));
    let n = |i: u32| format!("2026-10-0{i}_21-10-00");
    let lines = vec![
        line(
            &n(1),
            "normal",
            Some(100),
            Some(5000),
            &trees[0],
            None,
            pc,
            false,
        ),
        line(
            &n(2),
            "normal",
            Some(104),
            Some(5000),
            &trees[0],
            None,
            pc,
            false,
        ),
        line(
            &n(3),
            "normal",
            Some(98),
            Some(5000),
            &trees[0],
            None,
            pc,
            false,
        ),
        line(
            &n(4),
            "normal",
            Some(125),
            Some(6120),
            &trees[1],
            None,
            pc,
            false,
        ),
        line(
            &n(5),
            "normal",
            Some(128),
            Some(6120),
            &trees[1],
            None,
            pc,
            false,
        ),
        // Ficam de fora: perfil de desempenho, primeira abertura, outro computador, não salvo,
        // e um que não carregou.
        line(
            &with_profile,
            "performance",
            Some(300),
            Some(7000),
            &trees[1],
            None,
            pc,
            false,
        ),
        line(
            &n(6),
            "normal",
            Some(190),
            Some(6000),
            &trees[1],
            None,
            pc,
            true,
        ),
        line(
            &n(7),
            "normal",
            Some(80),
            Some(4000),
            &trees[1],
            None,
            "sha256:ffffff000000",
            false,
        ),
        // O mais novo: não salvo, com o nome do perfil carregando dados pessoais.
        line(
            &n(8),
            "normal",
            Some(124),
            None,
            &unsaved,
            Some("Perfil de C:\\Users\\Maria\\jogo, IP 192.168.0.42"),
            pc,
            false,
        ),
    ];
    let mut lines = lines;
    lines.push(line(
        &n(9),
        "normal",
        None,
        Some(900),
        &trees[1],
        None,
        pc,
        false,
    ));
    // O teste mais novo de todos (o último do arquivo) é o que travou antes de carregar.
    write_history(&setup, &lines);

    let output = tools(&setup)
        .call("get_test_performance", &json!({ "limit": 3 }))
        .await
        .unwrap();
    // Do mais novo ao mais antigo, limitado, e a comparação por último.
    assert_eq!(
        ids(&output),
        [
            format!("perf:{}", n(9)).as_str(),
            format!("perf:{}", n(8)).as_str(),
            format!("perf:{}", n(7)).as_str(),
            "perf:comparacao"
        ]
    );
    assert_eq!(
        item(&output, &format!("perf:{}", n(9))),
        "Teste de 2026-10-09T21:10:00Z (versão 1.5.0), modo normal, fechou normalmente; \
         tempo para abrir: sem medida; entrada no mundo: sem medida; pico de RAM 900 MB; \
         memória do jogo (pico depois das coletas): sem medida; perfil do teste default \
         (assinatura ab12cd); computador PC-7f3a9c; primeira abertura: não"
    );
    let unsaved_text = item(&output, &format!("perf:{}", n(8)));
    assert!(
        unsaved_text.contains("(versão não salva)"),
        "{unsaved_text}"
    );
    assert!(unsaved_text.contains("abriu em 2 min 4 s; entrou no mundo em 3 min 5 s"));
    assert!(unsaved_text.contains("pico de RAM: sem medida"));
    // Redação: nada pessoal no nome do perfil.
    for secret in ["Maria", "192.168.0.42"] {
        assert!(
            !unsaved_text.contains(secret),
            "{secret} vazou: {unsaved_text}"
        );
    }
    assert!(
        unsaved_text.contains("perfil do teste Perfil de "),
        "{unsaved_text}"
    );
    let other_pc = item(&output, &format!("perf:{}", n(7)));
    assert!(other_pc.contains("computador PC-ffffff"), "{other_pc}");
    assert!(!other_pc.contains("sha256"), "{other_pc}");

    let comparison = item(&output, "perf:comparacao");
    assert!(
        comparison.contains("Versão 1.5.0 (2 testes) contra a 1.4.2 (3 testes)"),
        "{comparison}"
    );
    assert!(
        comparison
            .contains("Tempo para abrir (mediana): 2 min 7 s contra 1 min 40 s (+27 s, +26,5%)"),
        "{comparison}"
    );
    assert!(
        comparison.contains(
            "Memória máxima, pico de RAM (mediana): 6.120 MB contra 5.000 MB (+1.120 MB, +22,4%)"
        ),
        "{comparison}"
    );
    assert!(
        comparison.contains(
            "a versão 1.5.0 está mais pesada que a anterior (tempo para abrir e memória máxima"
        ),
        "{comparison}"
    );
    assert!(
        comparison.contains(
            "5 testes ficaram de fora: 1 com perfil de desempenho, 1 que não chegaram a carregar, 1 na primeira abertura, 1 de outro computador, 1 sem versão salva igual (alterações não salvas)."
        ),
        "{comparison}"
    );
    assert!(comparison.contains("computador PC-7f3a9c"), "{comparison}");
    assert!(!comparison.contains("sha256"), "{comparison}");

    // O limite padrão (10) mostra todos os 10 testes; o teste com perfil leva a indicação.
    let all = tools(&setup)
        .call("get_test_performance", &json!({}))
        .await
        .unwrap();
    assert_eq!(all.items.len(), 11);
    let profiled = item(&all, &format!("perf:{with_profile}"));
    assert!(
        profiled.contains("modo com perfil de desempenho"),
        "{profiled}"
    );
    assert!(
        profiled.ends_with("com perfil de desempenho (use get_performance_profile)"),
        "{profiled}"
    );
    // Sem a sessão gravada (a poda a apagou), sem a indicação.
    assert!(!item(&all, &format!("perf:{}", n(5))).contains("get_performance_profile"));
    // Limites: 0 e 99 viram 1 e 20.
    let one = tools(&setup)
        .call("get_test_performance", &json!({ "limit": 0 }))
        .await
        .unwrap();
    assert_eq!(one.items.len(), 2);
}

#[tokio::test]
async fn comparacao_com_poucos_testes_e_sem_versoes() {
    let setup = setup(quiet()).await;
    let trees = save_versions(&setup, &["1.4.2", "1.5.0"]);
    let pc = "sha256:7f3a9c1d2e4b5a69";
    write_history(
        &setup,
        &[
            line(
                "2026-10-01_21-10-00",
                "normal",
                Some(100),
                None,
                &trees[0],
                None,
                pc,
                false,
            ),
            line(
                "2026-10-02_21-10-00",
                "normal",
                Some(104),
                None,
                &trees[0],
                None,
                pc,
                false,
            ),
            line(
                "2026-10-03_21-10-00",
                "normal",
                Some(125),
                None,
                &trees[1],
                None,
                pc,
                false,
            ),
        ],
    );
    let output = tools(&setup)
        .call("get_test_performance", &json!({}))
        .await
        .unwrap();
    let comparison = item(&output, "perf:comparacao");
    assert!(
        comparison.contains("Poucos testes para comparar."),
        "{comparison}"
    );
    assert!(
        !comparison.contains("mais pesada que a anterior"),
        "{comparison}"
    );
    assert!(
        comparison.contains("Nenhum teste ficou de fora."),
        "{comparison}"
    );
}

#[tokio::test]
async fn sem_historico_de_versoes_os_testes_ficam_nao_salvos() {
    // O pack não é um repositório: nenhuma árvore de versão, nenhum erro.
    let setup = setup(quiet()).await;
    let pc = "sha256:7f3a9c1d2e4b5a69";
    write_history(
        &setup,
        &[line(
            "2026-10-01_21-10-00",
            "normal",
            Some(100),
            Some(5000),
            "sha256:x",
            None,
            pc,
            false,
        )],
    );
    let output = tools(&setup)
        .call("get_test_performance", &json!({}))
        .await
        .unwrap();
    assert!(item(&output, "perf:2026-10-01_21-10-00").contains("(versão não salva)"));
    let comparison = item(&output, "perf:comparacao");
    assert!(
        comparison.contains("Nenhum teste comparável com versão salva."),
        "{comparison}"
    );
    assert!(
        comparison.contains("1 sem versão salva igual"),
        "{comparison}"
    );
}

#[tokio::test]
async fn perfil_de_desempenho_carregamento_e_spark_redigidos_e_sem_caminhos() {
    let setup = setup(quiet()).await;
    let id = session(&setup, 1_790_900_000, Some(&report()));
    let output = tools(&setup)
        .call("get_performance_profile", &json!({ "session_id": id }))
        .await
        .unwrap();
    assert_eq!(
        ids(&output),
        [
            format!("perfprofile:{id}#carregamento").as_str(),
            format!("perfprofile:{id}#spark1").as_str()
        ]
    );
    let load = item(&output, &format!("perfprofile:{id}#carregamento"));
    assert_eq!(
        load,
        "Carregamento por mod (pelo log do loader forge): o jogo e o loader levaram 41,0 s no \
         total e os 3 mods levaram 35,5 s. Os 3 mais demorados: Mekanism 20,1 s; Create 12,4 s; \
         C:\\Users\\<usuário>\\mods\\secreto 3,0 s."
    );
    let spark = item(&output, &format!("perfprofile:{id}#spark1"));
    assert_eq!(
        spark,
        "Spark 1: perfil de processamento no jogo (Forge 47.3.0); duração 2 min 5 s; \
         tempo por tick: mediana 18,4 ms, pior tick 212,0 ms, 95% dos ticks até 41,2 ms \
         (o pior 5% passa disso), 19,2 ticks por segundo; mods que mais processam: \
         Create 31,4%; Mekanism 12,0%; jogo, loader e Java: 44,1%"
    );
    let all: String = output.items.iter().map(|item| item.text.as_str()).collect();
    for private in ["sparkprofile", "logs/debug.log", "Maria", "profile-2026"] {
        assert!(!all.contains(private), "{private} vazou: {all}");
    }
    assert!(!output.game_data);
}

#[tokio::test]
async fn perfil_de_desempenho_sem_dado_do_loader_e_sem_spark() {
    let setup = setup(quiet()).await;
    let mut empty = report();
    empty.spark.clear();
    empty.spark_skipped = 2;
    if let Some(load) = &mut empty.load {
        load.status = TimingStatus::NotRecorded;
        load.loader = "fabric".into();
        load.mods.clear();
    }
    let id = session(&setup, 1_790_900_000, Some(&empty));
    let output = tools(&setup)
        .call("get_performance_profile", &json!({ "session_id": id }))
        .await
        .unwrap();
    assert_eq!(ids(&output), [format!("perfprofile:{id}#carregamento")]);
    assert_eq!(
        output.items[0].text,
        "Carregamento por mod: sem dado, o loader fabric não registra o tempo de cada mod. \
         Nenhum arquivo do spark foi salvo neste teste. 2 arquivo(s) do spark ficaram de fora do limite do Warden."
    );
    // Servidor local: sem tempo por mod.
    let mut server = report();
    server.side = PerfSide::Server;
    server.load = None;
    let id = session(&setup, 1_790_900_100, Some(&server));
    let output = tools(&setup)
        .call("get_performance_profile", &json!({ "session_id": id }))
        .await
        .unwrap();
    assert_eq!(
        output.items[0].text,
        "Carregamento por mod: sem dado (teste como servidor local)."
    );
}

#[tokio::test]
async fn perfil_de_desempenho_erros_de_sessao_inexistente_e_sem_perfil() {
    let setup = setup(quiet()).await;
    let plain = session(&setup, 1_790_900_000, None);
    let tools = tools(&setup);
    let missing = tools
        .call(
            "get_performance_profile",
            &json!({ "session_id": "2026-01-01_00-00-00" }),
        )
        .await
        .unwrap_err();
    assert_eq!(
        missing,
        "A sessão '2026-01-01_00-00-00' não existe (use list_sessions)."
    );
    // Um id estranho (outro caminho) também não existe, sem tocar em nada fora das sessões.
    let strange = tools
        .call("get_performance_profile", &json!({ "session_id": "../x" }))
        .await
        .unwrap_err();
    assert!(strange.contains("não existe"), "{strange}");
    let without = tools
        .call("get_performance_profile", &json!({ "session_id": plain }))
        .await
        .unwrap_err();
    assert_eq!(
        without,
        format!("O teste '{plain}' não foi feito com perfil de desempenho.")
    );
}

/// Laço completo (como os CA-T14): a IA chama `get_test_performance`, cita um teste e a
/// evidência é conferida contra o texto que de fato foi enviado.
#[tokio::test]
async fn laco_completo_consulta_cita_e_a_evidencia_e_verificada() {
    let setup = setup(Box::new(|n, request| match n {
        0 => calls(&[("get_test_performance", json!({ "limit": 2 }))]),
        _ => {
            let contents = request["contents"].as_array().unwrap();
            let first = &contents.last().unwrap()["parts"][0]["functionResponse"]["response"]["itens"][0];
            calls(&[(
                "responder",
                json!({ "resumo": "O último teste abriu o jogo em 2 min 4 s.", "semConclusao": false, "propostas": [],
                    "achados": [{ "afirmacao": "O jogo demora cerca de 2 min para abrir.", "mods": [], "confianca": "media",
                        "evidencias": [{ "id": first["id"], "citacao": first["texto"] }] }] }),
            )])
        }
    }))
    .await;
    let pc = "sha256:7f3a9c1d2e4b5a69";
    write_history(
        &setup,
        &[
            line(
                "2026-10-01_21-10-00",
                "normal",
                Some(100),
                Some(5000),
                "sha256:x",
                None,
                pc,
                false,
            ),
            line(
                "2026-10-02_21-10-00",
                "normal",
                Some(124),
                Some(5100),
                "sha256:x",
                None,
                pc,
                false,
            ),
        ],
    );
    let expected = tools(&setup)
        .call("get_test_performance", &json!({ "limit": 2 }))
        .await
        .unwrap();
    let preview = preview_impl(
        &setup.state,
        setup.pack,
        AiPreviewRequest {
            symptom: AiSymptom::Slow,
            source: AiSource::PackOnly,
            note: Some("o mundo demora para carregar".into()),
            mode: None,
        },
    )
    .await
    .unwrap();
    let id = start_impl(
        &setup.state,
        setup.pack,
        &preview.preview_id,
        AiPermissions::default(),
    )
    .unwrap();
    run_impl(&setup.state, setup.pack, &id, noop())
        .await
        .unwrap();

    let view = get_impl(&setup.state, setup.pack, &id).await.unwrap();
    let step = &view.turns[0].steps[0];
    assert_eq!(step.name, "get_test_performance");
    assert!(step.ok && !step.denied);
    // O bloco enviado é exatamente o resultado da ferramenta.
    let sent: Value = serde_json::from_str(&step.sent).unwrap();
    let sent_items = sent["functionResponse"]["response"]["itens"]
        .as_array()
        .unwrap();
    assert_eq!(sent_items.len(), expected.items.len());
    for (sent, expected) in sent_items.iter().zip(&expected.items) {
        assert_eq!(sent["id"], expected.id);
        assert_eq!(sent["texto"], expected.text);
    }
    assert_eq!(sent_items[0]["id"], "perf:2026-10-02_21-10-00");
    assert!(sent["functionResponse"]["response"].get("aviso").is_none());
    let answer = view.turns[0].answer.as_ref().unwrap();
    assert_eq!(answer.checked.claims[0].status, ClaimStatus::Verified);
    assert_eq!(
        answer.checked.claims[0].evidence[0].id,
        "perf:2026-10-02_21-10-00"
    );
}
