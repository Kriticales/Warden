//! Leitor incremental da saída do jogo (ARCHITECTURE §7.4): aceita na mesma saída eventos
//! XML do log4j (`<log4j:Event …>`, configuração da Mojang) e texto puro (Forge/NeoForge,
//! JVM, `System.out` dos mods), e produz [`LogLine`].
//!
//! Um evento XML só começa numa linha que começa com `<log4j:Event`; texto com `<init>` ou
//! outras tags no meio continua sendo texto (o defeito do theseus no S1 §4.3 foi tratar isso
//! como XML e reter a saída). Um evento que não fecha em [`MAX_EVENT_LINES`] linhas volta como
//! texto, linha a linha, para nada ficar retido.

use quick_xml::events::Event;
use serde::{Deserialize, Serialize};

/// Linhas que um evento XML pode ter antes de ser tratado como texto.
pub const MAX_EVENT_LINES: usize = 2_000;

/// De qual pipe veio.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum LogSource {
    /// Saída padrão.
    Stdout,
    /// Saída de erro.
    Stderr,
}

/// Nível da linha.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, specta::Type,
)]
#[serde(rename_all = "camelCase")]
pub enum LogLevel {
    /// `TRACE`.
    Trace,
    /// `DEBUG`.
    Debug,
    /// `INFO`.
    Info,
    /// `WARN`.
    Warn,
    /// `ERROR`.
    Error,
    /// `FATAL`.
    Fatal,
}

impl LogLevel {
    /// O nível escrito pelo log4j.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        Some(match text.trim() {
            "TRACE" => Self::Trace,
            "DEBUG" => Self::Debug,
            "INFO" => Self::Info,
            "WARN" | "WARNING" => Self::Warn,
            "ERROR" | "SEVERE" => Self::Error,
            "FATAL" => Self::Fatal,
            _ => return None,
        })
    }
}

/// Uma linha (ou um evento do log4j) da saída do jogo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct LogLine {
    /// Milissegundos desde o início do processo, pelo relógio do Warden (o do log não é
    /// confiável no Java 8u51; S-R5-3 §13).
    #[specta(type = specta_typescript::Number)]
    pub elapsed_ms: u64,
    /// Carimbo do evento XML (milissegundos desde 1970), quando houver.
    #[specta(type = Option<specta_typescript::Number>)]
    pub timestamp_ms: Option<i64>,
    /// Hora escrita na linha de texto (`19:53:26`, `04Oct2026 19:53:26.123`), quando houver.
    pub time: Option<String>,
    /// Nível.
    pub level: Option<LogLevel>,
    /// Thread (`Render thread`, `main`).
    pub thread: Option<String>,
    /// Logger (`net.minecraft.client.Minecraft`, `minecraft/Minecraft`).
    pub logger: Option<String>,
    /// A mensagem; num evento com exceção, a mensagem e o stack trace, separados por `\n`.
    pub text: String,
    /// O pipe.
    pub source: LogSource,
}

/// Leitor de um pipe. Guarda o evento XML em andamento.
#[derive(Debug)]
pub struct LogParser {
    source: LogSource,
    pending: Vec<String>,
}

impl LogParser {
    /// Leitor de um pipe.
    #[must_use]
    pub const fn new(source: LogSource) -> Self {
        Self {
            source,
            pending: Vec::new(),
        }
    }

    /// Recebe uma linha (já decodificada, sem o fim de linha) e devolve as linhas prontas.
    pub fn push(&mut self, line: &str, elapsed_ms: u64) -> Vec<LogLine> {
        if self.pending.is_empty() {
            if line.trim_start().starts_with("<log4j:Event") {
                self.pending.push(line.to_owned());
                return self.try_finish(elapsed_ms);
            }
            return vec![parse_text(line, self.source, elapsed_ms)];
        }
        self.pending.push(line.to_owned());
        if self.pending.len() >= MAX_EVENT_LINES {
            return self.flush_as_text(elapsed_ms);
        }
        self.try_finish(elapsed_ms)
    }

    /// Fim do pipe: o que estiver pendente sai como texto.
    pub fn finish(&mut self, elapsed_ms: u64) -> Vec<LogLine> {
        self.flush_as_text(elapsed_ms)
    }

    fn try_finish(&mut self, elapsed_ms: u64) -> Vec<LogLine> {
        let closed = self
            .pending
            .last()
            .is_some_and(|line| line.contains("</log4j:Event>"));
        if !closed {
            return Vec::new();
        }
        let xml = self.pending.join("\n");
        match parse_event(&xml, self.source, elapsed_ms) {
            Some(line) => {
                self.pending.clear();
                vec![line]
            }
            None => self.flush_as_text(elapsed_ms),
        }
    }

    fn flush_as_text(&mut self, elapsed_ms: u64) -> Vec<LogLine> {
        std::mem::take(&mut self.pending)
            .iter()
            .map(|line| parse_text(line, self.source, elapsed_ms))
            .collect()
    }
}

/// Em que parte do evento o leitor está.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Part {
    Other,
    Message,
    Throwable,
}

/// Lê um evento `<log4j:Event>` completo.
fn parse_event(xml: &str, source: LogSource, elapsed_ms: u64) -> Option<LogLine> {
    let mut reader = quick_xml::Reader::from_str(xml);
    let mut line = LogLine {
        elapsed_ms,
        timestamp_ms: None,
        time: None,
        level: None,
        thread: None,
        logger: None,
        text: String::new(),
        source,
    };
    let mut message = String::new();
    let mut throwable = String::new();
    let mut part = Part::Other;
    let mut saw_event = false;
    loop {
        match reader.read_event().ok()? {
            Event::Start(tag) | Event::Empty(tag) => match tag.name().as_ref() {
                "log4j:Event" => {
                    saw_event = true;
                    for attribute in tag.attributes().flatten() {
                        let value = attribute
                            .normalized_value(quick_xml::XmlVersion::Implicit1_0)
                            .ok()?
                            .into_owned();
                        match attribute.key.as_ref() {
                            "logger" => line.logger = Some(value),
                            "timestamp" => line.timestamp_ms = value.parse().ok(),
                            "level" => line.level = LogLevel::parse(&value),
                            "thread" => line.thread = Some(value),
                            _ => {}
                        }
                    }
                }
                "log4j:Message" => part = Part::Message,
                "log4j:Throwable" => part = Part::Throwable,
                _ => {}
            },
            Event::End(_) => part = Part::Other,
            Event::CData(data) => {
                let text: &str = data.as_ref();
                match part {
                    Part::Message => message.push_str(text),
                    Part::Throwable => throwable.push_str(text),
                    Part::Other => {}
                }
            }
            Event::Text(text) => {
                let text = quick_xml::escape::unescape(text.as_ref()).ok()?;
                match part {
                    Part::Message => message.push_str(&text),
                    Part::Throwable => throwable.push_str(&text),
                    Part::Other => {}
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }
    if !saw_event {
        return None;
    }
    let throwable = throwable.trim_end();
    line.text = if throwable.is_empty() {
        message
    } else {
        format!("{message}\n{throwable}")
    };
    Some(line)
}

/// Tira as sequências de cor ANSI (`ESC [ … m`) que o Forge/NeoForge põem no console.
fn strip_ansi(text: &str) -> std::borrow::Cow<'_, str> {
    if !text.contains('\u{1b}') {
        return std::borrow::Cow::Borrowed(text);
    }
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(character) = chars.next() {
        if character == '\u{1b}' && chars.peek() == Some(&'[') {
            chars.next();
            for next in chars.by_ref() {
                if next.is_ascii_alphabetic() {
                    break;
                }
            }
        } else {
            out.push(character);
        }
    }
    std::borrow::Cow::Owned(out)
}

/// O grupo `[…]` do começo do texto (depois de espaços) e o resto.
fn bracket(text: &str) -> Option<(&str, &str)> {
    let inner = text.trim_start_matches(' ').strip_prefix('[')?;
    let end = inner.find(']')?;
    Some((&inner[..end], &inner[end + 1..]))
}

/// `thread/NÍVEL`, se o grupo for isso.
fn thread_level(group: &str) -> Option<(&str, LogLevel)> {
    let (thread, level) = group.rsplit_once('/')?;
    Some((thread, LogLevel::parse(level)?))
}

/// Lê uma linha de texto nos formatos do log4j:
/// `[19:53:26] [main/WARN] [logger/]: msg` (NeoForge), `[04Oct2026 19:53:26.123]
/// [main/INFO] [logger/]: msg` (Forge), `[19:53:26] [Render thread/INFO]: msg` (vanilla sem
/// configuração). Outras linhas (stack traces, JVM) ficam como texto, sem nível.
fn parse_text(raw: &str, source: LogSource, elapsed_ms: u64) -> LogLine {
    let clean = strip_ansi(raw);
    let mut line = LogLine {
        elapsed_ms,
        timestamp_ms: None,
        time: None,
        level: None,
        thread: None,
        logger: None,
        text: clean.to_string(),
        source,
    };
    let Some((first, after_first)) = bracket(&clean) else {
        return line;
    };
    // `[thread/NÍVEL]` no começo ou depois da hora.
    let (time, thread, level, mut rest) = if let Some((thread, level)) = thread_level(first) {
        (None, thread, level, after_first)
    } else {
        let Some((second, after_second)) = bracket(after_first) else {
            return line;
        };
        let Some((thread, level)) = thread_level(second) else {
            return line;
        };
        (Some(first), thread, level, after_second)
    };
    // `[logger/marcador]` logo depois, sem os dois-pontos antes.
    let mut logger = None;
    if rest.starts_with(" [")
        && let Some((group, after)) = bracket(rest)
    {
        logger = Some(group.trim_end_matches('/').to_owned());
        rest = after;
    }
    let message = rest.strip_prefix(':').unwrap_or(rest);
    line.time = time.map(str::to_owned);
    line.thread = Some(thread.to_owned());
    line.level = Some(level);
    line.logger = logger;
    message
        .strip_prefix(' ')
        .unwrap_or(message)
        .clone_into(&mut line.text);
    line
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    fn feed(parser: &mut LogParser, text: &str) -> Vec<LogLine> {
        text.lines()
            .enumerate()
            .flat_map(|(index, line)| parser.push(line, index as u64))
            .collect()
    }

    #[test]
    fn evento_xml_de_varias_linhas_com_excecao() {
        let text = concat!(
            "  <log4j:Event logger=\"net.minecraft.class_310\" timestamp=\"1791156013396\" level=\"ERROR\" thread=\"Render thread\">\n",
            "    <log4j:Message><![CDATA[Falhou: ação inválida\n",
            "segunda linha]]></log4j:Message>\n",
            "    <log4j:Throwable><![CDATA[java.lang.RuntimeException: x\n",
            "\tat a.b.C.<init>(C.java:1)\n",
            "]]></log4j:Throwable>\n",
            "  </log4j:Event>\n",
        );
        let mut parser = LogParser::new(LogSource::Stdout);
        let lines = feed(&mut parser, text);
        assert_eq!(lines.len(), 1);
        let line = &lines[0];
        assert_eq!(line.level, Some(LogLevel::Error));
        assert_eq!(line.thread.as_deref(), Some("Render thread"));
        assert_eq!(line.logger.as_deref(), Some("net.minecraft.class_310"));
        assert_eq!(line.timestamp_ms, Some(1_791_156_013_396));
        assert_eq!(
            line.text,
            "Falhou: ação inválida\nsegunda linha\njava.lang.RuntimeException: x\n\tat a.b.C.<init>(C.java:1)"
        );
        assert_eq!(line.elapsed_ms, 6);
    }

    #[test]
    fn cdata_partido_pelo_log4j() {
        let text = "<log4j:Event logger=\"x\" timestamp=\"1\" level=\"INFO\" thread=\"main\"><log4j:Message><![CDATA[a]]]]><![CDATA[>b]]></log4j:Message></log4j:Event>";
        let mut parser = LogParser::new(LogSource::Stdout);
        let lines = parser.push(text, 0);
        assert_eq!(lines[0].text, "a]]>b");
    }

    #[test]
    fn texto_do_neoforge_e_do_forge() {
        let mut parser = LogParser::new(LogSource::Stdout);
        let line = &parser.push(
            "[19:53:26] [main/WARN] [os.ut.pl.wi.PerfDataUtil/]: Failed to add PDH Counter",
            0,
        )[0];
        assert_eq!(line.time.as_deref(), Some("19:53:26"));
        assert_eq!(line.thread.as_deref(), Some("main"));
        assert_eq!(line.level, Some(LogLevel::Warn));
        assert_eq!(line.logger.as_deref(), Some("os.ut.pl.wi.PerfDataUtil"));
        assert_eq!(line.text, "Failed to add PDH Counter");

        let line = &parser.push(
            "\u{1b}[32m[04Oct2026 19:53:26.123] [Render thread/INFO] [net.minecraft.client.Minecraft/]: Setting user: WardenTest\u{1b}[m",
            0,
        )[0];
        assert_eq!(line.time.as_deref(), Some("04Oct2026 19:53:26.123"));
        assert_eq!(line.thread.as_deref(), Some("Render thread"));
        assert_eq!(line.level, Some(LogLevel::Info));
        assert_eq!(line.text, "Setting user: WardenTest");

        let line = &parser.push(
            "[10:00:00] [Server thread/INFO]: Done (3,611s)! For help, type \"help\" or \"?\"",
            0,
        )[0];
        assert_eq!(line.logger, None);
        assert_eq!(line.text, "Done (3,611s)! For help, type \"help\" or \"?\"");
    }

    #[test]
    fn texto_sem_formato_fica_inteiro() {
        let mut parser = LogParser::new(LogSource::Stderr);
        for raw in [
            "\tat net.minecraft.client.Minecraft.<init>(Minecraft.java:1)",
            "Exception in thread \"main\" java.lang.Error",
            "[LWJGL] OpenAL error",
            "",
        ] {
            let line = &parser.push(raw, 3)[0];
            assert_eq!(line.text, raw);
            assert_eq!(line.level, None);
            assert_eq!(line.source, LogSource::Stderr);
        }
    }

    #[test]
    fn evento_que_nao_fecha_nao_retem_a_saida() {
        let mut parser = LogParser::new(LogSource::Stdout);
        assert!(
            parser
                .push("<log4j:Event logger=\"x\" level=\"INFO\">", 0)
                .is_empty()
        );
        let mut released = Vec::new();
        for index in 1..MAX_EVENT_LINES {
            released.extend(parser.push(&format!("linha {index}"), 0));
        }
        assert_eq!(released.len(), MAX_EVENT_LINES);
        assert_eq!(released[1].text, "linha 1");
        // O fim do pipe solta o que restar.
        assert!(parser.push("<log4j:Event logger=\"y\">", 0).is_empty());
        assert_eq!(parser.finish(0).len(), 1);
    }

    #[test]
    fn evento_estragado_vira_texto() {
        let mut parser = LogParser::new(LogSource::Stdout);
        assert!(
            parser
                .push("<log4j:Event logger=\"x\" level=\"INFO\">", 0)
                .is_empty()
        );
        assert!(parser.push("<log4j:Message><![CDATA[sem fim", 0).is_empty());
        let lines = parser.push("</log4j:Event>", 0);
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[1].text, "<log4j:Message><![CDATA[sem fim");
    }

    proptest! {
        #[test]
        fn nunca_perde_linhas_de_texto(lines in proptest::collection::vec("[^<\r\n]{0,60}", 0..40)) {
            let mut parser = LogParser::new(LogSource::Stdout);
            let mut out = Vec::new();
            for line in &lines {
                out.extend(parser.push(line, 0));
            }
            out.extend(parser.finish(0));
            prop_assert_eq!(out.len(), lines.len());
        }

        #[test]
        fn entradas_aleatorias_nao_quebram(lines in proptest::collection::vec("\\PC{0,80}", 0..40)) {
            let mut parser = LogParser::new(LogSource::Stdout);
            let mut total = 0;
            for line in &lines {
                total += parser.push(line, 0).len();
            }
            total += parser.finish(0).len();
            prop_assert!(total <= lines.len());
        }
    }
}
