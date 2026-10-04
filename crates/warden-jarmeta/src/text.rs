//! Decodificação dos arquivos de texto dos jars.
//!
//! Os descritores deveriam ser UTF-8, mas `mcmod.info` antigos às vezes foram salvos em
//! Windows-1252 (acentos nos nomes de autores). Texto que não é UTF-8 válido é lido como
//! Windows-1252, que cobre todos os bytes, e quem chama registra o aviso `NOT_UTF8`.

/// Caracteres do Windows-1252 de 0x80 a 0x9F (os demais bytes são iguais ao Latin-1).
const CP1252_HIGH: [char; 32] = [
    '\u{20AC}', '\u{0081}', '\u{201A}', '\u{0192}', '\u{201E}', '\u{2026}', '\u{2020}', '\u{2021}',
    '\u{02C6}', '\u{2030}', '\u{0160}', '\u{2039}', '\u{0152}', '\u{008D}', '\u{017D}', '\u{008F}',
    '\u{0090}', '\u{2018}', '\u{2019}', '\u{201C}', '\u{201D}', '\u{2022}', '\u{2013}', '\u{2014}',
    '\u{02DC}', '\u{2122}', '\u{0161}', '\u{203A}', '\u{0153}', '\u{009D}', '\u{017E}', '\u{0178}',
];

/// Texto decodificado e se foi preciso cair para Windows-1252.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Decoded {
    pub(crate) text: String,
    pub(crate) was_utf8: bool,
}

/// Decodifica bytes de um descritor, sem a marca de ordem de bytes (BOM) do UTF-8.
pub(crate) fn decode(bytes: &[u8]) -> Decoded {
    let bytes = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
    match std::str::from_utf8(bytes) {
        Ok(text) => Decoded {
            text: text.to_owned(),
            was_utf8: true,
        },
        Err(_) => Decoded {
            text: bytes.iter().map(|&b| cp1252_char(b)).collect(),
            was_utf8: false,
        },
    }
}

fn cp1252_char(byte: u8) -> char {
    match byte {
        0x80..=0x9F => CP1252_HIGH
            .get(usize::from(byte - 0x80))
            .copied()
            .unwrap_or(char::REPLACEMENT_CHARACTER),
        _ => char::from(byte),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utf8_com_e_sem_bom() {
        assert_eq!(decode("ação".as_bytes()).text, "ação");
        let with_bom = [b"\xEF\xBB\xBF".as_slice(), b"{}"].concat();
        let d = decode(&with_bom);
        assert_eq!(d.text, "{}");
        assert!(d.was_utf8);
    }

    #[test]
    fn windows_1252_quando_nao_e_utf8() {
        // "José “Zé” – €" em Windows-1252.
        let d = decode(b"Jos\xE9 \x93Z\xE9\x94 \x96 \x80");
        assert!(!d.was_utf8);
        assert_eq!(d.text, "José \u{201C}Zé\u{201D} \u{2013} €");
        assert_eq!(decode(b"\x81\x8D\x8F\x90\x9D").text.chars().count(), 5);
    }
}
