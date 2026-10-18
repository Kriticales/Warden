//! Leitura do formato `hsperfdata` (`PerfData` da `HotSpot`), sem depender de plataforma. Porte do
//! leitor do spike S-R5-2 (`spikes/s-r5-2/src/perfdata.rs`): parser puro, sem `unsafe`; um
//! buffer ruim vira [`ParseError`], nunca pânico.
//!
//! Formato (`OpenJDK` `src/hotspot/share/runtime/perfMemory.hpp`), o mesmo do Java 8 ao 25:
//!
//! ```text
//! PerfDataPrologue (32 bytes)
//!   0  u32 magic           bytes CA FE C0 C0 (sempre nessa ordem)
//!   4  u8  byte_order      0 = big endian, 1 = little endian (o resto do arquivo)
//!   5  u8  major_version   2 desde o Java 1.4.2
//!   6  u8  minor_version
//!   7  u8  accessible      1 quando a JVM terminou de montar o buffer
//!   8  i32 used            bytes usados
//!  12  i32 overflow
//!  16  i64 mod_time_stamp  muda quando entram contadores novos
//!  24  i32 entry_offset    primeiro PerfDataEntry
//!  28  i32 num_entries
//! PerfDataEntry (repetido num_entries vezes)
//!   0  i32 entry_length
//!   4  i32 name_offset     relativo ao início da entrada; nome terminado em NUL
//!   8  i32 vector_length   0 = escalar
//!  12  u8  data_type       'J' = long, 'B' = byte (vetor de bytes = texto)
//!  13  u8  flags
//!  14  u8  data_units
//!  15  u8  data_variability
//!  16  i32 data_offset     relativo ao início da entrada
//! ```
//!
//! **Privacidade** (S-R5-2 §9): o buffer traz o PATH, caminhos da instância e os argumentos do
//! jogo (`sun.rt.javaCommand`). Os textos lidos aqui ficam só na memória, para reconhecer o
//! coletor e a versão do Java; o que sai da crate são números ([`super::HeapReading`]).

use std::collections::BTreeMap;
use std::fmt;

/// Os 4 primeiros bytes de todo `hsperfdata`.
pub const MAGIC: [u8; 4] = [0xCA, 0xFE, 0xC0, 0xC0];
/// Tamanho do cabeçalho.
pub const PROLOGUE_LEN: usize = 32;
const ENTRY_HEADER_LEN: usize = 20;

/// O valor de um contador.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Value {
    /// Número (`J`).
    Long(i64),
    /// Texto (`B` com vetor), terminado no primeiro NUL.
    Text(String),
    /// Outros tipos, que o Warden não usa.
    Other,
}

/// Um contador e onde o valor dele fica no buffer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Counter {
    /// O valor.
    pub value: Value,
    /// Deslocamento do valor desde o início do buffer (para reler só os números).
    pub data_at: usize,
}

/// O cabeçalho do buffer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Header {
    /// Ordem dos bytes dos números (`true` = little endian, como no x64).
    pub little_endian: bool,
    /// A JVM terminou de montar o buffer.
    pub accessible: bool,
    /// Muda quando a JVM acrescenta contadores.
    pub mod_time_stamp: i64,
    /// Quantos contadores há.
    pub num_entries: i32,
    /// Onde começa o primeiro.
    pub entry_offset: i32,
}

/// Um instantâneo do buffer: o cabeçalho e os contadores pelo nome.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Snapshot {
    /// O cabeçalho.
    pub header: Header,
    /// Os contadores.
    pub counters: BTreeMap<String, Counter>,
}

/// Buffer que não é um `hsperfdata` válido.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    /// Menor que o cabeçalho.
    TooShort(usize),
    /// Os 4 primeiros bytes não são `CA FE C0 C0`.
    BadMagic([u8; 4]),
    /// Versão do formato que o Warden não conhece.
    BadVersion(u8, u8),
    /// Uma entrada sai do buffer ou tem campos impossíveis.
    BadEntry {
        /// Índice da entrada.
        index: usize,
        /// Deslocamento da entrada.
        offset: usize,
        /// O que estava errado.
        why: &'static str,
    },
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooShort(n) => write!(f, "buffer curto demais ({n} bytes)"),
            Self::BadMagic(m) => write!(f, "cabeçalho inválido: {m:02x?}"),
            Self::BadVersion(a, b) => write!(f, "versão não suportada: {a}.{b}"),
            Self::BadEntry { index, offset, why } => {
                write!(
                    f,
                    "entrada {index} inválida no deslocamento {offset}: {why}"
                )
            }
        }
    }
}

impl std::error::Error for ParseError {}

/// Leitura de números no buffer, na ordem de bytes do cabeçalho.
#[derive(Clone, Copy)]
pub(super) struct Reader<'a> {
    buf: &'a [u8],
    little: bool,
}

impl<'a> Reader<'a> {
    pub(super) fn new(buf: &'a [u8], little: bool) -> Self {
        Self { buf, little }
    }

    fn bytes<const N: usize>(&self, at: usize) -> Option<[u8; N]> {
        self.buf.get(at..at.checked_add(N)?)?.try_into().ok()
    }

    pub(super) fn i32(&self, at: usize) -> Option<i32> {
        let b = self.bytes::<4>(at)?;
        Some(if self.little {
            i32::from_le_bytes(b)
        } else {
            i32::from_be_bytes(b)
        })
    }

    pub(super) fn i64(&self, at: usize) -> Option<i64> {
        let b = self.bytes::<8>(at)?;
        Some(if self.little {
            i64::from_le_bytes(b)
        } else {
            i64::from_be_bytes(b)
        })
    }

    fn u8(&self, at: usize) -> Option<u8> {
        self.buf.get(at).copied()
    }
}

/// Lê só o cabeçalho (para saber se a lista de contadores mudou sem reler tudo).
pub fn parse_header(buf: &[u8]) -> Result<Header, ParseError> {
    if buf.len() < PROLOGUE_LEN {
        return Err(ParseError::TooShort(buf.len()));
    }
    let mut magic = [0u8; 4];
    magic.copy_from_slice(&buf[0..4]);
    if magic != MAGIC {
        return Err(ParseError::BadMagic(magic));
    }
    let (major, minor) = (buf[5], buf[6]);
    if major != 2 {
        return Err(ParseError::BadVersion(major, minor));
    }
    let little_endian = buf[4] == 1;
    let r = Reader::new(buf, little_endian);
    // O cabeçalho tem 32 bytes, já conferidos acima.
    Ok(Header {
        little_endian,
        accessible: buf[7] != 0,
        mod_time_stamp: r.i64(16).unwrap_or_default(),
        entry_offset: r.i32(24).unwrap_or_default(),
        num_entries: r.i32(28).unwrap_or_default(),
    })
}

/// Lê um instantâneo de um buffer `hsperfdata` (cópia do mapeamento ou do arquivo).
pub fn parse(buf: &[u8]) -> Result<Snapshot, ParseError> {
    let header = parse_header(buf)?;
    let r = Reader::new(buf, header.little_endian);
    let mut counters = BTreeMap::new();
    let mut off = usize::try_from(header.entry_offset).unwrap_or(0);
    for index in 0..usize::try_from(header.num_entries).unwrap_or(0) {
        let bad = |why| ParseError::BadEntry {
            index,
            offset: off,
            why,
        };
        let entry_len = r.i32(off).ok_or_else(|| bad("fora do buffer"))?;
        let entry_len = usize::try_from(entry_len)
            .ok()
            .filter(|&len| len >= ENTRY_HEADER_LEN)
            .ok_or_else(|| bad("tamanho inválido"))?;
        let end = off
            .checked_add(entry_len)
            .filter(|&end| end <= buf.len())
            .ok_or_else(|| bad("passa do fim"))?;
        let field = |delta: usize, why| {
            r.i32(off + delta)
                .and_then(|value| usize::try_from(value).ok())
                .ok_or_else(|| bad(why))
        };
        let name_off = field(4, "nome")?;
        let vector_len = field(8, "vetor")?;
        let data_type = r.u8(off + 12).ok_or_else(|| bad("tipo"))?;
        let data_off = field(16, "dados")?;

        let name_bytes = off
            .checked_add(name_off)
            .and_then(|start| buf.get(start..end))
            .ok_or_else(|| bad("nome fora da entrada"))?;
        let name_len = name_bytes
            .iter()
            .position(|&b| b == 0)
            .ok_or_else(|| bad("nome sem NUL"))?;
        let name = String::from_utf8_lossy(&name_bytes[..name_len]).into_owned();

        let data_at = off.checked_add(data_off).ok_or_else(|| bad("dados"))?;
        let value = match (data_type, vector_len) {
            (b'J', 0) => {
                if data_at.checked_add(8).is_none_or(|stop| stop > end) {
                    return Err(bad("número fora da entrada"));
                }
                Value::Long(
                    r.i64(data_at)
                        .ok_or_else(|| bad("número fora da entrada"))?,
                )
            }
            (b'B', n) if n > 0 => {
                let raw = data_at
                    .checked_add(n)
                    .filter(|&stop| stop <= end)
                    .and_then(|stop| buf.get(data_at..stop))
                    .ok_or_else(|| bad("texto fora da entrada"))?;
                let len = raw.iter().position(|&b| b == 0).unwrap_or(raw.len());
                Value::Text(decode_text(&raw[..len]))
            }
            _ => Value::Other,
        };
        counters.insert(name, Counter { value, data_at });
        off = end;
    }
    Ok(Snapshot { header, counters })
}

/// UTF-8 quando válido; senão Windows-1252 (a JVM grava texto na code page ANSI do Windows).
fn decode_text(raw: &[u8]) -> String {
    match std::str::from_utf8(raw) {
        Ok(text) => text.to_owned(),
        Err(_) => raw.iter().map(|&b| cp1252(b)).collect(),
    }
}

fn cp1252(b: u8) -> char {
    const HIGH: [char; 32] = [
        '€', '\u{81}', '‚', 'ƒ', '„', '…', '†', '‡', 'ˆ', '‰', 'Š', '‹', 'Œ', '\u{8d}', 'Ž',
        '\u{8f}', '\u{90}', '‘', '’', '“', '”', '•', '–', '—', '˜', '™', 'š', '›', 'œ', '\u{9d}',
        'ž', 'Ÿ',
    ];
    match b {
        0x80..=0x9f => HIGH[usize::from(b - 0x80)],
        _ => char::from(b),
    }
}

impl Snapshot {
    /// Um contador numérico.
    #[must_use]
    pub fn long(&self, name: &str) -> Option<i64> {
        match self.counters.get(name)?.value {
            Value::Long(value) => Some(value),
            _ => None,
        }
    }

    /// Um contador de texto.
    #[must_use]
    pub fn text(&self, name: &str) -> Option<&str> {
        match &self.counters.get(name)?.value {
            Value::Text(text) => Some(text),
            _ => None,
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
/// Um buffer mínimo com os contadores dados (`J`) e um texto, little endian.
pub(crate) fn test_buffer(longs: &[(&str, i64)], texts: &[(&str, &str)]) -> Vec<u8> {
    let mut entries = Vec::new();
    let mut push = |name: &str, data_type: u8, vector: usize, data: &[u8]| {
        let name_off = ENTRY_HEADER_LEN;
        let mut data_off = name_off + name.len() + 1;
        data_off = data_off.div_ceil(8) * 8;
        let len = (data_off + data.len()).div_ceil(8) * 8;
        let mut entry = vec![0u8; len];
        entry[0..4].copy_from_slice(&i32::try_from(len).expect("entrada pequena").to_le_bytes());
        entry[4..8].copy_from_slice(
            &i32::try_from(name_off)
                .expect("entrada pequena")
                .to_le_bytes(),
        );
        entry[8..12].copy_from_slice(
            &i32::try_from(vector)
                .expect("entrada pequena")
                .to_le_bytes(),
        );
        entry[12] = data_type;
        entry[16..20].copy_from_slice(
            &i32::try_from(data_off)
                .expect("entrada pequena")
                .to_le_bytes(),
        );
        entry[name_off..name_off + name.len()].copy_from_slice(name.as_bytes());
        entry[data_off..data_off + data.len()].copy_from_slice(data);
        entries.push(entry);
    };
    for (name, value) in longs {
        push(name, b'J', 0, &value.to_le_bytes());
    }
    for (name, text) in texts {
        let mut data = text.as_bytes().to_vec();
        data.push(0);
        push(name, b'B', data.len(), &data);
    }
    let mut buf = vec![0u8; PROLOGUE_LEN];
    buf[0..4].copy_from_slice(&MAGIC);
    buf[4] = 1;
    buf[5] = 2;
    buf[7] = 1;
    buf[16..24].copy_from_slice(&7i64.to_le_bytes());
    buf[24..28].copy_from_slice(&i32::try_from(PROLOGUE_LEN).expect("32").to_le_bytes());
    buf[28..32].copy_from_slice(
        &i32::try_from(entries.len())
            .expect("poucas entradas")
            .to_le_bytes(),
    );
    for entry in entries {
        buf.extend(entry);
    }
    let used = i32::try_from(buf.len()).expect("buffer pequeno");
    buf[8..12].copy_from_slice(&used.to_le_bytes());
    buf
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    #[test]
    fn le_numeros_e_textos() {
        let buf = test_buffer(&[("a.b", 42), ("c", -1)], &[("nome", "GarbageFirst")]);
        let snap = parse(&buf).unwrap();
        assert!(snap.header.accessible);
        assert_eq!(snap.header.num_entries, 3);
        assert_eq!(snap.long("a.b"), Some(42));
        assert_eq!(snap.long("c"), Some(-1));
        assert_eq!(snap.text("nome"), Some("GarbageFirst"));
        assert_eq!(snap.long("nome"), None);
        assert_eq!(snap.text("a.b"), None);
    }

    #[test]
    fn cabecalho_errado_vira_erro() {
        assert_eq!(parse(&[0u8; 10]), Err(ParseError::TooShort(10)));
        let mut buf = test_buffer(&[("a", 1)], &[]);
        buf[0] = 0;
        assert!(matches!(parse(&buf), Err(ParseError::BadMagic(_))));
        let mut buf = test_buffer(&[("a", 1)], &[]);
        buf[5] = 1;
        assert_eq!(parse(&buf), Err(ParseError::BadVersion(1, 0)));
    }

    #[test]
    fn buffer_cortado_vira_erro_em_qualquer_ponto() {
        let buf = test_buffer(
            &[("sun.gc.generation.0.space.0.used", 1 << 20)],
            &[("x", "y")],
        );
        for len in PROLOGUE_LEN..buf.len() {
            assert!(parse(&buf[..len]).is_err(), "cortado em {len}");
        }
        assert!(parse(&buf).is_ok());
    }

    #[test]
    fn entrada_com_deslocamento_absurdo_vira_erro() {
        let mut buf = test_buffer(&[("a", 1)], &[]);
        // O deslocamento dos dados aponta para fora da entrada.
        buf[PROLOGUE_LEN + 16..PROLOGUE_LEN + 20].copy_from_slice(&i32::MAX.to_le_bytes());
        assert!(matches!(parse(&buf), Err(ParseError::BadEntry { .. })));
        let mut buf = test_buffer(&[("a", 1)], &[]);
        // Tamanho da entrada negativo.
        buf[PROLOGUE_LEN..PROLOGUE_LEN + 4].copy_from_slice(&(-8i32).to_le_bytes());
        assert!(matches!(parse(&buf), Err(ParseError::BadEntry { .. })));
        let mut buf = test_buffer(&[("a", 1)], &[]);
        // Mais entradas do que o buffer tem.
        buf[28..32].copy_from_slice(&50i32.to_le_bytes());
        assert!(matches!(parse(&buf), Err(ParseError::BadEntry { .. })));
    }

    #[test]
    fn texto_fora_do_utf8_sai_como_cp1252() {
        assert_eq!(decode_text(b"Jo\xe3o"), "João");
        assert_eq!(decode_text(b"\x80 \x9f"), "€ Ÿ");
        assert_eq!(decode_text("João".as_bytes()), "João");
    }

    proptest::proptest! {
        /// Qualquer sequência de bytes depois de um cabeçalho válido: erro ou leitura, nunca
        /// pânico.
        #[test]
        fn bytes_quaisquer_nao_dao_panico(tail in proptest::collection::vec(proptest::num::u8::ANY, 0..512), entries in 0i32..40) {
            let mut buf = vec![0u8; PROLOGUE_LEN];
            buf[0..4].copy_from_slice(&MAGIC);
            buf[4] = 1;
            buf[5] = 2;
            buf[24..28].copy_from_slice(&32i32.to_le_bytes());
            buf[28..32].copy_from_slice(&entries.to_le_bytes());
            buf.extend(tail);
            let _ = parse(&buf);
        }
    }
}
