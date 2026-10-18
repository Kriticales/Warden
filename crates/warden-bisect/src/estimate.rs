//! Estimativa de rodadas e de tempo (SPEC T25; S-R5-3 §8; decisão D23).
//!
//! - Rodadas para *n* suspeitos: 1 culpado = ⌈log₂ n⌉ + 4 (rodadas 0 e 1, busca, sozinho e o
//!   pack sem ele); par com o segundo mod na posição *p* = ⌈log₂ n⌉ + ⌈log₂ p⌉ + 5 (o "par
//!   sozinho" já é a confirmação). A faixa do diálogo vai do culpado sozinho ao par com
//!   *p* = *n*.
//! - Tempo de uma rodada ≈ W + 20 s quando passa e ≈ W quando falha (o Warden encerra o jogo
//!   no sinal), com W = tempo até "entrou no mundo" **medido na rodada 0** (o tamanho do
//!   prefixo quase não muda o tempo; S-R5-3 §6). Antes da rodada 0, W vem do último teste.
//! - Problema intermitente: cada "passou" se repete; o tempo cresce na mesma conta.
//! - O número de minutos nunca é arredondado para baixo.

use serde::{Deserialize, Serialize};

/// Estabilidade depois de entrar no mundo antes de dizer "passou".
pub const STABILITY_MS: u64 = 20_000;
/// Folga por rodada (preparar a instância, abrir e encerrar o processo).
pub const ROUND_OVERHEAD_MS: u64 = 2_000;
/// W padrão quando não há teste anterior medido.
pub const DEFAULT_WORLD_MS: u64 = 60_000;

/// `⌈log₂ n⌉` (0 para n ≤ 1).
#[must_use]
pub fn ceil_log2(n: u32) -> u32 {
    if n <= 1 {
        0
    } else {
        u32::BITS - (n - 1).leading_zeros()
    }
}

/// Rodadas para `n` suspeitos: (1 culpado, par com o segundo mod na posição `n`).
#[must_use]
pub fn rounds_for(n: u32) -> (u32, u32) {
    let log = ceil_log2(n);
    (log + 4, 2 * log + 5)
}

/// Rodadas para um par com o segundo mod na posição `p`.
#[must_use]
pub fn rounds_for_pair(n: u32, p: u32) -> u32 {
    ceil_log2(n) + ceil_log2(p) + 5
}

/// A estimativa mostrada no diálogo e durante a busca.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct Estimate {
    /// Menos rodadas (um culpado sozinho).
    pub rounds_min: u32,
    /// Mais rodadas (uma dupla).
    pub rounds_max: u32,
    /// Minutos para o menor número de rodadas (arredondado para cima).
    pub minutes_min: u32,
    /// Minutos para o maior número de rodadas (arredondado para cima).
    pub minutes_max: u32,
    /// O W usado, em milissegundos.
    #[specta(type = specta_typescript::Number)]
    pub world_ms: u64,
    /// Se o W foi medido neste pack (rodada 0 ou último teste) ou é o padrão.
    pub measured: bool,
}

/// Tempo médio de uma rodada: metade passa (W + estabilidade, repetida), metade falha (W).
fn round_ms(world_ms: u64, repetitions: u32) -> u64 {
    let pass = (world_ms + STABILITY_MS + ROUND_OVERHEAD_MS) * u64::from(repetitions.max(1));
    let fail = world_ms + ROUND_OVERHEAD_MS;
    (pass + fail).div_ceil(2)
}

fn minutes(rounds: u32, round_ms: u64) -> u32 {
    let total = u64::from(rounds) * round_ms;
    u32::try_from(total.div_ceil(60_000)).unwrap_or(u32::MAX)
}

/// A estimativa para `n` suspeitos com W (`None` = sem medida: [`DEFAULT_WORLD_MS`]).
#[must_use]
pub fn estimate(n: u32, world_ms: Option<u64>, repetitions: u32) -> Estimate {
    let (rounds_min, rounds_max) = rounds_for(n);
    from_rounds(rounds_min, rounds_max, world_ms, repetitions)
}

/// A estimativa para uma faixa de rodadas já conhecida (durante a busca, as que faltam).
#[must_use]
pub fn from_rounds(
    rounds_min: u32,
    rounds_max: u32,
    world_ms: Option<u64>,
    repetitions: u32,
) -> Estimate {
    let measured = world_ms.is_some();
    let world_ms = world_ms.unwrap_or(DEFAULT_WORLD_MS);
    let per_round = round_ms(world_ms, repetitions);
    Estimate {
        rounds_min,
        rounds_max,
        minutes_min: minutes(rounds_min, per_round),
        minutes_max: minutes(rounds_max, per_round),
        world_ms,
        measured,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn logaritmo_para_cima() {
        assert_eq!(ceil_log2(0), 0);
        assert_eq!(ceil_log2(1), 0);
        assert_eq!(ceil_log2(2), 1);
        assert_eq!(ceil_log2(3), 2);
        assert_eq!(ceil_log2(118), 7);
        assert_eq!(ceil_log2(128), 7);
        assert_eq!(ceil_log2(129), 8);
    }

    #[test]
    fn rodadas_da_spec_e_do_spike() {
        // SPEC T25: 118 mods = 11 a 19 rodadas.
        assert_eq!(rounds_for(118), (11, 19));
        // S-R5-3 §8: n = 155, p = 101 → 8 + 7 + 5 = 20 (medido).
        assert_eq!(rounds_for_pair(155, 101), 20);
    }

    #[test]
    fn tempo_da_spec_sem_arredondar_para_baixo() {
        // S-R5-3 §7: W = 69 s, 20 rodadas medidas em 26 min 55 s.
        assert_eq!(from_rounds(20, 20, Some(69_000), 1).minutes_max, 27);
        let medium = estimate(118, Some(69_000), 1);
        assert_eq!((medium.rounds_min, medium.rounds_max), (11, 19));
        assert_eq!((medium.minutes_min, medium.minutes_max), (15, 26));
        assert!(medium.measured);
        // 300 mods que levam 2 min 30 s: a dupla passa de uma hora.
        let big = estimate(300, Some(150_000), 1);
        assert!(big.minutes_max >= 60, "{big:?}");
        // Intermitente custa mais.
        assert!(estimate(118, Some(69_000), 5).minutes_max > medium.minutes_max);
        // Sem medida, o padrão.
        assert!(!estimate(10, None, 1).measured);
    }
}
