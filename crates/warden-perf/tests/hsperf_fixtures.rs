//! O leitor do `hsperfdata` com arquivos reais de Java 8, 17, 21 e 25 (L-11; S-R5-2 §9).
//!
//! As fixtures de `tests/fixtures/hsperf/` vêm do spike S-R5-2 (origem e anonimização no
//! `README.md` de lá): cada `hsperfdata` foi gravado pelo *file mapping* de uma JVM parada com
//! 200 MB vivos, com a saída do `jstat -gc` do mesmo processo ao lado.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation
)]

use std::path::{Path, PathBuf};

use warden_perf::hsperf::parse::{parse, parse_header};
use warden_perf::hsperf::{Collector, read_buffer};

fn fixtures() -> Vec<PathBuf> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/hsperf");
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("hsperfdata"))
        .collect();
    files.sort();
    files
}

fn name(path: &Path) -> String {
    path.file_stem().unwrap().to_string_lossy().into_owned()
}

/// Cada fixture tem ao lado a saída do `jstat -gc` (JDK 25) do mesmo processo, tirada logo
/// depois, com o programa parado: todas as colunas têm de bater (tamanhos até 0,05 KB, o
/// arredondamento do `jstat`; contagens exatas; tempos até 0,0005 s).
#[test]
fn fixtures_match_jstat() {
    let mut checked = 0;
    let files = fixtures();
    assert_eq!(files.len(), 8, "as 8 fixtures do spike");
    for path in files {
        let jstat = std::fs::read_to_string(path.with_extension("jstat.txt")).unwrap();
        let mut lines = jstat.lines().filter(|line| !line.trim().is_empty());
        let head: Vec<&str> = lines.next().unwrap().split_whitespace().collect();
        let values: Vec<&str> = lines.next().unwrap().split_whitespace().collect();
        let snap = parse(&std::fs::read(&path).unwrap()).unwrap();
        let freq = snap.long("sun.os.hrt.frequency").unwrap() as f64;
        let kb = |n: &str| snap.long(n).map(|v| v as f64 / 1024.0);
        let secs = |n: &str| snap.long(n).map(|v| v as f64 / freq);
        let count = |n: &str| snap.long(n).map(|v| v as f64);
        for (column, value) in head.iter().zip(values) {
            let expected = match value {
                "-" => None,
                value => Some(value.replace(',', ".").parse::<f64>().unwrap()),
            };
            let (got, tolerance) = match *column {
                "S0C" => (kb("sun.gc.generation.0.space.1.capacity"), 0.05),
                "S1C" => (kb("sun.gc.generation.0.space.2.capacity"), 0.05),
                "S0U" => (kb("sun.gc.generation.0.space.1.used"), 0.05),
                "S1U" => (kb("sun.gc.generation.0.space.2.used"), 0.05),
                "EC" => (kb("sun.gc.generation.0.space.0.capacity"), 0.05),
                "EU" => (kb("sun.gc.generation.0.space.0.used"), 0.05),
                "OC" => (kb("sun.gc.generation.1.space.0.capacity"), 0.05),
                "OU" => (kb("sun.gc.generation.1.space.0.used"), 0.05),
                "MC" => (kb("sun.gc.metaspace.capacity"), 0.05),
                "MU" => (kb("sun.gc.metaspace.used"), 0.05),
                "CCSC" => (kb("sun.gc.compressedclassspace.capacity"), 0.05),
                "CCSU" => (kb("sun.gc.compressedclassspace.used"), 0.05),
                "YGC" => (count("sun.gc.collector.0.invocations"), 0.0),
                "FGC" => (count("sun.gc.collector.1.invocations"), 0.0),
                "CGC" => (count("sun.gc.collector.2.invocations"), 0.0),
                "YGCT" => (secs("sun.gc.collector.0.time"), 0.0005),
                "FGCT" => (secs("sun.gc.collector.1.time"), 0.0005),
                "CGCT" => (secs("sun.gc.collector.2.time"), 0.0005),
                _ => continue, // GCT: a soma, conferida pelas partes
            };
            match (expected, got) {
                (None, None) => {}
                (Some(x), Some(g)) => assert!(
                    (x - g).abs() <= tolerance,
                    "{} {column}: jstat {x}, leitor {g}",
                    name(&path)
                ),
                (x, g) => panic!("{} {column}: jstat {x:?}, leitor {g:?}", name(&path)),
            }
            checked += 1;
        }
    }
    assert!(checked > 100, "conferiu só {checked} colunas");
}

/// A leitura que o Warden usa (heap usada, máximo, coletas e tempo) bate com o `jstat` e com a
/// tabela do README das fixtures, e o coletor e a versão do Java saem certos.
#[test]
fn leitura_do_warden_em_cada_java_e_coletor() {
    // (fixture, coletor, major, heap usada em KB pelo README, coletas)
    let expected = [
        ("java17-g1", Collector::G1, 17, 217_536.5, 6),
        ("java21-g1", Collector::G1, 21, 217_664.1, 6),
        ("java25-g1", Collector::G1, 25, 217_988.4, 1),
        ("java25-shenandoah", Collector::Shenandoah, 25, 208_561.7, 0),
        ("java25-zgc", Collector::Zgc, 25, 227_328.0, 10),
        ("java8-g1", Collector::G1, 8, 217_984.5, 6),
        ("java8-parallel", Collector::Parallel, 8, 208_399.4, 1),
        ("mojang8u51-g1", Collector::G1, 8, 192_327.0, 2),
    ];
    let files = fixtures();
    assert_eq!(files.len(), expected.len());
    for (path, (fixture, collector, major, used_kb, gc_count)) in files.iter().zip(expected) {
        assert_eq!(name(path), fixture);
        let buf = std::fs::read(path).unwrap();
        let mut layout = None;
        let reading = read_buffer(&buf, &mut layout).unwrap().unwrap();
        let info = layout.unwrap().info();
        assert_eq!(info.collector, collector, "{fixture}");
        assert_eq!(info.major, Some(major), "{fixture}");
        assert!(
            (reading.used_bytes as f64 / 1024.0 - used_kb).abs() < 0.1,
            "{fixture}: {} KB",
            reading.used_bytes as f64 / 1024.0
        );
        assert_eq!(reading.gc_count, gc_count, "{fixture}");
        // `-Xmx1024m` em todos: a reserva pelos contadores dá 1 GiB (o G1 do Java 8 soma até
        // 24 bytes a mais; S-R5-2 §5.2).
        let max_mb = reading.max_bytes as f64 / (1024.0 * 1024.0);
        assert!(
            (max_mb - 1024.0).abs() < 0.01,
            "{fixture}: máximo {max_mb} MB"
        );
        // O tempo das coletas é o do `jstat` (GCT), em ms.
        let jstat = std::fs::read_to_string(path.with_extension("jstat.txt")).unwrap();
        let gct: f64 = jstat
            .lines()
            .nth(1)
            .unwrap()
            .split_whitespace()
            .last()
            .unwrap()
            .replace(',', ".")
            .parse()
            .unwrap();
        assert!(
            (reading.gc_time_ms as f64 - gct * 1000.0).abs() <= 1.5,
            "{fixture}: {} ms, jstat {gct} s",
            reading.gc_time_ms
        );
        // O ZGC e o Shenandoah contam pausas ("Pausas de coleta"); os outros, coletas.
        assert_eq!(
            info.collector.counts_pauses(),
            matches!(collector, Collector::Zgc | Collector::Shenandoah),
            "{fixture}"
        );
    }
}

/// Buffers reais truncados ou com bytes trocados viram erro (ou leitura), nunca pânico.
#[test]
fn fixtures_cortadas_ou_corrompidas_nao_dao_panico() {
    for path in fixtures() {
        let buf = std::fs::read(&path).unwrap();
        let header = parse_header(&buf).unwrap();
        assert!(header.accessible);
        // Cortes no meio das entradas: sempre erro (a lista de contadores não cabe mais).
        for len in [33, 100, buf.len() / 3, buf.len() / 2, buf.len() - 1] {
            assert!(
                parse(&buf[..len]).is_err(),
                "{} cortada em {len}",
                name(&path)
            );
            let mut layout = None;
            assert!(read_buffer(&buf[..len], &mut layout).is_err());
        }
        // Bytes trocados pelo buffer todo, a cada 97: nunca pânico.
        for start in (32..buf.len()).step_by(97) {
            let mut bad = buf.clone();
            bad[start] ^= 0xA5;
            let mut layout = None;
            let _ = read_buffer(&bad, &mut layout);
        }
        // Buffer ainda não pronto: sem leitura.
        let mut early = buf.clone();
        early[7] = 0;
        let mut layout = None;
        assert_eq!(read_buffer(&early, &mut layout).unwrap(), None);
    }
}

/// Privacidade (S-R5-2 §9): as fixtures não têm o nome de usuário do Windows nem pastas
/// pessoais; o PATH foi trocado por `<PATH removido>`.
#[test]
fn fixtures_sem_dados_pessoais() {
    for path in fixtures() {
        let buf = std::fs::read(&path).unwrap();
        let text = String::from_utf8_lossy(&buf).to_lowercase();
        for personal in ["\\users\\", "\\usuários\\", "/home/", "appdata"] {
            assert!(!text.contains(personal), "{} tem {personal}", name(&path));
        }
        let snap = parse(&buf).unwrap();
        assert_eq!(
            snap.text("java.property.java.library.path"),
            Some("<PATH removido>"),
            "{}",
            name(&path)
        );
    }
}
