//! Cruzamento: as alterações de mods diferentes no mesmo método do jogo, classificadas pelas
//! regras da R5A §5.5 com os refinamentos do S-R5-1 §6 (ARCHITECTURE §9.6).
//!
//! **Ponto de aplicação** = classe-alvo + método-alvo; dentro dele, o `@At` (tipo + alvo) e, no
//! `@ModifyVariable`/`@ModifyArg`, o seletor. Regras, da mais forte para a mais fraca:
//!
//! | Regra | Risco base |
//! |---|---|
//! | `@Overwrite` de 2+ mods | alto |
//! | `@Redirect`/`@ModifyConstant` de 2+ mods no mesmo `@At` | alto |
//! | `@Overwrite` + injeção no corpo de outro mod | médio |
//! | `@Redirect` + outro injetor (não `@Inject`) no mesmo `@At` | médio |
//! | `@ModifyVariable`/`@ModifyArg(s)` de 2+ mods no mesmo valor | médio |
//! | `@Redirect` + `@Inject` no mesmo `@At` (o `@Inject` acha o ponto antes) | baixo |
//! | o resto (`@Inject` e `MixinExtras` que se encadeiam, `@Overwrite` + começo/fim) | baixo |
//!
//! **Rebaixadores** (cada um desce um nível, com o motivo): todos os envolvidos com `plugin`;
//! compatibilidade intencional (config ou pacote `compat`, classe que referencia o outro mod,
//! dependência declarada que não é biblioteca); o plugin ou as classes de config citam o outro
//! mod; o par está na lista curada. **Elevadores**: o par está em `known-conflicts.toml` ou o
//! último travamento citou uma das configs (sobe para alto). `require`/`defaultRequire` ≥ 1 numa
//! injeção que o outro mod pode tirar fica como evidência, sem subir o risco (medido na D-10:
//! subir um nível dava 13 altos falsos no pack Fabric do S-R5-1).

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

use crate::model::{At, Change, ChangeKind, MixinClass, ScannedMod};
use crate::options::is_disabled;

/// Risco de uma sobreposição.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, specta::Type,
)]
#[serde(rename_all = "camelCase")]
pub enum Risk {
    /// Convivem (ou foram feitos para conviver).
    Low,
    /// Pode dar diferença, dependendo da ordem.
    Medium,
    /// Só um vale; o outro é pulado em silêncio.
    High,
}

impl Risk {
    fn down(self) -> Self {
        match self {
            Self::High => Self::Medium,
            Self::Medium | Self::Low => Self::Low,
        }
    }

    /// Lê `"baixo"`, `"medio"` ou `"alto"` (os dados curados).
    #[must_use]
    pub fn parse_pt(text: &str) -> Option<Self> {
        match text {
            "baixo" => Some(Self::Low),
            "medio" => Some(Self::Medium),
            "alto" => Some(Self::High),
            _ => None,
        }
    }
}

/// A regra que classificou a sobreposição.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum Rule {
    /// `@Overwrite` de 2+ mods.
    TwoOverwrites,
    /// `@Redirect`/`@ModifyConstant` de 2+ mods no mesmo `@At`.
    TwoReplacements,
    /// `@Overwrite` + injeção no corpo de outro mod.
    OverwriteAndBody,
    /// `@Redirect` + outro injetor no mesmo `@At`.
    RedirectAndOther,
    /// `@ModifyVariable`/`@ModifyArg(s)` de 2+ mods no mesmo valor.
    TwoValueChanges,
    /// `@Redirect` + `@Inject` no mesmo `@At`.
    RedirectAndInject,
    /// `@Overwrite` + injeção no começo ou no fim.
    OverwriteAndEdge,
    /// Só injeções que coexistem.
    Coexist,
}

/// Um mod envolvido numa sobreposição.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct Participant {
    /// Item do pack (caminho do metafile ou do arquivo).
    pub item: String,
    /// Id do mod que traz a classe (pode ser um embutido do item).
    pub mod_id: String,
    /// Tipo de alteração.
    pub kind: ChangeKind,
    /// Classe de mixin.
    pub mixin_class: String,
    /// Config de mixin.
    pub config: String,
    /// Prioridade (menor aplica antes).
    pub priority: i32,
    /// O ponto.
    pub at: At,
    /// A config tem plugin.
    pub plugin: bool,
}

/// Por que o risco desceu.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Demoter {
    /// Todos os mixins envolvidos têm plugin: o próprio mod pode desligá-los.
    AllPlugin,
    /// Compatibilidade intencional: a config ou o pacote do mixin é de compatibilidade.
    CompatPackage {
        /// Item cujo mixin é de compatibilidade.
        item: String,
        /// Classe de mixin.
        mixin_class: String,
    },
    /// Compatibilidade intencional: o mixin de um mod usa classes do outro.
    ReferencesOther {
        /// Item que referencia.
        item: String,
        /// O outro item.
        other: String,
    },
    /// Compatibilidade intencional: um mod declara o outro como dependência.
    DeclaresDependency {
        /// Item que declara.
        item: String,
        /// O outro item.
        other: String,
    },
    /// O plugin ou a config do mod cita o outro mod.
    PluginCitesOther {
        /// Item que cita.
        item: String,
        /// O outro item.
        other: String,
    },
    /// O par está na lista curada de compatíveis.
    KnownCompatible {
        /// De onde veio a informação.
        origin: String,
    },
}

/// Por que o risco subiu.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Elevator {
    /// O par está na lista de conflitos conhecidos.
    KnownConflict {
        /// Id da entrada.
        id: String,
    },
    /// O último travamento citou uma das configs envolvidas.
    Crash(CrashCitation),
    /// Injeção com `require` ≥ 1 num ponto que o outro mod pode tirar: se sumir, o jogo trava.
    /// Só evidência; não muda o risco.
    Require {
        /// Config da injeção.
        config: String,
    },
}

/// Uma linha de log que cita uma config de mixin.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct CrashCitation {
    /// A config citada.
    pub config: String,
    /// Arquivo do log (relativo à sessão).
    pub file: String,
    /// Linha (a partir de 1).
    pub line: u32,
    /// O trecho.
    pub excerpt: String,
}

/// Uma sobreposição: dois ou mais mods no mesmo método do jogo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct Overlap {
    /// Classe alterada (nome interno, em tempo de execução).
    pub target_class: String,
    /// Método alterado (em tempo de execução).
    pub method: String,
    /// Classe com nome legível quando algum refmap ensinou (`net/minecraft/client/render/X`).
    pub class_named: String,
    /// Método com nome legível quando conhecido.
    pub method_named: String,
    /// Itens que alteram o método (todos, não só os envolvidos na regra).
    pub items: Vec<String>,
    /// Regra.
    pub rule: Rule,
    /// Risco pela regra.
    pub base: Risk,
    /// Risco depois de rebaixadores e elevadores.
    pub risk: Risk,
    /// Os envolvidos na regra (os que disputam o mesmo ponto).
    pub participants: Vec<Participant>,
    /// Rebaixadores.
    pub demoters: Vec<Demoter>,
    /// Elevadores.
    pub elevators: Vec<Elevator>,
}

impl Overlap {
    /// A parte do jogo em poucas palavras: classe sem pacote e método, com os nomes legíveis
    /// quando conhecidos (`SodiumGameOptionPages.general`, `LevelRenderer.m_202423_`).
    #[must_use]
    pub fn label(&self) -> String {
        point_label(
            &self.target_class,
            &self.class_named,
            &self.method,
            &self.method_named,
        )
    }
}

/// Classe sem pacote + `.` + método, preferindo os nomes legíveis.
#[must_use]
pub fn point_label(class: &str, class_named: &str, method: &str, method_named: &str) -> String {
    let class = if class_named.is_empty() {
        class
    } else {
        class_named
    };
    let simple = class.rsplit('/').next().unwrap_or(class);
    let method = if method_named.is_empty() {
        method
    } else {
        method_named
    };
    format!("{simple}.{method}")
}

/// Par curado (lista de compatíveis ou de conflitos).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CuratedPair {
    /// Id do primeiro mod.
    pub first: String,
    /// Id do segundo.
    pub second: String,
    /// Id da entrada (conflito) ou origem (compatível).
    pub label: String,
}

/// O que vale para o pack todo no cruzamento.
#[derive(Debug, Clone, Default)]
pub struct CrossContext {
    /// Opções desligadas pelas configs do pack ([`crate::options::disabled_by_pack_configs`]).
    pub disabled: BTreeSet<String>,
    /// Pares conhecidos como compatíveis.
    pub known_compatible: Vec<CuratedPair>,
    /// Pares em conflito conhecido.
    pub known_conflicts: Vec<CuratedPair>,
    /// Linhas do último travamento que citam configs de mixin.
    pub crash: Vec<CrashCitation>,
}

/// Os jars de um item do pack, lidos.
#[derive(Debug, Clone, Copy)]
pub struct ItemScan<'a> {
    /// Item do pack.
    pub item: &'a str,
    /// Mods do jar (o de cima primeiro).
    pub mods: &'a [ScannedMod],
}

/// Um mod escolhido para o cruzamento.
#[derive(Debug, Clone, Copy)]
pub struct Kept<'a> {
    /// Item dono.
    pub item: &'a str,
    /// O mod.
    pub module: &'a ScannedMod,
}

/// Melhor cópia de um id: (item, posição no item, (versão, embutidos do jar de cima)).
type Best = (usize, usize, (Vec<u64>, u32));

/// Os mods que o loader carrega: os jars de cima sempre; entre cópias embutidas do mesmo id,
/// a de versão maior e, no empate, a do jar que traz mais embutidos (o agregador, como a Fabric
/// API). Embutido com o mesmo id de um jar de cima fica de fora.
#[must_use]
pub fn kept_mods<'a>(items: &[ItemScan<'a>]) -> Vec<Kept<'a>> {
    let top_ids: HashSet<&str> = items
        .iter()
        .filter_map(|i| i.mods.first())
        .map(|m| m.mod_id.as_str())
        .collect();
    let mut best: HashMap<&str, Best> = HashMap::new();
    for (item_index, item) in items.iter().enumerate() {
        let aggregator = item.mods.first().map_or(0, |m| m.nested_count);
        for (mod_index, module) in item.mods.iter().enumerate().skip(1) {
            if top_ids.contains(module.mod_id.as_str()) {
                continue;
            }
            let score = (version_key(&module.version), aggregator);
            match best.get(module.mod_id.as_str()) {
                Some((_, _, current)) if *current >= score => {}
                _ => {
                    best.insert(module.mod_id.as_str(), (item_index, mod_index, score));
                }
            }
        }
    }
    let mut out = Vec::new();
    for (item_index, item) in items.iter().enumerate() {
        for (mod_index, module) in item.mods.iter().enumerate() {
            let keep = mod_index == 0
                || best
                    .get(module.mod_id.as_str())
                    .is_some_and(|(i, m, _)| *i == item_index && *m == mod_index);
            if keep {
                out.push(Kept {
                    item: item.item,
                    module,
                });
            }
        }
    }
    out
}

/// Os números de uma versão em ordem (`0.92.7+1.20.1` → `[0, 92, 7, 1, 20, 1]`).
fn version_key(version: &str) -> Vec<u64> {
    version
        .split(|c: char| !c.is_ascii_digit())
        .filter(|s| !s.is_empty())
        .map(|s| s.parse().unwrap_or(u64::MAX))
        .collect()
}

const LIBRARIES_TOML: &str = include_str!("../data/libraries.toml");

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Libraries {
    ids: Vec<String>,
}

/// Lê e valida `data/libraries.toml`.
///
/// # Errors
///
/// TOML inválido, campo desconhecido ou id vazio.
pub fn parse_libraries(text: &str) -> Result<Vec<String>, String> {
    let libraries: Libraries = toml::from_str(text).map_err(|e| e.to_string())?;
    if libraries.ids.iter().any(|id| id.trim().is_empty()) {
        return Err("id vazio".to_owned());
    }
    Ok(libraries.ids)
}

fn libraries() -> &'static HashSet<String> {
    static IDS: OnceLock<HashSet<String>> = OnceLock::new();
    IDS.get_or_init(|| {
        parse_libraries(LIBRARIES_TOML)
            .unwrap_or_else(|error| {
                tracing::error!(%error, "libraries.toml inválido");
                Vec::new()
            })
            .into_iter()
            .collect()
    })
}

/// O id é de uma biblioteca (lista curada ou módulo da Fabric API).
#[must_use]
pub fn is_library(id: &str) -> bool {
    libraries().contains(id) || id.starts_with("fabric-") || id.starts_with("quilted_")
}

/// Uma alteração com quem a faz.
struct Entry<'a> {
    item: &'a str,
    module: &'a ScannedMod,
    class: &'a MixinClass,
    change: &'a Change,
}

impl Entry<'_> {
    fn participant(&self) -> Participant {
        Participant {
            item: self.item.to_owned(),
            mod_id: self.module.mod_id.clone(),
            kind: self.change.kind,
            mixin_class: self.class.name.clone(),
            config: self.class.config.clone(),
            priority: self.class.priority,
            at: self.change.at.clone(),
            plugin: self.class.plugin,
        }
    }
}

/// Mesmo ponto: mesmo `@At` e, nos que mudam um valor, o mesmo seletor (`*` = todos).
fn same_point(a: &Change, b: &Change) -> bool {
    a.at == b.at
        && (a.selector.is_empty()
            || b.selector.is_empty()
            || a.selector == "*"
            || b.selector == "*"
            || a.selector == b.selector)
}

fn owners(entries: &[&Entry<'_>]) -> BTreeSet<String> {
    entries.iter().map(|e| e.item.to_owned()).collect()
}

/// Classifica um método alterado por 2+ itens. Devolve a regra, o risco base e os envolvidos.
fn classify<'e, 'a>(entries: &'e [Entry<'a>]) -> (Rule, Risk, Vec<&'e Entry<'a>>) {
    let all: Vec<&Entry<'_>> = entries.iter().collect();
    let overwrites: Vec<&Entry<'_>> = entries
        .iter()
        .filter(|e| e.change.kind == ChangeKind::Overwrite)
        .collect();
    if owners(&overwrites).len() >= 2 {
        return (Rule::TwoOverwrites, Risk::High, overwrites);
    }
    for entry in entries.iter().filter(|e| {
        matches!(
            e.change.kind,
            ChangeKind::Redirect | ChangeKind::ModifyConstant
        )
    }) {
        let same: Vec<&Entry<'_>> = entries
            .iter()
            .filter(|x| {
                matches!(
                    x.change.kind,
                    ChangeKind::Redirect | ChangeKind::ModifyConstant
                ) && same_point(x.change, entry.change)
            })
            .collect();
        if owners(&same).len() >= 2 {
            return (Rule::TwoReplacements, Risk::High, same);
        }
    }
    if !overwrites.is_empty()
        && let Some(body) = entries.iter().find(|x| {
            x.change.kind != ChangeKind::Overwrite
                && x.change.at.in_body()
                && overwrites.iter().any(|o| o.item != x.item)
        })
    {
        let mut involved: Vec<&Entry<'_>> = overwrites
            .iter()
            .copied()
            .filter(|o| o.item != body.item)
            .collect();
        involved.push(body);
        return (Rule::OverwriteAndBody, Risk::Medium, involved);
    }
    let mut redirect_and_inject = None;
    for redirect in entries
        .iter()
        .filter(|e| e.change.kind == ChangeKind::Redirect)
    {
        for other in entries.iter().filter(|x| {
            x.item != redirect.item
                && x.change.kind != ChangeKind::Redirect
                && same_point(x.change, redirect.change)
        }) {
            if other.change.kind == ChangeKind::Inject {
                redirect_and_inject.get_or_insert_with(|| vec![redirect, other]);
            } else {
                return (Rule::RedirectAndOther, Risk::Medium, vec![redirect, other]);
            }
        }
    }
    for entry in entries.iter().filter(|e| e.change.kind.modifies_value()) {
        let same: Vec<&Entry<'_>> = entries
            .iter()
            .filter(|x| x.change.kind.modifies_value() && same_point(x.change, entry.change))
            .collect();
        if owners(&same).len() >= 2 {
            return (Rule::TwoValueChanges, Risk::Medium, same);
        }
    }
    if let Some(involved) = redirect_and_inject {
        return (Rule::RedirectAndInject, Risk::Low, involved);
    }
    if overwrites.is_empty() {
        (Rule::Coexist, Risk::Low, all)
    } else {
        (Rule::OverwriteAndEdge, Risk::Low, all)
    }
}

/// O que se sabe de cada item para os rebaixadores.
struct ItemFacts<'a> {
    ids: BTreeSet<&'a str>,
    deps: BTreeSet<&'a str>,
    packages: Vec<&'a str>,
    plugin_strings: HashSet<&'a str>,
}

/// Cruza os mods do pack.
#[must_use]
#[allow(clippy::too_many_lines)]
pub fn cross(kept: &[Kept<'_>], context: &CrossContext) -> Vec<Overlap> {
    let mut facts: BTreeMap<&str, ItemFacts<'_>> = BTreeMap::new();
    let mut package_owner: HashMap<&str, BTreeSet<&str>> = HashMap::new();
    let mut names: HashMap<&str, &str> = HashMap::new();
    let mut disabled = context.disabled.clone();
    for k in kept {
        disabled.extend(k.module.disables.iter().cloned());
        let entry = facts.entry(k.item).or_insert_with(|| ItemFacts {
            ids: BTreeSet::new(),
            deps: BTreeSet::new(),
            packages: Vec::new(),
            plugin_strings: HashSet::new(),
        });
        entry.ids.insert(k.module.mod_id.as_str());
        entry.deps.extend(
            k.module
                .deps
                .iter()
                .filter(|d| !is_library(&d.id))
                .map(|d| d.id.as_str()),
        );
        entry
            .plugin_strings
            .extend(k.module.plugin_strings.iter().map(String::as_str));
        for package in &k.module.packages {
            entry.packages.push(package);
            package_owner
                .entry(package.as_str())
                .or_default()
                .insert(k.item);
        }
        for (runtime, named) in &k.module.names {
            names.entry(runtime.as_str()).or_insert(named.as_str());
        }
    }

    let mut by_method: BTreeMap<(&str, &str), Vec<Entry<'_>>> = BTreeMap::new();
    for k in kept {
        for class in &k.module.classes {
            if is_disabled(&k.module.mod_id, &class.name, &class.config, &disabled) {
                continue;
            }
            for change in &class.changes {
                by_method
                    .entry((change.target_class.as_str(), change.method.as_str()))
                    .or_default()
                    .push(Entry {
                        item: k.item,
                        module: k.module,
                        class,
                        change,
                    });
            }
        }
    }

    let pair_in = |list: &[CuratedPair], a: &ItemFacts<'_>, b: &ItemFacts<'_>| {
        list.iter()
            .find(|pair| {
                (a.ids.contains(pair.first.as_str()) && b.ids.contains(pair.second.as_str()))
                    || (a.ids.contains(pair.second.as_str()) && b.ids.contains(pair.first.as_str()))
            })
            .map(|pair| pair.label.clone())
    };

    let mut out = Vec::new();
    for ((target_class, method), entries) in &by_method {
        let items: BTreeSet<&str> = entries.iter().map(|e| e.item).collect();
        if items.len() < 2 {
            continue;
        }
        let (rule, base, involved) = classify(entries);
        let involved_items: BTreeSet<&str> = involved.iter().map(|e| e.item).collect();

        // Rebaixadores: cada tipo desce um nível.
        let mut demoters = Vec::new();
        if involved.iter().all(|e| e.class.plugin) {
            demoters.push(Demoter::AllPlugin);
        }
        let mut compat = None;
        'compat: for a in &involved {
            for b in involved_items.iter().filter(|b| **b != a.item) {
                let lowered = format!("{}|{}", a.class.config, a.class.name).to_lowercase();
                if lowered.contains("compat") {
                    compat = Some(Demoter::CompatPackage {
                        item: a.item.to_owned(),
                        mixin_class: a.class.name.clone(),
                    });
                } else if a.class.refs.iter().any(|r| {
                    r.rsplit_once('/')
                        .and_then(|(package, _)| package_owner.get(package))
                        .is_some_and(|owners| owners.contains(b) && !owners.contains(a.item))
                }) {
                    compat = Some(Demoter::ReferencesOther {
                        item: a.item.to_owned(),
                        other: (*b).to_owned(),
                    });
                } else if let (Some(fa), Some(fb)) = (facts.get(a.item), facts.get(b))
                    && fb
                        .ids
                        .iter()
                        .any(|id| fa.deps.contains(id) && !is_library(id))
                {
                    compat = Some(Demoter::DeclaresDependency {
                        item: a.item.to_owned(),
                        other: (*b).to_owned(),
                    });
                }
                if compat.is_some() {
                    break 'compat;
                }
            }
        }
        if let Some(demoter) = compat {
            demoters.push(demoter);
        }
        'cites: for a in &involved_items {
            for b in involved_items.iter().filter(|b| *b != a) {
                if let (Some(fa), Some(fb)) = (facts.get(a), facts.get(b))
                    && fb
                        .ids
                        .iter()
                        .any(|id| id.len() > 2 && fa.plugin_strings.contains(id))
                {
                    demoters.push(Demoter::PluginCitesOther {
                        item: (*a).to_owned(),
                        other: (*b).to_owned(),
                    });
                    break 'cites;
                }
            }
        }
        let mut known = None;
        let mut conflict = None;
        for a in &involved_items {
            for b in involved_items.iter().filter(|b| *b > a) {
                if let (Some(fa), Some(fb)) = (facts.get(a), facts.get(b)) {
                    known = known.or_else(|| pair_in(&context.known_compatible, fa, fb));
                    conflict = conflict.or_else(|| pair_in(&context.known_conflicts, fa, fb));
                }
            }
        }
        if let Some(origin) = known {
            demoters.push(Demoter::KnownCompatible { origin });
        }
        let mut risk = base;
        for _ in &demoters {
            risk = risk.down();
        }

        // Elevadores.
        let mut elevators = Vec::new();
        if matches!(rule, Rule::OverwriteAndBody | Rule::RedirectAndOther)
            && let Some(at_risk) = involved.iter().find(|e| {
                !e.change.kind.replaces()
                    && e.change
                        .require
                        .or(e.class.default_require)
                        .is_some_and(|r| r >= 1)
            })
        {
            // Fica como evidência, sem subir o risco: no pack Fabric do S-R5-1 subir um nível
            // transformou 13 sobreposições "em dúvida" (R5A, categorias B e D) em risco alto,
            // nenhuma com conflito real conhecido. `require` agrava a consequência (o jogo trava
            // em vez de perder a alteração), não a chance de acontecer.
            elevators.push(Elevator::Require {
                config: at_risk.class.config.clone(),
            });
        }
        if let Some(id) = conflict {
            elevators.push(Elevator::KnownConflict { id });
            risk = Risk::High;
        }
        let configs: BTreeSet<&str> = involved.iter().map(|e| e.class.config.as_str()).collect();
        if let Some(citation) = context
            .crash
            .iter()
            .find(|c| configs.contains(c.config.as_str()))
        {
            elevators.push(Elevator::Crash(citation.clone()));
            risk = Risk::High;
        }

        let class_named = names
            .get(target_class)
            .map_or_else(String::new, |n| (*n).to_owned());
        let method_named = names
            .get(method)
            .map(|n| (*n).to_owned())
            .or_else(|| {
                entries
                    .iter()
                    .map(|e| e.change.method_named.as_str())
                    .find(|n| !n.is_empty())
                    .map(str::to_owned)
            })
            .unwrap_or_default();

        let mut participants: Vec<Participant> = involved.iter().map(|e| e.participant()).collect();
        participants.sort_by(|a, b| (a.priority, &a.item).cmp(&(b.priority, &b.item)));
        participants.dedup();
        out.push(Overlap {
            target_class: (*target_class).to_owned(),
            method: (*method).to_owned(),
            class_named,
            method_named,
            items: items.iter().map(|i| (*i).to_owned()).collect(),
            rule,
            base,
            risk,
            participants,
            demoters,
            elevators,
        });
    }
    out
}

/// As linhas de um log que citam configs de mixin (`algo.mixins.json`), a primeira de cada
/// config. `file` é o nome que a evidência mostra.
#[must_use]
pub fn crash_citations(text: &str, file: &str) -> Vec<CrashCitation> {
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    for (index, line) in text.lines().enumerate() {
        if !line.contains(".json") {
            continue;
        }
        for token in
            line.split(|c: char| !(c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-')))
        {
            let lowered = token.to_ascii_lowercase();
            let is_json = std::path::Path::new(&lowered)
                .extension()
                .is_some_and(|e| e == "json");
            if is_json && lowered.contains("mixin") && seen.insert(token.to_owned()) {
                out.push(CrashCitation {
                    config: token.to_owned(),
                    file: file.to_owned(),
                    line: u32::try_from(index + 1).unwrap_or(u32::MAX),
                    excerpt: line.trim().chars().take(300).collect(),
                });
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bibliotecas() {
        assert!(
            parse_libraries(LIBRARIES_TOML)
                .unwrap()
                .contains(&"architectury".to_owned())
        );
        assert!(is_library("fabric-api"));
        assert!(is_library("fabric-item-api-v1"));
        assert!(is_library("architectury"));
        assert!(!is_library("sodium"));
        assert!(parse_libraries("ids = [\"\"]").is_err());
        assert!(parse_libraries("ids = []\noutro = 1").is_err());
    }

    #[test]
    fn citacoes_do_log() {
        let log = "linha\n[12:00] Mixin apply failed epicfight.mixins.json:LivingEntityMixin\n\tat x (a.mixins.json)\nde novo epicfight.mixins.json\nconfig/sodium.json\n";
        let found = crash_citations(log, "output.log");
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].config, "epicfight.mixins.json");
        assert_eq!(found[0].line, 2);
        assert!(found[0].excerpt.starts_with("[12:00] Mixin apply failed"));
        assert_eq!(found[1].config, "a.mixins.json");
        assert_eq!(found[1].line, 3);
    }

    #[test]
    fn risco_desce_sem_passar_do_limite() {
        assert_eq!(Risk::High.down().down().down(), Risk::Low);
        assert_eq!(Risk::parse_pt("medio"), Some(Risk::Medium));
        assert_eq!(Risk::parse_pt("x"), None);
    }

    #[test]
    fn versoes() {
        assert!(version_key("0.92.7+1.20.1") > version_key("0.92.6+1.20.1"));
        assert!(version_key("1.10") > version_key("1.9"));
        assert_eq!(version_key(""), Vec::<u64>::new());
    }
}
