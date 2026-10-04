//! Protótipo do raio-x de mixins da R5A (§5.3 a §5.5), refeito no Windows para o
//! spike S-R5-1. O protótipo original ficou no WSL, que não existe mais; este
//! segue a mesma descrição: `cafebabe` sem bytecode, `zip`, `serde_json`, `toml`,
//! descida em jars aninhados, refmap, ponto de aplicação = classe + método + `@At`,
//! regras da §5.5, rebaixadores e elevadores. Código descartável.

use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::io::{Cursor, Read, Seek};
use std::path::{Path, PathBuf};

use cafebabe::attributes::{Annotation, AnnotationElementValue, AttributeData};
use cafebabe::constant_pool::ConstantPoolItem;
use cafebabe::{ParseOptions, parse_class_with_options};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Loader {
    Fabric,
    Forge,
    NeoForge,
}

impl Loader {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "fabric" => Some(Self::Fabric),
            "forge" => Some(Self::Forge),
            "neoforge" => Some(Self::NeoForge),
            _ => None,
        }
    }
}

/// Uma alteração extraída de uma classe de mixin (um injetor ou `@Overwrite`).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Change {
    pub kind: String,
    pub target_class: String,
    pub method: String,
    pub at_value: String,
    pub at_target: String,
    pub require: Option<i32>,
    pub mixin_class: String,
    pub priority: i32,
    /// Qual variável/argumento o `@ModifyVariable`/`@ModifyArg` escolhe (vazio nos outros).
    #[serde(default)]
    pub selector: String,
}

/// Uma classe de mixin com o que ela altera e o que ela referencia.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MixinClass {
    pub name: String,
    pub config: String,
    pub plugin: bool,
    pub default_require: Option<i32>,
    pub targets: Vec<String>,
    pub changes: Vec<Change>,
    /// Classes citadas no constant pool e nos descritores (para o rebaixador
    /// "referencia classes do outro mod").
    pub refs: Vec<String>,
}

/// Um mod (jar de topo ou aninhado) com o que interessa ao raio-x.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Unit {
    pub modid: String,
    pub version: String,
    pub jar: String,
    pub nested: bool,
    pub deps: Vec<String>,
    pub packages: Vec<String>,
    pub configs: Vec<String>,
    pub classes: Vec<MixinClass>,
    pub annotation_counts: BTreeMap<String, u32>,
    pub errors: Vec<String>,
    /// Textos das classes de plugin/config do mod (para "o plugin cita o outro mod").
    #[serde(default)]
    pub plugin_strings: Vec<String>,
    /// Opções de outros mods que este desliga no descritor (`lithium:mixin.chunk.palette`).
    #[serde(default)]
    pub disables: Vec<String>,
}

const MIXIN_DESC: &str = "org/spongepowered/asm/mixin/Mixin";

fn injector_kind(type_name: &str) -> Option<&'static str> {
    // Comparação pelo final do nome: MixinExtras aparece em `com/llamalad7/mixinextras/...`.
    let simple = type_name.rsplit('/').next().unwrap_or(type_name);
    let is_sponge = type_name.starts_with("org/spongepowered/asm/mixin/");
    let is_extras = type_name.contains("mixinextras/");
    match simple {
        "Overwrite" if is_sponge => Some("Overwrite"),
        "Inject" if is_sponge => Some("Inject"),
        "Redirect" if is_sponge => Some("Redirect"),
        "ModifyArg" if is_sponge => Some("ModifyArg"),
        "ModifyArgs" if is_sponge => Some("ModifyArgs"),
        "ModifyVariable" if is_sponge => Some("ModifyVariable"),
        "ModifyConstant" if is_sponge => Some("ModifyConstant"),
        "WrapOperation" if is_extras => Some("WrapOperation"),
        "ModifyExpressionValue" if is_extras => Some("ModifyExpressionValue"),
        "ModifyReturnValue" if is_extras => Some("ModifyReturnValue"),
        "WrapWithCondition" if is_extras => Some("WrapWithCondition"),
        "ModifyReceiver" if is_extras => Some("ModifyReceiver"),
        "WrapMethod" if is_extras => Some("WrapMethod"),
        _ => None,
    }
}

fn counted_only(type_name: &str) -> Option<&'static str> {
    let simple = type_name.rsplit('/').next().unwrap_or(type_name);
    if !type_name.starts_with("org/spongepowered/asm/mixin/") {
        return None;
    }
    match simple {
        "Shadow" => Some("Shadow"),
        "Accessor" => Some("Accessor"),
        "Invoker" => Some("Invoker"),
        _ => None,
    }
}

fn desc_to_internal(desc: &str) -> String {
    let d = desc.trim_start_matches('[');
    if let Some(s) = d.strip_prefix('L') {
        s.trim_end_matches(';').to_string()
    } else {
        d.to_string()
    }
}

fn element<'a, 'b>(a: &'b Annotation<'a>, name: &str) -> Option<&'b AnnotationElementValue<'a>> {
    a.elements.iter().find(|e| e.name == name).map(|e| &e.value)
}

fn strings(v: Option<&AnnotationElementValue<'_>>) -> Vec<String> {
    match v {
        Some(AnnotationElementValue::StringConstant(s)) => vec![s.to_string()],
        Some(AnnotationElementValue::ArrayValue(a)) => a
            .iter()
            .filter_map(|x| match x {
                AnnotationElementValue::StringConstant(s) => Some(s.to_string()),
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    }
}

fn int(v: Option<&AnnotationElementValue<'_>>) -> Option<i32> {
    match v {
        Some(AnnotationElementValue::IntConstant(i)) => Some(*i),
        _ => None,
    }
}

fn annotations_of<'x, 'a>(attrs: &'x [cafebabe::attributes::AttributeInfo<'a>]) -> Vec<&'x Annotation<'a>> {
    let mut out = Vec::new();
    for at in attrs {
        match &at.data {
            AttributeData::RuntimeVisibleAnnotations(v) | AttributeData::RuntimeInvisibleAnnotations(v) => {
                out.extend(v.iter())
            }
            _ => {}
        }
    }
    out
}

fn type_name(a: &Annotation<'_>) -> String {
    match &a.type_descriptor.field_type {
        cafebabe::descriptors::FieldType::Object(c) => c.to_string(),
        other => other.to_string(),
    }
}

/// Nome do método-alvo sem dono nem descritor: `Lx/Y;m_1(I)V` → `m_1`.
pub fn method_name(s: &str) -> String {
    let mut s = s;
    if s.starts_with('L')
        && let Some(i) = s.find(';')
        && s[..i].contains('/')
    {
        s = &s[i + 1..];
    }
    // `dono.metodo(...)` (forma alternativa com ponto).
    let paren = s.find('(').unwrap_or(s.len());
    let head = &s[..paren];
    let head = head.rsplit('.').next().unwrap_or(head);
    head.split(':').next().unwrap_or(head).to_string()
}

#[derive(Default)]
struct Refmap {
    mappings: HashMap<String, HashMap<String, String>>,
}

impl Refmap {
    fn parse(bytes: &[u8]) -> Self {
        let mut r = Refmap::default();
        let Ok(v) = serde_json::from_slice::<serde_json::Value>(bytes) else {
            return r;
        };
        let mut add = |m: &serde_json::Value| {
            if let Some(obj) = m.as_object() {
                for (cls, map) in obj {
                    if let Some(map) = map.as_object() {
                        let e = r.mappings.entry(cls.clone()).or_default();
                        for (k, val) in map {
                            if let Some(s) = val.as_str() {
                                e.entry(k.clone()).or_insert_with(|| s.to_string());
                            }
                        }
                    }
                }
            }
        };
        if let Some(m) = v.get("mappings") {
            add(m);
        }
        if let Some(data) = v.get("data").and_then(|d| d.as_object()) {
            for ns in data.values() {
                add(ns);
            }
        }
        r
    }

    fn map<'s>(&'s self, mixin: &str, s: &'s str) -> &'s str {
        self.mappings
            .get(mixin)
            .and_then(|m| m.get(s))
            .map(String::as_str)
            .unwrap_or(s)
    }
}

struct ConfigRef {
    name: String,
    client_ok: bool,
}

fn manifest_mixin_configs(text: &str) -> Vec<String> {
    // Linhas de continuação começam com um espaço (quebra a cada 72 bytes).
    let mut joined: Vec<String> = Vec::new();
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix(' ') {
            if let Some(last) = joined.last_mut() {
                last.push_str(rest);
            }
        } else {
            joined.push(line.to_string());
        }
    }
    joined
        .iter()
        .find_map(|l| l.strip_prefix("MixinConfigs:"))
        .map(|v| v.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect())
        .unwrap_or_default()
}

fn read_entry<R: Read + Seek>(z: &mut zip::ZipArchive<R>, name: &str) -> Option<Vec<u8>> {
    let mut f = z.by_name(name).ok()?;
    let mut buf = Vec::with_capacity(f.size() as usize);
    f.read_to_end(&mut buf).ok()?;
    Some(buf)
}

fn lenient_json(bytes: &[u8]) -> Option<serde_json::Value> {
    if let Ok(v) = serde_json::from_slice(bytes) {
        return Some(v);
    }
    // Alguns fabric.mod.json têm quebras de linha cruas dentro de textos.
    let s = String::from_utf8_lossy(bytes).replace(['\n', '\r', '\t'], " ");
    serde_json::from_str(&s).ok()
}

fn fabric_deps(v: &serde_json::Value) -> Vec<String> {
    let mut out = Vec::new();
    for key in ["depends", "recommends", "suggests"] {
        if let Some(o) = v.get(key).and_then(|d| d.as_object()) {
            out.extend(o.keys().cloned());
        }
    }
    out
}

fn toml_mod(bytes: &[u8]) -> Option<(String, String, Vec<String>, Vec<String>)> {
    let text = String::from_utf8_lossy(bytes);
    let v: toml::Value = toml::from_str(&text).ok()?;
    let m = v.get("mods")?.as_array()?.first()?;
    let id = m.get("modId")?.as_str()?.to_string();
    let ver = m.get("version").and_then(|x| x.as_str()).unwrap_or("").to_string();
    let mut deps = Vec::new();
    if let Some(d) = v.get("dependencies").and_then(|d| d.as_table()) {
        for list in d.values() {
            if let Some(arr) = list.as_array() {
                for e in arr {
                    if let Some(mid) = e.get("modId").and_then(|x| x.as_str()) {
                        deps.push(mid.to_string());
                    }
                }
            }
        }
    }
    let mut mixins = Vec::new();
    if let Some(arr) = v.get("mixins").and_then(|x| x.as_array()) {
        for e in arr {
            if let Some(c) = e.get("config").and_then(|x| x.as_str()) {
                mixins.push(c.to_string());
            }
        }
    }
    Some((id, ver, deps, mixins))
}

/// Lê um jar (de topo ou aninhado) e devolve as unidades (ele e os aninhados).
pub fn scan_jar_bytes<R: Read + Seek>(
    reader: R,
    jar_name: &str,
    nested: bool,
    loader: Loader,
    out: &mut Vec<Unit>,
) {
    let mut z = match zip::ZipArchive::new(reader) {
        Ok(z) => z,
        Err(e) => {
            out.push(Unit {
                modid: format!("?{jar_name}"),
                version: String::new(),
                jar: jar_name.into(),
                nested,
                deps: vec![],
                packages: vec![],
                configs: vec![],
                classes: vec![],
                annotation_counts: BTreeMap::new(),
                errors: vec![format!("zip: {e}")],
                plugin_strings: vec![],
                disables: vec![],
            });
            return;
        }
    };
    let names: Vec<String> = z.file_names().map(str::to_string).collect();
    let mut errors = Vec::new();

    // Pacotes (para o rebaixador de compatibilidade) e jars aninhados.
    let mut packages = BTreeSet::new();
    let mut nested_jars = Vec::new();
    for n in &names {
        if n.ends_with(".class") && !n.starts_with("META-INF/") {
            if let Some(i) = n.rfind('/') {
                packages.insert(n[..i].to_string());
            }
        } else if n.ends_with(".jar") && (n.starts_with("META-INF/jars/") || n.starts_with("META-INF/jarjar/")) {
            nested_jars.push(n.clone());
        }
    }

    let fabric = read_entry(&mut z, "fabric.mod.json").and_then(|b| lenient_json(&b));
    let forge_toml = read_entry(&mut z, "META-INF/mods.toml").and_then(|b| toml_mod(&b));
    let neo_toml = read_entry(&mut z, "META-INF/neoforge.mods.toml").and_then(|b| toml_mod(&b));
    let manifest = read_entry(&mut z, "META-INF/MANIFEST.MF")
        .map(|b| manifest_mixin_configs(&String::from_utf8_lossy(&b)))
        .unwrap_or_default();

    let mut modid = String::new();
    let mut version = String::new();
    let mut deps = Vec::new();
    let mut configs: Vec<ConfigRef> = Vec::new();

    let use_fabric = match loader {
        Loader::Fabric => true,
        // Forge/NeoForge: o fabric.mod.json só vale para mods Fabric rodando pelo
        // Sinytra Connector (jar sem descritor do loader).
        Loader::Forge => forge_toml.is_none() && neo_toml.is_none(),
        Loader::NeoForge => neo_toml.is_none() && forge_toml.is_none(),
    };
    if loader != Loader::Fabric {
        let t = if loader == Loader::NeoForge { neo_toml.clone().or(forge_toml.clone()) } else { forge_toml.clone() };
        if let Some((id, ver, d, mixins)) = t {
            modid = id;
            version = ver;
            deps = d;
            if loader == Loader::NeoForge {
                configs.extend(mixins.into_iter().map(|name| ConfigRef { name, client_ok: true }));
            }
        }
        configs.extend(manifest.iter().cloned().map(|name| ConfigRef { name, client_ok: true }));
    }
    if use_fabric && let Some(v) = &fabric {
        modid = v.get("id").and_then(|x| x.as_str()).unwrap_or("").to_string();
        version = v.get("version").and_then(|x| x.as_str()).unwrap_or("").to_string();
        deps = fabric_deps(v);
        if let Some(arr) = v.get("mixins").and_then(|m| m.as_array()) {
            for e in arr {
                if let Some(s) = e.as_str() {
                    configs.push(ConfigRef { name: s.to_string(), client_ok: true });
                } else if let Some(c) = e.get("config").and_then(|c| c.as_str()) {
                    let env = e.get("environment").and_then(|x| x.as_str()).unwrap_or("*");
                    configs.push(ConfigRef { name: c.to_string(), client_ok: env != "server" });
                }
            }
        }
    }
    if modid.is_empty() {
        modid = format!("?{}", jar_name.rsplit('/').next().unwrap_or(jar_name));
    }

    let mut dedup = HashSet::new();
    configs.retain(|c| dedup.insert(c.name.clone()));

    let mut classes = Vec::new();
    let mut counts: BTreeMap<String, u32> = BTreeMap::new();
    let mut config_names = Vec::new();
    let mut plugin_classes: Vec<String> = Vec::new();
    for c in configs.iter().filter(|c| c.client_ok) {
        let Some(bytes) = read_entry(&mut z, &c.name) else {
            errors.push(format!("config ausente: {}", c.name));
            continue;
        };
        let Some(cfg) = lenient_json(&bytes) else {
            errors.push(format!("config inválida: {}", c.name));
            continue;
        };
        config_names.push(c.name.clone());
        let package = cfg.get("package").and_then(|x| x.as_str()).unwrap_or("").replace('.', "/");
        let cfg_priority = cfg.get("priority").and_then(|x| x.as_i64()).unwrap_or(1000) as i32;
        let plugin = cfg.get("plugin").and_then(|x| x.as_str()).is_some_and(|s| !s.is_empty());
        if let Some(pc) = cfg.get("plugin").and_then(|x| x.as_str()) {
            plugin_classes.push(pc.to_string());
        }
        let default_require = cfg
            .get("injectors")
            .and_then(|i| i.get("defaultRequire"))
            .and_then(|x| x.as_i64())
            .map(|x| x as i32);
        let refmap = cfg
            .get("refmap")
            .and_then(|x| x.as_str())
            .and_then(|r| read_entry(&mut z, r))
            .map(|b| Refmap::parse(&b))
            .unwrap_or_default();
        let mut list: Vec<String> = Vec::new();
        for key in ["mixins", "client"] {
            if let Some(arr) = cfg.get(key).and_then(|x| x.as_array()) {
                list.extend(arr.iter().filter_map(|x| x.as_str()).map(str::to_string));
            }
        }
        for m in list {
            let internal = if package.is_empty() {
                m.replace('.', "/")
            } else {
                format!("{package}/{}", m.replace('.', "/"))
            };
            let path = format!("{internal}.class");
            let Some(cb) = read_entry(&mut z, &path) else {
                errors.push(format!("classe ausente: {path}"));
                continue;
            };
            match parse_mixin_class(&cb, &internal, &c.name, cfg_priority, plugin, default_require, &refmap, &mut counts) {
                Ok(mc) => classes.push(mc),
                Err(e) => errors.push(format!("classe {path}: {e}")),
            }
        }
    }

    // Textos do plugin e das classes de config/compat do próprio mod.
    let mut plugin_strings = BTreeSet::new();
    if !classes.is_empty() {
        let mut opts = ParseOptions::default();
        opts.parse_bytecode(false);
        let mut wanted: Vec<String> = plugin_classes.iter().map(|p| format!("{}.class", p.replace('.', "/"))).collect();
        wanted.extend(
            names
                .iter()
                .filter(|n| n.ends_with(".class") && !n.starts_with("META-INF/"))
                .filter(|n| {
                    let simple = n.rsplit('/').next().unwrap_or("");
                    simple.contains("Config") || simple.contains("Plugin") || simple.contains("Compat")
                })
                .take(60)
                .cloned(),
        );
        for w in wanted {
            if let Some(b) = read_entry(&mut z, &w)
                && let Ok(cf) = parse_class_with_options(&b, &opts)
            {
                for item in cf.constantpool_iter() {
                    if let ConstantPoolItem::LiteralConstant(cafebabe::constant_pool::LiteralConstant::String(t)) = item {
                        plugin_strings.insert(t.to_string());
                    }
                }
            }
        }
    }
    // `lithium:options` (e parentes) declarados no descritor desligam opções de outro mod.
    let mut disables = Vec::new();
    for desc in ["fabric.mod.json", "META-INF/mods.toml", "META-INF/neoforge.mods.toml"] {
        if let Some(b) = read_entry(&mut z, desc) {
            let t = String::from_utf8_lossy(&b).to_string();
            disables.extend(declared_disables(&t));
        }
    }

    out.push(Unit {
        modid,
        version,
        jar: jar_name.to_string(),
        nested,
        deps,
        packages: packages.into_iter().collect(),
        configs: config_names,
        classes,
        annotation_counts: counts,
        errors,
        plugin_strings: plugin_strings.into_iter().collect(),
        disables,
    });

    for nj in nested_jars {
        if let Some(bytes) = read_entry(&mut z, &nj) {
            scan_jar_bytes(Cursor::new(bytes), &format!("{jar_name}!/{nj}"), true, loader, out);
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn parse_mixin_class(
    bytes: &[u8],
    internal: &str,
    config: &str,
    cfg_priority: i32,
    plugin: bool,
    default_require: Option<i32>,
    refmap: &Refmap,
    counts: &mut BTreeMap<String, u32>,
) -> Result<MixinClass, String> {
    let mut opts = ParseOptions::default();
    opts.parse_bytecode(false);
    let class = parse_class_with_options(bytes, &opts).map_err(|e| e.to_string())?;

    let mut targets = Vec::new();
    let mut priority = cfg_priority;
    let mut is_mixin = false;
    for a in annotations_of(&class.attributes) {
        if type_name(a) != MIXIN_DESC {
            continue;
        }
        is_mixin = true;
        *counts.entry("Mixin".into()).or_default() += 1;
        if let Some(AnnotationElementValue::ArrayValue(v)) = element(a, "value") {
            for x in v {
                if let AnnotationElementValue::ClassLiteral { class_name } = x {
                    targets.push(desc_to_internal(class_name));
                }
            }
        }
        for t in strings(element(a, "targets")) {
            targets.push(refmap.map(internal, &t).replace('.', "/"));
        }
        if let Some(p) = int(element(a, "priority")) {
            priority = p;
        }
    }
    if !is_mixin {
        return Err("sem @Mixin".into());
    }

    let mut refs: BTreeSet<String> = BTreeSet::new();
    for item in class.constantpool_iter() {
        match item {
            ConstantPoolItem::ClassInfo(c) => {
                refs.insert(desc_to_internal(&c));
            }
            ConstantPoolItem::MethodRef(m) | ConstantPoolItem::FieldRef(m) | ConstantPoolItem::InterfaceMethodRef(m) => {
                refs.insert(m.class_name.to_string());
            }
            _ => {}
        }
    }
    for m in &class.methods {
        let d = m.descriptor.to_string();
        for part in d.split(';') {
            if let Some(i) = part.rfind('L') {
                refs.insert(part[i + 1..].to_string());
            }
        }
    }
    refs.extend(targets.iter().cloned());

    let mut changes = Vec::new();
    for m in &class.methods {
        for a in annotations_of(&m.attributes) {
            let tn = type_name(a);
            if let Some(k) = counted_only(&tn) {
                *counts.entry(k.into()).or_default() += 1;
                continue;
            }
            let Some(kind) = injector_kind(&tn) else { continue };
            *counts.entry(kind.into()).or_default() += 1;
            let require = int(element(a, "require"));
            let selector = selector_of(kind, a, &m.descriptor.to_string());
            let methods: Vec<String> = if kind == "Overwrite" {
                vec![m.name.to_string()]
            } else {
                strings(element(a, "method")).iter().map(|s| method_name(refmap.map(internal, s))).collect()
            };
            // Pontos de injeção (`@At` único ou lista).
            let mut ats: Vec<(String, String)> = Vec::new();
            match element(a, "at") {
                Some(AnnotationElementValue::AnnotationValue(at)) => ats.push(at_key(at, internal, refmap)),
                Some(AnnotationElementValue::ArrayValue(v)) => {
                    for x in v {
                        if let AnnotationElementValue::AnnotationValue(at) = x {
                            ats.push(at_key(at, internal, refmap));
                        }
                    }
                }
                _ => {}
            }
            if ats.is_empty() {
                let v = match kind {
                    "Overwrite" => "OVERWRITE",
                    "ModifyConstant" => "CONSTANT",
                    "WrapMethod" => "WRAP_METHOD",
                    _ => "?",
                };
                let t = if kind == "ModifyConstant" { constant_key(a) } else { String::new() };
                ats.push((v.to_string(), t));
            }
            for tc in &targets {
                for mm in &methods {
                    for (av, at) in &ats {
                        changes.push(Change {
                            kind: kind.into(),
                            target_class: tc.clone(),
                            method: mm.clone(),
                            at_value: av.clone(),
                            at_target: at.clone(),
                            require,
                            mixin_class: internal.to_string(),
                            priority,
                            selector: selector.clone(),
                        });
                    }
                }
            }
        }
    }

    Ok(MixinClass {
        name: internal.to_string(),
        config: config.to_string(),
        plugin,
        default_require,
        targets,
        changes,
        refs: refs.into_iter().collect(),
    })
}

fn at_key(at: &Annotation<'_>, internal: &str, refmap: &Refmap) -> (String, String) {
    let value = strings(element(at, "value")).into_iter().next().unwrap_or_default();
    let target = strings(element(at, "target"))
        .into_iter()
        .next()
        .map(|t| refmap.map(internal, &t).to_string())
        .unwrap_or_default();
    (value, target)
}

fn constant_key(a: &Annotation<'_>) -> String {
    let c: Vec<&Annotation<'_>> = match element(a, "constant") {
        Some(AnnotationElementValue::AnnotationValue(x)) => vec![x],
        Some(AnnotationElementValue::ArrayValue(v)) => v
            .iter()
            .filter_map(|x| match x {
                AnnotationElementValue::AnnotationValue(a) => Some(a),
                _ => None,
            })
            .collect(),
        _ => vec![],
    };
    c.iter()
        .map(|x| {
            x.elements
                .iter()
                .map(|e| format!("{}={}", e.name, value_text(&e.value)))
                .collect::<Vec<_>>()
                .join(",")
        })
        .collect::<Vec<_>>()
        .join("|")
}

fn value_text(v: &AnnotationElementValue<'_>) -> Cow<'static, str> {
    match v {
        AnnotationElementValue::IntConstant(i) => i.to_string().into(),
        AnnotationElementValue::LongConstant(i) => i.to_string().into(),
        AnnotationElementValue::FloatConstant(f) => f.to_string().into(),
        AnnotationElementValue::DoubleConstant(f) => f.to_string().into(),
        AnnotationElementValue::StringConstant(s) => s.to_string().into(),
        AnnotationElementValue::BooleanConstant(b) => b.to_string().into(),
        AnnotationElementValue::ClassLiteral { class_name } => class_name.to_string().into(),
        _ => "?".into(),
    }
}

// ---------------------------------------------------------------------------
// Varredura da pasta, cache e cruzamento.

#[derive(Serialize, Deserialize)]
struct CacheEntry {
    key: String,
    units: Vec<Unit>,
}

fn cache_key(p: &Path) -> String {
    let md = std::fs::metadata(p).ok();
    let len = md.as_ref().map(|m| m.len()).unwrap_or(0);
    let mt = md
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("v1:{len}:{mt}")
}

pub fn scan_dir(dir: &Path, loader: Loader, threads: usize, cache: Option<&Path>) -> Vec<Unit> {
    let mut jars: Vec<PathBuf> = std::fs::read_dir(dir)
        .expect("pasta de mods")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x.eq_ignore_ascii_case("jar")))
        .collect();
    jars.sort();
    let threads = threads.max(1);
    let chunks: Vec<Vec<PathBuf>> = (0..threads)
        .map(|i| jars.iter().skip(i).step_by(threads).cloned().collect())
        .collect();
    let mut all: Vec<(String, Vec<Unit>)> = std::thread::scope(|s| {
        let hs: Vec<_> = chunks
            .into_iter()
            .map(|chunk| {
                s.spawn(move || {
                    let mut res = Vec::new();
                    for p in chunk {
                        let name = p.file_name().unwrap().to_string_lossy().to_string();
                        let key = cache_key(&p);
                        let cfile = cache.map(|c| c.join(format!("{name}.json")));
                        if let Some(cf) = &cfile
                            && let Ok(b) = std::fs::read(cf)
                            && let Ok(e) = serde_json::from_slice::<CacheEntry>(&b)
                            && e.key == key
                        {
                            res.push((name, e.units));
                            continue;
                        }
                        let mut units = Vec::new();
                        match std::fs::File::open(&p) {
                            Ok(f) => scan_jar_bytes(std::io::BufReader::new(f), &name, false, loader, &mut units),
                            Err(e) => eprintln!("{name}: {e}"),
                        }
                        if let Some(cf) = &cfile {
                            let e = CacheEntry { key, units };
                            let _ = std::fs::write(cf, serde_json::to_vec(&e).unwrap());
                            units = e.units;
                        }
                        res.push((name, units));
                    }
                    res
                })
            })
            .collect();
        hs.into_iter().flat_map(|h| h.join().unwrap()).collect()
    });
    all.sort_by(|a, b| a.0.cmp(&b.0));
    all.into_iter().flat_map(|(_, u)| u).collect()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub enum Risk {
    Baixo,
    Medio,
    Alto,
}

impl Risk {
    fn down(self) -> Self {
        match self {
            Risk::Alto => Risk::Medio,
            _ => Risk::Baixo,
        }
    }
}

#[derive(Serialize)]
pub struct Overlap {
    pub target_class: String,
    pub method: String,
    pub owners: Vec<String>,
    pub base: Risk,
    pub rule: String,
    pub involved: Vec<String>,
    pub demoters: Vec<String>,
    /// Um nível a menos se houver pelo menos um rebaixador (leitura adotada).
    pub final_one: Risk,
    /// Um nível a menos por rebaixador (leitura alternativa, para comparação).
    pub final_stacked: Risk,
    pub require_elevator: bool,
}

struct Ent<'a> {
    owner: &'a str,
    unit: &'a Unit,
    class: &'a MixinClass,
    ch: &'a Change,
}

fn body_point(v: &str) -> bool {
    matches!(v, "INVOKE" | "INVOKE_ASSIGN" | "INVOKE_STRING" | "FIELD" | "CONSTANT" | "NEW" | "JUMP")
}

/// Cruza as alterações de mods diferentes. Dono = mod de topo (o jar que o
/// usuário pôs no pack); aninhados repetidos contam uma vez (primeira cópia).
/// Modo do cruzamento: `refined = false` aplica as regras da R5A §5.5 como estão
/// escritas; `refined = true` acrescenta o que o spike mostrou ser necessário
/// (opções desligadas, seletor de variável/argumento, plugin que cita o outro mod).
pub struct Mode<'a> {
    pub refined: bool,
    pub disabled: &'a HashSet<String>,
}

fn is_disabled(owner_modid: &str, class: &str, disabled: &HashSet<String>) -> bool {
    let Some(opt) = mixin_option(class) else { return false };
    let fam = owner_modid;
    let mut prefix = String::new();
    for part in opt.split('.') {
        if !prefix.is_empty() {
            prefix.push('.');
        }
        prefix.push_str(part);
        if disabled.contains(&format!("{fam}:{prefix}")) {
            return true;
        }
    }
    false
}

pub fn overlaps(units: &[Unit], known_compatible: &[(String, String)], mode: &Mode<'_>) -> (Vec<Overlap>, usize, usize) {
    // Dono de cada unidade: o jar de topo.
    let mut top_of_jar: HashMap<&str, &str> = HashMap::new();
    for u in units.iter().filter(|u| !u.nested) {
        top_of_jar.insert(u.jar.as_str(), u.modid.as_str());
    }
    let owner_of = |u: &Unit| -> String {
        let top = u.jar.split("!/").next().unwrap_or(&u.jar);
        top_of_jar.get(top).map(|s| s.to_string()).unwrap_or_else(|| u.modid.clone())
    };
    // Jars de topo sempre contam. Entre cópias aninhadas do mesmo modid, o loader
    // carrega uma só: fica a de versão maior; no empate, a do jar de topo que
    // traz mais aninhados (o agregador, como a Fabric API).
    let top_ids: HashSet<&str> = units.iter().filter(|u| !u.nested).map(|u| u.modid.as_str()).collect();
    let mut nested_count: HashMap<&str, usize> = HashMap::new();
    for u in units.iter().filter(|u| u.nested) {
        *nested_count.entry(u.jar.split("!/").next().unwrap_or("")).or_default() += 1;
    }
    let mut best: HashMap<&str, &Unit> = HashMap::new();
    for u in units.iter().filter(|u| u.nested && !top_ids.contains(u.modid.as_str())) {
        let score = |x: &Unit| (version_key(&x.version), nested_count.get(x.jar.split("!/").next().unwrap_or("")).copied().unwrap_or(0));
        match best.get(u.modid.as_str()) {
            Some(b) if score(b) >= score(u) => {}
            _ => {
                best.insert(u.modid.as_str(), u);
            }
        }
    }
    let picked: Vec<&Unit> = units
        .iter()
        .filter(|u| !u.nested || best.get(u.modid.as_str()).is_some_and(|b| std::ptr::eq(*b, *u)))
        .collect();
    let owner_names: Vec<String> = picked.iter().map(|u| owner_of(u)).collect();
    let kept: Vec<(&Unit, &str)> = picked.iter().copied().zip(owner_names.iter().map(String::as_str)).collect();

    // Pacotes e dependências por dono.
    let mut pkg_owner: HashMap<&str, BTreeSet<&str>> = HashMap::new();
    let mut deps_of: HashMap<&str, HashSet<&str>> = HashMap::new();
    let mut ids_of: HashMap<&str, HashSet<&str>> = HashMap::new();
    for &(u, o) in &kept {
        for p in &u.packages {
            pkg_owner.entry(p.as_str()).or_default().insert(o);
        }
        deps_of.entry(o).or_default().extend(u.deps.iter().map(String::as_str));
        ids_of.entry(o).or_default().insert(u.modid.as_str());
    }

    let mut by_method: BTreeMap<(&str, &str), Vec<Ent<'_>>> = BTreeMap::new();
    let mut by_class: BTreeMap<&str, BTreeSet<String>> = BTreeMap::new();
    let mut removed_by_option = 0usize;
    for &(u, o) in &kept {
        for c in &u.classes {
            for t in &c.targets {
                by_class.entry(t.as_str()).or_default().insert(o.to_string());
            }
            if mode.refined && is_disabled(&u.modid, &c.name, mode.disabled) {
                removed_by_option += 1;
                continue;
            }
            for ch in &c.changes {
                by_method
                    .entry((ch.target_class.as_str(), ch.method.as_str()))
                    .or_default()
                    .push(Ent { owner: o, unit: u, class: c, ch });
            }
        }
    }
    let shared_classes = by_class.values().filter(|s| s.len() >= 2).count();

    let mut out = Vec::new();
    for ((tc, m), ents) in by_method {
        let owners: BTreeSet<&str> = ents.iter().map(|e| e.owner).collect();
        if owners.len() < 2 {
            continue;
        }
        let owners_of = |pred: &dyn Fn(&Ent<'_>) -> bool| -> BTreeSet<&str> {
            ents.iter().filter(|e| pred(e)).map(|e| e.owner).collect()
        };
        let at_eq = |a: &Change, b: &Change| {
            a.at_value == b.at_value
                && a.at_target == b.at_target
                && (!mode.refined
                    || a.selector.is_empty()
                    || b.selector.is_empty()
                    || a.selector == "*"
                    || b.selector == "*"
                    || a.selector == b.selector)
        };

        let mut base = Risk::Baixo;
        let mut rule = "só injeções que coexistem".to_string();
        let mut inv: Vec<&Ent<'_>> = ents.iter().collect();

        let ow = owners_of(&|e| e.ch.kind == "Overwrite");
        if ow.len() >= 2 {
            base = Risk::Alto;
            rule = "@Overwrite de 2+ mods".into();
            inv = ents.iter().filter(|e| e.ch.kind == "Overwrite").collect();
        }
        if base < Risk::Alto {
            for e in &ents {
                if !matches!(e.ch.kind.as_str(), "Redirect" | "ModifyConstant") {
                    continue;
                }
                let same: Vec<&Ent<'_>> = ents
                    .iter()
                    .filter(|x| matches!(x.ch.kind.as_str(), "Redirect" | "ModifyConstant") && at_eq(x.ch, e.ch))
                    .collect();
                if same.iter().map(|x| x.owner).collect::<BTreeSet<_>>().len() >= 2 {
                    base = Risk::Alto;
                    rule = "@Redirect/@ModifyConstant de 2+ mods no mesmo @At".into();
                    inv = same;
                    break;
                }
            }
        }
        if base < Risk::Medio && !ow.is_empty() {
            let body: Vec<&Ent<'_>> = ents
                .iter()
                .filter(|x| x.ch.kind != "Overwrite" && body_point(&x.ch.at_value))
                .collect();
            if let Some(b) = body.iter().find(|b| ow.iter().any(|o| *o != b.owner)) {
                base = Risk::Medio;
                rule = "@Overwrite + injeção no corpo".into();
                inv = ents.iter().filter(|e| e.ch.kind == "Overwrite" && e.owner != b.owner).collect();
                inv.push(b);
            }
        }
        if base < Risk::Medio {
            'r: for e in ents.iter().filter(|e| e.ch.kind == "Redirect") {
                for x in &ents {
                    if x.owner != e.owner && x.ch.kind != "Redirect" && at_eq(x.ch, e.ch) {
                        base = Risk::Medio;
                        rule = "@Redirect + outro injetor no mesmo @At".into();
                        inv = vec![e, x];
                        break 'r;
                    }
                }
            }
        }
        if base < Risk::Medio {
            for e in ents.iter().filter(|e| matches!(e.ch.kind.as_str(), "ModifyVariable" | "ModifyArg" | "ModifyArgs")) {
                let same: Vec<&Ent<'_>> = ents
                    .iter()
                    .filter(|x| matches!(x.ch.kind.as_str(), "ModifyVariable" | "ModifyArg" | "ModifyArgs") && at_eq(x.ch, e.ch))
                    .collect();
                if same.iter().map(|x| x.owner).collect::<BTreeSet<_>>().len() >= 2 {
                    base = Risk::Medio;
                    rule = "@ModifyVariable/@ModifyArg(s) de 2+ mods no mesmo @At".into();
                    inv = same;
                    break;
                }
            }
        }
        if base == Risk::Baixo && !ow.is_empty() {
            rule = "@Overwrite + HEAD/RETURN/TAIL".into();
        }

        // Rebaixadores.
        let mut demoters = Vec::new();
        if inv.iter().all(|e| e.class.plugin) {
            demoters.push("plugin: todos os mixins envolvidos têm plugin".to_string());
        }
        let inv_owners: BTreeSet<&str> = inv.iter().map(|e| e.owner).collect();
        let mut compat = None;
        for a in &inv {
            for b in inv_owners.iter().filter(|b| **b != a.owner) {
                let low = format!("{}|{}", a.class.config, a.class.name).to_lowercase();
                if low.contains("compat") {
                    compat = Some(format!("compat: `{}` fica em config/pacote de compatibilidade", a.class.name));
                } else if a.class.refs.iter().any(|r| {
                    r.rfind('/')
                        .and_then(|i| pkg_owner.get(&r[..i]))
                        .is_some_and(|os| os.contains(b) && !os.contains(a.owner))
                }) {
                    compat = Some(format!("compat: `{}` referencia classes de {b}", a.class.name));
                } else if deps_of
                    .get(a.owner)
                    .is_some_and(|d| ids_of.get(b).is_some_and(|ids| ids.iter().any(|i| d.contains(i))))
                {
                    compat = Some(format!("compat: {} declara {b} como dependência/recomendação", a.owner));
                }
                if compat.is_some() {
                    break;
                }
            }
            if compat.is_some() {
                break;
            }
        }
        if compat.is_none() && mode.refined {
            'p: for a in &inv {
                for b in inv_owners.iter().filter(|b| **b != a.owner) {
                    if let Some(ids) = ids_of.get(b)
                        && ids.iter().any(|id| id.len() > 2 && a.unit.plugin_strings.iter().any(|s| s == id))
                    {
                        compat = Some(format!("compat: o plugin/config de {} cita {b}", a.owner));
                        break 'p;
                    }
                }
            }
        }
        if let Some(c) = compat {
            demoters.push(c);
        }
        let pair_known = inv_owners.iter().any(|a| {
            inv_owners.iter().any(|b| {
                a != b && known_compatible.iter().any(|(x, y)| (x == a && y == b) || (x == b && y == a))
            })
        });
        if pair_known {
            demoters.push("lista curada: par conhecidamente compatível".into());
        }
        let final_one = if demoters.is_empty() { base } else { base.down() };
        let mut final_stacked = base;
        for _ in &demoters {
            final_stacked = final_stacked.down();
        }
        let require_elevator = inv
            .iter()
            .any(|e| e.ch.require.or(e.class.default_require).is_some_and(|r| r >= 1));

        out.push(Overlap {
            target_class: tc.to_string(),
            method: m.to_string(),
            owners: owners.iter().map(|s| s.to_string()).collect(),
            base,
            rule,
            involved: inv
                .iter()
                .map(|e| {
                    let who = if e.unit.modid == e.owner { e.owner.to_string() } else { format!("{}/{}", e.owner, e.unit.modid) };
                    format!(
                        "{who}:{}@{}[{}{}]{{{}:{}}}",
                        e.ch.kind,
                        e.class.name.rsplit('/').next().unwrap_or(""),
                        e.ch.priority,
                        if e.class.plugin { ",plugin" } else { "" },
                        e.ch.at_value,
                        e.ch.at_target
                    )
                })
                .collect(),
            demoters,
            final_one,
            final_stacked,
            require_elevator,
        });
    }
    (out, shared_classes, removed_by_option)
}

/// Chave simples de versão: os números em ordem (`0.92.7+1.20.1` → [0, 92, 7, 1, 20, 1]).
fn version_key(v: &str) -> Vec<u64> {
    v.split(|c: char| !c.is_ascii_digit())
        .filter(|s| !s.is_empty())
        .map(|s| s.parse().unwrap_or(0))
        .collect()
}

/// Seletor de `@ModifyVariable` (ordinal/index/name/argsOnly + tipo) e de
/// `@ModifyArg` (index + tipo); `*` para `@ModifyArgs` (mexe em todos).
fn selector_of(kind: &str, a: &Annotation<'_>, handler_desc: &str) -> String {
    let first_param = handler_desc
        .strip_prefix('(')
        .and_then(|d| {
            let bytes = d.as_bytes();
            let mut end = 0;
            while end < bytes.len() && bytes[end] == b'[' {
                end += 1;
            }
            if end < bytes.len() && bytes[end] == b'L' {
                d[end..].find(';').map(|i| &d[..end + i + 1])
            } else if end < bytes.len() && bytes[end] != b')' {
                Some(&d[..end + 1])
            } else {
                None
            }
        })
        .unwrap_or("")
        .to_string();
    let get = |n: &str| element(a, n).map(|v| value_text(v).to_string()).unwrap_or_default();
    match kind {
        "ModifyVariable" => format!(
            "ord={};idx={};name={};args={};type={first_param}",
            get("ordinal"),
            get("index"),
            strings(element(a, "name")).join("|"),
            get("argsOnly")
        ),
        "ModifyArg" => format!("idx={};type={first_param}", get("index")),
        "ModifyArgs" => "*".into(),
        _ => String::new(),
    }
}

/// Lê `lithium:options` / `canary:options` / `radium:options` de um descritor e
/// devolve `familia:opcao` para cada opção posta em `false`.
pub fn declared_disables(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for fam in ["lithium", "canary", "radium"] {
        let key = format!("{fam}:options");
        let mut from = 0;
        while let Some(i) = text[from..].find(&key) {
            let start = from + i + key.len();
            let mut end = (start + 600).min(text.len());
            while !text.is_char_boundary(end) {
                end -= 1;
            }
            let window = &text[start..end];
            // Pula o `"` e o `]`/`=`/`{` logo depois da chave; para no fim do bloco.
            let body_start = window.find(['{', ']']).map(|i| i + 1).unwrap_or(0);
            let body = &window[body_start..];
            let stop = body.find(['}', '[']).unwrap_or(body.len());
            for part in body[..stop].split([',', '\n']) {
                let p = part.replace('"', "");
                if let Some((k, v)) = p.split_once(['=', ':'])
                    && v.trim() == "false"
                    && k.trim().starts_with("mixin.")
                {
                    out.push(format!("{fam}:{}", k.trim()));
                }
            }
            from = start;
        }
    }
    out
}

/// `.../mixin/chunk/palette/PalettedContainerMixin` → `mixin.chunk.palette`.
pub fn mixin_option(class_name: &str) -> Option<String> {
    let i = class_name.find("/mixin/")?;
    let rest = &class_name[i + "/mixin/".len()..];
    let pkg = rest.rsplit_once('/').map(|(p, _)| p)?;
    Some(format!("mixin.{}", pkg.replace('/', ".")))
}

/// Opções desligadas pelas configs do pack (`config/<arquivo>.properties`).
pub fn disabled_from_config_dir(dir: &Path) -> Vec<String> {
    let files = [
        ("lithium.properties", "lithium"),
        ("radium.properties", "radium"),
        ("canary.properties", "canary"),
        ("modernfix-mixins.properties", "modernfix"),
        ("sodium-mixins.properties", "sodium"),
        ("embeddium-mixins.properties", "embeddium"),
        ("xenon-mixins.properties", "xenon"),
    ];
    let mut out = Vec::new();
    for (f, fam) in files {
        if let Ok(t) = std::fs::read_to_string(dir.join(f)) {
            for l in t.lines() {
                if let Some((k, v)) = l.split_once('=')
                    && v.trim() == "false"
                    && k.trim().starts_with("mixin.")
                {
                    out.push(format!("{fam}:{}", k.trim()));
                }
            }
        }
    }
    out
}
