//! Leitura do formato `hsperfdata` (PerfData da HotSpot), sem depender de plataforma.
//!
//! Formato (OpenJDK `src/hotspot/share/runtime/perfMemory.hpp`):
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
//!  14  u8  data_units      1 none, 2 bytes, 3 ticks, 4 events, 5 string, 6 hertz
//!  15  u8  data_variability 1 constant, 2 monotonic, 3 variable
//!  16  i32 data_offset     relativo ao início da entrada
//! ```

use std::collections::BTreeMap;
use std::fmt;

pub const MAGIC: [u8; 4] = [0xCA, 0xFE, 0xC0, 0xC0];
const PROLOGUE_LEN: usize = 32;
const ENTRY_HEADER_LEN: usize = 20;

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Long(i64),
    Text(String),
    /// Outros tipos (não usados pelos contadores que interessam ao Warden).
    Other {
        data_type: u8,
        len: usize,
    },
}

#[derive(Debug, Clone)]
pub struct Counter {
    pub value: Value,
    pub units: u8,
    pub variability: u8,
}

#[derive(Debug, Clone)]
pub struct Snapshot {
    pub major: u8,
    pub minor: u8,
    pub accessible: bool,
    pub used: i32,
    pub mod_time_stamp: i64,
    pub counters: BTreeMap<String, Counter>,
}

#[derive(Debug)]
pub enum ParseError {
    TooShort(usize),
    BadMagic([u8; 4]),
    BadVersion(u8, u8),
    BadEntry {
        index: usize,
        offset: usize,
        why: &'static str,
    },
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParseError::TooShort(n) => write!(f, "buffer curto demais ({n} bytes)"),
            ParseError::BadMagic(m) => write!(f, "cabeçalho inválido: {m:02x?}"),
            ParseError::BadVersion(a, b) => write!(f, "versão não suportada: {a}.{b}"),
            ParseError::BadEntry { index, offset, why } => {
                write!(
                    f,
                    "entrada {index} inválida no deslocamento {offset}: {why}"
                )
            }
        }
    }
}

impl std::error::Error for ParseError {}

struct Reader<'a> {
    buf: &'a [u8],
    little: bool,
}

impl Reader<'_> {
    fn bytes<const N: usize>(&self, at: usize) -> Option<[u8; N]> {
        self.buf.get(at..at.checked_add(N)?)?.try_into().ok()
    }
    fn i32(&self, at: usize) -> Option<i32> {
        let b = self.bytes::<4>(at)?;
        Some(if self.little {
            i32::from_le_bytes(b)
        } else {
            i32::from_be_bytes(b)
        })
    }
    fn i64(&self, at: usize) -> Option<i64> {
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

/// Lê um instantâneo de um buffer `hsperfdata` (cópia do mapeamento ou do arquivo).
pub fn parse(buf: &[u8]) -> Result<Snapshot, ParseError> {
    if buf.len() < PROLOGUE_LEN {
        return Err(ParseError::TooShort(buf.len()));
    }
    let magic: [u8; 4] = buf[0..4].try_into().expect("4 bytes");
    if magic != MAGIC {
        return Err(ParseError::BadMagic(magic));
    }
    let r = Reader {
        buf,
        little: buf[4] == 1,
    };
    let (major, minor) = (buf[5], buf[6]);
    if major != 2 {
        return Err(ParseError::BadVersion(major, minor));
    }
    let accessible = buf[7] != 0;
    let used = r.i32(8).unwrap_or(0);
    let mod_time_stamp = r.i64(16).unwrap_or(0);
    let entry_offset = r.i32(24).unwrap_or(0);
    let num_entries = r.i32(28).unwrap_or(0);

    let mut counters = BTreeMap::new();
    let mut off = usize::try_from(entry_offset).unwrap_or(0);
    for index in 0..usize::try_from(num_entries).unwrap_or(0) {
        let bad = |why| ParseError::BadEntry {
            index,
            offset: off,
            why,
        };
        let entry_len = r.i32(off).ok_or_else(|| bad("fora do buffer"))?;
        let entry_len = usize::try_from(entry_len)
            .ok()
            .filter(|&l| l >= ENTRY_HEADER_LEN)
            .ok_or_else(|| bad("tamanho inválido"))?;
        let end = off
            .checked_add(entry_len)
            .filter(|&e| e <= buf.len())
            .ok_or_else(|| bad("passa do fim"))?;
        let name_off = r
            .i32(off + 4)
            .and_then(|v| usize::try_from(v).ok())
            .ok_or_else(|| bad("nome"))?;
        let vector_len = r
            .i32(off + 8)
            .and_then(|v| usize::try_from(v).ok())
            .ok_or_else(|| bad("vetor"))?;
        let data_type = r.u8(off + 12).ok_or_else(|| bad("tipo"))?;
        let units = r.u8(off + 14).ok_or_else(|| bad("unidade"))?;
        let variability = r.u8(off + 15).ok_or_else(|| bad("variabilidade"))?;
        let data_off = r
            .i32(off + 16)
            .and_then(|v| usize::try_from(v).ok())
            .ok_or_else(|| bad("dados"))?;

        let name_bytes = buf
            .get(off + name_off..end)
            .ok_or_else(|| bad("nome fora da entrada"))?;
        let name_len = name_bytes
            .iter()
            .position(|&b| b == 0)
            .ok_or_else(|| bad("nome sem NUL"))?;
        let name = String::from_utf8_lossy(&name_bytes[..name_len]).into_owned();

        let data_at = off + data_off;
        let value = match (data_type, vector_len) {
            (b'J', 0) => Value::Long(r.i64(data_at).ok_or_else(|| bad("long fora da entrada"))?),
            (b'B', n) if n > 0 => {
                let raw = buf
                    .get(data_at..data_at + n)
                    .filter(|_| data_at + n <= end)
                    .ok_or_else(|| bad("texto fora da entrada"))?;
                let len = raw.iter().position(|&b| b == 0).unwrap_or(raw.len());
                // A JVM grava texto na code page ANSI do Windows (ou UTF-8 no Linux).
                Value::Text(decode_text(&raw[..len]))
            }
            (t, n) => Value::Other {
                data_type: t,
                len: n,
            },
        };
        counters.insert(
            name,
            Counter {
                value,
                units,
                variability,
            },
        );
        off = end;
    }
    Ok(Snapshot {
        major,
        minor,
        accessible,
        used,
        mod_time_stamp,
        counters,
    })
}

/// UTF-8 quando válido; senão Windows-1252 (code page ANSI desta máquina).
fn decode_text(raw: &[u8]) -> String {
    match std::str::from_utf8(raw) {
        Ok(s) => s.to_owned(),
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
        0x80..=0x9f => HIGH[(b - 0x80) as usize],
        _ => b as char,
    }
}

impl Snapshot {
    pub fn long(&self, name: &str) -> Option<i64> {
        match self.counters.get(name)?.value {
            Value::Long(v) => Some(v),
            _ => None,
        }
    }

    pub fn text(&self, name: &str) -> Option<&str> {
        match &self.counters.get(name)?.value {
            Value::Text(s) => Some(s),
            _ => None,
        }
    }

    /// Soma de `sun.gc.generation.*.space.*.<campo>`.
    pub fn sum_spaces(&self, field: &str) -> i64 {
        self.counters
            .iter()
            .filter(|(k, _)| {
                k.starts_with("sun.gc.generation.")
                    && k.contains(".space.")
                    && k.ends_with(&format!(".{field}"))
            })
            .filter_map(|(_, c)| match c.value {
                Value::Long(v) => Some(v),
                _ => None,
            })
            .sum()
    }

    /// Os mesmos números do `jstat -gc` (em KB, como ele mostra; tempos em segundos).
    pub fn gc_summary(&self) -> GcSummary {
        let kb = |n: &str| self.long(n).map(|v| v as f64 / 1024.0);
        let freq = self.long("sun.os.hrt.frequency").unwrap_or(1).max(1) as f64;
        let secs = |n: &str| self.long(n).map(|v| v as f64 / freq);
        let collectors: Vec<(i64, f64)> = (0..4)
            .filter_map(|i| {
                Some((
                    self.long(&format!("sun.gc.collector.{i}.invocations"))?,
                    secs(&format!("sun.gc.collector.{i}.time"))?,
                ))
            })
            .collect();
        GcSummary {
            s0u: kb("sun.gc.generation.0.space.1.used"),
            s1u: kb("sun.gc.generation.0.space.2.used"),
            ec: kb("sun.gc.generation.0.space.0.capacity"),
            eu: kb("sun.gc.generation.0.space.0.used"),
            oc: kb("sun.gc.generation.1.space.0.capacity"),
            ou: kb("sun.gc.generation.1.space.0.used"),
            mu: kb("sun.gc.metaspace.used"),
            heap_used_kb: self.sum_spaces("used") as f64 / 1024.0,
            heap_capacity_kb: self.sum_spaces("capacity") as f64 / 1024.0,
            gc_count: collectors.iter().map(|c| c.0).sum(),
            gc_time_s: collectors.iter().map(|c| c.1).sum(),
            collectors,
        }
    }
}

#[derive(Debug, Clone)]
pub struct GcSummary {
    pub s0u: Option<f64>,
    pub s1u: Option<f64>,
    pub ec: Option<f64>,
    pub eu: Option<f64>,
    pub oc: Option<f64>,
    pub ou: Option<f64>,
    pub mu: Option<f64>,
    pub heap_used_kb: f64,
    pub heap_capacity_kb: f64,
    pub gc_count: i64,
    pub gc_time_s: f64,
    pub collectors: Vec<(i64, f64)>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_bad_magic() {
        let buf = [0u8; 64];
        assert!(matches!(parse(&buf), Err(ParseError::BadMagic(_))));
    }

    #[test]
    fn parses_fixtures() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures");
        let mut seen = 0;
        for e in std::fs::read_dir(dir).expect("fixtures") {
            let p = e.expect("entrada").path();
            if p.extension().and_then(|s| s.to_str()) != Some("hsperfdata") {
                continue;
            }
            let snap = parse(&std::fs::read(&p).expect("ler")).expect("parse");
            assert!(snap.accessible, "{p:?}");
            assert!(snap.counters.len() > 100, "{p:?}");
            assert!(snap.sum_spaces("used") > 0, "{p:?}");
            seen += 1;
        }
        assert!(
            seen >= 4,
            "esperava fixtures de 4 versões de Java, achei {seen}"
        );
    }

    /// Cada fixture tem ao lado a saída do `jstat -gc` (JDK 25) do mesmo processo,
    /// tirada logo depois, com o programa parado: todas as colunas têm de bater.
    #[test]
    fn fixtures_match_jstat() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures");
        let mut checked = 0;
        for e in std::fs::read_dir(&dir).expect("fixtures") {
            let p = e.expect("entrada").path();
            if p.extension().and_then(|s| s.to_str()) != Some("hsperfdata") {
                continue;
            }
            let jstat = std::fs::read_to_string(p.with_extension("jstat.txt")).expect("jstat.txt");
            let mut lines = jstat.lines().filter(|l| !l.trim().is_empty());
            let head: Vec<&str> = lines
                .next()
                .expect("cabeçalho")
                .split_whitespace()
                .collect();
            let vals: Vec<&str> = lines.next().expect("valores").split_whitespace().collect();
            let snap = parse(&std::fs::read(&p).expect("ler")).expect("parse");
            let freq = snap.long("sun.os.hrt.frequency").expect("frequência") as f64;
            let kb = |n: &str| snap.long(n).map(|v| v as f64 / 1024.0);
            let secs = |n: &str| snap.long(n).map(|v| v as f64 / freq);
            let count = |n: &str| snap.long(n).map(|v| v as f64);
            for (h, v) in head.iter().zip(vals) {
                let expected = match v {
                    "-" => None,
                    v => Some(v.replace(',', ".").parse::<f64>().expect("número")),
                };
                let (got, tol) = match *h {
                    "S0C" => (kb("sun.gc.generation.0.space.1.capacity"), 0.05),
                    "S1C" => (kb("sun.gc.generation.0.space.2.capacity"), 0.05),
                    "S0U" => (kb("sun.gc.generation.0.space.1.used"), 0.05),
                    "S1U" => (kb("sun.gc.generation.0.space.2.used"), 0.05),
                    "EC" => (kb("sun.gc.generation.0.space.0.capacity"), 0.05),
                    "EU" => (kb("sun.gc.generation.0.space.0.used"), 0.05),
                    "OC" => (kb("sun.gc.generation.1.space.0.capacity"), 0.05),
                    "OU" => (kb("sun.gc.generation.1.space.0.used"), 0.05),
                    "MC" => (kb("sun.gc.metaspace.capacity"), 0.05),
                    "MU" => (kb("sun.gc.metaspace.used"), 0.05),
                    "CCSC" => (kb("sun.gc.compressedclassspace.capacity"), 0.05),
                    "CCSU" => (kb("sun.gc.compressedclassspace.used"), 0.05),
                    "YGC" => (count("sun.gc.collector.0.invocations"), 0.0),
                    "FGC" => (count("sun.gc.collector.1.invocations"), 0.0),
                    "CGC" => (count("sun.gc.collector.2.invocations"), 0.0),
                    "YGCT" => (secs("sun.gc.collector.0.time"), 0.0005),
                    "FGCT" => (secs("sun.gc.collector.1.time"), 0.0005),
                    "CGCT" => (secs("sun.gc.collector.2.time"), 0.0005),
                    _ => continue, // GCT: soma, conferida pelas partes
                };
                match (expected, got) {
                    (None, None) => {}
                    (Some(x), Some(g)) => {
                        assert!((x - g).abs() <= tol, "{p:?} {h}: jstat {x}, leitor {g}")
                    }
                    (x, g) => panic!("{p:?} {h}: jstat {x:?}, leitor {g:?}"),
                }
                checked += 1;
            }
        }
        assert!(checked > 100, "conferiu só {checked} colunas");
    }
}
