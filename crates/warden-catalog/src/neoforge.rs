//! NeoForge (Maven `maven.neoforged.net`; R2 §3.5).
//!
//! Duas coordenadas:
//!
//! - `net.neoforged:forge`, só para a **1.20.1** (o fork inicial): `1.20.1-47.1.106`, e o
//!   `pack.toml` leva `47.1.106`. Um `47.1.82` sem o prefixo também aparece na lista da API,
//!   mas não tem instalador (404 em 05/10/2026) e fica de fora;
//! - `net.neoforged:neoforge`, da 1.20.2 em diante, com numeração própria:
//!   - `<minor>.<patch>.<build>[-beta]` até a 1.21.11: `20.2.3-beta` é a 1.20.2, `21.0.143`
//!     é a 1.21, `21.1.252` é a 1.21.1 e `21.11.45` é a 1.21.11;
//!   - `<ano>.<versão>.<patch>.<build>[-beta]` nas versões por ano: `26.1.0.19-beta` é a
//!     26.1 e `26.1.2.29-beta` é a 26.1.2.
//!
//! A versão do Minecraft é calculada de cada número, nunca por prefixo de texto: o filtro
//! `21.1` da API do NeoForge também casa `21.11.x` (R2 §3.5), e por isso os testes conferem
//! que `21.1.*` e `21.11.*` não se misturam. Versões de snapshot (`26.1.0.0-alpha.1+snapshot-1`)
//! e de brincadeira (`0.25w14craftmine.3-beta`) ficam de fora.

use serde::Deserialize;

use crate::model::LoaderVersion;
use crate::order::{compare_numeric, is_dotted_number};

/// A única versão do Minecraft com `net.neoforged:forge`.
pub(crate) const LEGACY_MINECRAFT: &str = "1.20.1";

#[derive(Deserialize)]
struct ApiVersions {
    versions: Vec<String>,
}

/// As versões da API `GET /api/maven/versions/releases/<grupo>/<artefato>`. Erro: o texto do
/// problema.
pub(crate) fn parse_api_versions(body: &[u8]) -> Result<Vec<String>, String> {
    let raw: ApiVersions = serde_json::from_slice(body).map_err(|e| e.to_string())?;
    if raw.versions.is_empty() {
        return Err("a lista de versões do NeoForge veio vazia".to_owned());
    }
    Ok(raw.versions)
}

/// A versão do Minecraft de uma versão de `net.neoforged:neoforge` e se ela é estável (sem
/// `-beta`). `None` para snapshots, brincadeiras e textos fora do formato.
pub(crate) fn minecraft_of(version: &str) -> Option<(String, bool)> {
    let (number, stable) = match version.strip_suffix("-beta") {
        Some(number) => (number, false),
        None => (version, true),
    };
    if !is_dotted_number(number, 3) {
        return None;
    }
    let parts: Vec<u64> = number
        .split('.')
        .map(str::parse)
        .collect::<Result<_, _>>()
        .ok()?;
    let minecraft = match parts.as_slice() {
        // Até a 1.21.11: <minor>.<patch>.<build>.
        [minor, patch, _] if (20..26).contains(minor) => {
            if *patch == 0 {
                format!("1.{minor}")
            } else {
                format!("1.{minor}.{patch}")
            }
        }
        // Por ano: <ano>.<versão>.<patch>.<build>.
        [year, release, patch, _] if *year >= 26 => {
            if *patch == 0 {
                format!("{year}.{release}")
            } else {
                format!("{year}.{release}.{patch}")
            }
        }
        _ => return None,
    };
    Some((minecraft, stable))
}

/// As versões de `net.neoforged:neoforge` para `minecraft`, da mais nova para a mais antiga.
pub(crate) fn for_minecraft(api_versions: &[String], minecraft: &str) -> Vec<LoaderVersion> {
    let mut versions: Vec<LoaderVersion> = api_versions
        .iter()
        .filter_map(|version| {
            let (mc, stable) = minecraft_of(version)?;
            (mc == minecraft).then(|| LoaderVersion {
                version: version.clone(),
                maven: format!("net.neoforged:neoforge:{version}"),
                stable,
                recommended: false,
            })
        })
        .collect();
    sort_and_dedup(&mut versions);
    versions
}

/// As versões de `net.neoforged:forge` (só 1.20.1), da mais nova para a mais antiga.
pub(crate) fn legacy_for_minecraft(api_versions: &[String], minecraft: &str) -> Vec<LoaderVersion> {
    if minecraft != LEGACY_MINECRAFT {
        return Vec::new();
    }
    let mut versions: Vec<LoaderVersion> = api_versions
        .iter()
        .filter_map(|raw| {
            let (mc, version) = raw.split_once('-')?;
            (mc == minecraft && is_dotted_number(version, 3)).then(|| LoaderVersion {
                version: version.to_owned(),
                maven: format!("net.neoforged:forge:{raw}"),
                stable: true,
                recommended: false,
            })
        })
        .collect();
    sort_and_dedup(&mut versions);
    versions
}

fn sort_and_dedup(versions: &mut Vec<LoaderVersion>) {
    versions.sort_by(|a, b| compare_numeric(&b.version, &a.version));
    versions.dedup_by(|a, b| a.version == b.version);
}

/// A pré-selecionada: a mais recente estável; sem estável, a mais recente.
pub(crate) fn preselect(versions: &[LoaderVersion]) -> Option<String> {
    versions
        .iter()
        .find(|item| item.stable)
        .or_else(|| versions.first())
        .map(|item| item.version.clone())
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    #[test]
    fn versao_do_minecraft_de_cada_numeracao() {
        let cases = [
            ("20.2.3-beta", Some(("1.20.2", false))),
            ("21.0.143", Some(("1.21", true))),
            ("21.1.252", Some(("1.21.1", true))),
            ("21.11.45", Some(("1.21.11", true))),
            ("26.1.0.19-beta", Some(("26.1", false))),
            ("26.1.2.29-beta", Some(("26.1.2", false))),
            ("26.3.0.48", Some(("26.3", true))),
            ("26.1.0.0-alpha.1+snapshot-1", None),
            ("0.25w14craftmine.3-beta", None),
            ("21.1", None),
            ("19.2.1", None),
            ("21.1.1-rc", None),
            ("25.1.0.1", None),
            ("21.1.99999999999999999999999", None),
        ];
        for (version, expected) in cases {
            let got = minecraft_of(version);
            let expected = expected.map(|(mc, stable)| (mc.to_owned(), stable));
            assert_eq!(got, expected, "{version}");
        }
    }

    #[test]
    fn filtro_com_ponto_final_nao_mistura_21_1_com_21_11() {
        let api: Vec<String> = ["21.1.1", "21.11.0-beta", "21.1.252", "21.11.45", "21.10.3"]
            .iter()
            .map(ToString::to_string)
            .collect();
        let list: Vec<String> = for_minecraft(&api, "1.21.1")
            .into_iter()
            .map(|v| v.version)
            .collect();
        assert_eq!(list, ["21.1.252", "21.1.1"]);
        let list: Vec<String> = for_minecraft(&api, "1.21.11")
            .into_iter()
            .map(|v| v.version)
            .collect();
        assert_eq!(list, ["21.11.45", "21.11.0-beta"]);
    }

    #[test]
    fn neoforge_1_20_1_vem_de_net_neoforged_forge() {
        let api: Vec<String> = ["1.20.1-47.1.5", "1.20.1-47.1.106", "47.1.82", "1.20.1-x"]
            .iter()
            .map(ToString::to_string)
            .collect();
        let list = legacy_for_minecraft(&api, "1.20.1");
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].version, "47.1.106");
        assert_eq!(list[0].maven, "net.neoforged:forge:1.20.1-47.1.106");
        assert!(legacy_for_minecraft(&api, "1.20.2").is_empty());
    }

    #[test]
    fn preselecao_prefere_estavel() {
        let api: Vec<String> = ["26.3.0.48-beta", "26.3.0.2-beta"]
            .iter()
            .map(ToString::to_string)
            .collect();
        let versions = for_minecraft(&api, "26.3");
        assert_eq!(preselect(&versions).as_deref(), Some("26.3.0.48-beta"));
        let api: Vec<String> = ["21.1.3-beta", "21.1.2", "21.1.2"]
            .iter()
            .map(ToString::to_string)
            .collect();
        let versions = for_minecraft(&api, "1.21.1");
        assert_eq!(versions.len(), 2);
        assert_eq!(preselect(&versions).as_deref(), Some("21.1.2"));
        assert!(preselect(&[]).is_none());
    }

    #[test]
    fn le_a_api() {
        assert_eq!(
            parse_api_versions(br#"{"isSnapshot":false,"versions":["21.1.1"]}"#).unwrap(),
            ["21.1.1"]
        );
        assert!(parse_api_versions(br#"{"versions":[]}"#).is_err());
        assert!(parse_api_versions(b"[]").is_err());
    }

    proptest! {
        /// `<minor>.<patch>.<build>` sempre vira `1.<minor>[.<patch>]`, e o filtro de uma
        /// versão do Minecraft nunca pega outra com o mesmo começo de texto.
        #[test]
        fn numeracao_antiga(minor in 20u64..22, patch in 0u64..12, build in 0u64..500, beta: bool) {
            let version = format!("{minor}.{patch}.{build}{}", if beta { "-beta" } else { "" });
            let (mc, stable) = minecraft_of(&version).unwrap();
            let expected = if patch == 0 { format!("1.{minor}") } else { format!("1.{minor}.{patch}") };
            prop_assert_eq!(&mc, &expected);
            prop_assert_eq!(stable, !beta);
            let other = format!("1.{minor}.{}", patch * 10 + 1);
            prop_assert!(for_minecraft(std::slice::from_ref(&version), &other).is_empty());
        }

        #[test]
        fn numeracao_por_ano(year in 26u64..40, release in 1u64..5, patch in 0u64..4, build in 0u64..100) {
            let version = format!("{year}.{release}.{patch}.{build}-beta");
            let (mc, _) = minecraft_of(&version).unwrap();
            let expected = if patch == 0 { format!("{year}.{release}") } else { format!("{year}.{release}.{patch}") };
            prop_assert_eq!(mc, expected);
        }

        #[test]
        fn entrada_aleatoria_nao_entra_em_panico(text in "\\PC*", bytes in proptest::collection::vec(any::<u8>(), 0..256)) {
            let _ = minecraft_of(&text);
            let _ = parse_api_versions(&bytes);
        }
    }
}
