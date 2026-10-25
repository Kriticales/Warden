//! Os números no jeito do português do Brasil: milhar com ponto, decimal com vírgula, tempos
//! em "1 min 12 s". Funções puras, sem I/O.

/// `6120` vira `6.120`.
pub(super) fn thousands(value: u32) -> String {
    let digits = value.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            out.push('.');
        }
        out.push(digit);
    }
    out
}

/// `6120` MB como `6.120 MB`.
pub(super) fn megabytes(value: u32) -> String {
    format!("{} MB", thousands(value))
}

/// Um número com `places` casas e vírgula (`26,5`).
pub(super) fn decimal(value: f64, places: usize) -> String {
    format!("{value:.places$}").replace('.', ",")
}

/// A duração em milissegundos como `1 min 12 s` (segundos arredondados), `45 s` ou
/// `menos de 1 s`.
pub(super) fn duration(ms: u32) -> String {
    let seconds = (u64::from(ms) + 500) / 1000;
    let (minutes, rest) = (seconds / 60, seconds % 60);
    if seconds == 0 {
        "menos de 1 s".to_owned()
    } else if minutes == 0 {
        format!("{rest} s")
    } else {
        format!("{minutes} min {rest} s")
    }
}

/// Segundos com uma casa (`3,2 s`), para o tempo de cada mod.
pub(super) fn seconds_one_place(ms: u32) -> String {
    format!("{} s", decimal(f64::from(ms) / 1000.0, 1))
}

/// A impressão do computador (`sha256:7f3a9c…`) reduzida a um código curto (`PC-7f3a9c`). Nunca
/// sai nada do computador além do prefixo do hash.
pub(super) fn machine_code(fingerprint: &str) -> String {
    let hex = fingerprint.rsplit(':').next().unwrap_or(fingerprint);
    let short: String = hex
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .take(6)
        .collect();
    format!("PC-{short}")
}

/// A assinatura do perfil (`sha256:…` ou só o hash) reduzida a 6 caracteres.
pub(super) fn signature_code(signature: &str) -> String {
    let hex = signature.rsplit(':').next().unwrap_or(signature);
    hex.chars()
        .filter(char::is_ascii_alphanumeric)
        .take(6)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numeros_no_jeito_do_brasil() {
        assert_eq!(thousands(0), "0");
        assert_eq!(thousands(999), "999");
        assert_eq!(thousands(6120), "6.120");
        assert_eq!(thousands(1_234_567), "1.234.567");
        assert_eq!(megabytes(4210), "4.210 MB");
        assert_eq!(decimal(26.54, 1), "26,5");
        assert_eq!(duration(72_000), "1 min 12 s");
        assert_eq!(duration(184_000), "3 min 4 s");
        assert_eq!(duration(45_400), "45 s");
        assert_eq!(duration(59_600), "1 min 0 s");
        assert_eq!(duration(300), "menos de 1 s");
        assert_eq!(seconds_one_place(3_240), "3,2 s");
    }

    #[test]
    fn computador_vira_so_um_codigo_curto() {
        assert_eq!(
            machine_code("sha256:7f3a9c1d2e4b5a69788796a5b4c3d2e1f0"),
            "PC-7f3a9c"
        );
        assert_eq!(machine_code("ab"), "PC-ab");
        assert_eq!(signature_code("ab12cd34ef56"), "ab12cd");
    }
}
