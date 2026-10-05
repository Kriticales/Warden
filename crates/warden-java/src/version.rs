//! Versões do Java e do Minecraft, com comparação.
//!
//! - [`JavaVersion`]: lê os formatos que aparecem na prática: o antigo do Java 8
//!   (`1.8.0_312-b07`, `8u51`, `jdk8u504-b01`), o novo (`21.0.12.1+1-LTS`, `jdk-25.0.4.1+1`,
//!   `17.0.15`) e os nomes dos runtimes da Mojang (`8u51-cacert462b08`, `16.0.1.9.1`).
//! - [`MinecraftVersion`]: só versões de lançamento numéricas (`1.7.10`, `1.20.1`, `26.3`). A
//!   ordem oficial das versões vem do manifesto da Mojang (P1-05); aqui a comparação numérica
//!   basta, porque as faixas da tabela de compatibilidade só usam versões de lançamento. Uma
//!   pré-versão (`1.20.5-pre1`, `1.21 Pre-Release 1`) conta como a versão que ela antecede;
//!   *snapshots* semanais (`24w14a`) e versões antigas (`b1.7.3`) não são lidas e caem na regra
//!   do JSON da versão (ARCHITECTURE §7.3, regra e).
//! - [`compare_dotted`]: compara versões de loader (`36.2.25`, `1.16.5-36.2.25`).

use std::cmp::Ordering;
use std::fmt;

use serde::{Deserialize, Serialize};

/// Versão de um Java.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, specta::Type,
)]
#[serde(rename_all = "camelCase")]
pub struct JavaVersion {
    /// Versão principal (8, 17, 21, 25…).
    pub major: u32,
    /// Segundo número (quase sempre 0).
    pub minor: u32,
    /// Atualização de segurança: o `312` de `8u312`, o `12` de `21.0.12`.
    pub security: u32,
    /// Quarto número, usado em correções extras (`21.0.12.1`).
    pub patch: u32,
    /// Número do build (`+1`, `-b07`); 0 quando não informado.
    pub build: u32,
}

impl JavaVersion {
    /// Versão com os números dados.
    #[must_use]
    pub const fn new(major: u32, minor: u32, security: u32, patch: u32, build: u32) -> Self {
        Self {
            major,
            minor,
            security,
            patch,
            build,
        }
    }

    /// Lê um texto de versão. `None` se não for reconhecido.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        let text = text.trim();
        let text = text
            .strip_prefix("jdk-")
            .or_else(|| text.strip_prefix("jdk"))
            .unwrap_or(text);
        if let Some(rest) = text.strip_prefix("1.")
            && let Some(version) = parse_legacy_dotted(rest)
        {
            return Some(version);
        }
        if let Some(version) = parse_u_form(text) {
            return Some(version);
        }
        parse_modern(text)
    }

    /// Texto curto para a interface: `8u312` no Java 8, `21.0.12.1` nos outros.
    #[must_use]
    pub fn display(&self) -> String {
        self.to_string()
    }
}

impl fmt::Display for JavaVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.major <= 8 {
            write!(f, "{}u{}", self.major, self.security)
        } else if self.patch > 0 {
            write!(
                f,
                "{}.{}.{}.{}",
                self.major, self.minor, self.security, self.patch
            )
        } else {
            write!(f, "{}.{}.{}", self.major, self.minor, self.security)
        }
    }
}

/// Os dígitos do começo de `text` e o resto.
fn leading_number(text: &str) -> Option<(u32, &str)> {
    let end = text
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(text.len());
    if end == 0 {
        return None;
    }
    let number = text[..end].parse().ok()?;
    Some((number, &text[end..]))
}

/// `8.0_312-b07` (depois de tirar o `1.` de `1.8.0_312-b07`).
fn parse_legacy_dotted(rest: &str) -> Option<JavaVersion> {
    let (major, rest) = leading_number(rest)?;
    let rest = rest.strip_prefix('.')?;
    let (minor, rest) = leading_number(rest)?;
    let (security, rest) = match rest.strip_prefix('_') {
        Some(after) => leading_number(after)?,
        None => (0, rest),
    };
    let build = rest
        .strip_prefix("-b")
        .and_then(leading_number)
        .map_or(0, |(build, _)| build);
    Some(JavaVersion::new(major, minor, security, 0, build))
}

/// `8u312-b07` e `8u51-cacert462b08`.
fn parse_u_form(text: &str) -> Option<JavaVersion> {
    let (major, rest) = leading_number(text)?;
    let rest = rest.strip_prefix('u')?;
    let (security, rest) = leading_number(rest)?;
    let build = rest
        .strip_prefix("-b")
        .and_then(leading_number)
        .map_or(0, |(build, _)| build);
    Some(JavaVersion::new(major, 0, security, 0, build))
}

/// `21.0.12.1+1-LTS`, `17.0.15`, `16.0.1.9.1`, `25`.
fn parse_modern(text: &str) -> Option<JavaVersion> {
    let (numbers, build_part) = match text.split_once('+') {
        Some((numbers, build)) => (numbers, Some(build)),
        None => (text, None),
    };
    // Sufixos como `-LTS` ou `-ea` antes do `+` não fazem parte dos números.
    let numbers = numbers.split('-').next().unwrap_or(numbers);
    let mut parts = [0_u32; 4];
    let mut count = 0;
    for piece in numbers.split('.') {
        let value: u32 = piece.parse().ok()?;
        if count < parts.len() {
            parts[count] = value;
        }
        count += 1;
    }
    if count == 0 || parts[0] == 0 {
        return None;
    }
    let build = build_part
        .and_then(leading_number)
        .map_or(0, |(build, _)| build);
    let [major, minor, security, patch] = parts;
    Some(JavaVersion::new(major, minor, security, patch, build))
}

/// Versão de lançamento do Minecraft (`1.20.1`, `26.3`), comparada número a número.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MinecraftVersion {
    parts: Vec<u32>,
}

impl MinecraftVersion {
    /// Lê uma versão de lançamento ou pré-versão. `None` para *snapshots* semanais, versões
    /// alfa/beta antigas e textos que não são versão.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        let text = text.trim();
        // `1.20.5-pre1`, `1.20.5-rc1`, `1.21 Pre-Release 1`, `26.1-snapshot-2`.
        let base = text.split(['-', ' ']).next().unwrap_or(text);
        let mut parts = Vec::new();
        for piece in base.split('.') {
            if piece.is_empty() || !piece.bytes().all(|b| b.is_ascii_digit()) || piece.len() > 6 {
                return None;
            }
            parts.push(piece.parse().ok()?);
        }
        if parts.len() < 2 {
            return None;
        }
        // Zeros no fim não mudam a versão (`1.20.0` = `1.20`): normalizados, para a igualdade
        // e o hash baterem com a ordem.
        while parts.len() > 2 && parts.last() == Some(&0) {
            parts.pop();
        }
        Some(Self { parts })
    }

    /// Os números da versão.
    #[must_use]
    pub fn parts(&self) -> &[u32] {
        &self.parts
    }
}

impl fmt::Display for MinecraftVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let texts: Vec<String> = self.parts.iter().map(ToString::to_string).collect();
        f.write_str(&texts.join("."))
    }
}

impl Ord for MinecraftVersion {
    fn cmp(&self, other: &Self) -> Ordering {
        compare_numbers(&self.parts, &other.parts)
    }
}

impl PartialOrd for MinecraftVersion {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Compara listas de números; zeros no fim não contam (`1.20` = `1.20.0`).
fn compare_numbers(a: &[u32], b: &[u32]) -> Ordering {
    let len = a.len().max(b.len());
    for index in 0..len {
        let left = a.get(index).copied().unwrap_or(0);
        let right = b.get(index).copied().unwrap_or(0);
        match left.cmp(&right) {
            Ordering::Equal => {}
            other => return other,
        }
    }
    Ordering::Equal
}

/// Os números de uma versão de loader. Aceita o prefixo com a versão do Minecraft que o Forge
/// usa no Maven (`1.16.5-36.2.25` → `36.2.25`) e ignora sufixos (`-beta`). `None` se não houver
/// número.
#[must_use]
pub fn loader_version_numbers(text: &str) -> Option<Vec<u32>> {
    let text = text.trim();
    let text = match text.split_once('-') {
        Some((prefix, rest))
            if prefix.starts_with("1.") && rest.starts_with(|c: char| c.is_ascii_digit()) =>
        {
            rest
        }
        _ => text,
    };
    let mut numbers = Vec::new();
    for piece in text.split('.') {
        match leading_number(piece) {
            Some((number, rest)) => {
                numbers.push(number);
                if !rest.is_empty() {
                    break;
                }
            }
            None => break,
        }
    }
    if numbers.is_empty() {
        None
    } else {
        Some(numbers)
    }
}

/// Compara duas versões de loader com [`loader_version_numbers`]. `None` se uma delas não tem
/// número.
#[must_use]
pub fn compare_dotted(a: &str, b: &str) -> Option<Ordering> {
    Some(compare_numbers(
        &loader_version_numbers(a)?,
        &loader_version_numbers(b)?,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn java_8_nos_formatos_antigos() {
        let cases = [
            ("1.8.0_312-b07", JavaVersion::new(8, 0, 312, 0, 7)),
            ("1.8.0_504", JavaVersion::new(8, 0, 504, 0, 0)),
            ("jdk8u504-b01", JavaVersion::new(8, 0, 504, 0, 1)),
            ("8u51", JavaVersion::new(8, 0, 51, 0, 0)),
            ("8u51-cacert462b08", JavaVersion::new(8, 0, 51, 0, 0)),
            ("8u202", JavaVersion::new(8, 0, 202, 0, 0)),
            ("1.8.0", JavaVersion::new(8, 0, 0, 0, 0)),
        ];
        for (text, expected) in cases {
            assert_eq!(JavaVersion::parse(text), Some(expected), "{text}");
        }
    }

    #[test]
    fn java_moderno() {
        let cases = [
            ("21.0.12.1+1-LTS", JavaVersion::new(21, 0, 12, 1, 1)),
            ("jdk-21.0.12.1+1", JavaVersion::new(21, 0, 12, 1, 1)),
            ("17.0.15", JavaVersion::new(17, 0, 15, 0, 0)),
            ("17.0.20.1+1", JavaVersion::new(17, 0, 20, 1, 1)),
            ("16.0.1.9.1", JavaVersion::new(16, 0, 1, 9, 0)),
            ("25.0.1", JavaVersion::new(25, 0, 1, 0, 0)),
            ("25", JavaVersion::new(25, 0, 0, 0, 0)),
            ("21.0.7+6-LTS", JavaVersion::new(21, 0, 7, 0, 6)),
            ("17-ea", JavaVersion::new(17, 0, 0, 0, 0)),
        ];
        for (text, expected) in cases {
            assert_eq!(JavaVersion::parse(text), Some(expected), "{text}");
        }
    }

    #[test]
    fn textos_que_nao_sao_versao() {
        for text in ["", "abc", "0", "x.1", "jdk", "1.", "u12", "21..1"] {
            assert_eq!(JavaVersion::parse(text), None, "{text:?}");
        }
    }

    #[test]
    fn ordem_e_texto_curto() {
        let older = JavaVersion::parse("1.8.0_302-b08").unwrap();
        let newer = JavaVersion::parse("8u312-b07").unwrap();
        assert!(older < newer);
        assert!(
            JavaVersion::parse("21.0.12+7").unwrap() < JavaVersion::parse("21.0.12.1+1").unwrap()
        );
        assert_eq!(newer.display(), "8u312");
        assert_eq!(
            JavaVersion::parse("21.0.12.1+1").unwrap().to_string(),
            "21.0.12.1"
        );
        assert_eq!(
            JavaVersion::parse("17.0.15").unwrap().to_string(),
            "17.0.15"
        );
    }

    #[test]
    fn minecraft_lancamentos_e_pre_versoes() {
        let v = |text| MinecraftVersion::parse(text).unwrap();
        assert!(v("1.7.10") < v("1.12.2"));
        assert!(v("1.12.2") < v("1.16.5"));
        assert!(v("1.20.4") < v("1.20.5"));
        assert!(v("1.21.11") < v("26.1"));
        assert!(v("1.21.2") < v("1.21.11"));
        assert_eq!(v("1.20"), v("1.20.0"));
        assert_eq!(v("1.20.5-pre1"), v("1.20.5"));
        assert_eq!(v("1.21 Pre-Release 1"), v("1.21"));
        assert_eq!(v("26.1-snapshot-2"), v("26.1"));
        assert_eq!(v("26.3").to_string(), "26.3");
    }

    #[test]
    fn minecraft_snapshots_e_antigas_nao_sao_lidas() {
        for text in [
            "24w14a",
            "b1.7.3",
            "a1.0.4",
            "rd-132211",
            "",
            "1",
            "inf-20100618",
            "1..2",
        ] {
            assert_eq!(MinecraftVersion::parse(text), None, "{text:?}");
        }
    }

    #[test]
    fn versoes_de_loader() {
        assert_eq!(compare_dotted("36.2.25", "36.2.26"), Some(Ordering::Less));
        assert_eq!(
            compare_dotted("1.16.5-36.2.25", "36.2.26"),
            Some(Ordering::Less)
        );
        assert_eq!(
            compare_dotted("36.2.34", "36.2.26"),
            Some(Ordering::Greater)
        );
        assert_eq!(compare_dotted("36.2.26", "36.2.26"), Some(Ordering::Equal));
        assert_eq!(compare_dotted("36.2", "36.2.0"), Some(Ordering::Equal));
        assert_eq!(
            compare_dotted("47.1.106-beta", "47.1.106"),
            Some(Ordering::Equal)
        );
        assert_eq!(compare_dotted("abc", "1.0"), None);
        assert_eq!(
            loader_version_numbers("1.7.10-10.13.4.1614-1.7.10"),
            Some(vec![10, 13, 4, 1614])
        );
    }
}
