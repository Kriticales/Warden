//! A comparação entre versões salvas pelo critério "mais pesada" da SPEC T33, como função pura
//! (sem I/O) sobre as linhas do histórico e a árvore de cada versão salva.
//!
//! **Testes comparáveis:** modo normal, que chegaram a carregar (`carregou_ms`), fora a primeira
//! abertura, com o mesmo perfil do teste (assinatura) e no mesmo computador do teste comparável
//! mais recente. Cada teste é atribuído à versão salva cuja árvore é igual; os de alterações não
//! salvas ficam de fora. O tempo por tick não entra: o histórico não o grava.
//!
//! **Mais pesada** (versão mais nova com testes comparáveis contra a anterior com testes
//! comparáveis, medianas): tempo para abrir pelo menos 20% maior e 10 s a mais; ou memória máxima
//! (pico de RAM) pelo menos 20% maior e 512 MB a mais. Pede pelo menos 2 testes em cada versão.

use super::line::PerfLine;
use crate::test_session::TestMode;

/// Testes comparáveis necessários em cada versão.
pub(super) const MIN_TESTS: usize = 2;
/// Tempo para abrir: o aumento mínimo, em ms.
const OPEN_MIN_DELTA_MS: u32 = 10_000;
/// Memória máxima: o aumento mínimo, em MB.
const MEMORY_MIN_DELTA_MB: u32 = 512;

/// Uma versão salva e a árvore do pack dela (mais nova primeiro na lista).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct SavedTree {
    pub(super) version: String,
    pub(super) tree: String,
}

/// Os testes que ficaram de fora, contados por motivo (cada teste conta uma vez só, no primeiro
/// motivo que se aplica, nesta ordem).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct Excluded {
    /// "Testar com perfil de desempenho" (o registro detalhado deixa o carregamento mais lento).
    pub(super) performance_profile: usize,
    /// Outro modo (como o jogador recebe, como servidor).
    pub(super) other_mode: usize,
    /// Não chegaram a carregar.
    pub(super) not_loaded: usize,
    /// Primeira abertura (caches frios).
    pub(super) first_launch: usize,
    /// Outro perfil do teste (outra assinatura).
    pub(super) other_profile: usize,
    /// Outro computador.
    pub(super) other_machine: usize,
    /// Sem versão salva igual (alterações não salvas, ou versão fora das mais recentes).
    pub(super) unsaved: usize,
}

impl Excluded {
    /// Quantos testes ficaram de fora.
    pub(super) fn total(&self) -> usize {
        self.performance_profile
            + self.other_mode
            + self.not_loaded
            + self.first_launch
            + self.other_profile
            + self.other_machine
            + self.unsaved
    }
}

/// Os testes comparáveis de uma versão, resumidos.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Group {
    pub(super) version: String,
    /// Quantos testes comparáveis.
    pub(super) tests: usize,
    /// Mediana do tempo para abrir, em ms.
    pub(super) open_ms: u32,
    /// Mediana do pico de RAM, em MB, e de quantos testes (os que têm a medida).
    pub(super) memory_mb: Option<(u32, usize)>,
}

/// Uma medida nas duas versões.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Metric {
    /// Na versão anterior.
    pub(super) before: u32,
    /// Na versão mais nova.
    pub(super) after: u32,
    /// Cumpre o critério de "mais pesada".
    pub(super) heavy: bool,
}

impl Metric {
    fn new(before: u32, after: u32, min_delta: u32) -> Self {
        let delta = after.saturating_sub(before);
        // 20% ou mais: o aumento vezes 5 alcança o valor antigo (sem número fracionário).
        let heavy = delta >= min_delta && u64::from(delta) * 5 >= u64::from(before);
        Self {
            before,
            after,
            heavy,
        }
    }

    /// O aumento absoluto (zero se diminuiu).
    pub(super) fn delta(&self) -> u32 {
        self.after.saturating_sub(self.before)
    }

    /// A diferença em % (negativa se diminuiu); `None` se o valor antigo é zero.
    pub(super) fn percent(&self) -> Option<f64> {
        (self.before > 0).then(|| {
            (f64::from(self.after) - f64::from(self.before)) * 100.0 / f64::from(self.before)
        })
    }
}

/// O veredito.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Verdict {
    /// Nenhuma versão tem teste comparável.
    NoComparable,
    /// Só uma versão tem testes comparáveis.
    OneVersion,
    /// Alguma das duas tem menos de [`MIN_TESTS`] testes.
    FewTests,
    /// Comparadas: `memory` é `None` se faltam medidas de RAM em alguma das versões.
    Compared {
        open: Metric,
        memory: Option<Metric>,
    },
}

impl Verdict {
    /// Se a versão mais nova ficou mais pesada que a anterior.
    pub(super) fn heavier(&self) -> bool {
        match self {
            Self::Compared { open, memory } => open.heavy || memory.is_some_and(|m| m.heavy),
            _ => false,
        }
    }
}

/// O resultado da comparação.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Comparison {
    /// A versão mais nova com testes comparáveis.
    pub(super) newest: Option<Group>,
    /// A anterior com testes comparáveis.
    pub(super) previous: Option<Group>,
    pub(super) excluded: Excluded,
    pub(super) verdict: Verdict,
    /// O perfil e o computador que definem "comparável" (os do teste comparável mais recente).
    pub(super) profile: Option<Reference>,
}

/// O que os testes comparáveis têm em comum.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Reference {
    pub(super) profile_name: String,
    pub(super) signature: Option<String>,
    pub(super) machine: Option<String>,
}

/// A mediana (a média dos dois do meio quando a quantidade é par, arredondada para baixo).
fn median(mut values: Vec<u32>) -> Option<u32> {
    values.sort_unstable();
    let middle = values.len() / 2;
    if values.is_empty() {
        None
    } else if values.len() % 2 == 1 {
        values.get(middle).copied()
    } else {
        let (low, high) = (values.get(middle - 1)?, values.get(middle)?);
        u32::try_from(u64::midpoint(u64::from(*low), u64::from(*high))).ok()
    }
}

/// A versão salva cuja árvore é a do teste (a mais nova, se duas forem iguais).
fn version_of<'a>(line: &PerfLine, trees: &'a [SavedTree]) -> Option<&'a str> {
    let tree = line.arvore.as_deref()?;
    trees
        .iter()
        .find(|saved| saved.tree == tree)
        .map(|saved| saved.version.as_str())
}

/// Compara as versões. `lines` vai da mais nova para a mais antiga; `trees`, da versão mais nova
/// para a mais antiga.
pub(super) fn compare(lines: &[PerfLine], trees: &[SavedTree]) -> Comparison {
    let mut excluded = Excluded::default();
    // Passa pelos três primeiros filtros (modo, carregou, primeira abertura).
    let mut candidates: Vec<&PerfLine> = Vec::new();
    for line in lines {
        if line.modo == TestMode::Performance {
            excluded.performance_profile += 1;
        } else if line.modo != TestMode::Normal {
            excluded.other_mode += 1;
        } else if line.carregou_ms.is_none() {
            excluded.not_loaded += 1;
        } else if line.primeira_abertura {
            excluded.first_launch += 1;
        } else {
            candidates.push(line);
        }
    }
    let reference = candidates.first().copied();
    let mut by_version: Vec<(&str, Vec<&PerfLine>)> = trees
        .iter()
        .map(|saved| (saved.version.as_str(), Vec::new()))
        .collect();
    if let Some(reference) = reference {
        for line in &candidates {
            if line.perfil.assinatura != reference.perfil.assinatura {
                excluded.other_profile += 1;
            } else if line.computador != reference.computador {
                excluded.other_machine += 1;
            } else if let Some(version) = version_of(line, trees) {
                if let Some((_, group)) = by_version.iter_mut().find(|(name, _)| *name == version) {
                    group.push(line);
                }
            } else {
                excluded.unsaved += 1;
            }
        }
    }
    let mut groups = by_version
        .into_iter()
        .filter(|(_, tests)| !tests.is_empty())
        .filter_map(|(version, tests)| {
            let open_ms = median(tests.iter().filter_map(|t| t.carregou_ms).collect())?;
            let memory: Vec<u32> = tests.iter().filter_map(|t| t.ram_pico_mb).collect();
            let memory_mb = median(memory.clone()).map(|value| (value, memory.len()));
            Some(Group {
                version: version.to_owned(),
                tests: tests.len(),
                open_ms,
                memory_mb,
            })
        });
    let newest = groups.next();
    let previous = groups.next();
    let verdict = match (&newest, &previous) {
        (None, _) => Verdict::NoComparable,
        (Some(_), None) => Verdict::OneVersion,
        (Some(new), Some(old)) if new.tests < MIN_TESTS || old.tests < MIN_TESTS => {
            Verdict::FewTests
        }
        (Some(new), Some(old)) => Verdict::Compared {
            open: Metric::new(old.open_ms, new.open_ms, OPEN_MIN_DELTA_MS),
            memory: match (new.memory_mb, old.memory_mb) {
                (Some((after, n_new)), Some((before, n_old)))
                    if n_new >= MIN_TESTS && n_old >= MIN_TESTS =>
                {
                    Some(Metric::new(before, after, MEMORY_MIN_DELTA_MB))
                }
                _ => None,
            },
        },
    };
    Comparison {
        newest,
        previous,
        excluded,
        verdict,
        profile: reference.map(|line| Reference {
            profile_name: line
                .perfil
                .nome
                .clone()
                .filter(|name| !name.trim().is_empty())
                .unwrap_or_else(|| line.perfil.id.clone()),
            signature: line.perfil.assinatura.clone(),
            machine: line.computador.clone(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use warden_launcher::outcome::Outcome;

    use super::*;
    use crate::ai_tools::perf::line::LineProfile;

    /// O hash da árvore da versão `v` nos testes.
    fn tree(version: &str) -> String {
        format!("sha256:arvore-{version}")
    }

    fn saved(versions: &[&str]) -> Vec<SavedTree> {
        versions
            .iter()
            .map(|version| SavedTree {
                version: (*version).to_owned(),
                tree: tree(version),
            })
            .collect()
    }

    /// Um teste normal, comparável, da versão `version`: abriu em `open_s` s, com `ram` MB.
    fn test(version: &str, open_s: u32, ram: Option<u32>) -> PerfLine {
        PerfLine {
            sessao: format!("s-{version}-{open_s}"),
            data: "2026-10-03T21:10:00Z".into(),
            modo: TestMode::Normal,
            carregou_ms: Some(open_s * 1000),
            mundo_ms: None,
            ram_pico_mb: ram,
            heap_pico_mb: None,
            perfil: LineProfile {
                id: "default".into(),
                nome: None,
                assinatura: Some("sha256:perfil".into()),
            },
            computador: Some("sha256:pc".into()),
            arvore: Some(tree(version)),
            primeira_abertura: false,
            resultado: Outcome::ClosedNormally,
        }
    }

    /// As linhas do mais novo ao mais antigo: a ordem em que o chamador entrega.
    fn run(lines: &[PerfLine], versions: &[&str]) -> Comparison {
        compare(lines, &saved(versions))
    }

    fn heavier(comparison: &Comparison) -> bool {
        comparison.verdict.heavier()
    }

    /// CA-T33-03, tabela de casos.
    #[test]
    #[allow(clippy::too_many_lines)] // a tabela de casos, uma entrada por caso
    fn ca_t33_03_tabela_de_casos() {
        struct Case {
            name: &'static str,
            old: &'static [(u32, Option<u32>)],
            new: &'static [(u32, Option<u32>)],
            heavier: bool,
            few: bool,
        }
        let cases = [
            Case {
                name: "100, 104 e 98 s contra 125 e 128 s: mais lenta",
                old: &[(100, None), (104, None), (98, None)],
                new: &[(125, None), (128, None)],
                heavier: true,
                few: false,
            },
            Case {
                name: "um teste só na 1.5.0: poucos testes",
                old: &[(100, None), (104, None), (98, None)],
                new: &[(125, None)],
                heavier: false,
                few: true,
            },
            Case {
                name: "um teste só na 1.4.2: poucos testes",
                old: &[(100, None)],
                new: &[(125, None), (128, None)],
                heavier: false,
                few: true,
            },
            Case {
                name: "110 e 112 s (+8%... +11%): sem aviso",
                old: &[(100, None), (104, None), (98, None)],
                new: &[(110, None), (112, None)],
                heavier: false,
                few: false,
            },
            Case {
                name: "memória de 5,0 para 5,4 GB (+8%, +400 MB): sem aviso",
                old: &[(100, Some(5000)), (100, Some(5000))],
                new: &[(100, Some(5400)), (100, Some(5400))],
                heavier: false,
                few: false,
            },
            Case {
                name: "memória de 5,0 para 6,0 GB (+20%, +1.000 MB): aviso",
                old: &[(100, Some(5000)), (100, Some(5000))],
                new: &[(100, Some(6000)), (100, Some(6000))],
                heavier: true,
                few: false,
            },
            Case {
                name: "memória +30% mas só +300 MB: sem aviso",
                old: &[(100, Some(1000)), (100, Some(1000))],
                new: &[(100, Some(1300)), (100, Some(1300))],
                heavier: false,
                few: false,
            },
            Case {
                name: "memória com uma medida só numa versão não conta",
                old: &[(100, Some(5000)), (100, None)],
                new: &[(100, Some(9000)), (100, Some(9000))],
                heavier: false,
                few: false,
            },
            Case {
                name: "tempo +50% mas só +5 s: sem aviso",
                old: &[(10, None), (10, None)],
                new: &[(15, None), (15, None)],
                heavier: false,
                few: false,
            },
            Case {
                name: "tempo +10 s mas só +10%: sem aviso",
                old: &[(100, None), (100, None)],
                new: &[(110, None), (110, None)],
                heavier: false,
                few: false,
            },
            Case {
                name: "limite exato: +20% e +10 s avisa",
                old: &[(50, None), (50, None)],
                new: &[(60, None), (60, None)],
                heavier: true,
                few: false,
            },
            Case {
                name: "ficou mais rápida: sem aviso",
                old: &[(125, None), (128, None)],
                new: &[(100, None), (104, None)],
                heavier: false,
                few: false,
            },
        ];
        for case in cases {
            // O mais novo primeiro.
            let mut lines: Vec<PerfLine> = case
                .new
                .iter()
                .map(|(s, ram)| test("1.5.0", *s, *ram))
                .collect();
            lines.extend(case.old.iter().map(|(s, ram)| test("1.4.2", *s, *ram)));
            let comparison = run(&lines, &["1.5.0", "1.4.2"]);
            assert_eq!(heavier(&comparison), case.heavier, "{}", case.name);
            assert_eq!(
                comparison.verdict == Verdict::FewTests,
                case.few,
                "{}",
                case.name
            );
        }
    }

    #[test]
    fn medianas_e_numeros_do_aviso() {
        let mut lines = vec![test("1.5.0", 125, None), test("1.5.0", 128, None)];
        lines.extend([
            test("1.4.2", 100, None),
            test("1.4.2", 104, None),
            test("1.4.2", 98, None),
        ]);
        let comparison = run(&lines, &["1.5.0", "1.4.2"]);
        let newest = comparison.newest.clone().unwrap();
        let previous = comparison.previous.clone().unwrap();
        assert_eq!((newest.version.as_str(), newest.tests), ("1.5.0", 2));
        assert_eq!((previous.version.as_str(), previous.tests), ("1.4.2", 3));
        // A mediana de um par é a média dos dois do meio.
        assert_eq!(newest.open_ms, 126_500);
        assert_eq!(previous.open_ms, 100_000);
        let Verdict::Compared { open, memory } = comparison.verdict else {
            panic!("{:?}", comparison.verdict);
        };
        assert!(open.heavy && memory.is_none());
        assert_eq!(open.delta(), 26_500);
        assert!((open.percent().unwrap() - 26.5).abs() < 1e-9);
    }

    #[test]
    fn so_uma_versao_ou_nenhuma_nao_compara() {
        let one = run(
            &[test("1.5.0", 100, None), test("1.5.0", 101, None)],
            &["1.5.0", "1.4.2"],
        );
        assert_eq!(one.verdict, Verdict::OneVersion);
        assert!(one.previous.is_none() && one.newest.is_some());
        let none = run(&[], &["1.5.0"]);
        assert_eq!(none.verdict, Verdict::NoComparable);
        assert!(none.profile.is_none());
        assert_eq!(none.excluded.total(), 0);
        // Versão sem teste no meio: a comparação pula para a anterior com testes.
        let skipping = run(
            &[
                test("1.5.0", 130, None),
                test("1.5.0", 131, None),
                test("1.3.0", 100, None),
                test("1.3.0", 102, None),
            ],
            &["1.5.0", "1.4.2", "1.3.0"],
        );
        assert_eq!(skipping.previous.unwrap().version, "1.3.0");
        assert!(skipping.verdict.heavier());
    }

    #[test]
    fn testes_de_fora_contados_por_motivo() {
        let mut lines = Vec::new();
        // O comparável mais novo define o perfil e o computador.
        lines.push(test("1.5.0", 125, None));
        lines.push(test("1.5.0", 128, None));
        let mut perf = test("1.5.0", 300, None);
        perf.modo = TestMode::Performance;
        lines.push(perf);
        let mut player = test("1.5.0", 111, None);
        player.modo = TestMode::AsPlayer;
        lines.push(player);
        let mut crashed = test("1.5.0", 1, None);
        crashed.carregou_ms = None;
        crashed.resultado = Outcome::Crashed;
        lines.push(crashed);
        let mut first = test("1.5.0", 200, None);
        first.primeira_abertura = true;
        lines.push(first);
        let mut other_profile = test("1.5.0", 90, None);
        other_profile.perfil.assinatura = Some("sha256:outro".into());
        lines.push(other_profile);
        let mut other_machine = test("1.5.0", 90, None);
        other_machine.computador = Some("sha256:outro-pc".into());
        lines.push(other_machine);
        let mut unsaved = test("1.5.0", 90, None);
        unsaved.arvore = Some("sha256:sem-versao".into());
        lines.push(unsaved);
        lines.push(test("1.4.2", 100, None));
        lines.push(test("1.4.2", 104, None));
        let comparison = run(&lines, &["1.5.0", "1.4.2"]);
        assert_eq!(
            comparison.excluded,
            Excluded {
                performance_profile: 1,
                other_mode: 1,
                not_loaded: 1,
                first_launch: 1,
                other_profile: 1,
                other_machine: 1,
                unsaved: 1,
            }
        );
        assert_eq!(comparison.excluded.total(), 7);
        assert_eq!(comparison.newest.unwrap().tests, 2);
        assert_eq!(comparison.previous.unwrap().tests, 2);
        let reference = comparison.profile.unwrap();
        assert_eq!(reference.signature.as_deref(), Some("sha256:perfil"));
        assert_eq!(reference.machine.as_deref(), Some("sha256:pc"));
        assert_eq!(reference.profile_name, "default");
    }

    #[test]
    fn mediana_de_tamanhos_diferentes() {
        assert_eq!(median(Vec::new()), None);
        assert_eq!(median(vec![7]), Some(7));
        assert_eq!(median(vec![9, 1, 5]), Some(5));
        assert_eq!(median(vec![4, 1, 3, 2]), Some(2));
    }
}
