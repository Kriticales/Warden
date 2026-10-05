//! Propriedades: facets sempre viram o JSON que a API espera, e respostas quaisquer nunca
//! derrubam a leitura dos modelos (entrada externa não confiável, QUALITY §9 item 6).

use proptest::prelude::*;
use warden_modrinth::{Facets, Project, SearchQuery, Version};

fn json_value() -> impl Strategy<Value = serde_json::Value> {
    let leaf = prop_oneof![
        Just(serde_json::Value::Null),
        any::<bool>().prop_map(serde_json::Value::from),
        any::<i64>().prop_map(serde_json::Value::from),
        ".{0,12}".prop_map(serde_json::Value::from),
    ];
    leaf.prop_recursive(4, 32, 6, |inner| {
        prop_oneof![
            prop::collection::vec(inner.clone(), 0..6).prop_map(serde_json::Value::Array),
            prop::collection::btree_map(
                prop_oneof![
                    Just("id".to_owned()),
                    Just("files".to_owned()),
                    Just("environment".to_owned()),
                    Just("version_type".to_owned()),
                    "[a-z_]{1,10}"
                ],
                inner,
                0..6
            )
            .prop_map(|map| serde_json::Value::Object(map.into_iter().collect())),
        ]
    })
}

proptest! {
    #[test]
    fn facets_viram_lista_de_listas(groups in prop::collection::vec(prop::collection::vec(".{0,16}", 0..4), 0..5)) {
        let mut facets = Facets::new();
        for group in &groups {
            facets = facets.any_of(group.clone());
        }
        let parsed: Vec<Vec<String>> = serde_json::from_str(&facets.to_json()).unwrap();
        let expected: Vec<Vec<String>> = groups.into_iter().filter(|g| !g.is_empty()).collect();
        prop_assert_eq!(parsed, expected);
    }

    #[test]
    fn limite_da_pagina_sempre_entre_1_e_100(limit in any::<u32>(), offset in any::<u32>()) {
        let params = SearchQuery::new("x").page(offset, limit).to_params();
        let limit: u32 = params.iter().find(|(k, _)| *k == "limit").unwrap().1.parse().unwrap();
        prop_assert!((1..=100).contains(&limit));
    }

    #[test]
    fn json_qualquer_nao_derruba_os_modelos(value in json_value()) {
        let _ = serde_json::from_value::<Version>(value.clone());
        let _ = serde_json::from_value::<Project>(value);
    }
}
