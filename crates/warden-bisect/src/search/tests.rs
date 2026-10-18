//! Testes do algoritmo com um executor simulado (ROADMAP D-12, verificação): um "jogo" que
//! falha quando certos conjuntos de mods estão ligados juntos.

use std::collections::BTreeSet;

use proptest::prelude::*;

use super::*;
use crate::estimate::ceil_log2;
use crate::universe::{ItemSpec, Requirement, Universe};

/// O jogo simulado: falha igual quando algum conjunto de `culprits` está todo ligado.
struct FakeGame {
    culprits: Vec<Vec<String>>,
}

impl FakeGame {
    fn run(&self, enabled: &[String]) -> Verdict {
        let on: BTreeSet<&str> = enabled.iter().map(String::as_str).collect();
        if self
            .culprits
            .iter()
            .any(|set| set.iter().all(|path| on.contains(path.as_str())))
        {
            Verdict::FailedSame
        } else {
            Verdict::Passed
        }
    }
}

/// Confere que nenhum item ligado está sem as dependências.
fn assert_closed(universe: &Universe, enabled: &[String]) {
    let on: BTreeSet<usize> = enabled
        .iter()
        .filter_map(|path| universe.index_of(path))
        .collect();
    for &item in &on {
        for group in universe.groups_of(item) {
            assert!(
                group.iter().any(|provider| on.contains(provider)),
                "{} ligado sem a dependência {:?}",
                universe.items()[item].path,
                group
            );
        }
    }
}

/// Roda a busca até o fim com o executor dado; devolve a busca e as rodadas.
fn run_with(mut search: Search, mut game: impl FnMut(&RoundPlan) -> Verdict) -> Search {
    for _ in 0..500 {
        let Some(plan) = search.next_round() else {
            return search;
        };
        assert_closed(search.universe(), &plan.enabled);
        let verdict = game(&plan);
        search.record(verdict);
    }
    panic!("a busca não terminou em 500 rodadas");
}

fn run(universe: Universe, game: &FakeGame) -> Search {
    run_with(Search::new(universe), |plan| game.run(&plan.enabled))
}

fn mod_path(index: usize) -> String {
    format!("mods/m{index:03}.pw.toml")
}

fn lib_path(index: usize) -> String {
    format!("mods/lib{index}.pw.toml")
}

/// Pack sintético de 118 itens: 10 bibliotecas e 108 mods, com cadeias de dependência (cada
/// mod múltiplo de 3 exige o anterior; todos exigem uma biblioteca; algumas bibliotecas
/// exigem outra).
fn synthetic_118() -> Universe {
    let mut items = Vec::new();
    let mut requirements = Vec::new();
    for lib in 0..10 {
        items.push(ItemSpec {
            path: lib_path(lib),
            name: format!("Lib {lib}"),
            library: true,
            player_tool: false,
        });
        if lib % 2 == 1 {
            requirements.push(Requirement {
                from: lib_path(lib),
                any_of: vec![lib_path(lib - 1)],
            });
        }
    }
    for index in 0..108 {
        items.push(ItemSpec {
            path: mod_path(index),
            name: format!("Mod {index:03}"),
            library: false,
            player_tool: false,
        });
        requirements.push(Requirement {
            from: mod_path(index),
            any_of: vec![lib_path(index % 10)],
        });
        if index % 3 == 0 && index > 0 {
            requirements.push(Requirement {
                from: mod_path(index),
                any_of: vec![mod_path(index - 1)],
            });
        }
    }
    Universe::new(items, &requirements, &[])
}

fn position(universe: &Universe, path: &str) -> usize {
    let item = universe.index_of(path).unwrap();
    let unit = universe.unit_of(item).unwrap();
    universe
        .order()
        .iter()
        .position(|&known| known == unit)
        .unwrap()
        + 1
}

/// CA-T25-01: par X + Y num pack de 118 itens com cadeias de dependência; a busca acha {X, Y}
/// em no máximo ⌈log₂ n⌉ + ⌈log₂ p⌉ + 5 rodadas (p = posição do segundo mod na ordem), nunca
/// liga um mod sem as dependências e confirma com os 2 testes de controle.
#[test]
fn ca_t25_01_par_em_pack_de_118_mods() {
    let universe = synthetic_118();
    let n = u32::try_from(universe.searched_units()).unwrap();
    assert_eq!(n, 118);
    let mut checked = 0;
    for x in (0..108).step_by(7) {
        for y in (0..108).step_by(11) {
            let (x_path, y_path) = (mod_path(x), mod_path(y));
            // X e Y independentes (nenhum exige o outro).
            let x_closure = universe.closure(&[universe
                .unit_of(universe.index_of(&x_path).unwrap())
                .unwrap()]);
            let y_closure = universe.closure(&[universe
                .unit_of(universe.index_of(&y_path).unwrap())
                .unwrap()]);
            let x_item = universe.index_of(&x_path).unwrap();
            let y_item = universe.index_of(&y_path).unwrap();
            if x == y || x_closure.contains(&y_item) || y_closure.contains(&x_item) {
                continue;
            }
            let game = FakeGame {
                culprits: vec![vec![x_path.clone(), y_path.clone()]],
            };
            let search = run(universe.clone(), &game);
            let Some(SearchOutcome::Found {
                culprits,
                confidence,
                another_problem,
            }) = search.outcome()
            else {
                panic!("sem resultado para {x} e {y}: {:?}", search.outcome());
            };
            let found: BTreeSet<&String> = culprits.iter().collect();
            assert_eq!(found, BTreeSet::from([&x_path, &y_path]), "par {x}, {y}");
            assert_eq!(*confidence, CulpritConfidence::High);
            assert!(!another_problem);
            let p = u32::try_from(position(&universe, &x_path).max(position(&universe, &y_path)))
                .unwrap();
            let rounds = u32::try_from(search.rounds().len()).unwrap();
            assert!(
                rounds <= ceil_log2(n) + ceil_log2(p) + 5,
                "par {x}, {y}: {rounds} rodadas (p = {p})"
            );
            assert!(rounds <= 19);
            // Os 2 testes de controle no fim: só o conjunto (falha) e o pack sem ele (passa).
            let last: Vec<&RoundRecord> = search.rounds().iter().rev().take(2).collect();
            assert_eq!(last[1].kind, RoundKind::Alone);
            assert_eq!(last[1].verdict, Verdict::FailedSame);
            assert_eq!(last[0].kind, RoundKind::Without);
            assert_eq!(last[0].verdict, Verdict::Passed);
            checked += 1;
        }
    }
    assert!(checked > 100, "{checked}");
}

#[test]
fn um_culpado_em_log2_n_mais_4_rodadas() {
    let universe = synthetic_118();
    for x in [0, 1, 50, 99, 107] {
        let game = FakeGame {
            culprits: vec![vec![mod_path(x)]],
        };
        let search = run(universe.clone(), &game);
        assert_eq!(
            search.outcome(),
            Some(&SearchOutcome::Found {
                culprits: vec![mod_path(x)],
                confidence: CulpritConfidence::High,
                another_problem: false,
            })
        );
        assert!(
            search.rounds().len() <= 7 + 4,
            "{x}: {}",
            search.rounds().len()
        );
    }
}

#[test]
fn nao_reproduziu_em_3_testes() {
    let search = run(synthetic_118(), &FakeGame { culprits: vec![] });
    assert_eq!(search.outcome(), Some(&SearchOutcome::NotReproduced));
    assert_eq!(search.rounds().len(), 3);
    assert!(
        search
            .rounds()
            .iter()
            .all(|round| matches!(round.kind, RoundKind::Reproduce { .. }))
    );
}

#[test]
fn sem_mods_ja_falha() {
    let search = run_with(Search::new(synthetic_118()), |_| Verdict::FailedSame);
    assert_eq!(search.outcome(), Some(&SearchOutcome::NotAMod));
    assert_eq!(search.rounds()[1].kind, RoundKind::Control);
    assert_eq!(search.rounds()[1].enabled_count, 0);
}

#[test]
fn java_errado_para_a_busca() {
    let search = run_with(Search::new(synthetic_118()), |_| Verdict::Environment);
    assert_eq!(search.outcome(), Some(&SearchOutcome::Environment));
    assert_eq!(search.rounds().len(), 1);
}

/// CA-T25-02 (algoritmo): "travou diferente" não conta (a rodada se repete); três seguidas
/// param a busca com os suspeitos.
#[test]
fn ca_t25_02_travou_diferente_repete_e_nao_conta() {
    let game = FakeGame {
        culprits: vec![vec![mod_path(40)]],
    };
    let mut flaky_once = true;
    let search = run_with(Search::new(synthetic_118()), |plan| {
        if matches!(plan.kind, RoundKind::Prefix { .. }) && flaky_once {
            flaky_once = false;
            return Verdict::FailedOther;
        }
        game.run(&plan.enabled)
    });
    assert!(
        matches!(search.outcome(), Some(SearchOutcome::Found { culprits, .. }) if culprits == &vec![mod_path(40)])
    );
    let other = search
        .rounds()
        .iter()
        .position(|round| round.verdict == Verdict::FailedOther)
        .unwrap();
    // A mesma rodada foi refeita.
    assert_eq!(search.rounds()[other].kind, search.rounds()[other + 1].kind);

    let search = run_with(Search::new(synthetic_118()), |plan| {
        if matches!(plan.kind, RoundKind::Prefix { .. }) {
            Verdict::FailedOther
        } else {
            game.run(&plan.enabled)
        }
    });
    let Some(SearchOutcome::Inconclusive { suspects }) = search.outcome() else {
        panic!("{:?}", search.outcome());
    };
    // Nenhuma rodada da busca binária contou: todos ainda são suspeitos.
    assert_eq!(suspects.len(), 118);
}

/// CA-T25-02 (algoritmo): dependência inferida reordena e refaz a rodada sem repetir as
/// rodadas 0 e 1.
#[test]
fn ca_t25_02_dependencia_inferida_refaz_a_rodada() {
    // m060 usa m095 sem declarar; o culpado é m070.
    let hidden = (mod_path(60), mod_path(95));
    let game = FakeGame {
        culprits: vec![vec![mod_path(70)]],
    };
    let search = run_with(Search::new(synthetic_118()), |plan| {
        let on: BTreeSet<&String> = plan.enabled.iter().collect();
        if on.contains(&hidden.0) && !on.contains(&hidden.1) {
            return Verdict::Inferred {
                from: hidden.0.clone(),
                to: hidden.1.clone(),
            };
        }
        game.run(&plan.enabled)
    });
    assert!(
        matches!(search.outcome(), Some(SearchOutcome::Found { culprits, .. }) if culprits == &vec![mod_path(70)])
    );
    assert_eq!(search.inferred(), std::slice::from_ref(&hidden));
    let controls = search
        .rounds()
        .iter()
        .filter(|round| matches!(round.kind, RoundKind::Control | RoundKind::Reproduce { .. }))
        .count();
    assert_eq!(controls, 2);
    // Depois da inferência, m095 vem antes de m060 na ordem.
    let universe = search.universe();
    assert!(position(universe, &hidden.1) < position(universe, &hidden.0));
}

#[test]
fn problema_intermitente_repete_os_passou() {
    // Falha em uma de duas tentativas quando o culpado está ligado.
    let culprit = mod_path(30);
    let mut tick = 0u32;
    let search = run_with(Search::new(synthetic_118()), |plan| {
        tick += 1;
        if plan.enabled.contains(&culprit) && tick.is_multiple_of(2) {
            Verdict::FailedSame
        } else {
            Verdict::Passed
        }
    });
    assert_eq!(search.rate(), Some(0.5));
    assert_eq!(search.repetitions(), 5);
    let Some(SearchOutcome::Found {
        culprits,
        confidence,
        ..
    }) = search.outcome()
    else {
        panic!("{:?}", search.outcome());
    };
    assert_eq!(culprits, &vec![culprit]);
    assert_eq!(*confidence, CulpritConfidence::Medium);
    assert_eq!(repetitions_for(1.0 / 3.0), 8);
    assert_eq!(repetitions_for(1.0), 1);
    assert_eq!(repetitions_for(0.0), u32::MAX);
}

#[test]
fn outro_problema_e_continuar_sem_ele() {
    let game = FakeGame {
        culprits: vec![vec![mod_path(20)], vec![mod_path(80)]],
    };
    let search = run(synthetic_118(), &game);
    let Some(SearchOutcome::Found {
        culprits,
        another_problem: true,
        ..
    }) = search.outcome().cloned()
    else {
        panic!("{:?}", search.outcome());
    };
    let mut search = search;
    assert!(search.continue_without());
    assert!(!search.continue_without());
    let search = run_with(search, |plan| game.run(&plan.enabled));
    let Some(SearchOutcome::Found {
        culprits: second,
        another_problem: false,
        ..
    }) = search.outcome()
    else {
        panic!("{:?}", search.outcome());
    };
    let both: BTreeSet<String> = culprits.into_iter().chain(second.clone()).collect();
    assert_eq!(both, BTreeSet::from([mod_path(20), mod_path(80)]));
    assert_eq!(search.earlier().len(), 1);
}

#[test]
fn mod_que_evita_o_problema_e_contradicao() {
    // m040 trava o jogo, mas m010 (antes na ordem) evita o problema; a rodada 0 falhou por
    // acaso. Nenhum conjunto mínimo aparece: a busca para em vez de culpar 4 ou mais mods.
    let (bad, guard) = (mod_path(40), mod_path(10));
    let search = run_with(Search::new(synthetic_118()), |plan| {
        let on: BTreeSet<&String> = plan.enabled.iter().collect();
        if plan.kind == RoundKind::Control {
            return Verdict::Passed;
        }
        if matches!(plan.kind, RoundKind::Reproduce { .. }) {
            return Verdict::FailedSame;
        }
        if on.contains(&bad) && !on.contains(&guard) {
            Verdict::FailedSame
        } else {
            Verdict::Passed
        }
    });
    assert!(
        matches!(search.outcome(), Some(SearchOutcome::Contradiction { .. })),
        "{:?}",
        search.outcome()
    );
}

#[test]
fn cancelar_guarda_os_suspeitos_e_estado_volta_igual() {
    let game = FakeGame {
        culprits: vec![vec![mod_path(64)]],
    };
    let mut search = Search::new(synthetic_118());
    for _ in 0..4 {
        let plan = search.next_round().unwrap();
        search.record(game.run(&plan.enabled));
    }
    // Pausar = gravar; continuar = ler e seguir.
    let saved = serde_json::to_string(&search).unwrap();
    let restored: Search = serde_json::from_str(&saved).unwrap();
    assert_eq!(restored, search);
    let suspects = search.suspects();
    assert!(suspects.len() < 118 && suspects.contains(&mod_path(64)));
    let (min, max) = search.remaining_rounds();
    assert!(min >= 1 && max >= min);
    search.cancel();
    assert_eq!(
        search.outcome(),
        Some(&SearchOutcome::Cancelled { suspects })
    );
    assert!(search.next_round().is_none());
    let finished = run(synthetic_118(), &game);
    assert_eq!(finished.remaining_rounds(), (0, 0));
}

#[test]
fn modo_assistido_conta_as_respostas() {
    let culprit = mod_path(5);
    let search = run_with(Search::new(synthetic_118()), |plan| {
        if plan.enabled.contains(&culprit) {
            Verdict::UserYes
        } else {
            Verdict::UserNo
        }
    });
    assert!(
        matches!(search.outcome(), Some(SearchOutcome::Found { culprits, .. }) if culprits == &vec![culprit.clone()])
    );
    let skipped = run_with(Search::new(synthetic_118()), |_| Verdict::UserSkip);
    assert!(matches!(
        skipped.outcome(),
        Some(SearchOutcome::Inconclusive { .. })
    ));
}

#[test]
fn plano_diz_o_que_foi_acrescentado() {
    let mut search = Search::new(synthetic_118());
    search.record(Verdict::FailedSame);
    search.record(Verdict::Passed);
    let plan = search.next_round().unwrap();
    assert_eq!(
        plan.kind,
        RoundKind::Prefix {
            size: 59,
            of: 118,
            with_found: false
        }
    );
    assert_eq!(plan.added.len(), 59);
    assert_eq!(plan.suspects, 118);
    assert_eq!(plan.number, 2);
}

/// Um grafo aleatório sem ciclos: cada item pode exigir itens anteriores, às vezes com dois
/// fornecedores.
fn arbitrary_universe() -> impl Strategy<Value = (Universe, usize, usize)> {
    (2usize..40)
        .prop_flat_map(|len| {
            let edges =
                proptest::collection::vec((0..len, 0..len, 0..len, any::<bool>()), 0..len * 2);
            let libs = proptest::collection::vec(any::<bool>(), len);
            (Just(len), edges, libs, 0..len, 0..len)
        })
        .prop_map(|(len, edges, libs, culprit, partner)| {
            let items: Vec<ItemSpec> = (0..len)
                .map(|index| ItemSpec {
                    path: mod_path(index),
                    name: format!("Item {index:03}"),
                    library: libs[index],
                    player_tool: false,
                })
                .collect();
            let requirements: Vec<Requirement> = edges
                .into_iter()
                .filter(|(from, to, _, _)| to < from)
                .map(|(from, to, other, two)| {
                    let mut any_of = vec![mod_path(to)];
                    if two && other < from && other != to {
                        any_of.push(mod_path(other));
                    }
                    Requirement {
                        from: mod_path(from),
                        any_of,
                    }
                })
                .collect();
            (Universe::new(items, &requirements, &[]), culprit, partner)
        })
}

proptest! {
    /// Nenhuma rodada liga um item sem as dependências, todo prefixo da ordem é fechado, e um
    /// culpado sozinho é sempre achado com confiança alta.
    #[test]
    fn nenhum_prefixo_sem_as_dependencias((universe, culprit, _) in arbitrary_universe()) {
        // Todo prefixo da ordem, com os sempre ligados, já tem as dependências.
        let order = universe.order().to_vec();
        let mut on: BTreeSet<usize> = universe.with_role(Role::AlwaysOn).into_iter().collect();
        for &unit in &order {
            on.extend(universe.unit_items(unit));
            for &item in &on {
                for group in universe.groups_of(item) {
                    prop_assert!(group.iter().any(|provider| on.contains(provider)));
                }
            }
        }
        let path = mod_path(culprit);
        let item = universe.index_of(&path).unwrap();
        prop_assume!(universe.role(item) == Role::Searched);
        let game = FakeGame { culprits: vec![vec![path]] };
        let search = run(universe.clone(), &game);
        let expected = universe.unit_paths(&[universe.unit_of(item).unwrap()]);
        prop_assert_eq!(
            search.outcome(),
            Some(&SearchOutcome::Found { culprits: expected, confidence: CulpritConfidence::High, another_problem: false })
        );
    }

    /// Com dois culpados, a busca termina, nunca liga item sem dependência, e o conjunto
    /// achado reproduz o problema sozinho e o pack sem ele passa.
    #[test]
    fn par_aleatorio_termina_com_conjunto_que_reproduz((universe, culprit, partner) in arbitrary_universe()) {
        prop_assume!(culprit != partner);
        let paths = vec![mod_path(culprit), mod_path(partner)];
        prop_assume!(paths.iter().all(|path| universe.role(universe.index_of(path).unwrap()) == Role::Searched));
        let game = FakeGame { culprits: vec![paths] };
        let search = run(universe, &game);
        let outcome = search.outcome().cloned();
        let Some(SearchOutcome::Found { another_problem, .. }) = outcome else {
            return Err(TestCaseError::fail(format!("{outcome:?}")));
        };
        prop_assert!(!another_problem);
        prop_assert!(search.rounds().len() <= 2 + 2 * 6 + 3 + 6);
    }
}
