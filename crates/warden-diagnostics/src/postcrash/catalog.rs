//! Catálogo de padrões de log (`data/log-patterns.toml`, ARCHITECTURE §9.3 item 3).
//!
//! O arquivo é embutido no binário e compilado uma vez. A validação (ids únicos, códigos de
//! regra, expressões, parâmetros dos modelos) roda no carregamento: um catálogo inválido vira
//! erro `INTERNAL`, e os testes garantem que o embutido é válido.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::LazyLock;

use regex::{Regex, RegexSet};
use serde::Deserialize;

use super::LogLoader;
use crate::error::{DiagnosticsError, Result};
use crate::model::{RuleCode, Severity, SideChoice};

/// O catálogo embutido.
pub(crate) const EMBEDDED: &str = include_str!("../../data/log-patterns.toml");

/// Parâmetros calculados pelo código (não vêm de grupos), aceitos nos modelos.
pub(crate) const DERIVED_PARAMS: [&str; 4] =
    ["java_required", "exception_type", "cause", "cause_type"];

/// Classe do padrão: onde o sinal costuma estar. Define a ordem dos achados.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum PatternClass {
    /// Sinal genérico.
    Generic = 1,
    /// Exceção (a mais profunda vence as genéricas).
    Exception = 2,
    /// Tela de erro do loader (lista de dependências, duplicados…).
    LoaderError = 3,
    /// Crash report do jogo ou relatório da JVM.
    CrashReport = 4,
}

impl PatternClass {
    pub(crate) fn tier(self) -> u8 {
        self as u8
    }
}

/// Onde o padrão vale.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Scope {
    /// Em qualquer linha.
    #[default]
    Anywhere,
    /// Só dentro de um crash report.
    CrashReport,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CatalogFile {
    version: u32,
    pattern: Vec<PatternDef>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PatternDef {
    id: String,
    origin: String,
    note: String,
    rule: String,
    severity: Severity,
    title_key: String,
    class: PatternClass,
    weight: u8,
    regex: String,
    #[serde(default)]
    scope: Scope,
    #[serde(default)]
    loaders: Vec<LogLoader>,
    #[serde(default)]
    follow: Vec<FollowDef>,
    #[serde(default)]
    unless: Vec<FollowDef>,
    #[serde(default)]
    only: BTreeMap<String, Vec<String>>,
    #[serde(default)]
    except: BTreeMap<String, Vec<String>>,
    key: Vec<String>,
    #[serde(default)]
    cause: bool,
    #[serde(default)]
    frames: bool,
    #[serde(default)]
    fallback: bool,
    #[serde(default)]
    suppresses: Vec<String>,
    #[serde(default)]
    items: Vec<ItemTemplate>,
    #[serde(default)]
    fixes: Vec<FixTemplate>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct FollowDef {
    regex: String,
    within: usize,
    #[serde(default)]
    required: bool,
}

/// Modelo de item: um dos campos, com `{parâmetro}`.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(crate) struct ItemTemplate {
    /// Id do mod.
    #[serde(default, rename = "mod")]
    pub mod_: Option<String>,
    /// Nome de um jar (o caminho é reduzido ao nome).
    #[serde(default)]
    pub jar: Option<String>,
    /// Lista de jars separada por vírgula.
    #[serde(default)]
    pub jars: Option<String>,
}

/// Modelo de correção.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum FixTemplate {
    /// Adicionar dependência.
    AddDependency {
        mod_id: String,
        #[serde(default)]
        version_range: Option<String>,
    },
    /// Remover um item.
    RemoveItem {
        #[serde(default, rename = "mod")]
        mod_: Option<String>,
        #[serde(default)]
        jar: Option<String>,
        /// Lista de jars: sugere remover todos menos o primeiro.
        #[serde(default)]
        jars_after_first: Option<String>,
    },
    /// Atualizar um item.
    UpdateItem {
        #[serde(default, rename = "mod")]
        mod_: Option<String>,
        #[serde(default)]
        jar: Option<String>,
    },
    /// Mudar o lado de um item.
    SetSide {
        #[serde(rename = "mod")]
        mod_: String,
        side: SideChoice,
    },
    /// Trocar o Java.
    ChangeJava {
        #[serde(default)]
        major: Option<String>,
    },
    /// Aumentar a memória.
    ChangeMemory,
    /// Tirar um argumento da JVM.
    RemoveJvmArgument { argument: String },
    /// Restaurar uma config.
    RestoreConfig { path: String },
}

/// Busca nas linhas seguintes.
#[derive(Debug)]
pub(crate) struct Follow {
    pub regex: Regex,
    pub within: usize,
    pub required: bool,
}

/// Um padrão compilado.
#[derive(Debug)]
pub(crate) struct Pattern {
    pub id: String,
    pub rule: RuleCode,
    pub severity: Severity,
    pub title_key: String,
    pub class: PatternClass,
    pub weight: u8,
    pub regex: Regex,
    pub scope: Scope,
    pub loaders: Vec<LogLoader>,
    pub follow: Vec<Follow>,
    pub unless: Vec<Follow>,
    pub only: BTreeMap<String, Vec<String>>,
    pub except: BTreeMap<String, Vec<String>>,
    pub key: Vec<String>,
    pub cause: bool,
    pub frames: bool,
    pub fallback: bool,
    pub suppresses: Vec<String>,
    pub items: Vec<ItemTemplate>,
    pub fixes: Vec<FixTemplate>,
}

/// O catálogo compilado.
#[derive(Debug)]
pub(crate) struct Catalog {
    pub patterns: Vec<Pattern>,
    /// Todas as expressões principais, para achar numa passada quais padrões casam numa linha.
    pub set: RegexSet,
}

/// Nome do parâmetro de um grupo: `dependency__2` → `dependency`.
pub(crate) fn param_name(group: &str) -> &str {
    group.split_once("__").map_or(group, |(name, _)| name)
}

/// Os `{parâmetro}` usados num modelo.
fn placeholders(template: &str) -> Vec<&str> {
    let mut found = Vec::new();
    let mut rest = template;
    while let Some(start) = rest.find('{') {
        let after = &rest[start + 1..];
        let Some(end) = after.find('}') else { break };
        found.push(&after[..end]);
        rest = &after[end + 1..];
    }
    found
}

fn compile(pattern_id: &str, expression: &str) -> Result<Regex> {
    Regex::new(expression).map_err(|error| {
        DiagnosticsError::Internal(format!("padrão {pattern_id}: expressão inválida: {error}"))
    })
}

fn invalid(pattern_id: &str, reason: &str) -> DiagnosticsError {
    DiagnosticsError::Internal(format!("padrão {pattern_id}: {reason}"))
}

impl ItemTemplate {
    fn templates(&self) -> impl Iterator<Item = &String> {
        [&self.mod_, &self.jar, &self.jars].into_iter().flatten()
    }
}

impl FixTemplate {
    fn templates(&self) -> Vec<&String> {
        match self {
            Self::AddDependency {
                mod_id,
                version_range,
            } => [Some(mod_id), version_range.as_ref()]
                .into_iter()
                .flatten()
                .collect(),
            Self::RemoveItem {
                mod_,
                jar,
                jars_after_first,
            } => [mod_, jar, jars_after_first]
                .into_iter()
                .flatten()
                .collect(),
            Self::UpdateItem { mod_, jar } => [mod_, jar].into_iter().flatten().collect(),
            Self::SetSide { mod_, .. } => vec![mod_],
            Self::ChangeJava { major } => major.iter().collect(),
            Self::ChangeMemory => Vec::new(),
            Self::RemoveJvmArgument { argument } => vec![argument],
            Self::RestoreConfig { path } => vec![path],
        }
    }
}

/// Lê, valida e compila um catálogo.
pub(crate) fn parse_catalog(text: &str) -> Result<Catalog> {
    let file: CatalogFile = toml::from_str(text)
        .map_err(|error| DiagnosticsError::Internal(format!("catálogo ilegível: {error}")))?;
    if file.version != 1 {
        return Err(DiagnosticsError::Internal(format!(
            "versão do catálogo desconhecida: {}",
            file.version
        )));
    }
    let mut ids = BTreeSet::new();
    let mut patterns = Vec::with_capacity(file.pattern.len());
    for def in file.pattern {
        if !ids.insert(def.id.clone()) {
            return Err(invalid(&def.id, "id repetido"));
        }
        patterns.push(build_pattern(def)?);
    }
    for pattern in &patterns {
        for target in &pattern.suppresses {
            if !ids.contains(target) {
                return Err(invalid(
                    &pattern.id,
                    &format!("suppresses cita padrão inexistente {target}"),
                ));
            }
        }
    }
    let set = RegexSet::new(patterns.iter().map(|pattern| pattern.regex.as_str()))
        .map_err(|error| DiagnosticsError::Internal(format!("conjunto de expressões: {error}")))?;
    Ok(Catalog { patterns, set })
}

/// Compila as buscas nas linhas seguintes, juntando os grupos a `params`.
fn compile_follows(
    id: &str,
    defs: Vec<FollowDef>,
    params: &mut BTreeSet<String>,
) -> Result<Vec<Follow>> {
    defs.into_iter()
        .map(|follow| {
            if follow.within == 0 {
                return Err(invalid(id, "follow/unless com within = 0"));
            }
            let regex = compile(id, &follow.regex)?;
            params.extend(
                regex
                    .capture_names()
                    .flatten()
                    .map(|name| param_name(name).to_owned()),
            );
            Ok(Follow {
                regex,
                within: follow.within,
                required: follow.required,
            })
        })
        .collect()
}

/// Confere que itens, correções, `key`, `only` e `except` só citam parâmetros conhecidos.
fn check_templates(id: &str, def: &PatternDef, params: &BTreeSet<String>) -> Result<()> {
    let check = |template: &str| -> Result<()> {
        for name in placeholders(template) {
            if !params.contains(name) {
                return Err(invalid(id, &format!("parâmetro desconhecido {{{name}}}")));
            }
        }
        Ok(())
    };
    for item in &def.items {
        let mut count = 0;
        for template in item.templates() {
            check(template)?;
            count += 1;
        }
        if count != 1 {
            return Err(invalid(id, "item precisa de exatamente um campo"));
        }
    }
    for fix in &def.fixes {
        for template in fix.templates() {
            check(template)?;
        }
    }
    for name in def
        .key
        .iter()
        .chain(def.only.keys())
        .chain(def.except.keys())
    {
        if !params.contains(name.as_str()) {
            return Err(invalid(
                id,
                &format!("parâmetro desconhecido {name} em key/only/except"),
            ));
        }
    }
    Ok(())
}

/// Valida e compila um padrão.
fn build_pattern(def: PatternDef) -> Result<Pattern> {
    let id = def.id.clone();
    if def.origin.trim().is_empty() || def.note.trim().is_empty() {
        return Err(invalid(&id, "sem origem ou sem nota"));
    }
    if def.title_key.trim().is_empty() {
        return Err(invalid(&id, "sem title_key"));
    }
    if !(1..=100).contains(&def.weight) {
        return Err(invalid(&id, "weight fora de 1..=100"));
    }
    let rule =
        RuleCode::parse(&def.rule).ok_or_else(|| invalid(&id, "código de regra inválido"))?;
    let regex = compile(&id, &def.regex)?;
    let mut params: BTreeSet<String> = regex
        .capture_names()
        .flatten()
        .map(|name| param_name(name).to_owned())
        .collect();
    let mut def = def;
    let follow = compile_follows(&id, std::mem::take(&mut def.follow), &mut params)?;
    let unless = compile_follows(&id, std::mem::take(&mut def.unless), &mut BTreeSet::new())?;
    params.extend(DERIVED_PARAMS.iter().map(|name| (*name).to_owned()));
    check_templates(&id, &def, &params)?;
    Ok(Pattern {
        id,
        rule,
        severity: def.severity,
        title_key: def.title_key,
        class: def.class,
        weight: def.weight,
        regex,
        scope: def.scope,
        loaders: def.loaders,
        follow,
        unless,
        only: lowercase_values(def.only),
        except: lowercase_values(def.except),
        key: def.key,
        cause: def.cause,
        frames: def.frames,
        fallback: def.fallback,
        suppresses: def.suppresses,
        items: def.items,
        fixes: def.fixes,
    })
}

fn lowercase_values(map: BTreeMap<String, Vec<String>>) -> BTreeMap<String, Vec<String>> {
    map.into_iter()
        .map(|(key, values)| {
            (
                key,
                values
                    .into_iter()
                    .map(|value| value.to_lowercase())
                    .collect(),
            )
        })
        .collect()
}

static CATALOG: LazyLock<std::result::Result<Catalog, String>> =
    LazyLock::new(|| parse_catalog(EMBEDDED).map_err(|error| error.to_string()));

/// O catálogo embutido, compilado uma vez.
pub(crate) fn embedded() -> Result<&'static Catalog> {
    CATALOG
        .as_ref()
        .map_err(|message| DiagnosticsError::Internal(message.clone()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalogo_embutido_e_valido() {
        let catalog = embedded().unwrap();
        assert!(catalog.patterns.len() >= 70, "{}", catalog.patterns.len());
        assert_eq!(catalog.set.len(), catalog.patterns.len());
    }

    #[test]
    fn nome_do_parametro_tira_o_sufixo() {
        assert_eq!(param_name("dependency__2"), "dependency");
        assert_eq!(param_name("mod"), "mod");
        assert_eq!(placeholders("{a} e {b}"), ["a", "b"]);
        assert_eq!(placeholders("sem"), Vec::<&str>::new());
        assert_eq!(placeholders("{aberto"), Vec::<&str>::new());
    }

    fn one(extra: &str) -> String {
        format!(
            "version = 1\n[[pattern]]\nid = \"a\"\norigin = \"o\"\nnote = \"n\"\nrule = \"E_X\"\nseverity = \"error\"\ntitle_key = \"t\"\nclass = \"generic\"\nweight = 10\nregex = '(?P<mod>\\S+)'\nkey = []\n{extra}"
        )
    }

    #[test]
    fn recusa_catalogo_invalido() {
        let cases = [
            ("version = 2\npattern = []".to_owned(), "versão"),
            ("não é toml".to_owned(), "ilegível"),
            (one("") + &one("").replace("version = 1\n", ""), "repetido"),
            (one("").replace("weight = 10", "weight = 0"), "weight"),
            (one("").replace("E_X", "X"), "código"),
            (one("").replace("(?P<mod>\\S+)", "("), "expressão"),
            (
                one("").replace("origin = \"o\"", "origin = \" \""),
                "origem",
            ),
            (
                one("").replace("title_key = \"t\"", "title_key = \"\""),
                "title_key",
            ),
            (one("items = [{ mod = \"{outro}\" }]"), "desconhecido"),
            (
                one("items = [{ mod = \"{mod}\", jar = \"{mod}\" }]"),
                "exatamente",
            ),
            (one("items = [{}]"), "exatamente"),
            (
                one("fixes = [{ kind = \"restore_config\", path = \"{x}\" }]"),
                "desconhecido",
            ),
            (one("only = { outro = [\"a\"] }"), "key/only/except"),
            (one("follow = [{ regex = 'x', within = 0 }]"), "within"),
            (one("suppresses = [\"nada\"]"), "inexistente"),
        ];
        for (text, expected) in cases {
            let error = parse_catalog(&text).unwrap_err().to_string();
            assert!(error.contains(expected), "{expected}: {error}");
        }
        assert!(parse_catalog(&one("fixes = [{ kind = \"change_memory\" }]")).is_ok());
    }

    #[test]
    fn modelos_listam_os_textos() {
        let fixes = [
            FixTemplate::AddDependency {
                mod_id: "a".into(),
                version_range: Some("b".into()),
            },
            FixTemplate::RemoveItem {
                mod_: None,
                jar: Some("j".into()),
                jars_after_first: None,
            },
            FixTemplate::UpdateItem {
                mod_: Some("m".into()),
                jar: None,
            },
            FixTemplate::SetSide {
                mod_: "m".into(),
                side: SideChoice::Both,
            },
            FixTemplate::ChangeJava { major: None },
            FixTemplate::ChangeMemory,
            FixTemplate::RemoveJvmArgument {
                argument: "x".into(),
            },
            FixTemplate::RestoreConfig { path: "p".into() },
        ];
        let counts: Vec<_> = fixes.iter().map(|fix| fix.templates().len()).collect();
        assert_eq!(counts, [2, 1, 1, 1, 0, 0, 1, 1]);
        assert_eq!(PatternClass::CrashReport.tier(), 4);
        assert_eq!(PatternClass::Generic.tier(), 1);
    }
}
