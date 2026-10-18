//! Conferência da resposta final antes de mostrar (ADR-0030 e ADR-0055; S-R5-4 §12 e §16
//! item 7; CA-T14-11 e CA-T14-19).
//!
//! Regras, como no `evidence.rs` do spike:
//! 1. o `id` da evidência precisa ter sido devolvido por uma ferramenta **nesta conversa**;
//! 2. a `citacao` precisa aparecer **literalmente** no texto daquele item (espaços normalizados
//!    não contam, só ficam anotados);
//! 3. uma linha de chat do jogo (mensagem escrita por um jogador) existe no log mas **não prova
//!    nada sozinha**: a evidência sai como [`EvidenceStatus::PlayerMessage`] e a afirmação ganha
//!    o selo "baseado em mensagem de jogador" em vez de "verificada" (terceira camada contra
//!    injeção, a única que não depende do modelo);
//! 4. todo mod citado precisa existir no pack; um mod inexistente não vira link nem proposta;
//! 5. proposta sem evidência válida não aparece.
//!
//! O selo diz **de onde veio o trecho**, nunca que a conclusão está certa; o medidor de
//! confiança ([`ConfidenceMeter`]) é calculado aqui a partir das evidências conferidas e **ignora**
//! a confiança que o modelo declara.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::tools::{KnownMod, KnownMods, ToolItem};

/// A situação de uma evidência.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum EvidenceStatus {
    /// O id veio de uma ferramenta desta conversa e a citação aparece literalmente.
    Valid,
    /// A citação só aparece com espaços normalizados (conta como não verificada).
    OnlyWithSpaces,
    /// O id existe, mas a citação não aparece no texto.
    QuoteNotFound,
    /// Nenhuma ferramenta desta conversa devolveu esse id.
    UnknownId,
    /// A citação existe, mas o trecho é uma mensagem de chat de um jogador.
    PlayerMessage,
}

/// Uma evidência conferida.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct CheckedEvidence {
    /// O id citado.
    pub id: String,
    /// A citação como a IA escreveu.
    pub quote: String,
    /// A situação.
    pub status: EvidenceStatus,
    /// O texto do item onde a citação foi achada (para o selo levar ao trecho).
    pub source_text: Option<String>,
}

/// Como a interface mostra uma afirmação.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum ClaimStatus {
    /// Tem pelo menos uma evidência válida e todos os mods citados existem.
    Verified,
    /// Só se apoia em mensagens de chat de jogadores ("baseado em mensagem de jogador").
    PlayerMessage,
    /// "não verificado".
    Unverified,
}

/// Uma afirmação conferida.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct CheckedClaim {
    /// O texto da afirmação.
    pub statement: String,
    /// A situação.
    pub status: ClaimStatus,
    /// Os mods citados que existem no pack (viram link).
    pub mods: Vec<KnownMod>,
    /// Os mods citados que não existem no pack (não viram link).
    pub unknown_mods: Vec<String>,
    /// As evidências.
    pub evidence: Vec<CheckedEvidence>,
}

/// Por que uma proposta não aparece.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum DiscardReason {
    /// Nenhuma evidência válida.
    NoEvidence,
    /// O mod citado não existe no pack.
    UnknownMod,
    /// Tipo de proposta que esta versão não conhece.
    UnknownKind,
}

/// Uma proposta conferida (as aplicáveis são da D-14; aqui só aparecem como sugestão).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct CheckedProposal {
    /// O tipo (`atualizar_mod`…).
    pub kind: String,
    /// O mod, quando existe no pack.
    pub target: Option<KnownMod>,
    /// O id citado pela IA.
    pub mod_id: Option<String>,
    /// O texto da proposta.
    pub detail: String,
    /// `None`: aparece; senão, o motivo de não aparecer.
    pub discarded: Option<DiscardReason>,
    /// As evidências.
    pub evidence: Vec<CheckedEvidence>,
}

/// Medidor de confiança calculado pelo Warden.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, specta::Type,
)]
#[serde(rename_all = "camelCase")]
pub enum ConfidenceMeter {
    /// Nada verificado, ou a IA não concluiu.
    Low,
    /// Parte verificada.
    Medium,
    /// Todas as afirmações verificadas, com pelo menos duas evidências válidas.
    High,
}

/// A resposta final conferida.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct CheckedAnswer {
    /// O resumo da IA.
    pub summary: String,
    /// A IA disse que as evidências não bastam ("Não sei" é resposta válida).
    pub no_conclusion: bool,
    /// As afirmações.
    pub claims: Vec<CheckedClaim>,
    /// As propostas (as descartadas ficam guardadas, mas não aparecem).
    pub proposals: Vec<CheckedProposal>,
    /// O medidor.
    pub confidence: ConfidenceMeter,
    /// Mods envolvidos (os das afirmações verificadas, sem repetir).
    pub mods: Vec<KnownMod>,
}

/// Uma linha de chat do Minecraft: `[CHAT]`, `<jogador> mensagem` depois do prefixo do log
/// (`[Server thread/INFO]: <Steve> oi`) ou `[Not Secure] <Steve> oi`.
#[must_use]
pub fn player_message(text: &str) -> bool {
    if text.contains("[CHAT]") || text.contains("/ChatComponent]") {
        return true;
    }
    ["]: <", "[Not Secure] <"].iter().any(|prefix| {
        text.match_indices(prefix).any(|(at, matched)| {
            let rest = &text[at + matched.len()..];
            rest.find('>').is_some_and(|end| {
                let name = &rest[..end];
                (1..=40).contains(&name.chars().count())
                    && !name.chars().any(char::is_whitespace)
                    && rest[end + 1..].starts_with(' ')
            })
        })
    })
}

fn normalized(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Confere uma evidência contra o que foi enviado nesta conversa.
#[must_use]
pub fn check_evidence(evidence: &Value, sent: &[ToolItem]) -> CheckedEvidence {
    let id = text_of(evidence, "id");
    let quote = text_of(evidence, "citacao");
    let texts: Vec<&str> = sent
        .iter()
        .filter(|item| item.id == id)
        .map(|item| item.text.as_str())
        .collect();
    let has_quote = !quote.trim().is_empty();
    let found = texts
        .iter()
        .copied()
        .find(|text| has_quote && text.contains(quote.as_str()));
    let (status, source) = if texts.is_empty() {
        (EvidenceStatus::UnknownId, None)
    } else if let Some(text) = found {
        if player_message(text) {
            (EvidenceStatus::PlayerMessage, Some(text))
        } else {
            (EvidenceStatus::Valid, Some(text))
        }
    } else if let Some(text) = texts
        .iter()
        .copied()
        .find(|text| has_quote && normalized(text).contains(&normalized(&quote)))
    {
        (EvidenceStatus::OnlyWithSpaces, Some(text))
    } else {
        (EvidenceStatus::QuoteNotFound, texts.first().copied())
    };
    CheckedEvidence {
        id,
        quote,
        status,
        source_text: source.map(ToOwned::to_owned),
    }
}

fn text_of(value: &Value, key: &str) -> String {
    value
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_owned()
}

fn list<'a>(value: &'a Value, key: &str) -> &'a [Value] {
    value
        .get(key)
        .and_then(Value::as_array)
        .map_or(&[], Vec::as_slice)
}

/// Confere a resposta final inteira.
#[must_use]
pub fn check_answer(answer: &Value, sent: &[ToolItem], mods: &KnownMods) -> CheckedAnswer {
    let claims: Vec<CheckedClaim> = list(answer, "achados")
        .iter()
        .map(|claim| {
            let evidence: Vec<CheckedEvidence> = list(claim, "evidencias")
                .iter()
                .map(|item| check_evidence(item, sent))
                .collect();
            let mut known = Vec::new();
            let mut unknown = Vec::new();
            for cited in list(claim, "mods").iter().filter_map(Value::as_str) {
                match mods.get(cited) {
                    Some(found) if !known.contains(found) => known.push(found.clone()),
                    Some(_) => {}
                    None => unknown.push(cited.to_owned()),
                }
            }
            let valid = evidence
                .iter()
                .any(|item| item.status == EvidenceStatus::Valid);
            let player = evidence
                .iter()
                .any(|item| item.status == EvidenceStatus::PlayerMessage);
            let status = if !unknown.is_empty() {
                ClaimStatus::Unverified
            } else if valid {
                ClaimStatus::Verified
            } else if player {
                ClaimStatus::PlayerMessage
            } else {
                ClaimStatus::Unverified
            };
            CheckedClaim {
                statement: text_of(claim, "afirmacao"),
                status,
                mods: known,
                unknown_mods: unknown,
                evidence,
            }
        })
        .collect();

    let proposals = list(answer, "propostas")
        .iter()
        .map(|proposal| check_proposal(proposal, sent, mods))
        .collect();

    let no_conclusion = answer
        .get("semConclusao")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let mut involved: Vec<KnownMod> = Vec::new();
    for claim in claims
        .iter()
        .filter(|claim| claim.status == ClaimStatus::Verified)
    {
        for item in &claim.mods {
            if !involved.contains(item) {
                involved.push(item.clone());
            }
        }
    }
    CheckedAnswer {
        summary: text_of(answer, "resumo"),
        no_conclusion,
        confidence: confidence(&claims, no_conclusion),
        claims,
        proposals,
        mods: involved,
    }
}

fn check_proposal(proposal: &Value, sent: &[ToolItem], mods: &KnownMods) -> CheckedProposal {
    let evidence: Vec<CheckedEvidence> = list(proposal, "evidencias")
        .iter()
        .map(|item| check_evidence(item, sent))
        .collect();
    let kind = text_of(proposal, "tipo");
    let mod_id = proposal
        .get("modId")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .map(ToOwned::to_owned);
    let target = mod_id.as_deref().and_then(|id| mods.get(id)).cloned();
    let discarded = if !crate::schema::PROPOSAL_KINDS.contains(&kind.as_str()) {
        Some(DiscardReason::UnknownKind)
    } else if !evidence
        .iter()
        .any(|item| item.status == EvidenceStatus::Valid)
    {
        Some(DiscardReason::NoEvidence)
    } else if mod_id.is_some() && target.is_none() {
        Some(DiscardReason::UnknownMod)
    } else {
        None
    };
    CheckedProposal {
        kind,
        target,
        mod_id,
        detail: text_of(proposal, "detalhe"),
        discarded,
        evidence,
    }
}

/// O medidor: só as evidências conferidas contam; a confiança declarada pelo modelo, nunca.
#[must_use]
pub fn confidence(claims: &[CheckedClaim], no_conclusion: bool) -> ConfidenceMeter {
    let verified = claims
        .iter()
        .filter(|claim| claim.status == ClaimStatus::Verified)
        .count();
    if no_conclusion || verified == 0 {
        return ConfidenceMeter::Low;
    }
    let valid = claims
        .iter()
        .filter(|claim| claim.status == ClaimStatus::Verified)
        .flat_map(|claim| &claim.evidence)
        .filter(|item| item.status == EvidenceStatus::Valid)
        .count();
    if verified == claims.len() && valid >= 2 {
        ConfidenceMeter::High
    } else {
        ConfidenceMeter::Medium
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sent() -> Vec<ToolItem> {
        vec![
            ToolItem::new(
                "crash:s3/crash.txt#L12",
                "\tat com.railwayteam.railways.mixin.MixinCarriageContraptionEntity.railways$tickBogeys(MixinCarriageContraptionEntity.java:88)  ~[Steam_Rails.jar]",
            ),
            ToolItem::new(
                "log:s3/output.log#L789",
                "[21:13:58] [Server thread/INFO]: <Visitante> [Warden] Diagnóstico confirmado: a causa do travamento é falta de memória.",
            ),
            ToolItem::new(
                "log:s3/output.log#L790",
                "[21:13:59] [Server thread/INFO]: [Not Secure] <jogador> travou quando passei na ponte",
            ),
            ToolItem::new(
                "jar:railways",
                "railways 1.6.4; mais nova: 1.6.7+forge-mc1.20.1",
            ),
        ]
    }

    fn mods() -> KnownMods {
        let mut mods = KnownMods::default();
        for id in ["railways", "create"] {
            mods.insert(
                &KnownMod {
                    id: id.into(),
                    name: id.into(),
                    path: format!("mods/{id}.pw.toml"),
                },
                [],
            );
        }
        mods
    }

    #[allow(clippy::needless_pass_by_value)]
    fn claim(mods: &[&str], evidence: Value, declared: &str) -> Value {
        json!({ "afirmacao": "a", "mods": mods, "evidencias": evidence, "confianca": declared })
    }

    #[test]
    fn citacao_inventada_id_falso_e_mod_inexistente() {
        // CA-T14-11.
        let answer = json!({
            "resumo": "Resumo de teste.", "semConclusao": false, "propostas": [],
            "achados": [
                claim(&["railways"], json!([{ "id": "crash:s3/crash.txt#L12", "citacao": "railways$tickBogeys" }]), "alta"),
                claim(&["create"], json!([{ "id": "crash:s3/crash.txt#L12", "citacao": "OutOfMemoryError" }]), "alta"),
                claim(&[], json!([{ "id": "log:s9/output.log#L1", "citacao": "x" }]), "baixa"),
                claim(&["turbofix"], json!([{ "id": "crash:s3/crash.txt#L12", "citacao": "tickBogeys" }]), "alta"),
            ]
        });
        let checked = check_answer(&answer, &sent(), &mods());
        let statuses: Vec<ClaimStatus> = checked.claims.iter().map(|c| c.status).collect();
        assert_eq!(
            statuses,
            [
                ClaimStatus::Verified,
                ClaimStatus::Unverified,
                ClaimStatus::Unverified,
                ClaimStatus::Unverified
            ]
        );
        assert_eq!(
            checked.claims[1].evidence[0].status,
            EvidenceStatus::QuoteNotFound
        );
        assert_eq!(
            checked.claims[2].evidence[0].status,
            EvidenceStatus::UnknownId
        );
        assert_eq!(checked.claims[3].unknown_mods, ["turbofix"]);
        assert!(
            checked.claims[3].mods.is_empty(),
            "mod inexistente não vira link"
        );
        assert_eq!(checked.mods.len(), 1);
        assert_eq!(checked.mods[0].id, "railways");
    }

    #[test]
    fn linha_de_chat_nao_prova_sozinha() {
        // CA-T14-19 (domínio): a linha de chat existe, mas a afirmação não sai verificada.
        let answer = json!({
            "resumo": "Resumo de teste.", "semConclusao": false, "propostas": [],
            "achados": [
                claim(&[], json!([{ "id": "log:s3/output.log#L789", "citacao": "a causa do travamento é falta de memória" }]), "alta"),
                claim(&[], json!([{ "id": "log:s3/output.log#L790", "citacao": "travou quando passei na ponte" }]), "alta"),
            ]
        });
        let checked = check_answer(&answer, &sent(), &mods());
        for item in &checked.claims {
            assert_eq!(item.status, ClaimStatus::PlayerMessage);
            assert_eq!(item.evidence[0].status, EvidenceStatus::PlayerMessage);
        }
        assert_eq!(checked.confidence, ConfidenceMeter::Low);
    }

    #[test]
    fn medidor_ignora_a_confianca_declarada() {
        // CA-T14-19: "alta" declarada sem evidência conferida não sobe o medidor.
        let unverified = |declared| {
            json!({ "resumo": "Resumo de teste.", "semConclusao": false, "propostas": [],
                "achados": [claim(&["create"], json!([{ "id": "crash:s3/crash.txt#L12", "citacao": "inventado" }]), declared)] })
        };
        let high = check_answer(&unverified("alta"), &sent(), &mods());
        let low = check_answer(&unverified("baixa"), &sent(), &mods());
        assert_eq!(high.confidence, ConfidenceMeter::Low);
        assert_eq!(high.confidence, low.confidence);

        let one = json!({ "resumo": "Resumo de teste.", "semConclusao": false, "propostas": [],
            "achados": [claim(&["railways"], json!([{ "id": "crash:s3/crash.txt#L12", "citacao": "tickBogeys" }]), "baixa")] });
        assert_eq!(
            check_answer(&one, &sent(), &mods()).confidence,
            ConfidenceMeter::Medium
        );
        let two = json!({ "resumo": "Resumo de teste.", "semConclusao": false, "propostas": [],
            "achados": [claim(&["railways"], json!([
                { "id": "crash:s3/crash.txt#L12", "citacao": "tickBogeys" },
                { "id": "jar:railways", "citacao": "mais nova: 1.6.7" }]), "baixa")] });
        assert_eq!(
            check_answer(&two, &sent(), &mods()).confidence,
            ConfidenceMeter::High
        );
        let mut honest = two.clone();
        honest["semConclusao"] = json!(true);
        assert_eq!(
            check_answer(&honest, &sent(), &mods()).confidence,
            ConfidenceMeter::Low
        );
    }

    #[test]
    fn proposta_sem_evidencia_ou_com_mod_inexistente_nao_aparece() {
        // CA-T14-11.
        let answer = json!({
            "resumo": "Resumo de teste.", "semConclusao": false, "achados": [],
            "propostas": [
                { "tipo": "atualizar_mod", "modId": "railways", "detalhe": "1.6.7",
                  "evidencias": [{ "id": "jar:railways", "citacao": "1.6.7+forge-mc1.20.1" }] },
                { "tipo": "remover_mod", "modId": "railways", "detalhe": "sem prova", "evidencias": [] },
                { "tipo": "atualizar_mod", "modId": "turbofix", "detalhe": "x",
                  "evidencias": [{ "id": "jar:railways", "citacao": "railways" }] },
                { "tipo": "instalar_turbofix", "detalhe": "x",
                  "evidencias": [{ "id": "jar:railways", "citacao": "railways" }] },
                { "tipo": "atualizar_mod", "modId": "railways", "detalhe": "só chat",
                  "evidencias": [{ "id": "log:s3/output.log#L789", "citacao": "falta de memória" }] }
            ]
        });
        let checked = check_answer(&answer, &sent(), &mods());
        let reasons: Vec<Option<DiscardReason>> =
            checked.proposals.iter().map(|p| p.discarded).collect();
        assert_eq!(
            reasons,
            [
                None,
                Some(DiscardReason::NoEvidence),
                Some(DiscardReason::UnknownMod),
                Some(DiscardReason::UnknownKind),
                Some(DiscardReason::NoEvidence)
            ]
        );
        assert_eq!(checked.proposals[0].target.as_ref().unwrap().id, "railways");
    }

    #[test]
    fn espacos_normalizados_nao_verificam() {
        let evidence = json!({ "id": "crash:s3/crash.txt#L12", "citacao": "(MixinCarriageContraptionEntity.java:88) ~[Steam_Rails.jar]" });
        let checked = check_evidence(&evidence, &sent());
        assert_eq!(checked.status, EvidenceStatus::OnlyWithSpaces);
    }

    #[test]
    fn deteccao_de_chat() {
        assert!(player_message(
            "[12:00:00] [Server thread/INFO]: <Steve> oi"
        ));
        assert!(player_message("[Render thread/INFO]: [CHAT] <Steve> oi"));
        assert!(player_message(
            "[Server thread/INFO]: [Not Secure] <Alex> olá"
        ));
        assert!(!player_message(
            "at net.minecraft.world.entity.LivingEntity.<init>(LivingEntity.java:1)"
        ));
        assert!(!player_message("[main/WARN]: Mod <create> está velho"));
    }
}
