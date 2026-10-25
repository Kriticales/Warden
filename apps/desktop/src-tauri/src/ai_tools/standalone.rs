//! A conversa sem pack (ROADMAP D-19; SPEC T14 item 8; ADR-0060): um log ou crash report do
//! computador ou aberto por um **link colado**, analisado pela IA fora de qualquer pack.
//!
//! - **Links aceitos** ([`parse_link`]): só HTTPS, só `mclo.gs` (o texto puro vem da API deles,
//!   `api.mclo.gs/1/raw/<id>`) e `pastebin.com` (`pastebin.com/raw/<id>`). O hastebin.com exige
//!   chave para ler desde 2023 (conferido em 19/10/2026: `401 missing access token`) e é recusado
//!   com a frase própria; qualquer outro endereço, com uma frase clara. Nada sai para a rede
//!   antes dessa conferência.
//! - **Download** ([`fetch`]): pelo Rust, uma tentativa, com tempo-limite ([`LINK_TIMEOUT`]) e
//!   teto ([`MAX_LOG_BYTES`], o mesmo dos logs): acima dele fica o fim do texto, como na leitura
//!   dos logs do computador. O link não vai à IA; o texto passa pela mesma redação antes do
//!   consentimento ([`build_initial`]) e só a cópia redigida fica guardada.
//! - **Ferramentas** ([`registry`], [`StandaloneTools`]): só as que fazem sentido sem pack, ler
//!   trechos do log (`search_log`, `read_log`) e, com a permissão, procurar issues no GitHub
//!   (`search_mod_issues`, pelo slug do mod no Modrinth). Sem ferramentas do pack e sem propostas.
//! - **Onde fica:** `ai/_sem-pack/<ULID>.json` nos dados do Warden ([`store`]).

use std::collections::HashMap;
use std::fmt::Write as _;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;

use async_trait::async_trait;
use serde_json::Value;
use warden_ai::conversation::{AiSymptom, ConversationOrigin, ConversationStore, PasteSite};
use warden_ai::external::{SEARCH_MOD_ISSUES, search_terms};
use warden_ai::providers::AiProvider;
use warden_ai::tools::{KnownMods, ToolOutput, ToolRegistry, ToolSource};
use warden_core::{CancellationToken, PackId};
use warden_http::{HttpClient, Url};
use warden_project::search::SourceId;

use super::initial::{Initial, excerpt, symptom_text};
use super::read::MAX_LOG_BYTES;
use super::{LINK_SESSION, LOCAL_FILE_SESSION, PackTools, external, read};
use crate::error::{AppError, AppErrorCode};
use crate::state::AppState;

/// Pasta das conversas sem pack, dentro de `ai/` nos dados do Warden.
pub(crate) const STANDALONE_DIR: &str = "_sem-pack";
/// Tempo-limite do download do texto de um link.
pub(crate) const LINK_TIMEOUT: Duration = Duration::from_secs(30);
/// Texto puro do mclo.gs.
const MCLOGS_RAW: &str = "https://api.mclo.gs/1/raw/";
/// Texto puro do pastebin.
const PASTEBIN_RAW: &str = "https://pastebin.com/raw/";
/// Em build de debug, o endereço do texto puro do mclo.gs (servidor de fixtures dos E2E).
const MCLOGS_ENV: &str = "WARDEN_API_BASE_MCLOGS";
/// Em build de debug, o endereço do texto puro do pastebin.
const PASTEBIN_ENV: &str = "WARDEN_API_BASE_PASTEBIN";

/// As ferramentas da conversa sem pack.
pub(crate) fn registry() -> ToolRegistry {
    super::registry().only(&["search_log", "read_log", SEARCH_MOD_ISSUES])
}

/// A pasta das conversas sem pack.
pub(crate) fn store(state: &AppState) -> ConversationStore {
    ConversationStore::new(state.paths.data_dir().join("ai").join(STANDALONE_DIR))
}

/// O nome do site como aparece nas frases.
pub(crate) const fn site_name(site: PasteSite) -> &'static str {
    match site {
        PasteSite::Mclogs => "mclo.gs",
        PasteSite::Pastebin => "pastebin",
    }
}

// ---------------------------------------------------------------------------------------
// Links
// ---------------------------------------------------------------------------------------

/// Um link aceito.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PasteLink {
    /// O site.
    pub(crate) site: PasteSite,
    /// O id do texto no site.
    pub(crate) id: String,
    /// O link como foi colado (sem espaços nas pontas).
    pub(crate) url: String,
}

fn link_error(code: AppErrorCode) -> AppError {
    AppError::new(code)
}

fn valid_id(id: &str) -> bool {
    (2..=64).contains(&id.len()) && id.chars().all(|c| c.is_ascii_alphanumeric())
}

/// Confere o link colado, sem rede. Erros: `AI_LINK_NOT_HTTPS`, `AI_LINK_NEEDS_KEY` (hastebin),
/// `AI_LINK_UNSUPPORTED` (outro site ou texto que não é link) e `AI_LINK_INVALID` (site certo,
/// sem o id do texto).
pub(crate) fn parse_link(text: &str) -> Result<PasteLink, AppError> {
    let text = text.trim();
    // "mclo.gs/abc" sem o esquema vale como https.
    let with_scheme = if text.contains("://") {
        text.to_owned()
    } else {
        format!("https://{text}")
    };
    let url = Url::parse(&with_scheme).map_err(|_| link_error(AppErrorCode::AiLinkUnsupported))?;
    let host = url
        .host_str()
        .unwrap_or("")
        .to_ascii_lowercase()
        .trim_start_matches("www.")
        .to_owned();
    let segments: Vec<&str> = url
        .path_segments()
        .map(|parts| parts.filter(|part| !part.is_empty()).collect())
        .unwrap_or_default();
    let hastebin = host == "hastebin.com"
        || (host == "toptal.com" && segments.starts_with(&["developers", "hastebin"]));
    if hastebin {
        return Err(link_error(AppErrorCode::AiLinkNeedsKey));
    }
    let (site, id) = match host.as_str() {
        // `mclo.gs/<id>` e o próprio texto puro, `api.mclo.gs/1/raw/<id>`.
        "mclo.gs" => (PasteSite::Mclogs, segments.first().copied()),
        "api.mclo.gs" => (
            PasteSite::Mclogs,
            match segments.as_slice() {
                ["1", "raw", id] => Some(*id),
                _ => None,
            },
        ),
        // `pastebin.com/<id>` e `pastebin.com/raw/<id>`.
        "pastebin.com" => (
            PasteSite::Pastebin,
            match segments.as_slice() {
                ["raw", id] | [id] => Some(*id),
                _ => None,
            },
        ),
        _ => return Err(link_error(AppErrorCode::AiLinkUnsupported)),
    };
    if url.scheme() != "https" {
        return Err(link_error(AppErrorCode::AiLinkNotHttps).with_param("site", site_name(site)));
    }
    let id = id.filter(|id| valid_id(id)).ok_or_else(|| {
        link_error(AppErrorCode::AiLinkInvalid).with_param("site", site_name(site))
    })?;
    Ok(PasteLink {
        site,
        id: id.to_owned(),
        url: text.to_owned(),
    })
}

/// O endereço do texto puro de cada site.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PasteBases {
    pub(crate) mclogs: String,
    pub(crate) pastebin: String,
}

#[cfg(test)]
thread_local! {
    /// Os sites simulados nos testes (cada teste roda numa thread, com o seu servidor).
    pub(crate) static TEST_BASES: std::cell::RefCell<Option<PasteBases>> =
        const { std::cell::RefCell::new(None) };
}

impl PasteBases {
    /// Os oficiais (ou os de `WARDEN_API_BASE_MCLOGS` e `WARDEN_API_BASE_PASTEBIN`, em build de
    /// debug, para o servidor de fixtures dos E2E).
    pub(crate) fn current() -> Self {
        #[cfg(test)]
        if let Some(bases) = TEST_BASES.with(|bases| bases.borrow().clone()) {
            return bases;
        }
        let pick = |env: &str, official: &str| {
            #[cfg(debug_assertions)]
            if let Some(base) = std::env::var(env).ok().filter(|value| !value.is_empty()) {
                return base;
            }
            #[cfg(not(debug_assertions))]
            let _ = env;
            official.to_owned()
        };
        Self {
            mclogs: pick(MCLOGS_ENV, MCLOGS_RAW),
            pastebin: pick(PASTEBIN_ENV, PASTEBIN_RAW),
        }
    }

    /// O endereço do texto puro de um link.
    pub(crate) fn raw_url(&self, link: &PasteLink) -> Result<Url, AppError> {
        let base = match link.site {
            PasteSite::Mclogs => &self.mclogs,
            PasteSite::Pastebin => &self.pastebin,
        };
        let base = if base.ends_with('/') {
            base.clone()
        } else {
            format!("{base}/")
        };
        Url::parse(&base)
            .and_then(|base| base.join(&link.id))
            .map_err(|error| AppError::internal(format!("endereço do texto puro: {error}")))
    }
}

/// O texto baixado de um link.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Fetched {
    /// O texto (o fim dele, se passou do teto).
    pub(crate) text: String,
    /// Passou do teto: só o fim foi guardado.
    pub(crate) truncated: bool,
}

fn http_error(site: PasteSite, error: &warden_http::Error) -> AppError {
    let site = site_name(site);
    let base = match error {
        warden_http::Error::Cancelled => return AppError::cancelled(),
        warden_http::Error::Timeout { seconds, .. } => {
            let mut error = AppError::new(AppErrorCode::AiLinkTimeout)
                .with_param("seconds", seconds.to_string());
            error.retryable = true;
            error
        }
        warden_http::Error::NotFound { .. } => AppError::new(AppErrorCode::AiLinkNotFound),
        other => {
            let mut error = AppError::new(AppErrorCode::AiLinkUnreachable).with_param(
                "status",
                other
                    .status()
                    .map(|status| status.to_string())
                    .unwrap_or_default(),
            );
            error.retryable = true;
            error
        }
    };
    // O detalhe técnico leva o erro HTTP (o endereço do texto puro, sem nada pessoal).
    base.with_param("site", site).with_detail(error.to_string())
}

/// Baixa o texto puro de um link: uma tentativa, com `timeout` no total, guardando só os
/// últimos `cap` bytes (o fim do log é o que importa; o resto da primeira linha cortada sai).
pub(crate) async fn fetch(
    http: &HttpClient,
    link: &PasteLink,
    bases: &PasteBases,
    cap: u64,
    timeout: Duration,
    cancel: &CancellationToken,
) -> Result<Fetched, AppError> {
    let url = bases.raw_url(link)?;
    let mut response = http
        .get(url)
        .idempotent(false)
        .total_timeout(Some(timeout))
        .cancel(cancel)
        .send()
        .await
        .map_err(|error| http_error(link.site, &error))?;
    let cap = usize::try_from(cap).unwrap_or(usize::MAX);
    let mut body: Vec<u8> = Vec::new();
    let mut truncated = false;
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|error| http_error(link.site, &error))?
    {
        body.extend_from_slice(&chunk);
        if body.len() > cap {
            truncated = true;
            body.drain(..body.len() - cap);
        }
    }
    let mut text = String::from_utf8_lossy(&body).into_owned();
    if truncated {
        // A primeira linha ficou pela metade.
        if let Some(newline) = text.find('\n') {
            text.drain(..=newline);
        }
    }
    let text = text.trim_start_matches('\u{feff}').to_owned();
    if text.trim().is_empty() {
        return Err(
            AppError::new(AppErrorCode::AiLinkEmpty).with_param("site", site_name(link.site))
        );
    }
    Ok(Fetched { text, truncated })
}

// ---------------------------------------------------------------------------------------
// Texto inicial
// ---------------------------------------------------------------------------------------

/// O log escolhido, já lido (do computador ou do link).
#[derive(Debug, Clone)]
pub(crate) struct Subject {
    /// A origem gravada na conversa.
    pub(crate) origin: ConversationOrigin,
    /// O texto lido, ainda sem redação.
    pub(crate) raw: String,
    /// Passou do teto: só o fim foi lido.
    pub(crate) truncated: bool,
}

/// O texto inicial da conversa sem pack, **já redigido**: é exatamente o que o consentimento
/// mostra e o que vai na primeira mensagem. O link não entra (só o site).
pub(crate) fn build_initial(
    redactor: &warden_diagnostics::Redactor,
    symptom: AiSymptom,
    subject: Subject,
    note: Option<&str>,
) -> Initial {
    let redacted = redactor.redact(&subject.raw).text;
    let lines = redacted.lines().count();
    let (what, session) = match &subject.origin {
        ConversationOrigin::PasteLink { site, .. } => (
            format!(
                "um log aberto por um link do {} ({lines} linhas)",
                site_name(*site)
            ),
            LINK_SESSION,
        ),
        ConversationOrigin::LocalFile { file_name } => (
            format!(
                "um log ou crash report escolhido no computador (\"{file_name}\", {lines} linhas)"
            ),
            LOCAL_FILE_SESSION,
        ),
        _ => (format!("um log ({lines} linhas)"), LOCAL_FILE_SESSION),
    };
    let mut text = String::new();
    let _ = writeln!(text, "Sintoma: {}.", symptom_text(symptom));
    if let Some(note) = note.map(str::trim).filter(|note| !note.is_empty()) {
        let _ = writeln!(text, "O que a pessoa contou: {note}");
    }
    let _ = writeln!(
        text,
        "\nSem pack: a pessoa quer entender um log avulso, fora de qualquer pack do Warden. Não há lista de mods, achados, testes nem configs: as regras sobre o pack, get_findings, list_mods e propose_* não valem nesta conversa. Não faça propostas (não há pack para mudar) e deixe vazio o campo mods das afirmações; cite os mods pelo nome que aparece no log."
    );
    let _ = writeln!(text, "\nAnalisar: {what}.");
    if subject.truncated {
        let _ = writeln!(
            text,
            "O texto passou de {} MB: só o fim dele foi lido.",
            MAX_LOG_BYTES / (1024 * 1024)
        );
    }
    let _ = write!(
        text,
        "\nÚltimas linhas com erro (sem dados pessoais):\n{}",
        excerpt(&redacted)
    );
    let _ = writeln!(
        text,
        "\nUse search_log e read_log com session_id \"{session}\" para ler o log. Para procurar issues parecidas no GitHub (se a busca estiver liberada), use search_mod_issues com o slug do mod no Modrinth (como sodium) no lugar do mod_id."
    );
    // Tudo redigido de novo no fim (o texto inteiro revela nomes que um pedaço sozinho não
    // revela), como no texto inicial com pack.
    let text = redactor.redact(text.trim_end()).text;
    Initial {
        text,
        origin: subject.origin,
        local_log: Some(redacted),
    }
}

// ---------------------------------------------------------------------------------------
// Prévias e ferramentas
// ---------------------------------------------------------------------------------------

/// O texto inicial mostrado no consentimento de uma conversa sem pack, guardado até o
/// "Começar conversa".
#[derive(Debug, Clone)]
pub(crate) struct StandalonePreview {
    pub(crate) symptom: AiSymptom,
    pub(crate) note: Option<String>,
    pub(crate) provider: AiProvider,
    pub(crate) model: String,
    /// Como procurar na primeira pergunta (`None` no Gemini; ADR-0064).
    pub(crate) mode: Option<warden_ai::providers::SearchMode>,
    /// O modelo mais leve para quando a cota diária de `model` acabar (como com pack).
    pub(crate) fallback_model: Option<String>,
    pub(crate) initial: Initial,
    pub(crate) created: u64,
}

/// As prévias das conversas sem pack (no `AiHub`).
#[derive(Debug, Default)]
pub(crate) struct StandaloneHub {
    pub(crate) previews: Mutex<HashMap<String, StandalonePreview>>,
}

/// Quem responde às ferramentas de uma conversa sem pack: o log guardado (redigido) e a busca
/// de issues pelo slug do Modrinth. Tudo passa pela redação antes de sair.
pub(crate) struct StandaloneTools<'a> {
    /// Só o log e a redação são usados: sem pack, o id e a pasta não apontam para nada.
    inner: PackTools<'a>,
}

impl<'a> StandaloneTools<'a> {
    pub(crate) fn new(
        state: &'a AppState,
        local_log: Option<PathBuf>,
        cancel: CancellationToken,
    ) -> Self {
        Self {
            inner: PackTools::new(state, PackId::new(), PathBuf::new(), local_log)
                .with_cancel(cancel),
        }
    }

    /// A redação usada no texto inicial e nas ferramentas.
    pub(crate) fn redactor(&self) -> &warden_diagnostics::Redactor {
        &self.inner.redactor
    }
}

#[async_trait]
impl ToolSource for StandaloneTools<'_> {
    async fn call(&self, name: &str, args: &Value) -> Result<ToolOutput, String> {
        let output = match name {
            "search_log" | "read_log" => return read::call(&self.inner, name, args).await,
            SEARCH_MOD_ISSUES => {
                let slug = super::text_arg(args, "mod_id");
                // O slug vai no caminho da API do Modrinth: só letras, números, `-` e `_`.
                let valid = (1..=64).contains(&slug.len())
                    && slug
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'));
                if !valid {
                    return Err(format!(
                        "'{slug}' não é um slug do Modrinth; use o slug da página do mod, como sodium."
                    ));
                }
                let query = super::text_arg(args, "query");
                let terms = search_terms(&query, &self.inner.redactor)
                    .map_err(|error| error.to_string())?;
                external::project_issues(&self.inner, SourceId::Modrinth, &slug, &terms).await
            }
            other => Err(format!(
                "A ferramenta {other} não existe numa conversa sem pack."
            )),
        };
        // Nada sai sem passar pela redação (inclusive as mensagens de erro).
        match output {
            Ok(mut output) => {
                for item in &mut output.items {
                    item.text = self.inner.redactor.redact(&item.text).text;
                }
                Ok(output)
            }
            Err(message) => Err(self.inner.redactor.redact(&message).text),
        }
    }

    async fn known_mods(&self) -> KnownMods {
        KnownMods::default()
    }
}

#[cfg(test)]
mod tests;
