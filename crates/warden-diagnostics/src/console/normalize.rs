//! Forma da linha (ARCHITECTURE §9.8, "Normalização"): números, coordenadas, UUIDs,
//! hexadecimais, ids de recurso (`minecraft:block/stone`), caminhos e endereços viram marcadores
//! (`Missing texture for {id} at {n}`). Duas linhas com a mesma forma são repetições.
//!
//! Só o ruído vira marcador. Identificadores de código ficam como estão: os nomes intermediary
//! do Fabric (`class_1937.method_8320`, `field_1724`), os SRG do Forge antigo (`func_71411_J`,
//! `field_70170_p`, `p_12345_`), classes internas (`Foo$1`), as referências de mixin
//! (`create.mixins.json:client.MinecraftMixin`, `sodium.mixins.json:core.render.MixinWorldRenderer`)
//! e as linhas de frame do stack trace inteiras (`at pacote.Classe.metodo(Classe.java:123)`,
//! `... 12 more`): dois stack traces ou duas falhas de mixin em lugares diferentes do código nunca
//! têm a mesma forma.
//!
//! Um id de recurso (`minecraft:block/stone`, `create:cogwheel`) é dado do jogo, não código: diz
//! qual bloco, item ou textura, e `Missing texture for {id}` junta as repetições de um mesmo aviso
//! (CA-T13-12). Só conta como id o `namespace:caminho` todo em minúsculas cujo namespace não é um
//! arquivo (`*.json`) e que não continua como nome de código (`.Classe`, `$Interna`).
//!
//! A troca é reversível: o modelo guarda onde cada valor estava e os valores trocados ficam
//! juntos numa só `String` por ocorrência, então "Copiar" devolve as linhas originais sem o
//! console guardar cada linha inteira. O leitor trabalha em bytes ASCII (os marcadores só
//! cortam em fronteiras ASCII), então o texto UTF-8 em volta passa intacto.

/// Começo de um marcador no modelo (uso privado do Unicode); o byte seguinte é o tipo.
pub(crate) const HOLE: char = '\u{E000}';
/// Separa os valores trocados de uma ocorrência.
const SEP: char = '\u{1F}';

/// Tipo de marcador.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Number,
    Id,
    Uuid,
    Hex,
    Path,
    Url,
    /// Linha com os caracteres reservados acima: guardada inteira, sem forma.
    Raw,
}

impl Kind {
    const fn code(self) -> u8 {
        match self {
            Self::Number => b'n',
            Self::Id => b'i',
            Self::Uuid => b'u',
            Self::Hex => b'h',
            Self::Path => b'p',
            Self::Url => b'w',
            Self::Raw => b'r',
        }
    }

    const fn from_code(code: char) -> Option<Self> {
        Some(match code {
            'n' => Self::Number,
            'i' => Self::Id,
            'u' => Self::Uuid,
            'h' => Self::Hex,
            'p' => Self::Path,
            'w' => Self::Url,
            'r' => Self::Raw,
            _ => return None,
        })
    }

    const fn label(self) -> &'static str {
        match self {
            Self::Number => "{n}",
            Self::Id => "{id}",
            Self::Uuid => "{uuid}",
            Self::Hex => "{hex}",
            Self::Path => "{caminho}",
            Self::Url => "{url}",
            Self::Raw => "",
        }
    }
}

/// O modelo da linha e os valores trocados.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Normalized {
    /// O texto com marcadores (interno: [`HOLE`] + tipo).
    pub(crate) template: String,
    /// Os valores trocados, na ordem, separados por um caractere de controle.
    pub(crate) values: String,
}

/// Monta a forma da linha.
pub(crate) fn normalize(text: &str) -> Normalized {
    if text.contains([HOLE, SEP]) {
        // Linha que já tem os caracteres reservados: fica inteira como valor; o modelo guarda
        // o texto (sem os reservados) só para separar uma linha da outra.
        let mut template = String::with_capacity(text.len() + 4);
        template.push(HOLE);
        template.push('r');
        template.extend(
            text.chars()
                .map(|c| if c == HOLE || c == SEP { '\u{FFFD}' } else { c }),
        );
        return Normalized {
            template,
            values: text.to_owned(),
        };
    }
    let bytes = text.as_bytes();
    let mut template: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut values = String::new();
    let mut copied = 0;
    let mut i = 0;
    let mut holes = 0usize;
    let mut hole = |template: &mut Vec<u8>, values: &mut String, kind: Kind, value: &str| {
        let mut buffer = [0u8; 4];
        template.extend_from_slice(HOLE.encode_utf8(&mut buffer).as_bytes());
        template.push(kind.code());
        if holes > 0 {
            values.push(SEP);
        }
        holes += 1;
        values.push_str(value);
    };
    while i < bytes.len() {
        let previous = if i == 0 { None } else { Some(bytes[i - 1]) };
        if previous.is_none_or(|p| p == b'\n') {
            let end = bytes[i..]
                .iter()
                .position(|&b| b == b'\n')
                .map_or(bytes.len(), |n| i + n);
            if is_frame_line(&text[i..end]) {
                // Uma linha de frame é o lugar no código: fica inteira.
                i = end;
                continue;
            }
        }
        if let Some(end) = mixin_ref_at(bytes, i, previous) {
            // Uma referência de mixin (`create.mixins.json:client.MinecraftMixin`) é o lugar no
            // código: fica inteira, em qualquer caixa.
            i = end;
        } else if let Some((kind, end)) = token_at(bytes, i, previous) {
            template.extend_from_slice(&bytes[copied..i]);
            hole(&mut template, &mut values, kind, &text[i..end]);
            i = end;
            copied = end;
        } else if is_ident(bytes[i]) && !bytes[i].is_ascii_digit() {
            // Um identificador (`log4j`, `x86_64`, `class_1937`, `func_71411_J`, `Foo$1`): os
            // dígitos colados a letras, `_` ou `$` ficam como estão.
            while i < bytes.len() && is_ident(bytes[i]) {
                i += 1;
            }
        } else {
            i += 1;
        }
    }
    template.extend_from_slice(&bytes[copied..]);
    Normalized {
        // Só se corta em fronteiras ASCII: continua UTF-8 válido.
        template: String::from_utf8(template).unwrap_or_default(),
        values,
    }
}

/// O texto original a partir do modelo e dos valores de uma ocorrência.
pub(crate) fn fill(template: &str, values: &str) -> String {
    if template.starts_with(HOLE) && template[HOLE.len_utf8()..].starts_with('r') {
        return values.to_owned();
    }
    let mut out = String::with_capacity(template.len() + values.len());
    let mut parts = values.split(SEP);
    let mut chars = template.chars();
    while let Some(c) = chars.next() {
        if c == HOLE {
            chars.next();
            out.push_str(parts.next().unwrap_or_default());
        } else {
            out.push(c);
        }
    }
    out
}

/// O modelo para mostrar (`Missing texture for {id} at {n}`).
pub(crate) fn display(template: &str) -> String {
    let mut out = String::with_capacity(template.len());
    let mut chars = template.chars();
    while let Some(c) = chars.next() {
        if c == HOLE {
            if let Some(kind) = chars.next().and_then(Kind::from_code) {
                out.push_str(kind.label());
            }
        } else {
            out.push(c);
        }
    }
    out
}

const fn is_alnum(b: u8) -> bool {
    b.is_ascii_alphanumeric()
}

/// Letra, dígito, `_` ou `$`: parte de um nome de código.
const fn is_ident(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b == b'$'
}

/// Linha de frame de um stack trace (`\tat pacote.Classe.metodo(…)`, `\t... 12 more`).
fn is_frame_line(line: &str) -> bool {
    let trimmed = line.trim_start();
    trimmed.starts_with("at ") || trimmed.starts_with("... ")
}

/// Um marcador que começa em `i`, com o fim.
fn token_at(bytes: &[u8], i: usize, previous: Option<u8>) -> Option<(Kind, usize)> {
    let b = bytes[i];
    let word_start = previous.is_none_or(|p| !is_ident(p));
    if word_start && b.is_ascii_alphabetic() {
        if let Some(end) = url_at(bytes, i) {
            return Some((Kind::Url, end));
        }
        if let Some(end) = windows_path_at(bytes, i) {
            return Some((Kind::Path, end));
        }
    }
    if word_start
        && b.is_ascii_hexdigit()
        && let Some(end) = uuid_at(bytes, i)
    {
        return Some((Kind::Uuid, end));
    }
    if word_start
        && previous.is_none_or(|p| p != b'.' && p != b'/' && p != b':')
        && (b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
        && let Some(end) = id_at(bytes, i)
    {
        return Some((Kind::Id, end));
    }
    if word_start
        && b.is_ascii_hexdigit()
        && let Some(end) = hex_at(bytes, i)
    {
        return Some((Kind::Hex, end));
    }
    if b == b'/'
        && previous.is_none_or(|p| b" \t\"'([=:,".contains(&p))
        && let Some(end) = unix_path_at(bytes, i)
    {
        return Some((Kind::Path, end));
    }
    let ident_before = previous.is_some_and(|p| p.is_ascii_alphabetic() || p == b'_' || p == b'$');
    if b.is_ascii_digit() && !ident_before {
        return Some((Kind::Number, number_end(bytes, i)));
    }
    if b == b'-'
        && previous.is_none_or(|p| !is_alnum(p))
        && bytes.get(i + 1).is_some_and(u8::is_ascii_digit)
    {
        return Some((Kind::Number, number_end(bytes, i + 1)));
    }
    None
}

/// `123`, `-4`, `1.20.1`, `0.5`.
fn number_end(bytes: &[u8], start: usize) -> usize {
    let mut end = start;
    while end < bytes.len() && bytes[end].is_ascii_digit() {
        end += 1;
    }
    while end + 1 < bytes.len() && bytes[end] == b'.' && bytes[end + 1].is_ascii_digit() {
        end += 1;
        while end < bytes.len() && bytes[end].is_ascii_digit() {
            end += 1;
        }
    }
    end
}

/// Fim de um trecho sem espaço, aspas nem fechamentos.
fn path_end(bytes: &[u8], start: usize) -> usize {
    let mut end = start;
    while end < bytes.len()
        && !matches!(
            bytes[end],
            b' ' | b'\t' | b'"' | b'\'' | b')' | b']' | b'}' | b'>' | b',' | b';'
        )
    {
        end += 1;
    }
    while end > start && matches!(bytes[end - 1], b'.' | b':') {
        end -= 1;
    }
    end
}

/// `https://…`, `file:/…`.
fn url_at(bytes: &[u8], i: usize) -> Option<usize> {
    let mut j = i;
    while j < bytes.len() && (is_alnum(bytes[j]) || matches!(bytes[j], b'+' | b'.' | b'-')) {
        j += 1;
    }
    let scheme = j - i;
    if !(2..=10).contains(&scheme) || bytes.get(j) != Some(&b':') || bytes.get(j + 1) != Some(&b'/')
    {
        return None;
    }
    let end = path_end(bytes, j + 1);
    (end > j + 2).then_some(end)
}

/// `C:\Users\…` e `C:/…`.
fn windows_path_at(bytes: &[u8], i: usize) -> Option<usize> {
    if bytes.get(i + 1) != Some(&b':') || !matches!(bytes.get(i + 2), Some(b'\\' | b'/')) {
        return None;
    }
    let end = path_end(bytes, i + 2);
    (end > i + 3).then_some(end)
}

/// `/home/…`, `/<DADOS>/…`: absoluto, com mais de uma parte.
fn unix_path_at(bytes: &[u8], i: usize) -> Option<usize> {
    let next = *bytes.get(i + 1)?;
    if !(is_alnum(next) || matches!(next, b'.' | b'_' | b'<' | b'~')) {
        return None;
    }
    let end = path_end(bytes, i + 1);
    bytes[i + 1..end].contains(&b'/').then_some(end)
}

/// `123e4567-e89b-12d3-a456-426614174000`.
fn uuid_at(bytes: &[u8], i: usize) -> Option<usize> {
    const GROUPS: [usize; 5] = [8, 4, 4, 4, 12];
    let mut j = i;
    for (index, size) in GROUPS.iter().enumerate() {
        if index > 0 {
            if bytes.get(j) != Some(&b'-') {
                return None;
            }
            j += 1;
        }
        for _ in 0..*size {
            if !bytes.get(j)?.is_ascii_hexdigit() {
                return None;
            }
            j += 1;
        }
    }
    bytes.get(j).is_none_or(|b| !is_alnum(*b)).then_some(j)
}

/// `0x1F`, `6d06d69c` (6+ dígitos hexadecimais, com letra e número), `@6d06d69c`.
fn hex_at(bytes: &[u8], i: usize) -> Option<usize> {
    if bytes[i] == b'0' && matches!(bytes.get(i + 1), Some(b'x' | b'X')) {
        let mut j = i + 2;
        while j < bytes.len() && bytes[j].is_ascii_hexdigit() {
            j += 1;
        }
        return (j > i + 2 && bytes.get(j).is_none_or(|b| !is_alnum(*b))).then_some(j);
    }
    let mut j = i;
    let (mut digit, mut letter) = (false, false);
    while j < bytes.len() && bytes[j].is_ascii_hexdigit() {
        if bytes[j].is_ascii_digit() {
            digit = true;
        } else {
            letter = true;
        }
        j += 1;
    }
    let after_ok = bytes.get(j).is_none_or(|b| !is_alnum(*b) && *b != b'_');
    (j - i >= 6 && digit && letter && after_ok).then_some(j)
}

/// `create.mixins.json:client.MinecraftMixin`, `mixins.jei.json:MixinFoo`: a config de mixin
/// (`*.json`), `:` e o caminho da classe, em qualquer caixa.
fn mixin_ref_at(bytes: &[u8], i: usize, previous: Option<u8>) -> Option<usize> {
    if previous.is_some_and(|p| is_ident(p) || matches!(p, b'.' | b'-')) || !is_alnum(bytes[i]) {
        return None;
    }
    let mut j = i;
    while j < bytes.len() && (is_ident(bytes[j]) || matches!(bytes[j], b'.' | b'-')) {
        j += 1;
    }
    if !bytes[i..j].ends_with(b".json") || bytes.get(j) != Some(&b':') {
        return None;
    }
    let start = j + 1;
    if !bytes
        .get(start)
        .is_some_and(|b| b.is_ascii_alphabetic() || *b == b'_' || *b == b'$')
    {
        return None;
    }
    let mut end = start;
    while end < bytes.len() && (is_ident(bytes[end]) || bytes[end] == b'.') {
        end += 1;
    }
    while bytes[end - 1] == b'.' {
        end -= 1;
    }
    Some(end)
}

/// `minecraft:block/stone`, `create:cogwheel`: espaço com letra, `:` e caminho minúsculo. O
/// namespace de um arquivo (`x.json:`) e o que continua como código (`a:b.Classe`, `a:b$C`) não
/// são ids.
fn id_at(bytes: &[u8], i: usize) -> Option<usize> {
    let mut j = i;
    let mut letter = false;
    while j < bytes.len() && matches!(bytes[j], b'a'..=b'z' | b'0'..=b'9' | b'_' | b'.' | b'-') {
        letter |= bytes[j].is_ascii_lowercase();
        j += 1;
    }
    if !letter || j == i || bytes.get(j) != Some(&b':') {
        return None;
    }
    let start = j + 1;
    if !bytes
        .get(start)
        .is_some_and(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || *b == b'_')
    {
        return None;
    }
    let mut end = start;
    while end < bytes.len()
        && matches!(bytes[end], b'a'..=b'z' | b'0'..=b'9' | b'_' | b'.' | b'/' | b'-')
    {
        end += 1;
    }
    while bytes[end - 1] == b'.' {
        end -= 1;
    }
    // `create:x: motivo` é um id seguido do motivo; `a:b:c` não é id.
    let code_after = match bytes.get(end) {
        Some(b'.') => bytes.get(end + 1).is_some_and(|b| is_ident(*b)),
        Some(b':') => bytes.get(end + 1).is_some_and(|b| !b.is_ascii_whitespace()),
        Some(b) => is_alnum(*b) || *b == b'$',
        None => false,
    };
    let file_namespace = bytes[i..j].ends_with(b".json");
    (!code_after && !file_namespace).then_some(end)
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    fn shape(text: &str) -> String {
        display(&normalize(text).template)
    }

    #[test]
    fn marcadores_da_arquitetura() {
        assert_eq!(
            shape("Missing texture for create:block/cogwheel at 12"),
            "Missing texture for {id} at {n}"
        );
        assert_eq!(
            shape(
                "WardenTest[local:E:a2b80958] logged in with entity id 186 at (0.5, 105.0, -0.5)"
            ),
            "WardenTest[local:E:{hex}] logged in with entity id {n} at ({n}, {n}, {n})"
        );
        assert_eq!(
            shape("Couldn't look up profile properties for 3f8f5c3a-6c2b-3c1b-9a4e-0d4c8f2b1a77"),
            "Couldn't look up profile properties for {uuid}"
        );
        assert_eq!(
            shape(
                r"Failed to add PDH Counter: \Paging File(_Total)\% Usage, Error code: 0xC0000BB8"
            ),
            r"Failed to add PDH Counter: \Paging File(_Total)\% Usage, Error code: {hex}"
        );
        assert_eq!(
            shape(r"Loading C:\Users\Fulano\AppData\x.jar and /home/fulano/.minecraft/mods/y.jar"),
            "Loading {caminho} and {caminho}"
        );
        assert_eq!(shape("See https://github.com/x/y/issues."), "See {url}.");
        assert_eq!(
            shape(
                "\tat net.minecraft.client.Minecraft.run(Minecraft.java:2108) ~[client-1.20.1-srg.jar%23154!/:?]"
            ),
            "\tat net.minecraft.client.Minecraft.run(Minecraft.java:2108) ~[client-1.20.1-srg.jar%23154!/:?]"
        );
        assert_eq!(
            shape("Created: 1024x512x4 minecraft:textures/atlas/blocks.png-atlas"),
            "Created: {n}x512x4 {id}"
        );
    }

    #[test]
    fn nao_troca_nomes_nem_classes() {
        for text in [
            "Mixin apply failed wardenfalhas.mojmap.mixins.json:MixinQuebrado -> net.minecraft.client.Minecraft",
            "Compatibility level set to JAVA_ok",
            "log4j e Java e x86",
            "Error: could not find",
        ] {
            assert_eq!(shape(text), text);
        }
    }

    /// Referências de mixin ficam inteiras em qualquer caixa (pacote em minúsculas, um só
    /// segmento, subpacotes fundos, classe interna); o ruído em volta continua virando marcador.
    #[test]
    fn referencias_de_mixin_ficam() {
        for text in [
            "Mixin [create.mixins.json:client.MinecraftMixin] has no targets",
            "Mixin apply for mod sodium failed sodium.mixins.json:core.render.MixinWorldRenderer from mod sodium -> net.minecraft.class_761",
            "Mixin [mymod.mixins.json:MixinFoo] and [mymod.mixins.json:mixinfoo]",
            "lithium.mixins.json:alloc.chunk_random.v2.ClientWorldMixin$Inner",
            "fabric-networking-api-v1.mixins.json:accessor.ServerLoginNetworkHandlerAccessor",
            "Error in mixins.jei.json:client.ScreenMixin and MyMod.mixins.json:Foo",
            "modernfix-common.mixins.json:perf.cache_strongholds.ServerLevelMixin",
        ] {
            assert_eq!(shape(text), text);
        }
        assert_ne!(
            normalize("Mixin [create.mixins.json:client.MinecraftMixin] has no targets").template,
            normalize("Mixin [create.mixins.json:common.MinecraftMixin] has no targets").template
        );
        assert_ne!(
            normalize("Mixin [mod-1.mixins.json:a.B]").template,
            normalize("Mixin [mod-2.mixins.json:a.B]").template
        );
        assert_eq!(
            shape("Mixin [create.mixins.json:client.MinecraftMixin] at 12 in 0x1F"),
            "Mixin [create.mixins.json:client.MinecraftMixin] at {n} in {hex}"
        );
        // Um arquivo com o número da linha não é referência: o número continua marcador.
        assert_eq!(
            shape("Erro em config/foo.json:12 e data/a.json: x"),
            "Erro em config/foo.json:{n} e data/a.json: x"
        );
    }

    /// Só o `namespace:caminho` que é dado do jogo vira `{id}`; o que continua como nome de
    /// código fica.
    #[test]
    fn ids_de_recurso_e_codigo() {
        for (text, expected) in [
            ("Unknown block minecraft:stone.", "Unknown block {id}."),
            (
                "Missing model fabric-api:item/x_1, retrying",
                "Missing model {id}, retrying",
            ),
            ("Bad recipe create:crushing/raw_zinc_ore", "Bad recipe {id}"),
            (
                "Parsing error loading custom advancement create:root: Missing display",
                "Parsing error loading custom advancement {id}: Missing display",
            ),
            (
                "Biome mymod:plains:12 not found",
                "Biome mymod:plains:{n} not found",
            ),
            (
                "Erro em mod:pacote.Classe e mod:pacote$Interna",
                "Erro em mod:pacote.Classe e mod:pacote$Interna",
            ),
            (
                "Erro em mod:pacote.sub.Classe",
                "Erro em mod:pacote.sub.Classe",
            ),
        ] {
            assert_eq!(shape(text), expected, "{text}");
        }
    }

    /// Nomes do jogo em pack Fabric (intermediary) e Forge antigo (SRG) não viram `{n}`: dois
    /// lugares diferentes do código nunca têm a mesma forma.
    #[test]
    fn nomes_intermediary_e_srg_ficam() {
        for text in [
            "JAVA_17",
            "x86_64",
            "Cannot invoke \"net.minecraft.class_2680.method_26204()\" because \"state\" is null",
            "Exception in class_1937.method_8320 and field_1724",
            "Exception in func_71411_J reading field_70170_p (p_12345_)",
            "Error in Foo$1 and Bar$Inner$2",
        ] {
            assert_eq!(shape(text), text);
        }
        assert_ne!(
            normalize("Erro em class_1937.method_8320").template,
            normalize("Erro em class_2338.method_10074").template
        );
        assert_ne!(
            normalize("Erro em func_71411_J").template,
            normalize("Erro em func_71407_l").template
        );
        // O ruído em volta continua virando marcador.
        assert_eq!(
            shape("class_1937 at 12, 64, -3 em 0x1F"),
            "class_1937 at {n}, {n}, {n} em {hex}"
        );
    }

    /// As linhas de frame ficam inteiras, com o número da linha do código; a mensagem da
    /// exceção continua normalizada.
    #[test]
    fn frames_ficam_inteiros() {
        let a = "Falha em 12, 64\njava.lang.NullPointerException: x 3\n\tat net.minecraft.world.level.Level.getBlockState(Level.java:851) ~[server-1.21.1.jar%23154!/:?]\n\t... 12 more";
        assert_eq!(
            shape(a),
            "Falha em {n}, {n}\njava.lang.NullPointerException: x {n}\n\tat net.minecraft.world.level.Level.getBlockState(Level.java:851) ~[server-1.21.1.jar%23154!/:?]\n\t... 12 more"
        );
        let b = a.replace("Level.java:851", "Level.java:900");
        assert_ne!(normalize(a).template, normalize(&b).template);
        let c = a.replace("em 12, 64", "em 7, -80").replace("x 3", "x 9");
        assert_eq!(normalize(a).template, normalize(&c).template);
    }

    #[test]
    fn repeticoes_tem_a_mesma_forma() {
        let a = normalize("Missing texture for supplementaries:jar_1 at 4");
        let b = normalize("Missing texture for minecraft:stone at 128");
        assert_eq!(a.template, b.template);
        assert_ne!(a.values, b.values);
    }

    #[test]
    fn reservados_ficam_inteiros() {
        let text = "a\u{E000}b 12";
        let normalized = normalize(text);
        assert_eq!(fill(&normalized.template, &normalized.values), text);
        assert_eq!(display(&normalized.template), "a\u{FFFD}b 12");
    }

    proptest! {
        #[test]
        fn volta_ao_original(text in "[ -~\u{e1}\u{e7}\t\u{E000}]{0,80}") {
            let normalized = normalize(&text);
            prop_assert_eq!(fill(&normalized.template, &normalized.values), text);
        }

        #[test]
        fn volta_ao_original_com_tokens(parts in proptest::collection::vec(prop_oneof![
            Just("12".to_owned()), Just("-3.5".to_owned()), Just("minecraft:stone".to_owned()),
            Just("0xFF".to_owned()), Just("6d06d69c".to_owned()), Just("C:\\a\\b".to_owned()),
            Just("/a/b".to_owned()), Just("http://x".to_owned()), Just(" ".to_owned()), Just("Ação".to_owned()),
            Just("3f8f5c3a-6c2b-3c1b-9a4e-0d4c8f2b1a77".to_owned()), Just(":".to_owned()),
        ], 0..12)) {
            let text: String = parts.concat();
            let normalized = normalize(&text);
            prop_assert_eq!(fill(&normalized.template, &normalized.values), text);
        }
    }
}
