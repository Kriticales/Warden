//! Comandos do domínio `ai` (ARCHITECTURE §4.1 e §9.5; SPEC T14 ✦; ROADMAP D-04).
//!
//! - `ai_settings_get` / `ai_settings_set` / `ai_provider_set`: o provedor de IA ativo (Gemini,
//!   o padrão, OpenAI ou Claude; ADR-0060) e, por provedor, "Mais econômico" ou "Análise mais
//!   cuidadosa" (`ai/settings.json` nos dados do Warden); `ai_models_get`: os modelos que a
//!   chave do provedor ativo alcança, pela lista de modelos da conta (nenhum nome fixo no
//!   código, ADR-0055).
//! - `ai_sources_list`: as sessões do Testar para "Começar pelo"; `ai_log_choose`: o diálogo
//!   nativo para um log ou crash report do computador (o caminho não sai do backend).
//! - `ai_conversation_preview`: o texto inicial **exato** (já redigido), o tamanho, o provedor e
//!   o modelo, para o consentimento. Não manda nada do pack: só a lista de modelos (a chave, uma
//!   vez por execução) e a contagem de tokens do *system prompt* e das ferramentas, sem o texto
//!   do pack (CA-T14-05: cancelar não manda nada).
//! - `ai_conversation_start`: depois do **✦ Começar conversa**, grava a conversa com as
//!   permissões e o texto mostrado (guardado pela prévia, byte a byte), sem rede.
//! - `ai_conversation_run`: roda a pergunta pendente (a primeira, uma nova ou de novo depois de
//!   um erro). Demora: a interface não espera; acompanha pelo evento `ai-conversation-updated`.
//!   Antes da primeira requisição, a contagem de tokens do pedido inteiro. Usa o provedor e a
//!   chave **da conversa**, não o ativo (ADR-0060): sem essa chave, `AI_PROVIDER_KEY_MISSING`.
//! - `ai_conversation_send`: acrescenta uma pergunta (sem rede); a interface chama `run` depois.
//! - `ai_conversations_list`, `ai_conversation_get`, `ai_conversation_delete`.
//! - `ai_standalone_*` (D-19): os mesmos passos para a conversa **sem pack**, sobre um log do
//!   computador ou aberto por um link colado (mclo.gs ou pastebin), em `ai/_sem-pack/`, com o
//!   evento `ai-standalone-updated` ([`crate::ai_tools::standalone`]).
//!
//! As conversas ficam em `ai/<pack-id>/<ULID>.json` nos dados do Warden, nunca na pasta do pack
//! (CA-T14-06).
#![allow(clippy::needless_pass_by_value)] // argumentos pertencem ao contrato do Tauri

use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use tauri::{AppHandle, State, WebviewWindow};
use tauri_plugin_dialog::DialogExt as _;
use tauri_specta::Event;
use warden_ai::conversation::{
    Conversation, ConversationStore, ConversationOrigin, PackChanges, PackSnapshot, AiSymptom, ConversationTurn,
    new_conversation, new_id,
};
use warden_ai::engine::{Engine, Limits, LiveStatus, NoObserver, Observer, now_ms, pending, push_question};
use warden_ai::evidence::ClaimStatus;
use warden_ai::proposals::{Proposal, ProposalState};
use warden_ai::providers::{self, AiProvider, Message, ModelChoices, ModelTier, Provider};
use warden_ai::tools::AiPermissions;
use warden_core::{CancellationToken, PackId, atomic_write_private};
use warden_http::{HttpClient, HttpConfig, SystemTimer};
use warden_launcher::outcome::Outcome;
use warden_secrets::{SecretKind, SecretString};

use crate::ai_tools::initial::{self, Subject};
use crate::ai_tools::{PackTools, registry};
use crate::commands::inventory::pack_root;
use crate::commands::secrets::SecretTester;
use crate::error::AppError;
use crate::state::AppState;

/// Prévias guardadas ao mesmo tempo (a mais velha sai).
const MAX_PREVIEWS: usize = 8;
/// Maior pergunta aceita.
const MAX_QUESTION_CHARS: usize = 8_000;
/// Em build de debug, o E2E troca o diálogo de arquivo pelo caminho escrito neste arquivo (o
/// mesmo do "Adicionar arquivo local").
const E2E_PICK_FILES_ENV: &str = "WARDEN_E2E_PICK_FILES";

fn domain(error: &warden_ai::Error) -> AppError {
    AppError::from_domain(error)
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// `ai-conversation-updated`: uma conversa mudou (pergunta, consulta, resposta, erro, espera).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type, Event)]
#[serde(rename_all = "camelCase")]
#[tauri_specta(event_name = "ai-conversation-updated")]
pub(crate) struct AiConversationUpdated {
    /// O pack.
    pub(crate) pack_id: PackId,
    /// A conversa (ULID).
    pub(crate) conversation_id: String,
}

/// Quem avisa a interface.
pub(crate) type Notify = Arc<dyn Fn(PackId, &str) + Send + Sync>;

pub(crate) fn app_notify(app: AppHandle) -> Notify {
    Arc::new(move |pack_id, id: &str| {
        let event = AiConversationUpdated {
            pack_id,
            conversation_id: id.to_owned(),
        };
        if let Err(error) = event.emit(&app) {
            tracing::warn!(%error, "evento ai-conversation-updated não enviado");
        }
    })
}

/// O texto inicial mostrado no consentimento, guardado até o "Começar conversa".
#[derive(Debug, Clone)]
struct Preview {
    pack_id: PackId,
    symptom: AiSymptom,
    note: Option<String>,
    provider: AiProvider,
    model: String,
    initial: initial::Initial,
    snapshot: PackSnapshot,
    created: u64,
}

/// Uma conversa em que a IA está respondendo.
#[derive(Debug, Clone)]
struct Running {
    cancel: CancellationToken,
    live: LiveStatus,
}

/// Estado da IA em memória (no `AppState`).
#[derive(Debug, Default)]
pub(crate) struct AiHub {
    previews: Mutex<HashMap<String, Preview>>,
    picks: Mutex<HashMap<String, PathBuf>>,
    running: Mutex<HashMap<String, Running>>,
    /// Gravar uma conversa que está rodando e apagar uma conversa não se cruzam: quem grava
    /// confere o cancelamento com ela presa, e quem apaga cancela e apaga com ela presa (a
    /// conversa apagada no meio de uma resposta não volta).
    files: Mutex<()>,
    /// Modelos resolvidos de cada provedor e o resumo da chave que os resolveu.
    models: Mutex<HashMap<AiProvider, (String, ModelChoices)>>,
    /// Tokens do *system prompt* e das ferramentas, por provedor e modelo.
    base_tokens: Mutex<HashMap<(AiProvider, String), u64>>,
    http: Mutex<Option<HttpClient>>,
    /// A busca de issues no GitHub das consultas da D-14 (o cache de 1 h vale para todas as
    /// conversas).
    pub(crate) github: crate::ai_tools::external::GithubHub,
    /// Endereço do provedor nos testes (o servidor simulado).
    #[cfg(test)]
    pub(crate) base_url: Mutex<Option<String>>,
    /// As prévias das conversas sem pack (D-19).
    pub(crate) standalone: crate::ai_tools::standalone::StandaloneHub,
}

impl AiHub {
    fn http(&self) -> Result<HttpClient, AppError> {
        let mut http = lock(&self.http);
        if let Some(client) = http.as_ref() {
            return Ok(client.clone());
        }
        let client = HttpClient::new(HttpConfig::for_version(env!("CARGO_PKG_VERSION")))
            .map_err(|error| AppError::from_domain(&error))?;
        *http = Some(client.clone());
        Ok(client)
    }

    fn client(
        &self,
        provider: AiProvider,
        key: SecretString,
    ) -> Result<Box<dyn Provider>, AppError> {
        let http = self.http()?;
        #[cfg(test)]
        if let Some(base) = lock(&self.base_url).clone() {
            return providers::connect_with_base_url(provider, http, &base, key)
                .map_err(|error| domain(&error));
        }
        providers::connect(provider, http, key).map_err(|error| domain(&error))
    }

    fn live(&self, id: &str) -> Option<LiveStatus> {
        lock(&self.running).get(id).map(|running| running.live)
    }

    fn set_live(&self, id: &str, live: LiveStatus) {
        if let Some(running) = lock(&self.running).get_mut(id) {
            running.live = live;
        }
    }

    fn begin(&self, id: &str) -> Result<CancellationToken, AppError> {
        let mut running = lock(&self.running);
        if running.contains_key(id) {
            return Err(domain(&warden_ai::Error::ConversationBusy { id: id.to_owned() }));
        }
        let cancel = CancellationToken::new();
        running.insert(
            id.to_owned(),
            Running {
                cancel: cancel.clone(),
                live: LiveStatus::Thinking,
            },
        );
        Ok(cancel)
    }

    fn end(&self, id: &str) {
        lock(&self.running).remove(id);
    }

    fn cancel(&self, id: &str) {
        if let Some(running) = lock(&self.running).get(id) {
            running.cancel.cancel();
        }
    }
}

/// Tira a conversa da lista das que estão rodando ao sair, mesmo com erro.
struct RunGuard<'a> {
    hub: &'a AiHub,
    id: String,
}

impl Drop for RunGuard<'_> {
    fn drop(&mut self) {
        self.hub.end(&self.id);
    }
}

/// Uma conversa marcada como ocupada fora do laço (Aplicar e Descartar uma proposta, D-14):
/// nenhuma pergunta nova nem resposta da IA grava por cima enquanto isso, e apagar a conversa
/// no meio não a faz voltar.
pub(crate) struct Busy<'a> {
    hub: &'a AiHub,
    cancel: CancellationToken,
    _guard: RunGuard<'a>,
}

impl<'a> Busy<'a> {
    /// Marca a conversa. Erro `CONVERSATION_BUSY` se a IA está respondendo nela.
    pub(crate) fn begin(state: &'a AppState, id: &str) -> Result<Self, AppError> {
        let cancel = state.ai.begin(id)?;
        Ok(Self {
            hub: &state.ai,
            cancel,
            _guard: RunGuard {
                hub: &state.ai,
                id: id.to_owned(),
            },
        })
    }

    /// Grava a conversa, a menos que ela tenha sido apagada no meio.
    pub(crate) fn save(
        &self,
        store: &ConversationStore,
        conversation: &Conversation,
    ) -> Result<(), AppError> {
        let _files = lock(&self.hub.files);
        if self.cancel.is_cancelled() {
            return Err(domain(&warden_ai::Error::ConversationNotFound {
                id: conversation.id.clone(),
            }));
        }
        store.save(conversation).map_err(|error| domain(&error))
    }
}

// ---------------------------------------------------------------------------------------
// Configurações
// ---------------------------------------------------------------------------------------

/// `ai/settings.json`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AiSettingsFile {
    /// O provedor das conversas novas (sem o campo, o arquivo da D-04: Gemini).
    #[serde(default)]
    ai_provider: AiProvider,
    /// A escolha do Gemini (o campo da D-04, mantido para o arquivo continuar valendo).
    #[serde(default)]
    tier: ModelTier,
    /// A escolha de cada provedor (o Gemini também, quando escolhido depois da D-15).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    tiers: BTreeMap<AiProvider, ModelTier>,
}

impl AiSettingsFile {
    /// A escolha do modelo de um provedor.
    fn tier_of(&self, provider: AiProvider) -> ModelTier {
        match (self.tiers.get(&provider), provider) {
            (Some(tier), _) => *tier,
            (None, AiProvider::Gemini) => self.tier,
            (None, _) => ModelTier::default(),
        }
    }

    fn set_tier(&mut self, provider: AiProvider, tier: ModelTier) {
        self.tiers.insert(provider, tier);
        if provider == AiProvider::Gemini {
            self.tier = tier;
        }
    }
}

/// A chave de cada provedor no cofre.
pub(crate) const fn secret_of(provider: AiProvider) -> SecretKind {
    match provider {
        AiProvider::Gemini => SecretKind::Gemini,
        AiProvider::Openai => SecretKind::Openai,
        AiProvider::Anthropic => SecretKind::Anthropic,
    }
}

fn settings_file(state: &AppState) -> PathBuf {
    state.paths.data_dir().join("ai").join("settings.json")
}

fn load_settings(state: &AppState) -> AiSettingsFile {
    std::fs::read(settings_file(state))
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
}

fn save_settings(state: &AppState, settings: &AiSettingsFile) -> Result<(), AppError> {
    let path = settings_file(state);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| AppError::internal(format!("criar a pasta da IA: {error}")))?;
    }
    let bytes = serde_json::to_vec_pretty(settings)
        .map_err(|error| AppError::internal(error.to_string()))?;
    // O detalhe leva só `ai/settings.json`, nunca a pasta do usuário.
    atomic_write_private(&path, &bytes).map_err(|error| match error {
        warden_core::CoreError::Io { source, .. } => {
            AppError::new(warden_core::CoreErrorCode::Io)
                .with_param("path", "ai/settings.json")
                .with_detail(format!("gravar ai/settings.json: {source}"))
        }
        other => AppError::from(other),
    })
}

/// O que Configurações mostra de um provedor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AiProviderSettings {
    /// O provedor.
    pub(crate) provider: AiProvider,
    /// A escolha do modelo neste provedor.
    pub(crate) tier: ModelTier,
    /// Há chave deste provedor.
    pub(crate) key_configured: bool,
    /// Os modelos já resolvidos nesta execução (sem rede).
    pub(crate) models: Option<ModelChoices>,
}

/// O que Configurações mostra da IA. `tier`, `keyConfigured` e `models` são os do provedor
/// ativo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AiSettingsView {
    /// O provedor das conversas novas.
    pub(crate) provider: AiProvider,
    /// A escolha do modelo no provedor ativo.
    pub(crate) tier: ModelTier,
    /// Há chave do provedor ativo.
    pub(crate) key_configured: bool,
    /// Os modelos do provedor ativo já resolvidos nesta execução (sem rede).
    pub(crate) models: Option<ModelChoices>,
    /// Cada provedor, na ordem da tela.
    pub(crate) providers: Vec<AiProviderSettings>,
}

pub(crate) fn settings_view(state: &AppState) -> Result<AiSettingsView, AppError> {
    let status = state.secrets.status()?;
    let settings = load_settings(state);
    let models = lock(&state.ai.models).clone();
    let providers: Vec<AiProviderSettings> = AiProvider::ALL
        .iter()
        .map(|&provider| AiProviderSettings {
            provider,
            tier: settings.tier_of(provider),
            key_configured: status.is_configured(secret_of(provider)),
            models: models.get(&provider).map(|(_, models)| models.clone()),
        })
        .collect();
    let active = providers
        .iter()
        .find(|item| item.provider == settings.ai_provider)
        .cloned()
        .ok_or_else(|| AppError::internal("provedor ativo fora da lista"))?;
    Ok(AiSettingsView {
        provider: active.provider,
        tier: active.tier,
        key_configured: active.key_configured,
        models: active.models,
        providers,
    })
}

/// O provedor ativo, a escolha do modelo de cada provedor e as chaves configuradas.
#[tauri::command]
#[specta::specta]
pub(crate) fn ai_settings_get(state: State<'_, AppState>) -> Result<AiSettingsView, AppError> {
    settings_view(&state)
}

pub(crate) fn settings_set_impl(
    state: &AppState,
    tier: ModelTier,
) -> Result<AiSettingsView, AppError> {
    let mut settings = load_settings(state);
    let provider = settings.ai_provider;
    settings.set_tier(provider, tier);
    save_settings(state, &settings)?;
    settings_view(state)
}

/// Grava a escolha do modelo **do provedor ativo** (cada provedor guarda a sua).
#[tauri::command]
#[specta::specta]
pub(crate) fn ai_settings_set(
    state: State<'_, AppState>,
    tier: ModelTier,
) -> Result<AiSettingsView, AppError> {
    settings_set_impl(&state, tier)
}

pub(crate) fn provider_set_impl(
    state: &AppState,
    provider: AiProvider,
) -> Result<AiSettingsView, AppError> {
    let mut settings = load_settings(state);
    settings.ai_provider = provider;
    save_settings(state, &settings)?;
    settings_view(state)
}

/// Troca o provedor das conversas novas. As conversas já começadas continuam no provedor delas.
// pendente-na-ui: D-18 a escolha do provedor em Configurações › IA
#[tauri::command]
#[specta::specta]
pub(crate) fn ai_provider_set(
    state: State<'_, AppState>,
    provider: AiProvider,
) -> Result<AiSettingsView, AppError> {
    provider_set_impl(&state, provider)
}

fn key_fingerprint(key: &SecretString) -> String {
    use secrecy::ExposeSecret as _;
    hex::encode(Sha256::digest(key.expose_secret().as_bytes()))
}

/// Os modelos da chave do provedor ativo (rede só na primeira vez, ou com `refresh`).
async fn models_impl(state: &AppState, refresh: bool) -> Result<ModelChoices, AppError> {
    let provider = load_settings(state).ai_provider;
    let key = state.secrets.require(secret_of(provider))?;
    let fingerprint = key_fingerprint(&key);
    if !refresh
        && let Some((cached, models)) = lock(&state.ai.models).get(&provider)
        && *cached == fingerprint
    {
        return Ok(models.clone());
    }
    let client = state.ai.client(provider, key)?;
    let models = client.models().await.map_err(|error| domain(&error))?;
    lock(&state.ai.models).insert(provider, (fingerprint, models.clone()));
    Ok(models)
}

/// Os modelos que a chave do provedor ativo alcança (no Gemini, o Flash-Lite e o Flash
/// estáveis mais novos).
#[tauri::command]
#[specta::specta]
pub(crate) async fn ai_models_get(
    state: State<'_, AppState>,
    refresh: bool,
) -> Result<ModelChoices, AppError> {
    models_impl(&state, refresh).await
}

/// Testador da chave do Gemini (Configurações → Chaves e contas → Testar): o `models.list`.
/// Chave recusada → `ai.KEY_INVALID`.
#[derive(Debug, Default)]
pub(crate) struct GeminiKeyTester {
    /// Endereço da API (`None`: o oficial, ou o de `WARDEN_API_BASE_GEMINI` em debug).
    base_url: Option<String>,
}

impl GeminiKeyTester {
    #[cfg(test)]
    pub(crate) fn with_base_url(base_url: &str) -> Self {
        Self {
            base_url: Some(base_url.to_owned()),
        }
    }
}

#[async_trait]
impl SecretTester for GeminiKeyTester {
    async fn test(&self, value: SecretString) -> Result<(), AppError> {
        let http = HttpClient::new(HttpConfig::for_version(env!("CARGO_PKG_VERSION")))
            .map_err(|error| AppError::from_domain(&error))?;
        let client = match &self.base_url {
            Some(base) => providers::connect_with_base_url(AiProvider::Gemini, http, base, value),
            None => providers::connect(AiProvider::Gemini, http, value),
        }
        .map_err(|error| domain(&error))?;
        client
            .models()
            .await
            .map(|_| ())
            .map_err(|error| domain(&error))
    }
}

// ---------------------------------------------------------------------------------------
// Origem e consentimento
// ---------------------------------------------------------------------------------------

/// Uma sessão do Testar para "Começar pelo".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AiSessionOption {
    /// A sessão.
    pub(crate) session_id: String,
    /// Quando começou.
    #[specta(type = specta_typescript::Number)]
    pub(crate) started_at_ms: u64,
    /// Resultado.
    pub(crate) outcome: Outcome,
    /// Versão do pack.
    pub(crate) pack_version: Option<String>,
}

/// As origens possíveis.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AiSources {
    /// O travamento mais novo (o padrão de "Começar pelo").
    pub(crate) last_crash: Option<AiSessionOption>,
    /// As sessões, da mais nova para a mais antiga (até 20).
    pub(crate) sessions: Vec<AiSessionOption>,
}

/// As sessões do pack que dá para analisar.
#[tauri::command]
#[specta::specta]
pub(crate) async fn ai_sources_list(
    state: State<'_, AppState>,
    pack_id: PackId,
) -> Result<AiSources, AppError> {
    pack_root(&state, pack_id)?;
    let sessions = crate::test_session::sessions_of(&state.paths, pack_id)?;
    let option = |session: &crate::test_session::TestSessionSummary| AiSessionOption {
        session_id: session.id.clone(),
        started_at_ms: session.started_at_ms,
        outcome: session.outcome,
        pack_version: session.pack_version.clone(),
    };
    Ok(AiSources {
        last_crash: sessions
            .iter()
            .find(|session| session.outcome == Outcome::Crashed)
            .map(option),
        sessions: sessions.iter().take(20).map(option).collect(),
    })
}

/// Um arquivo escolhido no computador (o caminho fica no backend).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AiLocalPick {
    /// Identificador da escolha, para a prévia.
    pub(crate) token: String,
    /// O nome do arquivo, sem a pasta.
    pub(crate) file_name: String,
    /// Tamanho.
    #[specta(type = specta_typescript::Number)]
    pub(crate) bytes: u64,
}

/// "Um log ou crash report do computador…": o diálogo nativo. `None`: desistiu.
#[tauri::command]
#[specta::specta]
pub(crate) async fn ai_log_choose(
    app: AppHandle,
    window: WebviewWindow,
    state: State<'_, AppState>,
    pack_id: PackId,
) -> Result<Option<AiLocalPick>, AppError> {
    pack_root(&state, pack_id)?;
    let Some(path) = pick_log(&app, &window).await? else {
        return Ok(None);
    };
    Ok(Some(remember_pick(&state, path)?))
}

pub(crate) fn remember_pick(state: &AppState, path: PathBuf) -> Result<AiLocalPick, AppError> {
    let meta = std::fs::metadata(&path)
        .map_err(|_| AppError::new(warden_core::CoreErrorCode::Io).with_param("path", "arquivo"))?;
    let file_name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let token = new_id();
    lock(&state.ai.picks).insert(token.clone(), path);
    Ok(AiLocalPick {
        token,
        file_name,
        bytes: meta.len(),
    })
}

async fn pick_log(app: &AppHandle, window: &WebviewWindow) -> Result<Option<PathBuf>, AppError> {
    #[cfg(debug_assertions)]
    if let Some(file) = std::env::var_os(E2E_PICK_FILES_ENV).filter(|value| !value.is_empty()) {
        let picked = std::fs::read_to_string(&file).unwrap_or_default();
        return Ok(picked
            .lines()
            .map(str::trim)
            .find(|line| !line.is_empty())
            .map(PathBuf::from));
    }
    #[cfg(not(debug_assertions))]
    let _ = E2E_PICK_FILES_ENV;
    let (sender, receiver) = tokio::sync::oneshot::channel();
    app.dialog()
        .file()
        .set_title("Escolha um log ou crash report")
        .add_filter("Logs e crash reports", &["log", "txt"])
        .set_parent(window)
        .pick_file(move |picked| {
            let _ = sender.send(picked.and_then(|path| path.into_path().ok()));
        });
    receiver
        .await
        .map_err(|error| AppError::internal(format!("diálogo de arquivo interrompido: {error}")))
}

/// O que analisar.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub(crate) enum AiSource {
    /// O travamento mais novo.
    LastCrash,
    /// Uma sessão.
    #[serde(rename_all = "camelCase")]
    Session {
        /// A sessão.
        session_id: String,
    },
    /// O arquivo escolhido por `ai_log_choose`.
    #[serde(rename_all = "camelCase")]
    LocalFile {
        /// O `token` da escolha.
        token: String,
    },
    /// Só o pack.
    PackOnly,
}

/// O pedido da prévia.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AiPreviewRequest {
    /// "O que está acontecendo?".
    pub(crate) symptom: AiSymptom,
    /// "Começar pelo".
    pub(crate) source: AiSource,
    /// "Contar algo à IA".
    pub(crate) note: Option<String>,
}

/// O consentimento.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AiPreview {
    /// Para o `ai_conversation_start`.
    pub(crate) preview_id: String,
    /// O texto inicial exato, já redigido.
    pub(crate) text: String,
    /// Tamanho do texto em bytes (UTF-8).
    #[specta(type = specta_typescript::Number)]
    pub(crate) bytes: u64,
    /// Tamanho inicial estimado em tokens: a contagem do *system prompt* e das ferramentas
    /// pelo provedor mais o texto estimado (`None`: o provedor não respondeu agora ou não conta).
    #[specta(type = Option<specta_typescript::Number>)]
    pub(crate) estimated_tokens: Option<u64>,
    /// O provedor (o consentimento mostra o nome dele).
    pub(crate) provider: AiProvider,
    /// O modelo.
    pub(crate) model: String,
    /// Mais econômico ou mais cuidadoso (custo esperado).
    pub(crate) tier: Option<ModelTier>,
    /// A busca no GitHub está disponível nesta versão (a ferramenta `search_mod_issues` da D-14
    /// está no registro).
    pub(crate) github_available: bool,
}

fn resolve_subject(
    state: &AppState,
    pack_id: PackId,
    source: &AiSource,
) -> Result<Subject, AppError> {
    let not_found = |id: &str| {
        AppError::new(crate::error::AppErrorCode::TestSessionNotFound).with_param("sessionId", id)
    };
    match source {
        AiSource::PackOnly => Ok(Subject::PackOnly),
        AiSource::LastCrash => {
            let sessions = crate::test_session::sessions_of(&state.paths, pack_id)?;
            let last = sessions
                .into_iter()
                .find(|session| session.outcome == Outcome::Crashed)
                .ok_or_else(|| not_found("último travamento"))?;
            Ok(Subject::Session {
                summary: Box::new(last),
                last_crash: true,
            })
        }
        AiSource::Session { session_id } => {
            let sessions = crate::test_session::sessions_of(&state.paths, pack_id)?;
            let newest_crash = sessions
                .iter()
                .find(|session| session.outcome == Outcome::Crashed)
                .map(|session| session.id.clone());
            let summary = sessions
                .into_iter()
                .find(|session| &session.id == session_id)
                .ok_or_else(|| not_found(session_id))?;
            Ok(Subject::Session {
                last_crash: newest_crash.as_deref() == Some(session_id.as_str()),
                summary: Box::new(summary),
            })
        }
        AiSource::LocalFile { token } => {
            let path = lock(&state.ai.picks)
                .get(token)
                .cloned()
                .ok_or_else(|| domain(&warden_ai::Error::PreviewExpired))?;
            let raw = crate::ai_tools::read_log_text(&path).map_err(|error| {
                AppError::new(warden_core::CoreErrorCode::Io)
                    .with_param("path", "arquivo")
                    .with_detail(error.to_string())
            })?;
            let file_name = path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default();
            Ok(Subject::LocalFile { file_name, raw })
        }
    }
}

/// Tokens do *system prompt* e das ferramentas, sem nada do pack (uma vez por modelo).
async fn base_tokens(state: &AppState, client: &dyn Provider, model: &str) -> Option<u64> {
    let cache = (client.id(), model.to_owned());
    if let Some(count) = lock(&state.ai.base_tokens).get(&cache) {
        return Some(*count);
    }
    let registry = registry();
    let source = NoSource;
    let engine = Engine {
        provider: client,
        registry: &registry,
        source: &source,
        observer: &NoObserver,
        timer: Arc::new(SystemTimer::new()),
        cancel: CancellationToken::new(),
        limits: Limits::default(),
    };
    match engine.count_tokens(model, &[Message::user(".")]).await {
        Ok(Some(count)) => {
            lock(&state.ai.base_tokens).insert(cache, count);
            Some(count)
        }
        Ok(None) => None,
        Err(error) => {
            tracing::info!(%error, "contagem de tokens da base não respondeu");
            None
        }
    }
}

/// Fonte vazia (só para contar os tokens da base).
struct NoSource;

#[async_trait]
impl warden_ai::tools::ToolSource for NoSource {
    async fn call(
        &self,
        _name: &str,
        _args: &serde_json::Value,
    ) -> Result<warden_ai::tools::ToolOutput, String> {
        Err("indisponível".to_owned())
    }
    async fn known_mods(&self) -> warden_ai::tools::KnownMods {
        warden_ai::tools::KnownMods::default()
    }
}

pub(crate) async fn preview_impl(
    state: &AppState,
    pack_id: PackId,
    request: AiPreviewRequest,
) -> Result<AiPreview, AppError> {
    let root = pack_root(state, pack_id)?;
    let settings = load_settings(state);
    let provider = settings.ai_provider;
    let key = state.secrets.require(secret_of(provider))?;
    let models = models_impl(state, false).await?;
    let (model, _) = models
        .pick(settings.tier_of(provider))
        .ok_or_else(|| domain(&warden_ai::Error::NoModel))?;
    let model = model.to_owned();
    let subject = resolve_subject(state, pack_id, &request.source)?;
    let note = request
        .note
        .as_deref()
        .map(str::trim)
        .filter(|note| !note.is_empty())
        .map(|note| note.chars().take(MAX_QUESTION_CHARS).collect::<String>());
    let tools = PackTools::new(state, pack_id, root.clone(), None);
    let built = initial::build(&tools, request.symptom, subject, note.as_deref()).await?;
    let client = state.ai.client(provider, key)?;
    let base = base_tokens(state, client.as_ref(), &model).await;
    let bytes = u64::try_from(built.text.len()).unwrap_or(u64::MAX);
    let snapshot = {
        let root = root.clone();
        tauri::async_runtime::spawn_blocking(move || initial::snapshot(&root))
            .await
            .unwrap_or_default()
    };
    let preview_id = new_id();
    let view = AiPreview {
        preview_id: preview_id.clone(),
        text: built.text.clone(),
        bytes,
        estimated_tokens: base.map(|base| base + bytes.div_ceil(4)),
        tier: provider.tier_of(&model),
        provider,
        model: model.clone(),
        github_available: registry()
            .get(warden_ai::external::SEARCH_MOD_ISSUES)
            .is_some(),
    };
    let mut previews = lock(&state.ai.previews);
    if previews.len() >= MAX_PREVIEWS
        && let Some(oldest) = previews
            .iter()
            .min_by_key(|(_, preview)| preview.created)
            .map(|(id, _)| id.clone())
    {
        previews.remove(&oldest);
    }
    previews.insert(
        preview_id,
        Preview {
            pack_id,
            symptom: request.symptom,
            note,
            provider,
            model,
            initial: built,
            snapshot,
            created: now_ms(),
        },
    );
    Ok(view)
}

/// O texto inicial exato e o que a IA poderá consultar, para o consentimento.
#[tauri::command]
#[specta::specta]
pub(crate) async fn ai_conversation_preview(
    state: State<'_, AppState>,
    pack_id: PackId,
    request: AiPreviewRequest,
) -> Result<AiPreview, AppError> {
    preview_impl(&state, pack_id, request).await
}

fn store(state: &AppState, pack_id: PackId) -> ConversationStore {
    ConversationStore::new(state.paths.ai_dir(pack_id))
}

pub(crate) fn start_impl(
    state: &AppState,
    pack_id: PackId,
    preview_id: &str,
    permissions: AiPermissions,
) -> Result<String, AppError> {
    pack_root(state, pack_id)?;
    let preview = lock(&state.ai.previews)
        .remove(preview_id)
        .filter(|preview| preview.pack_id == pack_id)
        .ok_or_else(|| domain(&warden_ai::Error::PreviewExpired))?;
    let id = new_id();
    let mut conversation = new_conversation(
        id.clone(),
        now_ms(),
        preview.provider,
        preview.model,
        permissions,
        preview.symptom,
        preview.initial.origin,
        preview.note,
        preview.snapshot,
    );
    push_question(&mut conversation, preview.initial.text);
    let store = store(state, pack_id);
    if let Some(log) = &preview.initial.local_log {
        store
            .save_local_log(&id, log)
            .map_err(|error| domain(&error))?;
    }
    store.save(&conversation).map_err(|error| domain(&error))?;
    Ok(id)
}

/// "✦ Começar conversa": grava a conversa com as permissões escolhidas e o texto mostrado.
/// Não manda nada; a interface chama `ai_conversation_run` em seguida.
#[tauri::command]
#[specta::specta]
pub(crate) fn ai_conversation_start(
    state: State<'_, AppState>,
    pack_id: PackId,
    preview_id: String,
    permissions: AiPermissions,
) -> Result<String, AppError> {
    start_impl(&state, pack_id, &preview_id, permissions)
}

// ---------------------------------------------------------------------------------------
// O laço
// ---------------------------------------------------------------------------------------

/// Grava a conversa a cada mudança e avisa a interface.
struct AppObserver<'a> {
    hub: &'a AiHub,
    store: ConversationStore,
    pack_id: PackId,
    id: String,
    notify: Notify,
    cancel: CancellationToken,
}

/// Grava a conversa que está rodando, a menos que ela tenha sido apagada (cancelada).
fn save_running(hub: &AiHub, store: &ConversationStore, cancel: &CancellationToken, conversation: &Conversation) {
    let _files = lock(&hub.files);
    if cancel.is_cancelled() {
        return;
    }
    if let Err(error) = store.save(conversation) {
        tracing::warn!(%error, "conversa não gravada");
    }
}

#[async_trait]
impl Observer for AppObserver<'_> {
    async fn changed(&self, conversation: &Conversation) {
        save_running(self.hub, &self.store, &self.cancel, conversation);
        (self.notify)(self.pack_id, &self.id);
    }

    fn live(&self, live: LiveStatus) {
        self.hub.set_live(&self.id, live);
        (self.notify)(self.pack_id, &self.id);
    }
}

pub(crate) async fn run_impl(
    state: &AppState,
    pack_id: PackId,
    id: &str,
    notify: Notify,
) -> Result<(), AppError> {
    let root = pack_root(state, pack_id)?;
    let store = store(state, pack_id);
    let mut conversation = store.load(id).map_err(|error| domain(&error))?;
    if !pending(&conversation) {
        return Ok(());
    }
    let cancel = state.ai.begin(id)?;
    let guard = RunGuard {
        hub: &state.ai,
        id: id.to_owned(),
    };
    notify(pack_id, id);
    let result = async {
        let key = conversation_key(state, conversation.provider)?;
        let client = state.ai.client(conversation.provider, key)?;
        let local_log = matches!(conversation.origin, ConversationOrigin::LocalFile { .. })
            .then(|| store.local_log(id).ok())
            .flatten();
        let tools = PackTools::new(state, pack_id, root, local_log).with_cancel(cancel.clone());
        let registry = registry();
        let observer = AppObserver {
            hub: &state.ai,
            store: store.clone(),
            pack_id,
            id: id.to_owned(),
            notify: Arc::clone(&notify),
            cancel: cancel.clone(),
        };
        let engine = Engine {
            provider: client.as_ref(),
            registry: &registry,
            source: &tools,
            observer: &observer,
            timer: Arc::new(SystemTimer::new()),
            cancel: cancel.clone(),
            limits: Limits::default(),
        };
        if conversation.initial_tokens.is_none() && conversation.requests == 0 {
            // O tamanho exato do pedido inicial, contado pelo provedor (sistema, ferramentas e o
            // texto), já depois do consentimento.
            match engine
                .count_tokens(&conversation.model, &conversation.messages)
                .await
            {
                Ok(count) => conversation.initial_tokens = count,
                Err(error) => tracing::info!(%error, "contagem do pedido inicial falhou"),
            }
        }
        engine
            .run(&mut conversation)
            .await
            .map_err(|error| domain(&error))
    }
    .await;
    if let Err(error) = &result
        && !matches!(
            conversation.turns.last(),
            Some(ConversationTurn {
                failure: Some(_),
                ..
            })
        )
    {
        // Erro antes do laço (chave, modelo): fica gravado na pergunta.
        if let Some(turn) = conversation.turns.last_mut() {
            turn.failure = Some(failure_from_app(error));
        }
        save_running(&state.ai, &store, &cancel, &conversation);
    }
    drop(guard);
    notify(pack_id, id);
    result
}

/// A chave do provedor **da conversa** (ADR-0060). Sem ela: o erro de sempre quando é o
/// provedor ativo; `AI_PROVIDER_KEY_MISSING` quando a conversa é de outro provedor (a pessoa
/// trocou de provedor depois de começar).
fn conversation_key(state: &AppState, provider: AiProvider) -> Result<SecretString, AppError> {
    match state.secrets.require(secret_of(provider)) {
        Err(warden_secrets::SecretsError::NotConfigured { .. })
            if load_settings(state).ai_provider != provider =>
        {
            Err(domain(&warden_ai::Error::ProviderKeyMissing { provider }))
        }
        other => other.map_err(AppError::from),
    }
}

fn failure_from_app(error: &AppError) -> warden_ai::conversation::TurnFailure {
    let code = serde_json::to_value(error.code).unwrap_or_default();
    warden_ai::conversation::TurnFailure {
        domain: code
            .get("domain")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("app")
            .to_owned(),
        code: code
            .get("code")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("INTERNAL")
            .to_owned(),
        params: error.params.clone(),
        detail: error.detail.clone(),
        retryable: error.retryable,
    }
}

/// Roda a pergunta pendente (a primeira, uma nova ou de novo depois de um erro). Volta quando a
/// IA responde ou para; a interface acompanha pelo `ai-conversation-updated`.
#[tauri::command]
#[specta::specta]
pub(crate) async fn ai_conversation_run(
    app: AppHandle,
    state: State<'_, AppState>,
    pack_id: PackId,
    conversation_id: String,
) -> Result<(), AppError> {
    run_impl(&state, pack_id, &conversation_id, app_notify(app)).await
}

pub(crate) fn send_impl(
    state: &AppState,
    pack_id: PackId,
    id: &str,
    text: &str,
) -> Result<(), AppError> {
    pack_root(state, pack_id)?;
    let text = text.trim();
    if text.is_empty() || text.chars().count() > MAX_QUESTION_CHARS {
        return Err(AppError::new(warden_project::ProjectErrorCode::InvalidInput)
            .with_param("field", "text"));
    }
    let store = store(state, pack_id);
    let mut conversation = store.load(id).map_err(|error| domain(&error))?;
    if state.ai.live(id).is_some() || pending(&conversation) {
        return Err(domain(&warden_ai::Error::ConversationBusy { id: id.to_owned() }));
    }
    push_question(&mut conversation, text.to_owned());
    store.save(&conversation).map_err(|error| domain(&error))
}

/// Acrescenta uma pergunta à conversa (sem rede); a interface chama `ai_conversation_run` em
/// seguida.
#[tauri::command]
#[specta::specta]
pub(crate) fn ai_conversation_send(
    app: AppHandle,
    state: State<'_, AppState>,
    pack_id: PackId,
    conversation_id: String,
    text: String,
) -> Result<(), AppError> {
    send_impl(&state, pack_id, &conversation_id, &text)?;
    app_notify(app)(pack_id, &conversation_id);
    Ok(())
}

// ---------------------------------------------------------------------------------------
// Conversas
// ---------------------------------------------------------------------------------------

/// A situação de uma conversa na lista.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) enum AiConversationState {
    /// A IA está respondendo.
    Running,
    /// A última pergunta foi respondida.
    Answered,
    /// A última pergunta parou num erro.
    Failed,
    /// A última pergunta ficou sem resposta (o app fechou no meio).
    Interrupted,
}

/// Uma linha de "Conversas".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AiConversationSummary {
    /// ULID.
    pub(crate) id: String,
    /// A causa provável (a primeira afirmação verificada, ou o resumo), quando houver.
    pub(crate) title: Option<String>,
    /// O sintoma.
    pub(crate) symptom: AiSymptom,
    /// O que foi analisado.
    pub(crate) origin: ConversationOrigin,
    /// Quando começou.
    #[specta(type = specta_typescript::Number)]
    pub(crate) created_at_ms: u64,
    /// Última mudança.
    #[specta(type = specta_typescript::Number)]
    pub(crate) updated_at_ms: u64,
    /// A situação.
    pub(crate) state: AiConversationState,
    /// Perguntas feitas.
    pub(crate) questions: u32,
    /// Tokens processados.
    #[specta(type = specta_typescript::Number)]
    pub(crate) tokens: u64,
    /// Propostas da IA esperando a decisão do usuário (D-14).
    pub(crate) pending_proposals: u32,
}

fn title_of(conversation: &Conversation) -> Option<String> {
    let answer = conversation
        .turns
        .iter()
        .find_map(|turn| turn.answer.as_ref())?;
    let claim = answer
        .checked
        .claims
        .iter()
        .find(|claim| claim.status == ClaimStatus::Verified)
        .map(|claim| claim.statement.clone());
    let title = claim.unwrap_or_else(|| answer.checked.summary.clone());
    let title = title.trim();
    (!title.is_empty()).then(|| {
        let mut short: String = title.chars().take(120).collect();
        if title.chars().count() > 120 {
            short.push('…');
        }
        short
    })
}

fn state_of(hub: &AiHub, conversation: &Conversation) -> AiConversationState {
    if hub.live(&conversation.id).is_some() {
        return AiConversationState::Running;
    }
    match conversation.turns.last() {
        Some(ConversationTurn {
            answer: Some(_), ..
        }) => AiConversationState::Answered,
        Some(ConversationTurn {
            failure: Some(_), ..
        }) => AiConversationState::Failed,
        _ => AiConversationState::Interrupted,
    }
}

fn summary_of(hub: &AiHub, conversation: &Conversation) -> AiConversationSummary {
    AiConversationSummary {
        id: conversation.id.clone(),
        title: title_of(conversation),
        symptom: conversation.symptom,
        origin: conversation.origin.clone(),
        created_at_ms: conversation.created_at_ms,
        updated_at_ms: conversation.updated_at_ms,
        state: state_of(hub, conversation),
        questions: u32::try_from(conversation.turns.len()).unwrap_or(u32::MAX),
        tokens: conversation.usage.total(),
        pending_proposals: u32::try_from(
            conversation
                .proposals
                .iter()
                .filter(|proposal| proposal.state == ProposalState::Pending)
                .count(),
        )
        .unwrap_or(u32::MAX),
    }
}

pub(crate) fn list_impl(
    state: &AppState,
    pack_id: PackId,
) -> Result<Vec<AiConversationSummary>, AppError> {
    pack_root(state, pack_id)?;
    Ok(store(state, pack_id)
        .list()
        .map_err(|error| domain(&error))?
        .iter()
        .map(|conversation| summary_of(&state.ai, conversation))
        .collect())
}

/// As conversas do pack, da mais nova para a mais antiga.
#[tauri::command]
#[specta::specta]
pub(crate) async fn ai_conversations_list(
    state: State<'_, AppState>,
    pack_id: PackId,
) -> Result<Vec<AiConversationSummary>, AppError> {
    list_impl(&state, pack_id)
}

/// Uma conversa aberta.
#[derive(Debug, Clone, PartialEq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AiConversationView {
    /// O resumo (título, situação…).
    pub(crate) summary: AiConversationSummary,
    /// O provedor em que a conversa começou (ela fica nele).
    pub(crate) provider: AiProvider,
    /// O modelo.
    pub(crate) model: String,
    /// Mais econômico ou mais cuidadoso.
    pub(crate) tier: Option<ModelTier>,
    /// As permissões escolhidas.
    pub(crate) permissions: AiPermissions,
    /// O que a pessoa contou.
    pub(crate) note: Option<String>,
    /// Tamanho do pedido inicial contado pelo provedor.
    #[specta(type = Option<specta_typescript::Number>)]
    pub(crate) initial_tokens: Option<u64>,
    /// Requisições feitas.
    pub(crate) requests: u32,
    /// As perguntas, com as consultas e as respostas.
    pub(crate) turns: Vec<ConversationTurn>,
    /// O que a conversa está fazendo agora.
    pub(crate) live: Option<LiveStatus>,
    /// O que mudou no pack desde o começo da conversa.
    pub(crate) pack_changes: PackChanges,
    /// As propostas da IA (cartões com Aplicar e Descartar, D-14).
    pub(crate) proposals: Vec<Proposal>,
}

pub(crate) async fn get_impl(
    state: &AppState,
    pack_id: PackId,
    id: &str,
) -> Result<AiConversationView, AppError> {
    let root = pack_root(state, pack_id)?;
    let conversation = store(state, pack_id)
        .load(id)
        .map_err(|error| domain(&error))?;
    let now = tauri::async_runtime::spawn_blocking(move || initial::snapshot(&root))
        .await
        .unwrap_or_default();
    Ok(AiConversationView {
        summary: summary_of(&state.ai, &conversation),
        tier: conversation.provider.tier_of(&conversation.model),
        provider: conversation.provider,
        model: conversation.model.clone(),
        permissions: conversation.permissions,
        note: conversation.note.clone(),
        initial_tokens: conversation.initial_tokens,
        requests: conversation.requests,
        live: state.ai.live(id),
        pack_changes: conversation.pack.changes(&now),
        turns: conversation.turns,
        proposals: conversation.proposals,
    })
}

/// Uma conversa com as mensagens, as consultas e o que mudou no pack desde então.
#[tauri::command]
#[specta::specta]
pub(crate) async fn ai_conversation_get(
    state: State<'_, AppState>,
    pack_id: PackId,
    conversation_id: String,
) -> Result<AiConversationView, AppError> {
    get_impl(&state, pack_id, &conversation_id).await
}

pub(crate) fn delete_impl(state: &AppState, pack_id: PackId, id: &str) -> Result<(), AppError> {
    pack_root(state, pack_id)?;
    let _files = lock(&state.ai.files);
    state.ai.cancel(id);
    store(state, pack_id)
        .delete(id)
        .map_err(|error| domain(&error))
}

/// Apaga a conversa (o arquivo dela nos dados do Warden). Se a IA estava respondendo, para.
#[tauri::command]
#[specta::specta]
pub(crate) fn ai_conversation_delete(
    app: AppHandle,
    state: State<'_, AppState>,
    pack_id: PackId,
    conversation_id: String,
) -> Result<(), AppError> {
    delete_impl(&state, pack_id, &conversation_id)?;
    app_notify(app)(pack_id, &conversation_id);
    Ok(())
}

// ---------------------------------------------------------------------------------------
// Conversa sem pack (D-19)
// ---------------------------------------------------------------------------------------

/// `ai-standalone-updated`: uma conversa sem pack mudou (pergunta, consulta, resposta, erro).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type, Event)]
#[serde(rename_all = "camelCase")]
#[tauri_specta(event_name = "ai-standalone-updated")]
pub(crate) struct AiStandaloneUpdated {
    /// A conversa (ULID).
    pub(crate) conversation_id: String,
}

/// Quem avisa a interface das conversas sem pack.
pub(crate) type StandaloneNotify = Arc<dyn Fn(&str) + Send + Sync>;

fn standalone_notify(app: AppHandle) -> StandaloneNotify {
    Arc::new(move |id: &str| {
        let event = AiStandaloneUpdated {
            conversation_id: id.to_owned(),
        };
        if let Err(error) = event.emit(&app) {
            tracing::warn!(%error, "evento ai-standalone-updated não enviado");
        }
    })
}

/// "Um log ou crash report do computador…" sem pack: o diálogo nativo. `None`: desistiu.
#[tauri::command]
#[specta::specta]
pub(crate) async fn ai_standalone_log_choose(
    app: AppHandle,
    window: WebviewWindow,
    state: State<'_, AppState>,
) -> Result<Option<AiLocalPick>, AppError> {
    let Some(path) = pick_log(&app, &window).await? else {
        return Ok(None);
    };
    Ok(Some(remember_pick(&state, path)?))
}

/// O que analisar sem pack.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub(crate) enum AiStandaloneSource {
    /// O arquivo escolhido por `ai_standalone_log_choose`.
    #[serde(rename_all = "camelCase")]
    LocalFile {
        /// O `token` da escolha.
        token: String,
    },
    /// Um link colado (mclo.gs ou pastebin).
    #[serde(rename_all = "camelCase")]
    Link {
        /// O link como foi colado.
        url: String,
    },
}

/// O pedido da prévia de uma conversa sem pack.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AiStandalonePreviewRequest {
    /// "O que está acontecendo?".
    pub(crate) symptom: AiSymptom,
    /// O arquivo ou o link.
    pub(crate) source: AiStandaloneSource,
    /// "Contar algo à IA".
    pub(crate) note: Option<String>,
}

/// O log da prévia sem pack, lido do computador ou baixado do link.
async fn standalone_subject(
    state: &AppState,
    source: &AiStandaloneSource,
) -> Result<crate::ai_tools::standalone::Subject, AppError> {
    use crate::ai_tools::standalone::{
        LINK_TIMEOUT, PasteBases, Subject, fetch, parse_link,
    };
    match source {
        AiStandaloneSource::LocalFile { token } => {
            let path = lock(&state.ai.picks)
                .get(token)
                .cloned()
                .ok_or_else(|| domain(&warden_ai::Error::PreviewExpired))?;
            let truncated = std::fs::metadata(&path)
                .is_ok_and(|meta| meta.len() > crate::ai_tools::MAX_LOG_BYTES);
            let raw = crate::ai_tools::read_log_text(&path).map_err(|error| {
                AppError::new(warden_core::CoreErrorCode::Io)
                    .with_param("path", "arquivo")
                    .with_detail(error.to_string())
            })?;
            let file_name = path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default();
            Ok(Subject {
                origin: ConversationOrigin::LocalFile { file_name },
                raw,
                truncated,
            })
        }
        AiStandaloneSource::Link { url } => {
            // Conferido antes de qualquer rede: site fora da lista não faz requisição.
            let link = parse_link(url)?;
            let fetched = fetch(
                &state.ai.http()?,
                &link,
                &PasteBases::current(),
                crate::ai_tools::MAX_LOG_BYTES,
                LINK_TIMEOUT,
                &CancellationToken::new(),
            )
            .await?;
            Ok(Subject {
                origin: ConversationOrigin::PasteLink {
                    site: link.site,
                    url: link.url,
                },
                raw: fetched.text,
                truncated: fetched.truncated,
            })
        }
    }
}

pub(crate) async fn standalone_preview_impl(
    state: &AppState,
    request: AiStandalonePreviewRequest,
) -> Result<AiPreview, AppError> {
    use crate::ai_tools::standalone::{StandalonePreview, StandaloneTools, build_initial};
    // Um link recusado não faz nenhuma requisição (nem à IA).
    if let AiStandaloneSource::Link { url } = &request.source {
        crate::ai_tools::standalone::parse_link(url)?;
    }
    let settings = load_settings(state);
    let provider = settings.ai_provider;
    let key = state.secrets.require(secret_of(provider))?;
    let models = models_impl(state, false).await?;
    let (model, _) = models
        .pick(settings.tier_of(provider))
        .ok_or_else(|| domain(&warden_ai::Error::NoModel))?;
    let model = model.to_owned();
    let subject = standalone_subject(state, &request.source).await?;
    let note = request
        .note
        .as_deref()
        .map(str::trim)
        .filter(|note| !note.is_empty())
        .map(|note| note.chars().take(MAX_QUESTION_CHARS).collect::<String>());
    let tools = StandaloneTools::new(state, None, CancellationToken::new());
    let built = build_initial(tools.redactor(), request.symptom, subject, note.as_deref());
    let client = state.ai.client(provider, key)?;
    let base = base_tokens(state, client.as_ref(), &model).await;
    let bytes = u64::try_from(built.text.len()).unwrap_or(u64::MAX);
    let preview_id = new_id();
    let view = AiPreview {
        preview_id: preview_id.clone(),
        text: built.text.clone(),
        bytes,
        estimated_tokens: base.map(|base| base + bytes.div_ceil(4)),
        tier: provider.tier_of(&model),
        provider,
        model: model.clone(),
        github_available: crate::ai_tools::standalone::registry()
            .get(warden_ai::external::SEARCH_MOD_ISSUES)
            .is_some(),
    };
    let mut previews = lock(&state.ai.standalone.previews);
    if previews.len() >= MAX_PREVIEWS
        && let Some(oldest) = previews
            .iter()
            .min_by_key(|(_, preview)| preview.created)
            .map(|(id, _)| id.clone())
    {
        previews.remove(&oldest);
    }
    previews.insert(
        preview_id,
        StandalonePreview {
            symptom: request.symptom,
            note,
            provider,
            model,
            initial: built,
            created: now_ms(),
        },
    );
    Ok(view)
}

/// O texto inicial exato de uma conversa sem pack, para o consentimento. Com um link, baixa
/// só o texto puro do site do link (nada vai à IA além da lista de modelos e da contagem de
/// tokens sem o log; ADR-0060).
#[tauri::command]
#[specta::specta]
pub(crate) async fn ai_standalone_preview(
    state: State<'_, AppState>,
    request: AiStandalonePreviewRequest,
) -> Result<AiPreview, AppError> {
    standalone_preview_impl(&state, request).await
}

pub(crate) fn standalone_start_impl(
    state: &AppState,
    preview_id: &str,
    permissions: AiPermissions,
) -> Result<String, AppError> {
    let preview = lock(&state.ai.standalone.previews)
        .remove(preview_id)
        .ok_or_else(|| domain(&warden_ai::Error::PreviewExpired))?;
    let id = new_id();
    let mut conversation = new_conversation(
        id.clone(),
        now_ms(),
        preview.provider,
        preview.model,
        permissions,
        preview.symptom,
        preview.initial.origin,
        preview.note,
        PackSnapshot::default(),
    );
    push_question(&mut conversation, preview.initial.text);
    let store = crate::ai_tools::standalone::store(state);
    if let Some(log) = &preview.initial.local_log {
        store
            .save_local_log(&id, log)
            .map_err(|error| domain(&error))?;
    }
    store.save(&conversation).map_err(|error| domain(&error))?;
    Ok(id)
}

/// "✦ Começar conversa" sem pack: grava a conversa com as permissões e o texto mostrado. Não
/// manda nada; a interface chama `ai_standalone_run` em seguida.
#[tauri::command]
#[specta::specta]
pub(crate) fn ai_standalone_start(
    state: State<'_, AppState>,
    preview_id: String,
    permissions: AiPermissions,
) -> Result<String, AppError> {
    standalone_start_impl(&state, &preview_id, permissions)
}

/// Grava a conversa sem pack a cada mudança e avisa a interface.
struct StandaloneObserver<'a> {
    hub: &'a AiHub,
    store: ConversationStore,
    id: String,
    notify: StandaloneNotify,
    cancel: CancellationToken,
}

#[async_trait]
impl Observer for StandaloneObserver<'_> {
    async fn changed(&self, conversation: &Conversation) {
        save_running(self.hub, &self.store, &self.cancel, conversation);
        (self.notify)(&self.id);
    }

    fn live(&self, live: LiveStatus) {
        self.hub.set_live(&self.id, live);
        (self.notify)(&self.id);
    }
}

pub(crate) async fn standalone_run_impl(
    state: &AppState,
    id: &str,
    notify: StandaloneNotify,
) -> Result<(), AppError> {
    use crate::ai_tools::standalone::{StandaloneTools, registry as standalone_registry};
    let store = crate::ai_tools::standalone::store(state);
    let mut conversation = store.load(id).map_err(|error| domain(&error))?;
    if !pending(&conversation) {
        return Ok(());
    }
    let cancel = state.ai.begin(id)?;
    let guard = RunGuard {
        hub: &state.ai,
        id: id.to_owned(),
    };
    notify(id);
    let result = async {
        let key = conversation_key(state, conversation.provider)?;
        let client = state.ai.client(conversation.provider, key)?;
        let tools = StandaloneTools::new(state, store.local_log(id).ok(), cancel.clone());
        let registry = standalone_registry();
        let observer = StandaloneObserver {
            hub: &state.ai,
            store: store.clone(),
            id: id.to_owned(),
            notify: Arc::clone(&notify),
            cancel: cancel.clone(),
        };
        let engine = Engine {
            provider: client.as_ref(),
            registry: &registry,
            source: &tools,
            observer: &observer,
            timer: Arc::new(SystemTimer::new()),
            cancel: cancel.clone(),
            limits: Limits::default(),
        };
        if conversation.initial_tokens.is_none() && conversation.requests == 0 {
            match engine
                .count_tokens(&conversation.model, &conversation.messages)
                .await
            {
                Ok(count) => conversation.initial_tokens = count,
                Err(error) => tracing::info!(%error, "contagem do pedido inicial falhou"),
            }
        }
        engine
            .run(&mut conversation)
            .await
            .map_err(|error| domain(&error))
    }
    .await;
    if let Err(error) = &result
        && !matches!(
            conversation.turns.last(),
            Some(ConversationTurn {
                failure: Some(_),
                ..
            })
        )
    {
        if let Some(turn) = conversation.turns.last_mut() {
            turn.failure = Some(failure_from_app(error));
        }
        save_running(&state.ai, &store, &cancel, &conversation);
    }
    drop(guard);
    notify(id);
    result
}

/// Roda a pergunta pendente de uma conversa sem pack; a interface acompanha pelo
/// `ai-standalone-updated`.
#[tauri::command]
#[specta::specta]
pub(crate) async fn ai_standalone_run(
    app: AppHandle,
    state: State<'_, AppState>,
    conversation_id: String,
) -> Result<(), AppError> {
    standalone_run_impl(&state, &conversation_id, standalone_notify(app)).await
}

pub(crate) fn standalone_send_impl(state: &AppState, id: &str, text: &str) -> Result<(), AppError> {
    let text = text.trim();
    if text.is_empty() || text.chars().count() > MAX_QUESTION_CHARS {
        return Err(AppError::new(warden_project::ProjectErrorCode::InvalidInput)
            .with_param("field", "text"));
    }
    let store = crate::ai_tools::standalone::store(state);
    let mut conversation = store.load(id).map_err(|error| domain(&error))?;
    if state.ai.live(id).is_some() || pending(&conversation) {
        return Err(domain(&warden_ai::Error::ConversationBusy { id: id.to_owned() }));
    }
    push_question(&mut conversation, text.to_owned());
    store.save(&conversation).map_err(|error| domain(&error))
}

/// Acrescenta uma pergunta à conversa sem pack (sem rede); a interface chama
/// `ai_standalone_run` em seguida.
#[tauri::command]
#[specta::specta]
pub(crate) fn ai_standalone_send(
    app: AppHandle,
    state: State<'_, AppState>,
    conversation_id: String,
    text: String,
) -> Result<(), AppError> {
    standalone_send_impl(&state, &conversation_id, &text)?;
    standalone_notify(app)(&conversation_id);
    Ok(())
}

pub(crate) fn standalone_list_impl(state: &AppState) -> Result<Vec<AiConversationSummary>, AppError> {
    Ok(crate::ai_tools::standalone::store(state)
        .list()
        .map_err(|error| domain(&error))?
        .iter()
        .map(|conversation| summary_of(&state.ai, conversation))
        .collect())
}

/// As conversas sem pack, da mais nova para a mais antiga.
#[tauri::command]
#[specta::specta]
pub(crate) async fn ai_standalone_list(
    state: State<'_, AppState>,
) -> Result<Vec<AiConversationSummary>, AppError> {
    standalone_list_impl(&state)
}

pub(crate) fn standalone_get_impl(state: &AppState, id: &str) -> Result<AiConversationView, AppError> {
    let conversation = crate::ai_tools::standalone::store(state)
        .load(id)
        .map_err(|error| domain(&error))?;
    Ok(AiConversationView {
        summary: summary_of(&state.ai, &conversation),
        tier: conversation.provider.tier_of(&conversation.model),
        provider: conversation.provider,
        model: conversation.model.clone(),
        permissions: conversation.permissions,
        note: conversation.note.clone(),
        initial_tokens: conversation.initial_tokens,
        requests: conversation.requests,
        live: state.ai.live(id),
        // Sem pack: nada para comparar nem para aplicar.
        pack_changes: PackChanges::default(),
        turns: conversation.turns,
        proposals: Vec::new(),
    })
}

/// Uma conversa sem pack com as mensagens e as consultas.
#[tauri::command]
#[specta::specta]
pub(crate) async fn ai_standalone_get(
    state: State<'_, AppState>,
    conversation_id: String,
) -> Result<AiConversationView, AppError> {
    standalone_get_impl(&state, &conversation_id)
}

pub(crate) fn standalone_delete_impl(state: &AppState, id: &str) -> Result<(), AppError> {
    let _files = lock(&state.ai.files);
    state.ai.cancel(id);
    crate::ai_tools::standalone::store(state)
        .delete(id)
        .map_err(|error| domain(&error))
}

/// Apaga a conversa sem pack (o arquivo dela e o log guardado ao lado). Se a IA estava
/// respondendo, para.
#[tauri::command]
#[specta::specta]
pub(crate) fn ai_standalone_delete(
    app: AppHandle,
    state: State<'_, AppState>,
    conversation_id: String,
) -> Result<(), AppError> {
    standalone_delete_impl(&state, &conversation_id)?;
    standalone_notify(app)(&conversation_id);
    Ok(())
}
