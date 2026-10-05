//! Sugestão de versão `SemVer` (SPEC T16), como função pura.
//!
//! - **MAIOR:** mudou a versão do Minecraft ou o loader; removeu mod que não é "só cliente"
//!   (pode apagar blocos ou itens de mundos existentes); adicionou ou removeu mod de geração de
//!   mundo (categoria `worldgen` no Modrinth).
//! - **MENOR:** adicionou mods, resource packs ou shaders.
//! - **CORREÇÃO:** só atualizações de itens e mudanças de configs (e outros ajustes).
//! - **Primeira versão salva:** a versão atual do `pack.toml` (`0.1.0` se ela não for `SemVer`).
//!
//! O incremento segue o do `npm version`: de uma pré-versão (`2.0.0-beta.1`), a correção e a
//! maior que "caberiam" nela só tiram o sufixo (`2.0.0`). A base é a maior versão já salva,
//! para a sugestão sempre passar na validação de "maior que a última" (CA-T16-04).

use semver::{Prerelease, Version};
use serde::{Deserialize, Serialize};

use crate::changes::{ChangeSet, ItemChange, ItemChangeKind};

/// Tamanho do incremento.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, specta::Type,
)]
#[serde(rename_all = "camelCase")]
pub enum Bump {
    /// CORREÇÃO (`x.y.Z`).
    Patch,
    /// MENOR (`x.Y.0`).
    Minor,
    /// MAIOR (`X.0.0`).
    Major,
}

/// Por que a versão foi sugerida (a interface monta a frase).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum SuggestReason {
    /// Primeira versão salva: a versão do `pack.toml`.
    #[serde(rename_all = "camelCase")]
    FirstVersion {
        /// Versão lida do `pack.toml` (vazia se não houver).
        pack_version: String,
        /// Se ela era `SemVer`; senão a sugestão é `0.1.0`.
        valid: bool,
    },
    /// Mudou a versão do Minecraft.
    MinecraftChanged,
    /// Mudou um loader (ou a versão dele).
    LoaderChanged,
    /// Removeu mods que não são "só cliente".
    #[serde(rename_all = "camelCase")]
    RemovedWorldMods {
        /// Quantos.
        count: u32,
    },
    /// Adicionou ou removeu mods de geração de mundo.
    #[serde(rename_all = "camelCase")]
    WorldgenChanged {
        /// Quantos.
        count: u32,
    },
    /// Adicionou mods, resource packs ou shaders.
    #[serde(rename_all = "camelCase")]
    AddedItems {
        /// Quantos.
        count: u32,
    },
    /// Atualizou itens.
    #[serde(rename_all = "camelCase")]
    UpdatedItems {
        /// Quantos.
        count: u32,
    },
    /// Mudou configs.
    #[serde(rename_all = "camelCase")]
    ConfigsChanged {
        /// Quantas.
        count: u32,
    },
    /// Outras mudanças (ajustes de itens, remoção de itens só cliente, arquivos de controle).
    OtherChanges,
}

/// Versão sugerida e o motivo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct VersionSuggestion {
    /// Versão sugerida (`1.3.0`).
    pub version: String,
    /// Incremento aplicado (`None` na primeira versão).
    pub bump: Option<Bump>,
    /// Motivos, do mais forte para o mais fraco.
    pub reasons: Vec<SuggestReason>,
}

/// Fatos de uma mudança que decidem a sugestão.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ChangeFacts {
    /// Mudou a versão do Minecraft.
    pub minecraft_changed: bool,
    /// Mudou algum loader.
    pub loader_changed: bool,
    /// Mods removidos que não são "só cliente".
    pub removed_world_mods: u32,
    /// Mods de geração de mundo adicionados ou removidos.
    pub worldgen_changed: u32,
    /// Mods, resource packs e shaders adicionados.
    pub added_items: u32,
    /// Itens atualizados.
    pub updated_items: u32,
    /// Configs alteradas.
    pub configs_changed: u32,
    /// Alguma outra mudança (para não sugerir nada só quando nada mudou).
    pub any_change: bool,
}

impl ChangeFacts {
    /// Fatos de um [`ChangeSet`]. `is_worldgen` diz se um item é de geração de mundo (a
    /// `warden-app` responde com as categorias do Modrinth).
    pub fn from_changes(changes: &ChangeSet, is_worldgen: impl Fn(&ItemChange) -> bool) -> Self {
        let count = |iter: &mut dyn Iterator<Item = &ItemChange>| {
            u32::try_from(iter.count()).unwrap_or(u32::MAX)
        };
        Self {
            minecraft_changed: changes.minecraft.is_some(),
            loader_changed: !changes.loaders.is_empty(),
            removed_world_mods: count(&mut changes.removed_world_mods()),
            worldgen_changed: count(&mut changes.items.iter().filter(|item| {
                matches!(item.kind, ItemChangeKind::Added | ItemChangeKind::Removed)
                    && is_worldgen(item)
            })),
            added_items: count(&mut changes.items_of(ItemChangeKind::Added)),
            updated_items: count(&mut changes.items_of(ItemChangeKind::Updated)),
            configs_changed: u32::try_from(changes.configs.len()).unwrap_or(u32::MAX),
            any_change: !changes.is_empty(),
        }
    }
}

/// Versão do `pack.toml` lida com tolerância: `v1.2.3` e `1.2` contam.
fn lenient_version(text: &str) -> Option<Version> {
    let text = text.trim();
    let text = text.strip_prefix(['v', 'V']).unwrap_or(text);
    if let Ok(version) = Version::parse(text) {
        return Some(version);
    }
    let parts: Vec<&str> = text.split('.').collect();
    let numbers: Option<Vec<u64>> = parts.iter().map(|part| part.parse().ok()).collect();
    match numbers?.as_slice() {
        [major] => Some(Version::new(*major, 0, 0)),
        [major, minor] => Some(Version::new(*major, *minor, 0)),
        _ => None,
    }
}

/// Aplica o incremento a uma versão (regras do `npm version`).
#[must_use]
pub fn bump(version: &Version, bump: Bump) -> Version {
    let pre = !version.pre.is_empty();
    let (major, minor, patch) = (version.major, version.minor, version.patch);
    match bump {
        Bump::Major if pre && minor == 0 && patch == 0 => Version::new(major, 0, 0),
        Bump::Major => Version::new(major.saturating_add(1), 0, 0),
        Bump::Minor if pre && patch == 0 => Version::new(major, minor, 0),
        Bump::Minor => Version::new(major, minor.saturating_add(1), 0),
        Bump::Patch if pre => Version {
            pre: Prerelease::EMPTY,
            build: semver::BuildMetadata::EMPTY,
            ..version.clone()
        },
        Bump::Patch => Version::new(major, minor, patch.saturating_add(1)),
    }
}

/// Sugere a próxima versão.
///
/// `highest` é a maior versão já salva (`None` antes da primeira); `pack_version` é o campo
/// `version` do `pack.toml`. Devolve `None` quando nada mudou.
#[must_use]
pub fn suggest_version(
    highest: Option<&Version>,
    pack_version: &str,
    facts: &ChangeFacts,
) -> Option<VersionSuggestion> {
    if !facts.any_change {
        return None;
    }
    let Some(highest) = highest else {
        let parsed = lenient_version(pack_version);
        return Some(VersionSuggestion {
            version: parsed
                .clone()
                .unwrap_or_else(|| Version::new(0, 1, 0))
                .to_string(),
            bump: None,
            reasons: vec![SuggestReason::FirstVersion {
                pack_version: pack_version.trim().to_owned(),
                valid: parsed.is_some(),
            }],
        });
    };

    let mut reasons = Vec::new();
    if facts.minecraft_changed {
        reasons.push(SuggestReason::MinecraftChanged);
    }
    if facts.loader_changed {
        reasons.push(SuggestReason::LoaderChanged);
    }
    if facts.removed_world_mods > 0 {
        reasons.push(SuggestReason::RemovedWorldMods {
            count: facts.removed_world_mods,
        });
    }
    if facts.worldgen_changed > 0 {
        reasons.push(SuggestReason::WorldgenChanged {
            count: facts.worldgen_changed,
        });
    }
    let level = if reasons.is_empty() {
        if facts.added_items > 0 {
            Bump::Minor
        } else {
            Bump::Patch
        }
    } else {
        Bump::Major
    };
    if facts.added_items > 0 {
        reasons.push(SuggestReason::AddedItems {
            count: facts.added_items,
        });
    }
    if facts.updated_items > 0 {
        reasons.push(SuggestReason::UpdatedItems {
            count: facts.updated_items,
        });
    }
    if facts.configs_changed > 0 {
        reasons.push(SuggestReason::ConfigsChanged {
            count: facts.configs_changed,
        });
    }
    if reasons.is_empty() {
        reasons.push(SuggestReason::OtherChanges);
    }
    Some(VersionSuggestion {
        version: bump(highest, level).to_string(),
        bump: Some(level),
        reasons,
    })
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    fn v(text: &str) -> Version {
        Version::parse(text).unwrap()
    }

    fn facts() -> ChangeFacts {
        ChangeFacts {
            any_change: true,
            ..ChangeFacts::default()
        }
    }

    /// Tabela de casos da SPEC T16: (descrição, maior versão salva, fatos, sugestão, incremento).
    #[test]
    #[allow(clippy::too_many_lines)] // a tabela de casos é o teste
    fn tabela_da_spec_t16() {
        let cases: Vec<(&str, &str, ChangeFacts, &str, Bump)> = vec![
            (
                "mudou o Minecraft",
                "1.2.3",
                ChangeFacts {
                    minecraft_changed: true,
                    ..facts()
                },
                "2.0.0",
                Bump::Major,
            ),
            (
                "mudou o loader",
                "1.2.3",
                ChangeFacts {
                    loader_changed: true,
                    updated_items: 4,
                    ..facts()
                },
                "2.0.0",
                Bump::Major,
            ),
            (
                "removeu mod cliente e servidor",
                "1.2.3",
                ChangeFacts {
                    removed_world_mods: 1,
                    ..facts()
                },
                "2.0.0",
                Bump::Major,
            ),
            (
                "adicionou mod de geração de mundo",
                "1.2.3",
                ChangeFacts {
                    worldgen_changed: 1,
                    added_items: 1,
                    ..facts()
                },
                "2.0.0",
                Bump::Major,
            ),
            (
                "adicionou mods (CA-T16-01: 2 adicionados e 1 atualizado)",
                "1.2.3",
                ChangeFacts {
                    added_items: 2,
                    updated_items: 1,
                    ..facts()
                },
                "1.3.0",
                Bump::Minor,
            ),
            (
                "adicionou resource pack",
                "0.4.1",
                ChangeFacts {
                    added_items: 1,
                    ..facts()
                },
                "0.5.0",
                Bump::Minor,
            ),
            (
                "só atualizações",
                "1.2.3",
                ChangeFacts {
                    updated_items: 3,
                    ..facts()
                },
                "1.2.4",
                Bump::Patch,
            ),
            (
                "só configs",
                "1.2.3",
                ChangeFacts {
                    configs_changed: 2,
                    ..facts()
                },
                "1.2.4",
                Bump::Patch,
            ),
            (
                "removeu só mod cliente",
                "1.2.3",
                facts(),
                "1.2.4",
                Bump::Patch,
            ),
            (
                "pré-versão: correção tira o sufixo",
                "2.0.0-beta.1",
                ChangeFacts {
                    updated_items: 1,
                    ..facts()
                },
                "2.0.0",
                Bump::Patch,
            ),
            (
                "pré-versão: maior em x.0.0 tira o sufixo",
                "2.0.0-rc.1",
                ChangeFacts {
                    minecraft_changed: true,
                    ..facts()
                },
                "2.0.0",
                Bump::Major,
            ),
            (
                "pré-versão: maior em x.1.0 sobe",
                "2.1.0-rc.1",
                ChangeFacts {
                    minecraft_changed: true,
                    ..facts()
                },
                "3.0.0",
                Bump::Major,
            ),
            (
                "pré-versão: menor em x.y.0 tira o sufixo",
                "2.1.0-rc.1",
                ChangeFacts {
                    added_items: 1,
                    ..facts()
                },
                "2.1.0",
                Bump::Minor,
            ),
            (
                "pré-versão: menor em x.y.1 sobe",
                "2.1.1-rc.1",
                ChangeFacts {
                    added_items: 1,
                    ..facts()
                },
                "2.2.0",
                Bump::Minor,
            ),
        ];
        for (description, highest, facts, expected, level) in cases {
            let suggestion = suggest_version(Some(&v(highest)), "9.9.9", &facts).unwrap();
            assert_eq!(suggestion.version, expected, "{description}");
            assert_eq!(suggestion.bump, Some(level), "{description}");
            assert!(!suggestion.reasons.is_empty(), "{description}");
        }
    }

    #[test]
    fn motivos_em_ordem_do_mais_forte() {
        let suggestion = suggest_version(
            Some(&v("1.0.0")),
            "",
            &ChangeFacts {
                minecraft_changed: true,
                removed_world_mods: 2,
                added_items: 1,
                configs_changed: 3,
                ..facts()
            },
        )
        .unwrap();
        assert_eq!(
            suggestion.reasons,
            vec![
                SuggestReason::MinecraftChanged,
                SuggestReason::RemovedWorldMods { count: 2 },
                SuggestReason::AddedItems { count: 1 },
                SuggestReason::ConfigsChanged { count: 3 },
            ]
        );
        let other = suggest_version(Some(&v("1.0.0")), "", &facts()).unwrap();
        assert_eq!(other.reasons, vec![SuggestReason::OtherChanges]);
    }

    #[test]
    fn primeira_versao_usa_o_pack_toml() {
        let cases = [
            ("0.1.0", "0.1.0", true),
            ("1.0", "1.0.0", true),
            ("v2.3.4", "2.3.4", true),
            ("3", "3.0.0", true),
            ("1.0.0-alpha", "1.0.0-alpha", true),
            ("", "0.1.0", false),
            ("beta", "0.1.0", false),
            ("1.2.3.4", "0.1.0", false),
        ];
        for (pack, expected, valid) in cases {
            let suggestion = suggest_version(None, pack, &facts()).unwrap();
            assert_eq!(suggestion.version, expected, "{pack:?}");
            assert_eq!(suggestion.bump, None);
            assert_eq!(
                suggestion.reasons,
                vec![SuggestReason::FirstVersion {
                    pack_version: pack.to_owned(),
                    valid
                }]
            );
        }
    }

    #[test]
    fn nada_mudou_nao_sugere() {
        assert_eq!(
            suggest_version(Some(&v("1.0.0")), "1.0.0", &ChangeFacts::default()),
            None
        );
        assert_eq!(
            suggest_version(None, "1.0.0", &ChangeFacts::default()),
            None
        );
    }

    proptest! {
        /// A sugestão é sempre maior que a maior versão salva.
        #[test]
        fn sugestao_sempre_maior(
            major in 0u64..50, minor in 0u64..50, patch in 0u64..50,
            pre in proptest::option::of("[a-z]{1,5}(\\.[1-9][0-9]?)?"),
            level in 0u8..3,
        ) {
            let mut highest = Version::new(major, minor, patch);
            if let Some(pre) = pre {
                highest.pre = Prerelease::new(&pre).unwrap();
            }
            let facts = ChangeFacts {
                minecraft_changed: level == 2,
                added_items: u32::from(level == 1),
                ..facts()
            };
            let suggestion = suggest_version(Some(&highest), "", &facts).unwrap();
            prop_assert!(v(&suggestion.version) > highest, "{} -> {}", highest, suggestion.version);
        }
    }
}
