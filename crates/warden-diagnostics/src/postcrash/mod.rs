//! Análise pós-crash (ARCHITECTURE §9.3; SPEC T14, CA-T14-02).
//!
//! Etapas:
//! 1. **Coleta** ([`collect_session`]): `output.log` da sessão, `logs/latest.log`,
//!    `logs/debug.log` (ou `logs/fml-client-latest.log` no Forge antigo), os crash reports e os
//!    `hs_err_pid*.log` gravados depois do início da sessão.
//! 2. **Limpeza** (`text`): decodificação linha a linha, códigos `§x` e ANSI, XML do log4j.
//! 3. **Padrões** do catálogo `data/log-patterns.toml` (`catalog`), com o loader e a versão do
//!    contexto ou do próprio log (`context`), a causa mais profunda e os mods citados na pilha
//!    (`stack`).
//! 4. **Evidência** = arquivo + linha + trecho em todo achado; achados iguais (mesma regra e
//!    mesma chave) viram um só com as evidências juntas; **ordem** do mais provável para o
//!    menos: gravidade, depois crash report > tela de erro do loader > exceção > genérico, depois
//!    o peso do padrão.
//!
//! Gancho 1.1 (ADR-0039): [`analyze_text`] aceita qualquer texto de log, sem pasta de sessão
//! (log escolhido no computador para a IA; travamento de um jogador).
//!
//! Limites: a análise só lê. Ligar ids e jars aos itens do pack, a assinatura do travamento
//! (D-06) e o índice config de mixin → mod (D-05) ficam com quem chama.

mod analyze;
mod catalog;
mod collect;
mod context;
mod stack;
mod text;

use std::path::Path;

use serde::{Deserialize, Serialize};

pub use analyze::{FROZE_PATTERN, MAX_EVIDENCE, analyze, analyze_text};
pub use collect::{MTIME_TOLERANCE, SessionFiles, analyze_session, collect_session};
pub use text::{HEAD_BYTES, TAIL_BYTES, strip_formatting};

pub(crate) use text::fixed_regex as text_fixed_regex;

use crate::error::{DiagnosticsError, Result};
use crate::model::Finding;
use text::RawLine;

/// Loader que o log menciona (escopo dos padrões).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, specta::Type,
)]
#[serde(rename_all = "lowercase")]
pub enum LogLoader {
    /// Fabric.
    Fabric,
    /// Quilt.
    Quilt,
    /// Forge (todas as versões).
    Forge,
    /// NeoForge.
    #[serde(rename = "neoforge")]
    NeoForge,
}

impl LogLoader {
    /// O loader do `pack.toml`, quando é um dos que o diagnóstico conhece.
    #[must_use]
    pub fn from_pack(loader: warden_packwiz::Loader) -> Option<Self> {
        match loader {
            warden_packwiz::Loader::Fabric => Some(Self::Fabric),
            warden_packwiz::Loader::Quilt => Some(Self::Quilt),
            warden_packwiz::Loader::Forge => Some(Self::Forge),
            warden_packwiz::Loader::NeoForge => Some(Self::NeoForge),
            warden_packwiz::Loader::LiteLoader => None,
        }
    }
}

/// Tipo de arquivo analisado. A ordem é a preferência da evidência principal quando o mesmo
/// achado aparece em mais de um arquivo.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, specta::Type,
)]
#[serde(rename_all = "camelCase")]
pub enum SourceKind {
    /// `crash-reports/crash-*.txt`.
    CrashReport,
    /// `hs_err_pid*.log` (queda nativa da JVM).
    JvmCrash,
    /// `logs/latest.log`.
    LatestLog,
    /// `output.log` da sessão (tudo o que saiu dos pipes, inclusive antes do log4j).
    Output,
    /// `logs/debug.log` ou `logs/fml-client-latest.log`.
    DebugLog,
    /// Texto solto (log escolhido no computador, de um jogador…).
    Text,
}

impl SourceKind {
    /// O arquivo inteiro é um relatório (crash report ou `hs_err)`: toda linha conta como
    /// "dentro de crash report".
    fn is_report(self) -> bool {
        matches!(self, Self::CrashReport | Self::JvmCrash)
    }
}

/// Um arquivo (ou texto) a analisar, já decodificado em linhas numeradas.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogSource {
    name: String,
    kind: SourceKind,
    lines: Vec<RawLine>,
}

impl LogSource {
    /// Texto já em memória. `name` é o que aparece na evidência (nunca um caminho absoluto).
    #[must_use]
    pub fn from_text(name: impl Into<String>, kind: SourceKind, text: &str) -> Self {
        Self::from_bytes(name, kind, text.as_bytes())
    }

    /// Bytes em memória, decodificados linha a linha (UTF-8 ou Windows-1252).
    #[must_use]
    pub fn from_bytes(name: impl Into<String>, kind: SourceKind, bytes: &[u8]) -> Self {
        Self {
            name: name.into(),
            kind,
            lines: text::lines_from_bytes(bytes),
        }
    }

    /// Lê um arquivo com o limite de tamanho (começo e fim de arquivos enormes, numeração
    /// preservada).
    pub fn read(path: &Path, name: impl Into<String>, kind: SourceKind) -> Result<Self> {
        let lines =
            text::read_lines_capped(path).map_err(|error| DiagnosticsError::io(path, error))?;
        Ok(Self {
            name: name.into(),
            kind,
            lines,
        })
    }

    /// Nome mostrado na evidência.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Tipo do arquivo.
    #[must_use]
    pub fn kind(&self) -> SourceKind {
        self.kind
    }

    /// Quantas linhas foram lidas.
    #[must_use]
    pub fn line_count(&self) -> usize {
        self.lines.len()
    }
}

/// O que se sabe do pack testado (opcional). Sem isso, o loader e a versão vêm do log.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AnalysisContext {
    /// Loader do pack.
    pub loader: Option<LogLoader>,
    /// Versão do Minecraft do pack.
    pub minecraft: Option<String>,
}

/// Como a sessão terminou, segundo o supervisor do processo (L-04).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SessionOutcome {
    /// Código de saída do processo, quando saiu sozinho.
    pub exit_code: Option<i32>,
    /// O usuário pediu para parar.
    pub stopped_by_user: bool,
    /// O supervisor viu o jogo parado (sem linha nova de log) e o encerrou. Vários travamentos
    /// do NeoForge não escrevem nada (S-R5-3 §5.2): o achado aponta a última linha escrita.
    pub froze: bool,
}

/// Um achado da análise com o id do padrão do catálogo que o produziu (entra na assinatura do
/// travamento, D-06).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct PostCrashFinding {
    /// Id do padrão em `log-patterns.toml` (ou `session.froze` para o jogo parado).
    pub pattern: String,
    /// O achado.
    pub finding: Finding,
}

/// Arquivo analisado.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct AnalyzedSource {
    /// Nome mostrado na evidência.
    pub name: String,
    /// Tipo.
    pub kind: SourceKind,
    /// Linhas lidas.
    pub lines: u32,
}

/// Resultado da análise.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct Analysis {
    /// Achados, o mais provável primeiro.
    pub findings: Vec<PostCrashFinding>,
    /// Loader (do contexto ou do log).
    pub loader: Option<LogLoader>,
    /// Versão do loader, se o log disser.
    pub loader_version: Option<String>,
    /// Versão do Minecraft (do contexto ou do log).
    pub minecraft: Option<String>,
    /// Arquivos analisados.
    pub sources: Vec<AnalyzedSource>,
}

impl Analysis {
    /// O achado principal (o mais provável).
    #[must_use]
    pub fn primary(&self) -> Option<&PostCrashFinding> {
        self.findings.first()
    }

    /// Se a análise achou uma causa (algum achado de erro). Sem causa, a interface mostra
    /// "Não encontramos a causa automaticamente." (SPEC T14).
    #[must_use]
    pub fn found_cause(&self) -> bool {
        self.findings
            .iter()
            .any(|entry| entry.finding.severity() == crate::model::Severity::Error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loader_do_pack_e_ordem_das_fontes() {
        assert_eq!(
            LogLoader::from_pack(warden_packwiz::Loader::NeoForge),
            Some(LogLoader::NeoForge)
        );
        assert_eq!(
            LogLoader::from_pack(warden_packwiz::Loader::Fabric),
            Some(LogLoader::Fabric)
        );
        assert_eq!(
            LogLoader::from_pack(warden_packwiz::Loader::Quilt),
            Some(LogLoader::Quilt)
        );
        assert_eq!(
            LogLoader::from_pack(warden_packwiz::Loader::Forge),
            Some(LogLoader::Forge)
        );
        assert_eq!(
            LogLoader::from_pack(warden_packwiz::Loader::LiteLoader),
            None
        );
        assert!(SourceKind::CrashReport < SourceKind::LatestLog);
        assert!(SourceKind::LatestLog < SourceKind::Output);
        assert!(SourceKind::JvmCrash.is_report());
        assert!(!SourceKind::Text.is_report());
        assert_eq!(
            serde_json::to_value(LogLoader::NeoForge).unwrap(),
            "neoforge"
        );
    }

    #[test]
    fn fonte_le_texto_e_arquivo() {
        let source = LogSource::from_text("a.log", SourceKind::Text, "um\ndois\n");
        assert_eq!(source.name(), "a.log");
        assert_eq!(source.kind(), SourceKind::Text);
        assert_eq!(source.line_count(), 2);
        let missing =
            LogSource::read(Path::new("nao-existe.log"), "x", SourceKind::Text).unwrap_err();
        assert!(matches!(missing, DiagnosticsError::Io { .. }));
    }
}
