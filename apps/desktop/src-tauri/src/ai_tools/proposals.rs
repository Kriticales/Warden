//! As propostas da IA no app (SPEC T14 ✦ passo 3; ARCHITECTURE §9.5 "Propostas"; ROADMAP D-14;
//! CA-T14-12): conferir o alvo quando a IA propõe ([`check`]), mostrar a diferença exata
//! ([`review`]), aplicar pelos fluxos normais ([`apply`]) e descartar ([`discard`]).
//!
//! Cada proposta é preparada pelos mesmos serviços da interface, sem gravar nada:
//! - adicionar: o plano de "Adicionar" (`add_plan`, com as dependências obrigatórias);
//! - remover: o plano de "Remover" (o que deixa de funcionar e as dependências sem uso);
//! - atualizar e trocar de versão: o plano de "Trocar de versão" (P1-15; dependências novas,
//!   incompatibilidades, rebaixamento);
//! - mudar o lado: o lado de agora e o pedido;
//! - editar uma config: o texto de agora e o texto com **só aquela chave** trocada (o mesmo
//!   editor do formulário de configs, que confere pela releitura que nada mais mudou);
//! - ajustes do teste: o perfil ativo antes e depois (neste computador, fora do pack);
//! - testar de novo e a busca do culpado: a interface abre o fluxo normal.
//!
//! A prévia tem um `token` (SHA-256 da prévia e do que será gravado). **Aplicar** prepara de
//! novo e só grava se o token for o mesmo que o usuário viu (senão, `PROPOSAL_STALE` e nada
//! muda); quando a proposta muda o pack, cria antes um ponto de segurança e grava pelos mesmos
//! `*_impl` dos comandos normais (transação, `packwiz refresh`, trava do pack). Descartar só
//! grava a decisão na conversa. A conversa fica marcada como ocupada enquanto isso (nenhuma
//! pergunta nova nem resposta da IA grava por cima da decisão).

use serde::Serialize;
use sha2::{Digest as _, Sha256};
use warden_ai::conversation::ConversationStore;
use warden_ai::engine::now_ms;
use warden_ai::proposals::{
    self, Proposal, ProposalKind, ProposalSide, ProposalSource, ProposalState, ProposalSummary,
};
use warden_configs::meta::{FieldKey, FormEdit, FormValue, apply_form_edits};
use warden_configs::{ConfigDocument, ConfigFormat, ConfigValue, KeyPath};
use warden_core::{CancellationToken, PackId};
use warden_packwiz_cli::Packwiz;
use warden_project::add::plan::{AddPlan, AddPlanRequest, NodeRole};
use warden_project::add::{AddApplyRequest, AddChoice, ApplyItem};
use warden_project::change_version::ChangeVersionPlan;
use warden_project::configs::ConfigOrigin;
use warden_project::inventory::ItemSide;
use warden_project::remove::RemovalPlan;
use warden_project::search::SourceId;
use warden_project::side::SideChoice;
use warden_versioning::{Identity, Moment, PackRepo, SafetyReason};

use super::PackTools;
use crate::commands::add::{add_sources, channel_policy};
use crate::commands::inventory::{pack_root, packwiz};
use crate::error::AppError;
use crate::events::PackArea;
use crate::settings::TestMemory;
use crate::state::AppState;
use crate::test_profiles::{self, TestProfile};

/// Os ajustes do teste que uma proposta pode mudar.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TestSettingsValues {
    /// Memória.
    pub(crate) memory: TestMemory,
    /// Versão do Java escolhido (`None`: Automático).
    pub(crate) java_major: Option<u32>,
    /// Argumentos extras da JVM.
    pub(crate) jvm_args: String,
}

/// A diferença exata que o diálogo de **Aplicar** mostra.
#[derive(Debug, Clone, PartialEq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub(crate) enum ProposalPreview {
    /// O plano de "Adicionar" (o item e as dependências que entram junto).
    #[serde(rename_all = "camelCase")]
    AddMod {
        /// O plano.
        plan: AddPlan,
    },
    /// O plano de "Remover".
    #[serde(rename_all = "camelCase")]
    RemoveMod {
        /// O plano.
        plan: RemovalPlan,
    },
    /// O plano de "Trocar de versão" (atualizar também é isso).
    #[serde(rename_all = "camelCase")]
    ChangeVersion {
        /// O plano.
        plan: ChangeVersionPlan,
    },
    /// Mudar o lado.
    #[serde(rename_all = "camelCase")]
    SetSide {
        /// Nome do item.
        name: String,
        /// Caminho do item.
        path: String,
        /// Lado de agora.
        from: ItemSide,
        /// Lado pedido.
        to: SideChoice,
    },
    /// Editar uma config: o texto de agora e o texto gravado.
    #[serde(rename_all = "camelCase")]
    EditConfig {
        /// Caminho relativo da config.
        path: String,
        /// A chave (`general.maxChunks`).
        key: String,
        /// Texto de agora.
        before: String,
        /// Texto com só a chave trocada.
        after: String,
    },
    /// Ajustes do perfil ativo do teste (neste computador).
    #[serde(rename_all = "camelCase")]
    TestSettings {
        /// Nome do perfil (vazio: o Padrão).
        profile: String,
        /// Antes.
        before: TestSettingsValues,
        /// Depois.
        after: TestSettingsValues,
    },
    /// Testar de novo (o Testar normal).
    RunTest,
    /// A busca do culpado (o diálogo normal).
    StartBisect,
}

/// O que o diálogo de **Aplicar** recebe.
#[derive(Debug, Clone, PartialEq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProposalReview {
    /// A proposta.
    pub(crate) proposal_id: String,
    /// Volta no `ai_proposal_apply`: garante que vai ser gravado o que foi mostrado.
    pub(crate) token: String,
    /// Muda arquivos do pack (cria ponto de segurança antes).
    pub(crate) changes_pack: bool,
    /// A diferença.
    pub(crate) preview: ProposalPreview,
}

/// Resultado de **Aplicar**.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProposalApplied {
    /// A proposta como ficou.
    pub(crate) proposal: Proposal,
    /// O ponto de segurança criado antes (quando a proposta mudou o pack).
    pub(crate) safety_point: Option<String>,
}

/// O que será gravado (entra no token junto com a prévia).
#[derive(Debug, Clone, PartialEq, Serialize)]
enum Action {
    Add(AddApplyRequest),
    Remove(Vec<String>),
    ChangeVersion {
        path: String,
        expected_current_id: Option<String>,
        version_id: String,
    },
    SetSide {
        path: String,
        side: SideChoice,
    },
    EditConfig {
        path: String,
        text: String,
        hash: String,
    },
    TestSettings(TestProfile),
    /// A interface abre o fluxo normal (Testar, busca do culpado).
    Ui,
}

/// Uma proposta preparada.
struct Prepared {
    preview: ProposalPreview,
    summary: ProposalSummary,
    action: Action,
}

impl Prepared {
    fn token(&self) -> String {
        let bytes = serde_json::to_vec(&(&self.preview, &self.action)).unwrap_or_default();
        format!("{:x}", Sha256::digest(bytes))
    }
}

#[allow(clippy::needless_pass_by_value)] // usado em `map_err`
fn domain(error: warden_project::Error) -> AppError {
    AppError::from_domain(&error)
}

fn invalid(message: impl Into<String>) -> AppError {
    AppError::from_domain(&warden_project::Error::new(
        warden_project::ProjectErrorCode::InvalidInput,
        message.into(),
    ))
}

fn source_id(source: ProposalSource) -> SourceId {
    match source {
        ProposalSource::Modrinth => SourceId::Modrinth,
        ProposalSource::Curseforge => SourceId::Curseforge,
    }
}

fn side_choice(side: ProposalSide) -> SideChoice {
    match side {
        ProposalSide::Client => SideChoice::Client,
        ProposalSide::Server => SideChoice::Server,
        ProposalSide::Both => SideChoice::Both,
    }
}

fn wire(value: impl Serialize) -> String {
    serde_json::to_value(value)
        .ok()
        .and_then(|value| value.as_str().map(ToOwned::to_owned))
        .unwrap_or_default()
}

/// Prepara a proposta pelos serviços normais, sem gravar nada. O erro (`AppError`) diz o que
/// não serve: a versão não existe, a chave não existe, o item já está no pack…
async fn prepare(
    state: &AppState,
    pack_id: PackId,
    change: &ProposalKind,
) -> Result<Prepared, AppError> {
    let root = pack_root(state, pack_id)?;
    match change {
        ProposalKind::AddMod { source, project_id } => {
            add_mod(state, pack_id, &root, *source, project_id).await
        }
        ProposalKind::RemoveMod { path } => {
            let plan = crate::commands::inventory::remove_plan_impl(
                state,
                pack_id,
                std::slice::from_ref(path),
            )
            .await?;
            let target = plan
                .targets
                .first()
                .ok_or_else(|| invalid(format!("o item {path} não está no pack")))?;
            let summary = ProposalSummary {
                target: Some(target.name.clone()),
                from: None,
                to: None,
            };
            Ok(Prepared {
                preview: ProposalPreview::RemoveMod { plan },
                summary,
                action: Action::Remove(vec![path.clone()]),
            })
        }
        ProposalKind::UpdateMod { path } => change_version(state, pack_id, &root, path, None).await,
        ProposalKind::ChangeVersion { path, version } => {
            change_version(state, pack_id, &root, path, Some(version)).await
        }
        ProposalKind::SetSide { path, side } => set_side(state, pack_id, &root, path, *side).await,
        ProposalKind::EditConfig { path, key, value } => {
            let content = {
                let _lock = state.locks.read(pack_id).await;
                let root = root.clone();
                let relative = path.clone();
                tauri::async_runtime::spawn_blocking(move || {
                    warden_project::configs::read_file(&root, ConfigOrigin::Pack, &relative)
                })
                .await
                .map_err(|error| AppError::internal(error.to_string()))?
                .map_err(domain)?
            };
            edit_config(content, key, value)
        }
        ProposalKind::TestSettings {
            memory_mb,
            java_major,
            jvm_args,
        } => test_settings(state, pack_id, *memory_mb, *java_major, jvm_args.as_deref()),
        ProposalKind::RunTest => Ok(Prepared {
            preview: ProposalPreview::RunTest,
            summary: ProposalSummary::default(),
            action: Action::Ui,
        }),
        ProposalKind::StartBisect => Ok(Prepared {
            preview: ProposalPreview::StartBisect,
            summary: ProposalSummary::default(),
            action: Action::Ui,
        }),
    }
}

/// "Adicionar": o plano com as dependências obrigatórias (as opcionais ficam de fora).
async fn add_mod(
    state: &AppState,
    pack_id: PackId,
    root: &std::path::Path,
    source: ProposalSource,
    project_id: &str,
) -> Result<Prepared, AppError> {
    let plan = {
        let _lock = state.locks.read(pack_id).await;
        warden_project::add::plan::add_plan(
            root,
            &add_sources(state),
            channel_policy(state),
            &AddPlanRequest {
                items: vec![AddChoice {
                    source: source_id(source),
                    project_id: project_id.to_owned(),
                    version_id: None,
                }],
            },
            &CancellationToken::new(),
        )
        .await
        .map_err(domain)?
    };
    let chosen = plan
        .nodes
        .iter()
        .find(|node| node.role == NodeRole::Chosen)
        .ok_or_else(|| invalid("o projeto já está no pack ou não tem versão compatível"))?;
    let summary = ProposalSummary {
        target: Some(chosen.title.clone()),
        from: None,
        to: Some(chosen.version_number.clone()),
    };
    let items = plan
        .nodes
        .iter()
        .filter(|node| node.role != NodeRole::Optional)
        .map(|node| ApplyItem {
            source: node.source,
            project_id: node.project_id.clone(),
            version_id: node.version_id.clone(),
            side: None,
            replaces: None,
        })
        .collect();
    Ok(Prepared {
        preview: ProposalPreview::AddMod { plan },
        summary,
        action: Action::Add(AddApplyRequest { items }),
    })
}

/// Mudar o lado de um item.
async fn set_side(
    state: &AppState,
    pack_id: PackId,
    root: &std::path::Path,
    path: &str,
    side: ProposalSide,
) -> Result<Prepared, AppError> {
    let item = {
        let _lock = state.locks.read(pack_id).await;
        warden_project::inventory::inventory(root, Some(&state.modrinth))
            .await
            .map_err(domain)?
            .items
            .into_iter()
            .find(|item| item.path == path)
            .ok_or_else(|| invalid(format!("o item {path} não está no pack")))?
    };
    if !item.side_editable {
        return Err(invalid(format!(
            "o lado de {} não pode ser mudado",
            item.name
        )));
    }
    let to = side_choice(side);
    if wire(item.side) == wire(to) {
        return Err(invalid(format!("{} já está nesse lado", item.name)));
    }
    Ok(Prepared {
        summary: ProposalSummary {
            target: Some(item.name.clone()),
            from: Some(wire(item.side)),
            to: Some(wire(to)),
        },
        preview: ProposalPreview::SetSide {
            name: item.name,
            path: path.to_owned(),
            from: item.side,
            to,
        },
        action: Action::SetSide {
            path: path.to_owned(),
            side: to,
        },
    })
}

/// Atualizar (`version` = `None`: a compatível mais nova) ou trocar de versão.
async fn change_version(
    state: &AppState,
    pack_id: PackId,
    root: &std::path::Path,
    path: &str,
    version: Option<&String>,
) -> Result<Prepared, AppError> {
    let _lock = state.locks.read(pack_id).await;
    let sources = add_sources(state);
    let cancel = CancellationToken::new();
    let options = warden_project::change_version::options(root, &sources, path, &cancel)
        .await
        .map_err(domain)?;
    let wanted = match version {
        None => options
            .versions
            .first()
            .ok_or_else(|| invalid("o mod não tem versões compatíveis com o pack"))?,
        Some(version) => {
            let version = version.trim();
            options
                .versions
                .iter()
                .find(|choice| choice.id == version || choice.number == version)
                .ok_or_else(|| {
                    let some: Vec<&str> = options
                        .versions
                        .iter()
                        .take(8)
                        .map(|choice| choice.number.as_str())
                        .collect();
                    invalid(format!(
                        "a versão {version} não está entre as compatíveis com o pack (algumas: {})",
                        some.join(", ")
                    ))
                })?
        }
    };
    if wanted.installed {
        return Err(invalid(format!(
            "{} já está na versão {}",
            options.name, wanted.number
        )));
    }
    let plan = warden_project::change_version::plan(root, &sources, path, &wanted.id, &cancel)
        .await
        .map_err(domain)?;
    Ok(Prepared {
        summary: ProposalSummary {
            target: Some(plan.name.clone()),
            from: plan.current.clone(),
            to: Some(plan.to.number.clone()),
        },
        action: Action::ChangeVersion {
            path: path.to_owned(),
            expected_current_id: options.current_id.clone(),
            version_id: plan.to.id.clone(),
        },
        preview: ProposalPreview::ChangeVersion { plan },
    })
}

/// O valor no tipo da chave de agora (o formulário de configs só troca o valor pelo mesmo tipo).
fn form_value(current: &ConfigValue, value: &str) -> Result<FormValue, AppError> {
    Ok(match current {
        ConfigValue::Bool(_) => FormValue::Bool(match value.trim() {
            "true" => true,
            "false" => false,
            other => {
                return Err(invalid(format!(
                    "a chave é liga/desliga: use true ou false, não {other}"
                )));
            }
        }),
        ConfigValue::Integer(_) => FormValue::Integer(value.trim().to_owned()),
        ConfigValue::Float(_) => FormValue::Float(value.trim().to_owned()),
        ConfigValue::String(_) | ConfigValue::Null => FormValue::String(value.to_owned()),
        ConfigValue::Datetime(_) => FormValue::Datetime(value.trim().to_owned()),
        ConfigValue::List(_) => {
            return Err(invalid(
                "listas não podem ser mudadas por proposta; mude no editor de configs",
            ));
        }
    })
}

fn value_text(value: &ConfigValue) -> String {
    match value {
        ConfigValue::Bool(flag) => flag.to_string(),
        ConfigValue::Integer(number) => number.to_string(),
        ConfigValue::Float(number) => number.to_string(),
        ConfigValue::String(text) | ConfigValue::Datetime(text) => text.clone(),
        ConfigValue::Null => "null".to_owned(),
        ConfigValue::List(items) => items.iter().map(value_text).collect::<Vec<_>>().join(", "),
    }
}

fn edit_config(
    content: warden_project::configs::ConfigContent,
    key: &[String],
    value: &str,
) -> Result<Prepared, AppError> {
    let path = content.path.clone();
    if content.read_only.is_some() {
        return Err(invalid(format!(
            "a config {path} não pode ser editada (grande demais ou binária)"
        )));
    }
    let text = content
        .text
        .ok_or_else(|| invalid(format!("a config {path} não é texto")))?;
    let format =
        ConfigFormat::detect(std::path::Path::new(&path), text.as_bytes()).ok_or_else(|| {
            invalid(format!(
                "o formato de {path} não permite mudar uma chave só"
            ))
        })?;
    let document = ConfigDocument::parse(format, text.as_bytes())
        .map_err(|error| invalid(format!("{path} não pôde ser lida: {error}")))?;
    let key_path = KeyPath::from_keys(key.iter().map(String::as_str));
    let dotted = key.join(".");
    let current = document
        .get(&key_path)
        .and_then(|entry| entry.value.clone())
        .ok_or_else(|| invalid(format!("a chave {dotted} não existe em {path}")))?;
    let edit = FormEdit {
        path: key.iter().cloned().map(FieldKey::Name).collect(),
        value: form_value(&current, value)?,
    };
    let after = apply_form_edits(&path, &text, std::slice::from_ref(&edit))
        .map_err(|error| invalid(format!("o valor {value} não serve para {dotted}: {error}")))?;
    if after == text {
        return Err(invalid(format!("a chave {dotted} já tem esse valor")));
    }
    Ok(Prepared {
        summary: ProposalSummary {
            target: Some(format!("{path}: {dotted}")),
            from: Some(value_text(&current)),
            to: Some(value.to_owned()),
        },
        action: Action::EditConfig {
            path: path.clone(),
            text: after.clone(),
            hash: content.hash,
        },
        preview: ProposalPreview::EditConfig {
            path,
            key: dotted,
            before: text,
            after,
        },
    })
}

fn test_settings(
    state: &AppState,
    pack_id: PackId,
    memory_mb: Option<u32>,
    java_major: Option<u32>,
    jvm_args: Option<&str>,
) -> Result<Prepared, AppError> {
    let profiles = test_profiles::profiles_of(state, pack_id)?;
    let profile = profiles
        .profiles
        .iter()
        .find(|profile| profile.id == profiles.active)
        .cloned()
        .ok_or_else(|| AppError::internal("perfil ativo do teste sumiu"))?;
    let installed = state
        .java
        .installed()
        .map_err(|error| AppError::from_domain(&error))?;
    let major_of = |id: &warden_java::RuntimeId| {
        installed
            .iter()
            .find(|runtime| &runtime.id == id)
            .map(|runtime| runtime.version.major)
    };
    let before = TestSettingsValues {
        memory: profile.fields.memory,
        java_major: profile.fields.java.as_ref().and_then(major_of),
        jvm_args: profile.fields.jvm_args.clone(),
    };
    let mut changed = profile.clone();
    if let Some(mb) = memory_mb {
        changed.fields.memory = TestMemory::Fixed { mb };
    }
    if let Some(major) = java_major {
        let runtime = installed
            .iter()
            .filter(|runtime| runtime.version.major == major)
            .max_by_key(|runtime| runtime.version)
            .ok_or_else(|| {
                invalid(format!(
                    "o Java {major} não está instalado neste computador"
                ))
            })?;
        changed.fields.java = Some(runtime.id.clone());
    }
    if let Some(args) = jvm_args {
        args.trim().clone_into(&mut changed.fields.jvm_args);
    }
    let after = TestSettingsValues {
        memory: changed.fields.memory,
        java_major: changed.fields.java.as_ref().and_then(major_of),
        jvm_args: changed.fields.jvm_args.clone(),
    };
    if before == after {
        return Err(invalid("os ajustes do teste já são esses"));
    }
    Ok(Prepared {
        summary: ProposalSummary {
            target: Some(profile.name.clone()),
            from: None,
            to: None,
        },
        preview: ProposalPreview::TestSettings {
            profile: profile.name,
            before,
            after,
        },
        action: Action::TestSettings(changed),
    })
}

/// Confere o alvo quando a IA propõe (o laço chama pelo `ToolSource`): prepara como o
/// diálogo faria, sem gravar nada, e devolve o resumo do cartão. O erro é a frase para a IA.
pub(super) async fn check(
    tools: &PackTools<'_>,
    change: &ProposalKind,
) -> Result<ProposalSummary, String> {
    prepare(tools.state, tools.pack_id, change)
        .await
        .map(|prepared| prepared.summary)
        .map_err(|error| {
            format!(
                "A proposta não serve: {}",
                error.detail.clone().unwrap_or_else(|| wire(error.code))
            )
        })
}

fn store(state: &AppState, pack_id: PackId) -> ConversationStore {
    ConversationStore::new(state.paths.ai_dir(pack_id))
}

fn ai(error: &warden_ai::Error) -> AppError {
    AppError::from_domain(error)
}

/// A diferença exata de uma proposta pendente (o diálogo de **Aplicar**). Nada é gravado.
pub(crate) async fn review(
    state: &AppState,
    pack_id: PackId,
    conversation_id: &str,
    proposal_id: &str,
) -> Result<ProposalReview, AppError> {
    pack_root(state, pack_id)?;
    let conversation = store(state, pack_id)
        .load(conversation_id)
        .map_err(|e| ai(&e))?;
    let proposal = proposals::pending(&conversation, proposal_id).map_err(|e| ai(&e))?;
    let prepared = prepare(state, pack_id, &proposal.change).await?;
    Ok(ProposalReview {
        proposal_id: proposal_id.to_owned(),
        token: prepared.token(),
        changes_pack: proposal.change.changes_pack(),
        preview: prepared.preview,
    })
}

/// As áreas do pack que a proposta muda (para a interface reler).
pub(crate) fn areas_of(change: &ProposalKind) -> Vec<PackArea> {
    match change {
        ProposalKind::EditConfig { .. } => vec![PackArea::Configs, PackArea::History],
        change if change.changes_pack() => vec![PackArea::Inventory, PackArea::History],
        _ => Vec::new(),
    }
}

/// O ponto de segurança antes de aplicar (ARCHITECTURE §11), com a trava do pack. A tarefa
/// que aparece na gaveta é a do fluxo normal que grava em seguida.
async fn safety_point(state: &AppState, pack_id: PackId, what: String) -> Result<String, AppError> {
    let _lock = state.locks.write(pack_id).await;
    let root = pack_root(state, pack_id)?;
    tauri::async_runtime::spawn_blocking(move || {
        let author = warden_packwiz::read_pack(&root)
            .map(|pack| pack.pack.value.author)
            .map_err(|e| AppError::from_domain(&e))?;
        let repo = PackRepo::open(&root).map_err(|e| AppError::from_domain(&e))?;
        repo.check_writable()
            .map_err(|e| AppError::from_domain(&e))?;
        repo.create_safety_point(
            &SafetyReason::AiProposal { what },
            &Identity::for_pack(&author, None),
            Moment::now_utc(),
        )
        .map(|point| point.name)
        .map_err(|e| AppError::from_domain(&e))
    })
    .await
    .map_err(|error| AppError::internal(error.to_string()))?
}

fn describe(proposal: &Proposal) -> String {
    let target = proposal.summary.target.clone().unwrap_or_default();
    match (&proposal.change, &proposal.summary.to) {
        (ProposalKind::AddMod { .. }, _) => format!("adicionar {target}"),
        (ProposalKind::RemoveMod { .. }, _) => format!("remover {target}"),
        (ProposalKind::UpdateMod { .. } | ProposalKind::ChangeVersion { .. }, Some(to)) => {
            format!("{target} para {to}")
        }
        (ProposalKind::SetSide { .. }, _) => format!("lado de {target}"),
        (ProposalKind::EditConfig { .. }, Some(to)) => format!("{target} = {to}"),
        _ => target,
    }
}

/// O packwiz dado (testes) ou o sidecar do app.
fn cli_or(state: &AppState, cli: Option<&Packwiz>) -> Result<Packwiz, AppError> {
    cli.map_or_else(|| packwiz(state), |cli| Ok(cli.clone()))
}

/// Grava o que foi preparado, pelos mesmos `*_impl` dos comandos normais.
async fn execute(
    state: &AppState,
    pack_id: PackId,
    action: Action,
    cli: Option<&Packwiz>,
) -> Result<(), AppError> {
    match action {
        Action::Add(request) => crate::commands::add::apply_impl(state, pack_id, &request)
            .await
            .map(|_| ()),
        Action::Remove(paths) => {
            let cli = cli_or(state, cli)?;
            crate::commands::inventory::remove_impl(state, &cli, pack_id, &paths)
                .await
                .map(|_| ())
        }
        Action::ChangeVersion {
            path,
            expected_current_id,
            version_id,
        } => crate::commands::versions_change::change_impl(
            state,
            pack_id,
            &path,
            expected_current_id.as_deref(),
            &version_id,
        )
        .await
        .map(|_| ()),
        Action::SetSide { path, side } => {
            let cli = cli_or(state, cli)?;
            crate::commands::inventory::set_side_impl(state, &cli, pack_id, &[path], side)
                .await
                .map(|_| ())
        }
        Action::EditConfig { path, text, hash } => {
            let cli = cli_or(state, cli)?;
            crate::commands::configs::write_impl(
                state,
                Some(&cli),
                pack_id,
                ConfigOrigin::Pack,
                &path,
                &text,
                &hash,
            )
            .await
            .map(|_| ())
        }
        Action::TestSettings(profile) => {
            test_profiles::save_profiles(state, pack_id, vec![profile])
        }
        Action::Ui => Ok(()),
    }
}

/// **Aplicar**: confere que vai ser gravado o que foi mostrado (`token`), cria o ponto de
/// segurança quando muda o pack, grava pelo fluxo normal e marca a proposta como aplicada.
pub(crate) async fn apply(
    state: &AppState,
    pack_id: PackId,
    conversation_id: &str,
    proposal_id: &str,
    token: &str,
) -> Result<ProposalApplied, AppError> {
    apply_using(state, pack_id, conversation_id, proposal_id, token, None).await
}

/// [`apply`] com o packwiz dado (`None`: o sidecar do app; os testes passam o de `binaries/`).
async fn apply_using(
    state: &AppState,
    pack_id: PackId,
    conversation_id: &str,
    proposal_id: &str,
    token: &str,
    cli: Option<&Packwiz>,
) -> Result<ProposalApplied, AppError> {
    pack_root(state, pack_id)?;
    let busy = crate::commands::ai::Busy::begin(state, conversation_id)?;
    let store = store(state, pack_id);
    let mut conversation = store.load(conversation_id).map_err(|e| ai(&e))?;
    let proposal = proposals::pending(&conversation, proposal_id)
        .map_err(|e| ai(&e))?
        .clone();
    let prepared = prepare(state, pack_id, &proposal.change).await?;
    if prepared.token() != token {
        return Err(ai(&warden_ai::Error::ProposalStale {
            id: proposal_id.to_owned(),
        }));
    }
    let safety = if proposal.change.changes_pack() {
        Some(safety_point(state, pack_id, describe(&proposal)).await?)
    } else {
        None
    };
    execute(state, pack_id, prepared.action, cli).await?;
    proposals::decide(
        &mut conversation,
        proposal_id,
        ProposalState::Applied {
            at_ms: now_ms(),
            safety_point: safety.clone(),
        },
    )
    .map_err(|e| ai(&e))?;
    busy.save(&store, &conversation)?;
    let proposal = conversation
        .proposals
        .iter()
        .find(|item| item.id == proposal_id)
        .cloned()
        .ok_or_else(|| AppError::internal("proposta sumiu ao gravar"))?;
    Ok(ProposalApplied {
        proposal,
        safety_point: safety,
    })
}

/// **Descartar**: só grava a decisão na conversa; nada muda no pack.
pub(crate) fn discard(
    state: &AppState,
    pack_id: PackId,
    conversation_id: &str,
    proposal_id: &str,
) -> Result<Proposal, AppError> {
    pack_root(state, pack_id)?;
    let busy = crate::commands::ai::Busy::begin(state, conversation_id)?;
    let store = store(state, pack_id);
    let mut conversation = store.load(conversation_id).map_err(|e| ai(&e))?;
    proposals::decide(
        &mut conversation,
        proposal_id,
        ProposalState::Discarded { at_ms: now_ms() },
    )
    .map_err(|e| ai(&e))?;
    busy.save(&store, &conversation)?;
    conversation
        .proposals
        .into_iter()
        .find(|item| item.id == proposal_id)
        .ok_or_else(|| AppError::internal("proposta sumiu ao gravar"))
}

#[cfg(test)]
mod tests;
