//! As ferramentas de desempenho da IA (ROADMAP: o Sculk lê o que o Warden já mede em cada
//! teste; SPEC T13 e T33; ARCHITECTURE §21.6):
//!
//! - `get_test_performance`: as medidas dos últimos testes do pack neste computador
//!   (`perf/<pack-id>.jsonl`, gravado por `test_hooks::perf`) e a comparação entre as versões
//!   salvas pelo critério "mais pesada" ([`compare`]). Itens `perf:<sessão>` (um por teste) e
//!   `perf:comparacao`;
//! - `get_performance_profile`: o resultado do "Testar com perfil de desempenho" de uma sessão
//!   (tempo de carregamento por mod e o resumo do spark). Itens
//!   `perfprofile:<sessão>#carregamento` e `perfprofile:<sessão>#spark<N>`.
//!
//! Só leitura, dados do próprio computador (não são arquivos do jogo: sem o aviso "é dado"). Nada
//! daqui menciona caminhos nem nomes de arquivo; a impressão do computador sai reduzida a um
//! código curto. A redação do pack ainda passa por cima de tudo, em [`super::PackTools`].

mod compare;
mod line;
mod text;

use std::collections::HashSet;
use std::fmt::Write as _;

use serde_json::Value;
use warden_ai::tools::{ToolItem, ToolOutput};
use warden_launcher::outcome::Outcome;
use warden_perf::mod_timing::TimingStatus;
use warden_perf::spark::{SparkKind, SparkSide};
use warden_versioning::{PackRepo, Snapshot};

use self::compare::{Comparison, Excluded, MIN_TESTS, SavedTree, Verdict, compare};
use self::line::{PerfLine, read_lines};
use super::PackTools;
use crate::commands::perf::{LoadReport, PerfReport, SparkReport, report_of};
use crate::test_session::{TestMode, pack_tree_hash_of, session_dir};

/// Testes devolvidos por padrão.
const DEFAULT_LIMIT: usize = 10;
/// Máximo de testes por chamada.
const MAX_LIMIT: usize = 20;
/// Versões salvas cujas árvores entram na comparação (as mais recentes, como o gráfico da T33).
const COMPARED_VERSIONS: usize = 12;
/// Mods no "o que mais demorou para carregar".
const LOAD_ROWS: usize = 15;
/// Mods no resumo de cada arquivo do spark.
const SPARK_ROWS: usize = 10;

fn plain(items: Vec<ToolItem>) -> ToolOutput {
    ToolOutput {
        items,
        ..ToolOutput::default()
    }
}

/// O que se lê do disco para responder.
struct Loaded {
    /// Os testes, do mais antigo ao mais novo (a ordem do arquivo).
    lines: Vec<PerfLine>,
    /// As árvores das versões salvas mais recentes (mais nova primeiro).
    trees: Vec<SavedTree>,
    /// As sessões entre as mostradas que têm o perfil de desempenho.
    profiled: HashSet<String>,
}

/// A árvore de cada versão salva alcançável (as mais recentes), como o Testar a calcula.
fn saved_trees(root: &std::path::Path) -> Vec<SavedTree> {
    let Ok(repo) = PackRepo::open(root) else {
        return Vec::new();
    };
    let Ok(versions) = repo.versions() else {
        return Vec::new();
    };
    versions
        .into_iter()
        .filter(|version| version.reachable)
        .take(COMPARED_VERSIONS)
        .filter_map(|version| {
            let snapshot = Snapshot::Version(version.version.clone());
            let pack = repo.read_file_at(&snapshot, "pack.toml").ok().flatten()?;
            let index_file = std::str::from_utf8(&pack)
                .ok()
                .and_then(|text| warden_packwiz::PackManifest::parse(text).ok())
                .map_or_else(|| "index.toml".to_owned(), |parsed| parsed.value.index.file);
            let index = repo.read_file_at(&snapshot, &index_file).ok().flatten()?;
            Some(SavedTree {
                version: version.version,
                tree: pack_tree_hash_of(&pack, &index_file, &index),
            })
        })
        .collect()
}

/// `get_test_performance`.
pub(super) async fn test_performance(
    tools: &PackTools<'_>,
    args: &Value,
) -> Result<ToolOutput, String> {
    let limit = args
        .get("limit")
        .and_then(Value::as_i64)
        .and_then(|limit| usize::try_from(limit).ok())
        .map_or(DEFAULT_LIMIT, |limit| limit.clamp(1, MAX_LIMIT));
    let (paths, pack_id, root) = (tools.state.paths.clone(), tools.pack_id, tools.root.clone());
    // A leitura do disco (arquivo, repositório e sessões) fora da thread do laço.
    let loaded = tauri::async_runtime::spawn_blocking(move || {
        let lines = read_lines(&paths.perf_file(pack_id));
        let profiled = lines
            .iter()
            .rev()
            .take(limit)
            .filter(|line| {
                session_dir(&paths, pack_id, &line.sessao)
                    .ok()
                    .and_then(|dir| dir.read_record().ok())
                    .is_some_and(|record| report_of(&record).is_some())
            })
            .map(|line| line.sessao.clone())
            .collect();
        Loaded {
            lines,
            trees: saved_trees(&root),
            profiled,
        }
    })
    .await
    .map_err(|error| error.to_string())?;
    Ok(plain(render(&loaded, limit)))
}

/// Os itens da resposta: um por teste, do mais novo ao mais antigo, e a comparação.
fn render(loaded: &Loaded, limit: usize) -> Vec<ToolItem> {
    if loaded.lines.is_empty() {
        return vec![ToolItem::new(
            "perf:nenhum",
            "Nenhum teste com medidas de desempenho neste computador.",
        )];
    }
    let newest_first: Vec<PerfLine> = loaded.lines.iter().rev().cloned().collect();
    let mut items: Vec<ToolItem> = newest_first
        .iter()
        .take(limit)
        .map(|line| {
            ToolItem::new(
                format!("perf:{}", line.sessao),
                test_line(
                    line,
                    &version_label(line, &loaded.trees),
                    loaded.profiled.contains(&line.sessao),
                ),
            )
        })
        .collect();
    items.push(ToolItem::new(
        "perf:comparacao",
        comparison_text(&compare(&newest_first, &loaded.trees)),
    ));
    items
}

/// A versão salva igual à árvore testada, ou "não salva".
fn version_label(line: &PerfLine, trees: &[SavedTree]) -> String {
    line.arvore
        .as_deref()
        .and_then(|tree| trees.iter().find(|saved| saved.tree == tree))
        .map_or_else(
            || "versão não salva".to_owned(),
            |saved| format!("versão {}", saved.version),
        )
}

fn mode_label(mode: TestMode) -> &'static str {
    match mode {
        TestMode::Normal => "normal",
        TestMode::AsPlayer => "como o jogador recebe",
        TestMode::Performance => "com perfil de desempenho",
        TestMode::Server => "como servidor",
    }
}

fn outcome_label(outcome: Outcome) -> &'static str {
    match outcome {
        Outcome::StoppedByUser => "foi parado pelo usuário",
        Outcome::Crashed => "travou",
        Outcome::ClosedNormally => "fechou normalmente",
    }
}

/// O texto de um teste.
fn test_line(line: &PerfLine, version: &str, profiled: bool) -> String {
    let none = "sem medida";
    let measure = |value: Option<String>, with: &str, without: &str| {
        value.map_or_else(
            || format!("{without}: {none}"),
            |value| format!("{with} {value}"),
        )
    };
    let mut out = format!(
        "Teste de {} ({version}), modo {}, {}",
        if line.data.is_empty() {
            "data desconhecida"
        } else {
            &line.data
        },
        mode_label(line.modo),
        outcome_label(line.resultado),
    );
    let parts = [
        measure(
            line.carregou_ms.map(text::duration),
            "abriu em",
            "tempo para abrir",
        ),
        measure(
            line.mundo_ms.map(text::duration),
            "entrou no mundo em",
            "entrada no mundo",
        ),
        measure(
            line.ram_pico_mb.map(text::megabytes),
            "pico de RAM",
            "pico de RAM",
        ),
        measure(
            line.heap_pico_mb.map(text::megabytes),
            "memória do jogo (pico depois das coletas)",
            "memória do jogo (pico depois das coletas)",
        ),
    ];
    for part in parts {
        let _ = write!(out, "; {part}");
    }
    let name = line
        .perfil
        .nome
        .as_deref()
        .filter(|name| !name.trim().is_empty())
        .unwrap_or(&line.perfil.id);
    let signature = line
        .perfil
        .assinatura
        .as_deref()
        .map_or_else(|| none.to_owned(), text::signature_code);
    let _ = write!(
        out,
        "; perfil do teste {name} (assinatura {signature}); computador {}; primeira abertura: {}",
        line.computador
            .as_deref()
            .map_or_else(|| none.to_owned(), text::machine_code),
        if line.primeira_abertura {
            "sim"
        } else {
            "não"
        },
    );
    if profiled {
        out.push_str("; com perfil de desempenho (use get_performance_profile)");
    }
    out
}

/// O texto da comparação entre versões.
fn comparison_text(comparison: &Comparison) -> String {
    let mut out = String::from("Comparação entre versões salvas (critério mais pesada). ");
    if let Some(reference) = &comparison.profile {
        let _ = write!(
            out,
            "Só entram testes no modo normal que abriram o jogo, fora a primeira abertura, com o perfil do teste {} (assinatura {}) e no computador {}, que são os do teste comparável mais recente. ",
            reference.profile_name,
            reference
                .signature
                .as_deref()
                .map_or_else(|| "sem medida".to_owned(), text::signature_code),
            reference
                .machine
                .as_deref()
                .map_or_else(|| "sem medida".to_owned(), text::machine_code),
        );
    }
    match comparison.verdict {
        Verdict::NoComparable => {
            out.push_str("Nenhum teste comparável com versão salva. ");
        }
        Verdict::OneVersion => {
            let newest = comparison.newest.as_ref();
            let _ = write!(
                out,
                "Só a versão {} tem testes comparáveis ({}). Salve outra versão e teste para comparar. ",
                newest.map_or("?", |group| group.version.as_str()),
                newest.map_or(0, |group| group.tests),
            );
        }
        Verdict::FewTests => {
            out.push_str("Poucos testes para comparar. ");
            out.push_str(&groups_sentence(comparison));
            let _ = write!(
                out,
                "São precisos pelo menos {MIN_TESTS} testes comparáveis em cada versão; teste de novo para confirmar. "
            );
        }
        Verdict::Compared { open, memory } => {
            out.push_str(&groups_sentence(comparison));
            let _ = write!(
                out,
                "Tempo para abrir (mediana): {} contra {} ({}). ",
                text::duration(open.after),
                text::duration(open.before),
                change_text(
                    open.delta(),
                    open.percent(),
                    open.after >= open.before,
                    true
                ),
            );
            match memory {
                Some(memory) => {
                    let _ = write!(
                        out,
                        "Memória máxima, pico de RAM (mediana): {} contra {} ({}). ",
                        text::megabytes(memory.after),
                        text::megabytes(memory.before),
                        change_text(
                            memory.delta(),
                            memory.percent(),
                            memory.after >= memory.before,
                            false
                        ),
                    );
                }
                None => out.push_str(
                    "Memória máxima: faltam medidas do pico de RAM em alguma das versões. ",
                ),
            }
            let heavy_open = open.heavy;
            let heavy_memory = memory.is_some_and(|memory| memory.heavy);
            if comparison.verdict.heavier() {
                let which = match (heavy_open, heavy_memory) {
                    (true, true) => "tempo para abrir e memória máxima",
                    (true, false) => "tempo para abrir",
                    _ => "memória máxima",
                };
                let newest = comparison
                    .newest
                    .as_ref()
                    .map_or("?", |g| g.version.as_str());
                let _ = write!(
                    out,
                    "Resultado: a versão {newest} está mais pesada que a anterior ({which}: pelo menos 20% maior e, no tempo, 10 s a mais, ou, na memória, 512 MB a mais). Testar com perfil de desempenho mostra o que mais pesa. "
                );
            } else {
                out.push_str(
                    "Resultado: sem aviso de versão mais pesada (é preciso pelo menos 20% a mais e, no tempo, 10 s a mais, ou, na memória, 512 MB a mais). ",
                );
            }
        }
    }
    out.push_str(&excluded_sentence(&comparison.excluded));
    out.push_str(" O tempo por tick não entra: o histórico de desempenho não o grava.");
    out
}

/// "A versão X (N testes) contra a Y (M testes). "
fn groups_sentence(comparison: &Comparison) -> String {
    match (&comparison.newest, &comparison.previous) {
        (Some(new), Some(old)) => format!(
            "Versão {} ({} {}) contra a {} ({} {}). ",
            new.version,
            new.tests,
            tests_word(new.tests),
            old.version,
            old.tests,
            tests_word(old.tests),
        ),
        _ => String::new(),
    }
}

fn tests_word(count: usize) -> &'static str {
    if count == 1 { "teste" } else { "testes" }
}

/// "+26 s, +26,5%" ou "-20 s, -16,0%" (`time`: o aumento em segundos; senão, em MB).
fn change_text(delta: u32, percent: Option<f64>, grew: bool, time: bool) -> String {
    let sign = if grew { "+" } else { "-" };
    let amount = if time {
        text::duration(delta)
    } else {
        text::megabytes(delta)
    };
    match percent {
        Some(percent) => format!("{sign}{amount}, {sign}{}%", text::decimal(percent.abs(), 1)),
        None => format!("{sign}{amount}"),
    }
}

/// Os testes que ficaram de fora, por motivo.
fn excluded_sentence(excluded: &Excluded) -> String {
    if excluded.total() == 0 {
        return "Nenhum teste ficou de fora.".to_owned();
    }
    let reasons = [
        (excluded.performance_profile, "com perfil de desempenho"),
        (excluded.other_mode, "em outro modo de teste"),
        (excluded.not_loaded, "que não chegaram a carregar"),
        (excluded.first_launch, "na primeira abertura"),
        (excluded.other_profile, "com outro perfil do teste"),
        (excluded.other_machine, "de outro computador"),
        (
            excluded.unsaved,
            "sem versão salva igual (alterações não salvas)",
        ),
    ];
    let parts: Vec<String> = reasons
        .into_iter()
        .filter(|(count, _)| *count > 0)
        .map(|(count, reason)| format!("{count} {reason}"))
        .collect();
    format!(
        "{} {} de fora: {}.",
        excluded.total(),
        if excluded.total() == 1 {
            "teste ficou"
        } else {
            "testes ficaram"
        },
        parts.join(", ")
    )
}

// ---------------------------------------------------------------------------------------------
// get_performance_profile
// ---------------------------------------------------------------------------------------------

/// `get_performance_profile`.
pub(super) async fn performance_profile(
    tools: &PackTools<'_>,
    session_id: &str,
) -> Result<ToolOutput, String> {
    let (paths, pack_id, id) = (
        tools.state.paths.clone(),
        tools.pack_id,
        session_id.to_owned(),
    );
    let not_found = format!("A sessão '{session_id}' não existe (use list_sessions).");
    let record = tauri::async_runtime::spawn_blocking(move || {
        session_dir(&paths, pack_id, &id)
            .ok()
            .and_then(|dir| dir.read_record().ok())
    })
    .await
    .map_err(|error| error.to_string())?
    .ok_or(not_found)?;
    let report = report_of(&record)
        .ok_or_else(|| format!("O teste '{session_id}' não foi feito com perfil de desempenho."))?;
    Ok(plain(profile_items(session_id, &report)))
}

/// Os itens do perfil: o carregamento e um por arquivo do spark.
fn profile_items(session_id: &str, report: &PerfReport) -> Vec<ToolItem> {
    let mut load = load_text(report);
    if report.spark.is_empty() && report.server_spark.is_none() {
        load.push_str(" Nenhum arquivo do spark foi salvo neste teste.");
    }
    if report.spark_skipped > 0 {
        let _ = write!(
            load,
            " {} arquivo(s) do spark ficaram de fora do limite do Warden.",
            report.spark_skipped
        );
    }
    let mut items = vec![ToolItem::new(
        format!("perfprofile:{session_id}#carregamento"),
        load,
    )];
    items.extend(report.spark.iter().enumerate().map(|(index, spark)| {
        ToolItem::new(
            format!("perfprofile:{session_id}#spark{}", index + 1),
            spark_text(index + 1, spark),
        )
    }));
    items
}

/// O tempo de carregamento por mod, ou por que não há dado.
fn load_text(report: &PerfReport) -> String {
    let Some(load) = &report.load else {
        return "Carregamento por mod: sem dado (teste como servidor local).".to_owned();
    };
    load_report_text(load)
}

fn load_report_text(load: &LoadReport) -> String {
    let loader = &load.loader;
    match load.status {
        TimingStatus::Measured => {
            let mut mods: Vec<_> = load.mods.iter().collect();
            mods.sort_by_key(|row| std::cmp::Reverse(row.ms));
            let rows: Vec<String> = mods
                .iter()
                .take(LOAD_ROWS)
                .map(|row| format!("{} {}", row.name, text::seconds_one_place(row.ms)))
                .collect();
            format!(
                "Carregamento por mod (pelo log do loader {loader}): o jogo e o loader levaram {} no total e os {} mods levaram {}. Os {} mais demorados: {}.",
                text::seconds_one_place(load.game_and_loader_ms),
                load.mods.len(),
                text::seconds_one_place(load.mods_ms),
                rows.len(),
                rows.join("; "),
            )
        }
        TimingStatus::NotRecorded => format!(
            "Carregamento por mod: sem dado, o loader {loader} não registra o tempo de cada mod."
        ),
        TimingStatus::LogMissing => "Carregamento por mod: sem dado, o log do loader não existe (o jogo pode não ter chegado a carregar os mods).".to_owned(),
        TimingStatus::NoLines => "Carregamento por mod: sem dado, o log do loader não tem as linhas de tempo (o jogo fechou antes ou o registro detalhado não ligou).".to_owned(),
    }
}

/// O resumo de um arquivo do spark (sem o nome do arquivo).
fn spark_text(number: usize, spark: &SparkReport) -> String {
    let kind = match spark.kind {
        SparkKind::Profile => "perfil de processamento",
        SparkKind::Heap => "resumo da memória",
    };
    if !spark.readable {
        return format!("Spark {number}: {kind}; o arquivo não pôde ser lido.");
    }
    let mut out = format!("Spark {number}: {kind}");
    if let Some(platform) = &spark.platform {
        let side = match platform.side {
            SparkSide::Client => "no jogo",
            SparkSide::Server => "no servidor",
            SparkSide::Other => "em outra plataforma",
        };
        let _ = write!(out, " {side} ({} {})", platform.name, platform.version);
    }
    if let Some(ms) = spark.duration_ms {
        let _ = write!(
            out,
            "; duração {}",
            text::duration(u32::try_from(ms).unwrap_or(u32::MAX))
        );
    }
    if let Some(ticks) = &spark.ticks {
        let _ = write!(
            out,
            "; tempo por tick: mediana {} ms, pior tick {} ms",
            text::decimal(ticks.mspt_median, 1),
            text::decimal(ticks.mspt_max, 1),
        );
        if let Some(p95) = ticks.mspt_p95 {
            let _ = write!(
                out,
                ", 95% dos ticks até {} ms (o pior 5% passa disso)",
                text::decimal(p95, 1)
            );
        }
        if let Some(tps) = ticks.tps {
            let _ = write!(out, ", {} ticks por segundo", text::decimal(tps, 1));
        }
    }
    let what = match spark.kind {
        SparkKind::Profile => "mods que mais processam",
        SparkKind::Heap => "mods que mais usam memória",
    };
    if spark.top.is_empty() {
        let _ = write!(out, "; {what}: nenhum dado");
    } else {
        let rows: Vec<String> = spark
            .top
            .iter()
            .take(SPARK_ROWS)
            .map(|row| format!("{} {}%", row.name, text::decimal(row.percent, 1)))
            .collect();
        let _ = write!(out, "; {what}: {}", rows.join("; "));
    }
    let _ = write!(
        out,
        "; jogo, loader e Java: {}%",
        text::decimal(spark.game_percent, 1)
    );
    out
}

#[cfg(test)]
mod tests;
