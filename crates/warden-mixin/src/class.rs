//! Camada B: abre uma classe de mixin com o `cafebabe` (sem decodificar o bytecode) e lê a
//! `@Mixin` (alvos e prioridade) e os injetores de cada método (método-alvo, `@At`, seletor e
//! `require`), traduzidos pelo refmap.

use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};
use std::panic::AssertUnwindSafe;

use cafebabe::attributes::{Annotation, AnnotationElementValue, AttributeData, AttributeInfo};
use cafebabe::constant_pool::{ConstantPoolItem, LiteralConstant};
use cafebabe::descriptors::FieldType;
use cafebabe::{ParseOptions, parse_class_with_options};

use crate::model::{At, Change, ChangeKind, IssueKind, MixinClass};
use crate::refmap::{Refmap, method_name};

const MIXIN: &str = "org/spongepowered/asm/mixin/Mixin";

/// O que a config diz sobre as classes dela.
pub(crate) struct ConfigDefaults<'a> {
    pub(crate) name: &'a str,
    pub(crate) priority: i32,
    pub(crate) plugin: bool,
    pub(crate) default_require: Option<i32>,
    pub(crate) refmap: &'a Refmap,
}

/// Lê a classe `internal` (nome interno). Erro: classe ilegível ou sem `@Mixin`.
pub(crate) fn parse_mixin_class(
    bytes: &[u8],
    internal: &str,
    config: &ConfigDefaults<'_>,
    counts: &mut BTreeMap<String, u32>,
) -> Result<MixinClass, IssueKind> {
    // O leitor não deveria entrar em pânico com bytes estranhos, mas a entrada não é confiável:
    // um jar corrompido nunca derruba a análise do pack.
    let mut local_counts = BTreeMap::new();
    let parsed = std::panic::catch_unwind(AssertUnwindSafe(|| {
        read_class(bytes, internal, config, &mut local_counts)
    }))
    .unwrap_or(Err(IssueKind::UnreadableClass))?;
    for (name, count) in local_counts {
        *counts.entry(name).or_default() += count;
    }
    Ok(parsed)
}

fn options() -> ParseOptions {
    let mut options = ParseOptions::default();
    options.parse_bytecode(false);
    options
}

#[allow(clippy::too_many_lines)] // uma passagem pela classe, na ordem do formato
fn read_class(
    bytes: &[u8],
    internal: &str,
    config: &ConfigDefaults<'_>,
    counts: &mut BTreeMap<String, u32>,
) -> Result<MixinClass, IssueKind> {
    let class =
        parse_class_with_options(bytes, &options()).map_err(|_| IssueKind::UnreadableClass)?;
    let refmap = config.refmap;

    let mut targets = Vec::new();
    let mut priority = config.priority;
    let mut is_mixin = false;
    for annotation in annotations(&class.attributes) {
        if type_name(annotation) != MIXIN {
            continue;
        }
        is_mixin = true;
        match element(annotation, "value") {
            Some(AnnotationElementValue::ArrayValue(values)) => {
                for value in values {
                    if let AnnotationElementValue::ClassLiteral { class_name } = value {
                        targets.push(descriptor_class(class_name));
                    }
                }
            }
            Some(AnnotationElementValue::ClassLiteral { class_name }) => {
                targets.push(descriptor_class(class_name));
            }
            _ => {}
        }
        for target in strings(element(annotation, "targets")) {
            targets.push(refmap.map(internal, &target).replace('.', "/"));
        }
        if let Some(value) = int(element(annotation, "priority")) {
            priority = value;
        }
    }
    if !is_mixin {
        return Err(IssueKind::NotAMixin);
    }
    targets.sort();
    targets.dedup();

    let mut refs: BTreeSet<String> = targets.iter().cloned().collect();
    for item in class.constantpool_iter() {
        match item {
            ConstantPoolItem::ClassInfo(name) => {
                refs.insert(descriptor_class(&name));
            }
            ConstantPoolItem::MethodRef(member)
            | ConstantPoolItem::FieldRef(member)
            | ConstantPoolItem::InterfaceMethodRef(member) => {
                refs.insert(member.class_name.to_string());
            }
            _ => {}
        }
    }

    let mut changes = Vec::new();
    for method in &class.methods {
        let descriptor = method.descriptor.to_string();
        for part in descriptor.split(';') {
            if let Some(start) = part.rfind('L') {
                refs.insert(part[start + 1..].to_owned());
            }
        }
        for annotation in annotations(&method.attributes) {
            let name = type_name(annotation);
            if let Some(counted) = counted_only(&name) {
                *counts.entry(counted.to_owned()).or_default() += 1;
                continue;
            }
            let Some(kind) = ChangeKind::from_annotation(&name) else {
                continue;
            };
            *counts.entry(kind.annotation().to_owned()).or_default() += 1;
            let require = int(element(annotation, "require"));
            let selector = selector_of(kind, annotation, &descriptor);
            let methods: Vec<(String, String)> = if kind == ChangeKind::Overwrite {
                vec![(method.name.to_string(), String::new())]
            } else {
                strings(element(annotation, "method"))
                    .iter()
                    .map(|written| {
                        let runtime = method_name(refmap.map(internal, written));
                        let named = method_name(written);
                        let named = if named == runtime {
                            String::new()
                        } else {
                            named
                        };
                        (runtime, named)
                    })
                    .collect()
            };
            let mut ats = Vec::new();
            match element(annotation, "at") {
                Some(AnnotationElementValue::AnnotationValue(at)) => {
                    ats.push(at_of(at, internal, refmap));
                }
                Some(AnnotationElementValue::ArrayValue(values)) => {
                    for value in values {
                        if let AnnotationElementValue::AnnotationValue(at) = value {
                            ats.push(at_of(at, internal, refmap));
                        }
                    }
                }
                _ => {}
            }
            if ats.is_empty() {
                let value = match kind {
                    ChangeKind::Overwrite => "OVERWRITE",
                    ChangeKind::ModifyConstant => "CONSTANT",
                    ChangeKind::WrapMethod => "WRAP_METHOD",
                    _ => "?",
                };
                let target = if kind == ChangeKind::ModifyConstant {
                    constant_key(annotation)
                } else {
                    String::new()
                };
                ats.push(At {
                    value: value.to_owned(),
                    target,
                });
            }
            for target_class in &targets {
                for (runtime, named) in &methods {
                    for at in &ats {
                        changes.push(Change {
                            kind,
                            target_class: target_class.clone(),
                            method: runtime.clone(),
                            method_named: named.clone(),
                            at: at.clone(),
                            selector: selector.clone(),
                            require,
                        });
                    }
                }
            }
        }
    }

    Ok(MixinClass {
        name: internal.to_owned(),
        config: config.name.to_owned(),
        plugin: config.plugin,
        priority,
        default_require: config.default_require,
        targets,
        changes,
        refs: refs.into_iter().filter(|r| !r.is_empty()).collect(),
    })
}

/// Os textos (`CONSTANT_String`) de uma classe qualquer (plugin, config, compat).
pub(crate) fn string_constants(bytes: &[u8], out: &mut BTreeSet<String>) {
    let _ = std::panic::catch_unwind(AssertUnwindSafe(|| {
        if let Ok(class) = parse_class_with_options(bytes, &options()) {
            for item in class.constantpool_iter() {
                if let ConstantPoolItem::LiteralConstant(LiteralConstant::String(text)) = item
                    && text.len() <= 200
                {
                    out.insert(text.to_string());
                }
            }
        }
    }));
}

fn counted_only(type_name: &str) -> Option<&'static str> {
    if !type_name.starts_with("org/spongepowered/asm/mixin/") {
        return None;
    }
    match type_name.rsplit('/').next().unwrap_or(type_name) {
        "Shadow" => Some("Shadow"),
        "Accessor" => Some("Accessor"),
        "Invoker" => Some("Invoker"),
        _ => None,
    }
}

/// `Lnet/minecraft/class_1;` (ou `[L…;`) → `net/minecraft/class_1`.
fn descriptor_class(descriptor: &str) -> String {
    let trimmed = descriptor.trim_start_matches('[');
    trimmed.strip_prefix('L').map_or_else(
        || trimmed.to_owned(),
        |inner| inner.trim_end_matches(';').to_owned(),
    )
}

fn element<'b, 'a>(
    annotation: &'b Annotation<'a>,
    name: &str,
) -> Option<&'b AnnotationElementValue<'a>> {
    annotation
        .elements
        .iter()
        .find(|e| e.name == name)
        .map(|e| &e.value)
}

fn strings(value: Option<&AnnotationElementValue<'_>>) -> Vec<String> {
    match value {
        Some(AnnotationElementValue::StringConstant(text)) => vec![text.to_string()],
        Some(AnnotationElementValue::ArrayValue(values)) => values
            .iter()
            .filter_map(|v| match v {
                AnnotationElementValue::StringConstant(text) => Some(text.to_string()),
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    }
}

fn int(value: Option<&AnnotationElementValue<'_>>) -> Option<i32> {
    match value {
        Some(AnnotationElementValue::IntConstant(number)) => Some(*number),
        _ => None,
    }
}

fn annotations<'x, 'a>(attributes: &'x [AttributeInfo<'a>]) -> Vec<&'x Annotation<'a>> {
    let mut out = Vec::new();
    for attribute in attributes {
        if let AttributeData::RuntimeVisibleAnnotations(list)
        | AttributeData::RuntimeInvisibleAnnotations(list) = &attribute.data
        {
            out.extend(list.iter());
        }
    }
    out
}

fn type_name(annotation: &Annotation<'_>) -> String {
    match &annotation.type_descriptor.field_type {
        FieldType::Object(class) => class.to_string(),
        other => other.to_string(),
    }
}

fn at_of(at: &Annotation<'_>, internal: &str, refmap: &Refmap) -> At {
    let value = strings(element(at, "value"))
        .into_iter()
        .next()
        .unwrap_or_default();
    let target = strings(element(at, "target"))
        .into_iter()
        .next()
        .map(|t| refmap.map(internal, &t).to_owned())
        .unwrap_or_default();
    At { value, target }
}

fn constant_key(annotation: &Annotation<'_>) -> String {
    let constants: Vec<&Annotation<'_>> = match element(annotation, "constant") {
        Some(AnnotationElementValue::AnnotationValue(constant)) => vec![constant],
        Some(AnnotationElementValue::ArrayValue(values)) => values
            .iter()
            .filter_map(|v| match v {
                AnnotationElementValue::AnnotationValue(constant) => Some(constant),
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    };
    constants
        .iter()
        .map(|constant| {
            constant
                .elements
                .iter()
                .map(|e| format!("{}={}", e.name, value_text(&e.value)))
                .collect::<Vec<_>>()
                .join(",")
        })
        .collect::<Vec<_>>()
        .join("|")
}

fn value_text(value: &AnnotationElementValue<'_>) -> Cow<'static, str> {
    match value {
        AnnotationElementValue::IntConstant(n)
        | AnnotationElementValue::ByteConstant(n)
        | AnnotationElementValue::CharConstant(n)
        | AnnotationElementValue::ShortConstant(n) => n.to_string().into(),
        AnnotationElementValue::BooleanConstant(n) => (*n != 0).to_string().into(),
        AnnotationElementValue::LongConstant(n) => n.to_string().into(),
        AnnotationElementValue::FloatConstant(n) => n.to_string().into(),
        AnnotationElementValue::DoubleConstant(n) => n.to_string().into(),
        AnnotationElementValue::StringConstant(text) => text.to_string().into(),
        AnnotationElementValue::ClassLiteral { class_name } => class_name.to_string().into(),
        AnnotationElementValue::EnumConstant { const_name, .. } => const_name.to_string().into(),
        AnnotationElementValue::AnnotationValue(_) | AnnotationElementValue::ArrayValue(_) => {
            "?".into()
        }
    }
}

/// Seletor do `@ModifyVariable` (`ordinal`, `index`, `name`, `argsOnly` e o tipo do primeiro
/// parâmetro do método, que é o tipo da variável) e do `@ModifyArg` (`index` e o tipo); `*` no
/// `@ModifyArgs`, que mexe em todos os argumentos.
fn selector_of(kind: ChangeKind, annotation: &Annotation<'_>, handler: &str) -> String {
    let get = |name: &str| {
        element(annotation, name)
            .map(|v| value_text(v).into_owned())
            .unwrap_or_default()
    };
    let first = first_parameter(handler);
    match kind {
        ChangeKind::ModifyVariable => format!(
            "ord={};idx={};name={};args={};type={first}",
            get("ordinal"),
            get("index"),
            strings(element(annotation, "name")).join("|"),
            get("argsOnly")
        ),
        ChangeKind::ModifyArg => format!("idx={};type={first}", get("index")),
        ChangeKind::ModifyArgs => "*".to_owned(),
        _ => String::new(),
    }
}

/// O tipo do primeiro parâmetro de um descritor de método (`(I)V` → `I`).
fn first_parameter(descriptor: &str) -> &str {
    let Some(params) = descriptor.strip_prefix('(') else {
        return "";
    };
    let bytes = params.as_bytes();
    let mut end = 0;
    while bytes.get(end) == Some(&b'[') {
        end += 1;
    }
    match bytes.get(end) {
        Some(b'L') => params[end..].find(';').map_or("", |i| &params[..=end + i]),
        Some(b')') | None => "",
        Some(_) => &params[..=end],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn primeiro_parametro() {
        assert_eq!(first_parameter("(I)V"), "I");
        assert_eq!(
            first_parameter("([[Ljava/lang/String;I)V"),
            "[[Ljava/lang/String;"
        );
        assert_eq!(first_parameter("()V"), "");
        assert_eq!(first_parameter("V"), "");
        assert_eq!(first_parameter("([J)V"), "[J");
    }

    #[test]
    fn classe_do_descritor() {
        assert_eq!(
            descriptor_class("Lnet/minecraft/class_1;"),
            "net/minecraft/class_1"
        );
        assert_eq!(descriptor_class("[La/B;"), "a/B");
        assert_eq!(descriptor_class("a/B"), "a/B");
    }

    #[test]
    fn bytes_quaisquer_nao_derrubam() {
        let refmap = Refmap::default();
        let config = ConfigDefaults {
            name: "x.mixins.json",
            priority: 1000,
            plugin: false,
            default_require: None,
            refmap: &refmap,
        };
        let mut counts = BTreeMap::new();
        for bytes in [
            &b""[..],
            b"\xCA\xFE\xBA\xBE",
            b"\xCA\xFE\xBA\xBE\x00\x00\x00\x34\xFF\xFF",
            b"PK\x03\x04",
        ] {
            assert_eq!(
                parse_mixin_class(bytes, "a/B", &config, &mut counts),
                Err(IssueKind::UnreadableClass)
            );
        }
        let mut out = BTreeSet::new();
        string_constants(b"\xCA\xFE", &mut out);
        assert!(out.is_empty());
    }
}
