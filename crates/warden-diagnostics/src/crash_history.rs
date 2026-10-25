//! Histórico de travamentos (ARCHITECTURE §9.6; R5A §7.5; ROADMAP D-06): as sessões que
//! travaram agrupadas pela [`CrashSignature`], com contagem, primeira e última vez, as versões do
//! pack de cada sessão e a situação.
//!
//! Situação de um grupo, nesta ordem:
//! 1. **não voltou a acontecer desde a versão X**: depois do último travamento do grupo, uma
//!    sessão fechou normalmente com uma árvore do pack em que esse travamento nunca aconteceu
//!    (X é a versão dessa sessão);
//! 2. **causa encontrada**: a análise do travamento mais novo achou um erro determinístico;
//! 3. **causa não encontrada**.
//!
//! Gancho 1.2 (ADR-0039): cada travamento tem a origem ([`CrashOrigin`]); por enquanto só
//! existe "teste". A W-07 acrescenta a do jogador.
//!
//! Limites: só agrupa o que recebe. Ler as sessões do disco e a retenção ficam com o app.

use std::cmp::Ordering;
use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::signature::CrashSignature;

/// De onde veio o travamento.
#[derive(
    Debug,
    Clone,
    Copy,
    Default,
    PartialEq,
    Eq,
    Hash,
    PartialOrd,
    Ord,
    Serialize,
    Deserialize,
    specta::Type,
)]
#[serde(rename_all = "camelCase")]
pub enum CrashOrigin {
    /// Um teste do Warden.
    #[default]
    Test,
}

/// Como uma sessão terminou, para o histórico.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionEnd {
    /// O jogo travou.
    Crashed,
    /// O jogo fechou normalmente.
    ClosedNormally,
    /// O usuário parou o jogo (não diz nada sobre o travamento).
    Stopped,
}

/// A versão do pack de uma sessão.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct PackState {
    /// `version` do `pack.toml`.
    pub version: Option<String>,
    /// Resumo da árvore do pack testada (`sha256:…`).
    pub tree: Option<String>,
    /// Havia alterações não salvas como versão.
    pub unsaved: bool,
}

/// Uma sessão gravada, como o histórico a vê.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionEntry {
    /// Nome da pasta da sessão.
    pub id: String,
    /// Início, em milissegundos desde 1970.
    pub started_at_ms: u64,
    /// Como terminou.
    pub end: SessionEnd,
    /// A versão do pack.
    pub pack: PackState,
    /// A assinatura (só nas que travaram).
    pub signature: Option<CrashSignature>,
    /// A análise achou a causa (só nas que travaram).
    pub found_cause: bool,
    /// Origem do travamento.
    pub origin: CrashOrigin,
}

/// A situação de um grupo de travamentos.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CrashStatus {
    /// Não voltou a acontecer desde a versão de [`PackState`].
    Resolved {
        /// A versão em que o jogo fechou normalmente depois do último travamento.
        since: PackState,
        /// A sessão que fechou normalmente.
        session_id: String,
    },
    /// A análise achou a causa.
    CauseFound,
    /// Nenhum padrão conhecido apontou a causa.
    CauseUnknown,
}

/// Os travamentos com a mesma assinatura.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrashGroup {
    /// A assinatura comum.
    pub signature: CrashSignature,
    /// As sessões, da mais nova para a mais antiga.
    pub sessions: Vec<String>,
    /// Primeira vez (milissegundos desde 1970).
    pub first_at_ms: u64,
    /// Última vez.
    pub last_at_ms: u64,
    /// As versões do pack em que travou, da mais antiga para a mais nova, sem repetir.
    pub versions: Vec<PackState>,
    /// Quantas vezes por origem.
    pub origins: Vec<(CrashOrigin, u32)>,
    /// A situação.
    pub status: CrashStatus,
}

impl CrashGroup {
    /// Quantas vezes travou.
    #[must_use]
    pub fn count(&self) -> usize {
        self.sessions.len()
    }

    /// A sessão mais nova do grupo.
    #[must_use]
    pub fn latest(&self) -> &str {
        self.sessions.first().map_or("", String::as_str)
    }
}

/// Tamanho do nome da pasta de uma sessão (`AAAA-MM-DD_HH-MM-SS`); a segunda sessão no mesmo
/// segundo ganha `-2`, a décima `-10`.
const SESSION_FOLDER_LEN: usize = 19;

/// Ordem cronológica de duas sessões: pelo início e, no empate, pelo nome da pasta com o
/// sufixo numérico (`…-2` antes de `…-10`, que na ordem do texto viria antes).
#[must_use]
pub fn session_order(a_started_ms: u64, a_id: &str, b_started_ms: u64, b_id: &str) -> Ordering {
    a_started_ms
        .cmp(&b_started_ms)
        .then_with(|| folder_key(a_id).cmp(&folder_key(b_id)))
}

/// O nome da pasta separado do sufixo de repetição no mesmo segundo (sem sufixo: 1).
fn folder_key(id: &str) -> (&str, u64) {
    if let Some((base, suffix)) = id.rsplit_once('-')
        && base.len() == SESSION_FOLDER_LEN
        && !suffix.is_empty()
        && suffix.bytes().all(|byte| byte.is_ascii_digit())
    {
        return (base, suffix.parse().unwrap_or(u64::MAX));
    }
    (id, 1)
}

/// Agrupa as sessões que travaram (e têm assinatura) pela assinatura. Os grupos vêm do que
/// travou por último para o que travou primeiro.
#[must_use]
pub fn group_crashes(sessions: &[SessionEntry]) -> Vec<CrashGroup> {
    let mut ordered: Vec<&SessionEntry> = sessions.iter().collect();
    ordered.sort_by(|a, b| session_order(a.started_at_ms, &a.id, b.started_at_ms, &b.id));

    let mut order: Vec<String> = Vec::new();
    let mut members: HashMap<String, Vec<&SessionEntry>> = HashMap::new();
    for session in &ordered {
        let (SessionEnd::Crashed, Some(signature)) = (session.end, &session.signature) else {
            continue;
        };
        let list = members.entry(signature.hash.clone()).or_default();
        if list.is_empty() {
            order.push(signature.hash.clone());
        }
        list.push(session);
    }

    let mut groups: Vec<CrashGroup> = order
        .iter()
        .filter_map(|hash| members.get(hash))
        .filter_map(|crashed| build_group(crashed, &ordered))
        .collect();
    groups.sort_by(|a, b| session_order(b.last_at_ms, b.latest(), a.last_at_ms, a.latest()));
    groups
}

/// Um grupo com as sessões dele (da mais antiga para a mais nova) e todas as sessões do pack.
fn build_group(crashed: &[&SessionEntry], all: &[&SessionEntry]) -> Option<CrashGroup> {
    let first = crashed.first()?;
    let last = crashed.last()?;
    let signature = last.signature.clone()?;

    let mut versions: Vec<PackState> = Vec::new();
    for session in crashed {
        let shown = PackState {
            tree: None,
            ..session.pack.clone()
        };
        if !versions.contains(&shown) {
            versions.push(shown);
        }
    }
    let mut origins: Vec<(CrashOrigin, u32)> = Vec::new();
    for session in crashed {
        match origins
            .iter_mut()
            .find(|(origin, _)| *origin == session.origin)
        {
            Some((_, count)) => *count += 1,
            None => origins.push((session.origin, 1)),
        }
    }

    let crashed_trees: Vec<&str> = crashed
        .iter()
        .filter_map(|session| session.pack.tree.as_deref())
        .collect();
    let fixed_in = all.iter().find(|session| {
        session_order(
            session.started_at_ms,
            &session.id,
            last.started_at_ms,
            &last.id,
        ) == Ordering::Greater
            && session.end == SessionEnd::ClosedNormally
            && session
                .pack
                .tree
                .as_deref()
                .is_some_and(|tree| !crashed_trees.contains(&tree))
    });
    let status = match fixed_in {
        Some(session) => CrashStatus::Resolved {
            since: PackState {
                tree: None,
                ..session.pack.clone()
            },
            session_id: session.id.clone(),
        },
        None if last.found_cause => CrashStatus::CauseFound,
        None => CrashStatus::CauseUnknown,
    };

    Some(CrashGroup {
        signature,
        sessions: crashed
            .iter()
            .rev()
            .map(|session| session.id.clone())
            .collect(),
        first_at_ms: first.started_at_ms,
        last_at_ms: last.started_at_ms,
        versions,
        origins,
        status,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn signature(hash: &str) -> CrashSignature {
        CrashSignature::new(Some(hash.to_owned()), None, None, Vec::new(), Vec::new())
    }

    fn pack(version: &str, tree: &str) -> PackState {
        PackState {
            version: Some(version.to_owned()),
            tree: Some(tree.to_owned()),
            unsaved: false,
        }
    }

    fn crash(id: &str, at: u64, cause: &str, pack_state: PackState) -> SessionEntry {
        SessionEntry {
            id: id.to_owned(),
            started_at_ms: at,
            end: SessionEnd::Crashed,
            pack: pack_state,
            signature: Some(signature(cause)),
            found_cause: false,
            origin: CrashOrigin::Test,
        }
    }

    fn closed(id: &str, at: u64, pack_state: PackState) -> SessionEntry {
        SessionEntry {
            id: id.to_owned(),
            started_at_ms: at,
            end: SessionEnd::ClosedNormally,
            pack: pack_state,
            signature: None,
            found_cause: false,
            origin: CrashOrigin::Test,
        }
    }

    #[test]
    fn mesma_assinatura_vira_um_grupo_com_contagem_datas_e_versoes() {
        let sessions = [
            crash("a", 10, "npe", pack("1.4.1", "t1")),
            crash("b", 20, "oom", pack("1.4.1", "t1")),
            crash("c", 30, "npe", pack("1.4.2", "t2")),
            crash("d", 40, "npe", pack("1.4.2", "t3")),
        ];
        let groups = group_crashes(&sessions);
        assert_eq!(groups.len(), 2);
        let npe = &groups[0];
        assert_eq!(npe.signature.hash, signature("npe").hash);
        assert_eq!(npe.count(), 3);
        assert_eq!(npe.sessions, ["d", "c", "a"]);
        assert_eq!(npe.latest(), "d");
        assert_eq!((npe.first_at_ms, npe.last_at_ms), (10, 40));
        // As versões sem repetir (as duas árvores da 1.4.2 aparecem uma vez só).
        let versions: Vec<_> = npe
            .versions
            .iter()
            .map(|state| state.version.as_deref().unwrap())
            .collect();
        assert_eq!(versions, ["1.4.1", "1.4.2"]);
        assert_eq!(npe.origins, [(CrashOrigin::Test, 3)]);
        assert_eq!(groups[1].count(), 1);
        assert_eq!(groups[1].sessions, ["b"]);
    }

    #[test]
    fn alteracoes_nao_salvas_aparecem_como_versao_propria() {
        let mut changed = pack("1.4.2", "t9");
        changed.unsaved = true;
        let groups = group_crashes(&[
            crash("a", 1, "x", pack("1.4.2", "t2")),
            crash("b", 2, "x", changed),
        ]);
        assert_eq!(groups[0].versions.len(), 2);
        assert!(groups[0].versions[1].unsaved);
    }

    #[test]
    fn situacao_nao_voltou_desde_a_versao_que_fechou_normalmente() {
        let sessions = [
            crash("a", 10, "oom", pack("1.4.0", "t0")),
            // Mesma árvore fechando normalmente não prova nada.
            closed("b", 20, pack("1.4.0", "t0")),
            closed("c", 30, pack("1.4.1", "t1")),
            closed("d", 40, pack("1.4.2", "t2")),
        ];
        let groups = group_crashes(&sessions);
        assert_eq!(
            groups[0].status,
            CrashStatus::Resolved {
                since: PackState {
                    version: Some("1.4.1".into()),
                    tree: None,
                    unsaved: false
                },
                session_id: "c".into(),
            }
        );
    }

    #[test]
    fn situacao_causa_encontrada_ou_nao() {
        let mut found = crash("a", 10, "dep", pack("1.4.2", "t2"));
        found.found_cause = true;
        let unknown = crash("b", 20, "npe", pack("1.4.2", "t2"));
        // Uma sessão parada pelo usuário com outra árvore não resolve.
        let mut stopped = closed("c", 30, pack("1.4.3", "t3"));
        stopped.end = SessionEnd::Stopped;
        let groups = group_crashes(&[found, unknown, stopped]);
        assert_eq!(groups[0].status, CrashStatus::CauseUnknown);
        assert_eq!(groups[1].status, CrashStatus::CauseFound);
    }

    #[test]
    fn travou_de_novo_depois_de_fechar_normalmente_nao_esta_resolvido() {
        let groups = group_crashes(&[
            crash("a", 10, "npe", pack("1.4.0", "t0")),
            closed("b", 20, pack("1.4.1", "t1")),
            crash("c", 30, "npe", pack("1.4.1", "t1")),
        ]);
        assert_eq!(groups[0].status, CrashStatus::CauseUnknown);
    }

    #[test]
    fn sem_assinatura_ou_sem_travar_fica_de_fora_e_lista_vazia() {
        let mut no_signature = crash("a", 1, "x", PackState::default());
        no_signature.signature = None;
        assert!(group_crashes(&[no_signature, closed("b", 2, PackState::default())]).is_empty());
        assert!(group_crashes(&[]).is_empty());
    }

    #[test]
    fn mesma_hora_desempata_pelo_nome_da_pasta() {
        let groups = group_crashes(&[
            crash("2026-10-01_10-00-00-2", 5, "x", PackState::default()),
            crash("2026-10-01_10-00-00", 5, "x", PackState::default()),
        ]);
        assert_eq!(
            groups[0].sessions,
            ["2026-10-01_10-00-00-2", "2026-10-01_10-00-00"]
        );
        // A décima sessão do mesmo segundo é mais nova que a segunda (no texto, `-10` < `-2`).
        let groups = group_crashes(&[
            crash("2026-10-01_10-00-00-10", 5, "x", PackState::default()),
            crash("2026-10-01_10-00-00-2", 5, "x", PackState::default()),
            crash("2026-10-01_10-00-00-9", 5, "y", PackState::default()),
        ]);
        assert_eq!(
            groups[0].sessions,
            ["2026-10-01_10-00-00-10", "2026-10-01_10-00-00-2"]
        );
        assert_eq!(groups[1].sessions, ["2026-10-01_10-00-00-9"]);
        assert_eq!(
            session_order(5, "2026-10-01_10-00-00-10", 5, "2026-10-01_10-00-00-2"),
            Ordering::Greater
        );
        assert_eq!(
            session_order(4, "2026-10-01_10-00-00-10", 5, "2026-10-01_10-00-00"),
            Ordering::Less
        );
    }

    #[test]
    fn origem_padrao_e_teste_e_serializa_em_camel_case() {
        assert_eq!(CrashOrigin::default(), CrashOrigin::Test);
        assert_eq!(serde_json::to_value(CrashOrigin::Test).unwrap(), "test");
    }
}
