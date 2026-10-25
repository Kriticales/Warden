//! O critério de "versão mais pesada" (SPEC T33; ADR-0047), como função pura.
//!
//! Compara os testes comparáveis de uma versão `V` com os da versão salva anterior `U` pela
//! **mediana** de cada métrica. `V` é mais pesada quando:
//!
//! - o **tempo para abrir** é pelo menos 20% maior **e** 10 s a mais;
//! - ou a **memória máxima** é pelo menos 20% maior **e** 512 MB a mais;
//! - ou o **tempo por tick** (só quando as duas têm dados do spark) é pelo menos 25% maior
//!   **e** 5 ms a mais.
//!
//! Com menos de 2 testes comparáveis em alguma das versões, não há veredito ("Poucos testes
//! para comparar"). Quem escolhe os testes comparáveis (mesmo perfil, mesmo computador, modo
//! normal, fora a primeira abertura) é o chamador; aqui só entram os números.

use serde::{Deserialize, Serialize};

/// Mínimo de testes comparáveis em cada versão.
pub const MIN_TESTS: usize = 2;

/// Os números de um teste comparável.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sample {
    /// Tempo até o jogo carregar, em ms.
    pub loaded_ms: u32,
    /// Pico da RAM do processo, em MB (`None` sem amostra).
    pub ram_peak_mb: Option<u32>,
    /// Tempo por tick do spark, em ms (`None` sem perfil do spark).
    pub tick_ms: Option<f64>,
}

/// A métrica que passou do critério.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum Metric {
    /// Tempo para abrir.
    LoadTime,
    /// Memória máxima.
    Memory,
    /// Tempo por tick (spark).
    Tick,
}

/// A versão mais pesada: a métrica e as duas medianas (ms, MB ou ms por tick).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Heavier {
    /// A primeira métrica, na ordem do critério, que passou.
    pub metric: Metric,
    /// Mediana da versão comparada.
    pub current: f64,
    /// Mediana da versão anterior.
    pub previous: f64,
}

impl Heavier {
    /// Quanto a mais, em % (arredondado para baixo, para nunca exagerar).
    #[must_use]
    pub fn percent(&self) -> u32 {
        if self.previous <= 0.0 {
            return 0;
        }
        let value = ((self.current - self.previous) / self.previous * 100.0).floor();
        if value <= 0.0 {
            0
        } else if value >= f64::from(u32::MAX) {
            u32::MAX
        } else {
            // Já é inteiro, positivo e menor que `u32::MAX`.
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let percent = value as u32;
            percent
        }
    }
}

/// O resultado da comparação.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Verdict {
    /// Passou do critério.
    Heavier(Heavier),
    /// Menos de [`MIN_TESTS`] testes comparáveis numa das versões.
    FewTests,
    /// Não passou do critério.
    NotHeavier,
}

/// Mediana (média dos dois do meio quando a quantidade é par). `None` sem valores.
#[must_use]
pub fn median(values: &[f64]) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    let middle = sorted.len() / 2;
    Some(if sorted.len().is_multiple_of(2) {
        f64::midpoint(sorted[middle - 1], sorted[middle])
    } else {
        sorted[middle]
    })
}

/// Um limiar: pelo menos `ratio` a mais **e** `absolute` a mais.
struct Threshold {
    ratio: f64,
    absolute: f64,
}

impl Threshold {
    fn passed(&self, current: f64, previous: f64) -> bool {
        current - previous >= self.absolute && current >= previous * (1.0 + self.ratio)
    }
}

const LOAD: Threshold = Threshold {
    ratio: 0.20,
    absolute: 10_000.0,
};
const MEMORY: Threshold = Threshold {
    ratio: 0.20,
    absolute: 512.0,
};
const TICK: Threshold = Threshold {
    ratio: 0.25,
    absolute: 5.0,
};

/// Uma métrica do critério: qual é, o limiar e como ler o valor de um teste.
type Check = (Metric, &'static Threshold, fn(&Sample) -> Option<f64>);

/// As medianas de uma métrica nas duas versões (`None` quando alguma não tem dado).
fn medians(
    current: &[Sample],
    previous: &[Sample],
    value: impl Fn(&Sample) -> Option<f64>,
) -> Option<(f64, f64)> {
    let of = |samples: &[Sample]| {
        let values: Vec<f64> = samples.iter().filter_map(&value).collect();
        median(&values)
    };
    Some((of(current)?, of(previous)?))
}

/// Compara `current` (a versão `V`) com `previous` (a versão salva anterior `U`).
#[must_use]
pub fn compare(current: &[Sample], previous: &[Sample]) -> Verdict {
    if current.len() < MIN_TESTS || previous.len() < MIN_TESTS {
        return Verdict::FewTests;
    }
    let checks: [Check; 3] = [
        (Metric::LoadTime, &LOAD, |sample| {
            Some(f64::from(sample.loaded_ms))
        }),
        (Metric::Memory, &MEMORY, |sample| {
            sample.ram_peak_mb.map(f64::from)
        }),
        (Metric::Tick, &TICK, |sample| sample.tick_ms),
    ];
    for (metric, threshold, value) in checks {
        if let Some((now, before)) = medians(current, previous, value)
            && threshold.passed(now, before)
        {
            return Verdict::Heavier(Heavier {
                metric,
                current: now,
                previous: before,
            });
        }
    }
    Verdict::NotHeavier
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    fn load(seconds: &[u32]) -> Vec<Sample> {
        seconds
            .iter()
            .map(|s| Sample {
                loaded_ms: s * 1000,
                ram_peak_mb: None,
                tick_ms: None,
            })
            .collect()
    }

    fn memory(mb: &[u32]) -> Vec<Sample> {
        mb.iter()
            .map(|mb| Sample {
                loaded_ms: 60_000,
                ram_peak_mb: Some(*mb),
                tick_ms: None,
            })
            .collect()
    }

    #[test]
    fn mediana_impar_par_e_vazia() {
        assert_eq!(median(&[3.0, 1.0, 2.0]), Some(2.0));
        assert_eq!(median(&[4.0, 1.0, 3.0, 2.0]), Some(2.5));
        assert_eq!(median(&[]), None);
    }

    /// CA-T33-03: 1.4.2 com 100, 104 e 98 s e 1.5.0 com 125 e 128 s gera "mais lenta".
    #[test]
    fn mais_lenta_pela_tabela_da_spec() {
        let Verdict::Heavier(heavier) = compare(&load(&[125, 128]), &load(&[100, 104, 98])) else {
            panic!("devia ser mais pesada");
        };
        assert_eq!(heavier.metric, Metric::LoadTime);
        assert!((heavier.current - 126_500.0).abs() < f64::EPSILON);
        assert!((heavier.previous - 100_000.0).abs() < f64::EPSILON);
        assert_eq!(heavier.percent(), 26);
    }

    /// CA-T33-03: um teste só gera "Poucos testes para comparar".
    #[test]
    fn um_teste_so_e_poucos_testes() {
        assert_eq!(
            compare(&load(&[125]), &load(&[100, 104, 98])),
            Verdict::FewTests
        );
        assert_eq!(
            compare(&load(&[125, 128]), &load(&[100])),
            Verdict::FewTests
        );
    }

    /// CA-T33-03: +8% não gera aviso.
    #[test]
    fn oito_por_cento_nao_e_mais_pesada() {
        assert_eq!(
            compare(&load(&[110, 112]), &load(&[100, 104, 98])),
            Verdict::NotHeavier
        );
    }

    /// CA-T33-03: memória de 5,0 GB para 5,4 GB (+8%, +400 MB) não gera aviso.
    #[test]
    fn memoria_com_pouca_diferenca_nao_e_mais_pesada() {
        assert_eq!(
            compare(&memory(&[5530, 5530]), &memory(&[5120, 5120])),
            Verdict::NotHeavier
        );
    }

    #[test]
    fn memoria_bem_maior_e_mais_pesada() {
        let Verdict::Heavier(heavier) = compare(&memory(&[6400, 6500]), &memory(&[5000, 5100]))
        else {
            panic!("devia ser mais pesada");
        };
        assert_eq!(heavier.metric, Metric::Memory);
    }

    /// Os dois limiares valem juntos: 30% a mais num pack que abre em 20 s são só 6 s.
    #[test]
    fn porcentagem_alta_com_poucos_segundos_nao_e_mais_pesada() {
        assert_eq!(
            compare(&load(&[26, 26]), &load(&[20, 20])),
            Verdict::NotHeavier
        );
    }

    #[test]
    fn tick_so_quando_as_duas_tem_spark() {
        let tick = |ms: f64| Sample {
            loaded_ms: 60_000,
            ram_peak_mb: None,
            tick_ms: Some(ms),
        };
        let Verdict::Heavier(heavier) =
            compare(&[tick(40.0), tick(42.0)], &[tick(30.0), tick(30.0)])
        else {
            panic!("devia ser mais pesada");
        };
        assert_eq!(heavier.metric, Metric::Tick);
        // Sem spark na anterior: o tick não entra.
        assert_eq!(
            compare(&[tick(40.0), tick(42.0)], &load(&[60, 60])),
            Verdict::NotHeavier
        );
    }

    proptest! {
        /// Uma versão nunca é mais pesada que ela mesma.
        #[test]
        fn nunca_mais_pesada_que_ela_mesma(
            values in prop::collection::vec((1u32..600_000, prop::option::of(1u32..32_000)), 2..8),
        ) {
            let samples: Vec<Sample> = values
                .iter()
                .map(|(loaded_ms, ram)| Sample { loaded_ms: *loaded_ms, ram_peak_mb: *ram, tick_ms: None })
                .collect();
            prop_assert_eq!(compare(&samples, &samples), Verdict::NotHeavier);
        }

        /// Mais lenta em tudo nunca deixa de ser mais lenta quando o tempo sobe ainda mais.
        #[test]
        fn subir_o_tempo_nao_tira_o_aviso(
            base in 30_000u32..300_000,
            extra in 0u32..300_000,
        ) {
            let previous = load(&[base / 1000, base / 1000]);
            let slow = base / 1000 * 2 + 10;
            let first = compare(&load(&[slow, slow]), &previous);
            let slower = slow + extra / 1000;
            let second = compare(&load(&[slower, slower]), &previous);
            prop_assert!(matches!(first, Verdict::Heavier(_)));
            prop_assert!(matches!(second, Verdict::Heavier(_)));
        }
    }
}
