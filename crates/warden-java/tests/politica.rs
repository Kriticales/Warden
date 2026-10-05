//! A política "o mais novo que funciona" contra a tabela embutida (ADR-0029).
//!
//! - Critério 1 da L-01: 8 para Forge 1.7.10 e 1.12.2; 8 ≤ u312 para Forge 1.16.5 36.2.25; 17
//!   para 1.17.1 e 1.20.1; 21 para 1.21.1; 25 para 26.3.
//! - Critério 4 da L-01: cada escolha vem com o motivo esperado; **cada regra da tabela tem um
//!   teste aqui** (o teste `toda_regra_da_tabela_tem_teste` falha se uma regra nova não tiver).

// Nos auxiliares dos testes, `unwrap`, `expect` e `panic!` são a forma idiomática de falhar
// (o `clippy.toml` só os libera dentro das funções `#[test]`).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::BTreeSet;

use warden_java::policy::{choose, decide, pick_runtime};
use warden_java::{
    CompatibilityTable, InstalledRuntime, JavaChoiceReason, JavaChoiceRequest, JavaRequirement,
    JavaVersion, LoaderKind, Platform, RuntimeId, RuntimeSource, VersionJavaRequirement,
};

fn table() -> &'static CompatibilityTable {
    CompatibilityTable::builtin().unwrap()
}

/// Um caso: pack, Java esperado, teto, motivo e a regra da tabela que decide.
struct Case {
    minecraft: &'static str,
    loader: LoaderKind,
    loader_version: Option<&'static str>,
    major: u32,
    max_update: Option<u32>,
    reason: JavaChoiceReason,
    rule: &'static str,
}

const fn case(
    minecraft: &'static str,
    loader: LoaderKind,
    loader_version: Option<&'static str>,
    major: u32,
    max_update: Option<u32>,
    reason: JavaChoiceReason,
    rule: &'static str,
) -> Case {
    Case {
        minecraft,
        loader,
        loader_version,
        major,
        max_update,
        reason,
        rule,
    }
}

use JavaChoiceReason::{Forge1165Old, ForgeLegacyJava8, NewestAvailable, NewestProvenForRange};
use LoaderKind::{Fabric, Forge, NeoForge, Quilt, Vanilla};

/// Os casos do critério 1 e um ou mais por regra da tabela, com as versões da matriz L-05.
const CASES: &[Case] = &[
    // Critério 1.
    case(
        "1.7.10",
        Forge,
        Some("10.13.4.1614"),
        8,
        None,
        ForgeLegacyJava8,
        "forge-antigo-so-java-8",
    ),
    case(
        "1.12.2",
        Forge,
        Some("14.23.5.2860"),
        8,
        None,
        ForgeLegacyJava8,
        "forge-antigo-so-java-8",
    ),
    case(
        "1.16.5",
        Forge,
        Some("36.2.25"),
        8,
        Some(312),
        Forge1165Old,
        "forge-1.16.5-anterior-ao-36.2.26",
    ),
    case(
        "1.17.1",
        Forge,
        Some("37.1.1"),
        17,
        None,
        NewestProvenForRange,
        "1.17-a-1.20.4",
    ),
    case(
        "1.20.1",
        Forge,
        Some("47.4.10"),
        17,
        None,
        NewestProvenForRange,
        "1.17-a-1.20.4",
    ),
    case(
        "1.21.1",
        NeoForge,
        Some("21.1.252"),
        21,
        None,
        NewestProvenForRange,
        "1.20.5-a-1.21.x",
    ),
    case(
        "26.3",
        Fabric,
        Some("0.19.5"),
        25,
        None,
        NewestAvailable,
        "26.x",
    ),
    // Forge 1.16.5 corrigido e versões antigas do Forge com o prefixo do Maven.
    case(
        "1.16.5",
        Forge,
        Some("36.2.34"),
        8,
        None,
        NewestProvenForRange,
        "ate-1.16.5",
    ),
    case(
        "1.16.5",
        Forge,
        Some("1.16.5-36.2.26"),
        8,
        None,
        NewestProvenForRange,
        "ate-1.16.5",
    ),
    case(
        "1.16.5",
        Forge,
        Some("1.16.5-36.2.25"),
        8,
        Some(312),
        Forge1165Old,
        "forge-1.16.5-anterior-ao-36.2.26",
    ),
    case(
        "1.7.10",
        Forge,
        Some("1.7.10-10.13.4.1614-1.7.10"),
        8,
        None,
        ForgeLegacyJava8,
        "forge-antigo-so-java-8",
    ),
    // Faixa até 1.16.5 com outros loaders (a regra de Forge não vale).
    case(
        "1.16.5",
        Fabric,
        Some("0.19.5"),
        8,
        None,
        NewestProvenForRange,
        "ate-1.16.5",
    ),
    case(
        "1.12.2",
        Vanilla,
        None,
        8,
        None,
        NewestProvenForRange,
        "ate-1.16.5",
    ),
    case(
        "1.7.10",
        LoaderKind::LiteLoader,
        None,
        8,
        None,
        NewestProvenForRange,
        "ate-1.16.5",
    ),
    // Forge 1.13+ não é "Forge antigo".
    case(
        "1.13.2",
        Forge,
        Some("25.0.223"),
        8,
        None,
        NewestProvenForRange,
        "ate-1.16.5",
    ),
    // 1.17–1.20.4.
    case(
        "1.18.2",
        Forge,
        Some("40.3.0"),
        17,
        None,
        NewestProvenForRange,
        "1.17-a-1.20.4",
    ),
    case(
        "1.19.2",
        Fabric,
        Some("0.19.5"),
        17,
        None,
        NewestProvenForRange,
        "1.17-a-1.20.4",
    ),
    case(
        "1.20.1",
        NeoForge,
        Some("47.1.106"),
        17,
        None,
        NewestProvenForRange,
        "1.17-a-1.20.4",
    ),
    case(
        "1.20.4",
        Quilt,
        Some("0.24.0"),
        17,
        None,
        NewestProvenForRange,
        "1.17-a-1.20.4",
    ),
    case(
        "1.17",
        Vanilla,
        None,
        17,
        None,
        NewestProvenForRange,
        "1.17-a-1.20.4",
    ),
    // 1.20.5–1.21.x (inclusive pré-versão e versões de dois dígitos).
    case(
        "1.20.5",
        Vanilla,
        None,
        21,
        None,
        NewestProvenForRange,
        "1.20.5-a-1.21.x",
    ),
    case(
        "1.20.5-pre1",
        Vanilla,
        None,
        21,
        None,
        NewestProvenForRange,
        "1.20.5-a-1.21.x",
    ),
    case(
        "1.21.1",
        Forge,
        Some("52.1.0"),
        21,
        None,
        NewestProvenForRange,
        "1.20.5-a-1.21.x",
    ),
    case(
        "1.21.11",
        Fabric,
        Some("0.19.5"),
        21,
        None,
        NewestProvenForRange,
        "1.20.5-a-1.21.x",
    ),
    // 26.x: o mais novo que o Warden baixa.
    case(
        "26.2",
        Forge,
        Some("65.1.0"),
        25,
        None,
        NewestAvailable,
        "26.x",
    ),
    case(
        "26.2",
        NeoForge,
        Some("26.2.0.88"),
        25,
        None,
        NewestAvailable,
        "26.x",
    ),
    case("26.1", Vanilla, None, 25, None, NewestAvailable, "26.x"),
];

fn request(case: &Case) -> JavaChoiceRequest {
    JavaChoiceRequest::automatic(case.minecraft, case.loader, case.loader_version)
}

#[test]
fn l01_ca1_e_ca4_cada_caso_com_o_java_e_o_motivo_esperados() {
    for case in CASES {
        let decision = decide(&request(case), table()).unwrap();
        let label = format!(
            "{} {:?} {:?}",
            case.minecraft, case.loader, case.loader_version
        );
        assert_eq!(decision.requirement.major, case.major, "{label}");
        assert_eq!(decision.requirement.max_update, case.max_update, "{label}");
        assert_eq!(decision.reason, case.reason, "{label}");
        assert_eq!(decision.rule_id.as_deref(), Some(case.rule), "{label}");
        assert_eq!(decision.newest_major, 25, "{label}");
        assert!(decision.explanation.len() > 40, "{label}: motivo vazio");
        assert!(decision.range_label.is_some(), "{label}");
        // "Por que não o Java N?" só aparece quando não é o mais novo.
        assert_eq!(
            decision.is_older_than_newest(),
            case.reason != NewestAvailable,
            "{label}"
        );
    }
}

#[test]
fn l01_ca4_toda_regra_da_tabela_tem_teste() {
    let tested: BTreeSet<&str> = CASES.iter().map(|case| case.rule).collect();
    let table = table();
    let all: BTreeSet<&str> = table
        .loader_rules
        .iter()
        .map(|rule| rule.id.as_str())
        .chain(table.ranges.iter().map(|rule| rule.id.as_str()))
        .collect();
    let missing: Vec<&&str> = all.difference(&tested).collect();
    assert!(missing.is_empty(), "regras sem teste: {missing:?}");
}

#[test]
fn l01_ca4_motivos_das_regras_de_loader() {
    let legacy = decide(
        &JavaChoiceRequest::automatic("1.7.10", Forge, Some("10.13.4.1614")),
        table(),
    )
    .unwrap();
    // O motivo do Forge 1.7.10 diz que essa versão só abre no Java 8 (CA-T21-04).
    assert!(
        legacy.explanation.contains("só abre no Java 8"),
        "{}",
        legacy.explanation
    );
    assert!(legacy.explanation.contains("1.7.10"));
    let old_1165 = decide(
        &JavaChoiceRequest::automatic("1.16.5", Forge, Some("36.2.25")),
        table(),
    )
    .unwrap();
    assert!(
        old_1165.explanation.contains("8u321"),
        "{}",
        old_1165.explanation
    );
    assert!(old_1165.explanation.contains("312"));
}

#[test]
fn versao_do_forge_1_16_5_desconhecida_usa_o_teto() {
    let decision = decide(
        &JavaChoiceRequest::automatic("1.16.5", Forge, None),
        table(),
    )
    .unwrap();
    assert_eq!(decision.reason, Forge1165Old);
    assert_eq!(decision.requirement.max_update, Some(312));
}

#[test]
fn fora_da_tabela_usa_o_json_da_versao() {
    let mut snapshot = JavaChoiceRequest::automatic("24w14a", Vanilla, None);
    // Sem JSON lido: erro com código próprio, nunca um palpite.
    let error = decide(&snapshot, table()).unwrap_err();
    assert!(matches!(
        error,
        warden_java::Error::VersionRequirementUnknown { .. }
    ));

    snapshot.version_json = VersionJavaRequirement::Major { major: 21 };
    let decision = decide(&snapshot, table()).unwrap();
    assert_eq!(decision.requirement.major, 21);
    assert_eq!(decision.reason, JavaChoiceReason::FromVersionJson);
    assert_eq!(decision.version_json_major, Some(21));
    assert!(decision.rule_id.is_none());

    // Java 16 do JSON (1.17 snapshot) vira 17; 22 vira 25 (o menor LTS que atende).
    snapshot.version_json = VersionJavaRequirement::Major { major: 16 };
    assert_eq!(decide(&snapshot, table()).unwrap().requirement.major, 17);
    snapshot.version_json = VersionJavaRequirement::Major { major: 22 };
    assert_eq!(decide(&snapshot, table()).unwrap().requirement.major, 25);

    let mut old = JavaChoiceRequest::automatic("b1.7.3", Vanilla, None);
    old.version_json = VersionJavaRequirement::Absent;
    let decision = decide(&old, table()).unwrap();
    assert_eq!(decision.requirement.major, 8);
    assert_eq!(decision.reason, JavaChoiceReason::FromVersionJson);

    // 27.x ainda não tem faixa: vale o JSON.
    let mut future = JavaChoiceRequest::automatic("27.1", Vanilla, None);
    future.version_json = VersionJavaRequirement::Major { major: 25 };
    assert_eq!(decide(&future, table()).unwrap().requirement.major, 25);
}

#[test]
fn a_tabela_vale_mesmo_com_o_json_da_versao() {
    // O 1.17.1 pede 16 no JSON; a tabela manda 17.
    let mut request = JavaChoiceRequest::automatic("1.17.1", Fabric, Some("0.19.5"));
    request.version_json = VersionJavaRequirement::Major { major: 16 };
    let decision = decide(&request, table()).unwrap();
    assert_eq!(decision.requirement.major, 17);
    assert_eq!(decision.reason, NewestProvenForRange);
}

fn runtime(source: RuntimeSource, version: &str) -> InstalledRuntime {
    let platform = Platform::current().unwrap();
    let version = JavaVersion::parse(version).unwrap();
    let id = RuntimeId::new(source, &version, &version.to_string(), platform);
    let home = std::path::PathBuf::from("runtimes").join(id.as_str());
    InstalledRuntime {
        java: platform.java_in(&home),
        launcher: platform.javaw_in(&home),
        id,
        source,
        version,
        vendor: String::new(),
        release_name: String::new(),
        arch: "x64".into(),
        home,
        installed_at_ms: 0,
        update_cap: None,
        mojang_component: None,
        archive_sha256: None,
        superseded_by: None,
    }
}

#[test]
fn escolhe_a_atualizacao_mais_nova_instalada_respeitando_o_teto() {
    let installed = vec![
        runtime(RuntimeSource::Temurin, "8u302-b08"),
        runtime(RuntimeSource::Temurin, "8u312-b07"),
        runtime(RuntimeSource::Temurin, "8u504-b01"),
        runtime(RuntimeSource::Mojang, "8u51"),
        runtime(RuntimeSource::Temurin, "17.0.15"),
        runtime(RuntimeSource::Mojang, "17.0.20.1"),
        runtime(RuntimeSource::Temurin, "17.0.20.1+1"),
    ];
    let any8 = JavaRequirement {
        major: 8,
        max_update: None,
    };
    assert_eq!(
        pick_runtime(&any8, &installed).unwrap().version.security,
        504
    );
    let capped = JavaRequirement {
        major: 8,
        max_update: Some(312),
    };
    assert_eq!(
        pick_runtime(&capped, &installed).unwrap().version.security,
        312
    );
    let none = JavaRequirement {
        major: 8,
        max_update: Some(50),
    };
    assert!(pick_runtime(&none, &installed).is_none());
    // Empate de versão: o Temurin.
    let java17 = JavaRequirement {
        major: 17,
        max_update: None,
    };
    let picked = pick_runtime(&java17, &installed).unwrap();
    assert_eq!(picked.source, RuntimeSource::Temurin);
    assert_eq!(picked.version.security, 20);
    let java21 = JavaRequirement {
        major: 21,
        max_update: None,
    };
    assert!(pick_runtime(&java21, &installed).is_none());
}

#[test]
fn escolha_do_usuario_prevalece_e_sobrevive_a_atualizacao() {
    let old17 = runtime(RuntimeSource::Temurin, "17.0.15");
    let new17 = runtime(RuntimeSource::Temurin, "17.0.20.1+1");
    let java21 = runtime(RuntimeSource::Temurin, "21.0.12.1+1");
    let mut request = JavaChoiceRequest::automatic("1.20.1", Forge, Some("47.4.10"));
    request.user_choice = Some(java21.id.clone());
    let installed = vec![old17.clone(), java21.clone()];
    let choice = choose(&request, table(), &installed).unwrap();
    assert_eq!(choice.reason, JavaChoiceReason::UserChoice);
    assert_eq!(choice.major, 21);
    assert_eq!(choice.runtime.as_ref().unwrap().id, java21.id);
    // O motivo automático continua disponível.
    assert_eq!(choice.automatic.requirement.major, 17);
    assert!(!choice.user_choice_unavailable);

    // O usuário escolheu o 17 antigo; uma atualização o substituiu e o removeu: vale o 17
    // novo da mesma fonte.
    request.user_choice = Some(old17.id.clone());
    let choice = choose(&request, table(), &[new17.clone(), java21.clone()]).unwrap();
    assert_eq!(choice.reason, JavaChoiceReason::UserChoice);
    assert_eq!(choice.runtime.unwrap().id, new17.id);

    // Escolhido um Java que não existe mais e sem substituto: vale o automático, com aviso.
    request.user_choice = Some(old17.id);
    let choice = choose(&request, table(), &[java21]).unwrap();
    assert_eq!(choice.reason, NewestProvenForRange);
    assert_eq!(choice.major, 17);
    assert!(choice.runtime.is_none());
    assert!(choice.user_choice_unavailable);
}

#[test]
fn automatico_acha_o_java_instalado() {
    let installed = vec![runtime(RuntimeSource::Temurin, "21.0.12.1+1")];
    let choice = choose(
        &JavaChoiceRequest::automatic("1.21.1", Fabric, Some("0.19.5")),
        table(),
        &installed,
    )
    .unwrap();
    assert_eq!(choice.runtime.unwrap().version.major, 21);
    let missing = choose(
        &JavaChoiceRequest::automatic("26.3", Fabric, Some("0.19.5")),
        table(),
        &installed,
    )
    .unwrap();
    assert_eq!(missing.major, 25);
    assert!(missing.runtime.is_none());
}

#[test]
fn motivos_serializados_como_a_architecture_define() {
    let json = serde_json::to_value([
        JavaChoiceReason::UserChoice,
        JavaChoiceReason::ForgeLegacyJava8,
        JavaChoiceReason::Forge1165Old,
        JavaChoiceReason::NewestProvenForRange,
        JavaChoiceReason::NewestAvailable,
        JavaChoiceReason::FromVersionJson,
    ])
    .unwrap();
    assert_eq!(
        json,
        serde_json::json!([
            "USER_CHOICE",
            "FORGE_LEGACY_JAVA8",
            "FORGE_1165_OLD",
            "NEWEST_PROVEN_FOR_RANGE",
            "NEWEST_AVAILABLE",
            "FROM_VERSION_JSON"
        ])
    );
}

mod propriedades {
    use proptest::prelude::*;

    use super::*;

    proptest! {
        /// Para qualquer versão de lançamento, a política decide sem erro (há faixa para tudo
        /// de 1.0 a 26.x), o major está na lista e nunca passa do mais novo.
        #[test]
        fn qualquer_lancamento_tem_decisao(
            minor in 0_u32..22,
            patch in 0_u32..12,
            loader in prop::sample::select(vec![Vanilla, Fabric, Forge, NeoForge, Quilt]),
            loader_version in "[0-9]{1,2}\\.[0-9]{1,2}\\.[0-9]{1,3}",
        ) {
            let minecraft = format!("1.{minor}.{patch}");
            let request = JavaChoiceRequest::automatic(&minecraft, loader, Some(&loader_version));
            let decision = decide(&request, table()).unwrap();
            prop_assert!(table().majors.contains(&decision.requirement.major));
            prop_assert!(decision.requirement.major <= decision.newest_major);
            if loader == Forge && minor <= 12 {
                prop_assert_eq!(decision.requirement.major, 8);
                prop_assert_eq!(decision.reason, ForgeLegacyJava8);
            }
        }

        /// Entradas arbitrárias nunca entram em pânico.
        #[test]
        fn entradas_arbitrarias_nao_quebram(minecraft in ".{0,20}", loader_version in ".{0,20}") {
            let request = JavaChoiceRequest::automatic(&minecraft, Forge, Some(&loader_version));
            let _ = decide(&request, table());
            let _ = JavaVersion::parse(&minecraft);
        }
    }
}
