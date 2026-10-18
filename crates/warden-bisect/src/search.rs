//! A busca por rodadas (ARCHITECTURE §9.7 itens 3 a 8; R5A §3.2; S-R5-3 §7 e §8).
//!
//! [`Search`] é uma máquina de estados pura: [`Search::next_round`] diz o que testar e
//! [`Search::record`] recebe o veredito. Quem abre o jogo é o chamador (o app; nos testes, um
//! executor simulado). O estado inteiro é serializável (`state/bisect.json`), então pausar e
//! continuar outro dia é só gravar e ler.
//!
//! 1. **Rodada 0:** o pack inteiro, até 3 tentativas; a tentativa em que o problema aparece
//!    dá a taxa de reprodução *p* (1, ½ ou ⅓). Com *p* < 1, cada "passou" só vale depois de
//!    `⌈ln 0,05 / ln(1−p)⌉` repetições (R5A §3.6). Não reproduziu em 3: para.
//! 2. **Rodada 1 (controle):** só os sempre ligados. Falhou igual: a causa não é um mod.
//! 3. **Busca binária no prefixo** da ordem topológica (`lo` = maior que passou, `hi` = menor
//!    que falhou) até `hi = lo + 1`: a unidade em `hi` é o último membro necessário.
//! 4. **Sozinho:** só o conjunto achado (com as dependências). Falhou igual: é o conjunto
//!    mínimo. Passou: falta um parceiro, procurado do mesmo jeito entre as unidades anteriores,
//!    com o conjunto sempre ligado (variante do *ddmin*).
//! 5. **Confirmação:** o pack sem o conjunto (e sem quem depende dele) precisa passar. Se
//!    falhar igual, há outro problema: [`Search::continue_without`] recomeça sem ele.
//!
//! "Travou diferente", tempo esgotado e "não deu para testar" são inconclusivos: a mesma
//! rodada se repete (até 3 vezes seguidas). Uma dependência inferida muda a ordem e recomeça a
//! busca binária sem repetir as rodadas 0 e 1. Um conjunto que passa contendo um que falhou
//! igual é contradição: a busca para e explica.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::universe::{Role, Universe};

/// Tentativas da rodada 0.
pub const REPRODUCE_ATTEMPTS: u32 = 3;
/// Resultados inconclusivos seguidos que param a busca.
pub const MAX_INCONCLUSIVE: u32 = 3;
/// Tamanho máximo do conjunto mínimo. Um conjunto de 3 que ainda passa sozinho quer dizer
/// que o problema não cresce com os mods ligados (um mod evita o problema, ou ele é
/// intermitente demais): a busca para e explica.
pub const MAX_CULPRITS: usize = 3;

/// O veredito de uma rodada.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Verdict {
    /// Entrou no mundo (ou chegou ao marcador pedido) e ficou estável.
    Passed,
    /// Travou com a mesma assinatura do problema.
    FailedSame,
    /// Travou com outra assinatura: não conta (como o `skip` do `git bisect`).
    FailedOther,
    /// Não chegou a nenhum marcador no tempo-limite.
    Timeout,
    /// Java errado ou outro problema de ambiente: não é mod, a busca para.
    Environment,
    /// Faltou uma classe ou um mod de outro item desligado: `from` precisa de `to`.
    Inferred {
        /// Caminho do item que precisa.
        from: String,
        /// Caminho do item de que ele precisa.
        to: String,
    },
    /// Modo assistido: "Aconteceu".
    UserYes,
    /// Modo assistido: "Não aconteceu".
    UserNo,
    /// Modo assistido: "Não deu para testar".
    UserSkip,
}

/// Como o veredito conta para o algoritmo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Effect {
    Pass,
    Fail,
    Inconclusive,
}

impl Verdict {
    fn effect(&self) -> Effect {
        match self {
            Self::Passed | Self::UserNo => Effect::Pass,
            Self::FailedSame | Self::UserYes => Effect::Fail,
            _ => Effect::Inconclusive,
        }
    }
}

/// O tipo de uma rodada.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum RoundKind {
    /// Rodada 0: o pack inteiro.
    Reproduce {
        /// Tentativa (1 a 3).
        attempt: u32,
    },
    /// Rodada 1: só os sempre ligados.
    Control,
    /// Os primeiros `size` de `of` suspeitos (mais o conjunto já achado, se houver).
    Prefix {
        /// Quantos suspeitos ligados.
        size: u32,
        /// Quantos suspeitos nesta etapa.
        of: u32,
        /// O conjunto já achado vai junto (busca do parceiro).
        with_found: bool,
    },
    /// Só o conjunto achado.
    Alone,
    /// O pack sem o conjunto achado.
    Without,
}

/// O que testar na próxima rodada.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct RoundPlan {
    /// Número da rodada (a primeira é a 0).
    pub number: u32,
    /// Tipo.
    pub kind: RoundKind,
    /// Repetição desta rodada (1 = primeira vez; problema intermitente repete o "passou").
    pub repeat: u32,
    /// Caminhos dos itens ligados (com as dependências e os sempre ligados).
    pub enabled: Vec<String>,
    /// Nomes dos suspeitos acrescentados desde o último prefixo que passou: se travar igual,
    /// o culpado está entre eles.
    pub added: Vec<String>,
    /// Nomes do conjunto já achado.
    pub found: Vec<String>,
    /// Quantos suspeitos ainda há.
    pub suspects: u32,
}

/// Uma rodada terminada.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct RoundRecord {
    /// Número.
    pub number: u32,
    /// Tipo.
    pub kind: RoundKind,
    /// Repetição.
    pub repeat: u32,
    /// Quantos itens ligados.
    pub enabled_count: u32,
    /// O veredito.
    pub verdict: Verdict,
}

/// Confiança no resultado.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum CulpritConfidence {
    /// Confirmado nos 2 testes de controle, com o problema aparecendo sempre.
    High,
    /// Confirmado, mas o problema é intermitente (cada "passou" foi repetido).
    Medium,
}

/// Como a busca terminou.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum SearchOutcome {
    /// Culpado (ou combinação) encontrado.
    Found {
        /// Caminhos do conjunto mínimo.
        culprits: Vec<String>,
        /// Confiança.
        confidence: CulpritConfidence,
        /// O pack sem o conjunto ainda falha igual: há outro problema.
        another_problem: bool,
    },
    /// O problema não apareceu em 3 testes seguidos na rodada 0.
    NotReproduced,
    /// Mesmo sem mods o jogo falha igual.
    NotAMod,
    /// Java errado ou outro problema do ambiente.
    Environment,
    /// Os testes se contradizem; `preventers` parecem evitar o problema.
    Contradiction {
        /// Caminhos dos itens que parecem evitar o problema.
        preventers: Vec<String>,
    },
    /// Resultados inconclusivos demais seguidos.
    Inconclusive {
        /// Caminhos dos suspeitos até ali.
        suspects: Vec<String>,
    },
    /// Cancelada pelo usuário.
    Cancelled {
        /// Caminhos dos suspeitos até ali ("o culpado está entre estes N mods").
        suspects: Vec<String>,
    },
}

/// A etapa da busca.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "phase", rename_all = "camelCase")]
enum Phase {
    Reproduce {
        passes: u32,
    },
    Control,
    Narrow {
        /// Unidades suspeitas, na ordem.
        candidates: Vec<usize>,
        /// Unidades já achadas (sempre ligadas nesta etapa).
        fixed: Vec<usize>,
        lo: usize,
        hi: usize,
    },
    Alone {
        /// Onde procurar o parceiro se o conjunto sozinho passar.
        candidates: Vec<usize>,
        fixed: Vec<usize>,
    },
    Without {
        culprits: Vec<usize>,
    },
    Done {
        outcome: SearchOutcome,
    },
}

/// Um conjunto testado com resultado conclusivo (para achar contradições).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Tested {
    items: BTreeSet<usize>,
    failed: bool,
}

/// O estado da busca (gravado em `state/bisect.json` depois de cada rodada).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Search {
    universe: Universe,
    phase: Phase,
    rounds: Vec<RoundRecord>,
    /// Repetições que um "passou" precisa (1 = problema que aparece sempre).
    repetitions: u32,
    /// Taxa de reprodução estimada na rodada 0.
    rate: Option<f64>,
    /// "Passou" seguidos na rodada atual (intermitente).
    passes: u32,
    /// Inconclusivos seguidos na rodada atual.
    inconclusive: u32,
    /// Itens tirados por [`Search::continue_without`].
    excluded: Vec<usize>,
    /// Conjuntos achados antes (outro problema).
    earlier: Vec<Vec<String>>,
    /// Dependências inferidas nesta busca (`from` → `to`).
    inferred: Vec<(String, String)>,
    tested: Vec<Tested>,
}

impl Search {
    /// Começa a busca.
    #[must_use]
    pub fn new(universe: Universe) -> Self {
        Self {
            universe,
            phase: Phase::Reproduce { passes: 0 },
            rounds: Vec::new(),
            repetitions: 1,
            rate: None,
            passes: 0,
            inconclusive: 0,
            excluded: Vec::new(),
            earlier: Vec::new(),
            inferred: Vec::new(),
            tested: Vec::new(),
        }
    }

    /// Os itens e a ordem.
    #[must_use]
    pub fn universe(&self) -> &Universe {
        &self.universe
    }

    /// As rodadas feitas.
    #[must_use]
    pub fn rounds(&self) -> &[RoundRecord] {
        &self.rounds
    }

    /// Repetições de cada "passou".
    #[must_use]
    pub fn repetitions(&self) -> u32 {
        self.repetitions
    }

    /// Taxa de reprodução estimada (depois da rodada 0).
    #[must_use]
    pub fn rate(&self) -> Option<f64> {
        self.rate
    }

    /// Dependências inferidas nesta busca.
    #[must_use]
    pub fn inferred(&self) -> &[(String, String)] {
        &self.inferred
    }

    /// Conjuntos achados antes de [`Search::continue_without`].
    #[must_use]
    pub fn earlier(&self) -> &[Vec<String>] {
        &self.earlier
    }

    /// Como terminou, se terminou.
    #[must_use]
    pub fn outcome(&self) -> Option<&SearchOutcome> {
        match &self.phase {
            Phase::Done { outcome } => Some(outcome),
            _ => None,
        }
    }

    /// Se a rodada 0 já reproduziu o problema.
    #[must_use]
    pub fn reproduced(&self) -> bool {
        self.rate.is_some()
    }

    /// As unidades procuradas que ainda podem ser o culpado.
    fn suspect_units(&self) -> Vec<usize> {
        match &self.phase {
            Phase::Reproduce { .. } | Phase::Control => self.remaining_units(),
            Phase::Narrow {
                candidates,
                fixed,
                lo,
                hi,
            } => {
                let mut units: Vec<usize> = candidates
                    .get(*lo..*hi)
                    .map(<[usize]>::to_vec)
                    .unwrap_or_default();
                units.extend(fixed);
                units
            }
            Phase::Alone { fixed, .. } | Phase::Without { culprits: fixed } => fixed.clone(),
            Phase::Done { outcome } => match outcome {
                SearchOutcome::Found { culprits, .. } => self.units_of_paths(culprits),
                SearchOutcome::Cancelled { suspects }
                | SearchOutcome::Inconclusive { suspects } => self.units_of_paths(suspects),
                _ => Vec::new(),
            },
        }
    }

    /// Caminhos dos suspeitos (para "o culpado está entre estes N mods").
    #[must_use]
    pub fn suspects(&self) -> Vec<String> {
        self.universe.unit_paths(&self.suspect_units())
    }

    fn units_of_paths(&self, paths: &[String]) -> Vec<usize> {
        let mut units: Vec<usize> = paths
            .iter()
            .filter_map(|path| self.universe.index_of(path))
            .filter_map(|item| self.universe.unit_of(item))
            .collect();
        units.dedup();
        units
    }

    /// As unidades procuradas que ainda entram (sem as tiradas por "continuar sem").
    fn remaining_units(&self) -> Vec<usize> {
        let excluded: BTreeSet<usize> = self
            .excluded
            .iter()
            .filter_map(|&item| self.universe.unit_of(item))
            .collect();
        self.universe
            .order()
            .iter()
            .copied()
            .filter(|unit| !excluded.contains(unit))
            .collect()
    }

    /// O que testar agora (`None` se terminou).
    #[must_use]
    pub fn next_round(&self) -> Option<RoundPlan> {
        let number = u32::try_from(self.rounds.len()).unwrap_or(u32::MAX);
        let repeat = self.passes + 1;
        let (kind, units, added, found): (RoundKind, Vec<usize>, Vec<usize>, Vec<usize>) =
            match &self.phase {
                Phase::Done { .. } => return None,
                Phase::Reproduce { passes } => (
                    RoundKind::Reproduce {
                        attempt: passes + self.inconclusive + 1,
                    },
                    self.remaining_units(),
                    Vec::new(),
                    Vec::new(),
                ),
                Phase::Control => (RoundKind::Control, Vec::new(), Vec::new(), Vec::new()),
                Phase::Narrow {
                    candidates,
                    fixed,
                    lo,
                    hi,
                } => {
                    let mid = lo + (hi - lo) / 2;
                    let mut units: Vec<usize> = candidates[..mid].to_vec();
                    units.extend(fixed);
                    (
                        RoundKind::Prefix {
                            size: count(mid),
                            of: count(candidates.len()),
                            with_found: !fixed.is_empty(),
                        },
                        units,
                        candidates[*lo..mid].to_vec(),
                        fixed.clone(),
                    )
                }
                Phase::Alone { fixed, .. } => {
                    (RoundKind::Alone, fixed.clone(), Vec::new(), fixed.clone())
                }
                Phase::Without { culprits } => (
                    RoundKind::Without,
                    self.without(culprits),
                    Vec::new(),
                    culprits.clone(),
                ),
            };
        let enabled = self.universe.closure(&units);
        Some(RoundPlan {
            number,
            kind,
            repeat,
            enabled: enabled
                .iter()
                .map(|&item| self.universe.items()[item].path.clone())
                .collect(),
            added: self.universe.unit_names(&added),
            found: self.universe.unit_names(&found),
            suspects: count(self.suspect_units().len()),
        })
    }

    /// As unidades do pack sem o conjunto e sem quem depende dele.
    fn without(&self, culprits: &[usize]) -> Vec<usize> {
        let gone: BTreeSet<usize> = self
            .universe
            .dependents(culprits)
            .into_iter()
            .chain(culprits.iter().copied())
            .collect();
        self.remaining_units()
            .into_iter()
            .filter(|unit| !gone.contains(unit))
            .collect()
    }

    /// Registra o veredito da rodada de [`Search::next_round`].
    pub fn record(&mut self, verdict: Verdict) {
        let Some(plan) = self.next_round() else {
            return;
        };
        self.rounds.push(RoundRecord {
            number: plan.number,
            kind: plan.kind.clone(),
            repeat: plan.repeat,
            enabled_count: count(plan.enabled.len()),
            verdict: verdict.clone(),
        });
        let enabled: BTreeSet<usize> = plan
            .enabled
            .iter()
            .filter_map(|path| self.universe.index_of(path))
            .collect();
        match verdict {
            Verdict::Environment => {
                self.finish(SearchOutcome::Environment);
                return;
            }
            Verdict::Inferred { from, to } => {
                if self.universe.add_requirement(&from, &to) {
                    self.inferred.push((from, to));
                    self.restart_after_inference();
                } else {
                    self.inconclusive_round();
                }
                return;
            }
            _ => {}
        }
        match verdict.effect() {
            Effect::Inconclusive => self.inconclusive_round(),
            Effect::Fail => {
                self.inconclusive = 0;
                self.passes = 0;
                if self.contradicts(&enabled, true) {
                    return;
                }
                self.tested.push(Tested {
                    items: enabled,
                    failed: true,
                });
                self.on_fail();
            }
            Effect::Pass => {
                self.inconclusive = 0;
                self.passes += 1;
                // Reprodução: cada tentativa conta uma vez.
                if matches!(self.phase, Phase::Reproduce { .. }) {
                    self.passes = 0;
                    self.on_reproduce_pass();
                    return;
                }
                if self.passes < self.repetitions {
                    return;
                }
                self.passes = 0;
                if self.contradicts(&enabled, false) {
                    return;
                }
                self.tested.push(Tested {
                    items: enabled,
                    failed: false,
                });
                self.on_pass();
            }
        }
    }

    fn inconclusive_round(&mut self) {
        self.inconclusive += 1;
        if self.inconclusive >= MAX_INCONCLUSIVE {
            let suspects = self.suspects();
            self.finish(SearchOutcome::Inconclusive { suspects });
        }
    }

    /// Um conjunto que passa contendo um que falhou igual (ou o contrário) quebra a regra da
    /// busca: para e explica quem parece evitar o problema.
    fn contradicts(&mut self, set: &BTreeSet<usize>, failed: bool) -> bool {
        let conflict = self.tested.iter().find(|tested| {
            tested.failed != failed
                && if failed {
                    set.is_subset(&tested.items)
                } else {
                    tested.items.is_subset(set)
                }
        });
        let Some(conflict) = conflict else {
            return false;
        };
        let (bigger, smaller) = if failed {
            (&conflict.items, set)
        } else {
            (set, &conflict.items)
        };
        let preventers: Vec<String> = bigger
            .difference(smaller)
            .filter(|&&item| self.universe.role(item) == Role::Searched)
            .map(|&item| self.universe.items()[item].path.clone())
            .collect();
        self.finish(SearchOutcome::Contradiction { preventers });
        true
    }

    fn on_reproduce_pass(&mut self) {
        let Phase::Reproduce { passes } = &mut self.phase else {
            return;
        };
        *passes += 1;
        if *passes >= REPRODUCE_ATTEMPTS {
            self.finish(SearchOutcome::NotReproduced);
        }
    }

    fn on_fail(&mut self) {
        match std::mem::replace(&mut self.phase, Phase::Control) {
            Phase::Reproduce { passes } => {
                let rate = 1.0 / f64::from(passes + 1);
                self.rate = Some(rate);
                self.repetitions = repetitions_for(rate);
                self.phase = Phase::Control;
            }
            Phase::Control => self.finish(SearchOutcome::NotAMod),
            Phase::Narrow {
                candidates,
                fixed,
                lo,
                hi,
            } => {
                let mid = lo + (hi - lo) / 2;
                self.phase = Phase::Narrow {
                    candidates,
                    fixed,
                    lo,
                    hi: mid,
                };
                self.settle();
            }
            Phase::Alone { fixed, .. } => {
                self.phase = Phase::Without { culprits: fixed };
            }
            Phase::Without { culprits } => {
                let confidence = self.confidence();
                self.finish(SearchOutcome::Found {
                    culprits: self.universe.unit_paths(&culprits),
                    confidence,
                    another_problem: true,
                });
            }
            done @ Phase::Done { .. } => self.phase = done,
        }
    }

    fn on_pass(&mut self) {
        match std::mem::replace(&mut self.phase, Phase::Control) {
            phase @ Phase::Reproduce { .. } => self.phase = phase,
            Phase::Control => self.start_narrow(Vec::new()),
            Phase::Narrow {
                candidates,
                fixed,
                lo,
                hi,
            } => {
                let mid = lo + (hi - lo) / 2;
                self.phase = Phase::Narrow {
                    candidates,
                    fixed,
                    lo: mid,
                    hi,
                };
                self.settle();
            }
            Phase::Alone { candidates, fixed } => {
                if candidates.is_empty() || fixed.len() >= MAX_CULPRITS {
                    // O conjunto sozinho passou, mas com os mesmos itens falhou antes.
                    self.finish(SearchOutcome::Contradiction {
                        preventers: Vec::new(),
                    });
                    return;
                }
                let hi = candidates.len();
                self.phase = Phase::Narrow {
                    candidates,
                    fixed,
                    lo: 0,
                    hi,
                };
                self.settle();
            }
            Phase::Without { culprits } => {
                let confidence = self.confidence();
                self.finish(SearchOutcome::Found {
                    culprits: self.universe.unit_paths(&culprits),
                    confidence,
                    another_problem: false,
                });
            }
            done @ Phase::Done { .. } => self.phase = done,
        }
    }

    fn confidence(&self) -> CulpritConfidence {
        if self.repetitions > 1 {
            CulpritConfidence::Medium
        } else {
            CulpritConfidence::High
        }
    }

    /// Começa a busca binária sobre as unidades que restam (o pack inteiro, menos o que
    /// "continuar sem" tirou, já falhou; nenhuma passou).
    fn start_narrow(&mut self, fixed: Vec<usize>) {
        let candidates = self.remaining_units();
        let hi = candidates.len();
        if hi == 0 {
            self.finish(SearchOutcome::Contradiction {
                preventers: Vec::new(),
            });
            return;
        }
        self.phase = Phase::Narrow {
            candidates,
            fixed,
            lo: 0,
            hi,
        };
        self.settle();
    }

    /// Fecha a busca binária quando sobra um suspeito: ele entra no conjunto e a próxima
    /// rodada o testa sozinho.
    fn settle(&mut self) {
        let Phase::Narrow {
            candidates,
            fixed,
            lo,
            hi,
        } = &self.phase
        else {
            return;
        };
        if *hi > lo + 1 {
            return;
        }
        let Some(&culprit) = candidates.get(hi.saturating_sub(1)) else {
            return;
        };
        let mut fixed = fixed.clone();
        fixed.push(culprit);
        // O parceiro vem antes; quem já entra com o conjunto não pode ser o parceiro.
        let along: BTreeSet<usize> = self
            .universe
            .closure(&fixed)
            .into_iter()
            .filter_map(|item| self.universe.unit_of(item))
            .collect();
        let candidates: Vec<usize> = candidates[..hi.saturating_sub(1)]
            .iter()
            .copied()
            .filter(|unit| !along.contains(unit))
            .collect();
        self.phase = Phase::Alone { candidates, fixed };
    }

    /// Uma dependência nova mudou a ordem: recomeça a busca binária (as rodadas 0 e 1 valem).
    fn restart_after_inference(&mut self) {
        self.passes = 0;
        self.inconclusive = 0;
        // Os conjuntos testados continuam valendo (eram conjuntos de itens).
        match self.phase {
            Phase::Reproduce { .. } | Phase::Control | Phase::Done { .. } => {}
            _ => self.start_narrow(Vec::new()),
        }
    }

    fn finish(&mut self, outcome: SearchOutcome) {
        self.phase = Phase::Done { outcome };
    }

    /// Cancela: guarda os suspeitos até aqui como resultado parcial.
    pub fn cancel(&mut self) {
        if self.outcome().is_some() {
            return;
        }
        let suspects = self.suspects();
        self.finish(SearchOutcome::Cancelled { suspects });
    }

    /// Depois de "Há outro problema além deste": recomeça sem o conjunto achado (e sem quem
    /// depende dele). O pack sem ele já falhou igual, e a rodada de controle já passou.
    /// Devolve `false` se a busca não terminou assim.
    pub fn continue_without(&mut self) -> bool {
        let Some(SearchOutcome::Found {
            culprits,
            another_problem: true,
            ..
        }) = self.outcome().cloned()
        else {
            return false;
        };
        let units = self.units_of_paths(&culprits);
        let gone: Vec<usize> = self
            .universe
            .dependents(&units)
            .into_iter()
            .chain(units)
            .flat_map(|unit| self.universe.unit_items(unit).to_vec())
            .collect();
        self.excluded.extend(gone);
        self.excluded.sort_unstable();
        self.excluded.dedup();
        self.earlier.push(culprits);
        self.tested.clear();
        self.start_narrow(Vec::new());
        true
    }

    /// Quantas rodadas ainda faltam, no mínimo e no máximo (sem contar repetições).
    #[must_use]
    pub fn remaining_rounds(&self) -> (u32, u32) {
        let searched = count(self.remaining_units().len());
        match &self.phase {
            Phase::Done { .. } => (0, 0),
            Phase::Reproduce { .. } => {
                let (min, max) = crate::estimate::rounds_for(searched);
                (min, max)
            }
            Phase::Control => {
                let (min, max) = crate::estimate::rounds_for(searched);
                (min.saturating_sub(1), max.saturating_sub(1))
            }
            Phase::Narrow { fixed, lo, hi, .. } => {
                let left = crate::estimate::ceil_log2(count(hi - lo));
                if fixed.is_empty() {
                    let partner = crate::estimate::ceil_log2(count(hi.saturating_sub(1)));
                    (left + 2, left + partner + 3)
                } else {
                    (left + 2, left + 2)
                }
            }
            Phase::Alone { candidates, .. } => {
                let partner = crate::estimate::ceil_log2(count(candidates.len()));
                (2, partner + 3)
            }
            Phase::Without { .. } => (1, 1),
        }
    }
}

/// `⌈ln 0,05 / ln(1−p)⌉` repetições para 95% de confiança num "passou" (R5A §3.6).
#[must_use]
pub fn repetitions_for(rate: f64) -> u32 {
    if rate >= 1.0 {
        return 1;
    }
    if rate <= 0.0 {
        return u32::MAX;
    }
    let value = (0.05_f64.ln() / (1.0 - rate).ln()).ceil();
    if value.is_finite() && value >= 1.0 {
        // Até 100 repetições: acima disso a busca não faz sentido.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let repetitions = value.min(100.0) as u32;
        repetitions
    } else {
        1
    }
}

fn count(len: usize) -> u32 {
    u32::try_from(len).unwrap_or(u32::MAX)
}

#[cfg(test)]
mod tests;
