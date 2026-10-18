//! Nota de saúde do pack (D-08; SPEC T14 "Saúde do pack"; ADR-0034; R5A §6.2; decisão D24).
//!
//! [`score`] é uma função pura: recebe o que o Warden já sabe do pack ([`HealthInput`]: os
//! achados do último relatório com a marca "ignorado por você", as sessões de teste gravadas, o
//! resumo do pack agora e as datas das versões mais novas) e devolve a nota de 0 a 100, a faixa e
//! "O que tirou pontos" ([`HealthLine`]), uma linha por categoria na ordem da tabela da SPEC
//! ([`HealthCategory::ORDER`]). Os pesos, os tetos e os limites vêm de
//! `data/health-score.toml` ([`weights`]).
//!
//! Regras que valem sempre (testes de propriedade):
//! - a nota fica entre 0 e 100;
//! - um achado a mais nunca aumenta a nota, e um a menos nunca a diminui;
//! - um achado ignorado não tira pontos e aparece numa linha à parte, marcada
//!   ([`HealthLine::ignored`]), com os pontos que tiraria.
//!
//! Cada achado conta numa categoria só: o aviso de mixin (`W_MIXIN_OVERLAP`) em "Mods que
//! alteram o mesmo ponto do jogo", o download bloqueado da CurseForge em "Download pelos
//! jogadores", a versão alfa ou beta e o projeto arquivado em "Manutenção"; o resto, em
//! "Problemas encontrados".

use std::collections::BTreeSet;
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

use crate::crash_history::{SessionEnd, session_order};
use crate::model::{Evidence, Finding, Severity};
use crate::pretest::mixin_overlap::RULE as MIXIN_RULE;
use crate::{DiagnosticsError, Result};

/// Mod da CurseForge que os jogadores não baixam sozinhos.
const DOWNLOAD_BLOCKED_RULE: &str = "W_DOWNLOAD_BLOCKED";
/// Versão alfa ou beta.
const PRERELEASE_RULE: &str = "I_PRERELEASE";
/// Mod obsoleto (lista curada) ou projeto arquivado (campo `status` da API).
const OBSOLETE_RULE: &str = "W_OBSOLETE";
/// Campo da API que marca o projeto arquivado.
const ARCHIVED_FIELD: &str = "status";
/// Um mês médio, em milissegundos (365,25 / 12 dias).
const MONTH_MS: u64 = 2_629_800_000;

// ---------------------------------------------------------------------------------------------
// Pesos (`data/health-score.toml`)
// ---------------------------------------------------------------------------------------------

/// As faixas da nota (também na saída, para o rodapé de "O que tirou pontos").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(deny_unknown_fields)]
pub struct Bands {
    /// A partir daqui, Ótimo.
    pub otimo: u8,
    /// A partir daqui, Bom.
    pub bom: u8,
    /// A partir daqui, Atenção; abaixo, Crítico.
    pub atencao: u8,
}

/// "Problemas encontrados".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProblemWeights {
    /// Por erro.
    pub erro: u32,
    /// Por aviso.
    pub aviso: u32,
    /// Teto da categoria.
    pub teto: u32,
}

/// "Último teste".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LastTestWeights {
    /// O último teste travou.
    pub travou: u32,
    /// O pack nunca foi testado.
    pub nunca_testado: u32,
    /// O pack mudou depois do último teste.
    pub alterado_depois: u32,
    /// Teto da categoria.
    pub teto: u32,
}

/// "Travamentos recentes".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CrashWeights {
    /// Por causa diferente.
    pub por_causa: u32,
    /// Quantas sessões, das mais novas, contam.
    pub sessoes: u32,
    /// Teto da categoria.
    pub teto: u32,
}

/// "Mods que alteram o mesmo ponto do jogo".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MixinWeights {
    /// Por sobreposição de risco alto.
    pub alto: u32,
    /// Por sobreposição de risco médio.
    pub medio: u32,
    /// Teto da categoria.
    pub teto: u32,
}

/// "Manutenção".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MaintenanceWeights {
    /// Por projeto arquivado.
    pub arquivado: u32,
    /// Por versão alfa ou beta.
    pub alfa_ou_beta: u32,
    /// Por mod sem versão nova há mais de [`Self::meses_sem_versao_nova`] meses.
    pub sem_versao_nova: u32,
    /// Meses sem versão nova para a versão do pack.
    pub meses_sem_versao_nova: u32,
    /// Teto da categoria.
    pub teto: u32,
}

/// "Download pelos jogadores".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DownloadWeights {
    /// Por mod que os jogadores não baixam sozinhos.
    pub por_mod: u32,
    /// Teto da categoria.
    pub teto: u32,
}

/// "Desempenho".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PerformanceWeights {
    /// O último teste carregou bem mais devagar que o último teste bom.
    pub carregamento_lento: u32,
    /// Quanto mais devagar, em porcentagem.
    pub percentual_mais_lento: u32,
    /// A memória do teste ficou abaixo da sugestão.
    pub memoria_abaixo: u32,
    /// Teto da categoria.
    pub teto: u32,
}

/// O conteúdo de `data/health-score.toml`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Weights {
    /// As faixas.
    pub faixas: Bands,
    /// "Problemas encontrados".
    pub problemas: ProblemWeights,
    /// "Último teste".
    pub ultimo_teste: LastTestWeights,
    /// "Travamentos recentes".
    pub travamentos: CrashWeights,
    /// "Mods que alteram o mesmo ponto do jogo".
    pub mixins: MixinWeights,
    /// "Manutenção".
    pub manutencao: MaintenanceWeights,
    /// "Download pelos jogadores".
    pub download: DownloadWeights,
    /// "Desempenho".
    pub desempenho: PerformanceWeights,
}

impl Weights {
    /// O teto de uma categoria.
    #[must_use]
    pub fn cap(&self, category: HealthCategory) -> u32 {
        match category {
            HealthCategory::Problems => self.problemas.teto,
            HealthCategory::LastTest => self.ultimo_teste.teto,
            HealthCategory::Crashes => self.travamentos.teto,
            HealthCategory::Mixins => self.mixins.teto,
            HealthCategory::Maintenance => self.manutencao.teto,
            HealthCategory::Download => self.download.teto,
            HealthCategory::Performance => self.desempenho.teto,
        }
    }
}

/// Lê e valida um `health-score.toml`.
///
/// # Errors
///
/// TOML inválido, campo desconhecido ou ausente, faixas fora de ordem (`0 < atencao < bom <
/// otimo <= 100`), teto zero, nenhuma sessão de travamentos, porcentagem ou meses zero.
pub fn parse_weights(text: &str) -> std::result::Result<Weights, String> {
    let weights: Weights = toml::from_str(text).map_err(|e| e.to_string())?;
    let bands = weights.faixas;
    if !(0 < bands.atencao && bands.atencao < bands.bom && bands.bom < bands.otimo)
        || bands.otimo > 100
    {
        return Err(format!(
            "faixas fora de ordem: atencao {}, bom {}, otimo {}",
            bands.atencao, bands.bom, bands.otimo
        ));
    }
    if let Some(category) = HealthCategory::ORDER
        .into_iter()
        .find(|category| weights.cap(*category) == 0)
    {
        return Err(format!("teto zero em {category:?}"));
    }
    if weights.travamentos.sessoes == 0 {
        return Err("travamentos.sessoes precisa ser maior que zero".into());
    }
    if weights.desempenho.percentual_mais_lento == 0 {
        return Err("desempenho.percentual_mais_lento precisa ser maior que zero".into());
    }
    if weights.manutencao.meses_sem_versao_nova == 0 {
        return Err("manutencao.meses_sem_versao_nova precisa ser maior que zero".into());
    }
    Ok(weights)
}

/// Os pesos embutidos (validados pelos testes).
///
/// # Errors
///
/// [`DiagnosticsError::Internal`] se o arquivo embutido for inválido (falha de desenvolvimento).
pub fn weights() -> Result<&'static Weights> {
    static WEIGHTS: OnceLock<std::result::Result<Weights, String>> = OnceLock::new();
    WEIGHTS
        .get_or_init(|| {
            parse_weights(include_str!("../data/health-score.toml"))
                .map_err(|e| format!("health-score.toml: {e}"))
        })
        .as_ref()
        .map_err(|error| DiagnosticsError::Internal(error.clone()))
}

// ---------------------------------------------------------------------------------------------
// Entrada
// ---------------------------------------------------------------------------------------------

/// O risco de um aviso de mixin, depois dos rebaixadores.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum MixinRisk {
    /// Alto.
    High,
    /// Médio.
    Medium,
}

/// Um achado do último relatório, como a nota o vê.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HealthFinding {
    /// Identificador estável no pack (o que "Ignorar neste pack" grava).
    pub id: String,
    /// Código da regra.
    pub rule: String,
    /// Gravidade.
    pub severity: Severity,
    /// Ignorado por você neste pack.
    pub ignored: bool,
    /// O primeiro item do achado que está no pack (caminho), para o link.
    pub item: Option<String>,
    /// Risco do aviso de mixin.
    pub risk: Option<MixinRisk>,
    /// Aviso de mod obsoleto que vem do projeto arquivado na fonte.
    pub archived: bool,
}

impl HealthFinding {
    /// O achado de um relatório.
    #[must_use]
    pub fn new(
        id: impl Into<String>,
        finding: &Finding,
        ignored: bool,
        item: Option<String>,
    ) -> Self {
        let rule = finding.rule().as_str();
        let risk = (rule == MIXIN_RULE).then(|| match finding.param("risco") {
            Some("alto") => MixinRisk::High,
            _ => MixinRisk::Medium,
        });
        let archived = rule == OBSOLETE_RULE
            && finding.evidence().iter().any(
                |evidence| matches!(evidence, Evidence::Api { field, .. } if field == ARCHIVED_FIELD),
            );
        Self {
            id: id.into(),
            rule: rule.to_owned(),
            severity: finding.severity(),
            ignored,
            item,
            risk,
            archived,
        }
    }
}

/// A memória de um teste.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionMemory {
    /// `-Xmx`, em MB.
    pub mb: u32,
    /// Automática (escolhida pelo Warden).
    pub auto: bool,
    /// A sugestão do Warden para a quantidade de mods do teste, em MB.
    pub suggested_mb: u32,
}

/// Uma sessão de teste gravada.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HealthSession {
    /// Nome da pasta da sessão.
    pub id: String,
    /// Início, em milissegundos desde 1970.
    pub started_at_ms: u64,
    /// Como terminou.
    pub end: SessionEnd,
    /// Hash da assinatura do travamento (só nas que travaram).
    pub signature: Option<String>,
    /// Resumo da árvore do pack testada.
    pub tree: Option<String>,
    /// Tempo até carregar, quando carregou.
    pub loaded_ms: Option<u32>,
    /// A memória do teste.
    pub memory: Option<SessionMemory>,
}

/// A versão mais nova de um mod para a versão do pack, com a data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LatestRelease {
    /// Caminho do item no pack.
    pub item: String,
    /// Nome do item.
    pub name: String,
    /// Publicação da versão mais nova, em milissegundos desde 1970.
    pub published_ms: u64,
}

/// O que a nota usa.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HealthInput {
    /// Os achados do último relatório do pré-teste.
    pub findings: Vec<HealthFinding>,
    /// As sessões de teste gravadas, em qualquer ordem.
    pub sessions: Vec<HealthSession>,
    /// Resumo da árvore do pack agora (`None`: não deu para ler).
    pub current_tree: Option<String>,
    /// As versões mais novas conhecidas (do último "Verificar atualizações").
    pub releases: Vec<LatestRelease>,
    /// Agora, em milissegundos desde 1970.
    pub now_ms: u64,
}

// ---------------------------------------------------------------------------------------------
// Saída
// ---------------------------------------------------------------------------------------------

/// A faixa da nota.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum HealthBand {
    /// Ótimo.
    Great,
    /// Bom.
    Good,
    /// Atenção.
    Attention,
    /// Crítico.
    Critical,
}

impl HealthBand {
    /// A faixa de uma nota.
    #[must_use]
    pub fn of(score: u8, bands: &Bands) -> Self {
        if score >= bands.otimo {
            Self::Great
        } else if score >= bands.bom {
            Self::Good
        } else if score >= bands.atencao {
            Self::Attention
        } else {
            Self::Critical
        }
    }
}

/// Uma categoria de "O que tirou pontos".
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, specta::Type,
)]
#[serde(rename_all = "camelCase")]
pub enum HealthCategory {
    /// "Problemas encontrados".
    Problems,
    /// "Último teste".
    LastTest,
    /// "Travamentos recentes".
    Crashes,
    /// "Mods que alteram o mesmo ponto do jogo".
    Mixins,
    /// "Manutenção".
    Maintenance,
    /// "Download pelos jogadores".
    Download,
    /// "Desempenho".
    Performance,
}

impl HealthCategory {
    /// A ordem da tabela da SPEC T14.
    pub const ORDER: [Self; 7] = [
        Self::Problems,
        Self::LastTest,
        Self::Crashes,
        Self::Mixins,
        Self::Maintenance,
        Self::Download,
        Self::Performance,
    ];
}

/// O estado do último teste que tirou pontos.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum LastTestState {
    /// O último teste travou.
    Crashed,
    /// O pack nunca foi testado.
    NeverTested,
    /// O pack mudou depois do último teste.
    ChangedSince,
}

/// Um motivo dentro de uma linha de "O que tirou pontos".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum HealthReason {
    /// Um achado do pré-teste.
    #[serde(rename_all = "camelCase")]
    Finding {
        /// O id do achado no relatório.
        finding_id: String,
        /// Código da regra.
        rule: String,
        /// Gravidade.
        severity: Severity,
        /// Risco do aviso de mixin.
        risk: Option<MixinRisk>,
        /// O item (caminho no pack), para o link.
        item: Option<String>,
        /// Pontos que tira (ou tiraria, se ignorado).
        points: u32,
    },
    /// Mod sem versão nova há meses.
    #[serde(rename_all = "camelCase")]
    NoNewVersion {
        /// O item (caminho no pack).
        item: String,
        /// Nome do item.
        name: String,
        /// Meses desde a versão mais nova.
        months: u32,
        /// Pontos que tira.
        points: u32,
    },
    /// O último teste.
    #[serde(rename_all = "camelCase")]
    LastTest {
        /// O estado.
        state: LastTestState,
        /// A sessão (nenhuma quando nunca testado).
        session: Option<String>,
        /// Pontos que tira.
        points: u32,
    },
    /// Uma causa de travamento recente.
    #[serde(rename_all = "camelCase")]
    Crash {
        /// Hash da assinatura.
        signature: String,
        /// A sessão mais nova com essa causa.
        session: String,
        /// Pontos que tira.
        points: u32,
    },
    /// O último teste carregou bem mais devagar que o último teste bom.
    #[serde(rename_all = "camelCase")]
    SlowLoad {
        /// A sessão.
        session: String,
        /// Quanto levou.
        loaded_ms: u32,
        /// O último teste bom.
        good_session: String,
        /// Quanto ele levou.
        good_ms: u32,
        /// Pontos que tira.
        points: u32,
    },
    /// A memória do último teste ficou abaixo da sugestão.
    #[serde(rename_all = "camelCase")]
    LowMemory {
        /// A sessão.
        session: String,
        /// A memória do teste, em MB.
        memory_mb: u32,
        /// A sugestão, em MB.
        suggested_mb: u32,
        /// Pontos que tira.
        points: u32,
    },
}

impl HealthReason {
    /// Os pontos do motivo.
    #[must_use]
    pub fn points(&self) -> u32 {
        match self {
            Self::Finding { points, .. }
            | Self::NoNewVersion { points, .. }
            | Self::LastTest { points, .. }
            | Self::Crash { points, .. }
            | Self::SlowLoad { points, .. }
            | Self::LowMemory { points, .. } => *points,
        }
    }
}

/// Uma linha de "O que tirou pontos".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct HealthLine {
    /// A categoria.
    pub category: HealthCategory,
    /// Pontos tirados (já com o teto; zero na linha dos ignorados).
    pub points: u32,
    /// A soma dos motivos, antes do teto (na linha dos ignorados, o que tirariam).
    pub raw_points: u32,
    /// Linha dos itens ignorados por você: aparece riscada e não tira pontos.
    pub ignored: bool,
    /// Os motivos, na ordem do relatório.
    pub reasons: Vec<HealthReason>,
}

/// A nota do pack.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct HealthScore {
    /// De 0 a 100.
    pub score: u8,
    /// A faixa.
    pub band: HealthBand,
    /// "O que tirou pontos", na ordem da tabela; em cada categoria, a linha que conta antes da
    /// dos ignorados.
    pub lines: Vec<HealthLine>,
    /// As faixas usadas.
    pub bands: Bands,
    /// Quantas sessões, das mais novas, contam em "Travamentos recentes".
    pub recent_sessions: u32,
}

// ---------------------------------------------------------------------------------------------
// Cálculo
// ---------------------------------------------------------------------------------------------

/// Calcula a nota.
#[must_use]
pub fn score(input: &HealthInput, weights: &Weights) -> HealthScore {
    let mut counted: Vec<Vec<HealthReason>> = vec![Vec::new(); HealthCategory::ORDER.len()];
    let mut ignored: Vec<Vec<HealthReason>> = vec![Vec::new(); HealthCategory::ORDER.len()];
    let slot = |category: HealthCategory| {
        HealthCategory::ORDER
            .iter()
            .position(|other| *other == category)
            .unwrap_or_default()
    };

    let mut seen = BTreeSet::new();
    for finding in &input.findings {
        if !seen.insert(finding.id.as_str()) {
            continue;
        }
        let Some((category, points)) = finding_points(finding, weights) else {
            continue;
        };
        let reason = HealthReason::Finding {
            finding_id: finding.id.clone(),
            rule: finding.rule.clone(),
            severity: finding.severity,
            risk: finding.risk,
            item: finding.item.clone(),
            points,
        };
        if finding.ignored {
            ignored[slot(category)].push(reason);
        } else {
            counted[slot(category)].push(reason);
        }
    }
    counted[slot(HealthCategory::Maintenance)].extend(no_new_version(input, weights));

    let mut sessions: Vec<&HealthSession> = input.sessions.iter().collect();
    sessions.sort_by(|a, b| session_order(b.started_at_ms, &b.id, a.started_at_ms, &a.id));
    counted[slot(HealthCategory::LastTest)].extend(last_test(&sessions, input, weights));
    counted[slot(HealthCategory::Crashes)].extend(recent_crashes(&sessions, weights));
    counted[slot(HealthCategory::Performance)].extend(performance(&sessions, weights));

    let mut lines = Vec::new();
    for (index, category) in HealthCategory::ORDER.into_iter().enumerate() {
        let reasons = std::mem::take(&mut counted[index]);
        if !reasons.is_empty() {
            let raw: u32 = reasons.iter().map(HealthReason::points).sum();
            lines.push(HealthLine {
                category,
                points: raw.min(weights.cap(category)),
                raw_points: raw,
                ignored: false,
                reasons,
            });
        }
        let reasons = std::mem::take(&mut ignored[index]);
        if !reasons.is_empty() {
            lines.push(HealthLine {
                category,
                points: 0,
                raw_points: reasons.iter().map(HealthReason::points).sum(),
                ignored: true,
                reasons,
            });
        }
    }
    let lost: u32 = lines.iter().map(|line| line.points).sum();
    let score = u8::try_from(100_u32.saturating_sub(lost)).unwrap_or(0);
    HealthScore {
        score,
        band: HealthBand::of(score, &weights.faixas),
        lines,
        bands: weights.faixas,
        recent_sessions: weights.travamentos.sessoes,
    }
}

/// A categoria e os pontos de um achado (`None`: não tira pontos, como as informações).
fn finding_points(finding: &HealthFinding, weights: &Weights) -> Option<(HealthCategory, u32)> {
    let (category, points) = match finding.rule.as_str() {
        rule if rule == MIXIN_RULE => (
            HealthCategory::Mixins,
            match finding.risk {
                Some(MixinRisk::High) => weights.mixins.alto,
                _ => weights.mixins.medio,
            },
        ),
        DOWNLOAD_BLOCKED_RULE => (HealthCategory::Download, weights.download.por_mod),
        PRERELEASE_RULE => (HealthCategory::Maintenance, weights.manutencao.alfa_ou_beta),
        OBSOLETE_RULE if finding.archived => {
            (HealthCategory::Maintenance, weights.manutencao.arquivado)
        }
        _ => (
            HealthCategory::Problems,
            match finding.severity {
                Severity::Error => weights.problemas.erro,
                Severity::Warning => weights.problemas.aviso,
                Severity::Info => 0,
            },
        ),
    };
    (points > 0).then_some((category, points))
}

/// Os mods cuja versão mais nova para o pack passou do limite de meses (um por item).
fn no_new_version(input: &HealthInput, weights: &Weights) -> Vec<HealthReason> {
    let limit = u64::from(weights.manutencao.meses_sem_versao_nova).saturating_mul(MONTH_MS);
    let points = weights.manutencao.sem_versao_nova;
    if points == 0 {
        return Vec::new();
    }
    let mut seen = BTreeSet::new();
    input
        .releases
        .iter()
        .filter(|release| seen.insert(release.item.as_str()))
        .filter_map(|release| {
            let elapsed = input.now_ms.checked_sub(release.published_ms)?;
            (elapsed > limit).then(|| HealthReason::NoNewVersion {
                item: release.item.clone(),
                name: release.name.clone(),
                months: u32::try_from(elapsed / MONTH_MS).unwrap_or(u32::MAX),
                points,
            })
        })
        .collect()
}

/// O último teste: travou, nunca testado ou alterado depois (só o mais grave).
fn last_test(
    sessions: &[&HealthSession],
    input: &HealthInput,
    weights: &Weights,
) -> Option<HealthReason> {
    let w = weights.ultimo_teste;
    let (state, session, points) = match sessions.first() {
        None => (LastTestState::NeverTested, None, w.nunca_testado),
        Some(last) if last.end == SessionEnd::Crashed => {
            (LastTestState::Crashed, Some(last.id.clone()), w.travou)
        }
        Some(last) => {
            let (Some(now), Some(tested)) = (&input.current_tree, &last.tree) else {
                return None;
            };
            if now == tested {
                return None;
            }
            (
                LastTestState::ChangedSince,
                Some(last.id.clone()),
                w.alterado_depois,
            )
        }
    };
    (points > 0).then_some(HealthReason::LastTest {
        state,
        session,
        points,
    })
}

/// Cada causa diferente nas últimas sessões, da mais nova para a mais antiga.
fn recent_crashes(sessions: &[&HealthSession], weights: &Weights) -> Vec<HealthReason> {
    let w = weights.travamentos;
    if w.por_causa == 0 {
        return Vec::new();
    }
    let window = usize::try_from(w.sessoes).unwrap_or(usize::MAX);
    let mut seen = BTreeSet::new();
    sessions
        .iter()
        .take(window)
        .filter(|session| session.end == SessionEnd::Crashed)
        .filter_map(|session| {
            let signature = session.signature.as_deref()?;
            seen.insert(signature).then(|| HealthReason::Crash {
                signature: signature.to_owned(),
                session: session.id.clone(),
                points: w.por_causa,
            })
        })
        .collect()
}

/// Carregamento lento e memória abaixo da sugestão, no último teste.
fn performance(sessions: &[&HealthSession], weights: &Weights) -> Vec<HealthReason> {
    let w = weights.desempenho;
    let mut reasons = Vec::new();
    let Some(last) = sessions.first() else {
        return reasons;
    };
    if let Some(loaded) = last.loaded_ms
        && w.carregamento_lento > 0
        && let Some(good) = sessions.iter().skip(1).find(|session| {
            session.end == SessionEnd::ClosedNormally && session.loaded_ms.is_some()
        })
        && let Some(good_ms) = good.loaded_ms
        && u64::from(loaded) * 100 > u64::from(good_ms) * (100 + u64::from(w.percentual_mais_lento))
    {
        reasons.push(HealthReason::SlowLoad {
            session: last.id.clone(),
            loaded_ms: loaded,
            good_session: good.id.clone(),
            good_ms,
            points: w.carregamento_lento,
        });
    }
    if let Some(memory) = last.memory
        && !memory.auto
        && memory.mb < memory.suggested_mb
        && w.memoria_abaixo > 0
    {
        reasons.push(HealthReason::LowMemory {
            session: last.id.clone(),
            memory_mb: memory.mb,
            suggested_mb: memory.suggested_mb,
            points: w.memoria_abaixo,
        });
    }
    reasons
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use proptest::prelude::*;

    use super::*;

    fn finding(id: &str, rule: &str, severity: Severity) -> HealthFinding {
        HealthFinding {
            id: id.into(),
            rule: rule.into(),
            severity,
            ignored: false,
            item: Some(format!("mods/{id}.pw.toml")),
            risk: (rule == MIXIN_RULE).then_some(MixinRisk::High),
            archived: false,
        }
    }

    fn session(id: &str, at: u64, end: SessionEnd, signature: Option<&str>) -> HealthSession {
        HealthSession {
            id: id.into(),
            started_at_ms: at,
            end,
            signature: signature.map(Into::into),
            tree: Some("sha256:agora".into()),
            loaded_ms: None,
            memory: None,
        }
    }

    /// O pack do CA-T14-07: 2 erros, 1 aviso, o último teste travado, 2 causas de travamento
    /// diferentes nas últimas 10 sessões, 1 sobreposição alta de mixin e 2 mods da CurseForge
    /// bloqueados.
    fn ca_t14_07() -> HealthInput {
        HealthInput {
            findings: vec![
                finding(
                    "E_MISSING_DEP|mods/waystones.pw.toml",
                    "E_MISSING_DEP",
                    Severity::Error,
                ),
                finding(
                    "E_EXCLUSIVE|mods/a.pw.toml|mods/b.pw.toml",
                    "E_EXCLUSIVE",
                    Severity::Error,
                ),
                finding("W_SIDE|mods/xaero.pw.toml", "W_SIDE", Severity::Warning),
                finding(
                    "W_MIXIN_OVERLAP|mods/c.pw.toml|mods/d.pw.toml",
                    MIXIN_RULE,
                    Severity::Warning,
                ),
                finding(
                    "W_DOWNLOAD_BLOCKED|mods/e.pw.toml",
                    DOWNLOAD_BLOCKED_RULE,
                    Severity::Warning,
                ),
                finding(
                    "W_DOWNLOAD_BLOCKED|mods/f.pw.toml",
                    DOWNLOAD_BLOCKED_RULE,
                    Severity::Warning,
                ),
                // Informação não tira pontos.
                finding(
                    "I_PACK_FORMAT|resourcepacks/g.pw.toml",
                    "I_PACK_FORMAT",
                    Severity::Info,
                ),
            ],
            sessions: vec![
                session(
                    "2026-10-10_10-00-00",
                    1_000,
                    SessionEnd::Crashed,
                    Some("aaaa"),
                ),
                session(
                    "2026-10-11_10-00-00",
                    2_000,
                    SessionEnd::ClosedNormally,
                    None,
                ),
                session(
                    "2026-10-12_10-00-00",
                    3_000,
                    SessionEnd::Crashed,
                    Some("bbbb"),
                ),
                session(
                    "2026-10-13_10-00-00",
                    4_000,
                    SessionEnd::Crashed,
                    Some("aaaa"),
                ),
            ],
            current_tree: Some("sha256:agora".into()),
            releases: Vec::new(),
            now_ms: 10_000,
        }
    }

    #[test]
    fn dados_embutidos_sao_validos() {
        let weights = weights().unwrap();
        assert_eq!(weights.faixas.otimo, 90);
        assert_eq!(weights.problemas.erro, 15);
    }

    #[test]
    fn dados_invalidos_sao_recusados() {
        let good = include_str!("../data/health-score.toml");
        assert!(parse_weights(&good.replace("otimo = 90", "otimo = 70")).is_err());
        assert!(parse_weights(&good.replace("teto = 40", "teto = 0")).is_err());
        assert!(parse_weights(&good.replace("sessoes = 10", "sessoes = 0")).is_err());
        assert!(parse_weights(&good.replace("erro = 15", "erro = 15\nnovo = 1")).is_err());
        assert!(parse_weights(&good.replace("percentual_mais_lento = 50", "")).is_err());
    }

    #[test]
    fn ca_t14_07_nota_33_critico_na_ordem_da_tabela() {
        let health = score(&ca_t14_07(), weights().unwrap());
        assert_eq!(health.score, 33);
        assert_eq!(health.band, HealthBand::Critical);
        let lines: Vec<(HealthCategory, u32)> = health
            .lines
            .iter()
            .map(|line| (line.category, line.points))
            .collect();
        assert_eq!(
            lines,
            [
                (HealthCategory::Problems, 33),
                (HealthCategory::LastTest, 20),
                (HealthCategory::Crashes, 6),
                (HealthCategory::Mixins, 4),
                (HealthCategory::Download, 4),
            ]
        );
        assert!(health.lines.iter().all(|line| !line.ignored));
        // A causa mais nova primeiro, com a sessão mais nova dela.
        assert_eq!(
            health.lines[2].reasons,
            [
                HealthReason::Crash {
                    signature: "aaaa".into(),
                    session: "2026-10-13_10-00-00".into(),
                    points: 3
                },
                HealthReason::Crash {
                    signature: "bbbb".into(),
                    session: "2026-10-12_10-00-00".into(),
                    points: 3
                },
            ]
        );
    }

    #[test]
    fn ignorar_um_aviso_devolve_os_pontos_e_fica_riscado() {
        let mut input = ca_t14_07();
        input.findings[2].ignored = true;
        let health = score(&input, weights().unwrap());
        assert_eq!(health.score, 36);
        let problems: Vec<&HealthLine> = health
            .lines
            .iter()
            .filter(|line| line.category == HealthCategory::Problems)
            .collect();
        assert_eq!(problems.len(), 2);
        assert_eq!((problems[0].points, problems[0].ignored), (30, false));
        assert_eq!(
            (
                problems[1].points,
                problems[1].raw_points,
                problems[1].ignored
            ),
            (0, 3, true)
        );
        assert!(matches!(
            &problems[1].reasons[..],
            [HealthReason::Finding { finding_id, .. }] if finding_id == "W_SIDE|mods/xaero.pw.toml"
        ));
    }

    #[test]
    fn tetos_por_categoria() {
        let mut input = HealthInput {
            current_tree: Some("sha256:agora".into()),
            sessions: vec![session("s", 1, SessionEnd::ClosedNormally, None)],
            ..HealthInput::default()
        };
        for n in 0..5 {
            input
                .findings
                .push(finding(&format!("E{n}"), "E_MISSING_DEP", Severity::Error));
            input
                .findings
                .push(finding(&format!("M{n}"), MIXIN_RULE, Severity::Warning));
            input.findings.push(finding(
                &format!("D{n}"),
                DOWNLOAD_BLOCKED_RULE,
                Severity::Warning,
            ));
            input.findings.push(finding(
                &format!("D{n}b"),
                DOWNLOAD_BLOCKED_RULE,
                Severity::Warning,
            ));
        }
        let health = score(&input, weights().unwrap());
        let line = |category| {
            health
                .lines
                .iter()
                .find(|line| line.category == category && !line.ignored)
                .map(|line| (line.points, line.raw_points))
                .unwrap()
        };
        assert_eq!(line(HealthCategory::Problems), (40, 75));
        assert_eq!(line(HealthCategory::Mixins), (10, 20));
        assert_eq!(line(HealthCategory::Download), (10, 20));
        assert_eq!(health.score, 40);
    }

    #[test]
    fn ultimo_teste_nunca_alterado_depois_e_sem_desconto() {
        let w = weights().unwrap();
        let mut input = HealthInput {
            current_tree: Some("sha256:agora".into()),
            ..HealthInput::default()
        };
        let health = score(&input, w);
        assert_eq!(health.score, 90);
        assert_eq!(
            health.lines[0].reasons,
            [HealthReason::LastTest {
                state: LastTestState::NeverTested,
                session: None,
                points: 10
            }]
        );

        input
            .sessions
            .push(session("antigo", 1, SessionEnd::Crashed, Some("x")));
        let mut ok = session("novo", 2, SessionEnd::ClosedNormally, None);
        ok.tree = Some("sha256:antes".into());
        input.sessions.push(ok);
        let health = score(&input, w);
        assert!(health.lines.iter().any(|line| line.reasons
            == [HealthReason::LastTest {
                state: LastTestState::ChangedSince,
                session: Some("novo".into()),
                points: 5
            }]));

        input.sessions[1].tree = Some("sha256:agora".into());
        let health = score(&input, w);
        // Só a causa do travamento antigo, nas últimas 10 sessões.
        assert_eq!(health.score, 97);
        assert_eq!(health.band, HealthBand::Great);

        // Sessão parada pelo usuário conta como o último teste, sem travamento.
        input
            .sessions
            .push(session("parado", 3, SessionEnd::Stopped, None));
        assert_eq!(score(&input, w).score, 97);
    }

    #[test]
    fn travamentos_so_das_ultimas_sessoes() {
        let w = weights().unwrap();
        let mut input = HealthInput {
            current_tree: Some("sha256:agora".into()),
            ..HealthInput::default()
        };
        input
            .sessions
            .push(session("velho", 0, SessionEnd::Crashed, Some("antiga")));
        for n in 1..=10 {
            input.sessions.push(session(
                &format!("s{n:02}"),
                n,
                SessionEnd::ClosedNormally,
                None,
            ));
        }
        assert_eq!(score(&input, w).score, 100);
    }

    #[test]
    fn desempenho_carregamento_lento_e_memoria_abaixo() {
        let w = weights().unwrap();
        let mut good = session("bom", 1, SessionEnd::ClosedNormally, None);
        good.loaded_ms = Some(60_000);
        let mut last = session("ultimo", 2, SessionEnd::ClosedNormally, None);
        last.loaded_ms = Some(90_001);
        last.memory = Some(SessionMemory {
            mb: 2048,
            auto: false,
            suggested_mb: 4096,
        });
        let input = HealthInput {
            sessions: vec![good, last.clone()],
            current_tree: Some("sha256:agora".into()),
            ..HealthInput::default()
        };
        let health = score(&input, w);
        assert_eq!(health.score, 95);
        assert_eq!(health.lines[0].category, HealthCategory::Performance);
        assert_eq!(health.lines[0].reasons.len(), 2);

        // 50% exatos não contam; memória automática também não.
        let mut input = input;
        input.sessions[1].loaded_ms = Some(90_000);
        input.sessions[1].memory = Some(SessionMemory {
            mb: 2048,
            auto: true,
            suggested_mb: 4096,
        });
        assert_eq!(score(&input, w).score, 100);
    }

    #[test]
    fn manutencao_arquivado_beta_e_sem_versao_nova() {
        let w = weights().unwrap();
        let mut archived = finding(
            "W_OBSOLETE|mods/clumps.pw.toml",
            OBSOLETE_RULE,
            Severity::Warning,
        );
        archived.archived = true;
        let curated = finding(
            "W_OBSOLETE|mods/starlight.pw.toml",
            OBSOLETE_RULE,
            Severity::Warning,
        );
        let beta = finding(
            "I_PRERELEASE|mods/beta.pw.toml",
            PRERELEASE_RULE,
            Severity::Info,
        );
        let now = 100 * MONTH_MS;
        let input = HealthInput {
            findings: vec![archived, curated, beta],
            sessions: vec![session("s", 1, SessionEnd::ClosedNormally, None)],
            current_tree: Some("sha256:agora".into()),
            releases: vec![
                LatestRelease {
                    item: "mods/velho.pw.toml".into(),
                    name: "Velho".into(),
                    published_ms: now - 19 * MONTH_MS,
                },
                LatestRelease {
                    item: "mods/recente.pw.toml".into(),
                    name: "Recente".into(),
                    published_ms: now - 18 * MONTH_MS,
                },
            ],
            now_ms: now,
        };
        let health = score(&input, w);
        // Obsoleto da lista curada continua em "Problemas encontrados" (aviso, −3).
        assert_eq!(health.lines[0].category, HealthCategory::Problems);
        assert_eq!(health.lines[0].points, 3);
        let maintenance = &health.lines[1];
        assert_eq!(maintenance.category, HealthCategory::Maintenance);
        assert_eq!(maintenance.points, 6);
        assert!(maintenance.reasons.contains(&HealthReason::NoNewVersion {
            item: "mods/velho.pw.toml".into(),
            name: "Velho".into(),
            months: 19,
            points: 2
        }));
        assert_eq!(health.score, 91);
    }

    #[test]
    fn achado_de_relatorio_le_risco_e_arquivado() {
        let mixin = Finding::new(
            RuleCode::parse(MIXIN_RULE).unwrap(),
            Severity::Warning,
            "t",
            Evidence::Curated {
                rule_id: "x".into(),
                note: "y".into(),
            },
        )
        .with_param("risco", "medio");
        let found = HealthFinding::new("id", &mixin, false, None);
        assert_eq!(found.risk, Some(MixinRisk::Medium));
        let archived = Finding::new(
            RuleCode::parse(OBSOLETE_RULE).unwrap(),
            Severity::Warning,
            "t",
            Evidence::Api {
                source: crate::Source::Modrinth,
                url: "https://example.invalid".into(),
                field: ARCHIVED_FIELD.into(),
            },
        );
        let found = HealthFinding::new("id", &archived, true, Some("mods/a.pw.toml".into()));
        assert!(found.archived && found.ignored);
        assert_eq!(found.risk, None);
    }

    #[test]
    fn faixas() {
        let bands = weights().unwrap().faixas;
        assert_eq!(HealthBand::of(100, &bands), HealthBand::Great);
        assert_eq!(HealthBand::of(90, &bands), HealthBand::Great);
        assert_eq!(HealthBand::of(89, &bands), HealthBand::Good);
        assert_eq!(HealthBand::of(75, &bands), HealthBand::Good);
        assert_eq!(HealthBand::of(74, &bands), HealthBand::Attention);
        assert_eq!(HealthBand::of(50, &bands), HealthBand::Attention);
        assert_eq!(HealthBand::of(49, &bands), HealthBand::Critical);
        assert_eq!(HealthBand::of(0, &bands), HealthBand::Critical);
    }

    #[test]
    fn json_do_motivo_em_camel_case() {
        let json = serde_json::to_value(HealthReason::SlowLoad {
            session: "s".into(),
            loaded_ms: 1,
            good_session: "g".into(),
            good_ms: 2,
            points: 3,
        })
        .unwrap();
        assert_eq!(
            json,
            serde_json::json!({"kind":"slowLoad","session":"s","loadedMs":1,"goodSession":"g","goodMs":2,"points":3})
        );
    }

    // -----------------------------------------------------------------------------------------
    // Propriedades
    // -----------------------------------------------------------------------------------------

    use crate::model::RuleCode;

    fn any_finding() -> impl Strategy<Value = HealthFinding> {
        (
            "[a-z0-9|/._]{1,12}",
            prop::sample::select(vec![
                "E_MISSING_DEP",
                "E_JAVA",
                "W_SIDE",
                "I_PACK_FORMAT",
                MIXIN_RULE,
                DOWNLOAD_BLOCKED_RULE,
                PRERELEASE_RULE,
                OBSOLETE_RULE,
            ]),
            prop::sample::select(vec![Severity::Error, Severity::Warning, Severity::Info]),
            any::<bool>(),
            any::<bool>(),
            any::<bool>(),
        )
            .prop_map(
                |(id, rule, severity, ignored, high, archived)| HealthFinding {
                    id,
                    rule: rule.into(),
                    severity,
                    ignored,
                    item: None,
                    risk: (rule == MIXIN_RULE).then_some(if high {
                        MixinRisk::High
                    } else {
                        MixinRisk::Medium
                    }),
                    archived,
                },
            )
    }

    fn any_session() -> impl Strategy<Value = HealthSession> {
        (
            "[0-9]{1,4}",
            0_u64..1_000,
            prop::sample::select(vec![
                SessionEnd::Crashed,
                SessionEnd::ClosedNormally,
                SessionEnd::Stopped,
            ]),
            prop::option::of("[ab]{1,2}"),
            prop::option::of(prop::sample::select(vec!["sha256:agora", "sha256:antes"])),
            prop::option::of(1_u32..200_000),
            prop::option::of((256_u32..8192, any::<bool>(), 256_u32..8192)),
        )
            .prop_map(
                |(id, at, end, signature, tree, loaded_ms, memory)| HealthSession {
                    id,
                    started_at_ms: at,
                    end,
                    signature,
                    tree: tree.map(Into::into),
                    loaded_ms,
                    memory: memory.map(|(mb, auto, suggested_mb)| SessionMemory {
                        mb,
                        auto,
                        suggested_mb,
                    }),
                },
            )
    }

    fn any_input() -> impl Strategy<Value = HealthInput> {
        (
            prop::collection::vec(any_finding(), 0..40),
            prop::collection::vec(any_session(), 0..15),
            prop::option::of(prop::sample::select(vec!["sha256:agora", "sha256:antes"])),
            prop::collection::vec(("[a-z]{1,6}", 0_u64..(60 * MONTH_MS)), 0..6),
        )
            .prop_map(|(mut findings, sessions, tree, releases)| HealthInput {
                // Os ids de um relatório são únicos.
                findings: {
                    let mut seen = BTreeSet::new();
                    findings.retain(|finding| seen.insert(finding.id.clone()));
                    findings
                },
                sessions,
                current_tree: tree.map(Into::into),
                releases: releases
                    .into_iter()
                    .map(|(item, published_ms)| LatestRelease {
                        name: item.clone(),
                        item,
                        published_ms,
                    })
                    .collect(),
                now_ms: 60 * MONTH_MS,
            })
    }

    proptest! {
        /// A nota nunca sai da faixa e é 100 menos o que as linhas tiram (com o piso em 0); a
        /// faixa é a da nota; as linhas seguem a ordem da tabela e respeitam o teto.
        #[test]
        fn nota_na_faixa_e_coerente_com_as_linhas(input in any_input()) {
            let w = weights().unwrap();
            let health = score(&input, w);
            prop_assert!(health.score <= 100);
            let lost: u32 = health.lines.iter().map(|line| line.points).sum();
            prop_assert_eq!(u32::from(health.score), 100_u32.saturating_sub(lost));
            prop_assert_eq!(health.band, HealthBand::of(health.score, &w.faixas));
            let order: Vec<usize> = health
                .lines
                .iter()
                .map(|line| HealthCategory::ORDER.iter().position(|c| *c == line.category).unwrap())
                .collect();
            prop_assert!(order.windows(2).all(|pair| pair[0] <= pair[1]));
            for line in &health.lines {
                prop_assert!(line.points <= w.cap(line.category));
                prop_assert!(!line.reasons.is_empty());
                if line.ignored {
                    prop_assert_eq!(line.points, 0);
                }
            }
        }

        /// Acrescentar um problema nunca aumenta a nota; tirar um nunca a diminui.
        #[test]
        fn problema_a_mais_nunca_aumenta_a_nota(input in any_input(), extra in any_finding()) {
            let w = weights().unwrap();
            let before = score(&input, w).score;
            let mut more = input.clone();
            more.findings.push(extra);
            prop_assert!(score(&more, w).score <= before);
            if !input.findings.is_empty() {
                let mut less = input.clone();
                less.findings.remove(0);
                prop_assert!(score(&less, w).score >= before);
            }
        }

        /// Ignorar um achado nunca diminui a nota, e ele sai das linhas que contam para a linha
        /// dos ignorados.
        #[test]
        fn ignorado_fica_riscado_e_sem_desconto(input in any_input(), index in any::<prop::sample::Index>()) {
            let w = weights().unwrap();
            prop_assume!(!input.findings.is_empty());
            let at = index.index(input.findings.len());
            let mut ignored = input.clone();
            ignored.findings[at].ignored = true;
            let id = ignored.findings[at].id.clone();
            let before = score(&input, w).score;
            let health = score(&ignored, w);
            prop_assert!(health.score >= before);
            let in_line = |want_ignored: bool| health.lines.iter().any(|line| {
                line.ignored == want_ignored
                    && line.reasons.iter().any(|reason| matches!(
                        reason,
                        HealthReason::Finding { finding_id, .. } if *finding_id == id
                    ))
            });
            prop_assert!(!in_line(false));
            if finding_points(&ignored.findings[at], w).is_some() {
                prop_assert!(in_line(true));
            }
        }

        /// Uma versão sem novidade a mais nunca aumenta a nota.
        #[test]
        fn mod_parado_a_mais_nunca_aumenta_a_nota(input in any_input(), published in 0_u64..(60 * MONTH_MS)) {
            let w = weights().unwrap();
            let before = score(&input, w).score;
            let mut more = input.clone();
            more.releases.push(LatestRelease {
                item: "mods/novo-item.pw.toml".into(),
                name: "Novo".into(),
                published_ms: published,
            });
            prop_assert!(score(&more, w).score <= before);
        }
    }
}
