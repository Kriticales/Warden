//! As conversas (ARCHITECTURE §9.5 "Conversas" e §13): `ai/<pack-id>/<ULID>.json` nos dados
//! locais do Warden, **nunca** na pasta do pack (CA-T14-06).
//!
//! O arquivo guarda tudo **como enviado**. Formato 2 (D-15, ADR-0060): o provedor e o modelo em
//! que a conversa começou (ela fica neles), o histórico no formato comum
//! ([`crate::providers::Message`], com o campo opaco de cada mensagem intacto, como as *thought
//! signatures* do Gemini), o corpo exato enviado em cada rodada, cada resultado de ferramenta,
//! as permissões, o uso de tokens, as respostas conferidas, as propostas com o que foi aplicado
//! ou descartado (D-14) e o retrato do pack no começo (para avisar quando o pack mudou,
//! CA-T14-15). O formato 1 (D-04: só Gemini, com o `contents` da API) é convertido ao abrir, sem
//! mudar um byte do que sai para o Gemini (CA-T14-22).
//! Apagar a conversa apaga o arquivo (e o log escolhido no computador, quando houver).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use warden_core::atomic_write_private;

use crate::error::{Error, Result};
use crate::evidence::CheckedAnswer;
use crate::providers::{AiProvider, Message, SearchMode, gemini};
use crate::tools::{AiPermissions, ToolItem};

/// Versão do formato do arquivo (1: D-04, só Gemini; 2: vários provedores, D-15).
pub const SCHEMA_VERSION: u32 = 2;

/// O sintoma escolhido em "Nova conversa" (`AiSymptom`: o `Symptom` é o da busca do culpado,
/// D-12, e os dois vão para o mesmo `bindings.ts`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum AiSymptom {
    /// Trava ao abrir o jogo.
    CrashOnStart,
    /// Trava ao entrar no mundo.
    CrashInWorld,
    /// Está lento.
    Slow,
    /// Outra coisa.
    Other,
}

/// O que foi analisado.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum ConversationOrigin {
    /// Uma sessão do Testar (o último travamento ou outra).
    #[serde(rename_all = "camelCase")]
    Session {
        /// A sessão.
        session_id: String,
        /// Era o último travamento do pack quando a conversa começou.
        last_crash: bool,
        /// A assinatura do travamento (liga a conversa à linha de Travamentos).
        signature: Option<String>,
    },
    /// Um log ou crash report escolhido no computador (só o nome do arquivo fica guardado).
    #[serde(rename_all = "camelCase")]
    LocalFile {
        /// O nome do arquivo, sem a pasta.
        file_name: String,
    },
    /// Só o pack, sem log.
    PackOnly,
    /// Um log aberto por um link colado, numa conversa sem pack (D-19). O texto puro, já
    /// redigido, fica guardado ao lado da conversa; o link não vai à IA.
    #[serde(rename_all = "camelCase")]
    PasteLink {
        /// O site do link.
        site: PasteSite,
        /// O link como foi colado.
        url: String,
    },
}

/// Os sites de onde uma conversa sem pack abre um log por link (D-19).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum PasteSite {
    /// mclo.gs (o texto puro vem de `api.mclo.gs/1/raw/<id>`).
    Mclogs,
    /// pastebin.com (o texto puro vem de `pastebin.com/raw/<id>`).
    Pastebin,
}

/// O retrato do pack no começo da conversa.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackSnapshot {
    /// Resumo da árvore do pack (`pack.toml`, índice e metafiles).
    pub tree_hash: String,
    /// Cada item (caminho na lista de Mods) e a versão legível.
    pub items: BTreeMap<String, String>,
}

/// O que mudou no pack desde o começo da conversa.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct PackChanges {
    /// Itens com outra versão.
    pub updated: u32,
    /// Itens novos.
    pub added: u32,
    /// Itens que saíram.
    pub removed: u32,
    /// Outras mudanças (configs, `pack.toml`) sem mudar a lista de itens.
    pub other: bool,
}

impl PackChanges {
    /// O pack mudou de algum jeito.
    #[must_use]
    pub fn any(&self) -> bool {
        self.updated + self.added + self.removed > 0 || self.other
    }
}

impl PackSnapshot {
    /// Compara com o retrato de agora.
    #[must_use]
    pub fn changes(&self, now: &PackSnapshot) -> PackChanges {
        let mut changes = PackChanges::default();
        for (path, version) in &self.items {
            match now.items.get(path) {
                None => changes.removed += 1,
                Some(current) if current != version => changes.updated += 1,
                Some(_) => {}
            }
        }
        let added = now
            .items
            .keys()
            .filter(|path| !self.items.contains_key(*path))
            .count();
        changes.added = u32::try_from(added).unwrap_or(u32::MAX);
        changes.other = !changes.any() && self.tree_hash != now.tree_hash;
        changes
    }
}

/// Tokens somados das respostas (cada provedor lê o uso da sua API).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct TokenUsage {
    /// Entrada (no Gemini, `promptTokenCount`).
    #[specta(type = specta_typescript::Number)]
    pub prompt: u64,
    /// Entrada em cache (no Gemini, `cachedContentTokenCount`).
    #[specta(type = specta_typescript::Number)]
    pub cached: u64,
    /// Saída (no Gemini, `candidatesTokenCount`).
    #[specta(type = specta_typescript::Number)]
    pub output: u64,
    /// Raciocínio (no Gemini, `thoughtsTokenCount`, cobrado como saída).
    #[specta(type = specta_typescript::Number)]
    pub thoughts: u64,
}

impl TokenUsage {
    /// Soma.
    pub fn add(&mut self, other: TokenUsage) {
        self.prompt += other.prompt;
        self.cached += other.cached;
        self.output += other.output;
        self.thoughts += other.thoughts;
    }

    /// Total de tokens processados (o que o provedor mede e cobra).
    #[must_use]
    pub fn total(&self) -> u64 {
        self.prompt + self.output + self.thoughts
    }
}

/// Uma consulta da IA e o que foi enviado em resposta ("Enviado à IA").
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ToolStep {
    /// O `id` da chamada.
    pub call_id: Option<String>,
    /// A ferramenta.
    pub name: String,
    /// Os argumentos que a IA pediu (JSON compacto, como vieram).
    pub args: String,
    /// O resultado exatamente como foi na requisição (bytes exatos; no Gemini, a parte
    /// `functionResponse`).
    pub sent: String,
    /// A ferramenta devolveu dados (falso: erro ou não permitida).
    pub ok: bool,
    /// Não permitida nesta conversa.
    pub denied: bool,
    /// O pedido feito a outro serviço para responder ("Enviado ao GitHub", D-14).
    #[serde(default)]
    pub external: Option<crate::external::ExternalRequest>,
}

/// Como a resposta final chegou.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum AnswerVia {
    /// Pela ferramenta `responder`.
    Respond,
    /// Pela requisição de reserva (sem ferramentas, resposta em JSON pelo esquema).
    Fallback,
}

/// A resposta final de uma pergunta.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct FinalAnswer {
    /// Como chegou.
    pub via: AnswerVia,
    /// A IA estourou o limite de rodadas antes de responder.
    pub round_limit: bool,
    /// A resposta conferida.
    pub checked: CheckedAnswer,
    /// O JSON como a IA mandou.
    pub raw: String,
    /// Quando chegou.
    #[specta(type = specta_typescript::Number)]
    pub at_ms: u64,
}

/// Um erro que interrompeu uma pergunta (código e parâmetros da frase, como o `AppError`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct TurnFailure {
    /// Domínio (`ai` ou `core`).
    pub domain: String,
    /// Código (`QUOTA_EXCEEDED`).
    pub code: String,
    /// Parâmetros da frase.
    pub params: BTreeMap<String, String>,
    /// Detalhe técnico.
    pub detail: Option<String>,
    /// Dá para tentar de novo.
    pub retryable: bool,
}

/// Uma pergunta (a primeira é o texto inicial do consentimento) e o que aconteceu.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ConversationTurn {
    /// O texto exato enviado.
    pub question: String,
    /// Quando foi enviado.
    #[specta(type = specta_typescript::Number)]
    pub asked_at_ms: u64,
    /// As consultas da IA, na ordem.
    pub steps: Vec<ToolStep>,
    /// A resposta final.
    pub answer: Option<FinalAnswer>,
    /// O erro que interrompeu (a conversa continua guardada).
    pub failure: Option<TurnFailure>,
    /// O modelo mais leve que respondeu esta pergunta porque a cota diária do modelo da conversa
    /// acabou no meio dela, ou porque ele estourou o tempo-limite (a tela avisa; ADR-0060,
    /// complemento de 19/10/2026).
    #[serde(default)]
    #[specta(optional)]
    pub fallback_model: Option<String>,
    /// Por que a pergunta trocou para [`Self::fallback_model`] (`None` numa conversa gravada
    /// antes da D-15b: a cota diária, a única troca de então).
    #[serde(default)]
    #[specta(optional)]
    pub fallback_reason: Option<FallbackReason>,
    /// Como o Sculk procurou nesta pergunta (ADR-0064): "Procurar rápido" ou "Procurar a fundo"
    /// no Claude e na OpenAI; `None` no Gemini (sem escolha) e nas conversas de antes da escolha
    /// por pergunta.
    #[serde(default)]
    #[specta(optional)]
    pub mode: Option<SearchMode>,
    /// O modelo pedido para esta pergunta (o do modo; `None`: o da conversa, como antes da
    /// escolha por pergunta). A troca para o mais leve fica em `fallback_model`.
    #[serde(default)]
    #[specta(optional)]
    pub model: Option<String>,
}

impl ConversationTurn {
    /// O modelo que respondeu (ou vai responder) esta pergunta: o mais leve da troca, o do modo
    /// ou o da conversa.
    #[must_use]
    pub fn model_or<'a>(&'a self, conversation_model: &'a str) -> &'a str {
        self.fallback_model
            .as_deref()
            .or(self.model.as_deref())
            .unwrap_or(conversation_model)
    }
}

/// Por que uma pergunta trocou para o modelo mais leve.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum FallbackReason {
    /// A cota diária do modelo da conversa acabou.
    Quota,
    /// O modelo da conversa estourou o tempo-limite (D-15b).
    Timeout,
}

/// O corpo exato de uma requisição que voltou com resposta.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SentRound {
    /// Quando foi enviada.
    pub at_ms: u64,
    /// O corpo, byte a byte (JSON).
    pub body: String,
}

/// Uma conversa, como fica no disco.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Conversation {
    /// Versão do formato.
    pub schema_version: u32,
    /// Versão do *system prompt* e das declarações.
    pub prompt_version: u32,
    /// ULID.
    pub id: String,
    /// Quando começou.
    pub created_at_ms: u64,
    /// Última mudança.
    pub updated_at_ms: u64,
    /// O provedor em que a conversa começou (ela fica nele, ADR-0060).
    #[serde(default)]
    pub provider: AiProvider,
    /// O modelo.
    pub model: String,
    /// O modelo mais leve do mesmo provedor para quando a cota diária de [`Self::model`] acabar
    /// (`None`: sem troca; ADR-0060, complemento de 19/10/2026).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fallback_model: Option<String>,
    /// Permissões escolhidas no consentimento.
    pub permissions: AiPermissions,
    /// O sintoma.
    pub symptom: AiSymptom,
    /// O que foi analisado.
    pub origin: ConversationOrigin,
    /// O que a pessoa contou à IA no começo.
    pub note: Option<String>,
    /// O retrato do pack no começo.
    pub pack: PackSnapshot,
    /// Tamanho do pedido inicial contado pelo provedor (no Gemini, `countTokens`).
    pub initial_tokens: Option<u64>,
    /// O histórico no formato comum (sem a mensagem de sistema, que vai em cada pedido).
    pub messages: Vec<Message>,
    /// O corpo exato de cada requisição respondida (repetições iguais seguidas ficam uma vez).
    #[serde(default)]
    pub rounds: Vec<SentRound>,
    /// Tudo o que as ferramentas devolveram (a base da conferência).
    pub sent: Vec<ToolItem>,
    /// As perguntas.
    pub turns: Vec<ConversationTurn>,
    /// Tokens somados.
    pub usage: TokenUsage,
    /// Requisições feitas ao provedor (com as novas tentativas).
    pub requests: u32,
    /// As propostas da IA e o que o usuário decidiu (D-14).
    #[serde(default)]
    pub proposals: Vec<crate::proposals::Proposal>,
    /// Campos que esta versão não conhece: preservados ao gravar.
    #[serde(flatten)]
    pub unknown: BTreeMap<String, Value>,
}

impl Conversation {
    /// Guarda o corpo exato de uma requisição respondida (uma vez, se repetiu o anterior).
    pub fn record_round(&mut self, at_ms: u64, body: &str) {
        if self.rounds.last().is_some_and(|last| last.body == body) {
            return;
        }
        self.rounds.push(SentRound {
            at_ms,
            body: body.to_owned(),
        });
    }
}

/// Converte uma conversa do formato 1 (D-04: só Gemini, `contents`) para o formato 2, no JSON
/// lido do disco. As outras versões passam sem mudar.
fn upgrade(value: &mut Value) {
    let Some(object) = value.as_object_mut() else {
        return;
    };
    let version = object
        .get("schemaVersion")
        .and_then(Value::as_u64)
        .unwrap_or(1);
    if version >= 2 {
        return;
    }
    let contents = match object.remove("contents") {
        Some(Value::Array(contents)) => contents,
        _ => Vec::new(),
    };
    let messages = gemini::translate::messages_from_contents(contents);
    object.insert(
        "messages".to_owned(),
        serde_json::to_value(messages).unwrap_or_default(),
    );
    object.insert("provider".to_owned(), Value::from(AiProvider::Gemini.id()));
    object.insert("schemaVersion".to_owned(), Value::from(SCHEMA_VERSION));
}

/// O caminho como aparece nos erros: só `ai/<pack>/<arquivo>`, nunca a pasta do usuário
/// (QUALITY §9).
fn shown(path: &Path) -> PathBuf {
    let parts: Vec<_> = path.components().rev().take(3).collect();
    parts.into_iter().rev().collect()
}

fn core_error(error: warden_core::CoreError) -> Error {
    match error {
        warden_core::CoreError::Io {
            action,
            path,
            source,
        } => Error::Io {
            action,
            path: shown(&path),
            source,
        },
        other => Error::Core(other),
    }
}

/// A pasta das conversas de um pack.
#[derive(Debug, Clone)]
pub struct ConversationStore {
    dir: PathBuf,
}

/// Um identificador novo (ULID): conversas e prévias do consentimento.
#[must_use]
pub fn new_id() -> String {
    ulid::Ulid::generate().to_string()
}

/// Um identificador de conversa válido (ULID: 26 caracteres de Crockford base32).
#[must_use]
pub fn valid_id(id: &str) -> bool {
    id.len() == 26 && id.parse::<ulid::Ulid>().is_ok()
}

impl ConversationStore {
    /// As conversas em `dir` (`AppPaths::ai_dir(pack)`).
    #[must_use]
    pub fn new(dir: PathBuf) -> Self {
        Self { dir }
    }

    /// A pasta.
    #[must_use]
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    fn file(&self, id: &str) -> Result<PathBuf> {
        if !valid_id(id) {
            return Err(Error::ConversationNotFound { id: id.to_owned() });
        }
        Ok(self.dir.join(format!("{id}.json")))
    }

    /// O log escolhido no computador, já redigido, guardado ao lado da conversa.
    pub fn local_log(&self, id: &str) -> Result<PathBuf> {
        self.file(id).map(|file| file.with_extension("log.txt"))
    }

    /// Grava (atômico, só para o usuário).
    pub fn save(&self, conversation: &Conversation) -> Result<()> {
        let path = self.file(&conversation.id)?;
        std::fs::create_dir_all(&self.dir).map_err(|source| Error::Io {
            action: "criar a pasta",
            path: shown(&self.dir),
            source,
        })?;
        let bytes = serde_json::to_vec_pretty(conversation)
            .map_err(|error| Error::Internal(format!("gravar a conversa: {error}")))?;
        atomic_write_private(&path, &bytes).map_err(core_error)
    }

    /// Grava o log escolhido no computador (já redigido) ao lado da conversa.
    pub fn save_local_log(&self, id: &str, text: &str) -> Result<()> {
        let path = self.local_log(id)?;
        std::fs::create_dir_all(&self.dir).map_err(|source| Error::Io {
            action: "criar a pasta",
            path: shown(&self.dir),
            source,
        })?;
        atomic_write_private(&path, text.as_bytes()).map_err(core_error)
    }

    /// Lê uma conversa.
    pub fn load(&self, id: &str) -> Result<Conversation> {
        let path = self.file(id)?;
        let bytes = match std::fs::read(&path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Err(Error::ConversationNotFound { id: id.to_owned() });
            }
            Err(source) => {
                return Err(Error::Io {
                    action: "ler",
                    path: shown(&path),
                    source,
                });
            }
        };
        let unreadable = |error: serde_json::Error| Error::ConversationUnreadable {
            path: shown(&path),
            reason: error.to_string(),
        };
        let mut value: Value = serde_json::from_slice(&bytes).map_err(unreadable)?;
        upgrade(&mut value);
        serde_json::from_value(value).map_err(unreadable)
    }

    /// Todas as conversas legíveis, da mais nova para a mais antiga (as ilegíveis ficam de
    /// fora, com aviso no registro).
    pub fn list(&self) -> Result<Vec<Conversation>> {
        let entries = match std::fs::read_dir(&self.dir) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(source) => {
                return Err(Error::Io {
                    action: "listar",
                    path: shown(&self.dir),
                    source,
                });
            }
        };
        let mut all = Vec::new();
        for entry in entries.flatten() {
            let name = entry.file_name();
            let Some(id) = name.to_str().and_then(|name| name.strip_suffix(".json")) else {
                continue;
            };
            if !valid_id(id) {
                continue;
            }
            match self.load(id) {
                Ok(conversation) => all.push(conversation),
                Err(error) => tracing::warn!(%error, "conversa ilegível ignorada"),
            }
        }
        all.sort_by(|a, b| b.created_at_ms.cmp(&a.created_at_ms).then(b.id.cmp(&a.id)));
        Ok(all)
    }

    /// Apaga a conversa (e o log guardado ao lado). Apagar o que não existe não é erro.
    pub fn delete(&self, id: &str) -> Result<()> {
        for path in [self.file(id)?, self.local_log(id)?] {
            match std::fs::remove_file(&path) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(source) => {
                    return Err(Error::Io {
                        action: "apagar",
                        path: shown(&path),
                        source,
                    });
                }
            }
        }
        Ok(())
    }
}

/// Uma conversa nova (ainda sem perguntas).
#[must_use]
#[allow(clippy::too_many_arguments)] // os campos do consentimento
pub fn new_conversation(
    id: String,
    now_ms: u64,
    provider: AiProvider,
    model: String,
    permissions: AiPermissions,
    symptom: AiSymptom,
    origin: ConversationOrigin,
    note: Option<String>,
    pack: PackSnapshot,
) -> Conversation {
    Conversation {
        schema_version: SCHEMA_VERSION,
        prompt_version: crate::schema::PROMPT_VERSION,
        id,
        created_at_ms: now_ms,
        updated_at_ms: now_ms,
        provider,
        model,
        fallback_model: None,
        permissions,
        symptom,
        origin,
        note,
        pack,
        initial_tokens: None,
        messages: Vec::new(),
        rounds: Vec::new(),
        sent: Vec::new(),
        turns: Vec::new(),
        usage: TokenUsage::default(),
        requests: 0,
        proposals: Vec::new(),
        unknown: BTreeMap::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn conversation(id: &str, at: u64) -> Conversation {
        new_conversation(
            id.to_owned(),
            at,
            AiProvider::Gemini,
            "modelo".into(),
            AiPermissions::default(),
            AiSymptom::CrashInWorld,
            ConversationOrigin::PackOnly,
            None,
            PackSnapshot::default(),
        )
    }

    #[test]
    fn grava_lista_e_apaga() {
        let dir = tempfile::tempdir().unwrap();
        let store = ConversationStore::new(dir.path().join("ai").join("pack"));
        assert!(store.list().unwrap().is_empty());
        let a = ulid::Ulid::generate().to_string();
        let b = ulid::Ulid::generate().to_string();
        store.save(&conversation(&a, 1)).unwrap();
        store.save(&conversation(&b, 2)).unwrap();
        std::fs::write(store.dir().join("lixo.json"), b"{").unwrap();
        let ids: Vec<String> = store.list().unwrap().into_iter().map(|c| c.id).collect();
        assert_eq!(ids, [b.clone(), a.clone()]);
        std::fs::write(store.local_log(&a).unwrap(), b"log").unwrap();
        store.delete(&a).unwrap();
        assert!(!store.dir().join(format!("{a}.json")).exists());
        assert!(!store.local_log(&a).unwrap().exists());
        assert!(matches!(
            store.load(&a),
            Err(Error::ConversationNotFound { .. })
        ));
        store.delete(&a).unwrap();
    }

    #[test]
    fn erro_sem_a_pasta_do_usuario() {
        let dir = tempfile::tempdir().unwrap();
        let store = ConversationStore::new(dir.path().join("ai").join("pack"));
        let id = ulid::Ulid::generate().to_string();
        std::fs::create_dir_all(store.dir()).unwrap();
        std::fs::write(store.dir().join(format!("{id}.json")), b"{").unwrap();
        let error = store.load(&id).unwrap_err().to_string();
        assert!(
            error.contains(&format!("ai/pack/{id}.json"))
                || error.contains(&format!("ai\\pack\\{id}.json")),
            "{error}"
        );
        assert!(
            !error.contains(&dir.path().display().to_string()),
            "{error}"
        );
    }

    #[test]
    fn identificador_estranho_nao_vira_caminho() {
        let store = ConversationStore::new(PathBuf::from("ai"));
        for id in ["../../x", "", "01J0000000000000000000000/"] {
            assert!(matches!(
                store.load(id),
                Err(Error::ConversationNotFound { .. })
            ));
        }
    }

    #[test]
    fn campos_desconhecidos_sobrevivem() {
        let dir = tempfile::tempdir().unwrap();
        let store = ConversationStore::new(dir.path().to_path_buf());
        let id = ulid::Ulid::generate().to_string();
        let mut json = serde_json::to_value(conversation(&id, 1)).unwrap();
        json["applied"] = serde_json::json!([{ "d14": true }]);
        std::fs::write(store.dir().join(format!("{id}.json")), json.to_string()).unwrap();
        let read = store.load(&id).unwrap();
        store.save(&read).unwrap();
        let again: Value =
            serde_json::from_slice(&std::fs::read(store.dir().join(format!("{id}.json"))).unwrap())
                .unwrap();
        assert_eq!(again["applied"][0]["d14"], true);
    }

    #[test]
    fn origem_do_link_ida_e_volta() {
        // D-19: a origem nova grava o site e o link e volta igual ao abrir.
        let dir = tempfile::tempdir().unwrap();
        let store = ConversationStore::new(dir.path().join("ai").join("_sem-pack"));
        let id = ulid::Ulid::generate().to_string();
        let mut saved = conversation(&id, 1);
        saved.origin = ConversationOrigin::PasteLink {
            site: PasteSite::Mclogs,
            url: "https://mclo.gs/abc123".into(),
        };
        store.save(&saved).unwrap();
        let json: Value =
            serde_json::from_slice(&std::fs::read(store.dir().join(format!("{id}.json"))).unwrap())
                .unwrap();
        assert_eq!(
            json["origin"],
            serde_json::json!({ "kind": "pasteLink", "site": "mclogs", "url": "https://mclo.gs/abc123" })
        );
        assert_eq!(store.load(&id).unwrap().origin, saved.origin);
    }

    #[test]
    fn mudancas_do_pack() {
        // CA-T14-15 (domínio): 3 mods atualizados.
        let before = PackSnapshot {
            tree_hash: "a".into(),
            items: [("a", "1"), ("b", "1"), ("c", "1"), ("d", "1")]
                .map(|(k, v)| (k.to_owned(), v.to_owned()))
                .into(),
        };
        let mut after = before.clone();
        after.tree_hash = "b".into();
        for key in ["a", "b", "c"] {
            after.items.insert(key.into(), "2".into());
        }
        let changes = before.changes(&after);
        assert_eq!(
            changes,
            PackChanges {
                updated: 3,
                added: 0,
                removed: 0,
                other: false
            }
        );
        let mut configs_only = before.clone();
        configs_only.tree_hash = "c".into();
        assert!(before.changes(&configs_only).other);
        assert!(!before.changes(&before).any());
    }
}
