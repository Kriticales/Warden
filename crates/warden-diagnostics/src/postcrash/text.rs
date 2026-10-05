//! Texto dos logs: decodificação, limites de tamanho e limpeza (ARCHITECTURE §9.3 item 2).
//!
//! - **Decodificação** linha a linha: UTF-8 quando a linha é UTF-8 válido, senão
//!   Windows-1252. A saída do Java 8–17 no Windows sai na página de código do console no meio de
//!   linhas UTF-8 do log4j (R2 §6.2), então decidir por arquivo inteiro estragaria uma das duas.
//! - **Limite:** arquivos enormes (um `latest.log` com spam de avisos passa de 100 MB) guardam
//!   o começo e o fim; as linhas do meio são contadas, para que o número de cada linha continue
//!   igual ao do arquivo.
//! - **Limpeza:** tira códigos `§x` do Minecraft e sequências ANSI, e desembrulha os eventos XML
//!   do log4j (`LegacyXMLLayout`, usado na saída padrão do vanilla, do Fabric e do começo do
//!   Forge/NeoForge) **sem mudar a numeração**: cada linha física continua sendo uma linha, só
//!   sem as marcas XML.

use std::fs::File;
use std::io::{self, BufRead, BufReader, Read, Seek, SeekFrom};
use std::path::Path;
use std::sync::LazyLock;

use regex::Regex;

/// Bytes lidos do começo de um arquivo grande.
pub const HEAD_BYTES: u64 = 4 * 1024 * 1024;
/// Bytes lidos do fim de um arquivo grande.
pub const TAIL_BYTES: u64 = 28 * 1024 * 1024;

/// Uma linha do arquivo original, já decodificada.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RawLine {
    /// Número da linha no arquivo (começa em 1).
    pub number: u32,
    /// O texto, sem o fim de linha.
    pub text: String,
}

/// Uma linha limpa, pronta para os padrões.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CleanLine {
    /// Número da linha no arquivo original.
    pub number: u32,
    /// O texto sem códigos de formatação e sem marcas XML.
    pub text: String,
    /// A linha começa um registro novo do log (evento XML ou linha com data/hora). Usado para
    /// saber onde termina um crash report impresso no meio do log.
    pub record_start: bool,
}

/// Decodifica uma linha: UTF-8 se for válido, senão Windows-1252.
#[must_use]
pub(crate) fn decode_line(bytes: &[u8]) -> String {
    match std::str::from_utf8(bytes) {
        Ok(text) => text.to_owned(),
        Err(_) => encoding_rs::WINDOWS_1252
            .decode_without_bom_handling(bytes)
            .0
            .into_owned(),
    }
}

/// Separa e decodifica as linhas de um bloco de bytes, numerando a partir de `first`.
fn split_lines(bytes: &[u8], first: u32, out: &mut Vec<RawLine>) {
    let bytes = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
    if bytes.is_empty() {
        return;
    }
    let body = bytes.strip_suffix(b"\n").unwrap_or(bytes);
    let mut number = first;
    for line in body.split(|&byte| byte == b'\n') {
        let line = line.strip_suffix(b"\r").unwrap_or(line);
        out.push(RawLine {
            number,
            text: decode_line(line),
        });
        number = number.saturating_add(1);
    }
}

/// Linhas de um texto já em memória (`analyze_text`).
#[must_use]
pub(crate) fn lines_from_bytes(bytes: &[u8]) -> Vec<RawLine> {
    let mut lines = Vec::new();
    split_lines(bytes, 1, &mut lines);
    lines
}

/// Lê um arquivo com o limite de tamanho: até `HEAD_BYTES + TAIL_BYTES` inteiro; acima disso,
/// o começo e o fim, com as linhas do meio contadas.
pub(crate) fn read_lines_capped(path: &Path) -> io::Result<Vec<RawLine>> {
    read_lines_with_limits(path, HEAD_BYTES, TAIL_BYTES)
}

/// [`read_lines_capped`] com limites escolhidos (testes).
pub(crate) fn read_lines_with_limits(
    path: &Path,
    head: u64,
    tail: u64,
) -> io::Result<Vec<RawLine>> {
    let mut file = File::open(path)?;
    let size = file.metadata()?.len();
    if size <= head.saturating_add(tail) {
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes)?;
        return Ok(lines_from_bytes(&bytes));
    }

    // Começo: até a última quebra de linha dentro dos primeiros `head` bytes.
    let mut head_bytes = Vec::new();
    (&mut file).take(head).read_to_end(&mut head_bytes)?;
    let head_end = head_bytes
        .iter()
        .rposition(|&byte| byte == b'\n')
        .map_or(0, |position| position + 1);
    head_bytes.truncate(head_end);
    let mut lines = Vec::new();
    split_lines(&head_bytes, 1, &mut lines);
    let head_lines = u32::try_from(lines.len()).unwrap_or(u32::MAX);

    // Meio: só conta as quebras de linha até o começo do fim.
    let tail_start = size - tail;
    file.seek(SeekFrom::Start(head_end as u64))?;
    let mut middle = BufReader::new((&mut file).take(tail_start - head_end as u64));
    let mut skipped: u32 = 0;
    loop {
        let buffer = middle.fill_buf()?;
        if buffer.is_empty() {
            break;
        }
        // Contagem simples: o meio de um log gigante é lido uma vez, sem dependência extra.
        #[allow(clippy::naive_bytecount)]
        let count = buffer.iter().filter(|&&byte| byte == b'\n').count();
        skipped = skipped.saturating_add(u32::try_from(count).unwrap_or(u32::MAX));
        let length = buffer.len();
        middle.consume(length);
    }

    // Fim: a partir da primeira quebra de linha (a linha cortada no meio fica de fora e conta
    // como pulada).
    file.seek(SeekFrom::Start(tail_start))?;
    let mut tail_bytes = Vec::new();
    file.read_to_end(&mut tail_bytes)?;
    let tail_from = tail_bytes
        .iter()
        .position(|&byte| byte == b'\n')
        .map_or(tail_bytes.len(), |position| position + 1);
    let partial = u32::from(tail_from > 0);
    let first = head_lines
        .saturating_add(skipped)
        .saturating_add(partial)
        .saturating_add(1);
    split_lines(&tail_bytes[tail_from..], first, &mut lines);
    Ok(lines)
}

static ANSI: LazyLock<Regex> = LazyLock::new(|| {
    // CSI (`ESC [ … letra`), OSC (`ESC ] … BEL` ou `ESC \`) e escapes de dois caracteres.
    fixed_regex(r"\x1b(?:\[[0-?]*[ -/]*[@-~]|\][^\x07\x1b]*(?:\x07|\x1b\\)|[@-Z\\-_])")
});

static FORMATTING_CODE: LazyLock<Regex> = LazyLock::new(|| fixed_regex(r"§[0-9a-fk-orxA-FK-ORX]"));

static RECORD_START: LazyLock<Regex> = LazyLock::new(|| {
    fixed_regex(concat!(
        r"^(?:\s*<log4j:Event\b",
        r"|\[\d{1,2}:\d{2}:\d{2}(?:\.\d+)?\]",
        r"|\[\d{2}[A-Za-z]{3}\d{4} \d{2}:\d{2}:\d{2}(?:\.\d+)?\]",
        r"|\d{4}-\d{2}-\d{2}[ T]\d{2}:\d{2}:\d{2}",
        r"|#@!@#",
        r")"
    ))
});

static EVENT_HEADER: LazyLock<Regex> =
    LazyLock::new(|| fixed_regex(r"^\s*<log4j:Event\b([^>]*)>\s*$"));

static ATTRIBUTE: LazyLock<Regex> = LazyLock::new(|| fixed_regex(r#"(\w+)="([^"]*)""#));

/// Compila uma expressão fixa do código.
#[allow(clippy::expect_used)] // só expressões literais da crate, conferidas pelos testes
pub(crate) fn fixed_regex(pattern: &str) -> Regex {
    Regex::new(pattern).expect("expressão fixa válida")
}

/// Tira códigos ANSI e `§x`, `\r` e `\u{0}`.
#[must_use]
pub fn strip_formatting(text: &str) -> String {
    let without_ansi = ANSI.replace_all(text, "");
    let without_codes = FORMATTING_CODE.replace_all(&without_ansi, "");
    without_codes.replace(['\r', '\u{0}'], "")
}

/// Tira as marcas XML do log4j de uma linha, mantendo o texto.
fn unwrap_log4j(text: &str) -> String {
    if let Some(captures) = EVENT_HEADER.captures(text) {
        let attributes = captures.get(1).map_or("", |m| m.as_str());
        let mut logger = "";
        let mut level = "";
        let mut thread = "";
        for attribute in ATTRIBUTE.captures_iter(attributes) {
            let value = attribute.get(2).map_or("", |m| m.as_str());
            match attribute.get(1).map_or("", |m| m.as_str()) {
                "logger" => logger = value,
                "level" => level = value,
                "thread" => thread = value,
                _ => {}
            }
        }
        return xml_unescape(&format!("[{thread}/{level}] [{logger}]:"));
    }
    let trimmed = text.trim_start();
    if trimmed.starts_with("</log4j:Event>") {
        return String::new();
    }
    let mut line = text.to_owned();
    let mut escaped = false;
    for opening in ["<log4j:Message><![CDATA[", "<log4j:Throwable><![CDATA["] {
        if let Some(position) = line.find(opening)
            && line[..position].trim().is_empty()
        {
            line.replace_range(..position + opening.len(), "");
        }
    }
    for opening in ["<log4j:Message>", "<log4j:Throwable>"] {
        if let Some(position) = line.find(opening)
            && line[..position].trim().is_empty()
        {
            line.replace_range(..position + opening.len(), "");
            escaped = true;
        }
    }
    for closing in [
        "]]></log4j:Message>",
        "]]></log4j:Throwable>",
        "</log4j:Message>",
        "</log4j:Throwable>",
    ] {
        if let Some(stripped) = line.trim_end().strip_suffix(closing) {
            let length = stripped.len();
            line.truncate(length);
            if !closing.starts_with("]]") {
                escaped = true;
            }
        }
    }
    if escaped { xml_unescape(&line) } else { line }
}

fn xml_unescape(text: &str) -> String {
    if !text.contains('&') {
        return text.to_owned();
    }
    text.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
}

/// Limpa as linhas: formatação, XML do log4j e marca de começo de registro.
#[must_use]
pub(crate) fn clean_lines(lines: &[RawLine]) -> Vec<CleanLine> {
    lines
        .iter()
        .map(|line| {
            let stripped = strip_formatting(&line.text);
            CleanLine {
                number: line.number,
                record_start: RECORD_START.is_match(&stripped),
                text: unwrap_log4j(&stripped),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::io::Write as _;

    use super::*;

    #[test]
    fn decodifica_utf8_e_recua_para_windows_1252_por_linha() {
        let bytes = b"\xEF\xBB\xBFcaf\xC3\xA9\r\nJos\xE9\n\nfim";
        let lines = lines_from_bytes(bytes);
        let texts: Vec<_> = lines.iter().map(|line| line.text.as_str()).collect();
        assert_eq!(texts, ["café", "José", "", "fim"]);
        assert_eq!(lines[3].number, 4);
        assert!(lines_from_bytes(b"").is_empty());
        assert_eq!(lines_from_bytes(b"a\n").len(), 1);
    }

    #[test]
    fn tira_ansi_e_codigos_de_formatacao() {
        assert_eq!(
            strip_formatting("\x1b[31;1mErro\x1b[0m §eMod§r §6moonlight§r\r"),
            "Erro Mod moonlight"
        );
        assert_eq!(strip_formatting("\x1b]0;título\x07ok"), "ok");
        assert_eq!(strip_formatting("§x§f§f§0§0§0§0cor"), "cor");
    }

    #[test]
    fn desembrulha_eventos_xml_sem_mudar_a_numeracao() {
        let raw = lines_from_bytes(
            br#"  <log4j:Event logger="FabricLoader" timestamp="1" level="ERROR" thread="main">
    <log4j:Message><![CDATA[Incompatible mods found!]]></log4j:Message>
    <log4j:Throwable><![CDATA[net.fabricmc.loader.impl.FormattedException: x
	at a.b(C.java:1)
]]></log4j:Throwable>
  </log4j:Event>
    <log4j:Message>a &lt;b&gt; &amp; c</log4j:Message>
[19:53:33] [Render thread/FATAL] [ne.ne.fm.ModLoader/CORE]: Error"#,
        );
        let clean = clean_lines(&raw);
        let texts: Vec<_> = clean.iter().map(|line| line.text.as_str()).collect();
        assert_eq!(
            texts,
            [
                "[main/ERROR] [FabricLoader]:",
                "Incompatible mods found!",
                "net.fabricmc.loader.impl.FormattedException: x",
                "\tat a.b(C.java:1)",
                "",
                "",
                "a <b> & c",
                "[19:53:33] [Render thread/FATAL] [ne.ne.fm.ModLoader/CORE]: Error",
            ]
        );
        let starts: Vec<_> = clean.iter().map(|line| line.record_start).collect();
        assert_eq!(
            starts,
            [true, false, false, false, false, false, false, true]
        );
        assert_eq!(clean[7].number, 8);
    }

    #[test]
    fn marca_comeco_de_registro_nos_formatos_conhecidos() {
        for line in [
            "[12:00:01] [main/INFO]: x",
            "[04Oct2026 19:53:33.123] [main/INFO] [cpw/]: x",
            "2026-10-04 19:42:36,564 main WARN x",
            "#@!@# Game crashed!",
        ] {
            let clean = clean_lines(&[RawLine {
                number: 1,
                text: line.to_owned(),
            }]);
            assert!(clean[0].record_start, "{line}");
        }
        let clean = clean_lines(&[RawLine {
            number: 1,
            text: "\tat x".to_owned(),
        }]);
        assert!(!clean[0].record_start);
    }

    #[test]
    fn arquivo_grande_guarda_comeco_e_fim_com_a_numeracao_certa() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("latest.log");
        let mut file = File::create(&path).unwrap();
        for number in 1..=1000 {
            writeln!(file, "linha {number:04}").unwrap();
        }
        drop(file);
        // Cada linha tem 11 bytes ("linha 0001\n").
        let lines = read_lines_with_limits(&path, 55, 110).unwrap();
        assert_eq!(lines.first().unwrap().text, "linha 0001");
        assert_eq!(lines[4].text, "linha 0005");
        assert_eq!(lines[4].number, 5);
        let last = lines.last().unwrap();
        assert_eq!(last.text, "linha 1000");
        assert_eq!(last.number, 1000);
        for line in &lines {
            assert_eq!(line.text, format!("linha {:04}", line.number));
        }
        // Pequeno: lido inteiro.
        let all = read_lines_capped(&path).unwrap();
        assert_eq!(all.len(), 1000);
    }

    #[test]
    fn expressoes_fixas_compilam() {
        for pattern in [
            &ANSI,
            &FORMATTING_CODE,
            &RECORD_START,
            &EVENT_HEADER,
            &ATTRIBUTE,
        ] {
            LazyLock::force(pattern);
        }
    }
}
