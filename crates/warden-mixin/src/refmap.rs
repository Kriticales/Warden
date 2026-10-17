//! Refmap: a tabela que o build do mod grava ao lado das classes de mixin, traduzindo os nomes
//! do código-fonte (Yarn, Mojang) para os do jogo em tempo de execução (*intermediary* no
//! Fabric, SRG no Forge 1.20.1).
//!
//! Além de traduzir o que cada mixin aponta, a tabela ensina nomes legíveis: a chave
//! `Lnet/minecraft/client/render/WorldRenderer;render(…)V` com o valor
//! `Lnet/minecraft/class_761;method_22710(…)V` diz que `class_761` é o `WorldRenderer` e que
//! `method_22710` é o `render`. É a única tradução de nomes da v1 (a camada D, com tabelas
//! completas de mapeamento, é P2; ARCHITECTURE §9.6).

use std::collections::{BTreeMap, HashMap};

/// Refmap lido. JSON ilegível vira uma tabela vazia (os nomes ficam como estão).
#[derive(Debug, Default)]
pub(crate) struct Refmap {
    mappings: HashMap<String, HashMap<String, String>>,
}

impl Refmap {
    pub(crate) fn parse(bytes: &[u8]) -> Self {
        let mut refmap = Self::default();
        let Ok(value) = serde_json::from_slice::<serde_json::Value>(bytes) else {
            return refmap;
        };
        if let Some(mappings) = value.get("mappings") {
            refmap.add(mappings);
        }
        // `data` tem uma tabela por ambiente (`named:intermediary`, `searge`); a primeira
        // tradução de cada chave vale.
        if let Some(data) = value.get("data").and_then(serde_json::Value::as_object) {
            for table in data.values() {
                refmap.add(table);
            }
        }
        refmap
    }

    fn add(&mut self, table: &serde_json::Value) {
        let Some(classes) = table.as_object() else {
            return;
        };
        for (class, entries) in classes {
            let Some(entries) = entries.as_object() else {
                continue;
            };
            let target = self.mappings.entry(class.clone()).or_default();
            for (key, value) in entries {
                if let Some(text) = value.as_str() {
                    target.entry(key.clone()).or_insert_with(|| text.to_owned());
                }
            }
        }
    }

    /// A tradução de `name` para a classe de mixin `mixin` (nome interno), ou o próprio nome.
    pub(crate) fn map<'s>(&'s self, mixin: &str, name: &'s str) -> &'s str {
        self.mappings
            .get(mixin)
            .and_then(|entries| entries.get(name))
            .map_or(name, String::as_str)
    }

    /// Os nomes legíveis que a tabela ensina (nome em tempo de execução → nome legível), sem
    /// repetir.
    pub(crate) fn learned_names(&self) -> Vec<(String, String)> {
        let mut out = BTreeMap::new();
        for entries in self.mappings.values() {
            for (named, runtime) in entries {
                learn(named, runtime, &mut out);
            }
        }
        out.into_iter().collect()
    }
}

/// Um nome em tempo de execução que vale a pena traduzir: classe `class_123` do *intermediary*,
/// método `method_123`/campo `field_123` do *intermediary* ou `m_123_`/`f_123_` do SRG.
pub(crate) fn is_obfuscated(name: &str) -> bool {
    let simple = name.rsplit(['/', '$']).next().unwrap_or(name);
    let numbered = |prefix: &str| {
        simple
            .strip_prefix(prefix)
            .is_some_and(|rest| !rest.is_empty() && rest.bytes().all(|b| b.is_ascii_digit()))
    };
    let srg = |prefix: &str| {
        simple
            .strip_prefix(prefix)
            .and_then(|rest| rest.strip_suffix('_'))
            .is_some_and(|rest| !rest.is_empty() && rest.bytes().all(|b| b.is_ascii_digit()))
    };
    numbered("class_") || numbered("method_") || numbered("field_") || srg("m_") || srg("f_")
}

/// Junta os pares (tempo de execução → legível) de uma entrada do refmap.
fn learn(named: &str, runtime: &str, out: &mut BTreeMap<String, String>) {
    // Classes: os `L…;` das duas formas, na mesma ordem.
    let named_classes = class_tokens(named);
    let runtime_classes = class_tokens(runtime);
    if !named_classes.is_empty() && named_classes.len() == runtime_classes.len() {
        for (n, r) in named_classes.iter().zip(&runtime_classes) {
            if n != r && is_obfuscated(r) {
                out.entry((*r).to_owned())
                    .or_insert_with(|| (*n).to_owned());
            }
        }
    } else if !named.contains(['(', ';', ':']) && !runtime.contains(['(', ';', ':']) {
        // Alvo de classe sozinho (`targets` da `@Mixin`): `a/b/C` → `net/minecraft/class_1`.
        let (n, r) = (named.replace('.', "/"), runtime.replace('.', "/"));
        if n != r && is_obfuscated(&r) {
            out.entry(r).or_insert(n);
        }
    }
    // Membro: `[Ldono;]nome(desc)` ou `[Ldono;]nome:desc`.
    let (n, r) = (member_name(named), member_name(runtime));
    if let (Some(n), Some(r)) = (n, r)
        && n != r
        && is_obfuscated(r)
    {
        out.entry(r.to_owned()).or_insert_with(|| n.to_owned());
    }
}

/// Os nomes de classe dentro de `L…;`.
fn class_tokens(text: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find('L') {
        let after = &rest[start + 1..];
        let Some(end) = after.find(';') else { break };
        let token = &after[..end];
        if token.contains('/') && !token.contains(['(', ')', ':', ' ']) {
            out.push(token);
            rest = &after[end + 1..];
        } else {
            rest = after;
        }
    }
    out
}

/// O nome do membro de `Ldono;nome(desc)`, `nome(desc)`, `nome:desc` ou `dono.nome(desc)`.
fn member_name(text: &str) -> Option<&str> {
    let mut text = text;
    if text.starts_with('L')
        && let Some(end) = text.find(';')
        && text[..end].contains('/')
    {
        text = &text[end + 1..];
    }
    let end = text.find(['(', ':']).unwrap_or(text.len());
    let head = &text[..end];
    let head = head.rsplit('.').next().unwrap_or(head);
    (!head.is_empty() && !head.contains(['/', ';', '<', '>', '[', ' '])).then_some(head)
}

/// Nome do método sem dono nem descritor: `Lx/Y;m_1_(I)V` → `m_1_`, `x.Y.m(I)V` → `m`,
/// `m:I` → `m`. Um `*` (todos) e nomes com `<init>` ficam como estão.
#[must_use]
pub fn method_name(text: &str) -> String {
    let mut text = text;
    if text.starts_with('L')
        && let Some(end) = text.find(';')
        && text[..end].contains('/')
    {
        text = &text[end + 1..];
    }
    let end = text.find('(').unwrap_or(text.len());
    let head = &text[..end];
    let head = head.rsplit('.').next().unwrap_or(head);
    head.split(':').next().unwrap_or(head).to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn traduz_e_aprende_nomes() {
        let refmap = Refmap::parse(
            br#"{"mappings":{"a/Mix":{"render(Lnet/minecraft/client/MinecraftClient;)V":"method_9(Lnet/minecraft/class_310;)V",
            "Lnet/minecraft/client/render/WorldRenderer;tick()V":"Lnet/minecraft/class_761;method_3()V",
            "net/minecraft/client/Keyboard":"net/minecraft/class_309"}},
            "data":{"named:intermediary":{"a/Mix":{"x":"y"}}}}"#,
        );
        assert_eq!(
            refmap.map("a/Mix", "render(Lnet/minecraft/client/MinecraftClient;)V"),
            "method_9(Lnet/minecraft/class_310;)V"
        );
        assert_eq!(refmap.map("a/Mix", "x"), "y");
        assert_eq!(refmap.map("b/Outro", "x"), "x");
        let names: HashMap<_, _> = refmap.learned_names().into_iter().collect();
        assert_eq!(
            names["net/minecraft/class_310"],
            "net/minecraft/client/MinecraftClient"
        );
        assert_eq!(
            names["net/minecraft/class_761"],
            "net/minecraft/client/render/WorldRenderer"
        );
        assert_eq!(
            names["net/minecraft/class_309"],
            "net/minecraft/client/Keyboard"
        );
        assert_eq!(names["method_9"], "render");
        assert_eq!(names["method_3"], "tick");
        assert!(!names.contains_key("y"));
    }

    #[test]
    fn json_quebrado_nao_derruba() {
        let refmap = Refmap::parse(b"{nada");
        assert_eq!(refmap.map("a", "b"), "b");
        assert!(refmap.learned_names().is_empty());
        assert!(
            Refmap::parse(br#"{"mappings":[1,2],"data":{"x":3}}"#)
                .learned_names()
                .is_empty()
        );
    }

    #[test]
    fn nomes_de_metodo() {
        assert_eq!(method_name("Lx/Y;m_1_(I)V"), "m_1_");
        assert_eq!(method_name("x.Y.metodo(I)V"), "metodo");
        assert_eq!(method_name("campo:I"), "campo");
        assert_eq!(method_name("<init>"), "<init>");
        assert_eq!(method_name("render"), "render");
    }

    #[test]
    fn reconhece_nomes_ofuscados() {
        for name in [
            "net/minecraft/class_1",
            "method_22",
            "field_3",
            "m_202423_",
            "f_1_",
            "a/B$class_4",
        ] {
            assert!(is_obfuscated(name), "{name}");
        }
        for name in [
            "LevelRenderer",
            "method_",
            "m_x_",
            "render",
            "class_a",
            "m__",
        ] {
            assert!(!is_obfuscated(name), "{name}");
        }
    }
}
