//! O motor da análise: aplica o catálogo às linhas limpas, junta achados iguais e ordena.

use std::collections::{BTreeMap, BTreeSet};
use std::ops::Range;

use super::catalog::{self, Catalog, FixTemplate, ItemTemplate, Pattern, Scope, param_name};
use super::context;
use super::stack::{self, ModMention};
use super::text::{self, CleanLine};
use super::{
    Analysis, AnalysisContext, AnalyzedSource, LogLoader, LogSource, PostCrashFinding,
    SessionOutcome, SourceKind,
};
use crate::error::Result;
use crate::model::{Evidence, Finding, ItemRef, RuleCode, Severity, SuggestedFix};

/// Máximo de evidências guardadas por achado.
pub const MAX_EVIDENCE: usize = 5;

/// Id do achado do jogo parado (vem do supervisor, não do catálogo).
pub const FROZE_PATTERN: &str = "session.froze";

/// Uma fonte limpa, com os blocos de crash report marcados.
struct Prepared<'a> {
    source: &'a LogSource,
    order: usize,
    lines: Vec<CleanLine>,
    in_block: Vec<Option<Range<usize>>>,
}

/// Um achado candidato, antes de juntar os iguais.
struct Candidate {
    pattern: usize,
    tier: u8,
    weight: u8,
    order: usize,
    line: u32,
    key: String,
    finding: Finding,
}

impl Candidate {
    /// Ordem dentro de um grupo: mais forte primeiro.
    fn strength(&self) -> (std::cmp::Reverse<u8>, std::cmp::Reverse<u8>, usize, u32) {
        (
            std::cmp::Reverse(self.tier),
            std::cmp::Reverse(self.weight),
            self.order,
            self.line,
        )
    }
}

/// Analisa um texto solto (gancho 1.1, ADR-0039): um log escolhido no computador, colado ou
/// baixado de um jogador. `name` aparece na evidência.
pub fn analyze_text(name: &str, text: &str, context: &AnalysisContext) -> Result<Analysis> {
    analyze(
        &[LogSource::from_text(name, SourceKind::Text, text)],
        context,
        None,
    )
}

/// Analisa os arquivos de uma sessão (ou qualquer conjunto de textos).
pub fn analyze(
    sources: &[LogSource],
    context: &AnalysisContext,
    outcome: Option<&SessionOutcome>,
) -> Result<Analysis> {
    let catalog = catalog::embedded()?;
    Ok(analyze_with(catalog, sources, context, outcome))
}

pub(crate) fn analyze_with(
    catalog: &Catalog,
    sources: &[LogSource],
    context: &AnalysisContext,
    outcome: Option<&SessionOutcome>,
) -> Analysis {
    let mut ordered: Vec<(usize, &LogSource)> = sources.iter().enumerate().collect();
    ordered.sort_by_key(|(index, source)| (source.kind(), *index));
    let prepared: Vec<Prepared<'_>> = ordered
        .into_iter()
        .enumerate()
        .map(|(order, (_, source))| prepare(source, order))
        .collect();

    let detected = context::detect(prepared.iter().flat_map(|source| source.lines.iter()));
    let loader = context.loader.or(detected.loader);
    let minecraft = context.minecraft.clone().or(detected.minecraft.clone());

    let mut candidates = Vec::new();
    for source in &prepared {
        scan(catalog, source, loader, &mut candidates);
    }
    let mut findings = merge(catalog, candidates);

    if let Some(froze) = outcome
        .filter(|outcome| outcome.froze)
        .and_then(|_| froze_finding(&prepared))
    {
        findings.push(froze);
    }
    sort_findings(&mut findings);

    Analysis {
        findings: findings
            .into_iter()
            .map(|ranked| PostCrashFinding {
                pattern: ranked.pattern,
                finding: ranked.finding,
            })
            .collect(),
        loader,
        loader_version: if context.loader.is_none() || context.loader == detected.loader {
            detected.loader_version.clone()
        } else {
            None
        },
        minecraft,
        sources: prepared
            .iter()
            .map(|source| AnalyzedSource {
                name: source.source.name().to_owned(),
                kind: source.source.kind(),
                lines: u32::try_from(source.source.line_count()).unwrap_or(u32::MAX),
            })
            .collect(),
    }
}

fn prepare(source: &LogSource, order: usize) -> Prepared<'_> {
    let lines = text::clean_lines(&source.lines);
    let blocks = stack::crash_blocks(&lines, source.kind() == SourceKind::CrashReport);
    let mut in_block = vec![None; lines.len()];
    for block in blocks {
        for slot in &mut in_block[block.clone()] {
            *slot = Some(block.clone());
        }
    }
    if source.kind() == SourceKind::JvmCrash {
        let whole = 0..lines.len();
        for slot in &mut in_block {
            *slot = Some(whole.clone());
        }
    }
    Prepared {
        source,
        order,
        lines,
        in_block,
    }
}

fn loader_matches(pattern: &Pattern, loader: Option<LogLoader>) -> bool {
    pattern.loaders.is_empty() || loader.is_none_or(|loader| pattern.loaders.contains(&loader))
}

/// Junta os grupos nomeados da expressão (sem sobrescrever valores já lidos).
fn collect_params(regex: &regex::Regex, text: &str, params: &mut BTreeMap<String, String>) -> bool {
    let Some(captures) = regex.captures(text) else {
        return false;
    };
    for name in regex.capture_names().flatten() {
        if let Some(value) = captures.name(name) {
            let value = value.as_str().trim();
            if !value.is_empty() {
                params
                    .entry(param_name(name).to_owned())
                    .or_insert_with(|| value.to_owned());
            }
        }
    }
    true
}

/// Primeira linha depois de `index` (até `within` linhas) em que a expressão casa.
fn find_after(
    lines: &[CleanLine],
    index: usize,
    within: usize,
    regex: &regex::Regex,
) -> Option<usize> {
    let end = lines
        .len()
        .min(index.saturating_add(within).saturating_add(1));
    (index + 1..end).find(|&next| regex.is_match(&lines[next].text))
}

fn scan(
    catalog: &Catalog,
    source: &Prepared<'_>,
    loader: Option<LogLoader>,
    out: &mut Vec<Candidate>,
) {
    for (index, line) in source.lines.iter().enumerate() {
        if line.text.is_empty() {
            continue;
        }
        for pattern_index in catalog.set.matches(&line.text) {
            let pattern = &catalog.patterns[pattern_index];
            if let Some(candidate) = match_pattern(pattern, pattern_index, source, index, loader) {
                out.push(candidate);
            }
        }
    }
}

fn match_pattern(
    pattern: &Pattern,
    pattern_index: usize,
    source: &Prepared<'_>,
    index: usize,
    loader: Option<LogLoader>,
) -> Option<Candidate> {
    let lines = &source.lines;
    let line = &lines[index];
    let block = source.in_block[index].clone();
    if !loader_matches(pattern, loader) || (pattern.scope == Scope::CrashReport && block.is_none())
    {
        return None;
    }
    let (mut params, exception_line) = read_params(pattern, lines, index)?;

    // Pilha: causa mais profunda e mods citados.
    let stack_start = exception_line.unwrap_or(index);
    let stack = stack::stack_range(lines, stack_start);
    if pattern.cause
        && let Some(cause) = stack::deepest_cause(lines, stack.clone())
    {
        params.entry("cause".to_owned()).or_insert(cause);
    }
    derive_params(&mut params);
    let mentions = if pattern.frames {
        let range = block
            .clone()
            .map_or(stack, |block| stack_start.min(block.start)..block.end);
        let fabric = matches!(loader, Some(LogLoader::Fabric | LogLoader::Quilt));
        stack::mods_in(lines, range, fabric)
    } else {
        Vec::new()
    };

    let tier = if block.is_some() || source.source.kind().is_report() {
        catalog::PatternClass::CrashReport.tier()
    } else {
        pattern.class.tier()
    };
    let evidence = Evidence::log(source.source.name(), line.number, &line.text);
    let mut finding = Finding::new(
        pattern.rule.clone(),
        pattern.severity,
        pattern.title_key.clone(),
        evidence,
    );
    for item in pattern
        .items
        .iter()
        .flat_map(|template| expand_item(template, &params))
    {
        finding.push_item(item);
    }
    for mention in mentions {
        finding.push_item(match mention {
            ModMention::Id(id) => ItemRef::ModId { id },
            ModMention::Jar(name) => ItemRef::JarFile { name },
        });
    }
    for fix in pattern
        .fixes
        .iter()
        .flat_map(|template| expand_fix(template, &params))
    {
        finding = finding.with_fix(fix);
    }
    let key = pattern
        .key
        .iter()
        .map(|name| {
            params
                .get(name)
                .map(|value| value.to_lowercase().replace('-', "_"))
                .unwrap_or_default()
        })
        .collect::<Vec<_>>()
        .join("\u{1f}");
    for (name, value) in params {
        finding = finding.with_param(name, value);
    }
    Some(Candidate {
        pattern: pattern_index,
        tier,
        weight: pattern.weight,
        order: source.order,
        line: line.number,
        key: format!("{}\u{1e}{key}", pattern.rule),
        finding,
    })
}

/// Parâmetros da linha e das seguintes (`follow`), com os filtros `unless`, `only` e
/// `except`. Devolve também a linha da exceção, quando um `follow` a achou.
fn read_params(
    pattern: &Pattern,
    lines: &[CleanLine],
    index: usize,
) -> Option<(BTreeMap<String, String>, Option<usize>)> {
    let mut params = BTreeMap::new();
    if !collect_params(&pattern.regex, &lines[index].text, &mut params) {
        return None;
    }
    let mut exception_line = None;
    for follow in &pattern.follow {
        match find_after(lines, index, follow.within, &follow.regex) {
            Some(found) => {
                collect_params(&follow.regex, &lines[found].text, &mut params);
                if follow
                    .regex
                    .capture_names()
                    .flatten()
                    .any(|name| param_name(name) == "exception")
                {
                    exception_line = Some(found);
                }
            }
            None if follow.required => return None,
            None => {}
        }
    }
    if pattern
        .unless
        .iter()
        .any(|unless| find_after(lines, index, unless.within, &unless.regex).is_some())
    {
        return None;
    }
    for (name, values) in &pattern.only {
        let value = params
            .get(name)
            .map(|value| value.to_lowercase())
            .unwrap_or_default();
        if !values.contains(&value) {
            return None;
        }
    }
    for (name, values) in &pattern.except {
        if params
            .get(name)
            .is_some_and(|value| values.contains(&value.to_lowercase()))
        {
            return None;
        }
    }
    Some((params, exception_line))
}

/// Parâmetros calculados: tipo da exceção e da causa, Java exigido pela versão de classe.
fn derive_params(params: &mut BTreeMap<String, String>) {
    for (source, target) in [("exception", "exception_type"), ("cause", "cause_type")] {
        if let Some(value) = params.get(source) {
            let kind = value
                .split([':', ' '])
                .next()
                .unwrap_or_default()
                .trim()
                .to_owned();
            if !kind.is_empty() {
                params.entry(target.to_owned()).or_insert(kind);
            }
        }
    }
    if let Some(version) = params
        .get("class_version")
        .and_then(|value| value.parse::<u32>().ok())
        && version >= 45
    {
        params
            .entry("java_required".to_owned())
            .or_insert_with(|| java_for_class_version(version).to_string());
    }
}

/// Java exigido por uma versão de classe (`major − 44`; 52 = Java 8).
#[must_use]
pub(crate) fn java_for_class_version(class_version: u32) -> u32 {
    class_version.saturating_sub(44)
}

/// Troca cada `{parâmetro}`; `None` se algum estiver ausente ou vazio.
fn expand(template: &str, params: &BTreeMap<String, String>) -> Option<String> {
    let mut out = String::new();
    let mut rest = template;
    while let Some(start) = rest.find('{') {
        out.push_str(&rest[..start]);
        let after = &rest[start + 1..];
        let end = after.find('}')?;
        let value = params
            .get(&after[..end])
            .filter(|value| !value.is_empty())?;
        out.push_str(value);
        rest = &after[end + 1..];
    }
    out.push_str(rest);
    let out = out.trim().to_owned();
    (!out.is_empty()).then_some(out)
}

/// Nome do arquivo de um caminho (com `/` ou `\`).
fn file_name(path: &str) -> String {
    path.trim()
        .trim_matches(['"', '\''])
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or_default()
        .trim()
        .to_owned()
}

fn jar_list(list: &str) -> Vec<String> {
    list.split(',')
        .map(file_name)
        .filter(|name| !name.is_empty())
        .collect()
}

fn expand_item(template: &ItemTemplate, params: &BTreeMap<String, String>) -> Vec<ItemRef> {
    if let Some(id) = template.mod_.as_deref().and_then(|t| expand(t, params)) {
        return vec![ItemRef::ModId { id }];
    }
    if let Some(name) = template.jar.as_deref().and_then(|t| expand(t, params)) {
        return vec![ItemRef::JarFile {
            name: file_name(&name),
        }];
    }
    if let Some(list) = template.jars.as_deref().and_then(|t| expand(t, params)) {
        return jar_list(&list)
            .into_iter()
            .map(|name| ItemRef::JarFile { name })
            .collect();
    }
    Vec::new()
}

fn item_from(
    mod_: Option<&String>,
    jar: Option<&String>,
    params: &BTreeMap<String, String>,
) -> Option<ItemRef> {
    mod_.and_then(|t| expand(t, params))
        .map(|id| ItemRef::ModId { id })
        .or_else(|| {
            jar.and_then(|t| expand(t, params))
                .map(|name| ItemRef::JarFile {
                    name: file_name(&name),
                })
        })
}

fn expand_fix(template: &FixTemplate, params: &BTreeMap<String, String>) -> Vec<SuggestedFix> {
    match template {
        FixTemplate::AddDependency {
            mod_id,
            version_range,
        } => expand(mod_id, params)
            .map(|mod_id| SuggestedFix::AddDependency {
                mod_id,
                version_range: version_range.as_deref().and_then(|t| expand(t, params)),
            })
            .into_iter()
            .collect(),
        FixTemplate::RemoveItem {
            mod_,
            jar,
            jars_after_first,
        } => {
            if let Some(list) = jars_after_first.as_deref().and_then(|t| expand(t, params)) {
                return jar_list(&list)
                    .into_iter()
                    .skip(1)
                    .map(|name| SuggestedFix::RemoveItem {
                        item: ItemRef::JarFile { name },
                    })
                    .collect();
            }
            item_from(mod_.as_ref(), jar.as_ref(), params)
                .map(|item| SuggestedFix::RemoveItem { item })
                .into_iter()
                .collect()
        }
        FixTemplate::UpdateItem { mod_, jar } => item_from(mod_.as_ref(), jar.as_ref(), params)
            .map(|item| SuggestedFix::UpdateItem { item })
            .into_iter()
            .collect(),
        FixTemplate::SetSide { mod_, side } => expand(mod_, params)
            .map(|id| SuggestedFix::SetSide {
                item: ItemRef::ModId { id },
                side: *side,
            })
            .into_iter()
            .collect(),
        FixTemplate::ChangeJava { major } => vec![SuggestedFix::ChangeJava {
            major: major
                .as_deref()
                .and_then(|t| expand(t, params))
                .and_then(|value| value.parse().ok()),
        }],
        FixTemplate::ChangeMemory => vec![SuggestedFix::ChangeMemory],
        FixTemplate::RemoveJvmArgument { argument } => expand(argument, params)
            .map(|argument| SuggestedFix::RemoveJvmArgument { argument })
            .into_iter()
            .collect(),
        FixTemplate::RestoreConfig { path } => expand(path, params)
            .map(|path| SuggestedFix::RestoreConfig { path })
            .into_iter()
            .collect(),
    }
}

/// Achado já juntado, com o que é preciso para ordenar.
struct Ranked {
    pattern: String,
    tier: u8,
    weight: u8,
    order: usize,
    line: u32,
    finding: Finding,
}

fn merge(catalog: &Catalog, candidates: Vec<Candidate>) -> Vec<Ranked> {
    let mut groups: BTreeMap<String, Vec<Candidate>> = BTreeMap::new();
    let present: BTreeSet<usize> = candidates
        .iter()
        .map(|candidate| candidate.pattern)
        .collect();
    for candidate in candidates {
        groups
            .entry(candidate.key.clone())
            .or_default()
            .push(candidate);
    }
    let suppressed: BTreeSet<&str> = present
        .iter()
        .flat_map(|&index| {
            catalog.patterns[index]
                .suppresses
                .iter()
                .map(String::as_str)
        })
        .collect();

    let mut merged = Vec::new();
    for mut group in groups.into_values() {
        group.sort_by_key(Candidate::strength);
        let fallback = group
            .iter()
            .all(|candidate| catalog.patterns[candidate.pattern].fallback);
        let mut members = group.into_iter();
        let Some(first) = members.next() else {
            continue;
        };
        let pattern = &catalog.patterns[first.pattern];
        if suppressed.contains(pattern.id.as_str()) {
            continue;
        }
        let mut finding = first.finding;
        let mut evidence: Vec<(u8, usize, u32, Evidence)> = Vec::new();
        for member in members {
            for item in member.finding.items() {
                finding.push_item(item.clone());
            }
            // As correções vêm do padrão mais forte; as dos outros só entram se ele não tiver
            // nenhuma (evita "adicionar moonlight" duas vezes com faixas escritas diferente).
            if finding.fixes().is_empty() {
                for fix in member.finding.fixes() {
                    finding = finding.with_fix(fix.clone());
                }
            }
            for (name, value) in member.finding.params() {
                if finding.param(name).is_none() {
                    finding = finding.with_param(name.clone(), value.clone());
                }
            }
            evidence.push((
                member.tier,
                member.order,
                member.line,
                member.finding.primary_evidence().clone(),
            ));
        }
        evidence.sort_by_key(|(tier, order, line, _)| (std::cmp::Reverse(*tier), *order, *line));
        for (_, _, _, item) in evidence {
            finding.push_evidence(item);
        }
        finding.truncate_evidence(MAX_EVIDENCE);
        merged.push((
            fallback,
            Ranked {
                pattern: pattern.id.clone(),
                tier: first.tier,
                weight: first.weight,
                order: first.order,
                line: first.line,
                finding,
            },
        ));
    }
    let has_specific_error = merged
        .iter()
        .any(|(fallback, ranked)| !fallback && ranked.finding.severity() == Severity::Error);
    merged
        .into_iter()
        .filter(|(fallback, _)| !(*fallback && has_specific_error))
        .map(|(_, ranked)| ranked)
        .collect()
}

/// Peso a partir do qual o padrão aponta uma causa específica (dependência, Java, mixin…);
/// abaixo dele, o achado só descreve o travamento (crash report genérico, classe que falta).
const SPECIFIC_WEIGHT: u8 = 50;

fn sort_findings(findings: &mut [Ranked]) {
    findings.sort_by_key(|ranked| {
        (
            ranked.finding.severity(),
            ranked.weight < SPECIFIC_WEIGHT,
            std::cmp::Reverse(ranked.tier),
            std::cmp::Reverse(ranked.weight),
            ranked.order,
            ranked.line,
        )
    });
}

/// Achado "o jogo parou de responder", com a última linha escrita como evidência.
fn froze_finding(prepared: &[Prepared<'_>]) -> Option<Ranked> {
    let source = prepared
        .iter()
        .filter(|source| !source.source.kind().is_report())
        .min_by_key(|source| match source.source.kind() {
            SourceKind::Output => 0,
            SourceKind::LatestLog => 1,
            _ => 2,
        })?;
    let line = source
        .lines
        .iter()
        .rev()
        .find(|line| !line.text.trim().is_empty())?;
    let rule = RuleCode::parse("E_GAME_FROZE")?;
    let finding = Finding::new(
        rule,
        Severity::Error,
        "diagnostico.achados.jogoParou",
        Evidence::log(source.source.name(), line.number, &line.text),
    );
    Some(Ranked {
        pattern: FROZE_PATTERN.to_owned(),
        tier: catalog::PatternClass::Exception.tier(),
        weight: 50,
        order: source.order,
        line: line.number,
        finding,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(text: &str) -> Analysis {
        analyze_text("output.log", text, &AnalysisContext::default()).unwrap()
    }

    fn rules(analysis: &Analysis) -> Vec<&str> {
        analysis
            .findings
            .iter()
            .map(|entry| entry.finding.rule().as_str())
            .collect()
    }

    #[test]
    fn dependencia_do_neoforge_junta_as_duas_linhas() {
        let analysis = run(concat!(
            "[19:53:33] [main/ERROR] [ne.ne.fm.ModSorter/]: Missing or unsupported mandatory dependencies:\n",
            "\tMod ID: 'moonlight', Requested by: 'supplementaries', Expected range: '[1.21-3.6.4,]', Actual version: '[MISSING]'\n",
            "[19:53:34] [Render thread/FATAL] [ne.ne.fm.ModLoader/CORE]: Error during pre-loading phase: Mod supplementaries requires moonlight 1.21-3.6.4 or above\n",
            "Currently, moonlight is not installed\n",
            "[19:53:35] [Render thread/INFO] [x/]: NeoForge mod loading, version 21.1.252, for MC 1.21.1\n",
        ));
        assert_eq!(rules(&analysis), ["E_MISSING_DEP"]);
        let finding = &analysis.findings[0].finding;
        assert_eq!(finding.param("mod"), Some("supplementaries"));
        assert_eq!(finding.param("dependency"), Some("moonlight"));
        assert_eq!(finding.evidence().len(), 2);
        assert!(matches!(
            finding.primary_evidence(),
            Evidence::Log { line: 2, .. }
        ));
        assert_eq!(
            finding.fixes(),
            &[SuggestedFix::AddDependency {
                mod_id: "moonlight".into(),
                version_range: Some("[1.21-3.6.4,]".into())
            }]
        );
        assert_eq!(analysis.loader, Some(LogLoader::NeoForge));
        assert_eq!(analysis.minecraft.as_deref(), Some("1.21.1"));
        assert_eq!(analysis.loader_version.as_deref(), Some("21.1.252"));
        assert!(analysis.found_cause());
        assert_eq!(
            analysis.primary().unwrap().pattern,
            "forge.mandatory_dependency_missing"
        );
    }

    #[test]
    fn crash_report_com_causa_e_mods() {
        let analysis = run(concat!(
            "[12:00:00] [main/INFO]: Loading Minecraft 1.20.1 with Fabric Loader 0.19.5\n",
            "---- Minecraft Crash Report ----\n",
            "Description: Initializing game\n",
            "\n",
            "java.lang.IllegalStateException: falhou\n",
            "\tat knot//net.minecraft.class_310.handler$zza000$wardenfalhas$warden$iniciar(class_310.java:2996)\n",
            "Caused by: java.lang.NullPointerException: nulo\n",
            "\t... 3 more\n",
        ));
        assert_eq!(rules(&analysis), ["E_CRASH_REPORT"]);
        let finding = &analysis.findings[0].finding;
        assert_eq!(finding.param("description"), Some("Initializing game"));
        assert_eq!(
            finding.param("exception_type"),
            Some("java.lang.IllegalStateException")
        );
        assert_eq!(
            finding.param("cause_type"),
            Some("java.lang.NullPointerException")
        );
        assert_eq!(
            finding.items(),
            &[ItemRef::ModId {
                id: "wardenfalhas".into()
            }]
        );
    }

    #[test]
    fn debug_crash_anula_o_crash_report_e_genericos_saem_com_causa_especifica() {
        let analysis = run(concat!(
            "---- Minecraft Crash Report ----\n",
            "Description: Manually triggered debug crash\n",
            "\n",
            "java.lang.Throwable: Manually triggered debug crash\n",
        ));
        assert_eq!(rules(&analysis), ["I_DEBUG_CRASH"]);
        assert!(!analysis.found_cause());

        let analysis = run(concat!(
            "Incompatible mods found!\n",
            "\t - Mod 'Comforts' (comforts) 6.4.0+1.20.1 requires any version of fabric-api, which is missing!\n",
        ));
        assert_eq!(rules(&analysis), ["E_MISSING_DEP"]);
        let analysis = run("Incompatible mods found!\n");
        assert_eq!(rules(&analysis), ["E_LOADING_FAILED"]);
    }

    #[test]
    fn java_exigido_vem_da_versao_de_classe() {
        let analysis = run(
            "java.lang.UnsupportedClassVersionError: net/minecraft/client/main/Main has been compiled by a more recent version of the Java Runtime (class file version 61.0), this version of the Java Runtime only recognizes class file versions up to 52.0\n",
        );
        let finding = &analysis.findings[0].finding;
        assert_eq!(finding.param("java_required"), Some("17"));
        assert_eq!(
            finding.fixes(),
            &[SuggestedFix::ChangeJava { major: Some(17) }]
        );
        assert_eq!(java_for_class_version(65), 21);
    }

    #[test]
    fn escopo_de_loader_e_contexto_do_pack() {
        let text = "MissingModsException: Mod quark (Quark) requires [autoreglib@[1.3-32,)]\n";
        let fabric = AnalysisContext {
            loader: Some(LogLoader::Fabric),
            minecraft: Some("1.20.1".into()),
        };
        let analysis = analyze_text("x.log", text, &fabric).unwrap();
        assert!(analysis.findings.is_empty());
        assert_eq!(analysis.minecraft.as_deref(), Some("1.20.1"));
        let analysis = run(text);
        assert_eq!(rules(&analysis), ["E_MISSING_DEP"]);
        assert_eq!(
            analysis.findings[0].finding.param("range"),
            Some("[1.3-32,)")
        );
    }

    #[test]
    fn duplicados_sugerem_remover_a_copia() {
        let analysis = run(
            "Found a duplicate mod comforts at [C:\\x\\mods\\comforts-1.jar, C:\\x\\mods\\copia-comforts-1.jar]\n",
        );
        let finding = &analysis.findings[0].finding;
        assert_eq!(
            finding.items(),
            &[
                ItemRef::JarFile {
                    name: "comforts-1.jar".into()
                },
                ItemRef::JarFile {
                    name: "copia-comforts-1.jar".into()
                },
            ]
        );
        assert_eq!(
            finding.fixes(),
            &[SuggestedFix::RemoveItem {
                item: ItemRef::JarFile {
                    name: "copia-comforts-1.jar".into()
                }
            }]
        );
    }

    #[test]
    fn jogo_parado_aponta_a_ultima_linha() {
        let sources = [
            LogSource::from_text("crash-reports/x.txt", SourceKind::CrashReport, ""),
            LogSource::from_text(
                "output.log",
                SourceKind::Output,
                "a\nStarting integrated minecraft server version 1.21.1\n\n",
            ),
        ];
        let outcome = SessionOutcome {
            froze: true,
            ..SessionOutcome::default()
        };
        let analysis = analyze(&sources, &AnalysisContext::default(), Some(&outcome)).unwrap();
        assert_eq!(rules(&analysis), ["E_GAME_FROZE"]);
        assert_eq!(analysis.findings[0].pattern, FROZE_PATTERN);
        assert!(matches!(
            analysis.findings[0].finding.primary_evidence(),
            Evidence::Log { line: 2, .. }
        ));
        let empty = [LogSource::from_text("output.log", SourceKind::Output, "")];
        let analysis = analyze(&empty, &AnalysisContext::default(), Some(&outcome)).unwrap();
        assert!(analysis.findings.is_empty());
        assert_eq!(analysis.sources.len(), 1);
    }

    #[test]
    fn modelos_e_parametros() {
        let mut params = BTreeMap::new();
        params.insert("a".to_owned(), "x".to_owned());
        params.insert("vazio".to_owned(), String::new());
        assert_eq!(expand("{a}-{a}", &params).as_deref(), Some("x-x"));
        assert_eq!(expand("{vazio}", &params), None);
        assert_eq!(expand("{b}", &params), None);
        assert_eq!(expand("{a", &params), None);
        assert_eq!(expand("8", &params).as_deref(), Some("8"));
        assert_eq!(file_name("C:\\a\\b.jar"), "b.jar");
        assert_eq!(file_name(" /a/b.jar "), "b.jar");
        assert_eq!(jar_list("a.jar, /x/b.jar,"), ["a.jar", "b.jar"]);

        let mut derived = BTreeMap::new();
        derived.insert("class_version".to_owned(), "40".to_owned());
        derived.insert("exception".to_owned(), "java.lang.X: y".to_owned());
        derive_params(&mut derived);
        assert_eq!(derived.get("java_required"), None);
        assert_eq!(
            derived.get("exception_type").map(String::as_str),
            Some("java.lang.X")
        );

        let fixes = [
            FixTemplate::RemoveItem {
                mod_: Some("{a}".into()),
                jar: None,
                jars_after_first: None,
            },
            FixTemplate::UpdateItem {
                mod_: None,
                jar: Some("{a}".into()),
            },
            FixTemplate::SetSide {
                mod_: "{a}".into(),
                side: crate::model::SideChoice::Both,
            },
            FixTemplate::RemoveJvmArgument {
                argument: "{a}".into(),
            },
            FixTemplate::RestoreConfig { path: "{a}".into() },
            FixTemplate::ChangeJava {
                major: Some("{a}".into()),
            },
            FixTemplate::AddDependency {
                mod_id: "{b}".into(),
                version_range: None,
            },
        ];
        let expanded: Vec<_> = fixes
            .iter()
            .flat_map(|fix| expand_fix(fix, &params))
            .collect();
        assert_eq!(expanded.len(), 6);
        assert!(expanded.contains(&SuggestedFix::ChangeJava { major: None }));
        assert!(
            expand_item(
                &ItemTemplate {
                    mod_: None,
                    jar: None,
                    jars: None
                },
                &params
            )
            .is_empty()
        );
    }
}
