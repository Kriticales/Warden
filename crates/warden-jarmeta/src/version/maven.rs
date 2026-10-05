//! Versões e faixas Maven, como o Forge e o NeoForge as avaliam.
//!
//! Porte do comportamento de `ComparableVersion`, `VersionRange` e `Restriction` do
//! `maven-artifact` (Apache Maven, Apache-2.0) e da cópia antiga que o FML do Forge 1.7.10 a
//! 1.12.2 trazia (`cpw.mods.fml.common.versioning` e `net.minecraftforge.fml.common.versioning`,
//! LGPL-2.1). O código foi reescrito em Rust a partir da leitura das classes, sem cópia literal;
//! os casos de `ComparableVersionTest` e `VersionRangeTest` foram portados em `tests` (registro
//! no `THIRD_PARTY.md`).
//!
//! O loader carrega uma versão do `maven-artifact` que muda com a versão dele, e as regras
//! mudam junto. As variantes ([`MavenFlavor`]) e qual versão de loader usa cada uma foram
//! medidas na lista de bibliotecas (`version.json`) dos instaladores oficiais em 2026-10-04;
//! [`MavenFlavor::for_forge`] e [`MavenFlavor::for_neoforge`] fazem a escolha.
//!
//! Atenção à semântica real, que difere de algumas documentações: a faixa `1.0` (sem colchetes)
//! é só uma "versão recomendada" e **aceita qualquer versão**; para "1.0 ou maior" o certo é
//! `[1.0,)`. A faixa vazia não aceita nenhuma versão.
//!
//! Diferenças conhecidas: só dígitos ASCII contam como dígitos (o Java usa `Character.isDigit`,
//! que aceita outros dígitos Unicode).

use std::cmp::Ordering;

use crate::version::flexver;

/// Variante das regras de versão e faixa, conforme a versão do `maven-artifact` do loader.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub enum MavenFlavor {
    /// Cópia antiga do FML (Forge 1.7.10 a 1.12.2, Forge abaixo de 25): números sem tipo, `-`
    /// só abre sublista entre dígitos, `release` não é apelido.
    LegacyFml,
    /// `maven-artifact` 3.6.0 (Forge 25 a 36.0.46: Minecraft 1.13.2 a 1.16.5).
    V3_6_0,
    /// 3.6.3 (Forge 36.0.48 a 40: Minecraft 1.16.5 a 1.18.2): números com tipo (`int`, `long`,
    /// grande) e `release` como apelido.
    V3_6_3,
    /// 3.8.5 (Forge 41 a 49.1.36: Minecraft 1.19 a 1.20.4; NeoForge 47.1 (1.20.1) a 21.4.61):
    /// comparação com o fim da lista olha todos os itens (MNG-6964).
    #[default]
    V3_8_5,
    /// 3.8.8 e 3.9.x (Forge 49.1.37 em diante; NeoForge 21.4.62 em diante): `.X1` vira sublista
    /// como `-X1`, e faixas como `(,)` passam a valer.
    V3_8_8,
}

impl MavenFlavor {
    /// Variante usada por uma versão do Forge (`47.2.0`, ou `1.20.1-47.2.0`).
    #[must_use]
    pub fn for_forge(version: &str) -> Self {
        let v = loader_number(version);
        let at_least = |b: &str| flexver::compare(&v, b) != Ordering::Less;
        if at_least("49.1.37") {
            Self::V3_8_8
        } else if at_least("41") {
            Self::V3_8_5
        } else if at_least("36.0.47") {
            Self::V3_6_3
        } else if at_least("25") {
            Self::V3_6_0
        } else {
            Self::LegacyFml
        }
    }

    /// Variante usada por uma versão do NeoForge (`21.1.77`, `21.4.62-beta`; `47.1.x` é o
    /// NeoForge de 1.20.1).
    #[must_use]
    pub fn for_neoforge(version: &str) -> Self {
        let v = loader_number(version);
        // 47.1.x é o NeoForge de 1.20.1 (artefato `net.neoforged:forge`), feito do Forge 47.
        if v.split('.').next() == Some("47") || flexver::compare(&v, "21.4.62") == Ordering::Less {
            Self::V3_8_5
        } else {
            Self::V3_8_8
        }
    }

    fn typed_numbers(self) -> bool {
        self >= Self::V3_6_3
    }

    fn release_alias(self) -> bool {
        self >= Self::V3_6_3
    }

    fn compare_whole_list_with_null(self) -> bool {
        self >= Self::V3_8_5
    }

    fn dot_qualifier_sublists(self) -> bool {
        self >= Self::V3_8_8
    }

    fn new_range_bounds_rule(self) -> bool {
        self >= Self::V3_8_8
    }
}

/// Parte numérica da versão do loader: sem o prefixo do Minecraft e sem sufixos (`-beta`).
fn loader_number(version: &str) -> String {
    let after_mc = version.rsplit_once('-').map_or(version, |(left, right)| {
        if left.contains('.') && right.chars().next().is_some_and(|c| c.is_ascii_digit()) {
            right
        } else {
            left
        }
    });
    after_mc
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == '.')
        .collect()
}

/// Tipo do número, como no Maven: `IntItem` (até 9 dígitos), `LongItem` (até 18) e
/// `BigIntegerItem`. Tipos diferentes se ordenam pelo tipo, não pelo valor. Nas variantes
/// antigas tudo é `Int` (um só `BigInteger`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum NumberKind {
    Int,
    Long,
    Big,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Item {
    /// Número: tipo e dígitos sem zeros à esquerda (vazio = zero).
    Number(NumberKind, String),
    /// Qualificador já com os apelidos aplicados (`ga` → vazio, `cr` → `rc`...).
    Text(String),
    List(Vec<Item>),
}

const QUALIFIERS: [&str; 7] = ["alpha", "beta", "milestone", "rc", "snapshot", "", "sp"];
const RELEASE_VERSION_INDEX: &str = "5";

fn comparable_qualifier(qualifier: &str) -> String {
    QUALIFIERS.iter().position(|q| *q == qualifier).map_or_else(
        || format!("{}-{qualifier}", QUALIFIERS.len()),
        |i| i.to_string(),
    )
}

fn number_item(digits: &str, flavor: MavenFlavor) -> Item {
    let stripped = digits.trim_start_matches('0');
    if !flavor.typed_numbers() {
        return Item::Number(NumberKind::Int, stripped.to_owned());
    }
    // `stripLeadingZeroes` do Maven: se só houver zeros, mantém o texto inteiro (e o tipo sai
    // do comprimento dele).
    let kept = if stripped.is_empty() {
        digits
    } else {
        stripped
    };
    let kind = if kept.len() <= 9 {
        NumberKind::Int
    } else if kept.len() <= 18 {
        NumberKind::Long
    } else {
        NumberKind::Big
    };
    Item::Number(kind, stripped.to_owned())
}

fn text_item(value: &str, followed_by_digit: bool, flavor: MavenFlavor) -> Item {
    let expanded = if followed_by_digit && value.len() == 1 {
        match value {
            "a" => "alpha",
            "b" => "beta",
            "m" => "milestone",
            other => other,
        }
    } else {
        value
    };
    let aliased = match expanded {
        "ga" | "final" => "",
        "release" if flavor.release_alias() => "",
        "cr" => "rc",
        other => other,
    };
    Item::Text(aliased.to_owned())
}

fn parse_item(is_digit: bool, text: &str, flavor: MavenFlavor) -> Item {
    if is_digit {
        number_item(text, flavor)
    } else {
        text_item(text, false, flavor)
    }
}

impl Item {
    fn is_null(&self) -> bool {
        match self {
            Self::Number(_, digits) => digits.is_empty(),
            Self::Text(value) => comparable_qualifier(value) == RELEASE_VERSION_INDEX,
            Self::List(items) => items.is_empty(),
        }
    }

    /// `other = None` é a comparação com "nada" (a lista do outro lado acabou).
    fn compare(&self, other: Option<&Self>, flavor: MavenFlavor) -> Ordering {
        match (self, other) {
            (Self::Number(_, digits), None) => {
                if digits.is_empty() {
                    Ordering::Equal
                } else {
                    Ordering::Greater
                }
            }
            (Self::Number(ka, a), Some(Self::Number(kb, b))) => ka
                .cmp(kb)
                .then_with(|| a.len().cmp(&b.len()))
                .then_with(|| a.cmp(b)),
            (Self::Number(..), Some(_)) | (Self::List(_), Some(Self::Text(_))) => Ordering::Greater,
            (Self::Text(value), None) => comparable_qualifier(value)
                .as_str()
                .cmp(RELEASE_VERSION_INDEX),
            (Self::Text(a), Some(Self::Text(b))) => {
                comparable_qualifier(a).cmp(&comparable_qualifier(b))
            }
            (Self::Text(_), Some(_)) | (Self::List(_), Some(Self::Number(..))) => Ordering::Less,
            (Self::List(items), None) => {
                if flavor.compare_whole_list_with_null() {
                    items
                        .iter()
                        .map(|i| i.compare(None, flavor))
                        .find(|o| *o != Ordering::Equal)
                        .unwrap_or(Ordering::Equal)
                } else {
                    items
                        .first()
                        .map_or(Ordering::Equal, |first| first.compare(None, flavor))
                }
            }
            (Self::List(left), Some(Self::List(right))) => compare_lists(left, right, flavor),
        }
    }
}

fn compare_lists(left: &[Item], right: &[Item], flavor: MavenFlavor) -> Ordering {
    let len = left.len().max(right.len());
    for i in 0..len {
        let ord = match (left.get(i), right.get(i)) {
            (None, None) => Ordering::Equal,
            (None, Some(r)) => r.compare(None, flavor).reverse(),
            (Some(l), r) => l.compare(r, flavor),
        };
        if ord != Ordering::Equal {
            return ord;
        }
    }
    Ordering::Equal
}

/// Remove do fim os itens nulos (`ListItem.normalize`). No Maven, continua depois de uma
/// sublista não nula; na cópia antiga do FML, para no primeiro item não nulo.
fn normalize(items: &mut Vec<Item>, flavor: MavenFlavor) {
    let mut i = items.len();
    while i > 0 {
        i -= 1;
        let Some(item) = items.get(i) else { break };
        if item.is_null() {
            items.remove(i);
        } else if flavor == MavenFlavor::LegacyFml || !matches!(item, Item::List(_)) {
            break;
        }
    }
}

/// Versão Maven (`DefaultArtifactVersion`/`ComparableVersion`). Qualquer texto é válido.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MavenVersion {
    items: Vec<Item>,
    flavor: MavenFlavor,
}

/// Sublistas em construção: a lista corrente é sempre o último item da anterior, então a
/// cadeia é guardada como níveis e montada no fim.
struct Levels {
    levels: Vec<Vec<Item>>,
}

impl Levels {
    fn push(&mut self, item: Item) {
        if let Some(level) = self.levels.last_mut() {
            level.push(item);
        }
    }

    fn open(&mut self) {
        self.levels.push(Vec::new());
    }

    fn current_is_empty(&self) -> bool {
        self.levels.last().is_none_or(Vec::is_empty)
    }

    fn normalize_current(&mut self, flavor: MavenFlavor) {
        if let Some(level) = self.levels.last_mut() {
            normalize(level, flavor);
        }
    }

    fn finish(mut self, flavor: MavenFlavor) -> Vec<Item> {
        let mut child: Option<Vec<Item>> = None;
        while let Some(mut level) = self.levels.pop() {
            if let Some(c) = child.take() {
                level.push(Item::List(c));
            }
            normalize(&mut level, flavor);
            child = Some(level);
        }
        child.unwrap_or_default()
    }
}

impl MavenVersion {
    /// Interpreta uma versão com as regras da variante padrão ([`MavenFlavor::V3_8_5`]).
    #[must_use]
    pub fn parse(version: &str) -> Self {
        Self::parse_with(version, MavenFlavor::default())
    }

    /// Interpreta uma versão com as regras de uma variante. Nunca falha.
    #[must_use]
    pub fn parse_with(version: &str, flavor: MavenFlavor) -> Self {
        let lower = version.to_lowercase();
        let chars: Vec<char> = lower.chars().collect();
        let mut levels = Levels {
            levels: vec![Vec::new()],
        };
        let mut is_digit = false;
        let mut start = 0usize;
        let text = |from: usize, to: usize| {
            chars
                .get(from..to)
                .unwrap_or(&[])
                .iter()
                .collect::<String>()
        };
        let legacy = flavor == MavenFlavor::LegacyFml;
        for (i, &c) in chars.iter().enumerate() {
            if c == '.' || c == '-' {
                let item = if i == start {
                    Item::Number(NumberKind::Int, String::new())
                } else {
                    parse_item(is_digit, &text(start, i), flavor)
                };
                levels.push(item);
                start = i + 1;
                if c == '-' {
                    if !legacy {
                        levels.open();
                    } else if is_digit {
                        // 1.0-* = 1-*; sublista nova só entre dígitos (1.1 contra 1-1).
                        levels.normalize_current(flavor);
                        if chars.get(i + 1).is_some_and(char::is_ascii_digit) {
                            levels.open();
                        }
                    }
                }
            } else if c.is_ascii_digit() {
                if !is_digit && i > start {
                    if flavor.dot_qualifier_sublists() && !levels.current_is_empty() {
                        levels.open();
                    }
                    levels.push(text_item(&text(start, i), true, flavor));
                    start = i;
                    if !legacy {
                        levels.open();
                    }
                }
                is_digit = true;
            } else {
                if is_digit && i > start {
                    levels.push(parse_item(true, &text(start, i), flavor));
                    start = i;
                    if !legacy {
                        levels.open();
                    }
                }
                is_digit = false;
            }
        }
        if chars.len() > start {
            if flavor.dot_qualifier_sublists() && !is_digit && !levels.current_is_empty() {
                levels.open();
            }
            levels.push(parse_item(is_digit, &text(start, chars.len()), flavor));
        }
        Self {
            items: levels.finish(flavor),
            flavor,
        }
    }

    /// Ordem do Maven (com as regras da variante desta versão).
    #[must_use]
    pub fn compare(&self, other: &Self) -> Ordering {
        compare_lists(&self.items, &other.items, self.flavor)
    }
}

/// Erro ao interpretar uma faixa Maven (`InvalidVersionSpecificationException`).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{message}")]
pub struct MavenRangeError {
    message: String,
}

impl MavenRangeError {
    fn new(message: impl Into<String>, spec: &str) -> Self {
        Self {
            message: format!("{}: {spec}", message.into()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Restriction {
    lower: Option<MavenVersion>,
    lower_inclusive: bool,
    upper: Option<MavenVersion>,
    upper_inclusive: bool,
}

impl Restriction {
    const EVERYTHING: Self = Self {
        lower: None,
        lower_inclusive: false,
        upper: None,
        upper_inclusive: false,
    };

    fn contains(&self, version: &MavenVersion) -> bool {
        if let Some(lower) = &self.lower {
            match lower.compare(version) {
                Ordering::Greater => return false,
                Ordering::Equal if !self.lower_inclusive => return false,
                _ => {}
            }
        }
        if let Some(upper) = &self.upper {
            match upper.compare(version) {
                Ordering::Less => return false,
                Ordering::Equal if !self.upper_inclusive => return false,
                _ => {}
            }
        }
        true
    }

    fn parse(spec: &str, flavor: MavenFlavor) -> Result<Self, MavenRangeError> {
        let lower_inclusive = spec.starts_with('[');
        let upper_inclusive = spec.ends_with(']');
        let inner = spec
            .get(1..spec.len().saturating_sub(1))
            .unwrap_or("")
            .trim();
        let Some(comma) = inner.find(',') else {
            if !lower_inclusive || !upper_inclusive {
                return Err(MavenRangeError::new(
                    "versão única precisa estar entre []",
                    spec,
                ));
            }
            let v = MavenVersion::parse_with(inner, flavor);
            return Ok(Self {
                lower: Some(v.clone()),
                lower_inclusive,
                upper: Some(v),
                upper_inclusive,
            });
        };
        let lower_text = inner[..comma].trim();
        let upper_text = inner[comma + 1..].trim();
        if !flavor.new_range_bounds_rule() && lower_text == upper_text {
            return Err(MavenRangeError::new(
                "a faixa tem os dois limites iguais",
                spec,
            ));
        }
        let lower = (!lower_text.is_empty()).then(|| MavenVersion::parse_with(lower_text, flavor));
        let upper = (!upper_text.is_empty()).then(|| MavenVersion::parse_with(upper_text, flavor));
        if let (Some(l), Some(u)) = (&lower, &upper) {
            let ord = u.compare(l);
            let invalid = ord == Ordering::Less
                || (flavor.new_range_bounds_rule()
                    && ord == Ordering::Equal
                    && (!lower_inclusive || !upper_inclusive));
            if invalid {
                return Err(MavenRangeError::new(
                    "a faixa contraria a ordem das versões",
                    spec,
                ));
            }
        }
        Ok(Self {
            lower,
            lower_inclusive,
            upper,
            upper_inclusive,
        })
    }
}

/// Faixa Maven (`VersionRange.createFromVersionSpec`), como `[1.20.1,1.21)` ou `[47,)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MavenRange {
    restrictions: Vec<Restriction>,
    /// Versão "recomendada" (faixa sem colchetes, como `1.0`), que aceita qualquer versão.
    recommended: Option<MavenVersion>,
    flavor: MavenFlavor,
}

impl MavenRange {
    /// Interpreta a faixa com as regras da variante padrão ([`MavenFlavor::V3_8_5`]).
    pub fn parse(spec: &str) -> Result<Self, MavenRangeError> {
        Self::parse_with(spec, MavenFlavor::default())
    }

    /// Interpreta a faixa com as regras de uma variante.
    pub fn parse_with(spec: &str, flavor: MavenFlavor) -> Result<Self, MavenRangeError> {
        let mut restrictions: Vec<Restriction> = Vec::new();
        let mut process = spec.to_owned();
        let mut upper_bound: Option<MavenVersion> = None;
        let mut recommended = None;
        while process.starts_with('[') || process.starts_with('(') {
            let close_paren = process.find(')');
            let close_bracket = process.find(']');
            let index = match (close_paren, close_bracket) {
                (Some(p), Some(b)) => p.min(b),
                (Some(p), None) => p,
                (None, Some(b)) => b,
                (None, None) => return Err(MavenRangeError::new("faixa sem fechamento", spec)),
            };
            let restriction = Restriction::parse(&process[..=index], flavor)?;
            if let Some(upper) = &upper_bound {
                let overlaps = restriction
                    .lower
                    .as_ref()
                    .is_none_or(|lower| lower.compare(upper) == Ordering::Less);
                if overlaps {
                    return Err(MavenRangeError::new("faixas sobrepostas", spec));
                }
            }
            upper_bound.clone_from(&restriction.upper);
            restrictions.push(restriction);
            process = process[index + 1..].trim().to_owned();
            if let Some(rest) = process.strip_prefix(',') {
                process = rest.trim().to_owned();
            }
        }
        if !process.is_empty() {
            if !restrictions.is_empty() {
                return Err(MavenRangeError::new(
                    "com vários conjuntos, todos precisam de colchetes ou parênteses",
                    spec,
                ));
            }
            recommended = Some(MavenVersion::parse_with(&process, flavor));
            restrictions.push(Restriction::EVERYTHING);
        }
        Ok(Self {
            restrictions,
            recommended,
            flavor,
        })
    }

    /// `containsVersion` do Maven, com a versão interpretada pela mesma variante da faixa.
    #[must_use]
    pub fn contains_text(&self, version: &str) -> bool {
        self.contains(&MavenVersion::parse_with(version, self.flavor))
    }

    /// `containsVersion` do Maven.
    #[must_use]
    pub fn contains(&self, version: &MavenVersion) -> bool {
        self.restrictions.iter().any(|r| r.contains(version))
    }

    /// `true` se a faixa é só uma versão recomendada (sem colchetes), que aceita tudo.
    #[must_use]
    pub fn is_recommendation_only(&self) -> bool {
        self.recommended.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn cmp(a: &str, b: &str) -> Ordering {
        MavenVersion::parse(a).compare(&MavenVersion::parse(b))
    }

    fn check_order(list: &[&str]) {
        for (i, low) in list.iter().enumerate() {
            for high in list.iter().skip(i + 1) {
                assert_eq!(cmp(low, high), Ordering::Less, "esperado {low} < {high}");
                assert_eq!(cmp(high, low), Ordering::Greater, "esperado {high} > {low}");
            }
        }
    }

    fn check_equal(a: &str, b: &str) {
        assert_eq!(cmp(a, b), Ordering::Equal, "esperado {a} == {b}");
        assert_eq!(cmp(b, a), Ordering::Equal, "esperado {b} == {a}");
    }

    // Casos de ComparableVersionTest.java (maven-artifact 3.8.5, Apache-2.0).

    #[test]
    fn maven_ordem_de_qualificadores() {
        check_order(&[
            "1-alpha2snapshot",
            "1-alpha2",
            "1-alpha-123",
            "1-beta-2",
            "1-beta123",
            "1-m2",
            "1-m11",
            "1-rc",
            "1-cr2",
            "1-rc123",
            "1-SNAPSHOT",
            "1",
            "1-sp",
            "1-sp2",
            "1-sp123",
            "1-abc",
            "1-def",
            "1-pom-1",
            "1-1-snapshot",
            "1-1",
            "1-2",
            "1-123",
        ]);
    }

    #[test]
    fn maven_ordem_de_numeros() {
        check_order(&[
            "2.0", "2-1", "2.0.a", "2.0.0.a", "2.0.2", "2.0.123", "2.1.0", "2.1-a", "2.1b",
            "2.1-c", "2.1-1", "2.1.0.1", "2.2", "2.123", "11.a2", "11.a11", "11.b2", "11.b11",
            "11.m2", "11.m11", "11", "11.a", "11b", "11c", "11m",
        ]);
    }

    #[test]
    fn maven_versoes_iguais() {
        for (a, b) in [
            ("1", "1"),
            ("1", "1.0"),
            ("1", "1.0.0"),
            ("1.0", "1.0.0"),
            ("1", "1-0"),
            ("1", "1.0-0"),
            ("1.0", "1.0-0"),
            ("1a", "1-a"),
            ("1a", "1.0-a"),
            ("1a", "1.0.0-a"),
            ("1.0a", "1-a"),
            ("1.0.0a", "1-a"),
            ("1x", "1-x"),
            ("1x", "1.0-x"),
            ("1x", "1.0.0-x"),
            ("1.0x", "1-x"),
            ("1.0.0x", "1-x"),
            ("1ga", "1"),
            ("1release", "1"),
            ("1final", "1"),
            ("1cr", "1rc"),
            ("1a1", "1-alpha-1"),
            ("1b2", "1-beta-2"),
            ("1m3", "1-milestone-3"),
            ("1X", "1x"),
            ("1A", "1a"),
            ("1B", "1b"),
            ("1M", "1m"),
            ("1Ga", "1"),
            ("1GA", "1"),
            ("1RELEASE", "1"),
            ("1release", "1"),
            ("1RELeaSE", "1"),
            ("1Final", "1"),
            ("1FinaL", "1"),
            ("1FINAL", "1"),
            ("1Cr", "1Rc"),
            ("1cR", "1rC"),
            ("1m3", "1Milestone3"),
            ("1m3", "1MileStone3"),
            ("1m3", "1MILESTONE3"),
            (
                "1-abcdefghijklmnopqrstuvwxyz",
                "1-ABCDEFGHIJKLMNOPQRSTUVWXYZ",
            ),
        ] {
            check_equal(a, b);
        }
    }

    #[test]
    fn maven_comparacoes() {
        for (a, b) in [
            ("1", "2"),
            ("1.5", "2"),
            ("1", "2.5"),
            ("1.0", "1.1"),
            ("1.1", "1.2"),
            ("1.0.0", "1.1"),
            ("1.0.1", "1.1"),
            ("1.1", "1.2.0"),
            ("1.0-alpha-1", "1.0"),
            ("1.0-alpha-1", "1.0-alpha-2"),
            ("1.0-alpha-1", "1.0-beta-1"),
            ("1.0-beta-1", "1.0-SNAPSHOT"),
            ("1.0-SNAPSHOT", "1.0"),
            ("1.0-alpha-1-SNAPSHOT", "1.0-alpha-1"),
            ("1.0", "1.0-1"),
            ("1.0-1", "1.0-2"),
            ("1.0.0", "1.0-1"),
            ("2.0-1", "2.0.1"),
            ("2.0.1-klm", "2.0.1-lmn"),
            ("2.0.1", "2.0.1-xyz"),
            ("2.0.1", "2.0.1-123"),
            ("2.0.1-xyz", "2.0.1-123"),
            // MNG-5568
            ("6.1.0rc3", "6.1.0"),
            ("6.1.0rc3", "6.1H.5-beta"),
            ("6.1.0", "6.1H.5-beta"),
            // MNG-6572
            ("20190126.230843", "1234567890.12345"),
            ("1234567890.12345", "123456789012345.1H.5-beta"),
            ("20190126.230843", "123456789012345.1H.5-beta"),
            (
                "123456789012345.1H.5-beta",
                "12345678901234567890.1H.5-beta",
            ),
            ("1234567890.12345", "12345678901234567890.1H.5-beta"),
            ("20190126.230843", "12345678901234567890.1H.5-beta"),
            // MNG-6964
            ("1-0.alpha", "1"),
            ("1-0.beta", "1"),
            ("1-0.alpha", "1-0.beta"),
        ] {
            check_order(&[a, b]);
        }
    }

    #[test]
    fn maven_zeros_a_esquerda() {
        let ones: Vec<String> = (0..19).map(|n| format!("{}1", "0".repeat(n))).collect();
        let zeros: Vec<String> = (0..19).map(|n| "0".repeat(n + 1)).collect();
        for list in [ones, zeros] {
            for a in &list {
                for b in &list {
                    check_equal(a, b);
                }
            }
        }
    }

    fn contains(spec: &str, version: &str) -> bool {
        MavenRange::parse(spec)
            .expect(spec)
            .contains(&MavenVersion::parse(version))
    }

    // Casos de VersionRangeTest.java (maven-artifact 3.8.5, Apache-2.0).

    #[test]
    fn maven_faixas_validas_e_contencao() {
        assert!(!contains("[1.0,)", "1.0-SNAPSHOT"));
        assert!(contains("[1.0,1.1-SNAPSHOT]", "1.1-SNAPSHOT"));
        assert!(contains("[5.0.9.0,5.0.10.0)", "5.0.9.0"));
        assert!(contains("[1.0,1.2]", "1.1-SNAPSHOT"));
        assert!(contains("[1.0,1.2]", "1.2-SNAPSHOT"));
        assert!(!contains("[1.0,1.2]", "1.0-SNAPSHOT"));
        assert!(contains("[1.0,1.2-SNAPSHOT]", "1.1-SNAPSHOT"));
        assert!(contains("[1.0,1.2-SNAPSHOT]", "1.2-SNAPSHOT"));
        assert!(contains("[1.0-SNAPSHOT,1.2]", "1.0-SNAPSHOT"));
        assert!(contains("[1.0-SNAPSHOT,1.2]", "1.1-SNAPSHOT"));
        assert!(contains("1.0-SNAPSHOT", "1.0-SNAPSHOT"));
        // testContains
        assert!(contains("2.0.5", "2.0.5"));
        assert!(contains("2.0.4", "2.0.5"));
        assert!(contains("[2.0.5]", "2.0.5"));
        assert!(!contains("[2.0.6,)", "2.0.5"));
        assert!(!contains("[2.0.6]", "2.0.5"));
        assert!(contains("[2.0,2.1]", "2.0.5"));
        assert!(!contains("[2.0,2.0.3]", "2.0.5"));
        assert!(contains("[2.0,2.0.5]", "2.0.5"));
        assert!(!contains("[2.0,2.0.5)", "2.0.5"));
        // Estruturas de testRange
        assert!(contains("(,1.0]", "1.0") && !contains("(,1.0]", "1.0.1"));
        assert!(contains("(,1.0],[1.2,)", "0.5") && contains("(,1.0],[1.2,)", "1.3"));
        assert!(!contains("(,1.0],[1.2,)", "1.1"));
        assert!(
            MavenRange::parse("1.0")
                .expect("1.0")
                .is_recommendation_only()
        );
        assert!(
            !MavenRange::parse("[1.0]")
                .expect("[1.0]")
                .is_recommendation_only()
        );
    }

    #[test]
    fn maven_faixas_invalidas() {
        for spec in [
            "(1.0)",
            "[1.0)",
            "(1.0]",
            "(1.0,1.0]",
            "[1.0,1.0)",
            "(1.0,1.0)",
            "[1.1,1.0]",
            "[1.0,1.2),1.3",
            "[1.0,1.2),(1.1,1.3]",
            "[1.1,1.3),(1.0,1.2]",
            "(1.1,1.2],[1.0,1.1)",
            "[1.0",
        ] {
            assert!(
                MavenRange::parse(spec).is_err(),
                "{spec} deveria ser inválida"
            );
        }
    }

    #[test]
    fn faixas_reais_de_mods_forge() {
        assert!(contains("[1.20.1,1.21)", "1.20.1"));
        assert!(!contains("[1.20.1,1.21)", "1.21"));
        assert!(contains("[47,)", "47.2.0"));
        assert!(!contains("[47,)", "46.0.14"));
        assert!(contains("[1.21.1,1.21.2)", "1.21.1"));
        assert!(contains("[21.1.0,)", "21.1.77"));
        assert!(contains("[14.23.5.2847,)", "14.23.5.2860"));
        assert!(!contains("[14.23.5.2847,)", "14.23.5.2838"));
        // Faixa vazia não aceita nada; `*` é versão recomendada e aceita tudo.
        assert!(!contains("", "1.0"));
        assert!(contains("*", "1.0"));
    }

    fn cmp_with(a: &str, b: &str, flavor: MavenFlavor) -> Ordering {
        MavenVersion::parse_with(a, flavor).compare(&MavenVersion::parse_with(b, flavor))
    }

    /// `testMng7644` e a nova ordem de `VERSIONS_NUMBER` do `ComparableVersionTest` 3.8.8.
    #[test]
    fn maven_3_8_8_qualificador_depois_de_ponto() {
        let f = MavenFlavor::V3_8_8;
        for x in [
            "abc",
            "alpha",
            "a",
            "beta",
            "b",
            "def",
            "milestone",
            "m",
            "RC",
        ] {
            assert_eq!(
                cmp_with(&format!("1.0.0.{x}1"), &format!("1.0.0-{x}2"), f),
                Ordering::Less
            );
            assert_eq!(
                cmp_with(&format!("2-{x}"), &format!("2.0.{x}"), f),
                Ordering::Equal
            );
            assert_eq!(
                cmp_with(&format!("2-{x}"), &format!("2.0.0.{x}"), f),
                Ordering::Equal
            );
            assert_eq!(
                cmp_with(&format!("2.0.{x}"), &format!("2.0.0.{x}"), f),
                Ordering::Equal
            );
        }
        let order = [
            "2.0", "2.0.a", "2-1", "2.0.2", "2.0.123", "2.1.0", "2.1-a", "2.1b", "2.1-c", "2.1-1",
            "2.1.0.1", "2.2", "2.123", "11.a2", "11.a11", "11.b2", "11.b11", "11.m2", "11.m11",
            "11", "11.a", "11b", "11c", "11m",
        ];
        for (i, low) in order.iter().enumerate() {
            for high in order.iter().skip(i + 1) {
                assert_eq!(cmp_with(low, high, f), Ordering::Less, "{low} < {high}");
            }
        }
        // Na 3.8.5, 2.0.a e 2.0.0.a eram diferentes (a ordem antiga do mesmo teste).
        assert_eq!(cmp("2.0.a", "2.0.0.a"), Ordering::Less);
    }

    #[test]
    fn faixas_mudam_com_a_variante() {
        // `(,)`: limites iguais (vazios) eram erro até a 3.8.5; da 3.8.8 em diante valem tudo.
        assert!(MavenRange::parse_with("(,)", MavenFlavor::V3_8_5).is_err());
        let all = MavenRange::parse_with("(,)", MavenFlavor::V3_8_8).expect("3.8.8 aceita");
        assert!(all.contains_text("1.0") && all.contains_text("0"));
        // `[1.0,1.0]` passou a valer; `(1.0,1.0]` continua inválida.
        assert!(MavenRange::parse_with("[1.0,1.0]", MavenFlavor::V3_8_5).is_err());
        assert!(MavenRange::parse_with("[1.0,1.0]", MavenFlavor::V3_8_8).is_ok());
        assert!(MavenRange::parse_with("(1.0,1.0]", MavenFlavor::V3_8_8).is_err());
        assert!(MavenRange::parse_with("[1.0,1.00]", MavenFlavor::V3_8_5).is_ok());
        assert!(MavenRange::parse_with("(1.0,1.00]", MavenFlavor::V3_8_8).is_err());
    }

    #[test]
    fn variantes_antigas() {
        // MNG-6964 só a partir da 3.8.5: antes, a lista contra o fim olhava só o 1º item.
        for old in [
            MavenFlavor::LegacyFml,
            MavenFlavor::V3_6_0,
            MavenFlavor::V3_6_3,
        ] {
            assert_eq!(cmp_with("1-0.alpha", "1", old), Ordering::Equal, "{old:?}");
        }
        assert_eq!(
            cmp_with("1-0.alpha", "1", MavenFlavor::V3_8_5),
            Ordering::Less
        );
        // Números com tipo a partir da 3.6.3: 10 zeros viram `long` e passam de um `int`.
        for single in [MavenFlavor::LegacyFml, MavenFlavor::V3_6_0] {
            assert_eq!(
                cmp_with("0000000000.1", "5.1", single),
                Ordering::Less,
                "{single:?}"
            );
        }
        assert_eq!(
            cmp_with("0000000000.1", "5.1", MavenFlavor::V3_6_3),
            Ordering::Greater
        );
        // `release` virou apelido de versão final na 3.6.3; o FML antigo nem `release` conhecia.
        for old in [MavenFlavor::LegacyFml, MavenFlavor::V3_6_0] {
            assert_ne!(cmp_with("1release", "1", old), Ordering::Equal, "{old:?}");
        }
        assert_eq!(
            cmp_with("1release", "1", MavenFlavor::V3_6_3),
            Ordering::Equal
        );
        // FML antigo: `-` só abre sublista entre dígitos; 1.0-* = 1-*.
        let legacy = MavenFlavor::LegacyFml;
        assert_eq!(cmp_with("1.0-1", "1-1", legacy), Ordering::Equal);
        assert_eq!(cmp_with("1-1", "1.1", legacy), Ordering::Less);
        assert_eq!(
            cmp_with("10.13.4.1614", "10.13.4.1558", legacy),
            Ordering::Greater
        );
        assert_eq!(cmp_with("1.0-SNAPSHOT", "1.0", legacy), Ordering::Less);
        assert_eq!(cmp_with("1a1", "1-alpha-1", legacy), Ordering::Equal);
        // Fim da lista no FML antigo: para no primeiro item não nulo.
        assert_eq!(cmp_with("1.0.0-0", "1", legacy), Ordering::Equal);
        assert!(
            MavenRange::parse_with("[14.23.5.2847,)", legacy)
                .expect("faixa")
                .contains_text("14.23.5.2860")
        );
    }

    #[test]
    fn variante_por_versao_do_loader() {
        for (forge, flavor) in [
            ("10.13.4.1614", MavenFlavor::LegacyFml),
            ("1.12.2-14.23.5.2860", MavenFlavor::LegacyFml),
            ("25.0.223", MavenFlavor::V3_6_0),
            ("1.16.5-36.0.46", MavenFlavor::V3_6_0),
            ("36.0.48", MavenFlavor::V3_6_3),
            ("40.3.12", MavenFlavor::V3_6_3),
            ("41.0.1", MavenFlavor::V3_8_5),
            ("1.20.1-47.4.10", MavenFlavor::V3_8_5),
            ("49.1.36", MavenFlavor::V3_8_5),
            ("49.1.37", MavenFlavor::V3_8_8),
            ("66.0.9", MavenFlavor::V3_8_8),
        ] {
            assert_eq!(MavenFlavor::for_forge(forge), flavor, "Forge {forge}");
        }
        for (neo, flavor) in [
            ("47.1.106", MavenFlavor::V3_8_5),
            ("20.2.93", MavenFlavor::V3_8_5),
            ("21.1.255", MavenFlavor::V3_8_5),
            ("21.4.61-beta", MavenFlavor::V3_8_5),
            ("21.4.62-beta", MavenFlavor::V3_8_8),
            ("21.11.45", MavenFlavor::V3_8_8),
            ("26.3.0.48-beta", MavenFlavor::V3_8_8),
        ] {
            assert_eq!(MavenFlavor::for_neoforge(neo), flavor, "NeoForge {neo}");
        }
        let range = MavenRange::parse("[1.0,2.0)").expect("faixa");
        assert!(range.contains_text("1.5"));
    }

    proptest! {
        #[test]
        fn variantes_nunca_entram_em_panico(text in ".{0,40}", version in ".{0,20}") {
            for flavor in [
                MavenFlavor::LegacyFml,
                MavenFlavor::V3_6_0,
                MavenFlavor::V3_6_3,
                MavenFlavor::V3_8_5,
                MavenFlavor::V3_8_8,
            ] {
                let v = MavenVersion::parse_with(&version, flavor);
                prop_assert_eq!(v.compare(&v), Ordering::Equal);
                if let Ok(r) = MavenRange::parse_with(&text, flavor) {
                    let _ = r.contains(&v);
                }
            }
        }

        #[test]
        fn nunca_entra_em_panico(text in ".{0,40}", version in ".{0,20}") {
            let v = MavenVersion::parse(&version);
            if let Ok(r) = MavenRange::parse(&text) {
                let _ = r.contains(&v);
            }
        }

        #[test]
        fn ordem_consistente(a in "[0-9a-z.-]{0,12}", b in "[0-9a-z.-]{0,12}") {
            prop_assert_eq!(cmp(&a, &a), Ordering::Equal);
            prop_assert_eq!(cmp(&a, &b), cmp(&b, &a).reverse());
        }
    }
}
