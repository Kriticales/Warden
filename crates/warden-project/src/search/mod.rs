//! Busca combinada de projetos (SPEC T08; ADR-0027; ARCHITECTURE §17).
//!
//! Uma busca consulta as fontes ativas ao mesmo tempo (cada uma com tempo-limite próprio) e
//! entrega **uma** página de até [`PAGE_SIZE`] itens já combinados:
//!
//! - **Fontes plugáveis:** cada fonte implementa [`SearchSource`]. A P1-09 liga o Modrinth
//!   ([`modrinth::ModrinthSearch`]); a P1-10 acrescenta a CurseForge sem mudar este motor.
//! - **Ordem** ([`merge`]): relevância intercalada pela posição em cada fonte; downloads,
//!   atualização e novos mesclados pelos números de cada fonte.
//! - **Sem duplicatas:** o mesmo projeto nas duas fontes (mesmo autor e mesmo slug ou nome
//!   normalizados, [`crate::dedup`]) vira um item com as duas referências.
//! - **Paginação por fonte:** o [`SearchCursor`] guarda quantos itens de cada fonte já foram
//!   entregues; a próxima página pede a cada fonte a partir dali.
//! - **Resultado parcial:** uma fonte com erro ou sem resposta no tempo-limite não derruba a
//!   busca: os itens das outras chegam com o aviso [`SourceWarning`]
//!   (`SEARCH_SOURCE_UNAVAILABLE`). Só quando **todas** as fontes consultadas falham a busca
//!   devolve erro.

pub mod merge;
pub mod modrinth;

use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use warden_core::CancellationToken;
use warden_packwiz::{Loader, PackManifest};

use crate::{Error, ProjectErrorCode as Code, Result};

/// Itens por página, já combinados (SPEC T08: "Rolagem infinita, 20 itens por página").
pub const PAGE_SIZE: usize = 20;

/// Tempo-limite de cada fonte numa busca. Passou dele, a fonte conta como indisponível e os
/// resultados das outras aparecem (CA-T08-09: a interface nunca fica carregando para sempre).
pub const SOURCE_TIMEOUT: Duration = Duration::from_secs(15);

/// Contagem (downloads, totais) como número da interface: o `specta` não exporta `u64`, e
/// contagens desse tamanho cabem com folga num `f64`.
#[allow(clippy::cast_precision_loss)]
#[must_use]
pub fn count(value: u64) -> f64 {
    value as f64
}

/// Uma fonte de projetos.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, specta::Type,
)]
#[serde(rename_all = "camelCase")]
pub enum SourceId {
    /// Modrinth (padrão quando o projeto está nas duas: informa o lado e tem cache local).
    Modrinth,
    /// CurseForge (P1-10).
    Curseforge,
}

impl SourceId {
    /// Nome da fonte na interface.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Modrinth => "Modrinth",
            Self::Curseforge => "CurseForge",
        }
    }

    /// Prefixo da chave estável dos itens (`modrinth:<projeto>`), o mesmo do inventário.
    #[must_use]
    pub fn key_prefix(self) -> &'static str {
        match self {
            Self::Modrinth => "modrinth",
            Self::Curseforge => "curseforge",
        }
    }

    /// Chave estável de um projeto desta fonte (`modrinth:AANobbMI`).
    #[must_use]
    pub fn key(self, project_id: &str) -> String {
        format!("{}:{project_id}", self.key_prefix())
    }
}

/// Tipo de projeto buscado (seletor **Tipo** da página Adicionar). Modpacks chegam com a
/// P1-17.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum ProjectKind {
    /// Mods (`mods/`).
    Mod,
    /// Resource packs (`resourcepacks/`).
    ResourcePack,
    /// Shaders (`shaderpacks/`).
    Shader,
}

impl ProjectKind {
    /// Pasta do pack onde os itens deste tipo ficam (ARCHITECTURE §6.2).
    #[must_use]
    pub fn folder(self) -> &'static str {
        match self {
            Self::Mod => "mods",
            Self::ResourcePack => "resourcepacks",
            Self::Shader => "shaderpacks",
        }
    }
}

/// Ordem dos resultados.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize, specta::Type,
)]
#[serde(rename_all = "camelCase")]
pub enum SearchSort {
    /// Relevância: intercalada pela posição em cada fonte.
    #[default]
    Relevance,
    /// Mais baixados.
    Downloads,
    /// Atualizados recentemente.
    Updated,
    /// Mais novos.
    Newest,
}

/// Filtro **Fonte** ("Mais filtros").
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize, specta::Type,
)]
#[serde(rename_all = "camelCase")]
pub enum SourceFilter {
    /// Todas as fontes ativas.
    #[default]
    All,
    /// Só o Modrinth.
    Modrinth,
    /// Só a CurseForge.
    Curseforge,
}

impl SourceFilter {
    /// Se a fonte entra na busca.
    #[must_use]
    pub fn allows(self, source: SourceId) -> bool {
        match self {
            Self::All => true,
            Self::Modrinth => source == SourceId::Modrinth,
            Self::Curseforge => source == SourceId::Curseforge,
        }
    }
}

/// Filtro **Ambiente**.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum EnvironmentFilter {
    /// Funciona no cliente.
    Client,
    /// Funciona no servidor.
    Server,
}

/// Onde a busca continua: quantos itens de cada fonte já foram entregues. Opaco para a
/// interface, que só devolve o `next` da página anterior.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SearchCursor {
    /// Deslocamento de cada fonte.
    pub offsets: Vec<SourceOffset>,
}

/// Deslocamento de uma fonte no [`SearchCursor`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SourceOffset {
    /// A fonte.
    pub source: SourceId,
    /// Itens já entregues desta fonte.
    pub offset: u32,
    /// A fonte não tem mais resultados.
    pub done: bool,
}

impl SearchCursor {
    fn get(&self, source: SourceId) -> Option<&SourceOffset> {
        self.offsets.iter().find(|entry| entry.source == source)
    }
}

/// Uma busca da página Adicionar. Os filtros travados (versão do Minecraft, loader e tipo)
/// vêm do pack, nunca da interface.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SearchRequest {
    /// Texto buscado (vazio: só os filtros).
    #[serde(default)]
    pub query: String,
    /// Tipo.
    pub kind: ProjectKind,
    /// Ordem.
    #[serde(default)]
    pub sort: SearchSort,
    /// Fonte.
    #[serde(default)]
    pub source: SourceFilter,
    /// Ambiente (nenhum: qualquer).
    #[serde(default)]
    pub environment: Option<EnvironmentFilter>,
    /// "Mostrar também os sem versão compatível": tira os filtros de versão e loader (os
    /// itens sem versão para o pack aparecem marcados, sem caixa de seleção).
    #[serde(default)]
    pub include_incompatible: bool,
    /// Categoria curada escolhida na coluna de filtros (`id` do mapeamento da `warden-discovery`);
    /// nenhuma: todas. O app a resolve em [`CategoryFilter`] antes de buscar.
    #[serde(default)]
    pub category: Option<String>,
    /// Página seguinte (`next` da anterior); nenhum: a primeira.
    #[serde(default)]
    pub cursor: Option<SearchCursor>,
}

/// Uma categoria curada já traduzida para as categorias de cada fonte (SPEC T08: "categoria sem
/// par filtra só a fonte que a tem"). Uma lista vazia significa que a fonte não tem par: ela
/// fica de fora da busca com categoria.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CategoryFilter {
    /// Categorias do Modrinth (valem como "ou").
    pub modrinth: Vec<String>,
    /// IDs de categoria da CurseForge (valem como "ou").
    pub curseforge: Vec<u32>,
}

impl CategoryFilter {
    /// Se a fonte tem par para a categoria.
    #[must_use]
    pub fn applies_to(&self, source: SourceId) -> bool {
        match source {
            SourceId::Modrinth => !self.modrinth.is_empty(),
            SourceId::Curseforge => !self.curseforge.is_empty(),
        }
    }
}

/// Referência de um resultado numa fonte.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SourceRef {
    /// A fonte.
    pub source: SourceId,
    /// ID do projeto na fonte (o Modrinth usa o ID, nunca o slug, que pode mudar).
    pub project_id: String,
    /// Slug do projeto na fonte (para o link da página).
    pub slug: String,
    /// Downloads nesta fonte.
    pub downloads: f64,
}

/// Um resultado da busca combinada.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SearchResult {
    /// Chave estável do item (a da fonte preferida, a mesma do inventário).
    pub key: String,
    /// As fontes do item, a preferida primeiro (Modrinth antes da CurseForge).
    pub sources: Vec<SourceRef>,
    /// Nome.
    pub title: String,
    /// Autor.
    pub author: String,
    /// Resumo.
    pub summary: String,
    /// Ícone.
    pub icon_url: Option<String>,
    /// Downloads somados das fontes.
    pub downloads: f64,
    /// Data da última atualização (RFC 3339).
    pub updated: String,
    /// Já está no pack (pelo projeto em qualquer uma das fontes): sem caixa de seleção.
    pub in_pack: bool,
    /// Tem versão para o Minecraft e o loader do pack. Falso só com "Mostrar também os sem
    /// versão compatível".
    pub compatible: bool,
    /// CurseForge: o autor bloqueou downloads por apps de terceiros ("Download manual
    /// necessário").
    pub manual_download: bool,
}

/// Aviso de uma fonte que ficou de fora desta página.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SourceWarning {
    /// A fonte.
    pub source: SourceId,
    /// Por quê.
    pub reason: SourceWarningReason,
    /// Detalhe técnico (sem segredos), para "Detalhes técnicos".
    pub detail: Option<String>,
}

/// Por que uma fonte ficou de fora.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum SourceWarningReason {
    /// Erro ou tempo esgotado (`SEARCH_SOURCE_UNAVAILABLE`): "Não foi possível buscar no
    /// <fonte> agora." com **Tentar de novo**.
    Unavailable,
    /// Sem chave da CurseForge: a fonte nem foi consultada (P1-10).
    KeyMissing,
    /// A CurseForge recusou a chave (P1-10).
    KeyRejected,
}

/// Uma página de resultados.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SearchPage {
    /// Até [`PAGE_SIZE`] itens, já combinados.
    pub items: Vec<SearchResult>,
    /// As fontes consultadas nesta página.
    pub sources: Vec<SourceId>,
    /// Total aproximado (soma dos totais das fontes que responderam).
    pub total: f64,
    /// Onde continuar; nenhum: acabou.
    pub next: Option<SearchCursor>,
    /// Fontes que ficaram de fora desta página, e por quê.
    pub warnings: Vec<SourceWarning>,
}

/// O que o pack aceita: versões do Minecraft e loaders (filtros travados da busca).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackTarget {
    /// Versão do Minecraft do pack.
    pub minecraft: String,
    /// A versão do pack seguida das `acceptable-game-versions`.
    pub game_versions: Vec<String>,
    /// Loaders cujos mods o pack aceita (`compatible_loaders` do packwiz), o do pack primeiro.
    pub loaders: Vec<Loader>,
}

impl PackTarget {
    /// Do `pack.toml`. Erro [`Code::InvalidPack`] sem versão do Minecraft.
    pub fn from_manifest(manifest: &PackManifest) -> Result<Self> {
        let minecraft = manifest
            .minecraft_version()
            .filter(|version| !version.trim().is_empty())
            .ok_or_else(|| Error::new(Code::InvalidPack, "pack.toml sem versão do Minecraft"))?
            .to_owned();
        let mut game_versions = vec![minecraft.clone()];
        for version in manifest.acceptable_game_versions() {
            if !game_versions.iter().any(|known| known == version) {
                game_versions.push(version.to_owned());
            }
        }
        Ok(Self {
            minecraft,
            game_versions,
            loaders: manifest.compatible_loaders(),
        })
    }

    /// Lê o `pack.toml` da pasta do pack.
    pub fn read(root: &std::path::Path) -> Result<Self> {
        let text = std::fs::read_to_string(root.join(warden_packwiz::PACK_FILE))
            .map_err(|e| Error::new(Code::InvalidPack, e.to_string()))?;
        let manifest =
            PackManifest::parse(&text).map_err(|e| Error::new(Code::InvalidPack, e.to_string()))?;
        Self::from_manifest(&manifest.value)
    }

    /// Nomes dos loaders aceitos para um tipo de projeto (vazio: sem filtro de loader).
    /// Resource packs e shaders não dependem do loader do pack.
    #[must_use]
    pub fn loader_names(&self, kind: ProjectKind) -> Vec<String> {
        match kind {
            ProjectKind::Mod => self.loaders.iter().map(|l| l.key().to_owned()).collect(),
            ProjectKind::ResourcePack | ProjectKind::Shader => Vec::new(),
        }
    }
}

/// Consulta repassada a uma fonte.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceQuery {
    /// Texto.
    pub query: String,
    /// Tipo.
    pub kind: ProjectKind,
    /// Ordem.
    pub sort: SearchSort,
    /// Ambiente.
    pub environment: Option<EnvironmentFilter>,
    /// Sem os filtros de versão e loader.
    pub include_incompatible: bool,
    /// Categoria escolhida, traduzida para esta busca (nenhuma: todas).
    pub category: Option<CategoryFilter>,
    /// O que o pack aceita.
    pub target: PackTarget,
    /// A partir de qual item.
    pub offset: u32,
    /// Quantos itens.
    pub limit: u32,
}

/// Um resultado como a fonte o entrega, antes da combinação.
#[derive(Debug, Clone, PartialEq)]
pub struct SourceHit {
    /// Referência do projeto.
    pub reference: SourceRef,
    /// Nome.
    pub title: String,
    /// Autor (Modrinth `author`; CurseForge o primeiro de `authors[].name`).
    pub author: String,
    /// Resumo.
    pub summary: String,
    /// Ícone.
    pub icon_url: Option<String>,
    /// Data da última atualização (RFC 3339).
    pub updated: String,
    /// Data de criação (RFC 3339).
    pub created: String,
    /// Tem versão para o pack.
    pub compatible: bool,
    /// Download manual necessário (CurseForge).
    pub manual_download: bool,
}

/// Uma página de uma fonte.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SourcePage {
    /// Resultados, na ordem da fonte.
    pub hits: Vec<SourceHit>,
    /// Total de resultados que a fonte diz ter.
    pub total: u64,
}

/// Erro de uma fonte numa busca.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceFailure {
    /// Por quê.
    pub reason: SourceWarningReason,
    /// Detalhe técnico, sem segredos.
    pub detail: String,
}

impl SourceFailure {
    /// Fonte fora do ar ou com erro.
    #[must_use]
    pub fn unavailable(detail: impl Into<String>) -> Self {
        Self {
            reason: SourceWarningReason::Unavailable,
            detail: detail.into(),
        }
    }
}

/// Links do projeto (abrem no navegador).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ProjectLinks {
    /// Página do projeto na fonte.
    pub page: Option<String>,
    /// Problemas conhecidos (issues).
    pub issues: Option<String>,
    /// Código.
    pub source: Option<String>,
    /// Wiki.
    pub wiki: Option<String>,
    /// Discord.
    pub discord: Option<String>,
}

/// A pré-visualização de um projeto (coluna da direita da página Adicionar).
#[derive(Debug, Clone, PartialEq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ProjectPreview {
    /// Fonte.
    pub source: SourceId,
    /// ID do projeto.
    pub project_id: String,
    /// Slug.
    pub slug: String,
    /// Nome.
    pub title: String,
    /// Resumo.
    pub summary: String,
    /// Descrição longa (a interface higieniza; ARCHITECTURE §18).
    pub body: String,
    /// Formato da descrição.
    pub body_format: crate::details::DescriptionFormat,
    /// Ícone.
    pub icon_url: Option<String>,
    /// Downloads.
    pub downloads: f64,
    /// Última atualização (RFC 3339).
    pub updated: String,
    /// Licença (nome ou identificador SPDX), se informada.
    pub license: Option<String>,
    /// Lado informado pelo projeto.
    pub side: Option<crate::side::SideChoice>,
    /// Links.
    pub links: ProjectLinks,
}

/// Uma versão no seletor **Versão** da pré-visualização.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct VersionOption {
    /// ID na fonte (vai para o `add_plan`).
    pub id: String,
    /// Número legível.
    pub number: String,
    /// Canal.
    pub channel: crate::add::VersionChannel,
    /// Data de publicação (RFC 3339).
    pub published: String,
    /// Nome do arquivo.
    pub file_name: String,
    /// SHA-1 do arquivo (comparação entre fontes, P1-10).
    pub sha1: Option<String>,
}

/// As versões de um projeto que servem para o pack, a mais nova primeiro.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ProjectVersions {
    /// As versões (vazia: nenhuma serve para o pack).
    pub versions: Vec<VersionOption>,
    /// A versão padrão: a mais nova do canal configurado (ou a mais nova de todas, se não
    /// houver nenhuma do canal).
    pub default_id: Option<String>,
}

/// Versões de um projeto para o seletor da pré-visualização.
///
/// Erro [`Code::SearchSourceUnavailable`] se a fonte não estiver ligada ou não responder.
pub async fn project_versions(
    sources: &crate::add::AddSources,
    target: &PackTarget,
    policy: crate::add::ChannelPolicy,
    source: SourceId,
    project_id: &str,
    cancel: &CancellationToken,
) -> Result<ProjectVersions> {
    let add = sources
        .get(source)
        .ok_or_else(|| crate::add::source_off(source))?;
    let Some(project) = add
        .projects(&[project_id.to_owned()], cancel)
        .await?
        .into_iter()
        .next()
    else {
        return Ok(ProjectVersions {
            versions: Vec::new(),
            default_id: None,
        });
    };
    let versions = add.compatible_versions(&project, target, cancel).await?;
    let default_id = policy.pick(&versions).map(|version| version.id.clone());
    Ok(ProjectVersions {
        versions: versions
            .into_iter()
            .map(|version| VersionOption {
                file_name: version
                    .file
                    .as_ref()
                    .map(|file| file.name.clone())
                    .unwrap_or_default(),
                sha1: version.file.as_ref().and_then(|file| file.sha1.clone()),
                id: version.id,
                number: version.number,
                channel: version.channel,
                published: version.published,
            })
            .collect(),
        default_id,
    })
}

impl ActiveSource {
    /// A fonte, se estiver ligada.
    #[must_use]
    pub fn ready(&self, id: SourceId) -> Option<&Arc<dyn SearchSource>> {
        match self {
            Self::Ready(source) if source.id() == id => Some(source),
            _ => None,
        }
    }
}

/// A pré-visualização de um projeto. Erro [`Code::SearchSourceUnavailable`] se a fonte não
/// estiver ligada.
pub async fn project_preview(
    sources: &[ActiveSource],
    source: SourceId,
    project_id: &str,
    cancel: &CancellationToken,
) -> Result<ProjectPreview> {
    let ready = sources
        .iter()
        .find_map(|active| active.ready(source))
        .ok_or_else(|| crate::add::source_off(source))?;
    ready.preview(project_id, cancel).await
}

/// Uma fonte de busca. Implementada pelo Modrinth (P1-09) e pela CurseForge (P1-10).
#[async_trait::async_trait]
pub trait SearchSource: Send + Sync {
    /// Qual fonte é.
    fn id(&self) -> SourceId;

    /// A pré-visualização de um projeto desta fonte. Erros: projeto inexistente
    /// ([`Code::ItemNotFound`]) ou fonte indisponível ([`Code::SearchSourceUnavailable`]).
    async fn preview(&self, project_id: &str, cancel: &CancellationToken)
    -> Result<ProjectPreview>;

    /// Uma página de resultados desta fonte, já filtrada pelo pack (salvo
    /// `include_incompatible`).
    async fn search(
        &self,
        query: &SourceQuery,
        cancel: &CancellationToken,
    ) -> std::result::Result<SourcePage, SourceFailure>;
}

/// Fonte já ligada no app, ou o motivo de estar de fora (sem chave, chave recusada).
#[derive(Clone)]
pub enum ActiveSource {
    /// Consultada.
    Ready(Arc<dyn SearchSource>),
    /// De fora: vira aviso sem nenhuma requisição (CA-T08-06).
    Off(SourceId, SourceWarningReason),
}

/// Projetos que já estão no pack, pelas chaves do inventário (`modrinth:<id>`).
pub type InstalledKeys = HashSet<String>;

/// Executa uma busca combinada.
///
/// `sources` são as fontes do app (as de fora viram aviso sem requisição); `installed` são as
/// chaves dos projetos que já estão no pack (marca "Já no pack"). Erro
/// [`Code::SearchSourceUnavailable`] (com o parâmetro `source`) só quando todas as fontes
/// consultadas falharam.
pub async fn search(
    sources: &[ActiveSource],
    target: &PackTarget,
    installed: &InstalledKeys,
    request: &SearchRequest,
    cancel: &CancellationToken,
) -> Result<SearchPage> {
    search_in_category(sources, target, installed, request, None, cancel).await
}

/// Como [`search`], restrita a uma categoria curada. As fontes sem par para a categoria nem são
/// consultadas (nem avisadas): a categoria filtra só quem a tem.
pub async fn search_in_category(
    sources: &[ActiveSource],
    target: &PackTarget,
    installed: &InstalledKeys,
    request: &SearchRequest,
    category: Option<&CategoryFilter>,
    cancel: &CancellationToken,
) -> Result<SearchPage> {
    search_with_timeout(
        sources,
        target,
        installed,
        request,
        category,
        cancel,
        SOURCE_TIMEOUT,
    )
    .await
}

/// A consulta repassada a uma fonte.
fn source_query(
    request: &SearchRequest,
    target: &PackTarget,
    category: Option<&CategoryFilter>,
    offset: u32,
) -> SourceQuery {
    SourceQuery {
        category: category.cloned(),
        query: request.query.trim().to_owned(),
        kind: request.kind,
        sort: request.sort,
        environment: request.environment,
        include_incompatible: request.include_incompatible,
        target: target.clone(),
        offset,
        limit: u32::try_from(PAGE_SIZE).unwrap_or(20),
    }
}

/// Uma fonte, com tempo-limite: passou dele, a fonte conta como indisponível.
async fn run_source(
    source: Arc<dyn SearchSource>,
    query: SourceQuery,
    cancel: CancellationToken,
    timeout: Duration,
) -> (
    SourceId,
    u32,
    std::result::Result<SourcePage, SourceFailure>,
) {
    let id = source.id();
    let result = match tokio::time::timeout(timeout, source.search(&query, &cancel)).await {
        Ok(result) => result,
        Err(_) => Err(SourceFailure::unavailable(format!(
            "sem resposta em {} s",
            timeout.as_secs()
        ))),
    };
    (id, query.offset, result)
}

/// Uma fonte que falhou fica de fora do resto desta busca (senão os itens dela chegariam fora
/// da intercalação, páginas depois); **Tentar de novo** recomeça a busca do início.
fn failed_offsets(
    request: &SearchRequest,
    failures: &[(SourceId, SourceFailure)],
) -> Vec<SourceOffset> {
    failures
        .iter()
        .map(|(id, _)| SourceOffset {
            source: *id,
            offset: request
                .cursor
                .as_ref()
                .and_then(|cursor| cursor.get(*id))
                .map_or(0, |state| state.offset),
            done: true,
        })
        .collect()
}

/// Respostas de cada fonte, na ordem em que chegaram, e as falhas (que viram avisos).
async fn collect(
    mut tasks: SourceTasks,
    warnings: &mut Vec<SourceWarning>,
) -> Result<(
    Vec<(SourceId, u32, SourcePage)>,
    Vec<(SourceId, SourceFailure)>,
)> {
    let mut pages = Vec::new();
    let mut failures = Vec::new();
    while let Some(joined) = tasks.join_next().await {
        let (id, offset, result) =
            joined.map_err(|e| Error::new(Code::Internal, format!("busca interrompida: {e}")))?;
        match result {
            Ok(page) => pages.push((id, offset, page)),
            Err(failure) => {
                tracing::warn!(fonte = id.label(), detalhe = %failure.detail, "fonte de busca indisponível");
                warnings.push(SourceWarning {
                    source: id,
                    reason: failure.reason,
                    detail: Some(failure.detail.clone()),
                });
                failures.push((id, failure));
            }
        }
    }
    Ok((pages, failures))
}

/// As buscas em andamento, uma por fonte.
type SourceTasks = tokio::task::JoinSet<(
    SourceId,
    u32,
    std::result::Result<SourcePage, SourceFailure>,
)>;

pub(crate) async fn search_with_timeout(
    sources: &[ActiveSource],
    target: &PackTarget,
    installed: &InstalledKeys,
    request: &SearchRequest,
    category: Option<&CategoryFilter>,
    cancel: &CancellationToken,
    timeout: Duration,
) -> Result<SearchPage> {
    let mut warnings = Vec::new();
    let mut tasks = SourceTasks::new();
    let mut queried = Vec::new();
    let mut previous_offsets = Vec::new();
    for active in sources {
        let id = match active {
            ActiveSource::Off(source, _) => *source,
            ActiveSource::Ready(source) => source.id(),
        };
        if category.is_some_and(|category| !category.applies_to(id)) {
            continue;
        }
        match active {
            ActiveSource::Off(source, reason) => {
                if request.source.allows(*source) {
                    warnings.push(SourceWarning {
                        source: *source,
                        reason: *reason,
                        detail: None,
                    });
                }
            }
            ActiveSource::Ready(source) => {
                let id = source.id();
                if !request.source.allows(id) {
                    continue;
                }
                let state = request.cursor.as_ref().and_then(|cursor| cursor.get(id));
                if state.is_some_and(|state| state.done) {
                    previous_offsets.push(SourceOffset {
                        source: id,
                        offset: state.map_or(0, |state| state.offset),
                        done: true,
                    });
                    continue;
                }
                let offset = state.map_or(0, |state| state.offset);
                queried.push(id);
                tasks.spawn(run_source(
                    Arc::clone(source),
                    source_query(request, target, category, offset),
                    cancel.clone(),
                    timeout,
                ));
            }
        }
    }
    let (mut pages, failures) = collect(tasks, &mut warnings).await?;
    if cancel.is_cancelled() {
        return Err(Error::new(Code::Internal, "busca cancelada"));
    }
    if pages.is_empty()
        && let Some((source, failure)) = failures.first()
    {
        return Err(
            Error::new(Code::SearchSourceUnavailable, failure.detail.clone())
                .param("source", source.label()),
        );
    }
    // Ordem estável das fontes (Modrinth primeiro), independente de quem respondeu antes.
    pages.sort_by_key(|(id, _, _)| *id);
    warnings.sort_by_key(|warning| warning.source);
    let total = pages.iter().map(|(_, _, page)| count(page.total)).sum();
    let batches: Vec<merge::SourceBatch<'_>> = pages
        .iter()
        .map(|(id, offset, page)| merge::SourceBatch {
            source: *id,
            offset: *offset,
            total: page.total,
            hits: &page.hits,
        })
        .collect();
    let merged = merge::combine(&batches, request.sort, PAGE_SIZE);
    let mut offsets = previous_offsets;
    offsets.extend(merged.offsets);
    offsets.extend(failed_offsets(request, &failures));
    offsets.sort_by_key(|entry| entry.source);
    let more = offsets.iter().any(|entry| !entry.done);
    let items = merged
        .items
        .into_iter()
        .map(|item| item.into_result(installed))
        .collect();
    Ok(SearchPage {
        items,
        sources: queried,
        total,
        next: more.then_some(SearchCursor { offsets }),
        warnings,
    })
}

#[cfg(test)]
mod tests {
    //! O motor com duas fontes simuladas: intercalação, deduplicação, paginação por fonte e
    //! resultado parcial (critério da P1-09 no ROADMAP).

    use std::sync::Mutex;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;

    /// Fonte simulada: `total` resultados `<prefixo><n>`, de um autor por resultado, ou falha.
    struct Fake {
        id: SourceId,
        prefix: &'static str,
        total: u32,
        /// Resultados extras no começo (para simular o mesmo projeto nas duas fontes).
        shared: Vec<(&'static str, &'static str)>,
        behavior: Behavior,
        calls: AtomicUsize,
        offsets: Mutex<Vec<u32>>,
    }

    #[derive(Clone, Copy, PartialEq)]
    enum Behavior {
        Ok,
        Fail,
        Hang,
    }

    impl Fake {
        fn new(id: SourceId, prefix: &'static str, total: u32) -> Self {
            Self {
                id,
                prefix,
                total,
                shared: Vec::new(),
                behavior: Behavior::Ok,
                calls: AtomicUsize::new(0),
                offsets: Mutex::new(Vec::new()),
            }
        }

        fn hit(&self, index: u32) -> SourceHit {
            let shared = self.shared.get(index as usize);
            let (slug, author) = shared.copied().unwrap_or(("", ""));
            let id = if shared.is_some() {
                format!("{}-{slug}", self.prefix)
            } else {
                format!("{}{index}", self.prefix)
            };
            SourceHit {
                reference: SourceRef {
                    source: self.id,
                    project_id: id.clone(),
                    slug: if shared.is_some() {
                        slug.into()
                    } else {
                        id.clone()
                    },
                    downloads: f64::from(1000 - index),
                },
                title: if shared.is_some() {
                    slug.into()
                } else {
                    id.clone()
                },
                author: if shared.is_some() {
                    author.into()
                } else {
                    format!("autor-{id}")
                },
                summary: String::new(),
                icon_url: None,
                updated: String::new(),
                created: String::new(),
                compatible: true,
                manual_download: false,
            }
        }
    }

    #[async_trait::async_trait]
    impl SearchSource for Fake {
        fn id(&self) -> SourceId {
            self.id
        }

        async fn preview(&self, _: &str, _: &CancellationToken) -> Result<ProjectPreview> {
            Err(Error::new(Code::Internal, "não usado"))
        }

        async fn search(
            &self,
            query: &SourceQuery,
            _: &CancellationToken,
        ) -> std::result::Result<SourcePage, SourceFailure> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            self.offsets.lock().unwrap().push(query.offset);
            match self.behavior {
                Behavior::Fail => return Err(SourceFailure::unavailable("HTTP 500")),
                Behavior::Hang => std::future::pending::<()>().await,
                Behavior::Ok => {}
            }
            let end = (query.offset + query.limit).min(self.total);
            Ok(SourcePage {
                hits: (query.offset..end).map(|index| self.hit(index)).collect(),
                total: u64::from(self.total),
            })
        }
    }

    fn target() -> PackTarget {
        PackTarget {
            minecraft: "1.20.1".into(),
            game_versions: vec!["1.20.1".into()],
            loaders: vec![Loader::Forge],
        }
    }

    fn request(cursor: Option<SearchCursor>) -> SearchRequest {
        SearchRequest {
            query: "x".into(),
            kind: ProjectKind::Mod,
            sort: SearchSort::Relevance,
            source: SourceFilter::All,
            environment: None,
            include_incompatible: false,
            category: None,
            cursor,
        }
    }

    fn ready(fake: &Arc<Fake>) -> ActiveSource {
        ActiveSource::Ready(Arc::clone(fake) as Arc<dyn SearchSource>)
    }

    fn ids(page: &SearchPage) -> Vec<String> {
        page.items
            .iter()
            .map(|item| {
                item.sources
                    .iter()
                    .map(|source| source.project_id.as_str())
                    .collect::<Vec<_>>()
                    .join("+")
            })
            .collect()
    }

    async fn run(sources: &[ActiveSource], request: &SearchRequest) -> Result<SearchPage> {
        search_with_timeout(
            sources,
            &target(),
            &InstalledKeys::from(["curseforge:c-JEI".to_owned()]),
            request,
            None,
            &CancellationToken::new(),
            Duration::from_millis(200),
        )
        .await
    }

    #[tokio::test]
    async fn intercala_deduplica_e_pagina_por_fonte() {
        let mut modrinth = Fake::new(SourceId::Modrinth, "m", 45);
        modrinth.shared = vec![("jei", "mezz")];
        let mut curseforge = Fake::new(SourceId::Curseforge, "c", 12);
        curseforge.shared = vec![("waystones", "blay"), ("JEI", "Mezz")];
        let (modrinth, curseforge) = (Arc::new(modrinth), Arc::new(curseforge));
        let sources = [ready(&modrinth), ready(&curseforge)];

        let first = run(&sources, &request(None)).await.unwrap();
        assert_eq!(first.items.len(), PAGE_SIZE);
        let page = ids(&first);
        // O JEI das duas fontes vira um item só, na posição do primeiro que apareceu.
        assert_eq!(page[0], "m-jei+c-JEI");
        assert_eq!(page[1], "c-waystones");
        // Na posição 1 da CurseForge estava o JEI, já juntado: o próximo dela é o c2.
        assert_eq!(page[2..5], ["m1", "m2", "c2"]);
        assert_eq!(page.iter().filter(|id| id.contains("JEI")).count(), 1);
        assert!(first.warnings.is_empty());
        assert!((first.total - 57.0).abs() < f64::EPSILON);
        // "Já no pack" pela chave de qualquer uma das fontes do item.
        assert!(first.items[0].in_pack);
        assert_eq!(first.items[0].key, "modrinth:m-jei");
        assert!(!first.items[1].in_pack);
        let next = first.next.clone().unwrap();
        let offsets: Vec<(SourceId, u32, bool)> = next
            .offsets
            .iter()
            .map(|o| (o.source, o.offset, o.done))
            .collect();
        assert_eq!(
            offsets,
            [
                (SourceId::Modrinth, 11, false),
                (SourceId::Curseforge, 10, false)
            ]
        );

        let second = run(&sources, &request(Some(next))).await.unwrap();
        assert_eq!(modrinth.offsets.lock().unwrap().as_slice(), [0, 11]);
        assert_eq!(curseforge.offsets.lock().unwrap().as_slice(), [0, 10]);
        let page = ids(&second);
        assert_eq!(page[..4], ["m11", "c10", "m12", "c11"]);
        assert_eq!(second.items.len(), PAGE_SIZE);
        // A CurseForge acabou (12 resultados) e não é mais consultada; o Modrinth continua.
        let next = second.next.clone().unwrap();
        assert!(
            next.offsets
                .iter()
                .any(|o| o.source == SourceId::Curseforge && o.done)
        );
        let third = run(&sources, &request(Some(next))).await.unwrap();
        assert_eq!(curseforge.calls.load(Ordering::SeqCst), 2);
        assert_eq!(ids(&third).first().map(String::as_str), Some("m29"));
        assert!(ids(&third).iter().all(|id| id.starts_with('m')));
        assert_eq!(third.items.len(), 16);
        assert!(third.next.is_none(), "as duas fontes acabaram");
    }

    #[tokio::test]
    async fn uma_fonte_com_erro_devolve_a_outra_com_aviso() {
        let modrinth = Arc::new(Fake::new(SourceId::Modrinth, "m", 3));
        let mut broken = Fake::new(SourceId::Curseforge, "c", 3);
        broken.behavior = Behavior::Fail;
        let broken = Arc::new(broken);
        let page = run(&[ready(&modrinth), ready(&broken)], &request(None))
            .await
            .unwrap();
        assert_eq!(ids(&page), ["m0", "m1", "m2"]);
        assert_eq!(page.warnings.len(), 1);
        assert_eq!(page.warnings[0].source, SourceId::Curseforge);
        assert_eq!(page.warnings[0].reason, SourceWarningReason::Unavailable);
        assert!(
            page.next.is_none(),
            "a fonte com erro fica de fora do resto da busca"
        );
    }

    #[tokio::test]
    async fn fonte_sem_resposta_nao_prende_a_busca() {
        let mut slow = Fake::new(SourceId::Modrinth, "m", 3);
        slow.behavior = Behavior::Hang;
        let slow = Arc::new(slow);
        let curseforge = Arc::new(Fake::new(SourceId::Curseforge, "c", 2));
        let started = std::time::Instant::now();
        let page = run(&[ready(&slow), ready(&curseforge)], &request(None))
            .await
            .unwrap();
        assert!(started.elapsed() < Duration::from_secs(5));
        assert_eq!(ids(&page), ["c0", "c1"]);
        assert_eq!(page.warnings[0].source, SourceId::Modrinth);
        assert!(
            page.warnings[0]
                .detail
                .as_deref()
                .unwrap()
                .contains("sem resposta")
        );
    }

    #[tokio::test]
    async fn todas_as_fontes_com_erro_e_erro_da_busca() {
        let mut broken = Fake::new(SourceId::Modrinth, "m", 3);
        broken.behavior = Behavior::Fail;
        let error = run(&[ready(&Arc::new(broken))], &request(None))
            .await
            .unwrap_err();
        assert_eq!(error.code, Code::SearchSourceUnavailable);
        assert_eq!(error.params["source"], "Modrinth");
        assert!(warden_core::DomainError::retryable(&error));
    }

    #[tokio::test]
    async fn fonte_desligada_vira_aviso_sem_requisicao_e_filtro_de_fonte() {
        let modrinth = Arc::new(Fake::new(SourceId::Modrinth, "m", 2));
        let sources = [
            ready(&modrinth),
            ActiveSource::Off(SourceId::Curseforge, SourceWarningReason::KeyMissing),
        ];
        let page = run(&sources, &request(None)).await.unwrap();
        assert_eq!(ids(&page), ["m0", "m1"]);
        assert_eq!(page.warnings[0].reason, SourceWarningReason::KeyMissing);
        assert_eq!(page.sources, [SourceId::Modrinth]);
        // "Só Modrinth": o aviso da CurseForge some.
        let mut only = request(None);
        only.source = SourceFilter::Modrinth;
        assert!(run(&sources, &only).await.unwrap().warnings.is_empty());
        // "Só CurseForge" com ela desligada: nada a buscar, só o aviso.
        only.source = SourceFilter::Curseforge;
        let page = run(&sources, &only).await.unwrap();
        assert!(page.items.is_empty());
        assert_eq!(page.warnings.len(), 1);
        assert_eq!(modrinth.calls.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn alvo_do_pack_com_versoes_aceitas() {
        let mut manifest = PackManifest::new("T", "1.20.1");
        manifest.set_loader_version(Loader::NeoForge, "47.1.106");
        manifest.set_option(
            "acceptable-game-versions",
            warden_packwiz::Value::Array(vec!["1.20".into(), "1.20.1".into()]),
        );
        let target = PackTarget::from_manifest(&manifest).unwrap();
        assert_eq!(target.game_versions, ["1.20.1", "1.20"]);
        assert_eq!(target.loader_names(ProjectKind::Mod), ["neoforge", "forge"]);
        assert!(target.loader_names(ProjectKind::Shader).is_empty());
        assert_eq!(SourceId::Curseforge.key("238222"), "curseforge:238222");
        assert_eq!(ProjectKind::ResourcePack.folder(), "resourcepacks");
    }
}
