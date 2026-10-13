//! "Dois mods alteram o mesmo ponto do jogo" (`W_MIXIN_OVERLAP`; SPEC T14, CA-T14-09; ARCHITECTURE
//! §9.6; D-10): o raio-x de mixins da `warden-mixin` sobre os jars do pack, com os dados curados
//! daqui (`data/mixin-known-compatible.toml` e os conflitos de `known-conflicts.toml`). O chamador
//! acha os jars (instância de teste e arquivos guardados no pack) e chama [`xray`] na passagem
//! completa; [`findings`] faz os avisos. O mesmo [`xray`] serve ao raio-x dos detalhes do item.
//!
//! **Só aviso**, nunca bloqueia o teste. Um aviso por grupo de mods (os itens que disputam os
//! mesmos pontos), com a evidência de cada ponto: a config de mixin e o tipo de alteração de cada
//! um e, quando subiu o risco, a linha do último travamento que citou a config. A gravidade mínima
//! para virar aviso está no dado (`risco_minimo_do_aviso`), para a decisão pendente do dono
//! (SPEC §10) ser uma linha.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::OnceLock;

use serde::Deserialize;
use warden_jarmeta::{Limits, Loader};
use warden_mixin::options::{disabled_by_pack_configs, options_data};
use warden_mixin::overlap::{CrashCitation, CrossContext, CuratedPair, Elevator, Overlap, Risk};
use warden_mixin::pack::{CacheSlot, PackJar, PackXray, scan_pack};

use super::curated;
use crate::model::{Evidence, Finding, ItemRef, RuleCode, Severity};
use crate::{DiagnosticsError, Result};

/// Código do aviso.
pub const RULE: &str = "W_MIXIN_OVERLAP";
/// Quantos pontos de um grupo entram como evidência (o resto fica no raio-x do mod).
const MAX_POINTS: usize = 5;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawPair {
    primeiro: Vec<String>,
    segundo: Vec<String>,
    origem: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawData {
    risco_minimo_do_aviso: String,
    par: Vec<RawPair>,
}

/// O conteúdo de `data/mixin-known-compatible.toml`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MixinData {
    /// Risco mínimo para a sobreposição virar aviso.
    pub min_risk: Risk,
    /// Pares conhecidos como compatíveis (um por combinação de ids).
    pub known_compatible: Vec<CuratedPair>,
}

/// Lê e valida um `mixin-known-compatible.toml`.
///
/// # Errors
///
/// TOML inválido, campo desconhecido, risco que não é `medio` nem `alto`, par com lado vazio,
/// id vazio ou sem origem.
pub fn parse_data(text: &str) -> std::result::Result<MixinData, String> {
    let raw: RawData = toml::from_str(text).map_err(|e| e.to_string())?;
    let Some(min_risk @ (Risk::Medium | Risk::High)) = Risk::parse_pt(&raw.risco_minimo_do_aviso)
    else {
        return Err(format!(
            "risco_minimo_do_aviso inválido: {}",
            raw.risco_minimo_do_aviso
        ));
    };
    let mut known_compatible = Vec::new();
    for pair in raw.par {
        let empty = |list: &[String]| list.is_empty() || list.iter().any(|id| id.trim().is_empty());
        if empty(&pair.primeiro) || empty(&pair.segundo) || pair.origem.trim().is_empty() {
            return Err(format!(
                "par incompleto: {:?} × {:?}",
                pair.primeiro, pair.segundo
            ));
        }
        for first in &pair.primeiro {
            for second in &pair.segundo {
                known_compatible.push(CuratedPair {
                    first: first.clone(),
                    second: second.clone(),
                    label: pair.origem.clone(),
                });
            }
        }
    }
    Ok(MixinData {
        min_risk,
        known_compatible,
    })
}

/// Os dados embutidos (validados pelos testes).
///
/// # Errors
///
/// [`DiagnosticsError::Internal`] se o arquivo embutido for inválido (falha de desenvolvimento).
pub fn data() -> Result<&'static MixinData> {
    static DATA: OnceLock<std::result::Result<MixinData, String>> = OnceLock::new();
    DATA.get_or_init(|| {
        parse_data(include_str!("../../data/mixin-known-compatible.toml"))
            .map_err(|e| format!("mixin-known-compatible.toml: {e}"))
    })
    .as_ref()
    .map_err(|error| DiagnosticsError::Internal(error.clone()))
}

/// O contexto do cruzamento: opções desligadas pelas configs do pack (`config_dir`), a lista de
/// compatíveis, os conflitos conhecidos e as citações do último travamento.
///
/// # Errors
///
/// Dado curado inválido (falha de desenvolvimento).
pub fn cross_context(config_dir: Option<&Path>, crash: Vec<CrashCitation>) -> Result<CrossContext> {
    let data = data()?;
    let conflicts = curated::get()?
        .conflicts
        .iter()
        .map(|c| CuratedPair {
            first: c.first.clone(),
            second: c.second.clone(),
            label: c.id.clone(),
        })
        .collect();
    Ok(CrossContext {
        disabled: config_dir
            .map(|dir| disabled_by_pack_configs(dir, options_data()))
            .unwrap_or_default(),
        known_compatible: data.known_compatible.clone(),
        known_conflicts: conflicts,
        crash,
    })
}

/// Raio-x dos jars dados (leitura com o cache do pack e cruzamento).
///
/// # Errors
///
/// Dado curado inválido (falha de desenvolvimento). Jar ilegível não é erro.
pub fn xray(
    jars: &[PackJar],
    loader: Loader,
    config_dir: Option<&Path>,
    cache: Option<CacheSlot<'_>>,
    crash: Vec<CrashCitation>,
) -> Result<PackXray> {
    let context = cross_context(config_dir, crash)?;
    let scan = scan_pack(jars, loader, cache, &Limits::default());
    tracing::debug!(stats = ?scan.stats, "raio-x de mixins lido");
    Ok(PackXray::build(scan, &context))
}

#[allow(clippy::expect_used)] // Literal deste módulo, conferido pelos testes.
fn rule_code() -> RuleCode {
    RuleCode::parse(RULE).expect("código fixo de regra")
}

/// Os avisos: um por grupo de itens que disputam pontos com risco a partir do mínimo do dado.
///
/// # Errors
///
/// Dado curado inválido (falha de desenvolvimento).
pub fn findings(overlaps: &[Overlap]) -> Result<Vec<Finding>> {
    let min = data()?.min_risk;
    let mut groups: BTreeMap<Vec<&str>, Vec<&Overlap>> = BTreeMap::new();
    for overlap in overlaps.iter().filter(|o| o.risk >= min) {
        let mut items: Vec<&str> = overlap
            .participants
            .iter()
            .map(|p| p.item.as_str())
            .collect();
        items.sort_unstable();
        items.dedup();
        if items.len() >= 2 {
            groups.entry(items).or_default().push(overlap);
        }
    }
    let mut out = Vec::new();
    for (items, mut points) in groups {
        points.sort_by(|a, b| b.risk.cmp(&a.risk).then_with(|| a.label().cmp(&b.label())));
        let Some(first) = points.first() else {
            continue;
        };
        let risk = first.risk;
        let mut evidence = Vec::new();
        for point in points.iter().take(MAX_POINTS) {
            for participant in &point.participants {
                evidence.push(Evidence::JarMetadata {
                    item: ItemRef::Metafile {
                        path: participant.item.clone(),
                    },
                    file_in_jar: participant.config.clone(),
                    excerpt: format!(
                        "@{} em {} ({}{}), prioridade {}",
                        participant.kind.annotation(),
                        point.label(),
                        participant.at.value,
                        if participant.at.target.is_empty() {
                            String::new()
                        } else {
                            format!(" {}", participant.at.target)
                        },
                        participant.priority
                    ),
                });
            }
            for elevator in &point.elevators {
                match elevator {
                    Elevator::Crash(citation) => {
                        evidence.push(Evidence::log(
                            &citation.file,
                            citation.line,
                            &citation.excerpt,
                        ));
                    }
                    Elevator::KnownConflict { id } => {
                        if let Some(conflict) =
                            curated::get()?.conflicts.iter().find(|c| &c.id == id)
                        {
                            evidence.push(Evidence::Curated {
                                rule_id: conflict.id.clone(),
                                note: conflict.note.clone(),
                            });
                        }
                    }
                    Elevator::Require { .. } => {}
                }
            }
        }
        let mut evidence = evidence.into_iter();
        let Some(primary) = evidence.next() else {
            continue;
        };
        let mut finding = Finding::new(
            rule_code(),
            Severity::Warning,
            format!("diagnostics.pretest.{}", RULE.to_ascii_lowercase()),
            primary,
        )
        .with_param("ponto", first.label())
        .with_param("mais", (points.len() - 1).to_string())
        .with_param("risco", if risk == Risk::High { "alto" } else { "medio" });
        let crashed = points
            .iter()
            .any(|p| p.elevators.iter().any(|e| matches!(e, Elevator::Crash(_))));
        finding = finding.with_param("travamento", if crashed { "sim" } else { "nao" });
        for item in items {
            finding = finding.with_item(ItemRef::Metafile {
                path: item.to_owned(),
            });
        }
        for more in evidence {
            finding = finding.with_evidence(more);
        }
        out.push(finding);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use warden_mixin::model::{At, ChangeKind};
    use warden_mixin::overlap::{Participant, Rule};

    use super::*;

    #[test]
    fn dados_embutidos_validos() {
        let data = data().unwrap();
        assert_eq!(
            data.min_risk,
            Risk::Medium,
            "decisão pendente do dono: vale médio (SPEC §10)"
        );
        let has = |a: &str, b: &str| {
            data.known_compatible
                .iter()
                .any(|p| p.first == a && p.second == b && !p.label.is_empty())
        };
        for (a, b) in [
            ("lithium", "redirected"),
            ("radium", "redirected"),
            ("canary", "redirected"),
            ("modernfix", "redirected"),
            ("modernfix", "noisium"),
            ("stfu", "charm"),
        ] {
            assert!(has(a, b), "{a} × {b}");
        }
    }

    #[test]
    fn esquema_recusa_o_que_nao_serve() {
        for bad in [
            "risco_minimo_do_aviso = \"baixo\"\npar = []",
            "risco_minimo_do_aviso = \"x\"\npar = []",
            "par = []",
            "risco_minimo_do_aviso = \"alto\"\npar = []\noutro = 1",
            "risco_minimo_do_aviso = \"alto\"\n[[par]]\nprimeiro = []\nsegundo = [\"b\"]\norigem = \"x\"",
            "risco_minimo_do_aviso = \"alto\"\n[[par]]\nprimeiro = [\"a\"]\nsegundo = [\" \"]\norigem = \"x\"",
            "risco_minimo_do_aviso = \"alto\"\n[[par]]\nprimeiro = [\"a\"]\nsegundo = [\"b\"]\norigem = \"\"",
        ] {
            assert!(parse_data(bad).is_err(), "{bad}");
        }
        let ok = parse_data("risco_minimo_do_aviso = \"alto\"\n[[par]]\nprimeiro = [\"a\", \"c\"]\nsegundo = [\"b\"]\norigem = \"x\"").unwrap();
        assert_eq!(ok.min_risk, Risk::High);
        assert_eq!(ok.known_compatible.len(), 2);
    }

    fn participant(item: &str, config: &str) -> Participant {
        Participant {
            item: item.into(),
            mod_id: item
                .trim_start_matches("mods/")
                .trim_end_matches(".pw.toml")
                .into(),
            kind: ChangeKind::Redirect,
            mixin_class: format!("{item}/Mixin"),
            config: config.into(),
            priority: 1000,
            at: At {
                value: "INVOKE".into(),
                target: "La/B;c()V".into(),
            },
            plugin: false,
        }
    }

    fn overlap(
        method: &str,
        risk: Risk,
        items: &[(&str, &str)],
        elevators: Vec<Elevator>,
    ) -> Overlap {
        Overlap {
            target_class: "net/minecraft/client/renderer/LevelRenderer".into(),
            method: method.into(),
            class_named: String::new(),
            method_named: String::new(),
            items: items.iter().map(|(i, _)| (*i).to_owned()).collect(),
            rule: Rule::TwoReplacements,
            base: Risk::High,
            risk,
            participants: items.iter().map(|(i, c)| participant(i, c)).collect(),
            demoters: vec![],
            elevators,
        }
    }

    #[test]
    fn um_aviso_por_grupo_com_a_evidencia() {
        let a = ("mods/a.pw.toml", "a.mixins.json");
        let b = ("mods/b.pw.toml", "b.mixins.json");
        let c = ("mods/c.pw.toml", "c.mixins.json");
        let citation = CrashCitation {
            config: "a.mixins.json".into(),
            file: "output.log".into(),
            line: 42,
            excerpt: "Mixin apply failed a.mixins.json:X".into(),
        };
        let overlaps = vec![
            overlap(
                "render",
                Risk::High,
                &[a, b],
                vec![Elevator::Crash(citation)],
            ),
            overlap("tick", Risk::Medium, &[b, a], vec![]),
            overlap("baixo", Risk::Low, &[a, c], vec![]),
            overlap(
                "conflito",
                Risk::High,
                &[a, c],
                vec![Elevator::KnownConflict {
                    id: "optifine-sodium".into(),
                }],
            ),
        ];
        let found = findings(&overlaps).unwrap();
        assert_eq!(
            found.len(),
            2,
            "a+b num aviso só; a+c pelo conflito; o baixo não conta"
        );
        let ab = &found[0];
        assert_eq!(ab.rule().as_str(), RULE);
        assert_eq!(ab.severity(), Severity::Warning, "nunca bloqueia");
        assert_eq!(ab.param("ponto"), Some("LevelRenderer.render"));
        assert_eq!(ab.param("mais"), Some("1"));
        assert_eq!(ab.param("risco"), Some("alto"));
        assert_eq!(ab.param("travamento"), Some("sim"));
        assert_eq!(found[1].param("travamento"), Some("nao"));
        assert_eq!(found[1].param("mais"), Some("0"));
        assert_eq!(ab.items().len(), 2);
        assert!(ab.evidence().iter().any(|e| matches!(e,
            Evidence::JarMetadata { file_in_jar, excerpt, .. }
                if file_in_jar == "a.mixins.json" && excerpt.starts_with("@Redirect em LevelRenderer.render (INVOKE La/B;c()V)"))));
        assert!(ab.evidence().iter().any(|e| matches!(e,
            Evidence::Log { file, line: 42, excerpt } if file == "output.log" && excerpt.contains("a.mixins.json"))));
        assert!(found[1].evidence().iter().any(
            |e| matches!(e, Evidence::Curated { rule_id, .. } if rule_id == "optifine-sodium")
        ));
    }

    #[test]
    fn so_medio_ou_alto_conforme_o_dado() {
        let a = ("mods/a.pw.toml", "a.mixins.json");
        let b = ("mods/b.pw.toml", "b.mixins.json");
        assert!(
            findings(&[overlap("x", Risk::Low, &[a, b], vec![])])
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            findings(&[overlap("x", Risk::Medium, &[a, b], vec![])])
                .unwrap()
                .len(),
            1
        );
    }
}
