//! Forge (`maven-metadata.xml` + `promotions_slim.json`; R2 §3.4, R3 §1.5.1).
//!
//! Cada versão do Maven é `<minecraft>-<forge>[-<sufixo>]`:
//!
//! - `1.20.1-47.4.10`: o caso comum;
//! - `1.7.10-10.13.4.1614-1.7.10`: as versões antigas (1.7.2 a parte da 1.12.2) repetem a
//!   versão do Minecraft no fim. O `pack.toml` leva só `10.13.4.1614` (o packwiz não acha
//!   essas versões, issue #393; por isso o Warden tem a própria lista), e o artefato Maven
//!   leva o texto inteiro;
//! - `1.10-12.18.0.2000-1.10.0`, `1.11-13.19.0.2129-1.11.x`, `1.7.2-10.12.2.1161-mc172`: o
//!   sufixo é a linha do Minecraft (a recomendada da 1.7.2 é uma dessas);
//! - `1.7.10-10.13.1.1216-new`, `…-1710ls`, `…-prerelease`, `…-failtests`, `…-4627`: builds de
//!   ramos de teste, fora da lista.
//!
//! A "recomendada" vem de `<minecraft>-recommended` no `promotions_slim.json`. A lista de
//! quebradas ([`DENYLIST`]) é aplicada aqui.

use std::collections::HashMap;

use quick_xml::events::Event;
use serde::Deserialize;

use crate::model::{Loader, LoaderVersion};
use crate::order::{compare_numeric, is_dotted_number};

/// Versões conhecidas por não funcionar, fora das listas: `(loader, minecraft, versão, por
/// quê)`. A `1.12.2-14.23.5.2851` é excluída também pelo Prism (`BAD_VERSIONS`) e pelo
/// Modrinth (`BLACKLIST`) (R2 §3.6).
pub const DENYLIST: &[(Loader, &str, &str, &str)] = &[(
    Loader::Forge,
    "1.12.2",
    "14.23.5.2851",
    "instalador quebrado: excluída pelo Prism e pelo Modrinth",
)];

/// Se a versão está na lista de quebradas; devolve o motivo.
#[must_use]
pub fn denied(loader: Loader, minecraft: &str, version: &str) -> Option<&'static str> {
    DENYLIST
        .iter()
        .find(|(l, mc, v, _)| *l == loader && *mc == minecraft && *v == version)
        .map(|(.., reason)| *reason)
}

/// Uma versão do Forge lida do Maven.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ForgeEntry {
    /// Versão do Minecraft.
    pub(crate) minecraft: String,
    /// Versão do Forge, como vai no `pack.toml`.
    pub(crate) version: String,
    /// O texto inteiro do Maven (coordenada do artefato).
    pub(crate) artifact: String,
}

/// Lê uma versão do Maven do Forge. `None` para builds de ramos de teste e textos que não
/// seguem o formato.
pub(crate) fn parse_entry(raw: &str) -> Option<ForgeEntry> {
    let mut parts = raw.splitn(3, '-');
    let minecraft = parts.next().filter(|mc| !mc.is_empty())?;
    let version = parts.next().filter(|v| is_dotted_number(v, 2))?;
    if let Some(suffix) = parts.next()
        && !is_minecraft_line(suffix, minecraft)
    {
        return None;
    }
    Some(ForgeEntry {
        minecraft: minecraft.to_owned(),
        version: version.to_owned(),
        artifact: raw.to_owned(),
    })
}

/// Sufixo que é uma linha do Minecraft (`1.7.10`, `1.8`, `1.10.0`, `1.11.x` ou `mc172` na
/// 1.7.2), e não um ramo (`new`, `1710ls`, `prerelease`, `4627`).
fn is_minecraft_line(suffix: &str, minecraft: &str) -> bool {
    let trimmed = suffix.strip_suffix(".x").unwrap_or(suffix);
    is_dotted_number(trimmed, 2)
        || suffix
            .strip_prefix("mc")
            .is_some_and(|digits| digits == minecraft.replace('.', ""))
}

/// Os textos de `<versioning><versions><version>` de um `maven-metadata.xml`. Erro: o texto
/// do problema (XML inválido, sem versões).
pub(crate) fn parse_maven_metadata(body: &[u8]) -> Result<Vec<String>, String> {
    let mut reader = quick_xml::Reader::from_reader(body);
    reader.config_mut().trim_text(true);
    let mut buffer = Vec::new();
    let mut path: Vec<String> = Vec::new();
    let mut versions = Vec::new();
    loop {
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Start(start)) => path.push(start.name().as_ref().to_owned()),
            Ok(Event::End(_)) => {
                path.pop();
            }
            Ok(Event::Text(text)) if is_version_path(&path) => {
                let text: &str = text.as_ref();
                versions.push(text.trim().to_owned());
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(error) => {
                return Err(format!(
                    "XML inválido na posição {}: {error}",
                    reader.error_position()
                ));
            }
        }
        buffer.clear();
    }
    if versions.is_empty() {
        return Err("o maven-metadata.xml não tem versões".to_owned());
    }
    Ok(versions)
}

fn is_version_path(path: &[String]) -> bool {
    matches!(
        path,
        [.., a, b, c] if a == "versioning" && b == "versions" && c == "version"
    )
}

#[derive(Deserialize)]
struct RawPromotions {
    promos: HashMap<String, String>,
}

/// A recomendada e a mais recente de uma versão do Minecraft.
pub(crate) type Promotion = (Option<String>, Option<String>);

/// As promoções por versão do Minecraft.
pub(crate) type Promotions = HashMap<String, Promotion>;

/// As promoções do Forge: `minecraft → (recomendada, mais recente)`. Erro: o texto do
/// problema.
pub(crate) fn parse_promotions(body: &[u8]) -> Result<Promotions, String> {
    let raw: RawPromotions = serde_json::from_slice(body).map_err(|e| e.to_string())?;
    let mut promotions = Promotions::new();
    for (key, version) in raw.promos {
        let Some((minecraft, kind)) = key.rsplit_once('-') else {
            continue;
        };
        let slot = promotions.entry(minecraft.to_owned()).or_default();
        match kind {
            "recommended" => slot.0 = Some(version),
            "latest" => slot.1 = Some(version),
            _ => {}
        }
    }
    Ok(promotions)
}

/// As versões do Forge para `minecraft`, da mais nova para a mais antiga, sem as quebradas e
/// sem repetição, com a recomendada marcada. Devolve também a pré-selecionada: a recomendada,
/// senão a "mais recente" das promoções, senão a primeira da lista.
pub(crate) fn for_minecraft(
    maven_versions: &[String],
    promotions: &Promotions,
    minecraft: &str,
) -> (Vec<LoaderVersion>, Option<String>) {
    let (recommended, latest) = promotions.get(minecraft).cloned().unwrap_or_default();
    let mut entries: Vec<ForgeEntry> = Vec::new();
    for entry in maven_versions.iter().filter_map(|raw| parse_entry(raw)) {
        if entry.minecraft != minecraft
            || denied(Loader::Forge, minecraft, &entry.version).is_some()
            || entries.iter().any(|seen| seen.version == entry.version)
        {
            continue;
        }
        entries.push(entry);
    }
    entries.sort_by(|a, b| compare_numeric(&b.version, &a.version));
    let versions: Vec<LoaderVersion> = entries
        .into_iter()
        .map(|entry| LoaderVersion {
            recommended: recommended.as_deref() == Some(entry.version.as_str()),
            maven: format!("net.minecraftforge:forge:{}", entry.artifact),
            version: entry.version,
            stable: true,
        })
        .collect();
    let in_list = |candidate: &Option<String>| {
        candidate
            .as_ref()
            .filter(|version| versions.iter().any(|item| &item.version == *version))
            .cloned()
    };
    let preselected = in_list(&recommended)
        .or_else(|| in_list(&latest))
        .or_else(|| versions.first().map(|item| item.version.clone()));
    (versions, preselected)
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    #[test]
    fn le_os_formatos_do_maven() {
        let entry = parse_entry("1.7.10-10.13.4.1614-1.7.10").unwrap();
        assert_eq!(entry.minecraft, "1.7.10");
        assert_eq!(entry.version, "10.13.4.1614");
        assert_eq!(entry.artifact, "1.7.10-10.13.4.1614-1.7.10");
        assert_eq!(parse_entry("1.20.1-47.4.10").unwrap().version, "47.4.10");
        assert_eq!(
            parse_entry("1.10-12.18.0.2000-1.10.0").unwrap().version,
            "12.18.0.2000"
        );
        assert_eq!(
            parse_entry("1.11-13.19.0.2129-1.11.x").unwrap().version,
            "13.19.0.2129"
        );
        assert_eq!(
            parse_entry("1.7.2-10.12.2.1161-mc172").unwrap().version,
            "10.12.2.1161"
        );
        for branch in [
            "1.7.10-10.13.2.1291-new",
            "1.7.10-10.13.3.1401-1710ls",
            "1.7.10-10.13.3.1401-mc172",
            "1.7.10_pre4-10.12.2.1149-prerelease",
            "1.12.2-14.23.4.2720-4627",
            "1.8-11.14.0.1281-1.8-EHUnit",
            "1.20.1",
            "-47.1.1",
            "1.20.1-47",
            "1.20.1-47.a.1",
        ] {
            assert!(parse_entry(branch).is_none(), "{branch}");
        }
    }

    #[test]
    fn le_maven_metadata() {
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<metadata><groupId>net.minecraftforge</groupId><artifactId>forge</artifactId>
<versioning><latest>1.20.1-47.4.26</latest><release>1.20.1-47.4.26</release>
<versions>
  <version>1.20.1-47.4.26</version>
  <version> 1.7.10-10.13.4.1614-1.7.10 </version>
</versions><lastUpdated>20261004</lastUpdated></versioning></metadata>"#;
        assert_eq!(
            parse_maven_metadata(xml).unwrap(),
            ["1.20.1-47.4.26", "1.7.10-10.13.4.1614-1.7.10"]
        );
        assert!(parse_maven_metadata(b"<metadata></metadata>").is_err());
        assert!(parse_maven_metadata(b"<a><b></a>").is_err());
        assert!(parse_maven_metadata(b"").is_err());
        // `<version>` fora de `<versions>` não conta.
        assert!(parse_maven_metadata(b"<metadata><version>1</version></metadata>").is_err());
    }

    #[test]
    fn promocoes_por_minecraft() {
        let promos = parse_promotions(
            br#"{"homepage":"x","promos":{"1.7.10-latest":"10.13.4.1614",
                "1.7.10-recommended":"10.13.4.1614","1.21.6-latest":"56.0.9","x":"1","1.1-outra":"2"}}"#,
        )
        .unwrap();
        assert_eq!(
            promos["1.7.10"],
            (Some("10.13.4.1614".into()), Some("10.13.4.1614".into()))
        );
        assert_eq!(promos["1.21.6"], (None, Some("56.0.9".into())));
        assert_eq!(promos["1.1"], (None, None));
        assert!(parse_promotions(b"{}").is_err());
    }

    #[test]
    fn lista_por_minecraft_ordena_marca_e_aplica_a_denylist() {
        let maven: Vec<String> = [
            "1.12.2-14.23.5.2860",
            "1.12.2-14.23.5.2851",
            "1.12.2-14.23.5.2864",
            "1.12.2-14.23.4.2720-4627",
            "1.12.2-14.23.5.860",
            "1.12.2-14.23.5.2860",
            "1.12-14.21.1.2387",
        ]
        .iter()
        .map(ToString::to_string)
        .collect();
        let promos = HashMap::from([(
            "1.12.2".to_owned(),
            (
                Some("14.23.5.2860".to_owned()),
                Some("14.23.5.2864".to_owned()),
            ),
        )]);
        let (versions, preselected) = for_minecraft(&maven, &promos, "1.12.2");
        let list: Vec<&str> = versions.iter().map(|v| v.version.as_str()).collect();
        assert_eq!(list, ["14.23.5.2864", "14.23.5.2860", "14.23.5.860"]);
        assert!(versions[1].recommended && !versions[0].recommended);
        assert_eq!(preselected.as_deref(), Some("14.23.5.2860"));
        assert_eq!(
            versions[0].maven,
            "net.minecraftforge:forge:1.12.2-14.23.5.2864"
        );
        assert!(denied(Loader::Forge, "1.12.2", "14.23.5.2851").is_some());
        assert!(denied(Loader::NeoForge, "1.12.2", "14.23.5.2851").is_none());

        // Sem recomendada: a "mais recente" das promoções; sem promoções, a primeira.
        let promos = HashMap::from([("1.12.2".to_owned(), (None, Some("14.23.5.860".to_owned())))]);
        assert_eq!(
            for_minecraft(&maven, &promos, "1.12.2").1.as_deref(),
            Some("14.23.5.860")
        );
        let promos = HashMap::from([("1.12.2".to_owned(), (Some("9.9".to_owned()), None))]);
        assert_eq!(
            for_minecraft(&maven, &promos, "1.12.2").1.as_deref(),
            Some("14.23.5.2864")
        );
        let (versions, preselected) = for_minecraft(&maven, &HashMap::new(), "1.19.2");
        assert!(versions.is_empty());
        assert!(preselected.is_none());
    }

    proptest! {
        #[test]
        fn entrada_aleatoria_nao_entra_em_panico(bytes in proptest::collection::vec(any::<u8>(), 0..512)) {
            let _ = parse_maven_metadata(&bytes);
            let _ = parse_promotions(&bytes);
            if let Ok(text) = std::str::from_utf8(&bytes) {
                let _ = parse_entry(text);
            }
        }

        /// O sufixo `-<minecraft>` nunca entra na versão do `pack.toml`.
        #[test]
        fn sufixo_do_minecraft_sai_da_versao(
            minor in 2u32..13, patch in 0u32..11, a in 1u32..20, b in 0u32..30, c in 0u32..9, build in 0u32..3000
        ) {
            let mc = format!("1.{minor}.{patch}");
            let version = format!("{a}.{b}.{c}.{build}");
            let raw = format!("{mc}-{version}-{mc}");
            let entry = parse_entry(&raw).unwrap();
            prop_assert_eq!(entry.minecraft, mc);
            prop_assert_eq!(entry.version, version);
            prop_assert_eq!(entry.artifact, raw);
        }
    }
}
