//! `FlexVer`: comparação de versões que não seguem `SemVer`.
//!
//! Porte do `FlexVerComparator` de referência
//! (<https://github.com/unascribed/FlexVer>, `java/src/main/java/com/unascribed/flexver/FlexVerComparator.java`,
//! domínio público via CC0-1.0). É o algoritmo que o packwiz usa para escolher versões e o recurso
//! da R3 §4.1 quando a versão de um mod não é `SemVer`.
//!
//! A versão é decomposta em componentes: sequências de dígitos ASCII, sequências de outros
//! caracteres e pré-lançamentos (`-texto`). Tudo depois do primeiro `+` é ignorado.

use std::cmp::Ordering;

/// Um componente da versão decomposta.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Component {
    /// Sequência de caracteres que não são dígitos.
    Text(Vec<char>),
    /// Sequência que começa com `-` (pré-lançamento): fica **antes** da ausência de componente.
    Prerelease(Vec<char>),
    /// Sequência de dígitos ASCII.
    Numeric(Vec<char>),
}

impl Component {
    fn chars(&self) -> &[char] {
        match self {
            Self::Text(c) | Self::Prerelease(c) | Self::Numeric(c) => c,
        }
    }
}

/// Compara duas versões pelo `FlexVer`.
///
/// Nunca falha: qualquer texto é uma versão válida.
#[must_use]
pub fn compare(a: &str, b: &str) -> Ordering {
    let left = decompose(a);
    let right = decompose(b);
    let len = left.len().max(right.len());
    for i in 0..len {
        let ord = compare_component(left.get(i), right.get(i));
        if ord != Ordering::Equal {
            return ord;
        }
    }
    Ordering::Equal
}

fn decompose(text: &str) -> Vec<Component> {
    let mut chars = text.chars().peekable();
    let Some(&first) = chars.peek() else {
        return Vec::new();
    };
    let mut last_was_number = first.is_ascii_digit();
    let mut accum: Vec<char> = Vec::new();
    let mut out = Vec::new();
    for c in chars {
        if c == '+' {
            break;
        }
        let number = c.is_ascii_digit();
        let starts_prerelease = c == '-' && accum.first().is_some_and(|&f| f != '-');
        if number != last_was_number || starts_prerelease {
            out.push(make_component(last_was_number, std::mem::take(&mut accum)));
            last_was_number = number;
        }
        accum.push(c);
    }
    out.push(make_component(last_was_number, accum));
    out
}

fn make_component(number: bool, chars: Vec<char>) -> Component {
    if number {
        Component::Numeric(chars)
    } else if chars.len() > 1 && chars.first() == Some(&'-') {
        Component::Prerelease(chars)
    } else {
        Component::Text(chars)
    }
}

/// `None` é o componente ausente (a versão mais curta acabou).
fn compare_component(a: Option<&Component>, b: Option<&Component>) -> Ordering {
    match (a, b) {
        (None, None) => Ordering::Equal,
        (None, Some(other)) => compare_component(Some(other), None).reverse(),
        (Some(Component::Prerelease(_)), None) => Ordering::Less,
        (Some(_), None) => Ordering::Greater,
        (Some(Component::Numeric(x)), Some(Component::Numeric(y))) => {
            let x = strip_leading_zeroes(x);
            let y = strip_leading_zeroes(y);
            x.len().cmp(&y.len()).then_with(|| x.cmp(y))
        }
        (Some(x), Some(y)) => compare_codepoints(x.chars(), y.chars()),
    }
}

fn compare_codepoints(a: &[char], b: &[char]) -> Ordering {
    for (x, y) in a.iter().zip(b) {
        if x != y {
            return x.cmp(y);
        }
    }
    a.len().cmp(&b.len())
}

/// Tira os zeros à esquerda, mantendo pelo menos um dígito (como a referência).
fn strip_leading_zeroes(digits: &[char]) -> &[char] {
    if digits.len() <= 1 {
        return digits;
    }
    let stop = digits.len() - 1;
    let mut i = 0;
    while i < stop && digits.get(i) == Some(&'0') {
        i += 1;
    }
    digits.get(i..).unwrap_or(digits)
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    /// Vetores oficiais do `FlexVer` (`test/test_vectors.txt`, CC0-1.0), copiados sem alteração.
    const VECTORS: &str = include_str!("../../tests/data/flexver_test_vectors.txt");

    #[test]
    fn vetores_oficiais_do_flexver() {
        let mut checked = 0;
        for line in VECTORS.lines() {
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let parts: Vec<&str> = line.split(' ').collect();
            assert_eq!(parts.len(), 3, "linha mal formada: {line:?}");
            let expected = match parts[1] {
                "<" => Ordering::Less,
                "=" => Ordering::Equal,
                ">" => Ordering::Greater,
                other => panic!("operador desconhecido {other:?}"),
            };
            assert_eq!(compare(parts[0], parts[2]), expected, "{line:?}");
            assert_eq!(
                compare(parts[2], parts[0]),
                expected.reverse(),
                "inverso de {line:?}"
            );
            checked += 1;
        }
        assert_eq!(checked, 38, "todos os vetores do arquivo");
    }

    #[test]
    fn versoes_reais_de_mods() {
        assert_eq!(compare("1.20.1-47.2.0", "1.20.1-47.10.0"), Ordering::Less);
        assert_eq!(
            compare("mc1.20.1-0.5.13", "mc1.20.1-0.5.9"),
            Ordering::Greater
        );
        assert_eq!(compare("0.92.2+1.20.1", "0.92.2+1.20.4"), Ordering::Equal);
        assert_eq!(compare("1.7.10-1.0", "1.7.10-1.0"), Ordering::Equal);
    }

    /// Comportamento fiel à referência, registrado para ninguém "consertar" sem querer: com um
    /// caractere menor que `-` (como `!`), um texto vazio vindo de `+` no início ou um `-`
    /// seguido de dígito (`1-0`, que vira o texto `-` e o número `0`), a ordem deixa de ser
    /// transitiva. Quem ordena listas com `FlexVer` não deve supor ordem total.
    #[test]
    fn nao_transitiva_em_casos_extremos() {
        assert_eq!(compare("1-a", "1"), Ordering::Less);
        assert_eq!(compare("1", "1!"), Ordering::Less);
        assert_eq!(compare("1!", "1-a"), Ordering::Less);
        assert_eq!(compare("-a", ""), Ordering::Less);
        assert_eq!(compare("", "+"), Ordering::Less);
        assert_eq!(compare("-a", "+"), Ordering::Greater);
        // Também com versões plausíveis: pré-lançamento numérico contra textual.
        assert_eq!(compare("1-0", "1-a"), Ordering::Less);
        assert_eq!(compare("1-a", "1"), Ordering::Less);
        assert_eq!(compare("1", "1-0"), Ordering::Less);
    }

    proptest! {
        #[test]
        fn reflexiva_e_antissimetrica(a in ".{0,24}", b in ".{0,24}") {
            prop_assert_eq!(compare(&a, &a), Ordering::Equal);
            prop_assert_eq!(compare(&a, &b), compare(&b, &a).reverse());
        }

        /// Transitiva em versões sem `-` e sem `+`. Com eles o FlexVer de referência não é
        /// transitivo; ver `nao_transitiva_em_casos_extremos`.
        #[test]
        fn transitiva(a in "[0-9a-c._]{0,8}", b in "[0-9a-c._]{0,8}", c in "[0-9a-c._]{0,8}") {
            if compare(&a, &b) != Ordering::Greater && compare(&b, &c) != Ordering::Greater {
                prop_assert_ne!(compare(&a, &c), Ordering::Greater);
            }
        }
    }
}
