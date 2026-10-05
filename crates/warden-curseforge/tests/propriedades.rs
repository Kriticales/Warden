//! Propriedades: a decodificação tolera qualquer JSON sem pânico, e a busca só passa na
//! validação quando respeita os limites da API.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use proptest::prelude::*;
use serde_json::{Map, Value, json};
use warden_curseforge::{
    File, MAX_CATEGORY_IDS, MAX_GAME_VERSIONS, MAX_MOD_LOADER_TYPES, MAX_PAGE_SIZE,
    MAX_SEARCH_WINDOW, Mod, ModLoaderType, SearchQuery,
};

/// JSON qualquer, com os nomes de campo da API misturados a nomes aleatórios.
fn any_json() -> impl Strategy<Value = Value> {
    let leaf = prop_oneof![
        Just(Value::Null),
        any::<bool>().prop_map(Value::Bool),
        any::<i64>().prop_map(|n| json!(n)),
        any::<f64>().prop_map(|n| json!(n)),
        ".{0,12}".prop_map(Value::String),
    ];
    let key = prop_oneof![
        Just("id".to_owned()),
        Just("modId".to_owned()),
        Just("hashes".to_owned()),
        Just("algo".to_owned()),
        Just("value".to_owned()),
        Just("downloadUrl".to_owned()),
        Just("gameVersions".to_owned()),
        Just("dependencies".to_owned()),
        Just("relationType".to_owned()),
        Just("latestFiles".to_owned()),
        Just("classId".to_owned()),
        Just("allowModDistribution".to_owned()),
        Just("downloadCount".to_owned()),
        Just("fileLength".to_owned()),
        "[a-zA-Z]{1,8}",
    ];
    leaf.prop_recursive(4, 64, 6, move |inner| {
        prop_oneof![
            prop::collection::vec(inner.clone(), 0..6).prop_map(Value::Array),
            prop::collection::vec((key.clone(), inner), 0..8)
                .prop_map(|pairs| Value::Object(pairs.into_iter().collect::<Map<_, _>>())),
        ]
    })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    /// Qualquer JSON vira projeto/arquivo ou erro, nunca pânico; o que decodifica tem os
    /// auxiliares funcionando.
    #[test]
    fn decodificacao_nunca_entra_em_panico(value in any_json()) {
        if let Ok(file) = serde_json::from_value::<File>(value.clone()) {
            let _ = file.sha1();
            let _ = file.minecraft_versions();
            let _ = file.loaders();
            let _ = file.is_distribution_blocked();
            let _ = file.required_dependencies().count();
        }
        if let Ok(project) = serde_json::from_value::<Mod>(value) {
            let _ = project.file_page_url(1);
            let _ = project.is_distribution_blocked();
        }
    }

    /// A validação aceita exatamente as buscas dentro dos limites da API.
    #[test]
    fn validacao_igual_aos_limites(
        index in 0_u32..12_000,
        page_size in 0_u32..60,
        versions in 0_usize..7,
        loaders in 0_usize..8,
        categories in 0_usize..13,
    ) {
        let mut query = SearchQuery::new("x").page(index, page_size);
        for i in 0..versions {
            query = query.game_version(format!("1.{i}"));
        }
        for _ in 0..loaders {
            query = query.loader(ModLoaderType::Forge);
        }
        for id in 0..categories {
            query = query.category(u32::try_from(id).unwrap());
        }
        let within = (1..=MAX_PAGE_SIZE).contains(&page_size)
            && index + page_size <= MAX_SEARCH_WINDOW
            && versions <= MAX_GAME_VERSIONS
            && loaders <= MAX_MOD_LOADER_TYPES
            && categories <= MAX_CATEGORY_IDS;
        prop_assert_eq!(query.validate().is_ok(), within);
        let params = query.to_params();
        prop_assert!(params.iter().any(|(k, v)| *k == "pageSize" && *v == page_size.to_string()));
        prop_assert!(params.iter().any(|(k, v)| *k == "gameId" && v == "432"));
    }
}
