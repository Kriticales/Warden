//! Versões e faixas Maven, como o Forge e o NeoForge as avaliam.
//!
//! Porte do comportamento do `maven-artifact` **3.8.5** (Apache Maven, Apache-2.0, tag
//! `maven-3.8.5`): `ComparableVersion`, `VersionRange` e `Restriction`. É a versão usada pelo
//! Forge 1.20.1 (`APACHE_MAVEN_ARTIFACT_VERSION = '3.8.5'` no `build.gradle`) e pelo
//! `FancyModLoader` do NeoForge (`apache_maven_artifact_version=3.8.5`). O código foi reescrito em
//! Rust a partir da leitura das classes (sem cópia literal); os casos de `ComparableVersionTest`
//! e `VersionRangeTest` foram portados em `tests` (registro no `THIRD_PARTY.md`).
//!
//! Atenção à semântica real, que difere de algumas documentações: a faixa `1.0` (sem colchetes)
//! é só uma "versão recomendada" e **aceita qualquer versão**; para "1.0 ou maior" o certo é
//! `[1.0,)`. A faixa vazia não aceita nenhuma versão.
//!
//! Diferenças conhecidas: só dígitos ASCII contam como dígitos (o Java usa `Character.isDigit`,
//! que aceita outros dígitos Unicode).

use std::cmp::Ordering;

/// Tipo do número, como no Maven: `IntItem` (até 9 dígitos), `LongItem` (até 18) e
/// `BigIntegerItem`. Tipos diferentes se ordenam pelo tipo, não pelo valor.
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

fn number_item(digits: &str) -> Item {
    // `stripLeadingZeroes` do Maven: se só houver zeros, mantém o texto inteiro.
    let stripped = digits.trim_start_matches('0');
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

fn text_item(value: &str, followed_by_digit: bool) -> Item {
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
        "ga" | "final" | "release" => "",
        "cr" => "rc",
        other => other,
    };
    Item::Text(aliased.to_owned())
}

fn parse_item(is_digit: bool, text: &str) -> Item {
    if is_digit {
        number_item(text)
    } else {
        text_item(text, false)
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
    fn compare(&self, other: Option<&Self>) -> Ordering {
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
            (Self::List(items), None) => items
                .iter()
                .map(|i| i.compare(None))
                .find(|o| *o != Ordering::Equal)
                .unwrap_or(Ordering::Equal),
            (Self::List(left), Some(Self::List(right))) => compare_lists(left, right),
        }
    }
}

fn compare_lists(left: &[Item], right: &[Item]) -> Ordering {
    let len = left.len().max(right.len());
    for i in 0..len {
        let ord = match (left.get(i), right.get(i)) {
            (None, None) => Ordering::Equal,
            (None, Some(r)) => r.compare(None).reverse(),
            (Some(l), r) => l.compare(r),
        };
        if ord != Ordering::Equal {
            return ord;
        }
    }
    Ordering::Equal
}

/// Remove do fim os itens nulos, parando no primeiro item que não é lista (`ListItem.normalize`).
fn normalize(items: &mut Vec<Item>) {
    let mut i = items.len();
    while i > 0 {
        i -= 1;
        let Some(item) = items.get(i) else { break };
        if item.is_null() {
            items.remove(i);
        } else if !matches!(item, Item::List(_)) {
            break;
        }
    }
}

/// Versão Maven (`DefaultArtifactVersion`/`ComparableVersion`). Qualquer texto é válido.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MavenVersion {
    items: Vec<Item>,
}

impl MavenVersion {
    /// Interpreta uma versão Maven. Nunca falha.
    #[must_use]
    pub fn parse(version: &str) -> Self {
        let lower = version.to_lowercase();
        let chars: Vec<char> = lower.chars().collect();
        // A lista corrente é sempre o último item da anterior: guardamos a cadeia como níveis.
        let mut levels: Vec<Vec<Item>> = vec![Vec::new()];
        let mut is_digit = false;
        let mut start = 0usize;
        let text = |from: usize, to: usize| {
            chars
                .get(from..to)
                .unwrap_or(&[])
                .iter()
                .collect::<String>()
        };
        for (i, &c) in chars.iter().enumerate() {
            if c == '.' || c == '-' {
                let item = if i == start {
                    Item::Number(NumberKind::Int, String::new())
                } else {
                    parse_item(is_digit, &text(start, i))
                };
                push(&mut levels, item);
                start = i + 1;
                if c == '-' {
                    levels.push(Vec::new());
                }
            } else if c.is_ascii_digit() {
                if !is_digit && i > start {
                    push(&mut levels, text_item(&text(start, i), true));
                    start = i;
                    levels.push(Vec::new());
                }
                is_digit = true;
            } else {
                if is_digit && i > start {
                    push(&mut levels, parse_item(true, &text(start, i)));
                    start = i;
                    levels.push(Vec::new());
                }
                is_digit = false;
            }
        }
        if chars.len() > start {
            push(&mut levels, parse_item(is_digit, &text(start, chars.len())));
        }
        let mut child: Option<Vec<Item>> = None;
        while let Some(mut level) = levels.pop() {
            if let Some(c) = child.take() {
                level.push(Item::List(c));
            }
            normalize(&mut level);
            child = Some(level);
        }
        Self {
            items: child.unwrap_or_default(),
        }
    }

    /// Ordem do Maven.
    #[must_use]
    pub fn compare(&self, other: &Self) -> Ordering {
        compare_lists(&self.items, &other.items)
    }
}

fn push(levels: &mut [Vec<Item>], item: Item) {
    if let Some(level) = levels.last_mut() {
        level.push(item);
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

    fn parse(spec: &str) -> Result<Self, MavenRangeError> {
        let lower_inclusive = spec.starts_with('[');
        let upper_inclusive = spec.ends_with(']');
        let inner = spec
            .get(1..spec.len().saturating_sub(1))
            .unwrap_or("")
            .trim();
        match inner.find(',') {
            None => {
                if !lower_inclusive || !upper_inclusive {
                    return Err(MavenRangeError::new(
                        "versão única precisa estar entre []",
                        spec,
                    ));
                }
                let v = MavenVersion::parse(inner);
                Ok(Self {
                    lower: Some(v.clone()),
                    lower_inclusive,
                    upper: Some(v),
                    upper_inclusive,
                })
            }
            Some(comma) => {
                let lower_text = inner[..comma].trim();
                let upper_text = inner[comma + 1..].trim();
                if lower_text == upper_text {
                    return Err(MavenRangeError::new(
                        "a faixa tem os dois limites iguais",
                        spec,
                    ));
                }
                let lower = (!lower_text.is_empty()).then(|| MavenVersion::parse(lower_text));
                let upper = (!upper_text.is_empty()).then(|| MavenVersion::parse(upper_text));
                if let (Some(l), Some(u)) = (&lower, &upper)
                    && u.compare(l) == Ordering::Less
                {
                    return Err(MavenRangeError::new(
                        "a faixa contraria a ordem das versões",
                        spec,
                    ));
                }
                Ok(Self {
                    lower,
                    lower_inclusive,
                    upper,
                    upper_inclusive,
                })
            }
        }
    }
}

/// Faixa Maven (`VersionRange.createFromVersionSpec`), como `[1.20.1,1.21)` ou `[47,)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MavenRange {
    restrictions: Vec<Restriction>,
    /// Versão "recomendada" (faixa sem colchetes, como `1.0`), que aceita qualquer versão.
    recommended: Option<MavenVersion>,
}

impl MavenRange {
    /// Interpreta a especificação de faixa.
    pub fn parse(spec: &str) -> Result<Self, MavenRangeError> {
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
            let restriction = Restriction::parse(&process[..=index])?;
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
            recommended = Some(MavenVersion::parse(&process));
            restrictions.push(Restriction::EVERYTHING);
        }
        Ok(Self {
            restrictions,
            recommended,
        })
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

    proptest! {
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
