//! Resumo das metas da SPEC §8 para o `docs/DESEMPENHO.md` (A-04).
//!
//! O criterion mede cada operação com estatística (média com intervalo de confiança); este
//! resumo roda a mesma operação algumas vezes e imprime a **mediana** e o **pior caso** ao lado
//! da meta, numa linha fácil de copiar:
//!
//! `A-04 | <medida> | mediana 12.3 ms | pior 15.0 ms | meta < 1000 ms | ok`
//!
//! Incluído pelos benchmarks com `#[path = ".../medida.rs"] mod medida;`.

#![allow(dead_code, unreachable_pub, clippy::print_stderr)]

use std::time::{Duration, Instant};

/// Quantas vezes o resumo roda cada medida.
pub const VEZES: usize = 10;

/// Mede `operacao` [`VEZES`] vezes, cada uma com a entrada nova de `preparar` (fora do tempo),
/// e imprime a mediana e o pior caso contra `meta`. Devolve (mediana, pior).
pub fn resumir<E, S>(
    medida: &str,
    meta: Duration,
    mut preparar: impl FnMut() -> E,
    mut operacao: impl FnMut(E) -> S,
) -> (Duration, Duration) {
    let mut tempos = Vec::with_capacity(VEZES);
    for _ in 0..VEZES {
        let entrada = preparar();
        let inicio = Instant::now();
        let saida = operacao(entrada);
        tempos.push(inicio.elapsed());
        drop(std::hint::black_box(saida));
    }
    tempos.sort();
    let mediana = tempos[tempos.len() / 2];
    let pior = tempos[tempos.len() - 1];
    let estado = if pior < meta { "ok" } else { "ESTOUROU" };
    eprintln!(
        "A-04 | {medida} | mediana {} | pior {} | meta < {} ms | {estado}",
        ms(mediana),
        ms(pior),
        meta.as_millis()
    );
    (mediana, pior)
}

fn ms(tempo: Duration) -> String {
    format!("{:.1} ms", tempo.as_secs_f64() * 1000.0)
}
