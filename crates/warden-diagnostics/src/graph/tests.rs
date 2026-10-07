//! Testes de domínio do grafo (CA-T06-06, CA-T07-03): packs de teste com cadeias, `provides`,
//! jar-in-jar, ciclos, alternativas e arestas inferidas.

use proptest::prelude::*;
use serde_json::json;
use warden_jarmeta::{JarMetadata, Loader};

use super::*;

/// Um mod no formato do NeoForge, com as dependências `(id, tipo)` e os `provides`.
fn module(id: &str, deps: &[(&str, &str)], provides: &[&str]) -> serde_json::Value {
    json!({
        "source": "neoForgeModsToml",
        "loader": "neoForge",
        "id": id,
        "version": "1.0",
        "provides": provides,
        "dependencies": deps.iter().map(|(dep, kind)| json!({
            "id": dep, "kind": kind, "range": {"dialect": "any"}
        })).collect::<Vec<_>>(),
    })
}

fn jar_json(module: &serde_json::Value, nested: &[serde_json::Value]) -> serde_json::Value {
    json!({
        "descriptors": ["neoForgeModsToml"],
        "mods": [module],
        "nested": nested.iter().enumerate().map(|(n, jar)| json!({
            "path": format!("META-INF/jarjar/n{n}.jar"),
            "declaredBy": "jarJarMetadata",
            "metadata": jar,
        })).collect::<Vec<_>>(),
    })
}

fn jar(id: &str, deps: &[(&str, &str)]) -> JarMetadata {
    serde_json::from_value(jar_json(&module(id, deps, &[]), &[])).unwrap()
}

fn item(id: &str, deps: &[(&str, &str)]) -> GraphItem {
    GraphItem {
        path: format!("mods/{id}.pw.toml"),
        name: id.to_owned(),
        jar: Some(jar(id, deps)),
        api_required: Vec::new(),
    }
}

fn input(items: Vec<GraphItem>) -> GraphInput {
    GraphInput {
        loader: Loader::NeoForge,
        items,
        additions: None,
        inferred: Vec::new(),
    }
}

fn path(id: &str) -> String {
    format!("mods/{id}.pw.toml")
}

fn names(nodes: &[NodeRef]) -> Vec<&str> {
    nodes.iter().map(|node| node.name.as_str()).collect()
}

fn affected_names(report: &DependentsReport) -> Vec<(&str, u32)> {
    report
        .affected
        .iter()
        .map(|a| (a.item.name.as_str(), a.depth))
        .collect()
}

fn history(entries: &[(&str, Option<&str>)]) -> AdditionHistory {
    AdditionHistory {
        entries: entries
            .iter()
            .map(|(id, at)| {
                (
                    path(id),
                    Addition {
                        at: at.map(str::to_owned),
                    },
                )
            })
            .collect(),
    }
}

#[test]
fn ca_t06_06_remover_c_lista_b_e_a_na_cadeia() {
    // A depende de B, que depende de C; D não tem nada a ver.
    let graph = Graph::build(&input(vec![
        item("a", &[("b", "required")]),
        item("b", &[("c", "required"), ("minecraft", "required")]),
        item("c", &[("neoforge", "required")]),
        item("d", &[]),
    ]));
    let report = graph.dependents(&[path("c")]).unwrap();
    assert_eq!(affected_names(&report), vec![("b", 1), ("a", 2)]);
    assert_eq!(names(&report.affected[1].needs), vec!["b"]);
    assert_eq!(
        names(
            &report
                .used_by
                .iter()
                .map(|d| d.item.clone())
                .collect::<Vec<_>>()
        ),
        vec!["b"]
    );
    // Remover só B quebra A; remover A não quebra ninguém.
    let report = graph.dependents(&[path("b")]).unwrap();
    assert_eq!(affected_names(&report), vec![("a", 1)]);
    assert!(graph.dependents(&[path("a")]).unwrap().affected.is_empty());
}

/// Item cujo jar se declara biblioteca (`FMLModType: LIBRARY`).
fn library(id: &str, deps: &[(&str, &str)]) -> GraphItem {
    let mut library = item(id, deps);
    library.jar = serde_json::from_value(json!({
        "descriptors": ["neoForgeModsToml"],
        "mods": [module(id, deps, &[])],
        "manifest": {"fmlModType": "LIBRARY"},
    }))
    .ok();
    library
}

fn orphan_names(graph: &Graph) -> Vec<String> {
    graph.orphans().into_iter().map(|o| o.item.name).collect()
}

#[test]
fn ca_t06_06_biblioteca_que_nenhum_mod_usa_mais_aparece_como_sem_uso() {
    // `flywheel` é biblioteca por descritor; `geckolib`, pela lista de conhecidas.
    let with_create = Graph::build(&input(vec![
        item("create", &[("flywheel", "required")]),
        library("flywheel", &[]),
        item("geckolib", &[]),
        item("sodium", &[]),
    ]));
    // Sem histórico: o `geckolib` que ninguém exige é sobra; o `flywheel` é usado; os mods
    // soltos (create, sodium) foram escolha do usuário.
    assert_eq!(orphan_names(&with_create), vec!["geckolib"]);

    // Sem o `create` (removido), o `flywheel` fica sem uso.
    let without_create = Graph::build(&input(vec![
        library("flywheel", &[]),
        item("geckolib", &[]),
        item("sodium", &[]),
    ]));
    assert_eq!(orphan_names(&without_create), vec!["flywheel", "geckolib"]);
}

#[test]
fn provides_satisfaz_a_dependencia_do_outro_id() {
    // `fabric-api` fornece `fabric`: quem pede `fabric` depende dele.
    let provider: JarMetadata =
        serde_json::from_value(jar_json(&module("fabric-api", &[], &["fabric"]), &[])).unwrap();
    let mut api = item("fabric-api", &[]);
    api.jar = Some(provider);
    let graph = Graph::build(&input(vec![item("sodium", &[("fabric", "required")]), api]));
    let report = graph.dependents(&[path("fabric-api")]).unwrap();
    assert_eq!(affected_names(&report), vec![("sodium", 1)]);
    assert_eq!(report.used_by[0].id, "fabric");
    let sodium = graph.dependents(&[path("sodium")]).unwrap();
    assert_eq!(sodium.depends_on.len(), 1);
    assert_eq!(sodium.depends_on[0].state, DependsOnState::InPack);
    assert!(sodium.depends_on[0].providers[0].alias);
    assert_eq!(sodium.depends_on[0].providers[0].mod_id, "fabric-api");
}

#[test]
fn dependencia_com_dois_fornecedores_so_quebra_quando_os_dois_saem() {
    let first: JarMetadata =
        serde_json::from_value(jar_json(&module("forge-config", &[], &["config-api"]), &[]))
            .unwrap();
    let second: JarMetadata =
        serde_json::from_value(jar_json(&module("config-port", &[], &["config-api"]), &[]))
            .unwrap();
    let mut one = item("forge-config", &[]);
    one.jar = Some(first);
    let mut two = item("config-port", &[]);
    two.jar = Some(second);
    let graph = Graph::build(&input(vec![
        item("mod", &[("config-api", "required")]),
        one,
        two,
    ]));
    assert!(
        graph
            .dependents(&[path("forge-config")])
            .unwrap()
            .affected
            .is_empty()
    );
    let both = graph
        .dependents(&[path("forge-config"), path("config-port")])
        .unwrap();
    assert_eq!(affected_names(&both), vec![("mod", 1)]);
    assert_eq!(both.targets.len(), 2);
    assert!(both.depends_on.is_empty());
}

#[test]
fn jar_in_jar_fica_dentro_do_no_do_item_que_o_traz() {
    // `host` embute `lib`; `other` exige `lib`; `host` também exige `lib` (satisfeita dentro).
    let lib = jar_json(&module("lib", &[], &[]), &[]);
    let host: JarMetadata = serde_json::from_value(jar_json(
        &module("host", &[("lib", "required")], &[]),
        &[lib],
    ))
    .unwrap();
    let mut host_item = item("host", &[]);
    host_item.jar = Some(host);
    let graph = Graph::build(&input(vec![
        host_item,
        item("other", &[("lib", "required")]),
    ]));

    // O que `host` exige está dentro dele mesmo.
    let host_report = graph.dependents(&[path("host")]).unwrap();
    assert_eq!(host_report.depends_on[0].state, DependsOnState::Own);
    assert!(host_report.depends_on[0].providers.is_empty());
    // `other` depende do `lib` embutido, então depende do `host`.
    assert_eq!(affected_names(&host_report), vec![("other", 1)]);
    assert!(host_report.used_by[0].embedded);
    let other_report = graph.dependents(&[path("other")]).unwrap();
    let provider = &other_report.depends_on[0].providers[0];
    assert!(provider.embedded);
    assert_eq!(provider.item.name, "host");
    assert_eq!(provider.mod_id, "lib");
    // Remover o `other` não mexe no `host`.
    assert!(other_report.affected.is_empty());
}

#[test]
fn ciclos_viram_um_no_e_nao_prendem_as_consultas() {
    let graph = Graph::build(&input(vec![
        item("a", &[("b", "required")]),
        item("b", &[("a", "required")]),
        item("c", &[("a", "required")]),
    ]));
    let cycles = graph.cycles();
    assert_eq!(cycles.len(), 1);
    assert_eq!(names(&cycles[0]), vec!["a", "b"]);
    // Remover A leva B (que o exige) e C.
    let report = graph.dependents(&[path("a")]).unwrap();
    let mut affected = affected_names(&report);
    affected.sort_unstable();
    assert_eq!(affected, vec![("b", 1), ("c", 1)]);
    // Sem histórico, o ciclo mantido por C... C não é exigido por ninguém: é raiz; A e B são
    // mantidos por ele.
    assert!(graph.orphans().is_empty());
    let why = graph.why_in_pack(&path("c")).unwrap();
    assert_eq!(why.why, Why::NoDependents);
    let why_a = graph.why_in_pack(&path("a")).unwrap();
    assert_eq!(names(&why_a.cycle), vec!["b"]);
    let Why::RequiredBy { chains } = why_a.why else {
        panic!("esperava cadeia: {:?}", why_a.why);
    };
    assert_eq!(
        chains[0]
            .iter()
            .map(|l| l.item.name.as_str())
            .collect::<Vec<_>>(),
        vec!["a", "c"]
    );
}

#[test]
fn ciclo_que_ninguem_de_fora_usa_e_uma_raiz_sem_historico_e_sobra_se_forem_bibliotecas() {
    let graph = Graph::build(&input(vec![
        item("geckolib", &[("curios", "required")]),
        item("curios", &[("geckolib", "required")]),
    ]));
    let orphans: Vec<_> = graph.orphans().into_iter().map(|o| o.item.name).collect();
    assert_eq!(orphans, vec!["curios", "geckolib"]);

    // Dois mods comuns que dependem um do outro foram escolhidos: sem sobra.
    let graph = Graph::build(&input(vec![
        item("x", &[("y", "required")]),
        item("y", &[("x", "required")]),
    ]));
    assert!(graph.orphans().is_empty());
    assert_eq!(
        graph.why_in_pack(&path("x")).unwrap().why,
        Why::NoDependents
    );
}

#[test]
fn ca_t07_03_por_que_esta_no_pack_com_historico() {
    // O usuário adicionou `create`; `create` exige `flywheel`, que exige `lib`.
    let mut data = input(vec![
        item("create", &[("flywheel", "required")]),
        item("flywheel", &[("lib", "required")]),
        item("lib", &[]),
        item("sodium", &[]),
    ]);
    data.additions = Some(history(&[("create", Some("2026-08-12")), ("sodium", None)]));
    let graph = Graph::build(&data);

    let direct = graph.why_in_pack(&path("create")).unwrap();
    assert_eq!(
        direct.why,
        Why::UserAdded {
            at: Some("2026-08-12".into())
        }
    );
    let unknown_date = graph.why_in_pack(&path("sodium")).unwrap();
    assert_eq!(unknown_date.why, Why::UserAdded { at: None });

    let library = graph.why_in_pack(&path("lib")).unwrap();
    let Why::RequiredBy { chains } = &library.why else {
        panic!("esperava cadeia: {:?}", library.why);
    };
    assert_eq!(chains.len(), 1);
    let chain: Vec<&str> = chains[0].iter().map(|l| l.item.name.as_str()).collect();
    assert_eq!(chain, vec!["lib", "flywheel", "create"]);
    assert!(graph.orphans().is_empty());
}

#[test]
fn historico_manda_mesmo_para_mod_que_ninguem_exige() {
    // `extra` não está no histórico e ninguém o exige: foi trazido e ficou sem uso.
    let mut data = input(vec![item("create", &[]), item("extra", &[])]);
    data.additions = Some(history(&[("create", None)]));
    let graph = Graph::build(&data);
    let orphans = graph.orphans();
    assert_eq!(
        names(&orphans.iter().map(|o| o.item.clone()).collect::<Vec<_>>()),
        vec!["extra"]
    );
    assert!(matches!(
        graph.why_in_pack(&path("extra")).unwrap().why,
        Why::Unused { .. }
    ));
}

#[test]
fn varias_cadeias_as_mais_curtas_primeiro_e_no_maximo_tres() {
    let mut items = vec![item("lib", &[])];
    let mut added = Vec::new();
    for n in 0..5 {
        let id = format!("user{n}");
        items.push(item(&id, &[("lib", "required")]));
        added.push(id);
    }
    // Um caminho mais longo: lib <- mid <- far (far é do usuário).
    items.push(item("mid", &[("lib", "required")]));
    items.push(item("far", &[("mid", "required")]));
    let mut data = input(items);
    let mut entries: Vec<(&str, Option<&str>)> =
        added.iter().map(|id| (id.as_str(), None)).collect();
    entries.push(("far", None));
    data.additions = Some(history(&entries));
    let graph = Graph::build(&data);
    let Why::RequiredBy { chains } = graph.why_in_pack(&path("lib")).unwrap().why else {
        panic!("esperava cadeia");
    };
    assert_eq!(chains.len(), 3);
    assert!(chains.iter().all(|chain| chain.len() == 2));
}

#[test]
fn opcional_nao_mantem_a_biblioteca() {
    let mut data = input(vec![
        item("mod", &[("geckolib", "optional")]),
        item("geckolib", &[]),
    ]);
    data.additions = None;
    let graph = Graph::build(&data);
    let orphans = graph.orphans();
    assert_eq!(orphans.len(), 1);
    assert_eq!(orphans[0].item.name, "geckolib");
    assert_eq!(names(&orphans[0].optional_users), vec!["mod"]);
    let Why::Unused { optional_users } = graph.why_in_pack(&path("geckolib")).unwrap().why else {
        panic!("esperava sem uso");
    };
    assert_eq!(names(&optional_users), vec!["mod"]);
    // Mas aparece em "Usado por", com o tipo.
    let used = graph.dependents(&[path("geckolib")]).unwrap();
    assert_eq!(used.used_by[0].kind, RelationKind::Optional);
    assert!(used.affected.is_empty());
}

#[test]
fn depende_de_mostra_falta_incompatibilidade_presente_e_ignora_o_que_o_loader_fornece() {
    let graph = Graph::build(&input(vec![
        item(
            "main",
            &[
                ("minecraft", "required"),
                ("neoforge", "required"),
                ("falta", "required"),
                ("rec", "recommends"),
                ("ruim", "incompatible"),
                ("ausente", "breaks"),
                ("lib", "optional"),
            ],
        ),
        item("ruim", &[]),
        item("lib", &[]),
    ]));
    let report = graph.dependents(&[path("main")]).unwrap();
    let summary: Vec<(&str, RelationKind, DependsOnState)> = report
        .depends_on
        .iter()
        .map(|d| (d.id.as_str(), d.kind, d.state))
        .collect();
    assert_eq!(
        summary,
        vec![
            ("falta", RelationKind::Required, DependsOnState::Missing),
            ("lib", RelationKind::Optional, DependsOnState::InPack),
            ("rec", RelationKind::Recommended, DependsOnState::Missing),
            ("ruim", RelationKind::Breaks, DependsOnState::InPack),
        ]
    );
    // Dependência que faltava não faz nada quebrar "de novo".
    assert!(
        graph
            .dependents(&[path("lib")])
            .unwrap()
            .affected
            .is_empty()
    );
}

#[test]
fn relacao_da_api_entra_quando_nao_ha_jar_e_nao_duplica_a_do_jar() {
    let mut a = GraphItem {
        path: path("a"),
        name: "a".into(),
        jar: None,
        api_required: vec![path("b"), path("nao-existe")],
    };
    let b = GraphItem {
        path: path("b"),
        name: "b".into(),
        jar: None,
        api_required: Vec::new(),
    };
    let graph = Graph::build(&input(vec![a.clone(), b.clone()]));
    assert_eq!(
        affected_names(&graph.dependents(&[path("b")]).unwrap()),
        vec![("a", 1)]
    );

    // Com jar que já declara a dependência, não há segunda ligação.
    a.jar = Some(jar("a", &[("b", "required")]));
    let b_with_jar = GraphItem {
        jar: Some(jar("b", &[])),
        ..b
    };
    let graph = Graph::build(&input(vec![a, b_with_jar]));
    assert_eq!(graph.dependents(&[path("b")]).unwrap().used_by.len(), 1);
}

#[test]
fn converte_os_itens_do_diagnostico_ligando_a_api_pelo_projeto() {
    use crate::pretest::{ApiMetadata, ApiRelation, ApiRelationKind, ReleaseChannel};
    use warden_packwiz::Side;

    let api = |project: &str, relations: Vec<ApiRelation>| ApiMetadata {
        source: crate::Source::Modrinth,
        url: format!("https://api.modrinth.com/v2/project/{project}"),
        project_id: project.into(),
        project_slug: None,
        game_versions: vec![],
        loaders: vec![],
        release: ReleaseChannel::Release,
        relations,
        archived: false,
        download_blocked: false,
        client_side: None,
        server_side: None,
    };
    let make = |id: &str, api: ApiMetadata| PackItem {
        path: path(id),
        filename: format!("{id}.jar"),
        side: Side::Both,
        file_hash: None,
        api: Some(api),
        jar: None,
        accepted_minecraft_versions: vec![],
        resource_pack_format: None,
    };
    let relation = |project: &str, kind| ApiRelation {
        project_id: project.into(),
        kind,
        field: "dependencies".into(),
    };
    let items = vec![
        make(
            "a",
            api(
                "PA",
                vec![
                    relation("PB", ApiRelationKind::Required),
                    relation("PC", ApiRelationKind::Incompatible),
                    relation("PX", ApiRelationKind::Required),
                ],
            ),
        ),
        make("b", api("PB", vec![])),
        make("c", api("PC", vec![])),
    ];
    let converted = GraphItem::from_pack_items(&items);
    assert_eq!(converted[0].api_required, vec![path("b")]);
    assert_eq!(converted[0].name, "a.jar");
    assert!(converted[1].api_required.is_empty());
    let graph = Graph::build(&input(converted));
    assert_eq!(
        affected_names(&graph.dependents(&[path("b")]).unwrap()),
        vec![("a.jar", 1)]
    );
}

#[test]
fn aresta_inferida_conta_como_dependencia_e_leva_a_nota() {
    let mut data = input(vec![item("a", &[]), item("b", &[]), item("c", &[])]);
    data.inferred = vec![
        InferredEdge {
            from: path("a"),
            to: path("b"),
            source: InferredSource::Bisect,
            note: "a busca do culpado travou sem o B".into(),
        },
        // Aresta para item que saiu do pack: ignorada.
        InferredEdge {
            from: path("a"),
            to: path("sumiu"),
            source: InferredSource::Log,
            note: String::new(),
        },
    ];
    let graph = Graph::build(&data);
    let report = graph.dependents(&[path("b")]).unwrap();
    assert_eq!(affected_names(&report), vec![("a", 1)]);
    assert_eq!(report.used_by[0].kind, RelationKind::Inferred);
    assert_eq!(
        report.used_by[0].note.as_deref(),
        Some("a busca do culpado travou sem o B")
    );
    let a = graph.dependents(&[path("a")]).unwrap();
    assert_eq!(a.depends_on.len(), 1);
    assert_eq!(a.depends_on[0].kind, RelationKind::Inferred);
}

#[test]
fn item_desconhecido_e_erro_e_o_json_segue_o_contrato() {
    let graph = Graph::build(&input(vec![
        item("a", &[("b", "required")]),
        item("b", &[]),
    ]));
    assert_eq!(
        graph.dependents(&[path("x")]).unwrap_err(),
        UnknownItem(path("x"))
    );
    assert!(graph.why_in_pack("x").is_err());
    assert!(
        graph
            .dependents(&[path("x")])
            .unwrap_err()
            .to_string()
            .contains("não está no grafo")
    );

    let report = graph.dependents(&[path("b")]).unwrap();
    let value = serde_json::to_value(&report).unwrap();
    assert_eq!(value["usedBy"][0]["kind"], "required");
    assert_eq!(value["affected"][0]["item"]["path"], "mods/a.pw.toml");
    let why = serde_json::to_value(graph.why_in_pack(&path("b")).unwrap()).unwrap();
    assert_eq!(why["why"]["kind"], "requiredBy");
    assert_eq!(why["why"]["chains"][0][1]["item"]["name"], "a");
    let unused = Why::Unused {
        optional_users: vec![],
    };
    assert_eq!(
        serde_json::to_value(unused).unwrap()["optionalUsers"],
        json!([])
    );
}

#[test]
fn mesmo_caminho_repetido_vale_uma_vez_e_pack_vazio_funciona() {
    let graph = Graph::build(&input(vec![item("a", &[]), item("a", &[])]));
    assert_eq!(graph.len(), 1);
    let empty = Graph::build(&input(vec![]));
    assert!(empty.is_empty());
    assert!(empty.orphans().is_empty());
    assert!(empty.cycles().is_empty());
}

proptest! {
    /// Para qualquer grafo de relações obrigatórias (com ciclos), o fecho de `dependents` é
    /// estável: tirar alvos e afetados não afeta mais ninguém; os afetados nunca incluem os
    /// alvos; e as sobras nunca incluem quem o usuário escolheu.
    #[test]
    fn dependentes_formam_um_ponto_fixo(
        edges in prop::collection::vec((0usize..8, 0usize..8), 0..20),
        targets in prop::collection::vec(0usize..8, 1..4),
        added in prop::collection::vec(0usize..8, 0..4),
    ) {
        let mut items: Vec<GraphItem> = (0..8)
            .map(|n| GraphItem { path: path(&format!("m{n}")), name: format!("m{n}"), jar: None, api_required: vec![] })
            .collect();
        for (from, to) in edges {
            if from != to {
                items[from].api_required.push(path(&format!("m{to}")));
            }
        }
        let mut data = input(items);
        let entries: Vec<(String, Option<&str>)> = added.iter().map(|n| (format!("m{n}"), None)).collect();
        data.additions = Some(history(&entries.iter().map(|(id, at)| (id.as_str(), *at)).collect::<Vec<_>>()));
        let graph = Graph::build(&data);
        let target_paths: Vec<String> = targets.iter().map(|n| path(&format!("m{n}"))).collect();
        let report = graph.dependents(&target_paths).unwrap();
        for affected in &report.affected {
            prop_assert!(!target_paths.contains(&affected.item.path));
            prop_assert!(affected.depth >= 1);
        }
        let mut all = target_paths.clone();
        all.extend(report.affected.iter().map(|a| a.item.path.clone()));
        let closed = graph.dependents(&all).unwrap();
        prop_assert!(closed.affected.is_empty());
        for orphan in graph.orphans() {
            prop_assert!(!data.additions.as_ref().unwrap().entries.contains_key(&orphan.item.path));
        }
    }
}
