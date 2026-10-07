//! Linhas do console do teste (SPEC T13 "Console"): as do jogo, como saíram (sem tradução), e
//! as do Warden ("Abrindo o jogo…", "O jogo fechou…"), que vão como chave do catálogo da
//! interface com os valores.
//!
//! O buffer guarda as últimas [`MAX_LINES`] linhas da sessão aberta (a interface mostra até
//! esse limite); tudo o que o jogo escreveu fica no `output.log` da sessão.

use std::collections::{BTreeMap, VecDeque};
use std::io::{BufRead, BufReader};
use std::path::Path;

use serde::Serialize;
use warden_launcher::decode::{decode_line, trim_line_end};
use warden_launcher::log::{LogLevel, LogLine, LogParser, LogSource};

/// Linhas mostradas no console (SPEC T13: "Mostra até 50.000 linhas na tela").
pub(crate) const MAX_LINES: usize = 50_000;

/// Nível da linha (o do log4j). Tipo próprio do console: o nome `LogLevel` já é o do nível dos
/// registros do Warden (Configurações) no `bindings.ts`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum ConsoleLevel {
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

impl From<LogLevel> for ConsoleLevel {
    fn from(level: LogLevel) -> Self {
        match level {
            LogLevel::Trace => Self::Trace,
            LogLevel::Debug => Self::Debug,
            LogLevel::Info => Self::Info,
            LogLevel::Warn => Self::Warn,
            LogLevel::Error => Self::Error,
            LogLevel::Fatal => Self::Fatal,
        }
    }
}

/// Pipe de uma linha do jogo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum ConsoleStream {
    /// Saída padrão.
    Stdout,
    /// Saída de erro.
    Stderr,
}

impl From<LogSource> for ConsoleStream {
    fn from(source: LogSource) -> Self {
        match source {
            LogSource::Stdout => Self::Stdout,
            LogSource::Stderr => Self::Stderr,
        }
    }
}

/// De onde veio a linha.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum ConsoleOrigin {
    /// Saída do jogo.
    Game,
    /// Mensagem do Warden: `text` é a chave do catálogo `teste` e `params`, os valores.
    Warden,
}

/// Uma linha do console.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ConsoleLine {
    /// Número da linha na sessão (cresce sempre; serve de chave na lista).
    #[specta(type = specta_typescript::Number)]
    pub seq: u64,
    /// Quando o Warden recebeu a linha (milissegundos desde 1970), pelo relógio do Warden (o
    /// do log não é confiável no Java 8u51). `None` nas linhas lidas de uma sessão gravada.
    #[specta(type = Option<specta_typescript::Number>)]
    pub at_ms: Option<u64>,
    /// Hora escrita pelo próprio jogo na linha (`19:53:26`), quando houver.
    pub time: Option<String>,
    /// Nível.
    pub level: Option<ConsoleLevel>,
    /// Logger do jogo (`minecraft/Minecraft`), a origem mostrada no console.
    pub logger: Option<String>,
    /// Thread.
    pub thread: Option<String>,
    /// O texto (jogo) ou a chave do catálogo (Warden).
    pub text: String,
    /// Valores da frase do Warden.
    pub params: BTreeMap<String, String>,
    /// Jogo ou Warden.
    pub origin: ConsoleOrigin,
    /// Pipe da linha do jogo.
    pub stream: Option<ConsoleStream>,
}

impl ConsoleLine {
    /// Linha do jogo.
    pub(crate) fn from_game(line: LogLine, started_at_ms: Option<u64>) -> Self {
        Self {
            seq: 0,
            at_ms: started_at_ms.map(|start| start.saturating_add(line.elapsed_ms)),
            time: line.time,
            level: line.level.map(ConsoleLevel::from),
            logger: line.logger,
            thread: line.thread,
            text: line.text,
            params: BTreeMap::new(),
            origin: ConsoleOrigin::Game,
            stream: Some(line.source.into()),
        }
    }

    /// Mensagem do Warden.
    pub(crate) fn warden(
        key: &str,
        level: ConsoleLevel,
        at_ms: u64,
        params: impl IntoIterator<Item = (&'static str, String)>,
    ) -> Self {
        Self {
            seq: 0,
            at_ms: Some(at_ms),
            time: None,
            level: Some(level),
            logger: None,
            thread: None,
            text: key.to_owned(),
            params: params
                .into_iter()
                .map(|(name, value)| (name.to_owned(), value))
                .collect(),
            origin: ConsoleOrigin::Warden,
            stream: None,
        }
    }
}

/// As últimas linhas da sessão aberta.
#[derive(Debug, Default)]
pub(crate) struct ConsoleBuffer {
    lines: VecDeque<ConsoleLine>,
    next_seq: u64,
}

impl ConsoleBuffer {
    /// Numera e guarda as linhas; devolve as linhas numeradas para o canal.
    pub(crate) fn push(&mut self, lines: Vec<ConsoleLine>) -> Vec<ConsoleLine> {
        let numbered: Vec<ConsoleLine> = lines
            .into_iter()
            .map(|mut line| {
                line.seq = self.next_seq;
                self.next_seq += 1;
                line
            })
            .collect();
        self.lines.extend(numbered.iter().cloned());
        while self.lines.len() > MAX_LINES {
            self.lines.pop_front();
        }
        numbered
    }

    /// Cópia das linhas guardadas.
    pub(crate) fn snapshot(&self) -> Vec<ConsoleLine> {
        self.lines.iter().cloned().collect()
    }
}

/// As últimas [`MAX_LINES`] linhas do `output.log` de uma sessão gravada, lidas pelo mesmo
/// leitor do jogo ao vivo (eventos XML do log4j e texto). Devolve as linhas e se o começo foi
/// cortado. Arquivo ausente: nenhuma linha.
pub(crate) fn read_output_log(path: &Path) -> std::io::Result<(Vec<ConsoleLine>, bool)> {
    let file = match std::fs::File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok((Vec::new(), false));
        }
        Err(error) => return Err(error),
    };
    let mut reader = BufReader::new(file);
    let mut parser = LogParser::new(LogSource::Stdout);
    let mut lines: VecDeque<ConsoleLine> = VecDeque::new();
    let mut truncated = false;
    let mut keep = |parsed: Vec<LogLine>, lines: &mut VecDeque<ConsoleLine>| {
        for line in parsed {
            lines.push_back(ConsoleLine::from_game(line, None));
            if lines.len() > MAX_LINES {
                lines.pop_front();
                truncated = true;
            }
        }
    };
    let mut buffer = Vec::with_capacity(512);
    loop {
        buffer.clear();
        if reader.read_until(b'\n', &mut buffer)? == 0 {
            break;
        }
        let text = decode_line(trim_line_end(&buffer));
        keep(parser.push(&text, 0), &mut lines);
    }
    keep(parser.finish(0), &mut lines);
    let lines = lines
        .into_iter()
        .enumerate()
        .map(|(index, mut line)| {
            line.seq = index as u64;
            line
        })
        .collect();
    Ok((lines, truncated))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn game(text: &str) -> ConsoleLine {
        ConsoleLine::from_game(
            LogLine {
                elapsed_ms: 1500,
                timestamp_ms: None,
                time: Some("12:00:00".into()),
                level: Some(LogLevel::Info),
                thread: Some("main".into()),
                logger: Some("teste".into()),
                text: text.into(),
                source: LogSource::Stdout,
            },
            Some(1_000_000),
        )
    }

    #[test]
    fn linha_do_jogo_usa_o_relogio_do_warden() {
        let line = game("olá");
        assert_eq!(line.at_ms, Some(1_001_500));
        assert_eq!(line.origin, ConsoleOrigin::Game);
        let json = serde_json::to_value(&line).unwrap();
        assert_eq!(json["level"], "info");
        assert_eq!(json["origin"], "game");
        assert_eq!(json["stream"], "stdout");
    }

    #[test]
    fn buffer_numera_e_guarda_so_as_ultimas() {
        let mut buffer = ConsoleBuffer::default();
        let first = buffer.push(vec![game("a"), game("b")]);
        assert_eq!(first.iter().map(|l| l.seq).collect::<Vec<_>>(), vec![0, 1]);
        let many: Vec<_> = (0..MAX_LINES).map(|i| game(&i.to_string())).collect();
        buffer.push(many);
        let lines = buffer.snapshot();
        assert_eq!(lines.len(), MAX_LINES);
        assert_eq!(lines[0].text, "0", "as duas primeiras saíram");
        assert_eq!(lines.last().unwrap().seq, MAX_LINES as u64 + 1);
    }

    #[test]
    fn le_o_output_log_com_xml_texto_e_acentos() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("output.log");
        let mut bytes = Vec::new();
        bytes.extend_from_slice("[12:00:00] [main/INFO] [teste/]: Ação e coração\r\n".as_bytes());
        bytes.extend_from_slice(
            b"<log4j:Event logger=\"teste\" timestamp=\"1\" level=\"WARN\" thread=\"Render thread\">\n",
        );
        bytes.extend_from_slice(
            "  <log4j:Message><![CDATA[Configuração inválida]]></log4j:Message>\n".as_bytes(),
        );
        bytes.extend_from_slice(b"</log4j:Event>\n");
        // Uma linha em Windows-1252 (Java 8 no Windows): "pé" com 0xE9.
        bytes.extend_from_slice(b"Exce\xe7\xe3o no p\xe9\n");
        std::fs::write(&path, bytes).unwrap();
        let (lines, truncated) = read_output_log(&path).unwrap();
        assert!(!truncated);
        let texts: Vec<_> = lines.iter().map(|l| l.text.as_str()).collect();
        assert_eq!(
            texts,
            vec!["Ação e coração", "Configuração inválida", "Exceção no pé"]
        );
        assert_eq!(lines[1].level, Some(ConsoleLevel::Warn));
        assert_eq!(lines[2].seq, 2);
        assert!(
            read_output_log(&dir.path().join("nao-existe"))
                .unwrap()
                .0
                .is_empty()
        );
    }
}
