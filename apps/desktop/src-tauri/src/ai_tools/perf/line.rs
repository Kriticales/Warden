//! Uma linha de `perf/<pack-id>.jsonl` (gravada por `test_hooks::perf`; ARCHITECTURE §21.6) e a
//! leitura do arquivo. Os nomes dos campos são o contrato com o gravador.

use std::path::Path;

use serde::Deserialize;
use warden_launcher::outcome::Outcome;

use crate::test_session::TestMode;

/// Quantas linhas, no máximo, são lidas (as mais novas): o arquivo cresce um pouco por teste.
const MAX_LINES: usize = 2000;

/// O perfil do teste na linha.
#[derive(Debug, Clone, Default, Deserialize)]
pub(super) struct LineProfile {
    #[serde(default)]
    pub(super) id: String,
    /// Nome do perfil (os perfis com nome; o Padrão fica sem).
    #[serde(default)]
    pub(super) nome: Option<String>,
    /// Hash dos ajustes do perfil (memória, Java, argumentos, janela, "ao abrir").
    #[serde(default)]
    pub(super) assinatura: Option<String>,
}

/// Um teste no histórico de desempenho.
#[derive(Debug, Clone, Deserialize)]
pub(super) struct PerfLine {
    /// O id da sessão (o nome da pasta da sessão).
    pub(super) sessao: String,
    /// Quando o teste começou (RFC 3339, UTC).
    #[serde(default)]
    pub(super) data: String,
    pub(super) modo: TestMode,
    /// Tempo até o jogo carregar, em ms.
    #[serde(default)]
    pub(super) carregou_ms: Option<u32>,
    /// Tempo até entrar no mundo, em ms.
    #[serde(default)]
    pub(super) mundo_ms: Option<u32>,
    /// Pico de RAM do processo do jogo, em MB.
    #[serde(default)]
    pub(super) ram_pico_mb: Option<u32>,
    /// Pico da memória do jogo depois das coletas, em MB.
    #[serde(default)]
    pub(super) heap_pico_mb: Option<u32>,
    #[serde(default)]
    pub(super) perfil: LineProfile,
    /// A impressão do computador (`sha256:…`, só local).
    #[serde(default)]
    pub(super) computador: Option<String>,
    /// O hash da árvore do pack testada.
    #[serde(default)]
    pub(super) arvore: Option<String>,
    #[serde(default)]
    pub(super) primeira_abertura: bool,
    pub(super) resultado: Outcome,
}

/// Lê as linhas do arquivo, da mais antiga para a mais nova. Sem arquivo, vazio; linha
/// ilegível é ignorada.
pub(super) fn read_lines(file: &Path) -> Vec<PerfLine> {
    let Ok(bytes) = std::fs::read(file) else {
        return Vec::new();
    };
    parse_lines(&String::from_utf8_lossy(&bytes))
}

/// [`read_lines`] sobre o texto.
pub(super) fn parse_lines(text: &str) -> Vec<PerfLine> {
    let mut lines: Vec<PerfLine> = text
        .lines()
        .filter_map(|line| serde_json::from_str::<PerfLine>(line.trim()).ok())
        .filter(|line| !line.sessao.is_empty())
        .collect();
    if lines.len() > MAX_LINES {
        lines.drain(..lines.len() - MAX_LINES);
    }
    lines
}
