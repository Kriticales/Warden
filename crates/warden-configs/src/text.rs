//! Texto de um arquivo de config: BOM, linhas, fim de linha e conversão de posições.
//!
//! Os parsers trabalham sobre o corpo (o texto sem o BOM); as posições públicas
//! ([`TextSpan`](crate::TextSpan)) contam os bytes do arquivo, BOM incluído.

use crate::error::{ConfigError, Result};
use crate::format::ConfigFormat;

/// BOM do UTF-8.
pub(crate) const BOM: &str = "\u{feff}";

/// Uma linha física: `start..end` sem o fim de linha; `next` é o início da linha seguinte.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Line {
    /// Primeiro byte da linha.
    pub start: usize,
    /// Fim do conteúdo (antes de `\r\n` ou `\n`).
    pub end: usize,
    /// Início da próxima linha (depois do fim de linha), ou o tamanho do corpo.
    pub next: usize,
}

/// Texto decodificado de um arquivo.
#[derive(Debug)]
pub(crate) struct DocText<'a> {
    /// Corpo do arquivo, sem o BOM.
    pub body: &'a str,
    /// Tamanho do BOM em bytes (0 ou 3).
    pub bom_len: usize,
    /// Linhas físicas do corpo (separadas por `\n`; o `\r` antes dele não entra no conteúdo).
    pub lines: Vec<Line>,
    /// Fim de linha predominante.
    pub line_ending: &'static str,
}

impl<'a> DocText<'a> {
    /// Decodifica os bytes (UTF-8, com ou sem BOM), respeitando o limite de tamanho.
    pub(crate) fn new(bytes: &'a [u8]) -> Result<Self> {
        if bytes.len() > crate::MAX_STRUCTURED_BYTES {
            return Err(ConfigError::TooLarge {
                size: bytes.len(),
                limit: crate::MAX_STRUCTURED_BYTES,
            });
        }
        let text = std::str::from_utf8(bytes).map_err(|error| ConfigError::NotUtf8 {
            offset: error.valid_up_to(),
        })?;
        Ok(Self::from_str(text))
    }

    /// Texto já decodificado.
    pub(crate) fn from_str(text: &'a str) -> Self {
        let (body, bom_len) = match text.strip_prefix(BOM) {
            Some(rest) => (rest, BOM.len()),
            None => (text, 0),
        };
        let mut lines = Vec::new();
        let mut start = 0;
        let mut crlf = 0usize;
        let mut lf = 0usize;
        for (position, byte) in body.bytes().enumerate() {
            if byte == b'\n' {
                let end = if position > start && body.as_bytes().get(position - 1) == Some(&b'\r') {
                    crlf += 1;
                    position - 1
                } else {
                    lf += 1;
                    position
                };
                lines.push(Line {
                    start,
                    end,
                    next: position + 1,
                });
                start = position + 1;
            }
        }
        if start < body.len() || lines.is_empty() {
            lines.push(Line {
                start,
                end: body.len(),
                next: body.len(),
            });
        }
        let line_ending = if crlf > lf { "\r\n" } else { "\n" };
        Self {
            body,
            bom_len,
            lines,
            line_ending,
        }
    }

    /// Conteúdo da linha `index` (começa em 0), sem o fim de linha.
    pub(crate) fn line_text(&self, index: usize) -> &'a str {
        self.lines
            .get(index)
            .and_then(|line| self.body.get(line.start..line.end))
            .unwrap_or("")
    }

    /// Índice (começa em 0) da linha que contém o byte `offset` do corpo.
    pub(crate) fn line_index_of(&self, offset: usize) -> usize {
        match self.lines.binary_search_by(|line| line.start.cmp(&offset)) {
            Ok(index) => index,
            Err(insert) => insert.saturating_sub(1),
        }
    }

    /// Linha (começa em 1) e coluna em caracteres (começa em 1) do byte `offset` do corpo.
    pub(crate) fn line_column(&self, offset: usize) -> (usize, usize) {
        let index = self.line_index_of(offset);
        let start = self.lines.get(index).map_or(0, |line| line.start);
        let column = self
            .body
            .get(start..offset.min(self.body.len()))
            .map_or(1, |prefix| prefix.chars().count() + 1);
        (index + 1, column)
    }

    /// Erro de leitura na posição `offset` do corpo.
    pub(crate) fn parse_error(
        &self,
        format: ConfigFormat,
        offset: usize,
        message: impl Into<String>,
    ) -> ConfigError {
        let (line, column) = self.line_column(offset);
        ConfigError::Parse {
            format,
            line,
            column,
            message: message.into(),
        }
    }

    /// Comentário logo acima da linha `index`: as linhas contíguas que `strip` reconhece como
    /// comentário (uma linha vazia ou qualquer outra linha interrompe). Devolve o texto sem os
    /// marcadores, na ordem do arquivo.
    pub(crate) fn leading_comment(
        &self,
        index: usize,
        strip: impl Fn(&str) -> Option<&str>,
    ) -> Option<String> {
        let mut collected = Vec::new();
        let mut current = index;
        while current > 0 {
            current -= 1;
            let text = self.line_text(current).trim();
            match strip(text) {
                Some(comment) if !text.is_empty() => collected.push(comment.trim_end()),
                _ => break,
            }
        }
        if collected.is_empty() {
            return None;
        }
        collected.reverse();
        Some(collected.join("\n"))
    }
}

/// Tira o marcador de um comentário de linha começado por `marker` e um espaço opcional.
pub(crate) fn strip_line_comment<'t>(text: &'t str, markers: &[char]) -> Option<&'t str> {
    let rest = text.strip_prefix(|c: char| markers.contains(&c))?;
    Some(rest.strip_prefix(' ').unwrap_or(rest))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linhas_com_crlf_lf_e_sem_quebra_final() {
        let doc = DocText::from_str("a\r\nbb\nccc");
        assert_eq!(doc.lines.len(), 3);
        assert_eq!(doc.line_text(0), "a");
        assert_eq!(doc.line_text(1), "bb");
        assert_eq!(doc.line_text(2), "ccc");
        assert_eq!(doc.line_text(9), "");
        assert_eq!(
            doc.lines[0],
            Line {
                start: 0,
                end: 1,
                next: 3
            }
        );
        assert_eq!(doc.line_ending, "\n");
        let crlf = DocText::from_str("a\r\nb\r\n");
        assert_eq!(crlf.line_ending, "\r\n");
        assert_eq!(crlf.lines.len(), 2);
        let empty = DocText::from_str("");
        assert_eq!(empty.lines.len(), 1);
    }

    #[test]
    fn bom_e_posicoes() {
        let doc = DocText::from_str("\u{feff}x\nyé z");
        assert_eq!(doc.bom_len, 3);
        assert_eq!(doc.body, "x\nyé z");
        assert_eq!(doc.line_column(0), (1, 1));
        assert_eq!(doc.line_column(2), (2, 1));
        // "yé" ocupa 3 bytes; o espaço é o terceiro caractere da linha.
        assert_eq!(doc.line_column(5), (2, 3));
        assert_eq!(doc.line_index_of(100), 1);
    }

    #[test]
    fn limites_de_tamanho_e_utf8() {
        let big = vec![b'a'; crate::MAX_STRUCTURED_BYTES + 1];
        assert!(matches!(
            DocText::new(&big),
            Err(ConfigError::TooLarge { .. })
        ));
        assert_eq!(
            DocText::new(b"ab\xff").unwrap_err(),
            ConfigError::NotUtf8 { offset: 2 }
        );
        assert!(DocText::new(b"ok").is_ok());
    }

    #[test]
    fn comentario_acima() {
        fn strip(t: &str) -> Option<&str> {
            strip_line_comment(t, &['#'])
        }
        let doc = DocText::from_str("#a\n\n# b\n  #c\nkey=1\n");
        assert_eq!(doc.leading_comment(4, strip).as_deref(), Some("b\nc"));
        assert_eq!(doc.leading_comment(0, strip), None);
        assert_eq!(doc.leading_comment(2, strip), None);
    }
}
