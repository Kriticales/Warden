//! A memória da JVM com processos Java de verdade (L-11; CA-T13-10, CA-T13-11).
//!
//! O programa `tests/fixtures/java/WardenHeapProbe.class` (compilado para Java 8) roda com cada
//! Java pedido:
//!
//! - **com o programa parado** (100 MB vivos, nada alocando), a memória lida pelo Warden fica a
//!   até 5% da do `jstat -gc` (S0U + S1U + EU + OU) e as coletas batem; com
//!   `-XX:+PerfDisableSharedMem` ou `-XX:-UsePerfData` não há leitura (CA-T13-10);
//! - quando o processo sai, a amostra seguinte não tem leitura e o `hsperfdata` fica fechado
//!   (no Windows, o *file mapping* deixa de ser do Warden);
//! - com a heap acima de 90% depois das coletas por mais de 1 minuto, o `W_LOW_HEAP` aparece; com
//!   menos de 1 minuto, não (CA-T13-11).
//!
//! Javas: `WARDEN_HSPERF_JAVAS` (caminhos do `java`, separados por `;` no Windows e `:` no
//! resto; a CI Windows do Noturno baixa os JREs 8, 17, 21 e 25), senão o de `WARDEN_TEST_JAVA`
//! ou `JAVA_HOME`. O `jstat` vem de `WARDEN_HSPERF_JSTAT` ou do `bin` desse Java (um JDK). Sem
//! Java, avisa e passa; com `WARDEN_REQUIRE_EXTERNALS=1` (CI), a falta é falha.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]

use std::io::{BufRead as _, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use warden_perf::findings::{FindingContext, HeapTracker, RULE_LOW_HEAP, TrackerConfig};
use warden_perf::hsperf::{HeapReading, JvmHeap, game_temp_dirs};
use warden_perf::process::{MB, ProcessTree};

const EXE: &str = if cfg!(windows) { ".exe" } else { "" };

fn require_externals() -> bool {
    std::env::var_os("WARDEN_REQUIRE_EXTERNALS").is_some_and(|value| value == "1")
}

/// Os Javas do teste.
fn javas() -> Vec<PathBuf> {
    if let Some(list) = std::env::var_os("WARDEN_HSPERF_JAVAS").filter(|value| !value.is_empty()) {
        let javas: Vec<PathBuf> = std::env::split_paths(&list).collect();
        for java in &javas {
            assert!(
                java.is_file(),
                "WARDEN_HSPERF_JAVAS: {} não existe",
                java.display()
            );
        }
        return javas;
    }
    let java = std::env::var_os("WARDEN_TEST_JAVA")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("JAVA_HOME")
                .filter(|value| !value.is_empty())
                .map(|home| PathBuf::from(home).join("bin").join(format!("java{EXE}")))
        })
        .filter(|path| path.is_file());
    if java.is_none() {
        assert!(
            !require_externals(),
            "WARDEN_REQUIRE_EXTERNALS=1, mas não há Java (WARDEN_TEST_JAVA ou JAVA_HOME)"
        );
        eprintln!("AVISO: sem Java (WARDEN_TEST_JAVA ou JAVA_HOME); teste com a JVM real pulado.");
    }
    java.into_iter().collect()
}

/// O `jstat` de um JDK.
fn jstat(java: &Path) -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("WARDEN_HSPERF_JSTAT").filter(|value| !value.is_empty()) {
        return Some(PathBuf::from(path));
    }
    let beside = java.with_file_name(format!("jstat{EXE}"));
    if beside.is_file() {
        return Some(beside);
    }
    eprintln!(
        "AVISO: sem jstat ao lado de {} (é um JRE); comparação com o jstat pulada.",
        java.display()
    );
    None
}

fn classes() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/java")
}

/// Abre o programa de teste e espera ele escrever "pronto".
fn probe(java: &Path, jvm_args: &[&str], args: &[&str]) -> Child {
    let mut child = Command::new(java)
        .args(jvm_args)
        .arg("-cp")
        .arg(classes())
        .arg("WardenHeapProbe")
        .args(args)
        .env_remove("JAVA_TOOL_OPTIONS")
        .env_remove("_JAVA_OPTIONS")
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .unwrap();
    let stdout = child.stdout.take().unwrap();
    let mut line = String::new();
    BufReader::new(stdout).read_line(&mut line).unwrap();
    assert!(line.starts_with("pronto"), "o programa escreveu {line:?}");
    child
}

/// Espera uma leitura (o buffer fica pronto logo depois de a JVM abrir).
fn first_reading(heap: &mut JvmHeap) -> Option<HeapReading> {
    let until = Instant::now() + Duration::from_secs(10);
    while Instant::now() < until {
        if let Some(reading) = heap.read() {
            return Some(reading);
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    None
}

/// `jstat -gc <pid>`: a memória usada (S0U + S1U + EU + OU, em bytes) e as coletas
/// (YGC + FGC + CGC).
fn jstat_gc(jstat: &Path, pid: u32) -> (u64, u64) {
    let output = Command::new(jstat)
        .args(["-gc", &pid.to_string()])
        .env_remove("JAVA_TOOL_OPTIONS")
        .env_remove("_JAVA_OPTIONS")
        .output()
        .unwrap();
    assert!(output.status.success(), "jstat falhou: {output:?}");
    let text = String::from_utf8_lossy(&output.stdout);
    let mut lines = text.lines().filter(|line| !line.trim().is_empty());
    let head: Vec<&str> = lines.next().unwrap().split_whitespace().collect();
    let values: Vec<&str> = lines.next().unwrap().split_whitespace().collect();
    let column = |name: &str| -> f64 {
        head.iter()
            .position(|&h| h == name)
            .and_then(|at| values.get(at))
            .and_then(|value| value.replace(',', ".").parse().ok())
            .unwrap_or(0.0)
    };
    let used_kb = column("S0U") + column("S1U") + column("EU") + column("OU");
    let collections = column("YGC") + column("FGC") + column("CGC");
    ((used_kb * 1024.0) as u64, collections as u64)
}

fn stop(mut child: Child) {
    let _ = child.kill();
    let _ = child.wait();
}

/// CA-T13-10: com o programa parado, a leitura do Warden fica a até 5% do `jstat`; sem
/// publicação (`-XX:+PerfDisableSharedMem`, `-XX:-UsePerfData`), não há leitura.
#[test]
fn memoria_bate_com_o_jstat_com_o_programa_parado() {
    for java in javas() {
        let child = probe(&java, &["-Xmx256m", "-XX:+UseG1GC"], &["parar", "100"]);
        let pid = child.id();
        let mut heap = JvmHeap::new(pid, game_temp_dirs(&[]));
        let before = first_reading(&mut heap)
            .unwrap_or_else(|| panic!("{}: sem leitura do hsperfdata", java.display()));
        assert!(heap.is_open());
        assert!(
            before.used_bytes >= 100 * MB,
            "{}: {} bytes",
            java.display(),
            before.used_bytes
        );
        if let Some(jstat) = jstat(&java) {
            let (expected, collections) = jstat_gc(&jstat, pid);
            let after = heap.read().unwrap();
            let diff = (after.used_bytes as f64 - expected as f64).abs() / expected as f64;
            eprintln!(
                "{} ({:?}): Warden {} KB, jstat {} KB, diferença {:.2}%",
                java.display(),
                heap.info(),
                after.used_bytes / 1024,
                expected / 1024,
                diff * 100.0
            );
            assert!(
                diff <= 0.05,
                "{}: diferença de {:.2}%",
                java.display(),
                diff * 100.0
            );
            assert_eq!(after.gc_count, collections, "{}", java.display());
        }
        stop(child);

        for flag in ["-XX:+PerfDisableSharedMem", "-XX:-UsePerfData"] {
            let child = probe(&java, &["-Xmx64m", flag], &["parar", "8"]);
            let mut heap = JvmHeap::new(child.id(), game_temp_dirs(&[]));
            std::thread::sleep(Duration::from_millis(500));
            assert_eq!(heap.read(), None, "{} com {flag}", java.display());
            assert!(!heap.is_open());
            stop(child);
        }
    }
}

/// O `hsperfdata` é solto na mesma amostra em que o processo some (critério da L-11: "em até 1
/// amostra depois de o processo sair").
#[test]
fn mapeamento_fecha_na_amostra_seguinte_a_saida_do_processo() {
    for java in javas() {
        let mut child = probe(&java, &["-Xmx64m"], &["parar", "8"]);
        let pid = child.id();
        let mut tree = ProcessTree::open(pid).unwrap();
        let mut heap = JvmHeap::new(pid, game_temp_dirs(&[]));
        assert!(first_reading(&mut heap).is_some(), "{}", java.display());
        let (_, reading) = heap.sample(&mut tree).unwrap();
        assert!(reading.is_some());
        assert!(heap.is_open());
        child.kill().unwrap();
        child.wait().unwrap();
        assert!(heap.sample(&mut tree).is_none(), "{}", java.display());
        assert!(!heap.is_open(), "{}: ficou aberto", java.display());
        assert_eq!(heap.read(), None);
    }
}

/// CA-T13-11: a heap acima de 90% depois das coletas por 70 s gera o `W_LOW_HEAP`; antes de 1
/// minuto, não. Uma amostra por segundo, como no teste de verdade.
#[test]
fn memoria_quase_cheia_por_70_s_gera_o_aviso_e_antes_de_1_minuto_nao() {
    let Some(java) = javas().into_iter().next() else {
        return;
    };
    let child = probe(&java, &["-Xmx256m", "-XX:+UseG1GC"], &["cheia", "92"]);
    let mut heap = JvmHeap::new(child.id(), game_temp_dirs(&[]));
    let mut tracker = HeapTracker::new(TrackerConfig {
        xmx_mb: Some(256),
        pack_suggestion_mb: None,
    });
    let start = Instant::now();
    let context = FindingContext {
        session_id: "teste",
        loaded: true,
    };
    let mut checked_early = false;
    let mut found_at = None;
    while start.elapsed() < Duration::from_secs(80) {
        let elapsed = u64::try_from(start.elapsed().as_millis()).unwrap();
        if let Some(reading) = heap.read() {
            tracker.push(elapsed, reading, heap.info());
        }
        let low = tracker
            .findings(context)
            .iter()
            .any(|finding| finding.rule().as_str() == RULE_LOW_HEAP);
        if elapsed < 55_000 {
            assert!(!low, "aviso antes de 1 minuto ({elapsed} ms)");
            checked_early = elapsed > 50_000 || checked_early;
        } else if low {
            found_at = Some(elapsed);
            break;
        }
        std::thread::sleep(Duration::from_secs(1));
    }
    stop(child);
    assert!(checked_early);
    let found_at = found_at.expect("sem W_LOW_HEAP depois de 80 s com a heap cheia");
    eprintln!("W_LOW_HEAP em {found_at} ms: {:?}", tracker.live());
    let finding = tracker
        .findings(context)
        .into_iter()
        .find(|finding| finding.rule().as_str() == RULE_LOW_HEAP)
        .unwrap();
    // Mais de 60 s (em segundos inteiros, 60 já é "mais de 1 minuto" por alguns ms).
    assert!(finding.param("seconds").unwrap().parse::<u64>().unwrap() >= 60);
    assert!(finding.param("percent").unwrap().parse::<u64>().unwrap() >= 90);
}
