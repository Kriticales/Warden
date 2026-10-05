//! Manifesto de versões da Mojang (`version_manifest_v2.json`; R2 §1.2).
//!
//! A ordem do manifesto é a ordem oficial (da mais nova para a mais antiga) e é mantida como
//! veio: o Minecraft passou a usar números por ano (`26.1`, `26.3`, `26.4-snapshot-2`), então
//! comparar ids como texto ou semver dá errado. O `sha1` de cada versão identifica o JSON
//! dela (o `time` muda quando a Mojang republica; o conteúdo, não), e é por ele que o JSON
//! fica no cache.

use serde::Deserialize;
use sha1::{Digest as _, Sha1};

use crate::model::{Freshness, MinecraftVersion, MinecraftVersionKind, MinecraftVersions};

/// A versão a partir da qual o suporte é garantido; as anteriores, pela ordem do manifesto,
/// são "melhor esforço" (ADR-0005).
pub const SUPPORTED_SINCE: &str = "1.7.10";

/// O manifesto lido.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Manifest {
    pub(crate) latest_release: Option<String>,
    pub(crate) latest_snapshot: Option<String>,
    pub(crate) entries: Vec<ManifestEntry>,
}

/// Uma versão do manifesto.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ManifestEntry {
    pub(crate) id: String,
    pub(crate) kind: MinecraftVersionKind,
    pub(crate) url: String,
    pub(crate) sha1: String,
    pub(crate) release_time: String,
}

#[derive(Deserialize)]
struct RawManifest {
    #[serde(default)]
    latest: RawLatest,
    versions: Vec<RawEntry>,
}

#[derive(Deserialize, Default)]
struct RawLatest {
    release: Option<String>,
    snapshot: Option<String>,
}

#[derive(Deserialize)]
struct RawEntry {
    id: String,
    #[serde(rename = "type", default)]
    kind: String,
    url: String,
    #[serde(default)]
    sha1: String,
    #[serde(default, rename = "releaseTime")]
    release_time: String,
}

/// Lê o manifesto. Erro: o texto do problema (JSON inválido, sem versões, `sha1` inválido,
/// id repetido).
pub(crate) fn parse_manifest(body: &[u8]) -> Result<Manifest, String> {
    let raw: RawManifest = serde_json::from_slice(body).map_err(|e| e.to_string())?;
    if raw.versions.is_empty() {
        return Err("o manifesto não tem versões".to_owned());
    }
    let mut seen = std::collections::HashSet::new();
    let mut entries = Vec::with_capacity(raw.versions.len());
    for entry in raw.versions {
        if entry.id.is_empty() {
            return Err("versão sem id no manifesto".to_owned());
        }
        if !seen.insert(entry.id.clone()) {
            return Err(format!("a versão {:?} aparece duas vezes", entry.id));
        }
        if !is_sha1(&entry.sha1) {
            return Err(format!("a versão {:?} não tem um sha1 válido", entry.id));
        }
        entries.push(ManifestEntry {
            id: entry.id,
            kind: MinecraftVersionKind::from_manifest(&entry.kind),
            url: entry.url,
            sha1: entry.sha1.to_ascii_lowercase(),
            release_time: entry.release_time,
        });
    }
    Ok(Manifest {
        latest_release: raw.latest.release,
        latest_snapshot: raw.latest.snapshot,
        entries,
    })
}

impl Manifest {
    /// A versão com esse id.
    pub(crate) fn entry(&self, id: &str) -> Option<&ManifestEntry> {
        self.entries.iter().find(|entry| entry.id == id)
    }

    /// As versões para a interface, na ordem do manifesto, com a etiqueta "melhor esforço"
    /// nas anteriores a [`SUPPORTED_SINCE`]. Se o manifesto não tiver a 1.7.10, só as alfas e
    /// betas antigas levam a etiqueta.
    pub(crate) fn to_versions(&self, freshness: Freshness) -> MinecraftVersions {
        let cutoff = self
            .entries
            .iter()
            .position(|entry| entry.id == SUPPORTED_SINCE);
        let versions = self
            .entries
            .iter()
            .enumerate()
            .map(|(index, entry)| MinecraftVersion {
                id: entry.id.clone(),
                kind: entry.kind,
                release_time: entry.release_time.clone(),
                best_effort: match cutoff {
                    Some(cutoff) => index > cutoff,
                    None => matches!(
                        entry.kind,
                        MinecraftVersionKind::OldAlpha | MinecraftVersionKind::OldBeta
                    ),
                },
            })
            .collect();
        MinecraftVersions {
            versions,
            latest_release: self.latest_release.clone(),
            latest_snapshot: self.latest_snapshot.clone(),
            freshness,
        }
    }
}

/// `sha1` em hexadecimal (40 caracteres).
pub(crate) fn is_sha1(text: &str) -> bool {
    text.len() == 40 && text.bytes().all(|byte| byte.is_ascii_hexdigit())
}

/// `sha1` de um conteúdo, em hexadecimal minúsculo.
pub(crate) fn sha1_hex(body: &[u8]) -> String {
    hex::encode(Sha1::digest(body))
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    fn manifest(versions: &str) -> String {
        format!(r#"{{"latest":{{"release":"26.3","snapshot":"x"}},"versions":[{versions}]}}"#)
    }

    fn entry(id: &str, kind: &str) -> String {
        format!(
            r#"{{"id":"{id}","type":"{kind}","url":"https://piston-meta.mojang.com/v1/packages/{sha}/{id}.json","sha1":"{sha}","releaseTime":"2026-01-01T00:00:00+00:00","complianceLevel":1}}"#,
            sha = "a".repeat(40)
        )
    }

    #[test]
    fn mantem_a_ordem_e_marca_melhor_esforco() {
        let body = manifest(
            &[
                entry("26.3", "release"),
                entry("1.7.10", "release"),
                entry("1.7.9", "release"),
                entry("b1.7.3", "old_beta"),
            ]
            .join(","),
        );
        let manifest = parse_manifest(body.as_bytes()).unwrap();
        let versions = manifest.to_versions(Freshness {
            fetched_at_ms: 1,
            offline: false,
        });
        let flags: Vec<(&str, bool)> = versions
            .versions
            .iter()
            .map(|v| (v.id.as_str(), v.best_effort))
            .collect();
        assert_eq!(
            flags,
            [
                ("26.3", false),
                ("1.7.10", false),
                ("1.7.9", true),
                ("b1.7.3", true)
            ]
        );
        assert_eq!(versions.latest_release.as_deref(), Some("26.3"));
        assert_eq!(manifest.entry("1.7.9").unwrap().sha1, "a".repeat(40));
        assert!(manifest.entry("2").is_none());
    }

    #[test]
    fn sem_1_7_10_so_as_antigas_sao_melhor_esforco() {
        let body = manifest(&[entry("1.2.5", "release"), entry("a1.0", "old_alpha")].join(","));
        let versions = parse_manifest(body.as_bytes())
            .unwrap()
            .to_versions(Freshness {
                fetched_at_ms: 1,
                offline: false,
            });
        assert!(!versions.versions[0].best_effort);
        assert!(versions.versions[1].best_effort);
    }

    #[test]
    fn recusa_manifesto_quebrado() {
        assert!(parse_manifest(b"{").is_err());
        assert!(parse_manifest(manifest("").as_bytes()).is_err());
        let repeated = manifest(&[entry("1", "release"), entry("1", "release")].join(","));
        assert!(
            parse_manifest(repeated.as_bytes())
                .unwrap_err()
                .contains("duas vezes")
        );
        let bad_sha = manifest(&entry("1", "release")).replace(&"a".repeat(40), "zz");
        assert!(parse_manifest(bad_sha.as_bytes()).is_err());
        let no_id = manifest(&entry("", "release"));
        assert!(parse_manifest(no_id.as_bytes()).is_err());
        // Sem `latest`: aceito.
        let body = format!(r#"{{"versions":[{}]}}"#, entry("1", "release"));
        assert!(
            parse_manifest(body.as_bytes())
                .unwrap()
                .latest_release
                .is_none()
        );
    }

    #[test]
    fn sha1_do_conteudo() {
        assert_eq!(sha1_hex(b""), "da39a3ee5e6b4b0d3255bfef95601890afd80709");
        assert!(is_sha1("DA39A3EE5E6B4B0D3255BFEF95601890AFD80709"));
        assert!(!is_sha1("da39"));
    }

    proptest! {
        #[test]
        fn manifesto_aleatorio_nao_entra_em_panico(bytes in proptest::collection::vec(any::<u8>(), 0..512)) {
            let _ = parse_manifest(&bytes);
        }
    }
}
