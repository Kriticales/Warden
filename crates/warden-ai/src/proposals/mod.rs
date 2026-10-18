//! As propostas da IA (SPEC T14 ✦ passo 3; ARCHITECTURE §9.5 "Propostas"; ADR-0030; CA-T14-11
//! e CA-T14-12).
//!
//! - As ferramentas `propose_*` ([`tools`]) entram no registro da D-04 e são liberadas por
//!   `allowedFunctionNames` quando a IA já abriu um suspeito. **Nunca têm efeito**: o laço
//!   ([`crate::engine`]) chama [`record`], que confere a proposta e a guarda na conversa como
//!   um cartão pendente.
//! - Toda proposta precisa de evidência: as citações passam pela mesma conferência das
//!   afirmações ([`crate::evidence::check_evidence`], contra o que as ferramentas devolveram
//!   **nesta conversa**). Pelo menos uma precisa ser válida, nenhuma pode ser inventada, e uma
//!   linha de chat de jogador não sustenta proposta sozinha. Sem isso, o Warden devolve o erro
//!   à IA e nada aparece.
//! - O mod citado precisa existir no pack (os mesmos nomes da conferência da D-04); o alvo é
//!   conferido pela `warden-app` ([`crate::tools::ToolSource::check_proposal`]: a versão existe,
//!   a chave da config existe, o projeto existe na fonte), que devolve o resumo do cartão.
//! - Aplicar e Descartar são da `warden-app` (`ai_tools/proposals.rs`), pelos fluxos normais
//!   (dependências, diferença antes de salvar, ponto de segurança); aqui fica só a decisão
//!   gravada na conversa ([`decide`]): uma proposta decidida não muda mais.

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::conversation::Conversation;
use crate::engine::now_ms;
use crate::error::{Error, Result};
use crate::evidence::{CheckedEvidence, EvidenceStatus, check_evidence};
use crate::tools::{KnownMods, Needs, Stage, ToolSource, ToolSpec};

#[cfg(test)]
mod tests;

/// Prefixo das ferramentas de proposta.
pub const PREFIX: &str = "propose_";
/// `propose_add_mod`.
pub const ADD_MOD: &str = "propose_add_mod";
/// `propose_remove_mod`.
pub const REMOVE_MOD: &str = "propose_remove_mod";
/// `propose_update_mod`.
pub const UPDATE_MOD: &str = "propose_update_mod";
/// `propose_change_version`.
pub const CHANGE_VERSION: &str = "propose_change_version";
/// `propose_set_side`.
pub const SET_SIDE: &str = "propose_set_side";
/// `propose_edit_config`.
pub const EDIT_CONFIG: &str = "propose_edit_config";
/// `propose_test_settings`.
pub const TEST_SETTINGS: &str = "propose_test_settings";
/// `propose_run_test`.
pub const RUN_TEST: &str = "propose_run_test";
/// `propose_start_bisect`.
pub const START_BISECT: &str = "propose_start_bisect";

/// Memória de teste aceita numa proposta, em MB.
const MEMORY_RANGE: std::ops::RangeInclusive<u32> = 512..=65_536;
/// Java aceito numa proposta.
const JAVA_RANGE: std::ops::RangeInclusive<u32> = 8..=99;
/// Tamanho do motivo mostrado no cartão.
const MAX_REASON_CHARS: usize = 500;

/// Se a ferramenta é uma proposta.
#[must_use]
pub fn is_proposal(name: &str) -> bool {
    name.starts_with(PREFIX)
}

/// Fonte de um projeto proposto.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum ProposalSource {
    /// Modrinth.
    Modrinth,
    /// CurseForge.
    Curseforge,
}

/// Lado proposto.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum ProposalSide {
    /// Só cliente.
    Client,
    /// Só servidor.
    Server,
    /// Cliente e servidor.
    Both,
}

/// A mudança proposta, já com os mods traduzidos para o caminho na lista de Mods.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum ProposalKind {
    /// Adicionar um projeto (com as dependências, pelo plano normal de "Adicionar").
    #[serde(rename_all = "camelCase")]
    AddMod {
        /// Fonte.
        source: ProposalSource,
        /// Id do projeto na fonte.
        project_id: String,
    },
    /// Remover um item.
    #[serde(rename_all = "camelCase")]
    RemoveMod {
        /// Caminho do item (`mods/x.pw.toml`).
        path: String,
    },
    /// Atualizar um item para a versão compatível mais nova.
    #[serde(rename_all = "camelCase")]
    UpdateMod {
        /// Caminho do item.
        path: String,
    },
    /// Trocar um item para outra versão (inclusive mais antiga, "Trocar de versão").
    #[serde(rename_all = "camelCase")]
    ChangeVersion {
        /// Caminho do item.
        path: String,
        /// Número ou id da versão.
        version: String,
    },
    /// Mudar o lado de um item.
    #[serde(rename_all = "camelCase")]
    SetSide {
        /// Caminho do item.
        path: String,
        /// Lado.
        side: ProposalSide,
    },
    /// Mudar uma chave de uma config do pack.
    #[serde(rename_all = "camelCase")]
    EditConfig {
        /// Caminho relativo da config (`config/x.toml`).
        path: String,
        /// A chave, parte por parte (`["general", "maxChunks"]`).
        key: Vec<String>,
        /// O valor novo, em texto.
        value: String,
    },
    /// Mudar a memória, o Java ou os argumentos do perfil ativo do teste (neste computador).
    #[serde(rename_all = "camelCase")]
    TestSettings {
        /// Memória do teste, em MB.
        memory_mb: Option<u32>,
        /// Versão do Java (17, 21…).
        java_major: Option<u32>,
        /// Argumentos extras da JVM.
        jvm_args: Option<String>,
    },
    /// Testar de novo.
    RunTest,
    /// Começar a busca do culpado.
    StartBisect,
}

impl ProposalKind {
    /// Muda arquivos do pack (pede ponto de segurança antes).
    #[must_use]
    pub fn changes_pack(&self) -> bool {
        matches!(
            self,
            Self::AddMod { .. }
                | Self::RemoveMod { .. }
                | Self::UpdateMod { .. }
                | Self::ChangeVersion { .. }
                | Self::SetSide { .. }
                | Self::EditConfig { .. }
        )
    }
}

/// O que o cartão mostra além do tipo, conferido pela `warden-app` ao registrar.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ProposalSummary {
    /// O item, o projeto ou o arquivo ("Supplementaries", "config/x.toml").
    pub target: Option<String>,
    /// Como está agora ("2.8.17", "64", "Cliente e servidor").
    pub from: Option<String>,
    /// Como fica ("2.8.21", "2").
    pub to: Option<String>,
}

/// A decisão do usuário sobre uma proposta.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase", tag = "state")]
pub enum ProposalState {
    /// Esperando o usuário.
    Pending,
    /// Aplicada.
    #[serde(rename_all = "camelCase")]
    Applied {
        /// Quando.
        #[specta(type = specta_typescript::Number)]
        at_ms: u64,
        /// O ponto de segurança criado antes (quando a proposta mudou o pack).
        safety_point: Option<String>,
    },
    /// Descartada (nada foi gravado no pack).
    #[serde(rename_all = "camelCase")]
    Discarded {
        /// Quando.
        #[specta(type = specta_typescript::Number)]
        at_ms: u64,
    },
}

/// Uma proposta guardada na conversa.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct Proposal {
    /// Id na conversa (`p1`, `p2`…).
    pub id: String,
    /// A pergunta em que a IA propôs (posição em `turns`).
    pub turn: u32,
    /// A mudança.
    pub change: ProposalKind,
    /// Por que, nas palavras da IA.
    pub reason: String,
    /// As evidências conferidas.
    pub evidence: Vec<CheckedEvidence>,
    /// O resumo do cartão.
    pub summary: ProposalSummary,
    /// A decisão.
    pub state: ProposalState,
    /// Quando a IA propôs.
    #[specta(type = specta_typescript::Number)]
    pub created_at_ms: u64,
}

fn evidence_schema() -> Value {
    json!({
        "type": "array",
        "minItems": 1,
        "description": "Trechos que justificam a mudança: id = o campo id de um item devolvido por uma ferramenta nesta conversa; citacao = um trecho contínuo copiado literalmente do campo texto desse item.",
        "items": {
            "type": "object",
            "properties": { "id": { "type": "string" }, "citacao": { "type": "string" } },
            "required": ["id", "citacao"]
        }
    })
}

#[allow(clippy::needless_pass_by_value)] // montado na hora por quem chama
fn proposal_spec(
    name: &'static str,
    description: &'static str,
    fields: Value,
    required: &[&str],
) -> ToolSpec {
    let mut properties = fields;
    properties["motivo"] = json!({ "type": "string", "description": "Por que a mudança deve resolver, em uma frase para o usuário." });
    properties["evidencias"] = evidence_schema();
    let mut all_required: Vec<&str> = required.to_vec();
    all_required.extend(["motivo", "evidencias"]);
    ToolSpec {
        name,
        description,
        parameters: json!({ "type": "object", "properties": properties, "required": all_required }),
        stage: Stage::AfterSuspect,
        needs: if name == EDIT_CONFIG {
            Needs::Configs
        } else {
            Needs::Nothing
        },
        opens_suspects: false,
    }
}

/// As ferramentas de proposta (ARCHITECTURE §9.5): nenhuma muda nada; cada uma vira um cartão
/// com **Aplicar** e **Descartar**.
#[must_use]
pub fn tools() -> Vec<ToolSpec> {
    let mod_id = json!({ "type": "string", "description": "Id do mod no pack, como devolvido por list_mods (o id depois de 'mod:')." });
    vec![
        proposal_spec(
            ADD_MOD,
            "Propõe adicionar um mod do Modrinth ou da CurseForge (com as dependências). Só um projeto que uma ferramenta mostrou nesta conversa. Não muda nada sozinha: vira um cartão que o usuário decide se aplica.",
            json!({
                "source": { "type": "string", "enum": ["modrinth", "curseforge"] },
                "project_id": { "type": "string", "description": "Id do projeto na fonte." }
            }),
            &["source", "project_id"],
        ),
        proposal_spec(
            REMOVE_MOD,
            "Propõe remover um mod do pack. Não muda nada sozinha: vira um cartão que o usuário decide se aplica.",
            json!({ "mod_id": mod_id }),
            &["mod_id"],
        ),
        proposal_spec(
            UPDATE_MOD,
            "Propõe atualizar um mod para a versão compatível mais nova. Não muda nada sozinha: vira um cartão que o usuário decide se aplica.",
            json!({ "mod_id": mod_id }),
            &["mod_id"],
        ),
        proposal_spec(
            CHANGE_VERSION,
            "Propõe trocar um mod para uma versão específica (inclusive mais antiga). Não muda nada sozinha: vira um cartão que o usuário decide se aplica.",
            json!({
                "mod_id": mod_id,
                "version": { "type": "string", "description": "Número da versão (como 2.8.21) ou id na fonte." }
            }),
            &["mod_id", "version"],
        ),
        proposal_spec(
            SET_SIDE,
            "Propõe mudar o lado de um mod (só cliente, só servidor ou os dois). Não muda nada sozinha: vira um cartão que o usuário decide se aplica.",
            json!({
                "mod_id": mod_id,
                "side": { "type": "string", "enum": ["client", "server", "both"] }
            }),
            &["mod_id", "side"],
        ),
        proposal_spec(
            EDIT_CONFIG,
            "Propõe mudar o valor de uma chave de uma config do pack (o resto do arquivo fica igual). Não muda nada sozinha: vira um cartão que o usuário decide se aplica.",
            json!({
                "path": { "type": "string", "description": "Caminho relativo da config, como config/create-common.toml." },
                "key": { "type": "array", "items": { "type": "string" }, "minItems": 1, "description": "A chave, parte por parte: seção e nome, como [\"general\", \"maxChunks\"]." },
                "value": { "type": "string", "description": "O valor novo, em texto (true, 64, 1.5, texto)." }
            }),
            &["path", "key", "value"],
        ),
        proposal_spec(
            TEST_SETTINGS,
            "Propõe mudar a memória, o Java ou os argumentos da JVM do teste (só neste computador; não muda o pack). Não muda nada sozinha: vira um cartão que o usuário decide se aplica.",
            json!({
                "memory_mb": { "type": "integer", "minimum": 512, "maximum": 65536, "description": "Memória máxima do teste, em MB." },
                "java_major": { "type": "integer", "minimum": 8, "description": "Versão do Java, como 17 ou 21." },
                "jvm_args": { "type": "string", "description": "Argumentos extras da JVM." }
            }),
            &[],
        ),
        proposal_spec(
            RUN_TEST,
            "Propõe testar o pack de novo (por exemplo, depois de aplicar outra proposta). Não muda nada sozinha: vira um cartão que o usuário decide se aplica.",
            json!({}),
            &[],
        ),
        proposal_spec(
            START_BISECT,
            "Propõe a busca do culpado: o Warden testa o pack em rodadas, ligando e desligando mods, até achar quais causam o problema. Use quando as evidências não bastam. Não muda nada sozinha: vira um cartão que o usuário decide se aplica.",
            json!({}),
            &[],
        ),
    ]
}

fn text_arg<'a>(args: &'a Value, key: &str) -> Option<&'a str> {
    args.get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn required<'a>(args: &'a Value, key: &str) -> std::result::Result<&'a str, String> {
    text_arg(args, key).ok_or_else(|| format!("falta o campo {key}"))
}

fn number_arg(args: &Value, key: &str) -> std::result::Result<Option<u32>, String> {
    match args.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(value) => value
            .as_u64()
            .or_else(|| value.as_str().and_then(|text| text.trim().parse().ok()))
            .and_then(|number| u32::try_from(number).ok())
            .map(Some)
            .ok_or_else(|| format!("o campo {key} precisa ser um número inteiro")),
    }
}

/// O caminho do mod citado pela IA, pelos mesmos nomes da conferência da D-04.
fn mod_path(args: &Value, mods: &KnownMods) -> std::result::Result<String, String> {
    let alias = required(args, "mod_id")?;
    let alias = alias.strip_prefix("mod:").unwrap_or(alias);
    mods.get(alias)
        .map(|known| known.path.clone())
        .ok_or_else(|| {
            format!("o mod {alias} não existe no pack; use um id devolvido por list_mods")
        })
}

/// Caminho relativo seguro de uma config: barras normais, sem `..`, sem raiz nem unidade.
fn config_path(path: &str) -> std::result::Result<String, String> {
    let path = path.trim().trim_start_matches("./");
    let bad = path.is_empty()
        || path.contains(['\\', ':', '\0'])
        || path.starts_with('/')
        || path
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..");
    if bad {
        return Err(format!(
            "caminho de config inválido: {path}; use o caminho relativo devolvido por search_configs ou read_config"
        ));
    }
    Ok(path.to_owned())
}

fn config_value(value: Option<&Value>) -> std::result::Result<String, String> {
    match value {
        Some(Value::String(text)) if !text.contains(['\n', '\r']) => Ok(text.clone()),
        Some(Value::Bool(flag)) => Ok(flag.to_string()),
        Some(Value::Number(number)) => Ok(number.to_string()),
        _ => Err(
            "o campo value precisa ser um valor de uma linha (texto, número ou true/false)"
                .to_owned(),
        ),
    }
}

/// Lê os argumentos de uma ferramenta de proposta. O erro é a frase devolvida à IA.
pub fn parse(
    name: &str,
    args: &Value,
    mods: &KnownMods,
) -> std::result::Result<ProposalKind, String> {
    Ok(match name {
        ADD_MOD => ProposalKind::AddMod {
            source: match required(args, "source")? {
                "modrinth" => ProposalSource::Modrinth,
                "curseforge" => ProposalSource::Curseforge,
                other => {
                    return Err(format!(
                        "fonte desconhecida: {other}; use modrinth ou curseforge"
                    ));
                }
            },
            project_id: required(args, "project_id")?.to_owned(),
        },
        REMOVE_MOD => ProposalKind::RemoveMod {
            path: mod_path(args, mods)?,
        },
        UPDATE_MOD => ProposalKind::UpdateMod {
            path: mod_path(args, mods)?,
        },
        CHANGE_VERSION => ProposalKind::ChangeVersion {
            path: mod_path(args, mods)?,
            version: required(args, "version")?.to_owned(),
        },
        SET_SIDE => ProposalKind::SetSide {
            path: mod_path(args, mods)?,
            side: match required(args, "side")? {
                "client" => ProposalSide::Client,
                "server" => ProposalSide::Server,
                "both" => ProposalSide::Both,
                other => {
                    return Err(format!(
                        "lado desconhecido: {other}; use client, server ou both"
                    ));
                }
            },
        },
        EDIT_CONFIG => {
            let key: Vec<String> = args
                .get("key")
                .and_then(Value::as_array)
                .map(|parts| {
                    parts
                        .iter()
                        .filter_map(Value::as_str)
                        .map(|part| part.trim().to_owned())
                        .collect()
                })
                .unwrap_or_default();
            if key.is_empty()
                || key
                    .iter()
                    .any(|part| part.is_empty() || part.contains(['\n', '\r']))
            {
                return Err("o campo key precisa ser a lista das partes da chave, como [\"general\", \"maxChunks\"]".to_owned());
            }
            ProposalKind::EditConfig {
                path: config_path(required(args, "path")?)?,
                key,
                value: config_value(args.get("value"))?,
            }
        }
        TEST_SETTINGS => {
            let memory_mb = number_arg(args, "memory_mb")?;
            let java_major = number_arg(args, "java_major")?;
            let jvm_args = text_arg(args, "jvm_args").map(ToOwned::to_owned);
            if memory_mb.is_none() && java_major.is_none() && jvm_args.is_none() {
                return Err("diga o que mudar: memory_mb, java_major ou jvm_args".to_owned());
            }
            if memory_mb.is_some_and(|mb| !MEMORY_RANGE.contains(&mb)) {
                return Err("memory_mb precisa estar entre 512 e 65536".to_owned());
            }
            if java_major.is_some_and(|major| !JAVA_RANGE.contains(&major)) {
                return Err("java_major precisa ser uma versão do Java, como 17 ou 21".to_owned());
            }
            if jvm_args
                .as_deref()
                .is_some_and(|text| text.contains(['\n', '\r']))
            {
                return Err("jvm_args precisa caber numa linha".to_owned());
            }
            ProposalKind::TestSettings {
                memory_mb,
                java_major,
                jvm_args,
            }
        }
        RUN_TEST => ProposalKind::RunTest,
        START_BISECT => ProposalKind::StartBisect,
        other => return Err(format!("Ferramenta desconhecida: {other}")),
    })
}

/// Confere as evidências de uma proposta: pelo menos uma válida, nenhuma inventada (id que
/// nenhuma ferramenta devolveu, citação que não está no texto ou só com espaços mudados) e não
/// só linhas de chat de jogador.
pub fn check_proposal_evidence(
    args: &Value,
    sent: &[crate::tools::ToolItem],
) -> std::result::Result<Vec<CheckedEvidence>, String> {
    let cited = args
        .get("evidencias")
        .and_then(Value::as_array)
        .map_or(&[][..], Vec::as_slice);
    if cited.is_empty() {
        return Err("AI_PROPOSAL_NO_EVIDENCE: toda proposta precisa citar evidência desta conversa (id e citação literal de um resultado de ferramenta). Nada foi mostrado ao usuário.".to_owned());
    }
    let checked: Vec<CheckedEvidence> = cited
        .iter()
        .map(|item| check_evidence(item, sent))
        .collect();
    if let Some(bad) = checked.iter().find(|item| {
        !matches!(
            item.status,
            EvidenceStatus::Valid | EvidenceStatus::PlayerMessage
        )
    }) {
        let why = match bad.status {
            EvidenceStatus::UnknownId => "nenhuma ferramenta desta conversa devolveu esse id",
            _ => "a citação não aparece literalmente no texto desse item",
        };
        return Err(format!(
            "AI_PROPOSAL_NO_EVIDENCE: a evidência {} não confere ({why}). Nada foi mostrado ao usuário.",
            bad.id
        ));
    }
    if !checked
        .iter()
        .any(|item| item.status == EvidenceStatus::Valid)
    {
        return Err("AI_PROPOSAL_NO_EVIDENCE: uma mensagem de chat de jogador não sustenta uma proposta sozinha. Nada foi mostrado ao usuário.".to_owned());
    }
    Ok(checked)
}

/// Registra uma chamada `propose_*`: confere tudo e guarda o cartão pendente. Devolve o
/// `response` para a IA; o erro é a frase devolvida a ela (nada aparece para o usuário).
pub async fn record(
    conversation: &mut Conversation,
    name: &str,
    args: &Value,
    source: &dyn ToolSource,
) -> std::result::Result<Value, String> {
    let mods = source.known_mods().await;
    let change = parse(name, args, &mods)?;
    let evidence = check_proposal_evidence(args, &conversation.sent)?;
    if let Some(same) = conversation
        .proposals
        .iter()
        .find(|proposal| proposal.state == ProposalState::Pending && proposal.change == change)
    {
        return Ok(json!({
            "proposta": same.id,
            "situacao": "já proposta nesta conversa e esperando o usuário",
        }));
    }
    let summary = source.check_proposal(&change).await?;
    let reason: String = text_arg(args, "motivo")
        .unwrap_or("")
        .chars()
        .take(MAX_REASON_CHARS)
        .collect();
    let id = format!("p{}", conversation.proposals.len() + 1);
    conversation.proposals.push(Proposal {
        id: id.clone(),
        turn: u32::try_from(conversation.turns.len().saturating_sub(1)).unwrap_or(u32::MAX),
        change,
        reason,
        evidence,
        summary,
        state: ProposalState::Pending,
        created_at_ms: now_ms(),
    });
    Ok(json!({
        "proposta": id,
        "situacao": "registrada como cartão para o usuário decidir; ainda não foi aplicada (não diga que foi)",
    }))
}

/// Grava a decisão do usuário. Uma proposta já decidida não muda
/// ([`Error::ProposalDecided`]).
pub fn decide(conversation: &mut Conversation, id: &str, state: ProposalState) -> Result<()> {
    let proposal = find_mut(conversation, id)?;
    if proposal.state != ProposalState::Pending {
        return Err(Error::ProposalDecided { id: id.to_owned() });
    }
    proposal.state = state;
    conversation.updated_at_ms = now_ms();
    Ok(())
}

/// A proposta pendente `id`, para preparar a prévia ou aplicar.
pub fn pending<'a>(conversation: &'a Conversation, id: &str) -> Result<&'a Proposal> {
    let proposal = conversation
        .proposals
        .iter()
        .find(|proposal| proposal.id == id)
        .ok_or_else(|| Error::ProposalNotFound { id: id.to_owned() })?;
    if proposal.state != ProposalState::Pending {
        return Err(Error::ProposalDecided { id: id.to_owned() });
    }
    Ok(proposal)
}

fn find_mut<'a>(conversation: &'a mut Conversation, id: &str) -> Result<&'a mut Proposal> {
    conversation
        .proposals
        .iter_mut()
        .find(|proposal| proposal.id == id)
        .ok_or_else(|| Error::ProposalNotFound { id: id.to_owned() })
}

/// A IA propôs algo na pergunta atual (a resposta final sem achados ainda tem conteúdo).
#[must_use]
pub fn made_in_last_turn(conversation: &Conversation) -> bool {
    let last = conversation.turns.len().saturating_sub(1);
    conversation
        .proposals
        .iter()
        .any(|proposal| usize::try_from(proposal.turn).is_ok_and(|turn| turn == last))
}
