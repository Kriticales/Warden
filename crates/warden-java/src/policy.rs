//! Política "o mais novo que funciona" (ADR-0029; ARCHITECTURE §7.3).
//!
//! [`decide`] é uma função pura: recebe a versão do Minecraft, o loader, a exigência do JSON da
//! versão e a escolha do usuário, consulta a [`CompatibilityTable`] e devolve o major, o teto de
//! atualização e o **motivo** ([`JavaChoiceReason`]). [`pick_runtime`] escolhe, entre os Javas
//! instalados, o que atende à decisão (sempre a atualização mais nova).
//!
//! Ordem das regras (a primeira que se aplica):
//! - a) Java escolhido pelo usuário em Ajustes do teste ([`JavaChoiceReason::UserChoice`]);
//! - b/c) regras de loader da tabela (Forge antigo → 8; Forge 1.16.5 < 36.2.26 → 8 ≤ u312);
//! - d) o major mais novo provado para a faixa ([`JavaChoiceReason::NewestAvailable`] quando é
//!   o mais novo que o Warden baixa, senão [`JavaChoiceReason::NewestProvenForRange`]);
//! - e) sem faixa: `javaVersion.majorVersion` do JSON da versão (16 vira 17), ou 8 sem ele
//!   ([`JavaChoiceReason::FromVersionJson`]).
//!
//! As exigências de mods (`depends.java`, `[features] javaVersion`) não mudam a escolha: são
//! verificações do diagnóstico (E-JAVA).

use serde::{Deserialize, Serialize};

use crate::compat::{CompatibilityTable, LoaderKind};
use crate::error::{Error, Result};
use crate::runtime::{InstalledRuntime, RuntimeId, RuntimeSource};
use crate::version::MinecraftVersion;

/// Por que a política escolheu aquele Java. Código estável, traduzido pela interface.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, specta::Type,
)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum JavaChoiceReason {
    /// O usuário escolheu este Java em Ajustes do teste.
    UserChoice,
    /// Forge até o 1.12.2: só abre no Java 8.
    #[serde(rename = "FORGE_LEGACY_JAVA8")]
    ForgeLegacyJava8,
    /// Forge 1.16.5 anterior ao 36.2.26: Java 8 até a atualização 312.
    #[serde(rename = "FORGE_1165_OLD")]
    Forge1165Old,
    /// O mais novo provado para a faixa, que não é o mais novo que o Warden baixa.
    NewestProvenForRange,
    /// O mais novo que o Warden baixa (sem explicação extra).
    NewestAvailable,
    /// Versão sem faixa na tabela: o Java pedido pelo JSON da versão.
    FromVersionJson,
}

impl JavaChoiceReason {
    /// Se o motivo é de uma regra de loader da tabela.
    #[must_use]
    pub const fn is_loader_rule(self) -> bool {
        matches!(self, Self::ForgeLegacyJava8 | Self::Forge1165Old)
    }
}

/// O que o JSON da versão do Minecraft diz sobre o Java (`javaVersion`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum VersionJavaRequirement {
    /// O JSON da versão não foi lido (a tabela costuma bastar).
    NotLoaded,
    /// O JSON não tem `javaVersion` (versões antigas): Java 8.
    Absent,
    /// `javaVersion.majorVersion`.
    Major {
        /// O major pedido.
        major: u32,
    },
}

/// Pedido de escolha do Java para um pack.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct JavaChoiceRequest {
    /// Versão do Minecraft do `pack.toml`.
    pub minecraft: String,
    /// Loader do pack.
    pub loader: LoaderKind,
    /// Versão exata do loader, quando houver.
    pub loader_version: Option<String>,
    /// O que o JSON da versão diz sobre o Java.
    pub version_json: VersionJavaRequirement,
    /// Java escolhido pelo usuário em Ajustes do teste (`None` = Automático).
    pub user_choice: Option<RuntimeId>,
}

impl JavaChoiceRequest {
    /// Pedido automático, sem JSON da versão e sem escolha do usuário.
    #[must_use]
    pub fn automatic(minecraft: &str, loader: LoaderKind, loader_version: Option<&str>) -> Self {
        Self {
            minecraft: minecraft.to_owned(),
            loader,
            loader_version: loader_version.map(ToOwned::to_owned),
            version_json: VersionJavaRequirement::NotLoaded,
            user_choice: None,
        }
    }
}

/// O Java que o pack precisa: o major e, às vezes, um teto de atualização.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct JavaRequirement {
    /// Major.
    pub major: u32,
    /// Atualização máxima (o `312` de "8 ≤ u312").
    pub max_update: Option<u32>,
}

impl JavaRequirement {
    /// Se um Java instalado atende.
    #[must_use]
    pub fn accepts(&self, runtime: &InstalledRuntime) -> bool {
        runtime.version.major == self.major
            && self
                .max_update
                .is_none_or(|cap| runtime.version.security <= cap)
    }
}

/// Decisão da política, antes de olhar os Javas instalados.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct JavaDecision {
    /// O Java automático (o que a política escolhe sem a escolha do usuário).
    pub requirement: JavaRequirement,
    /// Motivo do Java automático.
    pub reason: JavaChoiceReason,
    /// Regra da tabela que decidiu, quando houver.
    pub rule_id: Option<String>,
    /// A faixa em texto curto ("1.17 a 1.20.4"), quando a decisão veio da tabela.
    pub range_label: Option<String>,
    /// O motivo em português (texto da tabela), para "Detalhes técnicos".
    pub explanation: String,
    /// O major mais novo que o Warden baixa ("Por que não o Java N?").
    pub newest_major: u32,
    /// O major pedido pelo JSON da versão, quando a decisão veio dele.
    pub version_json_major: Option<u32>,
}

impl JavaDecision {
    /// Se o Java automático não é o mais novo que o Warden baixa: a interface mostra
    /// "Por que não o Java N?".
    #[must_use]
    pub fn is_older_than_newest(&self) -> bool {
        self.requirement.major < self.newest_major
    }
}

/// Resultado completo de `java_choice`: o Java que o teste vai usar e por quê.
#[derive(Debug, Clone, PartialEq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct JavaChoice {
    /// Major do Java que o teste vai usar.
    pub major: u32,
    /// Teto de atualização, quando houver.
    pub max_update: Option<u32>,
    /// Motivo (`USER_CHOICE` quando o usuário escolheu e o Java existe).
    pub reason: JavaChoiceReason,
    /// O Java instalado que será usado; `None` = será baixado ao preparar o teste.
    pub runtime: Option<InstalledRuntime>,
    /// A decisão automática (sempre presente, para mostrar o motivo mesmo com escolha manual).
    pub automatic: JavaDecision,
    /// O usuário escolheu um Java que não está mais instalado; vale o automático.
    pub user_choice_unavailable: bool,
}

/// Aplica a política ao pedido (sem a escolha do usuário, que [`choose`] trata).
pub fn decide(request: &JavaChoiceRequest, table: &CompatibilityTable) -> Result<JavaDecision> {
    let newest_major = table.newest_major();
    let minecraft = MinecraftVersion::parse(&request.minecraft);
    if let Some(minecraft) = &minecraft {
        if let Some(rule) =
            table.loader_rule_for(minecraft, request.loader, request.loader_version.as_deref())
        {
            return Ok(JavaDecision {
                requirement: JavaRequirement {
                    major: rule.java,
                    max_update: rule.max_update,
                },
                reason: rule.reason,
                rule_id: Some(rule.id.clone()),
                range_label: Some(rule.label.clone()),
                explanation: rule.motivo.clone(),
                newest_major,
                version_json_major: None,
            });
        }
        if let Some(range) = table.range_for(minecraft, request.loader) {
            let major = range.newest();
            let reason = if major >= newest_major {
                JavaChoiceReason::NewestAvailable
            } else {
                JavaChoiceReason::NewestProvenForRange
            };
            return Ok(JavaDecision {
                requirement: JavaRequirement {
                    major,
                    max_update: None,
                },
                reason,
                rule_id: Some(range.id.clone()),
                range_label: Some(range.label.clone()),
                explanation: range.motivo.clone(),
                newest_major,
                version_json_major: None,
            });
        }
    }
    let (major, required, explanation) = match request.version_json {
        VersionJavaRequirement::NotLoaded => {
            return Err(Error::VersionRequirementUnknown {
                minecraft: request.minecraft.clone(),
            });
        }
        VersionJavaRequirement::Absent => (
            8,
            None,
            "O JSON desta versão do Minecraft não diz qual Java usar; versões assim são antigas e usam o Java 8.".to_owned(),
        ),
        VersionJavaRequirement::Major { major: required } => {
            // O Adoptium não publica o 16; o 17 o substitui (R2 §2.1).
            let major = table.smallest_major_at_least(required.max(8));
            (
                major,
                Some(required),
                format!(
                    "A tabela do Warden não tem esta versão do Minecraft; o JSON da versão pede o Java {required}, e o Warden usa o Java {major}."
                ),
            )
        }
    };
    Ok(JavaDecision {
        requirement: JavaRequirement {
            major,
            max_update: None,
        },
        reason: JavaChoiceReason::FromVersionJson,
        rule_id: None,
        range_label: None,
        explanation,
        newest_major,
        version_json_major: required,
    })
}

/// O Java instalado que atende ao requisito: a versão mais nova; no empate, o Temurin.
#[must_use]
pub fn pick_runtime<'a>(
    requirement: &JavaRequirement,
    installed: &'a [InstalledRuntime],
) -> Option<&'a InstalledRuntime> {
    installed
        .iter()
        .filter(|runtime| requirement.accepts(runtime))
        .max_by(|a, b| {
            a.version
                .cmp(&b.version)
                .then_with(|| source_rank(a.source).cmp(&source_rank(b.source)))
        })
}

const fn source_rank(source: RuntimeSource) -> u8 {
    match source {
        RuntimeSource::Mojang => 0,
        RuntimeSource::Temurin => 1,
    }
}

/// O Java escolhido pelo usuário: o mesmo id ou, se ele foi trocado por uma atualização e
/// removido, o mais novo da mesma fonte e do mesmo major.
fn resolve_user_choice<'a>(
    id: &RuntimeId,
    installed: &'a [InstalledRuntime],
) -> Option<&'a InstalledRuntime> {
    if let Some(runtime) = installed.iter().find(|runtime| &runtime.id == id) {
        return Some(runtime);
    }
    let (source, major) = id.source_and_major()?;
    installed
        .iter()
        .filter(|runtime| runtime.source == source && runtime.version.major == major)
        .max_by(|a, b| a.version.cmp(&b.version))
}

/// Escolha completa: aplica a política e acha o Java instalado.
pub fn choose(
    request: &JavaChoiceRequest,
    table: &CompatibilityTable,
    installed: &[InstalledRuntime],
) -> Result<JavaChoice> {
    let automatic = decide(request, table)?;
    if let Some(id) = &request.user_choice
        && let Some(runtime) = resolve_user_choice(id, installed)
    {
        return Ok(JavaChoice {
            major: runtime.version.major,
            max_update: None,
            reason: JavaChoiceReason::UserChoice,
            runtime: Some(runtime.clone()),
            automatic,
            user_choice_unavailable: false,
        });
    }
    let runtime = pick_runtime(&automatic.requirement, installed).cloned();
    Ok(JavaChoice {
        major: automatic.requirement.major,
        max_update: automatic.requirement.max_update,
        reason: automatic.reason,
        runtime,
        user_choice_unavailable: request.user_choice.is_some(),
        automatic,
    })
}
