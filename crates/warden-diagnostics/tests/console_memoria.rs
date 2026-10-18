//! Teto de memória do console agrupado (D-09): um log de milhões de linhas não faz a memória
//! crescer sem limite. Um alocador que conta os bytes vivos mede o que o agrupador guarda
//! depois de ler o log (a leitura do texto pelo `LogParser`, em pedaços como a do
//! `output.log`, e a montagem da resposta da tela entram no tempo medido; a memória medida é a
//! que fica com o agrupador).
//!
//! Este arquivo é um binário de teste à parte: o alocador vale só aqui, e o único teste que
//! roda por padrão é o de memória (outro teste em paralelo bagunçaria a conta).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::print_stderr,
    unsafe_code
)]

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use warden_diagnostics::console::{ConsoleGrouper, LineLevel, LogEntry, ModDirectory};
use warden_launcher::log::{LogLevel, LogLine, LogParser, LogSource};

/// O alocador do sistema, contando os bytes vivos.
struct Counting;

static LIVE: AtomicUsize = AtomicUsize::new(0);

// SAFETY: repassa cada chamada ao alocador do sistema sem mudar o `Layout` nem o ponteiro; só
// soma e subtrai o tamanho num contador atômico.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: o mesmo contrato de `GlobalAlloc::alloc`, repassado.
        let pointer = unsafe { System.alloc(layout) };
        if !pointer.is_null() {
            LIVE.fetch_add(layout.size(), Ordering::Relaxed);
        }
        pointer
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        // SAFETY: o ponteiro veio de `alloc`/`realloc` deste alocador com este `Layout`.
        unsafe { System.dealloc(pointer, layout) };
        LIVE.fetch_sub(layout.size(), Ordering::Relaxed);
    }

    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        // SAFETY: o mesmo contrato de `GlobalAlloc::realloc`, repassado.
        let moved = unsafe { System.realloc(pointer, layout, new_size) };
        if !moved.is_null() {
            LIVE.fetch_add(new_size, Ordering::Relaxed);
            LIVE.fetch_sub(layout.size(), Ordering::Relaxed);
        }
        moved
    }
}

#[global_allocator]
static ALLOCATOR: Counting = Counting;

fn live() -> usize {
    LIVE.load(Ordering::Relaxed)
}

fn mib(bytes: usize) -> f64 {
    #[allow(clippy::cast_precision_loss)] // só para mostrar
    let value = bytes as f64;
    value / (1024.0 * 1024.0)
}

const fn level(level: LogLevel) -> LineLevel {
    match level {
        LogLevel::Trace => LineLevel::Trace,
        LogLevel::Debug => LineLevel::Debug,
        LogLevel::Info => LineLevel::Info,
        LogLevel::Warn => LineLevel::Warn,
        LogLevel::Error => LineLevel::Error,
        LogLevel::Fatal => LineLevel::Fatal,
    }
}

/// Uma palavra diferente para cada número (linhas que nunca repetem).
fn word(mut n: usize) -> String {
    let mut out = String::new();
    loop {
        out.push(char::from(b'a' + u8::try_from(n % 26).unwrap()));
        n /= 26;
        if n == 0 {
            return out;
        }
    }
}

/// O texto da linha `n` de um log grande e variado: repetições com números, linhas que nunca
/// repetem, avisos e stack traces do Fabric (intermediary) em poucos lugares do código.
fn line(n: usize, out: &mut String) -> usize {
    use std::fmt::Write as _;
    let second = n % 60;
    match n % 20 {
        0 | 10 => {
            let _ = writeln!(
                out,
                "[18:02:{second:02}] [Render thread/WARN] [minecraft/ModelBakery]: Missing texture for supplementaries:block/x{} at {n}",
                n % 37
            );
            1
        }
        1 => {
            let _ = writeln!(
                out,
                "[18:02:{second:02}] [Render thread/INFO] [minecraft/ModelBakery]: Carregando o modelo {}",
                word(n)
            );
            1
        }
        3 if n % 1000 == 3 => {
            let _ = writeln!(
                out,
                "[18:02:{second:02}] [Render thread/ERROR] [minecraft/Minecraft]: Erro no bloco em {n}, 64, -3\njava.lang.NullPointerException: null\n\tat net.minecraft.class_1937.method_{}(class_1937.java:612)\n\tat net.minecraft.class_310.method_1523(class_310.java:1205)\n\t... 12 more",
                8320 + n % 7
            );
            5
        }
        _ => {
            let _ = writeln!(
                out,
                "[18:02:{second:02}] [Server thread/INFO] [minecraft/MinecraftServer]: Salvando {n} chunks em {}, {} ({} ms)",
                n % 512,
                n % 97,
                n % 13
            );
            1
        }
    }
}

/// Lê `count` linhas, em pedaços como os do `output.log`, e devolve o agrupador e o tempo.
fn feed(count: usize) -> (ConsoleGrouper, Duration) {
    let started = Instant::now();
    let mut grouper = ConsoleGrouper::new(ModDirectory::empty());
    let mut parser = LogParser::new(LogSource::Stdout);
    let mut n = 0;
    let mut seq: u32 = 0;
    while n < count {
        let mut text = String::new();
        while n < count && text.len() < 2 * 1024 * 1024 {
            n += line(n, &mut text);
        }
        let lines: Vec<LogLine> = text.lines().flat_map(|l| parser.push(l, 0)).collect();
        let entries: Vec<LogEntry<'_>> = lines
            .iter()
            .map(|line| {
                seq += 1;
                LogEntry {
                    seq,
                    level: line.level.map(level),
                    logger: line.logger.as_deref(),
                    time: line.time.as_deref(),
                    timestamp_ms: line.timestamp_ms,
                    text: &line.text,
                }
            })
            .collect();
        grouper.push_batch(&entries);
    }
    (grouper, started.elapsed())
}

/// A memória que fica com o agrupador depois de ler `count` linhas.
fn measure(count: usize) -> (ConsoleGrouper, usize, Duration) {
    let before = live();
    let (grouper, elapsed) = feed(count);
    let retained = live().saturating_sub(before);
    (grouper, retained, elapsed)
}

/// 400.000 linhas cabem em menos de 24 MiB (antes do teto, cada ocorrência ficava guardada:
/// 36 MiB aqui e 179 MiB com 2 milhões de linhas), com as contagens certas e as
/// últimas ocorrências guardadas para "Copiar".
#[test]
fn memoria_tem_teto() {
    let (grouper, retained, elapsed) = measure(400_000);
    eprintln!(
        "400.000 linhas: {:.1} MiB guardados, {} itens, {} ocorrências guardadas, {} fora, {elapsed:?}",
        mib(retained),
        grouper.item_count(),
        grouper.kept(),
        grouper.hidden(),
    );
    assert!(retained < 24 * 1024 * 1024, "{:.1} MiB", mib(retained));
    assert_eq!(grouper.lines(), 400_000);
    // As contagens continuam completas, mesmo com o texto guardado só em parte.
    let (items, _) = grouper.group_items("game", "Missing texture", 5);
    assert_eq!(items[0].count, 40_000);
    let (problems, _) = grouper.problems("Erro no bloco", 10);
    assert_eq!(problems.len(), 7, "{problems:?}");
    assert_eq!(problems.iter().map(|item| item.count).sum::<u32>(), 400);
    // "Só problemas" não perde nada: os erros cabem mesmo depois do teto.
    assert_eq!(grouper.hidden_problems(), 0);
}

/// Medição com 2 milhões de linhas (rode com `--release --ignored --nocapture`): memória,
/// tempo de leitura e tempo de montar a tela.
#[test]
#[ignore = "medição: rodar em release"]
fn memoria_2_milhoes_de_linhas() {
    for count in [1_000_000, 2_000_000] {
        let (grouper, retained, elapsed) = measure(count);
        let started = Instant::now();
        let groups = grouper.groups("");
        let _ = grouper.group_items("game", "", 200);
        let _ = grouper.problems("", 200);
        let view = started.elapsed();
        let started = Instant::now();
        let _ = grouper.groups("texture");
        let _ = grouper.group_items("game", "texture", 200);
        let search = started.elapsed();
        let started = Instant::now();
        let copy = grouper
            .original_lines(
                &warden_diagnostics::console::LinesTarget::Group("game".to_owned()),
                None,
            )
            .len();
        let copied = started.elapsed();
        eprintln!(
            "{count} linhas: {:.1} MiB guardados, ler e agrupar {elapsed:?}, montar a tela {view:?}, buscar {search:?}, copiar o grupo ({copy} linhas) {copied:?}, {} itens, {} ocorrências guardadas, {} entradas fora, {} grupos",
            mib(retained),
            grouper.item_count(),
            grouper.kept(),
            grouper.hidden(),
            groups.len(),
        );
        drop(grouper);
    }
}
