//! Ordem das versões de loader.
//!
//! As versões do **Minecraft** nunca são comparadas por texto: a ordem é a do manifesto da
//! Mojang ([`crate::MinecraftVersions::compare`]). Já as versões do Forge e do NeoForge são
//! números separados por ponto (`14.23.5.2864`, `21.1.252`, `26.3.0.48-beta`), e a ordem
//! certa é a numérica, componente a componente (`14.23.5.2864` > `14.23.5.860`).

use std::cmp::Ordering;

/// Os números separados por ponto do começo de `version` (até o primeiro `-` ou `+`).
/// Componente que não é número vale zero.
#[must_use]
pub(crate) fn numeric_parts(version: &str) -> Vec<u64> {
    let end = version.find(['-', '+']).unwrap_or(version.len());
    version
        .get(..end)
        .unwrap_or(version)
        .split('.')
        .map(|part| part.parse().unwrap_or(0))
        .collect()
}

/// Compara duas versões numéricas; no empate dos números, a estável (sem sufixo) vence e o
/// texto desempata, para a ordem ser total.
#[must_use]
pub(crate) fn compare_numeric(a: &str, b: &str) -> Ordering {
    numeric_parts(a)
        .cmp(&numeric_parts(b))
        .then_with(|| has_suffix(b).cmp(&has_suffix(a)))
        .then_with(|| a.cmp(b))
}

fn has_suffix(version: &str) -> bool {
    version.contains(['-', '+'])
}

/// Se `text` é só números separados por ponto, com pelo menos `min_parts` partes.
#[must_use]
pub(crate) fn is_dotted_number(text: &str, min_parts: usize) -> bool {
    let parts: Vec<&str> = text.split('.').collect();
    parts.len() >= min_parts
        && parts
            .iter()
            .all(|part| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit()))
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    #[test]
    fn ordem_numerica_e_nao_de_texto() {
        assert_eq!(
            compare_numeric("14.23.5.2864", "14.23.5.860"),
            Ordering::Greater
        );
        assert_eq!(compare_numeric("21.11.4", "21.1.252"), Ordering::Greater);
        assert_eq!(compare_numeric("20.4.1", "20.4.1-beta"), Ordering::Greater);
        assert_eq!(compare_numeric("1.0", "1.0"), Ordering::Equal);
        assert_eq!(numeric_parts("26.1.0.0-alpha.1+snapshot-1"), [26, 1, 0, 0]);
        assert_eq!(numeric_parts("a.2"), [0, 2]);
    }

    #[test]
    fn numeros_com_ponto() {
        assert!(is_dotted_number("10.13.4.1614", 2));
        assert!(is_dotted_number("1.8", 2));
        assert!(!is_dotted_number("1", 2));
        assert!(!is_dotted_number("1..2", 2));
        assert!(!is_dotted_number("1.x", 2));
        assert!(!is_dotted_number("", 1));
    }

    proptest! {
        /// A ordem é total e antissimétrica (pode ordenar listas com ela).
        #[test]
        fn ordem_total(a in "[0-9]{1,3}(\\.[0-9]{1,4}){0,3}(-beta)?",
                       b in "[0-9]{1,3}(\\.[0-9]{1,4}){0,3}(-beta)?") {
            prop_assert_eq!(compare_numeric(&a, &b), compare_numeric(&b, &a).reverse());
            prop_assert_eq!(compare_numeric(&a, &b) == Ordering::Equal, a == b);
        }

        #[test]
        fn nunca_entra_em_panico(text in "\\PC*") {
            let _ = numeric_parts(&text);
            let _ = is_dotted_number(&text, 1);
            let _ = compare_numeric(&text, "1.0");
        }
    }
}
