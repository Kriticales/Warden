//! Mods iniciais do assistente Criar pack: spark e Crash Assistant (SPEC T03; ADR-0033; D16).
//!
//! Os dados ficam em `data/initial-mods.toml` (embutido no binário), por faixa de loader e de
//! versão do Minecraft. Duas partes:
//!
//! - [`offer`]: o que a etapa "Mods iniciais" mostra para um (versão do Minecraft, loader), com a
//!   versão que seria usada. Não precisa de pack: a etapa vem antes de criá-lo.
//! - [`apply`]: depois do "Pack criado", planeja e grava os itens marcados pelo mesmo caminho de
//!   "Adicionar" (`add_plan` + `add_apply`, uma só transação), grava a config inicial do Crash
//!   Assistant e marca as ferramentas do jogador em `.warden/project.toml`. Um ponto único.

use std::cmp::Ordering;
use std::path::Path;
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};
use warden_core::CancellationToken;
use warden_packwiz::Loader;
use warden_packwiz_cli::Packwiz;

use crate::add::plan::{AddPlanRequest, NodeRole, add_plan};
use crate::add::{
    AddApplyRequest, AddChoice, AddSources, AddedItem, ApplyItem, ChannelPolicy, SourceProject,
    SourceVersion,
};
use crate::player_tools::{
    self, CRASH_ASSISTANT_CONFIG, PROJECT_FILE, PlayerTool, ROLE_CRASH_ASSISTANT,
};
use crate::search::{PackTarget, SourceId};
use crate::side::SideChoice;
use crate::transaction::PackTransaction;
use crate::{Error, ProjectErrorCode as Code, Result};

const DATA: &str = include_str!("../data/initial-mods.toml");

/// Fonte de um item nos dados.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DataSource {
    /// Modrinth (o ID é o do projeto).
    Modrinth,
    /// CurseForge (o ID é o número do projeto).
    Curseforge,
}

impl DataSource {
    /// A fonte do domínio.
    #[must_use]
    pub fn source(self) -> SourceId {
        match self {
            Self::Modrinth => SourceId::Modrinth,
            Self::Curseforge => SourceId::Curseforge,
        }
    }
}

/// Lado escrito nos dados.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DataSide {
    /// Cliente e servidor.
    Both,
    /// Só cliente.
    Client,
    /// Só servidor.
    Server,
}

impl DataSide {
    /// O lado do domínio.
    #[must_use]
    pub fn choice(self) -> SideChoice {
        match self {
            Self::Both => SideChoice::Both,
            Self::Client => SideChoice::Client,
            Self::Server => SideChoice::Server,
        }
    }
}

/// Um item que entra junto com a ferramenta numa faixa (a Fabric API no Fabric).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Extra {
    /// Fonte.
    pub source: DataSource,
    /// ID do projeto.
    pub project: String,
    /// Nome (para a interface e para o relatório do `check-kits`).
    pub name: String,
}

/// Uma faixa de uma ferramenta.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Band {
    /// Loaders (`forge`, `neoforge`, `fabric`).
    pub loaders: Vec<String>,
    /// Primeira versão do Minecraft (inclusiva).
    #[serde(default)]
    pub from: Option<String>,
    /// Última versão do Minecraft (inclusiva).
    #[serde(default)]
    pub to: Option<String>,
    /// Fonte.
    pub source: DataSource,
    /// ID do projeto na fonte.
    pub project: String,
    /// Lado; sem ele vale o que a fonte informa.
    #[serde(default)]
    pub side: Option<DataSide>,
    /// Versão antiga, sem atualizações.
    #[serde(default)]
    pub old: bool,
    /// Versão conhecida, para a interface quando a fonte não é consultada.
    #[serde(default)]
    pub version_label: Option<String>,
    /// Itens que entram junto.
    #[serde(default)]
    pub with: Vec<Extra>,
}

impl Band {
    /// Se a faixa vale para o loader e a versão do Minecraft.
    #[must_use]
    pub fn matches(&self, loader: &str, minecraft: &str) -> bool {
        self.loaders.iter().any(|known| known == loader)
            && version_in_range(minecraft, self.from.as_deref(), self.to.as_deref())
    }
}

/// Uma ferramenta do jogador nos dados.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ToolData {
    /// Identificador (`spark`, `crash-assistant`).
    pub id: String,
    /// Papel em `[player-tools]`.
    pub role: String,
    /// Vem marcada no assistente.
    pub checked: bool,
    /// Versões do Minecraft que o `check-kits` consulta além de `from` e `to`.
    #[serde(default)]
    pub check: Vec<String>,
    /// Faixas, na ordem de preferência.
    pub band: Vec<Band>,
}

impl ToolData {
    /// A primeira faixa que serve ao loader e à versão.
    #[must_use]
    pub fn band_for(&self, loader: &str, minecraft: &str) -> Option<&Band> {
        self.band
            .iter()
            .find(|band| band.matches(loader, minecraft))
    }
}

/// O arquivo `initial-mods.toml`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct InitialModsData {
    /// Versão do formato.
    pub schema: u32,
    /// Ferramentas.
    pub tool: Vec<ToolData>,
}

impl InitialModsData {
    /// Lê um texto no formato de `initial-mods.toml` e confere as regras (IDs preenchidos,
    /// papéis únicos, faixas com versões que existem). Erro [`Code::Internal`] com o motivo.
    pub fn parse(text: &str) -> Result<Self> {
        let data: Self = toml::from_str(text)
            .map_err(|e| Error::new(Code::Internal, format!("initial-mods.toml: {e}")))?;
        data.validate()?;
        Ok(data)
    }

    fn validate(&self) -> Result<()> {
        let bad =
            |reason: String| Error::new(Code::Internal, format!("initial-mods.toml: {reason}"));
        if self.schema != 1 {
            return Err(bad(format!("schema {} não é conhecido", self.schema)));
        }
        let mut seen = std::collections::BTreeSet::new();
        for tool in &self.tool {
            if !seen.insert(tool.id.as_str()) {
                return Err(bad(format!("ferramenta {} repetida", tool.id)));
            }
            if tool.band.is_empty() {
                return Err(bad(format!("{} sem faixas", tool.id)));
            }
            for band in &tool.band {
                check_band(&tool.id, band).map_err(bad)?;
            }
        }
        Ok(())
    }
}

fn check_band(tool: &str, band: &Band) -> std::result::Result<(), String> {
    if band.loaders.is_empty() {
        return Err(format!("{tool}: faixa sem loaders"));
    }
    for loader in &band.loaders {
        if !matches!(loader.as_str(), "forge" | "neoforge" | "fabric") {
            return Err(format!("{tool}: loader {loader} desconhecido"));
        }
    }
    for (what, id, source) in std::iter::once(("projeto", &band.project, band.source)).chain(
        band.with
            .iter()
            .map(|extra| ("extra", &extra.project, extra.source)),
    ) {
        if !valid_project_id(id, source) {
            return Err(format!("{tool}: ID de {what} inválido: {id}"));
        }
    }
    for version in [&band.from, &band.to].into_iter().flatten() {
        if parse_version(version).is_none() {
            return Err(format!("{tool}: versão {version} inválida"));
        }
    }
    if let (Some(from), Some(to)) = (&band.from, &band.to)
        && compare_versions(from, to) == Some(Ordering::Greater)
    {
        return Err(format!(
            "{tool}: faixa de {from} até {to} está ao contrário"
        ));
    }
    Ok(())
}

/// O ID do Modrinth tem 8 letras e números; o da CurseForge é um número.
fn valid_project_id(id: &str, source: DataSource) -> bool {
    match source {
        DataSource::Modrinth => id.len() == 8 && id.chars().all(|c| c.is_ascii_alphanumeric()),
        DataSource::Curseforge => !id.is_empty() && id.chars().all(|c| c.is_ascii_digit()),
    }
}

/// Os mods iniciais embutidos no binário.
///
/// # Panics
///
/// Nunca com os dados do repositório: um teste confere o arquivo.
#[must_use]
pub fn embedded() -> &'static InitialModsData {
    static CELL: OnceLock<InitialModsData> = OnceLock::new();
    CELL.get_or_init(|| {
        InitialModsData::parse(DATA).unwrap_or_else(|error| {
            // Os dados são do repositório e conferidos por teste; chegar aqui é um bug.
            unreachable!("dados embutidos inválidos: {error}")
        })
    })
}

/// Números de uma versão do Minecraft (`1.20.1` → `[1, 20, 1]`); `None` se não for só números.
fn parse_version(version: &str) -> Option<Vec<u64>> {
    version
        .split('.')
        .map(|part| part.parse::<u64>().ok())
        .collect()
}

/// Compara duas versões número a número (`1.20` = `1.20.0`); `None` se alguma não for numérica.
#[must_use]
pub fn compare_versions(a: &str, b: &str) -> Option<Ordering> {
    let (a, b) = (parse_version(a)?, parse_version(b)?);
    let len = a.len().max(b.len());
    for index in 0..len {
        let order = a
            .get(index)
            .copied()
            .unwrap_or(0)
            .cmp(&b.get(index).copied().unwrap_or(0));
        if order != Ordering::Equal {
            return Some(order);
        }
    }
    Some(Ordering::Equal)
}

/// Se a versão está entre `from` e `to` (inclusivas; ausentes não limitam). Versão não numérica
/// (snapshot) nunca está numa faixa.
#[must_use]
pub fn version_in_range(version: &str, from: Option<&str>, to: Option<&str>) -> bool {
    if parse_version(version).is_none() {
        return false;
    }
    from.is_none_or(|from| compare_versions(version, from).is_some_and(Ordering::is_ge))
        && to.is_none_or(|to| compare_versions(version, to).is_some_and(Ordering::is_le))
}

/// Chave do loader (`forge`, `neoforge`, `fabric`) para o catálogo e para o packwiz.
fn loader_of(key: &str) -> Option<Loader> {
    match key {
        "forge" => Some(Loader::Forge),
        "neoforge" => Some(Loader::NeoForge),
        "fabric" => Some(Loader::Fabric),
        _ => None,
    }
}

/// Por que um item não pode ser marcado.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum Unavailable {
    /// Item da CurseForge e a CurseForge não está ligada (sem chave).
    CurseforgeOff,
    /// A fonte não tem versão para este Minecraft e loader.
    NoVersion,
}

/// Um item da etapa "Mods iniciais".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct OfferItem {
    /// Identificador da ferramenta (`spark`, `crash-assistant`).
    pub id: String,
    /// Vem marcada.
    pub checked: bool,
    /// Fonte.
    pub source: SourceId,
    /// ID do projeto na fonte.
    pub project_id: String,
    /// Nome (o da fonte quando ela respondeu; senão, o identificador).
    pub title: String,
    /// Versão que será usada ("1.14.1"), quando conhecida.
    pub version: Option<String>,
    /// Versão antiga e sem atualizações (a interface mostra o aviso).
    pub old: bool,
    /// Lado fixado nos dados.
    pub side: Option<SideChoice>,
    /// Nomes dos itens que entram junto (a Fabric API no Fabric).
    pub with: Vec<String>,
    /// Por que não pode ser marcado.
    pub unavailable: Option<Unavailable>,
}

/// A etapa "Mods iniciais" para um Minecraft e um loader.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct InitialOffer {
    /// Os itens oferecidos, na ordem dos dados.
    pub items: Vec<OfferItem>,
}

fn target_of(minecraft: &str, loader: Loader) -> PackTarget {
    PackTarget {
        minecraft: minecraft.to_owned(),
        game_versions: vec![minecraft.to_owned()],
        loaders: vec![loader],
    }
}

/// A versão que o plano escolheria para o projeto: a mais nova do canal.
async fn pick_version(
    sources: &AddSources,
    source: SourceId,
    project_id: &str,
    target: &PackTarget,
    policy: ChannelPolicy,
    cancel: &CancellationToken,
) -> Result<Option<(SourceProject, SourceVersion)>> {
    let Some(add_source) = sources.get(source) else {
        return Ok(None);
    };
    let projects = add_source
        .projects(&[project_id.to_owned()], cancel)
        .await?;
    let Some(project) = projects.into_iter().next() else {
        return Ok(None);
    };
    let versions = add_source
        .compatible_versions(&project, target, cancel)
        .await?;
    Ok(policy
        .pick(&versions)
        .cloned()
        .map(|version| (project, version)))
}

/// O que a etapa "Mods iniciais" mostra para a versão do Minecraft e o loader.
///
/// Vanilla (`loader` ausente) e faixas sem a ferramenta não oferecem nada. Item da CurseForge sem
/// a CurseForge ligada vem desabilitado (CA-T03-08). Se a fonte do Modrinth não responder, o
/// erro é [`Code::SearchSourceUnavailable`]: a interface mostra "Não foi possível consultar os
/// mods iniciais agora" e deixa seguir sem eles.
pub async fn offer(
    sources: &AddSources,
    policy: ChannelPolicy,
    minecraft: &str,
    loader: Option<&str>,
    cancel: &CancellationToken,
) -> Result<InitialOffer> {
    offer_with(embedded(), sources, policy, minecraft, loader, cancel).await
}

/// [`offer`] com dados dados (testes).
pub async fn offer_with(
    data: &InitialModsData,
    sources: &AddSources,
    policy: ChannelPolicy,
    minecraft: &str,
    loader: Option<&str>,
    cancel: &CancellationToken,
) -> Result<InitialOffer> {
    let Some((loader_key, loader)) = loader.and_then(|key| loader_of(key).map(|l| (key, l))) else {
        return Ok(InitialOffer { items: Vec::new() });
    };
    let target = target_of(minecraft, loader);
    let mut items = Vec::new();
    for tool in &data.tool {
        let Some(band) = tool.band_for(loader_key, minecraft) else {
            continue;
        };
        let source = band.source.source();
        let mut item = OfferItem {
            id: tool.id.clone(),
            checked: tool.checked,
            source,
            project_id: band.project.clone(),
            title: tool.id.clone(),
            version: band.version_label.clone(),
            old: band.old,
            side: band.side.map(DataSide::choice),
            with: band.with.iter().map(|extra| extra.name.clone()).collect(),
            unavailable: None,
        };
        if sources.get(source).is_none() {
            item.unavailable = Some(Unavailable::CurseforgeOff);
            items.push(item);
            continue;
        }
        match pick_version(sources, source, &band.project, &target, policy, cancel).await? {
            Some((project, version)) => {
                item.title = project.title;
                item.version = Some(version.number);
            }
            None => item.unavailable = Some(Unavailable::NoVersion),
        }
        items.push(item);
    }
    Ok(InitialOffer { items })
}

/// Um kit escolhido no assistente: o `id` e os itens que continuaram marcados.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct KitChoice {
    /// Identificador do kit.
    pub id: String,
    /// IDs dos projetos marcados (todos da fonte do item).
    pub projects: Vec<String>,
}

/// O que gravar depois do "Pack criado".
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct InitialModsRequest {
    /// Identificadores das ferramentas marcadas.
    pub tools: Vec<String>,
    /// Kit de desempenho escolhido.
    #[serde(default)]
    pub kit: Option<KitChoice>,
}

/// O que foi gravado.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct InitialModsResult {
    /// Itens que entraram (os escolhidos e as dependências obrigatórias).
    pub added: Vec<AddedItem>,
    /// Nomes dos itens que não entraram (sem versão para o pack ou fonte desligada).
    pub left_out: Vec<String>,
    /// Se a config inicial do Crash Assistant foi gravada.
    pub crash_assistant_config: bool,
    /// Papéis marcados em `[player-tools]`.
    pub player_tools: Vec<String>,
}

/// Grava os mods iniciais no pack recém-criado: plano, gravação, config do Crash Assistant e
/// ferramentas do jogador. Itens sem versão para o pack ou de uma fonte desligada ficam de fora e
/// voltam em [`InitialModsResult::left_out`]; o pack continua válido.
///
/// Erros: os do plano e da gravação ([`Code::SearchSourceUnavailable`] quando o Modrinth não
/// responde, e nada é gravado) e [`Code::InvalidInput`] com ferramenta ou kit desconhecido.
pub async fn apply(
    root: &Path,
    sources: &AddSources,
    policy: ChannelPolicy,
    request: &InitialModsRequest,
    packwiz: &Packwiz,
    cancel: &CancellationToken,
) -> Result<InitialModsResult> {
    let target = PackTarget::read(root)?;
    let loader = target
        .loaders
        .first()
        .copied()
        .ok_or_else(|| Error::new(Code::UnsupportedLoader, "pack sem loader"))?;
    let minecraft = target.minecraft.clone();
    let wanted = wanted_items(&minecraft, loader.key(), request)?;
    let mut left_out = Vec::new();
    let mut choices: Vec<(AddChoice, Wanted)> = Vec::new();
    for item in wanted {
        if sources.get(item.source).is_none() {
            left_out.push(item.name.clone());
        } else {
            choices.push((
                AddChoice {
                    source: item.source,
                    project_id: item.project.clone(),
                    version_id: None,
                },
                item,
            ));
        }
    }
    if choices.is_empty() {
        return Ok(InitialModsResult {
            added: Vec::new(),
            left_out,
            crash_assistant_config: false,
            player_tools: Vec::new(),
        });
    }
    let plan = add_plan(
        root,
        sources,
        policy,
        &AddPlanRequest {
            items: choices.iter().map(|(choice, _)| choice.clone()).collect(),
        },
        cancel,
    )
    .await?;
    for missing in &plan.missing {
        left_out.push(missing.title.clone());
    }
    let mut apply_items = Vec::new();
    for node in &plan.nodes {
        if node.role == NodeRole::Optional {
            continue;
        }
        let side = choices
            .iter()
            .find(|(choice, _)| choice.source.key(&choice.project_id) == node.key)
            .and_then(|(_, wanted)| wanted.side);
        apply_items.push(ApplyItem {
            source: node.source,
            project_id: node.project_id.clone(),
            version_id: node.version_id.clone(),
            side,
            replaces: None,
        });
    }
    if apply_items.is_empty() {
        return Ok(InitialModsResult {
            added: Vec::new(),
            left_out,
            crash_assistant_config: false,
            player_tools: Vec::new(),
        });
    }
    let result = crate::add::add_apply(
        root,
        sources,
        &AddApplyRequest { items: apply_items },
        packwiz,
        cancel,
    )
    .await?;
    let marked = mark_player_tools(root, &choices, &result.added, packwiz, cancel).await?;
    Ok(InitialModsResult {
        added: result.added,
        left_out,
        crash_assistant_config: marked.config,
        player_tools: marked.roles,
    })
}

/// Um item a pedir ao plano.
#[derive(Debug, Clone)]
struct Wanted {
    source: SourceId,
    project: String,
    name: String,
    side: Option<SideChoice>,
    /// Papel de ferramenta do jogador, se for uma.
    role: Option<String>,
}

fn wanted_items(
    minecraft: &str,
    loader: &str,
    request: &InitialModsRequest,
) -> Result<Vec<Wanted>> {
    let data = embedded();
    let mut wanted = Vec::new();
    for id in &request.tools {
        let tool = data
            .tool
            .iter()
            .find(|tool| &tool.id == id)
            .ok_or_else(|| {
                Error::new(Code::InvalidInput, format!("ferramenta {id} desconhecida"))
                    .param("field", "tools")
            })?;
        let Some(band) = tool.band_for(loader, minecraft) else {
            continue;
        };
        wanted.push(Wanted {
            source: band.source.source(),
            project: band.project.clone(),
            name: tool.id.clone(),
            side: band.side.map(DataSide::choice),
            role: Some(tool.role.clone()),
        });
        for extra in &band.with {
            wanted.push(Wanted {
                source: extra.source.source(),
                project: extra.project.clone(),
                name: extra.name.clone(),
                side: None,
                role: None,
            });
        }
    }
    if let Some(choice) = &request.kit {
        wanted.extend(
            crate::kits::wanted_for(minecraft, loader, choice)?
                .into_iter()
                .map(|item| Wanted {
                    source: item.source,
                    project: item.project,
                    name: item.name,
                    side: item.side,
                    role: None,
                }),
        );
    }
    Ok(wanted)
}

struct Marked {
    config: bool,
    roles: Vec<String>,
}

/// Grava a config inicial do Crash Assistant e a tabela `[player-tools]`.
async fn mark_player_tools(
    root: &Path,
    choices: &[(AddChoice, Wanted)],
    added: &[AddedItem],
    packwiz: &Packwiz,
    cancel: &CancellationToken,
) -> Result<Marked> {
    let mut tools = Vec::new();
    for (choice, wanted) in choices {
        let Some(role) = &wanted.role else { continue };
        let key = choice.source.key(&choice.project_id);
        if let Some(item) = added.iter().find(|item| item.key == key) {
            tools.push(PlayerTool {
                role: role.clone(),
                path: item.path.clone(),
            });
        }
    }
    if tools.is_empty() {
        return Ok(Marked {
            config: false,
            roles: Vec::new(),
        });
    }
    let current = std::fs::read_to_string(root.join(PROJECT_FILE))
        .map_err(|e| Error::new(Code::InvalidPack, format!("{PROJECT_FILE}: {e}")))?;
    let mut tx = PackTransaction::new(root.to_path_buf());
    tx.write(
        PROJECT_FILE,
        player_tools::with_tools(&current, &tools)?.into_bytes(),
    )?;
    let has_crash_assistant = tools.iter().any(|tool| tool.role == ROLE_CRASH_ASSISTANT);
    let write_config = has_crash_assistant && !root.join(CRASH_ASSISTANT_CONFIG).exists();
    if write_config {
        tx.write(
            CRASH_ASSISTANT_CONFIG,
            player_tools::crash_assistant_config().into_bytes(),
        )?;
    }
    tx.commit(packwiz, cancel).await?;
    Ok(Marked {
        config: write_config,
        roles: tools.into_iter().map(|tool| tool.role).collect(),
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    #[test]
    fn os_dados_embutidos_sao_validos() {
        let data = embedded();
        let ids: Vec<&str> = data.tool.iter().map(|tool| tool.id.as_str()).collect();
        assert_eq!(ids, ["spark", "crash-assistant"]);
        assert!(data.tool.iter().all(|tool| tool.checked));
    }

    #[test]
    fn versoes_comparam_numero_a_numero() {
        assert_eq!(compare_versions("1.20", "1.20.0"), Some(Ordering::Equal));
        assert_eq!(compare_versions("1.9", "1.10"), Some(Ordering::Less));
        assert_eq!(compare_versions("26.3", "1.21.1"), Some(Ordering::Greater));
        assert_eq!(compare_versions("1.21-pre1", "1.21"), None);
        assert!(version_in_range("1.12.2", Some("1.12.2"), Some("1.12.2")));
        assert!(!version_in_range("1.12.1", Some("1.12.2"), None));
        assert!(!version_in_range("24w14a", None, None));
    }

    #[test]
    fn faixas_do_spark_seguem_a_pesquisa() {
        let spark = embedded().tool.iter().find(|t| t.id == "spark").unwrap();
        let band = |loader, mc| spark.band_for(loader, mc);
        // Modrinth de 1.16.5 em diante no Forge, com a Fabric API no Fabric.
        assert_eq!(
            band("forge", "1.16.5").unwrap().source,
            DataSource::Modrinth
        );
        assert!(band("forge", "1.16.4").is_none());
        let fabric = band("fabric", "1.21.1").unwrap();
        assert_eq!(fabric.with[0].name, "Fabric API");
        assert!(band("fabric", "1.16.5").is_none());
        assert!(band("neoforge", "1.20.2").is_none());
        assert!(band("neoforge", "1.20.3").is_some());
        // CurseForge antiga em 1.7.10 e 1.12.2.
        let old = band("forge", "1.7.10").unwrap();
        assert_eq!((old.source, old.old), (DataSource::Curseforge, true));
        assert_eq!(old.version_label.as_deref(), Some("1.10.19"));
        let old = band("forge", "1.12.2").unwrap();
        assert_eq!(old.version_label.as_deref(), Some("1.6.3"));
        // Crash Assistant em todas as faixas, sempre pelo Modrinth.
        let crash = embedded()
            .tool
            .iter()
            .find(|t| t.id == "crash-assistant")
            .unwrap();
        for (loader, mc) in [
            ("forge", "1.7.10"),
            ("forge", "1.12.2"),
            ("fabric", "1.14.4"),
            ("neoforge", "26.3"),
        ] {
            assert_eq!(
                crash.band_for(loader, mc).unwrap().source,
                DataSource::Modrinth
            );
        }
    }

    #[test]
    fn dados_invalidos_sao_recusados() {
        let base = "schema = 1\n[[tool]]\nid = \"x\"\nrole = \"x\"\nchecked = true\n[[tool.band]]\nloaders = [\"forge\"]\nsource = \"modrinth\"\n";
        assert!(InitialModsData::parse(&format!("{base}project = \"l6YH9Als\"\n")).is_ok());
        for bad in [
            "project = \"curto\"\n",
            "project = \"l6YH9Als\"\nfrom = \"1.x\"\n",
            "project = \"l6YH9Als\"\nfrom = \"1.20\"\nto = \"1.12\"\n",
        ] {
            assert!(
                InitialModsData::parse(&format!("{base}{bad}")).is_err(),
                "{bad}"
            );
        }
        assert!(InitialModsData::parse("schema = 2\ntool = []\n").is_err());
        assert!(InitialModsData::parse(&base.replace("forge", "quilt")).is_err());
    }
}
