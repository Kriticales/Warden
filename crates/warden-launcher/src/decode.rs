//! Decodificação da saída do jogo (ARCHITECTURE §7.4).
//!
//! Cada linha é decodificada sozinha: UTF-8 quando é UTF-8 válido, senão Windows-1252. O Java
//! 8 no Windows escreve na página de código do sistema (1252 num Windows em português) e o
//! log4j às vezes em UTF-8, às vezes não; mods escrevem direto em `System.out`. Decidir por
//! linha evita o `Ã©` quando as duas codificações se misturam na mesma saída (CA-T13-03).

use encoding_rs::WINDOWS_1252;

/// Decodifica uma linha (sem o fim de linha).
#[must_use]
pub fn decode_line(bytes: &[u8]) -> String {
    if let Ok(text) = std::str::from_utf8(bytes) {
        text.to_owned()
    } else {
        let (text, _, _) = WINDOWS_1252.decode(bytes);
        text.into_owned()
    }
}

/// Tira o `\n` e o `\r` do fim.
#[must_use]
pub fn trim_line_end(bytes: &[u8]) -> &[u8] {
    let mut end = bytes.len();
    while end > 0 && matches!(bytes[end - 1], b'\n' | b'\r') {
        end -= 1;
    }
    &bytes[..end]
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    #[test]
    fn utf8_e_windows_1252() {
        assert_eq!(decode_line("ação é útil".as_bytes()), "ação é útil");
        // "ação é útil" em Windows-1252.
        let cp1252 = [
            0x61, 0xE7, 0xE3, 0x6F, 0x20, 0xE9, 0x20, 0xFA, 0x74, 0x69, 0x6C,
        ];
        assert_eq!(decode_line(&cp1252), "ação é útil");
        // Aspas e travessão do 1252 (0x93, 0x94, 0x96).
        assert_eq!(decode_line(&[0x93, 0x6F, 0x69, 0x94, 0x20, 0x96]), "“oi” –");
    }

    #[test]
    fn fim_de_linha() {
        assert_eq!(trim_line_end(b"abc\r\n"), b"abc");
        assert_eq!(trim_line_end(b"abc\n"), b"abc");
        assert_eq!(trim_line_end(b"\r\n"), b"");
        assert_eq!(trim_line_end(b"abc"), b"abc");
    }

    proptest! {
        #[test]
        fn texto_utf8_volta_igual(text in "\\PC{0,80}") {
            prop_assert_eq!(decode_line(text.as_bytes()), text);
        }

        #[test]
        fn bytes_quaisquer_nunca_falham(bytes in proptest::collection::vec(any::<u8>(), 0..200)) {
            let text = decode_line(&bytes);
            let replaced = text.contains(char::REPLACEMENT_CHARACTER);
            prop_assert!(!replaced || std::str::from_utf8(&bytes).is_ok());
        }
    }
}
