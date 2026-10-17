// Jars e classes de mixin sintéticos (montados na memória), compartilhados pelos testes desta
// crate e pelos testes do comando `xray` da `warden-app` (por `include!`, sem atributos
// internos nem comentários de módulo aqui).
//
// [`MixinClass`] escreve um `.class` de verdade (formato da JVM) com a `@Mixin` e os injetores
// que o raio-x lê; [`JarBuilder`] monta o zip.

use std::collections::HashMap;
use std::io::{Cursor, Write};

use zip::CompressionMethod;
use zip::write::{SimpleFileOptions, ZipWriter};

/// Valor de um elemento de anotação.
#[derive(Clone, Debug)]
pub enum Value {
    Str(String),
    Int(i32),
    Bool(bool),
    /// Descritor de classe (`Lnet/minecraft/class_1;`).
    Class(String),
    Ann(Ann),
    Array(Vec<Value>),
}

pub fn s(text: &str) -> Value {
    Value::Str(text.to_owned())
}

pub fn strs(list: &[&str]) -> Value {
    Value::Array(list.iter().map(|t| s(t)).collect())
}

/// Uma anotação: tipo (nome interno) e elementos.
#[derive(Clone, Debug)]
pub struct Ann {
    pub type_name: String,
    pub elements: Vec<(String, Value)>,
}

impl Ann {
    pub fn new(type_name: &str) -> Self {
        Self {
            type_name: type_name.to_owned(),
            elements: Vec::new(),
        }
    }

    pub fn with(mut self, name: &str, value: Value) -> Self {
        self.elements.push((name.to_owned(), value));
        self
    }
}

pub const SPONGE: &str = "org/spongepowered/asm/mixin/";
pub const EXTRAS_WRAP: &str = "com/llamalad7/mixinextras/injector/wrapoperation/WrapOperation";

/// `@At(value = …, target = …)`.
pub fn at(value: &str, target: &str) -> Value {
    let mut ann = Ann::new(&format!("{SPONGE}injection/At")).with("value", s(value));
    if !target.is_empty() {
        ann = ann.with("target", s(target));
    }
    Value::Ann(ann)
}

/// Um injetor do Mixin (`Inject`, `Redirect`, `ModifyArg`…) no método `method` com o `@At`.
pub fn injector(kind: &str, method: &str, at_value: Value) -> Ann {
    Ann::new(&format!("{SPONGE}injection/{kind}"))
        .with("method", strs(&[method]))
        .with("at", at_value)
}

/// Método de uma classe de mixin.
#[derive(Clone, Debug)]
pub struct Method {
    pub name: String,
    pub descriptor: String,
    pub annotations: Vec<Ann>,
}

/// Classe de mixin sintética.
#[derive(Clone, Debug, Default)]
pub struct MixinClass {
    pub name: String,
    pub targets: Vec<String>,
    pub string_targets: Vec<String>,
    pub priority: Option<i32>,
    pub methods: Vec<Method>,
    /// Classes citadas no constant pool (para "referencia classes do outro mod").
    pub refs: Vec<String>,
    /// Textos no constant pool (para "o plugin cita o outro mod").
    pub strings: Vec<String>,
    /// Sem `@Mixin` (classe comum, como um plugin).
    pub plain: bool,
}

impl MixinClass {
    pub fn new(name: &str, target: &str) -> Self {
        Self {
            name: name.to_owned(),
            targets: vec![target.to_owned()],
            ..Self::default()
        }
    }

    pub fn plain(name: &str) -> Self {
        Self {
            name: name.to_owned(),
            plain: true,
            ..Self::default()
        }
    }

    pub fn method(mut self, name: &str, descriptor: &str, annotation: Ann) -> Self {
        self.methods.push(Method {
            name: name.to_owned(),
            descriptor: descriptor.to_owned(),
            annotations: vec![annotation],
        });
        self
    }

    /// `@Overwrite` no método `name` (o nome do método é o do alvo).
    pub fn overwrite(self, name: &str) -> Self {
        self.method(name, "()V", Ann::new(&format!("{SPONGE}Overwrite")))
    }

    pub fn priority(mut self, priority: i32) -> Self {
        self.priority = Some(priority);
        self
    }

    pub fn refs(mut self, class: &str) -> Self {
        self.refs.push(class.to_owned());
        self
    }

    pub fn string(mut self, text: &str) -> Self {
        self.strings.push(text.to_owned());
        self
    }

    /// Os bytes do `.class`.
    pub fn bytes(&self) -> Vec<u8> {
        let mut pool = Pool::default();
        let this_class = pool.class(&self.name);
        let super_class = pool.class("java/lang/Object");
        for r in &self.refs {
            pool.class(r);
        }
        for text in &self.strings {
            pool.string(text);
        }
        let mut class_attrs = Vec::new();
        if !self.plain {
            let mut mixin = Ann::new(&format!("{SPONGE}Mixin")).with(
                "value",
                Value::Array(
                    self.targets
                        .iter()
                        .map(|t| Value::Class(format!("L{t};")))
                        .collect(),
                ),
            );
            if !self.string_targets.is_empty() {
                let list: Vec<&str> = self.string_targets.iter().map(String::as_str).collect();
                mixin = mixin.with("targets", strs(&list));
            }
            if let Some(priority) = self.priority {
                mixin = mixin.with("priority", Value::Int(priority));
            }
            class_attrs.push(annotations_attr(
                &mut pool,
                "RuntimeInvisibleAnnotations",
                &[mixin],
            ));
        }
        let mut methods = Vec::new();
        for m in &self.methods {
            let name = pool.utf8(&m.name);
            let desc = pool.utf8(&m.descriptor);
            let attr = annotations_attr(&mut pool, "RuntimeVisibleAnnotations", &m.annotations);
            let mut out = Vec::new();
            out.extend(0x0001u16.to_be_bytes());
            out.extend(name.to_be_bytes());
            out.extend(desc.to_be_bytes());
            out.extend(1u16.to_be_bytes());
            out.extend(attr);
            methods.push(out);
        }

        let mut out = vec![0xCA, 0xFE, 0xBA, 0xBE, 0, 0, 0, 61];
        out.extend(pool.bytes());
        out.extend(0x0021u16.to_be_bytes());
        out.extend(this_class.to_be_bytes());
        out.extend(super_class.to_be_bytes());
        out.extend(0u16.to_be_bytes()); // interfaces
        out.extend(0u16.to_be_bytes()); // fields
        out.extend(u16::try_from(methods.len()).expect("métodos").to_be_bytes());
        for m in methods {
            out.extend(m);
        }
        out.extend(
            u16::try_from(class_attrs.len())
                .expect("atributos")
                .to_be_bytes(),
        );
        for a in class_attrs {
            out.extend(a);
        }
        out
    }
}

#[derive(Default)]
struct Pool {
    entries: Vec<Vec<u8>>,
    utf8: HashMap<String, u16>,
    ints: HashMap<i32, u16>,
}

impl Pool {
    fn push(&mut self, bytes: Vec<u8>) -> u16 {
        self.entries.push(bytes);
        u16::try_from(self.entries.len()).expect("pool")
    }

    fn utf8(&mut self, text: &str) -> u16 {
        if let Some(i) = self.utf8.get(text) {
            return *i;
        }
        let mut b = vec![1];
        b.extend(u16::try_from(text.len()).expect("utf8").to_be_bytes());
        b.extend(text.as_bytes());
        let i = self.push(b);
        self.utf8.insert(text.to_owned(), i);
        i
    }

    fn int(&mut self, value: i32) -> u16 {
        if let Some(i) = self.ints.get(&value) {
            return *i;
        }
        let mut b = vec![3];
        b.extend(value.to_be_bytes());
        let i = self.push(b);
        self.ints.insert(value, i);
        i
    }

    fn class(&mut self, name: &str) -> u16 {
        let n = self.utf8(name);
        let mut b = vec![7];
        b.extend(n.to_be_bytes());
        self.push(b)
    }

    fn string(&mut self, text: &str) -> u16 {
        let n = self.utf8(text);
        let mut b = vec![8];
        b.extend(n.to_be_bytes());
        self.push(b)
    }

    fn bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend(
            u16::try_from(self.entries.len() + 1)
                .expect("pool")
                .to_be_bytes(),
        );
        for e in &self.entries {
            out.extend(e);
        }
        out
    }
}

fn annotations_attr(pool: &mut Pool, name: &str, list: &[Ann]) -> Vec<u8> {
    let attr_name = pool.utf8(name);
    let mut body = Vec::new();
    body.extend(u16::try_from(list.len()).expect("anotações").to_be_bytes());
    for a in list {
        annotation(pool, a, &mut body);
    }
    let mut out = Vec::new();
    out.extend(attr_name.to_be_bytes());
    out.extend(u32::try_from(body.len()).expect("atributo").to_be_bytes());
    out.extend(body);
    out
}

fn annotation(pool: &mut Pool, a: &Ann, out: &mut Vec<u8>) {
    let type_index = pool.utf8(&format!("L{};", a.type_name));
    out.extend(type_index.to_be_bytes());
    out.extend(
        u16::try_from(a.elements.len())
            .expect("elementos")
            .to_be_bytes(),
    );
    for (name, value) in &a.elements {
        let n = pool.utf8(name);
        out.extend(n.to_be_bytes());
        element(pool, value, out);
    }
}

fn element(pool: &mut Pool, value: &Value, out: &mut Vec<u8>) {
    match value {
        Value::Str(text) => {
            out.push(b's');
            out.extend(pool.utf8(text).to_be_bytes());
        }
        Value::Int(v) => {
            out.push(b'I');
            out.extend(pool.int(*v).to_be_bytes());
        }
        Value::Bool(v) => {
            out.push(b'Z');
            out.extend(pool.int(i32::from(*v)).to_be_bytes());
        }
        Value::Class(descriptor) => {
            out.push(b'c');
            out.extend(pool.utf8(descriptor).to_be_bytes());
        }
        Value::Ann(a) => {
            out.push(b'@');
            annotation(pool, a, out);
        }
        Value::Array(list) => {
            out.push(b'[');
            out.extend(u16::try_from(list.len()).expect("lista").to_be_bytes());
            for v in list {
                element(pool, v, out);
            }
        }
    }
}

/// Monta um jar (zip) com as entradas dadas.
#[derive(Default)]
pub struct JarBuilder {
    entries: Vec<(String, Vec<u8>, CompressionMethod)>,
}

impl JarBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn file(mut self, name: &str, content: impl AsRef<[u8]>) -> Self {
        self.entries.push((
            name.to_owned(),
            content.as_ref().to_vec(),
            CompressionMethod::Deflated,
        ));
        self
    }

    pub fn stored(mut self, name: &str, content: impl AsRef<[u8]>) -> Self {
        self.entries.push((
            name.to_owned(),
            content.as_ref().to_vec(),
            CompressionMethod::Stored,
        ));
        self
    }

    /// A classe de mixin no caminho do nome dela.
    pub fn class(self, class: &MixinClass) -> Self {
        let path = format!("{}.class", class.name);
        self.file(&path, class.bytes())
    }

    pub fn build(self) -> Vec<u8> {
        let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
        for (name, content, method) in self.entries {
            let options = SimpleFileOptions::default().compression_method(method);
            writer.start_file(name, options).expect("start_file");
            writer.write_all(&content).expect("write");
        }
        writer.finish().expect("finish").into_inner()
    }
}

/// `fabric.mod.json` com mixins, dependências e jars embutidos.
pub fn fabric_mod_json(
    id: &str,
    version: &str,
    mixins: &[&str],
    depends: &[&str],
    jars: &[&str],
) -> String {
    let mixins: Vec<String> = mixins.iter().map(|m| format!("\"{m}\"")).collect();
    let depends: Vec<String> = depends.iter().map(|d| format!("\"{d}\": \"*\"")).collect();
    let jars: Vec<String> = jars
        .iter()
        .map(|j| format!(r#"{{"file": "{j}"}}"#))
        .collect();
    format!(
        r#"{{"schemaVersion": 1, "id": "{id}", "version": "{version}", "mixins": [{}], "depends": {{{}}}, "jars": [{}]}}"#,
        mixins.join(","),
        depends.join(","),
        jars.join(",")
    )
}

/// Config de mixin: pacote, classes (nomes relativos ao pacote), plugin opcional.
pub fn mixin_config(package: &str, classes: &[&str], plugin: Option<&str>) -> String {
    let classes: Vec<String> = classes.iter().map(|c| format!("\"{c}\"")).collect();
    let plugin = plugin.map_or_else(String::new, |p| format!(r#""plugin": "{p}","#));
    format!(
        r#"{{"required": true, "package": "{package}", {plugin} "mixins": [{}], "injectors": {{"defaultRequire": 1}}}}"#,
        classes.join(",")
    )
}
