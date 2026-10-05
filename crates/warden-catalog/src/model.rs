//! Tipos do catálogo que chegam à interface (pelo `tauri-specta`) e às outras crates.

use std::cmp::Ordering;
use std::fmt;

use serde::{Deserialize, Serialize};

/// Quem fornece cada lista.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Source {
    /// Manifesto e JSON das versões (`piston-meta.mojang.com`).
    Mojang,
    /// `meta.fabricmc.net`.
    Fabric,
    /// Maven e promoções do Forge (`maven.minecraftforge.net`, `files.minecraftforge.net`).
    Forge,
    /// Maven do NeoForge (`maven.neoforged.net`).
    NeoForge,
}

impl fmt::Display for Source {
    /// O nome mostrado nas frases de erro.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Mojang => "Mojang",
            Self::Fabric => "Fabric",
            Self::Forge => "Forge",
            Self::NeoForge => "NeoForge",
        })
    }
}

/// Loaders do catálogo (ADR-0005). O texto é o mesmo do `[versions]` do `pack.toml`.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, specta::Type,
)]
#[serde(rename_all = "lowercase")]
pub enum Loader {
    /// Forge (todas as versões, inclusive 1.7.10 e 1.12.2).
    Forge,
    /// NeoForge (1.20.1 em diante).
    #[serde(rename = "neoforge")]
    NeoForge,
    /// Fabric (1.14 em diante).
    Fabric,
}

impl Loader {
    /// A fonte da lista deste loader.
    #[must_use]
    pub fn source(self) -> Source {
        match self {
            Self::Forge => Source::Forge,
            Self::NeoForge => Source::NeoForge,
            Self::Fabric => Source::Fabric,
        }
    }
}

/// De quando é a lista e se veio do cache porque a fonte não respondeu (SPEC T03: "Lista de
/// versões de <data>").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct Freshness {
    /// Quando a lista foi baixada da fonte (milissegundos desde 1970, UTC). Numa lista
    /// montada de várias respostas, a mais antiga.
    #[specta(type = specta_typescript::Number)]
    pub fetched_at_ms: u64,
    /// `true` quando a fonte não respondeu (sem internet, fora do ar) e o catálogo usou o que
    /// estava guardado, mesmo vencido. A interface mostra a data.
    pub offline: bool,
}

impl Freshness {
    /// A mais antiga das duas; `offline` se qualquer uma for.
    #[must_use]
    pub fn combine(self, other: Self) -> Self {
        Self {
            fetched_at_ms: self.fetched_at_ms.min(other.fetched_at_ms),
            offline: self.offline || other.offline,
        }
    }
}

/// Tipo de uma versão no manifesto da Mojang.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum MinecraftVersionKind {
    /// Versão final.
    Release,
    /// Snapshot, pre-release ou release candidate (fora da v1, ADR-0005).
    Snapshot,
    /// Beta antiga (2010–2011).
    OldBeta,
    /// Alfa antiga (2010).
    OldAlpha,
    /// Tipo que o Warden ainda não conhece.
    Other,
}

impl MinecraftVersionKind {
    /// O tipo pelo texto do campo `type` do manifesto; texto desconhecido vira [`Self::Other`].
    #[must_use]
    pub fn from_manifest(text: &str) -> Self {
        match text {
            "release" => Self::Release,
            "snapshot" => Self::Snapshot,
            "old_beta" => Self::OldBeta,
            "old_alpha" => Self::OldAlpha,
            _ => Self::Other,
        }
    }
}

/// Uma versão do Minecraft.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct MinecraftVersion {
    /// O id da Mojang (`1.20.1`, `26.3`, `26.4-snapshot-2`).
    pub id: String,
    /// Tipo.
    pub kind: MinecraftVersionKind,
    /// Data de lançamento, como a Mojang escreve (RFC 3339).
    pub release_time: String,
    /// Anterior a 1.7.10 pela ordem do manifesto: aparece com a etiqueta "melhor esforço"
    /// (ADR-0005).
    pub best_effort: bool,
}

/// As versões do Minecraft, na ordem oficial do manifesto da Mojang (da mais nova para a mais
/// antiga). Nunca reordene por texto ou semver: existem `26.3` e `1.21.11`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct MinecraftVersions {
    /// Todas as versões, na ordem do manifesto.
    pub versions: Vec<MinecraftVersion>,
    /// `latest.release` do manifesto.
    pub latest_release: Option<String>,
    /// `latest.snapshot` do manifesto.
    pub latest_snapshot: Option<String>,
    /// De quando é a lista.
    pub freshness: Freshness,
}

impl MinecraftVersions {
    /// Posição de uma versão no manifesto (0 é a mais nova).
    #[must_use]
    pub fn position(&self, id: &str) -> Option<usize> {
        self.versions.iter().position(|version| version.id == id)
    }

    /// A versão com esse id.
    #[must_use]
    pub fn get(&self, id: &str) -> Option<&MinecraftVersion> {
        self.versions.iter().find(|version| version.id == id)
    }

    /// Compara duas versões pela ordem oficial: `Greater` quando `a` é mais nova que `b`.
    /// `None` se alguma não estiver no manifesto.
    #[must_use]
    pub fn compare(&self, a: &str, b: &str) -> Option<Ordering> {
        Some(self.position(b)?.cmp(&self.position(a)?))
    }

    /// Só as versões finais, na mesma ordem (o que a etapa "Versão do Minecraft" mostra).
    pub fn releases(&self) -> impl Iterator<Item = &MinecraftVersion> {
        self.versions
            .iter()
            .filter(|version| version.kind == MinecraftVersionKind::Release)
    }
}

/// Uma versão de loader.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct LoaderVersion {
    /// A versão como vai no `[versions]` do `pack.toml` (`10.13.4.1614`, `47.1.106`,
    /// `21.1.252`, `0.19.5`).
    pub version: String,
    /// Coordenada Maven completa do artefato, que o launcher e o servidor usam para baixar o
    /// instalador (`net.minecraftforge:forge:1.7.10-10.13.4.1614-1.7.10`,
    /// `net.neoforged:forge:1.20.1-47.1.106`, `net.fabricmc:fabric-loader:0.19.5`).
    pub maven: String,
    /// Estável: o Fabric diz na meta; no NeoForge, sem `-beta`; no Forge, todas.
    pub stable: bool,
    /// A "recomendada" do `promotions_slim.json` (só Forge).
    pub recommended: bool,
}

/// As versões de um loader para uma versão do Minecraft. Lista vazia: o loader não existe
/// para essa versão (CA-T03-03).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct LoaderVersions {
    /// O loader.
    pub loader: Loader,
    /// A versão do Minecraft pedida.
    pub minecraft: String,
    /// Da mais nova para a mais antiga, sem as versões da lista de quebradas.
    pub versions: Vec<LoaderVersion>,
    /// A versão pré-selecionada no assistente (SPEC T03): Forge, a recomendada ou, sem ela, a
    /// mais recente; NeoForge e Fabric, a mais recente estável ou, sem estável, a mais
    /// recente.
    pub preselected: Option<String>,
    /// De quando é a lista.
    pub freshness: Freshness,
}

impl LoaderVersions {
    /// Se o loader existe para essa versão do Minecraft.
    #[must_use]
    pub fn available(&self) -> bool {
        !self.versions.is_empty()
    }

    /// A versão com esse texto.
    #[must_use]
    pub fn get(&self, version: &str) -> Option<&LoaderVersion> {
        self.versions.iter().find(|item| item.version == version)
    }
}

/// O JSON de uma versão do Minecraft, conferido pelo `sha1` do manifesto.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersionJson {
    /// A versão.
    pub id: String,
    /// O `sha1` do manifesto (o mesmo do conteúdo).
    pub sha1: String,
    /// O conteúdo, sem alteração.
    pub body: Vec<u8>,
}

impl VersionJson {
    /// O conteúdo lido como JSON.
    pub fn json(&self) -> serde_json::Result<serde_json::Value> {
        serde_json::from_slice(&self.body)
    }
}

/// Quando pedir a lista de novo à fonte.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Refresh {
    /// Usa o cache enquanto valer (6 h) e pede à fonte depois disso.
    #[default]
    IfStale,
    /// Pede à fonte agora ("Tentar de novo"); sem resposta, usa o cache.
    Always,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn version(id: &str) -> MinecraftVersion {
        MinecraftVersion {
            id: id.into(),
            kind: MinecraftVersionKind::Release,
            release_time: String::new(),
            best_effort: false,
        }
    }

    #[test]
    fn compara_pela_posicao_no_manifesto() {
        let mut snapshot = version("26.4-snapshot-1");
        snapshot.kind = MinecraftVersionKind::Snapshot;
        let versions = MinecraftVersions {
            versions: vec![
                snapshot,
                version("26.3"),
                version("1.21.11"),
                version("1.21.2"),
            ],
            latest_release: Some("26.3".into()),
            latest_snapshot: None,
            freshness: Freshness {
                fetched_at_ms: 0,
                offline: false,
            },
        };
        assert_eq!(versions.compare("26.3", "1.21.11"), Some(Ordering::Greater));
        assert_eq!(versions.compare("1.21.2", "1.21.11"), Some(Ordering::Less));
        assert_eq!(versions.compare("1.21.2", "1.21.2"), Some(Ordering::Equal));
        assert_eq!(versions.compare("1.21.2", "9"), None);
        assert_eq!(versions.position("26.3"), Some(1));
        assert!(versions.get("1.21.11").is_some());
        let releases: Vec<_> = versions.releases().map(|v| v.id.as_str()).collect();
        assert_eq!(releases, ["26.3", "1.21.11", "1.21.2"]);
    }

    #[test]
    fn data_da_lista_combinada_e_a_mais_antiga() {
        let a = Freshness {
            fetched_at_ms: 10,
            offline: false,
        };
        let b = Freshness {
            fetched_at_ms: 5,
            offline: true,
        };
        assert_eq!(
            a.combine(b),
            Freshness {
                fetched_at_ms: 5,
                offline: true
            }
        );
        assert_eq!(a.combine(a), a);
    }

    #[test]
    fn nomes_dos_loaders_e_das_fontes() {
        assert_eq!(
            serde_json::to_value([Loader::Forge, Loader::NeoForge, Loader::Fabric]).unwrap(),
            serde_json::json!(["forge", "neoforge", "fabric"])
        );
        let loader: Loader = serde_json::from_str("\"neoforge\"").unwrap();
        assert_eq!(loader, Loader::NeoForge);
        assert_eq!(Loader::NeoForge.source().to_string(), "NeoForge");
        assert_eq!(Loader::Fabric.source().to_string(), "Fabric");
        assert_eq!(Loader::Forge.source().to_string(), "Forge");
        assert_eq!(Source::Mojang.to_string(), "Mojang");
        assert_eq!(
            MinecraftVersionKind::from_manifest("pending"),
            MinecraftVersionKind::Other
        );
        assert_eq!(
            MinecraftVersionKind::from_manifest("old_alpha"),
            MinecraftVersionKind::OldAlpha
        );
        assert_eq!(
            serde_json::to_value(MinecraftVersionKind::OldBeta).unwrap(),
            "old_beta"
        );
    }

    #[test]
    fn json_da_versao() {
        let json = VersionJson {
            id: "x".into(),
            sha1: String::new(),
            body: b"{\"id\":\"x\"}".to_vec(),
        };
        assert_eq!(json.json().unwrap()["id"], "x");
        assert_eq!(Refresh::default(), Refresh::IfStale);
    }
}
