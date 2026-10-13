//! Assinatura de travamento (ARCHITECTURE §9.3 item 6 e §9.6; R5A §7.5; ROADMAP D-06).
//!
//! A assinatura junta o que identifica a **causa** de um travamento e deixa de fora o que muda
//! de uma vez para outra:
//! - o id do padrão do catálogo que deu o achado principal (`crash_report.ticking_entity`);
//! - o tipo da exceção (a causa mais profunda, quando há `Caused by:`);
//! - o primeiro frame de mod da pilha, normalizado: só classe e método, sem número de linha,
//!   módulo, jar, endereço, sufixos de lambda nem o prefixo e o id que o Mixin dá aos métodos
//!   injetados (`handler$zza00a$meumod$aoTick` → `meumod$aoTick`); numa queda nativa da JVM, o
//!   módulo do frame problemático sem o deslocamento;
//! - os mods citados pelo achado principal (ids; jars sem a versão no nome);
//! - o **sujeito** do achado: os parâmetros que o catálogo usa para identificá-lo (`key`: o mod e
//!   a dependência, a config de mixin, a classe…) e, no crash report, a descrição e o que estava
//!   sendo atualizado ("Ticking entity": o tipo e a classe da entidade; "Ticking block entity": o
//!   bloco e a classe). Sem isso, dois crash reports diferentes só com classes do jogo na pilha
//!   teriam a mesma assinatura.
//!
//! Mensagens, coordenadas, UUIDs, caminhos e horários nunca entram: dois travamentos com a mesma
//! causa e números diferentes têm o mesmo [`CrashSignature::hash`] (CA-T14-08). O hash é estável
//! entre versões do app enquanto [`SIGNATURE_VERSION`] não mudar.
//!
//! Limites: só lê a análise e as fontes já carregadas; não abre arquivos.

use std::collections::BTreeSet;
use std::ops::Range;
use std::sync::LazyLock;

use md5::{Digest as _, Md5};
use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::model::{Evidence, Finding, ItemRef};
use crate::postcrash::{
    Analysis, CleanLine, LogLoader, LogSource, SourceKind, crash_blocks, deepest_cause, mods_in,
    pattern_identity, stack_range, text_fixed_regex as fixed_regex,
};

/// Versão das regras de normalização. Mudar as regras muda os hashes: suba a versão.
///
/// 2: o sujeito do achado entra na assinatura; o id dos métodos injetados pelo Mixin sai em
/// todas as formas.
pub const SIGNATURE_VERSION: u32 = 2;

/// Até quantas linhas depois da evidência a exceção do achado é procurada.
const EXCEPTION_WITHIN: usize = 60;

/// Máximo de mods guardados na assinatura.
const MAX_MODS: usize = 8;

/// Linha de exceção (`java.lang.NullPointerException: …`, `Caused by: …`).
static EXCEPTION_LINE: LazyLock<Regex> = LazyLock::new(|| {
    fixed_regex(
        r"^\s*(?:Caused by(?: \d+)?: |Exception in thread .*? )?(?P<type>(?:[A-Za-z_$][\w$]*\.)+[\w$]*(?:Exception|Error|Throwable)[\w$]*)(?::|\s*$)",
    )
});

/// Frame Java: `at [módulo/]pacote.Classe.método(Arquivo.java:12) [~jar]`.
static JAVA_FRAME: LazyLock<Regex> = LazyLock::new(|| {
    fixed_regex(r"^\s*at\s+(?:[^\s/(]*/)*(?P<method>[\w$.<>]+)\((?P<location>[^)]*)\)(?P<rest>.*)$")
});

/// Frame nativo do `hs_err`: `C  [ig9icd64.dll+0x8599ee]`, `J 123 C2 pacote.Classe.m()V …`.
static NATIVE_FRAME: LazyLock<Regex> = LazyLock::new(|| {
    fixed_regex(
        r"^#?\s*(?P<kind>[CJjVv])\s+(?:\[(?P<module>[^\]+]+)|(?:\d+\s+)?(?:C\d\s+)?(?P<method>[\w$.<>/]+))",
    )
});

/// Classes geradas para lambdas (`$$Lambda$123/0x0000…`, `$$Lambda/0x…`).
static LAMBDA_CLASS: LazyLock<Regex> =
    LazyLock::new(|| fixed_regex(r"\$\$Lambda(?:\$\d+)?(?:/0x[0-9a-fA-F]+)?"));

/// Métodos de lambda (`lambda$tick$12`, `lambda$3`; do Kotlin, `aoTick$lambda-3`) e
/// acessores sintéticos (`access$000`): o número muda com a compilação.
static LAMBDA_METHOD: LazyLock<Regex> =
    LazyLock::new(|| fixed_regex(r"(lambda(?:\$[A-Za-z_][\w]*?)?)[$-]\d+\b|(access\$)\d+\b"));

/// Sufixo dos métodos fundidos pelo Mixin (`$md$1a2b3c$0`).
static MIXIN_MERGED: LazyLock<Regex> = LazyLock::new(|| fixed_regex(r"\$md\$[0-9a-f]+\$\d+"));

/// Prefixos que o Mixin e o `MixinExtras` dão aos métodos injetados (`handler$…`, `redirect$…`).
const MIXIN_PREFIXES: [&str; 15] = [
    "handler",
    "redirect",
    "modify",
    "localvar",
    "constant",
    "args",
    "wrapOperation",
    "wrapWithCondition",
    "wrapMethod",
    "modifyExpressionValue",
    "modifyReturnValue",
    "modifyReceiver",
    "modifyVariable",
    "modifyArg",
    "modifyArgs",
];

/// O id que o Mixin dá a cada método injetado (`zza000`, `zza00a`, `bkb1f3`): três letras e um
/// número em hexadecimal. Muda de uma sessão para outra conforme a ordem em que as classes
/// carregam.
static MIXIN_ID: LazyLock<Regex> = LazyLock::new(|| fixed_regex(r"^[a-z]{3}[0-9a-f]{3,}$"));

/// Classes geradas com contador (`ASMEventHandler_12_…`, `GeneratedMethodAccessor34`,
/// `$Proxy56`).
static GENERATED_CLASS: LazyLock<Regex> = LazyLock::new(|| {
    fixed_regex(
        r"(ASMEventHandler_)\d+_|(Generated(?:Method|Constructor|SerializationConstructor)Accessor)\d+|(\$Proxy)\d+",
    )
});

/// Descrição do crash report.
static DESCRIPTION: LazyLock<Regex> =
    LazyLock::new(|| fixed_regex(r"^\s*Description:\s*(?P<text>\S.*?)\s*$"));

/// A descrição de um crash report de entidade ou de bloco que quebrou ao ser atualizado.
static TICKING: LazyLock<Regex> =
    LazyLock::new(|| fixed_regex(r"(?i)^ticking (?:(?P<block>block |tile )?entity|player)$"));

/// Entidade do "Ticking entity": `Entity Type: alpha:golem (com.alpha.Golem)`.
static ENTITY_TYPE: LazyLock<Regex> = LazyLock::new(|| {
    fixed_regex(r"^\s*Entity Type:\s*(?P<kind>[^\s(]+)(?:\s*\((?P<class>[^)\s]+)\))?")
});

/// Bloco do "Ticking block entity": `Name: minecraft:furnace // net.minecraft….FurnaceBlockEntity`.
static BLOCK_ENTITY: LazyLock<Regex> =
    LazyLock::new(|| fixed_regex(r"^\s*Name:\s*(?P<kind>[^\s/]+)(?:\s*//\s*(?P<class>\S+))?"));

/// Números (contadores, coordenadas, ids) numa descrição ou argumento.
static NUMBERS: LazyLock<Regex> = LazyLock::new(|| fixed_regex(r"0x[0-9a-fA-F]+|\d+(?:[.,]\d+)*"));

/// Sufixo de versão no nome de um jar (`-1.20.1-0.5.1.f`, `_1.2`, `+mc1.20`).
static JAR_VERSION: LazyLock<Regex> = LazyLock::new(|| fixed_regex(r"[-_+](?:mc|v)?\d.*$|%23\d+$"));

/// Pacotes do jogo, do loader, do Java e de bibliotecas: frames que não são de mod.
const NOT_MOD_PACKAGES: [&str; 30] = [
    "java.",
    "javax.",
    "jdk.",
    "sun.",
    "com.sun.",
    "net.minecraft.",
    "com.mojang.",
    "net.minecraftforge.",
    "net.neoforged.",
    "cpw.mods.",
    "net.fabricmc.",
    "org.quiltmc.",
    "org.spongepowered.",
    "com.llamalad7.mixinextras.",
    "org.lwjgl.",
    "io.netty.",
    "com.google.",
    "org.apache.",
    "it.unimi.",
    "org.objectweb.",
    "kotlin.",
    "scala.",
    "org.slf4j.",
    "oshi.",
    "org.prismlauncher.",
    "org.multimc.",
    "org.jline.",
    "com.electronwill.",
    "net.jodah.",
    "org.openjdk.",
];

/// A assinatura de um travamento.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct CrashSignature {
    /// [`SIGNATURE_VERSION`] das regras que calcularam o hash.
    pub version: u32,
    /// Hash estável das partes abaixo (16 dígitos hexadecimais). Agrupa os travamentos.
    pub hash: String,
    /// Id do padrão do catálogo do achado principal (`None`: nenhum padrão reconheceu).
    pub rule: Option<String>,
    /// Tipo da exceção (`java.lang.NullPointerException`).
    pub exception: Option<String>,
    /// Primeiro frame de mod, normalizado (`com.example.Foo.tick`).
    pub frame: Option<String>,
    /// Mods citados, normalizados e em ordem (`mod:balm`, `jar:create`).
    pub mods: Vec<String>,
    /// O sujeito do achado, normalizado (`description=rendering overlay`,
    /// `entity_type=alpha:golem`, `entity_class=com.alpha.Golem`, `mixin_config=x.mixins.json`).
    #[serde(default)]
    pub subject: Vec<String>,
}

impl CrashSignature {
    /// Monta a assinatura com as partes já normalizadas e calcula o hash.
    #[must_use]
    pub fn new(
        rule: Option<String>,
        exception: Option<String>,
        frame: Option<String>,
        mods: Vec<String>,
        subject: Vec<String>,
    ) -> Self {
        let mods: Vec<String> = mods
            .into_iter()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .take(MAX_MODS)
            .collect();
        let mut hasher = Md5::new();
        hasher.update(format!("v{SIGNATURE_VERSION}").as_bytes());
        for part in [&rule, &exception, &frame] {
            hasher.update([0x1e]);
            hasher.update(part.as_deref().unwrap_or_default().as_bytes());
        }
        for name in &mods {
            hasher.update([0x1f]);
            hasher.update(name.as_bytes());
        }
        for part in &subject {
            hasher.update([0x1d]);
            hasher.update(part.as_bytes());
        }
        let digest = hasher.finalize();
        let hash = digest[..8]
            .iter()
            .fold(String::with_capacity(16), |mut out, byte| {
                use std::fmt::Write as _;
                let _ = write!(out, "{byte:02x}");
                out
            });
        Self {
            version: SIGNATURE_VERSION,
            hash,
            rule,
            exception,
            frame,
            mods,
            subject,
        }
    }
}

/// A assinatura do travamento analisado. `sources` são as mesmas fontes passadas a
/// [`crate::analyze`] (a pilha é lida delas, a partir da evidência do achado principal).
#[must_use]
pub fn signature(analysis: &Analysis, sources: &[LogSource]) -> CrashSignature {
    let fabric = matches!(analysis.loader, Some(LogLoader::Fabric | LogLoader::Quilt));
    let Some(primary) = analysis.primary() else {
        let (exception, frame) = first_crash_block(sources, fabric).unwrap_or_default();
        let subject = report_subject(sources, None);
        return CrashSignature::new(None, exception, frame, Vec::new(), subject);
    };
    let finding = &primary.finding;
    let at = finding
        .evidence()
        .iter()
        .find_map(|evidence| match evidence {
            Evidence::Log { file, line, .. } => sources
                .iter()
                .find(|source| source.name() == file)
                .map(|source| (source, *line)),
            _ => None,
        });
    let (scanned_exception, frame) = at
        .and_then(|(source, line)| stack_at(source, line, fabric))
        .unwrap_or_default();
    let exception = finding
        .param("cause_type")
        .or_else(|| finding.param("exception_type"))
        .map(ToOwned::to_owned)
        .or(scanned_exception);
    let mods = finding.items().iter().map(normalize_item).collect();
    let (key, generic) = pattern_identity(&primary.pattern);
    let mut subject = key_subject(finding, key);
    // O crash report diz o que quebrou quando o achado é dele ou é genérico sem sujeito próprio.
    // Um achado de causa certa (falta de memória, Java errado…) não depende de onde quebrou.
    if primary.pattern.starts_with("crash_report.") || (generic && subject.is_empty()) {
        for part in report_subject(sources, at) {
            if !subject.contains(&part) {
                subject.push(part);
            }
        }
    }
    // `server.ticking_entity`: se foi entidade ou bloco.
    if let Some(what) = finding.param("what") {
        subject.push(format!("what={}", normalize_param("what", what)));
    }
    CrashSignature::new(
        Some(primary.pattern.clone()),
        exception,
        frame,
        mods,
        subject,
    )
}

/// Os parâmetros que o catálogo usa para identificar o achado (`key`), normalizados. O tipo da
/// exceção já tem lugar próprio na assinatura.
fn key_subject(finding: &Finding, key: &[String]) -> Vec<String> {
    key.iter()
        .map(String::as_str)
        .filter(|name| !matches!(*name, "exception_type" | "cause_type"))
        .filter_map(|name| {
            let value = normalize_param(name, finding.param(name)?);
            (!value.is_empty()).then(|| format!("{name}={value}"))
        })
        .collect()
}

/// Um parâmetro do achado sem as partes que mudam de uma vez para outra.
fn normalize_param(name: &str, value: &str) -> String {
    let value = value.trim();
    match name {
        "file" => jar_stem(value),
        "class" | "entity_class" | "block_class" => normalize_class(value),
        "mod" | "dependency" | "other" => value.to_lowercase().replace('-', "_"),
        "description" | "argument" | "what" => {
            let lower = value.to_lowercase();
            let masked = NUMBERS.replace_all(&lower, "#");
            masked.split_whitespace().collect::<Vec<_>>().join(" ")
        }
        _ => value.to_lowercase(),
    }
}

/// O sujeito do crash report do travamento: o bloco que contém `at` (a evidência do achado) ou,
/// se ela não está num crash report, o primeiro crash report das fontes (os arquivos de crash
/// report antes dos logs).
fn report_subject(sources: &[LogSource], at: Option<(&LogSource, u32)>) -> Vec<String> {
    let from_evidence = at.and_then(|(source, number)| {
        let lines = source.clean_lines();
        let index = lines.iter().position(|line| line.number == number)?;
        let block = crash_blocks(&lines, source.kind() == SourceKind::CrashReport)
            .into_iter()
            .find(|block| block.contains(&index))?;
        Some(block_subject(&lines, block))
    });
    from_evidence
        .or_else(|| {
            let mut ordered: Vec<&LogSource> = sources.iter().collect();
            ordered.sort_by_key(|source| source.kind());
            ordered.into_iter().find_map(|source| {
                if source.kind() == SourceKind::JvmCrash {
                    return None;
                }
                let lines = source.clean_lines();
                let block = crash_blocks(&lines, source.kind() == SourceKind::CrashReport)
                    .into_iter()
                    .next()?;
                Some(block_subject(&lines, block))
            })
        })
        .unwrap_or_default()
}

/// A descrição de um bloco de crash report e, quando uma entidade ou um bloco quebrou ao ser
/// atualizado, o tipo e a classe dele.
fn block_subject(lines: &[CleanLine], block: Range<usize>) -> Vec<String> {
    let mut parts = Vec::new();
    let Some((at, description)) = block.clone().find_map(|at| {
        DESCRIPTION
            .captures(&lines[at].text)
            .and_then(|captures| captures.name("text"))
            .map(|text| (at, text.as_str().to_owned()))
    }) else {
        return parts;
    };
    parts.push(format!(
        "description={}",
        normalize_param("description", &description)
    ));
    let Some(ticking) = TICKING.captures(description.trim()) else {
        return parts;
    };
    let (pattern, kind, class) = if ticking.name("block").is_some() {
        (&*BLOCK_ENTITY, "block_entity", "block_class")
    } else {
        (&*ENTITY_TYPE, "entity_type", "entity_class")
    };
    if let Some(captures) = (at + 1..block.end).find_map(|line| pattern.captures(&lines[line].text))
    {
        if let Some(value) = captures.name("kind") {
            parts.push(format!("{kind}={}", normalize_param(kind, value.as_str())));
        }
        if let Some(value) = captures.name("class") {
            parts.push(format!(
                "{class}={}",
                normalize_param(class, value.as_str())
            ));
        }
    }
    parts
}

/// Exceção e primeiro frame de mod da pilha que começa na linha `number` (ou logo depois).
fn stack_at(
    source: &LogSource,
    number: u32,
    fabric: bool,
) -> Option<(Option<String>, Option<String>)> {
    let lines = source.clean_lines();
    let index = lines.iter().position(|line| line.number == number)?;
    if source.kind() == SourceKind::JvmCrash {
        return Some((None, native_frame(&lines, index)));
    }
    Some(exception_and_frame(&lines, index, fabric))
}

/// Sem achado: a exceção e o frame do primeiro crash report (os arquivos de crash report vêm
/// antes dos logs, como na análise).
fn first_crash_block(
    sources: &[LogSource],
    fabric: bool,
) -> Option<(Option<String>, Option<String>)> {
    let mut ordered: Vec<&LogSource> = sources.iter().collect();
    ordered.sort_by_key(|source| source.kind());
    ordered.into_iter().find_map(|source| {
        let lines = source.clean_lines();
        if source.kind() == SourceKind::JvmCrash {
            let index = lines
                .iter()
                .position(|line| line.text.contains("Problematic frame"))?;
            return Some((None, native_frame(&lines, index)));
        }
        let block = crash_blocks(&lines, source.kind() == SourceKind::CrashReport)
            .into_iter()
            .next()?;
        Some(exception_and_frame(&lines, block.start, fabric))
    })
}

/// A exceção depois de `index` (sem passar para outro registro do log) e o primeiro frame de
/// mod da pilha dela.
fn exception_and_frame(
    lines: &[CleanLine],
    index: usize,
    fabric: bool,
) -> (Option<String>, Option<String>) {
    let end = lines.len().min(index + EXCEPTION_WITHIN);
    let start = (index..end).find(|&at| {
        (at == index || !lines[at].record_start) && EXCEPTION_LINE.is_match(&lines[at].text)
    });
    let Some(start) = start else {
        return (None, None);
    };
    let stack = stack_range(lines, start);
    let exception = deepest_cause(lines, stack.clone())
        .as_deref()
        .and_then(exception_type)
        .or_else(|| exception_type(&lines[start].text));
    (exception, first_mod_frame(lines, stack, fabric))
}

/// O tipo de exceção de uma linha.
fn exception_type(text: &str) -> Option<String> {
    EXCEPTION_LINE
        .captures(text)
        .and_then(|captures| captures.name("type"))
        .map(|kind| kind.as_str().to_owned())
}

/// O primeiro frame de mod: primeiro na seção da causa mais profunda (o último `Caused by:`),
/// depois nas seções de cima.
fn first_mod_frame(lines: &[CleanLine], stack: Range<usize>, fabric: bool) -> Option<String> {
    let mut starts: Vec<usize> = stack
        .clone()
        .filter(|&at| lines[at].text.trim_start().starts_with("Caused by"))
        .collect();
    starts.insert(0, stack.start);
    let mut sections = Vec::with_capacity(starts.len());
    for (position, &start) in starts.iter().enumerate() {
        let end = starts.get(position + 1).copied().unwrap_or(stack.end);
        sections.push(start..end);
    }
    sections.into_iter().rev().find_map(|section| {
        section
            .into_iter()
            .find_map(|at| mod_frame(lines, at, fabric))
    })
}

/// O frame normalizado, se a linha `at` é um frame de mod: cita um mod (módulo, jar ou handler
/// de mixin do Fabric) ou, sem módulo nem jar na linha, não é de um pacote do jogo, do loader ou
/// de biblioteca.
fn mod_frame(lines: &[CleanLine], at: usize, fabric: bool) -> Option<String> {
    let text = LAMBDA_CLASS.replace_all(&lines[at].text, "$$$$Lambda");
    let captures = JAVA_FRAME.captures(&text)?;
    let method = captures.name("method")?.as_str();
    let annotated = text.contains(".jar") || text.contains('@');
    let mentions = !mods_in(lines, at..at + 1, fabric).is_empty();
    let library = NOT_MOD_PACKAGES
        .iter()
        .any(|prefix| method.starts_with(prefix));
    (mentions || (!annotated && !library)).then(|| normalize_method(method))
}

/// Classe e método sem as partes que mudam a cada execução: o número das lambdas e das classes
/// geradas, e o prefixo e o id dos métodos injetados pelo Mixin (o mod, quando o nome traz, e o
/// método ficam: `Classe.handler$zza00a$meumod$aoTick` → `Classe.meumod$aoTick`).
fn normalize_method(method: &str) -> String {
    let method = normalize_class(method);
    let method = MIXIN_MERGED.replace_all(&method, "$$");
    let (owner, name) = match method.rsplit_once('.') {
        Some((owner, name)) => (Some(owner), name),
        None => (None, method.as_ref()),
    };
    let name = match name.split_once('$') {
        Some((prefix, rest)) if MIXIN_PREFIXES.contains(&prefix) => {
            // O id vem logo depois do prefixo (`handler$zza00a$…`); sem id, o resto fica.
            match rest.split_once('$') {
                Some((id, after)) if is_mixin_id(id) => after.to_owned(),
                _ => rest.to_owned(),
            }
        }
        _ => name.to_owned(),
    };
    match owner {
        Some(owner) => format!("{owner}.{name}"),
        None => name,
    }
}

/// Se o trecho é o id de um método injetado: o formato do Mixin, com pelo menos um algarismo
/// (um id de mod como `bigbeef` fica).
fn is_mixin_id(segment: &str) -> bool {
    MIXIN_ID.is_match(segment) && segment.bytes().any(|byte| byte.is_ascii_digit())
}

/// Classe (ou classe e método) sem os números de lambdas, acessores e classes geradas.
fn normalize_class(text: &str) -> String {
    let text = LAMBDA_CLASS.replace_all(text, "$$$$Lambda");
    let text = LAMBDA_METHOD.replace_all(&text, "$1$2");
    GENERATED_CLASS.replace_all(&text, "$1$2$3").into_owned()
}

/// O frame problemático de um `hs_err`: a partir de `index`, a linha depois de
/// `# Problematic frame:` (ou a própria linha da evidência), sem endereços.
fn native_frame(lines: &[CleanLine], index: usize) -> Option<String> {
    let header = (index.saturating_sub(1)..lines.len().min(index + 20))
        .find(|&at| lines[at].text.contains("Problematic frame"));
    let at = header.map_or(index, |header| header + 1);
    let text = lines.get(at)?.text.trim_start_matches('#').trim();
    let captures = NATIVE_FRAME.captures(text)?;
    let kind = captures.name("kind")?.as_str();
    let place = captures
        .name("module")
        .or_else(|| captures.name("method"))?
        .as_str()
        .trim();
    Some(format!("{kind} {}", normalize_method(place)))
}

/// Um mod citado, normalizado.
fn normalize_item(item: &ItemRef) -> String {
    match item {
        ItemRef::ModId { id } => format!("mod:{}", id.trim().to_lowercase().replace('-', "_")),
        ItemRef::JarFile { name } => format!("jar:{}", jar_stem(name)),
        ItemRef::Metafile { path } => format!("item:{}", path.trim()),
    }
}

/// O nome do jar sem pastas, extensão e versão (`create-1.20.1-0.5.1.f.jar` → `create`).
fn jar_stem(name: &str) -> String {
    let file = name.rsplit(['/', '\\']).next().unwrap_or(name);
    let lower = file.trim().to_lowercase();
    let base = lower.strip_suffix(".jar").unwrap_or(&lower);
    let base = base.split("%23").next().unwrap_or(base);
    let stem = JAR_VERSION.replace(base, "");
    if stem.is_empty() {
        base.to_owned()
    } else {
        stem.into_owned()
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};

    use super::*;
    use crate::postcrash::{AnalysisContext, analyze};

    fn source(name: &str, kind: SourceKind, text: &str) -> LogSource {
        LogSource::from_text(name, kind, text)
    }

    fn of(sources: &[LogSource]) -> CrashSignature {
        let analysis = analyze(sources, &AnalysisContext::default(), None).unwrap();
        signature(&analysis, sources)
    }

    // ---- Corpus de logs reais (`tests/corpus/`, da D-02) ------------------------------------

    fn corpus_dir() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("corpus")
    }

    fn kind_of(relative: &str) -> SourceKind {
        let name = relative.rsplit('/').next().unwrap_or(relative);
        if relative.starts_with("crash-reports/") {
            SourceKind::CrashReport
        } else if name.starts_with("hs_err_pid") {
            SourceKind::JvmCrash
        } else if name == "output.log" {
            SourceKind::Output
        } else if name == "latest.log" {
            SourceKind::LatestLog
        } else if name == "debug.log" || name.starts_with("fml-") {
            SourceKind::DebugLog
        } else {
            SourceKind::Text
        }
    }

    /// Os arquivos de um caso: (nome relativo, bytes).
    fn files_of(case: &Path) -> Vec<(String, Vec<u8>)> {
        fn walk(dir: &Path, base: &Path, out: &mut Vec<(String, Vec<u8>)>) {
            for entry in fs::read_dir(dir).unwrap() {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    walk(&path, base, out);
                    continue;
                }
                let relative = path
                    .strip_prefix(base)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/");
                if relative != "esperado.toml" && relative != ".gitattributes" {
                    out.push((relative, fs::read(&path).unwrap()));
                }
            }
        }
        let mut files = Vec::new();
        walk(case, case, &mut files);
        files.sort();
        files
    }

    fn sources_from(files: &[(String, Vec<u8>)]) -> Vec<LogSource> {
        files
            .iter()
            .map(|(name, bytes)| LogSource::from_bytes(name.clone(), kind_of(name), bytes))
            .collect()
    }

    fn corpus_cases() -> Vec<PathBuf> {
        let mut cases: Vec<PathBuf> = fs::read_dir(corpus_dir())
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .filter(|path| path.is_dir())
            .collect();
        cases.sort();
        cases
    }

    /// O mesmo travamento outra vez: números de linha das pilhas, índices de jar, endereços,
    /// coordenadas, horários, UUIDs, lambdas e prefixos de mixin diferentes, e o log com linhas
    /// a mais antes do travamento (a evidência cai em outra linha do arquivo).
    fn another_run(name: &str, text: &str) -> String {
        static JAVA_LINE: LazyLock<Regex> = LazyLock::new(|| fixed_regex(r"\.java:(\d+)\)"));
        static JAR_INDEX: LazyLock<Regex> = LazyLock::new(|| fixed_regex(r"%23(\d+)"));
        static HEX: LazyLock<Regex> = LazyLock::new(|| fixed_regex(r"0x[0-9a-fA-F]+"));
        static LAMBDA: LazyLock<Regex> =
            LazyLock::new(|| fixed_regex(r"(\$\$Lambda\$|lambda\$\w*?\$)(\d+)"));
        static MIXIN: LazyLock<Regex> = LazyLock::new(|| fixed_regex(r"\$[a-z]{3}[0-9a-f]{3,}\$"));
        static UUID: LazyLock<Regex> = LazyLock::new(|| {
            fixed_regex(r"[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}")
        });
        static CLOCK: LazyLock<Regex> =
            LazyLock::new(|| fixed_regex(r"\b\d{2}:\d{2}:\d{2}(?:\.\d+)?\b"));
        static NUMBER: LazyLock<Regex> = LazyLock::new(|| fixed_regex(r"-?\d+(?:\.\d+)?"));

        let bump = |captures: &regex::Captures<'_>, add: u64| {
            captures[1].parse::<u64>().unwrap_or(0) + add
        };
        let text = JAVA_LINE.replace_all(text, |c: &regex::Captures<'_>| {
            format!(".java:{})", bump(c, 17))
        });
        let text = JAR_INDEX.replace_all(&text, |c: &regex::Captures<'_>| {
            format!("%23{}", bump(c, 5))
        });
        let text = HEX.replace_all(&text, "0x7ffd3c2a10");
        let text = LAMBDA.replace_all(&text, |c: &regex::Captures<'_>| {
            format!("{}{}", &c[1], c[2].parse::<u64>().unwrap_or(0) + 3)
        });
        let text = MIXIN.replace_all(&text, "$$qqx0af$$");
        let text = UUID.replace_all(&text, "0f0e0d0c-0b0a-0908-0706-050403020100");
        let text = CLOCK.replace_all(&text, "23:59:58");
        let mut out = String::new();
        if matches!(kind_of(name), SourceKind::Output | SourceKind::LatestLog) {
            if text.contains("<log4j:Event") {
                out.push_str(concat!(
                    "<log4j:Event logger=\"x\" timestamp=\"1\" level=\"INFO\" thread=\"main\">\n",
                    "  <log4j:Message><![CDATA[linha a mais]]></log4j:Message>\n",
                    "</log4j:Event>\n",
                ));
            } else {
                out.push_str(
                    "[00:00:01] [main/INFO]: linha a mais\n[00:00:02] [main/INFO]: outra\n",
                );
            }
        }
        for line in text.lines() {
            let lower = line.to_lowercase();
            if lower.contains("location") || lower.contains("time:") || lower.contains("pos=") {
                out.push_str(&NUMBER.replace_all(line, "42.5"));
            } else {
                out.push_str(line);
            }
            out.push('\n');
        }
        out
    }

    #[test]
    fn ca_t14_08_corpus_o_mesmo_travamento_com_numeros_diferentes_tem_a_mesma_assinatura() {
        let mut checked = 0;
        let mut with_frame = 0;
        for case in corpus_cases() {
            let files = files_of(&case);
            let original = of(&sources_from(&files));
            if original.rule.is_none() {
                continue;
            }
            let changed: Vec<(String, Vec<u8>)> = files
                .iter()
                .map(|(name, bytes)| {
                    let text = String::from_utf8_lossy(bytes);
                    (name.clone(), another_run(name, &text).into_bytes())
                })
                .collect();
            let again = of(&sources_from(&changed));
            assert_eq!(original, again, "{}", case.display());
            checked += 1;
            with_frame += usize::from(original.frame.is_some());
        }
        // O corpus tem dezenas de travamentos; muitos com frame de mod na pilha.
        assert!(checked >= 25, "casos conferidos: {checked}");
        assert!(with_frame >= 5, "casos com frame de mod: {with_frame}");
    }

    #[test]
    fn ca_t14_08_corpus_causas_diferentes_tem_assinaturas_diferentes() {
        let mut seen: std::collections::HashMap<String, (String, CrashSignature)> =
            std::collections::HashMap::new();
        for case in corpus_cases() {
            let name = case.file_name().unwrap().to_string_lossy().into_owned();
            let signature = of(&sources_from(&files_of(&case)));
            if signature.rule.is_none() {
                continue;
            }
            if let Some((other, previous)) = seen.get(&signature.hash) {
                // Só pode coincidir quando as partes são as mesmas (mesma causa em logs
                // diferentes), nunca por colisão.
                assert_eq!(
                    (
                        &previous.rule,
                        &previous.exception,
                        &previous.frame,
                        &previous.mods,
                        &previous.subject
                    ),
                    (
                        &signature.rule,
                        &signature.exception,
                        &signature.frame,
                        &signature.mods,
                        &signature.subject
                    ),
                    "{other} × {name}"
                );
            }
            seen.insert(signature.hash.clone(), (name, signature));
        }
        // Os dois "Ticking entity" do corpus são de mods diferentes.
        let ticking: Vec<&CrashSignature> = seen
            .values()
            .map(|(_, signature)| signature)
            .filter(|signature| signature.rule.as_deref() == Some("crash_report.ticking_entity"))
            .collect();
        assert!(ticking.len() >= 2, "{ticking:?}");
    }

    #[test]
    fn corpus_partes_da_assinatura_em_casos_conhecidos() {
        let case = |name: &str| of(&sources_from(&files_of(&corpus_dir().join(name))));

        let ticking = case("codex-forge-ticking-entity-1-18-2");
        assert_eq!(ticking.rule.as_deref(), Some("crash_report.ticking_entity"));
        assert_eq!(ticking.mods, ["jar:village-employment"]);
        assert!(ticking.exception.is_some());

        let native = case("github-hs-err-driver-intel");
        assert_eq!(
            native.rule.as_deref(),
            Some("graphics.native_frame_in_driver")
        );
        assert_eq!(native.frame.as_deref(), Some("C ig9icd64.dll"));
    }

    // ---- Normalização --------------------------------------------------------------------

    const NPE_REPORT: &str = "---- Minecraft Crash Report ----
// Who set us up the TNT?

Time: 2026-10-13 12:00:01
Description: Ticking entity

java.lang.NullPointerException: Cannot invoke \"Object.hashCode()\" because \"<local3>\" is null
\tat com.example.meumod.EntityTicker.lambda$tick$12(EntityTicker.java:41) ~[meumod-1.0.jar%23101!/:?]
\tat net.minecraft.world.level.Level.guardEntityTick(Level.java:501) ~[client-1.20.1-20230612.114412-srg.jar%23185!/:?]

-- Entity being ticked --
Details:
\tEntity Type: meumod:golem (com.example.meumod.Golem)
\tEntity's Exact location: 13.37, 64.00, -7.10
";

    #[test]
    fn numeros_mudam_assinatura_nao() {
        let first = of(&[source(
            "crash-reports/crash-1.txt",
            SourceKind::CrashReport,
            NPE_REPORT,
        )]);
        let second_text = NPE_REPORT
            .replace("EntityTicker.java:41", "EntityTicker.java:97")
            .replace("lambda$tick$12", "lambda$tick$3")
            .replace("%23101", "%2377")
            .replace("13.37, 64.00, -7.10", "-512.00, 70.00, 1024.50")
            .replace("12:00:01", "18:44:09");
        let second = of(&[source(
            "crash-reports/crash-2.txt",
            SourceKind::CrashReport,
            &second_text,
        )]);
        assert_eq!(first, second);
        assert_eq!(first.rule.as_deref(), Some("crash_report.ticking_entity"));
        assert_eq!(
            first.exception.as_deref(),
            Some("java.lang.NullPointerException")
        );
        assert_eq!(
            first.frame.as_deref(),
            Some("com.example.meumod.EntityTicker.lambda$tick")
        );
        assert_eq!(first.mods, ["jar:meumod"]);
        assert_eq!(first.hash.len(), 16);
        assert_eq!(first.version, SIGNATURE_VERSION);

        // Outro mod na pilha: outra assinatura.
        let other = NPE_REPORT.replace("com.example.meumod.EntityTicker", "net.outro.Ticker");
        let other = of(&[source(
            "crash-reports/crash-3.txt",
            SourceKind::CrashReport,
            &other,
        )]);
        assert_ne!(first.hash, other.hash);
    }

    // ---- Sujeito do achado (falsos agrupamentos da 1ª revisão) ---------------------------

    /// Crash report com `NullPointerException` e pilha só do jogo.
    fn game_only_report(description: &str, details: &str) -> String {
        format!(
            "---- Minecraft Crash Report ----
// Oops.

Time: 2026-10-13 12:00:01
Description: {description}

java.lang.NullPointerException: Cannot invoke \"Object.hashCode()\" because \"<local3>\" is null
\tat net.minecraft.world.level.Level.guardEntityTick(Level.java:501) ~[client-1.20.1-srg.jar%23185!/:?]
\tat net.minecraft.server.MinecraftServer.tickServer(MinecraftServer.java:880) ~[client-1.20.1-srg.jar%23185!/:?]

{details}
-- System Details --
Details:
\tMinecraft Version: 1.20.1
"
        )
    }

    fn report_signature(name: &str, text: &str) -> CrashSignature {
        of(&[source(
            &format!("crash-reports/{name}.txt"),
            SourceKind::CrashReport,
            text,
        )])
    }

    #[test]
    fn descricoes_diferentes_com_pilha_so_do_jogo_tem_assinaturas_diferentes() {
        let overlay = report_signature(
            "a",
            &game_only_report("Rendering overlay", "-- Overlay render details --\n"),
        );
        let tick = report_signature(
            "b",
            &game_only_report("Exception in server tick loop", "-- Server --\n"),
        );
        // Na ponta da 1ª revisão as duas davam 6fa9d7e2a146d61a.
        assert_eq!(overlay.rule.as_deref(), Some("crash_report.description"));
        assert_eq!(tick.rule.as_deref(), Some("crash_report.description"));
        assert_eq!(overlay.frame, None);
        assert!(overlay.mods.is_empty());
        assert_ne!(overlay.hash, tick.hash);
        assert_eq!(overlay.subject, ["description=rendering overlay"]);
        assert_eq!(tick.subject, ["description=exception in server tick loop"]);

        // A mesma descrição com números diferentes continua junta.
        let again = report_signature(
            "c",
            &game_only_report("Rendering overlay", "-- Overlay render details --\n")
                .replace("Level.java:501", "Level.java:530")
                .replace("%23185", "%23190")
                .replace("12:00:01", "21:13:09"),
        );
        assert_eq!(overlay, again);
    }

    fn ticking_entity(kind: &str, class: &str, location: &str) -> String {
        game_only_report(
            "Ticking entity",
            &format!(
                "-- Entity being ticked --
Details:
\tEntity Type: {kind} ({class})
\tEntity ID: 4817
\tEntity Name: {kind}
\tEntity's Exact location: {location}
\tEntity's Block location: World: (13,64,-8), Section: (at 13,0,8 in 0,4,-1)
"
            ),
        )
    }

    #[test]
    fn entidades_diferentes_com_pilha_so_do_jogo_tem_assinaturas_diferentes() {
        let golem = report_signature(
            "a",
            &ticking_entity("alpha:golem", "com.alpha.Golem", "13.37, 64.00, -7.10"),
        );
        let wolf = report_signature(
            "b",
            &ticking_entity("beta:wolf", "com.beta.Wolf", "13.37, 64.00, -7.10"),
        );
        // Na ponta da 1ª revisão as duas davam 5845882d291a584d.
        assert_eq!(golem.rule.as_deref(), Some("crash_report.ticking_entity"));
        assert_eq!(golem.frame, None);
        assert_ne!(golem.hash, wolf.hash);
        assert_eq!(
            golem.subject,
            [
                "entity_type=alpha:golem",
                "description=ticking entity",
                "entity_class=com.alpha.Golem",
            ]
        );

        // A mesma entidade em outro lugar e com outro id: a mesma assinatura.
        let elsewhere = report_signature(
            "c",
            &ticking_entity("alpha:golem", "com.alpha.Golem", "-512.00, 70.00, 1024.50")
                .replace("4817", "99")
                .replace("(13,64,-8)", "(-512,70,1024)"),
        );
        assert_eq!(golem, elsewhere);
    }

    #[test]
    fn blocos_diferentes_com_pilha_so_do_jogo_tem_assinaturas_diferentes() {
        let block = |name: &str, class: &str, at: &str| {
            game_only_report(
                "Ticking block entity",
                &format!(
                    "-- Block entity being ticked --
Details:
\tName: {name} // {class}
\tBlock: Block{{{name}}}[facing=north]
\tBlock location: World: ({at}), Section: (at 1,2,3 in 4,5,6)
"
                ),
            )
        };
        let furnace = report_signature(
            "a",
            &block("alpha:forno", "com.alpha.FornoBlockEntity", "10,64,10"),
        );
        let chest = report_signature(
            "b",
            &block("beta:bau", "com.beta.BauBlockEntity", "10,64,10"),
        );
        let moved = report_signature(
            "c",
            &block("alpha:forno", "com.alpha.FornoBlockEntity", "-300,12,77"),
        );
        assert_eq!(
            furnace.rule.as_deref(),
            Some("crash_report.ticking_block_entity")
        );
        assert_ne!(furnace.hash, chest.hash);
        assert_eq!(furnace, moved);
        assert!(
            furnace
                .subject
                .contains(&"block_class=com.alpha.FornoBlockEntity".to_owned()),
            "{:?}",
            furnace.subject
        );
    }

    #[test]
    fn sujeito_do_achado_vem_da_key_do_catalogo() {
        // Duas dependências diferentes do mesmo mod: causas diferentes.
        let missing = |dependency: &str| {
            of(&[source(
                "latest.log",
                SourceKind::LatestLog,
                &format!(
                    "[12:00:00] [main/ERROR]: Missing or unsupported mandatory dependencies:\n\tMod ID: '{dependency}', Requested by: 'supplementaries', Expected range: '[1.20-2.0,)', Actual version: '[MISSING]'\n"
                ),
            )])
        };
        let moonlight = missing("moonlight");
        let other = missing("cloth_config");
        assert!(moonlight.rule.is_some(), "{moonlight:?}");
        assert_ne!(moonlight.hash, other.hash, "{moonlight:?} × {other:?}");
    }

    #[test]
    fn causa_certa_nao_depende_de_onde_o_jogo_quebrou() {
        // Falta de memória com e sem crash report (e com descrições diferentes): a mesma causa.
        let log = "[12:00:00] [Render thread/ERROR]: Erro\njava.lang.OutOfMemoryError: Java heap space\n\tat net.minecraft.client.Minecraft.run(Minecraft.java:10)\n";
        let alone = of(&[source("latest.log", SourceKind::LatestLog, log)]);
        let with_report = |description: &str| {
            of(&[
                source(
                    "crash-reports/c.txt",
                    SourceKind::CrashReport,
                    &format!(
                        "---- Minecraft Crash Report ----\nDescription: {description}\n\njava.lang.OutOfMemoryError: Java heap space\n\tat net.minecraft.client.Minecraft.run(Minecraft.java:10)\n"
                    ),
                ),
                source("latest.log", SourceKind::LatestLog, log),
            ])
        };
        assert_eq!(alone.rule.as_deref(), Some("memory.out_of_memory"));
        assert_eq!(alone.subject, Vec::<String>::new());
        assert_eq!(with_report("Unexpected error"), alone);
        assert_eq!(with_report("Rendering overlay"), alone);
    }

    // ---- Mixin ------------------------------------------------------------------------------

    #[test]
    fn id_dos_metodos_injetados_pelo_mixin_sai_em_todas_as_formas() {
        // Fabric (com o mod) e Forge/NeoForge (sem), id decimal e hexadecimal, e sem id.
        let cases = [
            ("a.B.handler$zza000$meumod$aoTick", "a.B.meumod$aoTick"),
            ("a.B.handler$zza00a$meumod$aoTick", "a.B.meumod$aoTick"),
            ("a.B.handler$bkb1f3c$meumod$aoTick", "a.B.meumod$aoTick"),
            ("a.B.handler$meumod$aoTick", "a.B.meumod$aoTick"),
            ("a.B.redirect$zzb00f$meumod$trocar", "a.B.meumod$trocar"),
            ("a.B.modify$bca000$meumod$valor", "a.B.meumod$valor"),
            ("a.B.localvar$zhm00e$meumod$x", "a.B.meumod$x"),
            ("a.B.constant$zzc001$meumod$k", "a.B.meumod$k"),
            ("a.B.args$zzd010$meumod$a", "a.B.meumod$a"),
            (
                "a.B.wrapOperation$cfa00b$meumod$embrulhar",
                "a.B.meumod$embrulhar",
            ),
            ("a.B.wrapWithCondition$cfb000$meumod$se", "a.B.meumod$se"),
            (
                "a.B.modifyExpressionValue$zza0ff$meumod$expr",
                "a.B.meumod$expr",
            ),
            ("a.B.modifyReturnValue$zza100$meumod$ret", "a.B.meumod$ret"),
            ("a.B.modifyReceiver$zza101$meumod$rec", "a.B.meumod$rec"),
            ("a.B.wrapMethod$zza102$meumod$tudo", "a.B.meumod$tudo"),
            ("a.B.handler$zza000$aoTick", "a.B.aoTick"),
            ("a.B$Interna.redirect$abc123$aoTick", "a.B$Interna.aoTick"),
            // Um mod com nome de "id" sem algarismo fica.
            ("a.B.handler$bigbeef$aoTick", "a.B.bigbeef$aoTick"),
            // Outros números que mudam com a compilação.
            ("a.B.lambda$tick$12", "a.B.lambda$tick"),
            ("a.B.lambda$3", "a.B.lambda"),
            ("a.B.aoTick$lambda-3", "a.B.aoTick$lambda"),
            ("a.B.access$000", "a.B.access$"),
            (
                "net.minecraftforge.eventbus.ASMEventHandler_12_Foo_bar_Evento.invoke",
                "net.minecraftforge.eventbus.ASMEventHandler_Foo_bar_Evento.invoke",
            ),
            (
                "jdk.internal.reflect.GeneratedMethodAccessor34.invoke",
                "jdk.internal.reflect.GeneratedMethodAccessor.invoke",
            ),
            (
                "com.sun.proxy.$Proxy56.chamar",
                "com.sun.proxy.$Proxy.chamar",
            ),
        ];
        for (raw, expected) in cases {
            assert_eq!(normalize_method(raw), expected, "{raw}");
        }
    }

    #[test]
    fn o_mesmo_travamento_com_ids_de_mixin_diferentes_tem_a_mesma_assinatura() {
        let crash = |id: &str| {
            format!(
                "---- Minecraft Crash Report ----
Description: Unexpected error

java.lang.NullPointerException: null
\tat net.minecraft.class_1297.handler${id}$meumod$aoTick(class_1297.java:1234)
\tat net.minecraft.class_1297.method_5773(class_1297.java:500)
"
            )
        };
        let sources = |id: &str| {
            [source(
                "crash-reports/c.txt",
                SourceKind::CrashReport,
                &crash(id),
            )]
        };
        let context = AnalysisContext {
            loader: Some(LogLoader::Fabric),
            minecraft: None,
        };
        let sign = |id: &str| {
            let sources = sources(id);
            signature(&analyze(&sources, &context, None).unwrap(), &sources)
        };
        let first = sign("zza000");
        assert_eq!(
            first.frame.as_deref(),
            Some("net.minecraft.class_1297.meumod$aoTick")
        );
        for id in ["zza00a", "bkb1f3", "zzc0a1f"] {
            assert_eq!(sign(id), first, "{id}");
        }
        // Outro mod no mesmo ponto: outra causa.
        let other = sources("zza000").map(|_| {
            source(
                "crash-reports/c.txt",
                SourceKind::CrashReport,
                &crash("zza000").replace("meumod", "outromod"),
            )
        });
        let other = signature(&analyze(&other, &context, None).unwrap(), &other);
        assert_ne!(other.hash, first.hash);
    }

    #[test]
    fn frame_de_mod_vem_da_causa_mais_profunda_e_pula_jogo_e_bibliotecas() {
        let log = "[12:00:00] [Render thread/ERROR]: Erro
java.lang.RuntimeException: Erro ao atualizar
\tat net.minecraft.client.Minecraft.tick(Minecraft.java:10)
\tat com.primeiro.Mod.run(Mod.java:3)
Caused by: java.lang.IllegalStateException: estado
\tat java.base/java.util.Objects.requireNonNull(Objects.java:233)
\tat com.google.common.Foo.bar(Foo.java:5)
\tat com.segundo.Mod$$Lambda$1234/0x0000000800c3b2a8.accept(Unknown Source)
\tat com.segundo.Mod.handle(Mod.java:9)
";
        let lines: Vec<CleanLine> = source("latest.log", SourceKind::LatestLog, log).clean_lines();
        let (exception, frame) = exception_and_frame(&lines, 0, false);
        assert_eq!(
            exception.as_deref(),
            Some("java.lang.IllegalStateException")
        );
        assert_eq!(frame.as_deref(), Some("com.segundo.Mod$$Lambda.accept"));
    }

    #[test]
    fn modulos_e_jars_decidem_quando_a_linha_os_tem() {
        let log = "java.lang.NullPointerException
\tat TRANSFORMER/minecraft@1.20.1/net.minecraft.world.Foo.bar(Foo.java:1)
\tat TRANSFORMER/create@0.5.1.f/com.simibubi.create.Bar.tick(Bar.java:2)
";
        let lines = source("x.log", SourceKind::Text, log).clean_lines();
        assert_eq!(
            exception_and_frame(&lines, 0, false).1.as_deref(),
            Some("com.simibubi.create.Bar.tick")
        );
        // Handler de mixin do Fabric numa classe do jogo é frame do mod dono.
        let fabric = "java.lang.NullPointerException
\tat net.minecraft.class_1297.handler$zza000$meumod$aoTick(class_1297.java:1234)
";
        let lines = source("x.log", SourceKind::Text, fabric).clean_lines();
        assert_eq!(
            exception_and_frame(&lines, 0, true).1.as_deref(),
            Some("net.minecraft.class_1297.meumod$aoTick")
        );
        assert_eq!(exception_and_frame(&lines, 0, false).1, None);
    }

    #[test]
    fn sem_achado_usa_o_crash_report_e_sem_nada_so_o_hash_vazio() {
        let report = "---- Minecraft Crash Report ----
Algo estranho
java.lang.ArithmeticException: / by zero
\tat org.exemplo.Calc.div(Calc.java:12)
";
        let sources = [source(
            "crash-reports/c.txt",
            SourceKind::CrashReport,
            report,
        )];
        let analysis = Analysis {
            findings: Vec::new(),
            loader: None,
            loader_version: None,
            minecraft: None,
            sources: Vec::new(),
        };
        let found = signature(&analysis, &sources);
        assert_eq!(found.rule, None);
        assert_eq!(
            found.exception.as_deref(),
            Some("java.lang.ArithmeticException")
        );
        assert_eq!(found.frame.as_deref(), Some("org.exemplo.Calc.div"));
        let nothing = signature(
            &analysis,
            &[source("output.log", SourceKind::Output, "oi\n")],
        );
        assert_eq!(
            nothing,
            CrashSignature::new(None, None, None, Vec::new(), Vec::new())
        );
        assert_ne!(nothing.hash, found.hash);
    }

    #[test]
    fn frame_nativo_sem_endereco() {
        let text = "#
# A fatal error has been detected by the Java Runtime Environment:
#
#  EXCEPTION_ACCESS_VIOLATION (0xc0000005) at pc=0x000000006b1d99ee, pid=13332, tid=11504
#
# Problematic frame:
# J 4321 C2 net.minecraft.client.Foo.bar()V (45 bytes) @ 0x0000000003a1b2c3 [0x0000000003a1b000+0x2c3]
";
        let lines = source("hs_err_pid1.log", SourceKind::JvmCrash, text).clean_lines();
        assert_eq!(
            native_frame(&lines, 3).as_deref(),
            Some("J net.minecraft.client.Foo.bar")
        );
    }

    #[test]
    fn nomes_de_jar_e_ids_normalizados() {
        assert_eq!(jar_stem("create-1.20.1-0.5.1.f.jar"), "create");
        assert_eq!(jar_stem("mods/Botania-1.20.1-443.jar"), "botania");
        assert_eq!(
            jar_stem("sodium-fabric-0.5.8+mc1.20.1.jar"),
            "sodium-fabric"
        );
        assert_eq!(jar_stem("jei_1.12.2-4.16.jar%2390"), "jei");
        assert_eq!(jar_stem("1.12.jar"), "1.12");
        assert_eq!(
            normalize_item(&ItemRef::ModId {
                id: "Village-Employment".into()
            }),
            "mod:village_employment"
        );
        assert_eq!(
            normalize_item(&ItemRef::Metafile {
                path: "mods/a.pw.toml".into()
            }),
            "item:mods/a.pw.toml"
        );
        assert_eq!(
            normalize_method("a.B.handler$bcd123$m$md$1a2b3c$0"),
            "a.B.m$"
        );
    }

    #[test]
    fn hash_estavel_e_mods_em_ordem() {
        let one = CrashSignature::new(
            Some("r".into()),
            Some("E".into()),
            None,
            vec!["mod:b".into(), "mod:a".into(), "mod:a".into()],
            Vec::new(),
        );
        let two = CrashSignature::new(
            Some("r".into()),
            Some("E".into()),
            None,
            vec!["mod:a".into(), "mod:b".into()],
            Vec::new(),
        );
        assert_eq!(one, two);
        assert_eq!(one.mods, ["mod:a", "mod:b"]);
        // Fixo: mudar a normalização ou o formato exige subir SIGNATURE_VERSION.
        assert_eq!(one.hash, "5a6c64dda3831c4e");
        let with_subject = CrashSignature::new(
            Some("r".into()),
            Some("E".into()),
            None,
            vec!["mod:a".into(), "mod:b".into()],
            vec!["description=x".into()],
        );
        assert_ne!(with_subject.hash, one.hash);
        let empty_rule = CrashSignature::new(None, Some("r".into()), None, Vec::new(), Vec::new());
        let empty_exception =
            CrashSignature::new(Some("r".into()), None, None, Vec::new(), Vec::new());
        assert_ne!(empty_rule.hash, empty_exception.hash);
    }
}
