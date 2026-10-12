//! Atualizações dos mods (SPEC T10; CA-T10-01 a CA-T10-04).
//!
//! Um único produtor ([`check`]) calcula o estado de cada item; a lista, os contadores e a
//! revisão leem desse mesmo [`ModUpdateReport`] (R4 §2.4). O Modrinth é consultado em lote para o
//! pack inteiro (`POST /version_files/update` e `POST /version_files`: duas requisições para
//! até mil arquivos); a CurseForge, em dois lotes (projetos e arquivos novos).
//!
//! Regras que valem para todas as fontes:
//! - falha de rede, chave ausente ou recusada nunca vira "Em dia" (CA-T10-04): o item fica em
//!   "Não foi possível verificar", com o motivo, ou em "Não verificado" quando nem foi tentado;
//! - item fixado (`pin = true`), arquivo local e link direto não são consultados;
//! - só estáveis: beta e alfa mais novos não são oferecidos (CA-T10-03). Uma versão mais antiga
//!   que a instalada também nunca é oferecida;
//! - nada é gravado por [`check`] nem por [`plan`]; [`apply`] grava tudo numa transação só
//!   (um `packwiz refresh`) e, quando pedido, cria antes um ponto de segurança.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::Path;

use serde::{Deserialize, Serialize};
use warden_core::CancellationToken;
use warden_curseforge::{
    CurseforgeClient, File as CfFile, HashAlgo, ModLoaderType, RelationType, ReleaseType,
};
use warden_modrinth::{
    DependencyType, HashAlgorithm, ModrinthClient, UpdateFilter, Version, VersionFilter,
    VersionType,
};
use warden_packwiz::{CurseForgeFile, HashFormat, Loader, Metafile, ModrinthFile, Side, read_pack};
use warden_packwiz_cli::Packwiz;
use warden_versioning::{Identity, Moment, PackRepo, SafetyReason};

use crate::inventory::{
    InventoryItem, ItemSource, ItemState, modrinth_projects, readable_file_version, scan,
};
use crate::transaction::PackTransaction;
use crate::{Error, ProjectErrorCode as Code, Result};

/// Quantas versões "do meio" entram nas novidades de um item.
const CHANGELOG_LIMIT: usize = 10;

/// Estado de um item na verificação.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum UpdateStatus {
    /// A versão instalada é a mais nova compatível com o pack.
    UpToDate,
    /// Há uma versão mais nova (em [`UpdateItem::new_version`]).
    Available,
    /// Nunca foi consultado (por exemplo, a CurseForge sem chave).
    NotChecked,
    /// A consulta falhou; o motivo está em [`UpdateItem::reason`].
    Failed,
    /// Arquivo local ou link direto: não há onde consultar.
    NotApplicable,
    /// Versão fixada: não é consultada nem entra em "Atualizar todos".
    Pinned,
}

/// Por que um item ficou sem resultado.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum UpdateReason {
    /// Sem conexão, tempo esgotado, limite de requisições ou servidor fora do ar.
    Offline,
    /// Falta a chave da CurseForge (Configurações → Chaves e contas).
    KeyMissing,
    /// A CurseForge recusou a chave.
    KeyInvalid,
    /// A fonte respondeu algo que o Warden não entendeu ou recusou o pedido.
    Service,
    /// O Modrinth não conhece o arquivo exato do pack (hash desconhecido).
    FileUnknown,
    /// O projeto ou o arquivo não está mais na plataforma.
    Removed,
    /// O `.pw.toml` usa um hash que a consulta em lote não aceita (só `sha1` e `sha512`).
    HashUnsupported,
}

/// Canal de uma versão.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum UpdateChannel {
    /// Estável.
    Release,
    /// Beta.
    Beta,
    /// Alfa.
    Alpha,
}

/// A versão que o item pode receber.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct NewVersion {
    /// ID da versão (Modrinth) ou do arquivo (CurseForge). Volta em [`UpdateSelection`].
    pub id: String,
    /// Número legível.
    pub number: String,
    /// Canal.
    pub channel: UpdateChannel,
    /// Data de publicação (RFC 3339), quando a fonte informa.
    pub published: Option<String>,
}

/// Uma linha do relatório.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct UpdateItem {
    /// Chave estável do item (`modrinth:<projeto>`, `curseforge:<projeto>` ou `path:<caminho>`).
    pub key: String,
    /// Caminho do `.pw.toml` (ou do arquivo) relativo à pasta do pack.
    pub path: String,
    /// Nome legível.
    pub name: String,
    /// Fonte.
    pub source: ItemSource,
    /// Estado.
    pub status: UpdateStatus,
    /// ID da versão instalada quando foi consultado; a interface só confia no resultado se ele
    /// ainda for o do item (o pack pode ter mudado depois).
    pub current_id: Option<String>,
    /// Versão instalada, legível.
    pub current: Option<String>,
    /// A versão nova, quando `status` é `available`.
    pub new_version: Option<NewVersion>,
    /// Motivo, quando `status` é `failed` ou `notChecked`.
    pub reason: Option<UpdateReason>,
    /// Detalhe técnico do erro, sem segredos.
    pub detail: Option<String>,
}

/// O relatório do pack inteiro.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ModUpdateReport {
    /// Quando a verificação terminou (RFC 3339).
    pub checked_at: String,
    /// Um item por linha da lista de Mods que pode ser consultada, na ordem do inventário.
    pub items: Vec<UpdateItem>,
    /// Quantos itens têm atualização disponível.
    pub available: u32,
}

impl ModUpdateReport {
    fn new(items: Vec<UpdateItem>) -> Self {
        let available = items
            .iter()
            .filter(|item| item.status == UpdateStatus::Available)
            .count();
        Self {
            checked_at: Moment::now_utc().rfc3339(),
            items,
            available: u32::try_from(available).unwrap_or(u32::MAX),
        }
    }

    /// O item de um caminho.
    #[must_use]
    pub fn item(&self, path: &str) -> Option<&UpdateItem> {
        self.items.iter().find(|item| item.path == path)
    }

    /// Marca itens como atualizados, sem consultar de novo (depois de [`apply`]).
    pub fn mark_applied(&mut self, applied: &[AppliedUpdate]) {
        for done in applied {
            if let Some(item) = self.items.iter_mut().find(|item| item.path == done.path) {
                item.status = UpdateStatus::UpToDate;
                item.current_id = Some(done.to_id.clone());
                item.current = Some(done.to.clone());
                item.new_version = None;
                item.reason = None;
                item.detail = None;
            }
        }
        self.available = u32::try_from(
            self.items
                .iter()
                .filter(|item| item.status == UpdateStatus::Available)
                .count(),
        )
        .unwrap_or(u32::MAX);
    }
}

/// Os clientes e as escolhas da verificação.
#[derive(Debug)]
pub struct UpdateContext<'a> {
    /// Cliente do Modrinth.
    pub modrinth: &'a ModrinthClient,
    /// Cliente da CurseForge; `None` ou sem chave deixa os itens da CurseForge "Não verificado".
    pub curseforge: Option<&'a CurseforgeClient>,
    /// Oferece beta e alfa (Configurações → "Mostrar versões beta e alfa"). Falso: só estáveis.
    pub prereleases: bool,
}

impl UpdateContext<'_> {
    fn allows(&self, channel: UpdateChannel) -> bool {
        channel == UpdateChannel::Release || self.prereleases
    }

    fn modrinth_types(&self) -> Vec<VersionType> {
        if self.prereleases {
            Vec::new()
        } else {
            vec![VersionType::Release]
        }
    }
}

/// Minecraft e loaders do pack, para filtrar as versões.
#[derive(Debug, Clone)]
struct Target {
    minecraft: String,
    loaders: Vec<Loader>,
}

impl Target {
    fn read(root: &Path) -> Result<Self> {
        let pack = read_pack(root).map_err(|e| Error::new(Code::InvalidPack, e.to_string()))?;
        let manifest = &pack.pack.value;
        let minecraft = manifest
            .minecraft_version()
            .ok_or_else(|| {
                Error::new(
                    Code::InvalidPack,
                    "o pack.toml não tem [versions] minecraft",
                )
            })?
            .to_owned();
        Ok(Self {
            minecraft,
            loaders: manifest.compatible_loaders(),
        })
    }

    fn modrinth_loaders(&self) -> Vec<String> {
        self.loaders
            .iter()
            .map(|loader| loader.key().to_owned())
            .collect()
    }

    fn curseforge_loaders(&self) -> Vec<ModLoaderType> {
        self.loaders
            .iter()
            .filter_map(|loader| ModLoaderType::from_loader_name(loader.key()))
            .collect()
    }
}

fn channel_of_modrinth(kind: VersionType) -> UpdateChannel {
    match kind {
        VersionType::Release => UpdateChannel::Release,
        VersionType::Beta => UpdateChannel::Beta,
        VersionType::Alpha | VersionType::Unknown => UpdateChannel::Alpha,
    }
}

fn channel_of_curseforge(kind: Option<ReleaseType>) -> UpdateChannel {
    match kind {
        Some(ReleaseType::Release) => UpdateChannel::Release,
        Some(ReleaseType::Beta) => UpdateChannel::Beta,
        _ => UpdateChannel::Alpha,
    }
}

fn base_item(item: &InventoryItem) -> UpdateItem {
    UpdateItem {
        key: item.key.clone(),
        path: item.path.clone(),
        name: item.name.clone(),
        source: item.source,
        status: UpdateStatus::NotChecked,
        current_id: item.source_version_id.clone(),
        current: item.file_name.as_deref().map(readable_file_version),
        new_version: None,
        reason: None,
        detail: None,
    }
}

fn set_status(item: &mut UpdateItem, status: UpdateStatus) {
    item.status = status;
    item.reason = None;
    item.detail = None;
    item.new_version = None;
}

fn fail(item: &mut UpdateItem, reason: UpdateReason, detail: impl Into<String>) {
    item.status = UpdateStatus::Failed;
    item.reason = Some(reason);
    item.detail = Some(detail.into());
    item.new_version = None;
}

fn modrinth_reason(error: &warden_modrinth::Error) -> UpdateReason {
    if error.is_offline() {
        UpdateReason::Offline
    } else {
        UpdateReason::Service
    }
}

fn curseforge_reason(error: &warden_curseforge::Error) -> UpdateReason {
    match error {
        warden_curseforge::Error::KeyMissing => UpdateReason::KeyMissing,
        warden_curseforge::Error::KeyInvalid { .. } => UpdateReason::KeyInvalid,
        other if other.is_offline() => UpdateReason::Offline,
        _ => UpdateReason::Service,
    }
}

/// A verificação: um relatório com o estado de cada item consultável do pack. Só falha se o
/// pack não puder ser lido; qualquer problema de rede fica nos itens.
pub async fn check(
    root: &Path,
    ctx: &UpdateContext<'_>,
    cancel: Option<&CancellationToken>,
) -> Result<ModUpdateReport> {
    let scan = scan(root)?;
    if let Some(error) = &scan.inventory.index_error {
        return Err(Error::new(Code::InvalidPack, error.clone()));
    }
    let target = Target::read(root)?;
    let mut items: Vec<UpdateItem> = Vec::new();
    let mut modrinth_jobs: Vec<ModrinthJob> = Vec::new();
    let mut curseforge_jobs: Vec<CurseforgeJob> = Vec::new();
    for entry in &scan.inventory.items {
        if entry.state != ItemState::Ok {
            continue;
        }
        let index = items.len();
        let mut item = base_item(entry);
        if entry.pinned {
            set_status(&mut item, UpdateStatus::Pinned);
        } else {
            match entry.source {
                ItemSource::Local | ItemSource::Url => {
                    set_status(&mut item, UpdateStatus::NotApplicable);
                }
                ItemSource::Modrinth => match scan.metafiles.get(&entry.path) {
                    Some(metafile) => match modrinth_job(index, metafile) {
                        Some(job) => modrinth_jobs.push(job),
                        None => fail(
                            &mut item,
                            UpdateReason::HashUnsupported,
                            format!("hash {}", metafile.download.hash_format),
                        ),
                    },
                    None => set_status(&mut item, UpdateStatus::NotApplicable),
                },
                ItemSource::Curseforge => {
                    match scan
                        .metafiles
                        .get(&entry.path)
                        .and_then(Metafile::curseforge_ids)
                    {
                        Some((project, file)) => curseforge_jobs.push(CurseforgeJob {
                            index,
                            project,
                            file,
                        }),
                        None => set_status(&mut item, UpdateStatus::NotApplicable),
                    }
                }
            }
        }
        items.push(item);
    }
    check_modrinth(ctx, &target, &modrinth_jobs, &mut items, cancel).await;
    check_curseforge(ctx, &target, &curseforge_jobs, &mut items, cancel).await;
    Ok(ModUpdateReport::new(items))
}

struct ModrinthJob {
    index: usize,
    hash: String,
    algorithm: HashAlgorithm,
    installed_id: String,
}

fn modrinth_job(index: usize, metafile: &Metafile) -> Option<ModrinthJob> {
    let algorithm = match metafile.download.hash_format.to_ascii_lowercase().as_str() {
        "sha512" => HashAlgorithm::Sha512,
        "sha1" => HashAlgorithm::Sha1,
        _ => return None,
    };
    let hash = metafile.download.hash.trim().to_ascii_lowercase();
    if hash.is_empty() {
        return None;
    }
    let (_, installed_id) = metafile.modrinth_ids()?;
    Some(ModrinthJob {
        index,
        hash,
        algorithm,
        installed_id: installed_id.to_owned(),
    })
}

/// `true` se a data `candidate` é depois da `installed` (RFC 3339 em UTC; compara até o
/// segundo). Sem uma das datas, vale que a versão é outra.
fn is_newer(candidate: &str, installed: &str) -> bool {
    if candidate.len() < 19 || installed.len() < 19 {
        return true;
    }
    candidate[..19] > installed[..19]
}

async fn check_modrinth(
    ctx: &UpdateContext<'_>,
    target: &Target,
    jobs: &[ModrinthJob],
    items: &mut [UpdateItem],
    cancel: Option<&CancellationToken>,
) {
    let filter = UpdateFilter {
        loaders: target.modrinth_loaders(),
        game_versions: vec![target.minecraft.clone()],
        version_types: ctx.modrinth_types(),
    };
    for algorithm in [HashAlgorithm::Sha512, HashAlgorithm::Sha1] {
        let group: Vec<&ModrinthJob> = jobs
            .iter()
            .filter(|job| job.algorithm == algorithm)
            .collect();
        if group.is_empty() {
            continue;
        }
        let hashes: Vec<String> = group.iter().map(|job| job.hash.clone()).collect();
        let result = async {
            let newest = ctx
                .modrinth
                .version_files_update(&hashes, algorithm, &filter, cancel)
                .await?;
            let installed = ctx
                .modrinth
                .version_files(&hashes, algorithm, cancel)
                .await?;
            Ok::<_, warden_modrinth::Error>((newest, installed))
        }
        .await;
        let (newest, installed) = match result {
            Ok(pair) => pair,
            Err(error) => {
                for job in &group {
                    fail(
                        &mut items[job.index],
                        modrinth_reason(&error),
                        error.to_string(),
                    );
                }
                continue;
            }
        };
        for job in group {
            let item = &mut items[job.index];
            let Some(current) = installed.get(&job.hash) else {
                fail(
                    item,
                    UpdateReason::FileUnknown,
                    "o Modrinth não conhece o arquivo deste item",
                );
                continue;
            };
            if !current.version_number.trim().is_empty() {
                item.current = Some(current.version_number.clone());
            }
            let offered = newest.get(&job.hash).filter(|candidate| {
                candidate.id != job.installed_id
                    && candidate.id != current.id
                    && ctx.allows(channel_of_modrinth(candidate.version_type))
                    && is_newer(&candidate.date_published, &current.date_published)
            });
            match offered {
                Some(candidate) => {
                    item.status = UpdateStatus::Available;
                    item.reason = None;
                    item.detail = None;
                    item.new_version = Some(NewVersion {
                        id: candidate.id.clone(),
                        number: candidate.version_number.clone(),
                        channel: channel_of_modrinth(candidate.version_type),
                        published: (!candidate.date_published.is_empty())
                            .then(|| candidate.date_published.clone()),
                    });
                }
                None => set_status(item, UpdateStatus::UpToDate),
            }
        }
    }
}

struct CurseforgeJob {
    index: usize,
    project: u32,
    file: u32,
}

async fn check_curseforge(
    ctx: &UpdateContext<'_>,
    target: &Target,
    jobs: &[CurseforgeJob],
    items: &mut [UpdateItem],
    cancel: Option<&CancellationToken>,
) {
    if jobs.is_empty() {
        return;
    }
    let Some(client) = ctx.curseforge.filter(|client| client.has_key()) else {
        for job in jobs {
            let item = &mut items[job.index];
            item.status = UpdateStatus::NotChecked;
            item.reason = Some(UpdateReason::KeyMissing);
        }
        return;
    };
    let ids: Vec<u64> = jobs.iter().map(|job| u64::from(job.project)).collect();
    let projects = match client.projects(&ids, cancel).await {
        Ok(projects) => projects,
        Err(error) => {
            for job in jobs {
                fail(
                    &mut items[job.index],
                    curseforge_reason(&error),
                    error.to_string(),
                );
            }
            return;
        }
    };
    let by_id: HashMap<u64, _> = projects
        .into_iter()
        .map(|project| (project.id, project))
        .collect();
    let loaders = target.curseforge_loaders();
    // Candidato de cada item: o arquivo mais novo para o Minecraft e o loader do pack.
    let mut wanted: Vec<(usize, u64)> = Vec::new();
    for job in jobs {
        let item = &mut items[job.index];
        let Some(project) = by_id.get(&u64::from(job.project)) else {
            fail(
                item,
                UpdateReason::Removed,
                "a CurseForge não devolveu o projeto",
            );
            continue;
        };
        let best = project
            .latest_files_indexes
            .iter()
            .filter(|index| index.game_version == target.minecraft)
            .filter(|index| {
                index
                    .mod_loader
                    .is_none_or(|loader| loader == ModLoaderType::Any || loaders.contains(&loader))
            })
            .filter(|index| ctx.allows(channel_of_curseforge(index.release_type)))
            .map(|index| index.file_id)
            .max();
        match best {
            Some(file_id) if file_id > u64::from(job.file) => wanted.push((job.index, file_id)),
            _ => set_status(item, UpdateStatus::UpToDate),
        }
    }
    if wanted.is_empty() {
        return;
    }
    let file_ids: Vec<u64> = wanted.iter().map(|(_, id)| *id).collect();
    let files = match client.files_by_id(&file_ids, cancel).await {
        Ok(files) => files,
        Err(error) => {
            for (index, _) in &wanted {
                fail(
                    &mut items[*index],
                    curseforge_reason(&error),
                    error.to_string(),
                );
            }
            return;
        }
    };
    let by_file: HashMap<u64, CfFile> = files.into_iter().map(|file| (file.id, file)).collect();
    for (index, file_id) in wanted {
        let item = &mut items[index];
        match by_file.get(&file_id) {
            Some(file) => {
                item.status = UpdateStatus::Available;
                item.reason = None;
                item.detail = None;
                item.new_version = Some(NewVersion {
                    id: file.id.to_string(),
                    number: readable_file_version(&file.file_name),
                    channel: channel_of_curseforge(file.release_type),
                    published: file.file_date.clone(),
                });
            }
            None => fail(
                item,
                UpdateReason::Removed,
                "a CurseForge não devolveu o arquivo novo",
            ),
        }
    }
}

/// Uma versão nas novidades de um item.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ChangelogEntry {
    /// Número da versão.
    pub version: String,
    /// Canal.
    pub channel: UpdateChannel,
    /// Data de publicação (RFC 3339).
    pub published: Option<String>,
    /// Notas (Markdown ou HTML, conforme `format`); a interface higieniza antes de mostrar.
    pub text: Option<String>,
    /// Formato das notas.
    pub format: crate::details::DescriptionFormat,
}

/// Uma dependência obrigatória que a versão nova pede e o pack ainda não tem.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct NewDependency {
    /// ID do projeto na fonte do item.
    pub project_id: String,
    /// Nome do projeto (o ID, se a fonte não deu).
    pub name: String,
    /// Fonte do projeto.
    pub source: ItemSource,
    /// Nomes dos itens da revisão que a exigem (uma dependência comum aparece uma vez).
    pub needed_by: Vec<String>,
}

/// Uma incompatibilidade nova: a versão nova declara que não funciona com um item do pack.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct NewConflict {
    /// Item que seria atualizado.
    pub item: String,
    /// Item do pack com o qual a versão nova se declara incompatível.
    pub with: String,
}

/// Um item da revisão.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct PlanItem {
    /// Chave estável.
    pub key: String,
    /// Caminho do metafile.
    pub path: String,
    /// Nome.
    pub name: String,
    /// Fonte.
    pub source: ItemSource,
    /// Versão instalada, legível.
    pub current: Option<String>,
    /// A versão nova.
    pub new_version: NewVersion,
    /// Novidades das versões do meio e da nova (Modrinth) ou da nova (CurseForge), da mais
    /// nova para a mais antiga.
    pub changelog: Vec<ChangelogEntry>,
    /// O arquivo novo não pode ser baixado pelo Warden (a CurseForge bloqueia terceiros): o
    /// jogo vai pedir o download manual.
    pub manual_download: bool,
}

/// O que a revisão mostra antes de aplicar.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct UpdatePlan {
    /// Itens, na ordem pedida.
    pub items: Vec<PlanItem>,
    /// Dependências obrigatórias novas que não estão no pack.
    pub new_dependencies: Vec<NewDependency>,
    /// Incompatibilidades novas com itens do pack.
    pub conflicts: Vec<NewConflict>,
}

/// Item e versão escolhidos para aplicar.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct UpdateSelection {
    /// Caminho do metafile.
    pub path: String,
    /// ID da versão nova que a revisão mostrou (`NewVersion::id`).
    pub to_version_id: String,
}

fn stale(path: &str) -> Error {
    Error::new(
        Code::PackChangedExternally,
        format!("{path} mudou depois da verificação"),
    )
    .param("path", path)
}

fn not_updatable(path: &str) -> Error {
    Error::new(
        Code::InvalidInput,
        format!("{path} não tem atualização disponível"),
    )
    .param("path", path)
}

/// Itens do relatório que podem ser atualizados agora: com atualização disponível e ainda na
/// versão em que foram consultados. Devolve (item do relatório, item do inventário).
fn selected<'a>(
    report: &'a ModUpdateReport,
    inventory: &'a [InventoryItem],
    paths: &[String],
) -> Result<Vec<(&'a UpdateItem, &'a InventoryItem)>> {
    let mut seen = BTreeSet::new();
    let mut result = Vec::new();
    for path in paths {
        let cleaned = warden_packwiz::clean_path(&path.replace('\\', "/"));
        if !seen.insert(cleaned.clone()) {
            continue;
        }
        let entry = inventory
            .iter()
            .find(|item| item.path == cleaned)
            .ok_or_else(|| crate::inventory::item_not_found(path))?;
        let reported = report
            .item(&cleaned)
            .filter(|item| item.status == UpdateStatus::Available && item.new_version.is_some())
            .ok_or_else(|| not_updatable(path))?;
        if reported.current_id != entry.source_version_id || entry.pinned {
            return Err(stale(path));
        }
        result.push((reported, entry));
    }
    Ok(result)
}

/// Monta a revisão dos itens (os caminhos precisam estar "Atualização disponível" no
/// relatório): versões, novidades, dependências novas e incompatibilidades. Só lê; falha de
/// rede ao buscar novidades ou nomes nunca impede a revisão (o item aparece sem novidades).
///
/// Erros: [`Code::ItemNotFound`], [`Code::InvalidInput`] (item sem atualização) e
/// [`Code::PackChangedExternally`] (o item mudou depois da verificação).
pub async fn plan(
    root: &Path,
    report: &ModUpdateReport,
    paths: &[String],
    ctx: &UpdateContext<'_>,
    cancel: Option<&CancellationToken>,
) -> Result<UpdatePlan> {
    let scan = scan(root)?;
    let target = Target::read(root)?;
    let chosen = selected(report, &scan.inventory.items, paths)?;
    let in_pack = |source: ItemSource| -> HashMap<&str, &InventoryItem> {
        scan.inventory
            .items
            .iter()
            .filter(|item| item.source == source)
            .filter_map(|item| Some((item.project_id.as_deref()?, item)))
            .collect()
    };
    let in_pack_modrinth = in_pack(ItemSource::Modrinth);
    let in_pack_curseforge = in_pack(ItemSource::Curseforge);
    let new_versions = fetch_targets(&chosen, ctx, cancel).await;

    let mut findings = Findings::default();
    let mut items = Vec::new();
    for (reported, entry) in &chosen {
        let Some(new_version) = reported.new_version.clone() else {
            continue;
        };
        let mut changelog = Vec::new();
        let mut manual_download = false;
        match entry.source {
            ItemSource::Modrinth => {
                let version = new_versions.modrinth.get(&new_version.id);
                if let Some(version) = version {
                    findings.modrinth(&entry.name, version, &in_pack_modrinth);
                }
                changelog =
                    modrinth_changelog(entry, &new_version, version, &target, ctx, cancel).await;
            }
            ItemSource::Curseforge => {
                let file = new_version
                    .id
                    .parse::<u64>()
                    .ok()
                    .and_then(|id| new_versions.curseforge.get(&id));
                if let Some(file) = file {
                    manual_download = file.is_distribution_blocked();
                    findings.curseforge(&entry.name, file, &in_pack_curseforge);
                    changelog = curseforge_changelog(file, &new_version, ctx, cancel).await;
                }
            }
            ItemSource::Url | ItemSource::Local => {}
        }
        items.push(PlanItem {
            key: entry.key.clone(),
            path: entry.path.clone(),
            name: entry.name.clone(),
            source: entry.source,
            current: reported.current.clone(),
            new_version,
            changelog,
            manual_download,
        });
    }
    let new_dependencies = resolve_names(ctx, findings.needed, cancel).await;
    Ok(UpdatePlan {
        items,
        new_dependencies,
        conflicts: findings.conflicts,
    })
}

/// As versões novas dos itens escolhidos, buscadas em lote (Modrinth: cache primeiro).
#[derive(Default)]
struct NewVersions {
    modrinth: HashMap<String, Version>,
    curseforge: HashMap<u64, CfFile>,
    /// A consulta que falhou, para [`apply`] dizer o motivo.
    error: Option<String>,
}

/// Busca as versões novas dos itens escolhidos. Falha de rede deixa o mapa vazio e o motivo em
/// `error`: a revisão continua (sem novidades nem dependências) e [`apply`] recusa de verdade.
async fn fetch_targets(
    chosen: &[(&UpdateItem, &InventoryItem)],
    ctx: &UpdateContext<'_>,
    cancel: Option<&CancellationToken>,
) -> NewVersions {
    let ids_of = |source: ItemSource| -> Vec<&str> {
        chosen
            .iter()
            .filter(|(_, entry)| entry.source == source)
            .filter_map(|(reported, _)| Some(reported.new_version.as_ref()?.id.as_str()))
            .collect()
    };
    let modrinth_ids: Vec<String> = ids_of(ItemSource::Modrinth)
        .into_iter()
        .map(str::to_owned)
        .collect();
    let curseforge_ids: Vec<u64> = ids_of(ItemSource::Curseforge)
        .into_iter()
        .filter_map(|id| id.parse().ok())
        .collect();
    let mut found = NewVersions::default();
    if !modrinth_ids.is_empty() {
        match ctx.modrinth.versions(&modrinth_ids, cancel).await {
            Ok(versions) => {
                found.modrinth = versions.into_iter().map(|v| (v.id.clone(), v)).collect();
            }
            Err(error) => found.error = Some(error.to_string()),
        }
    }
    if let Some(client) = ctx.curseforge
        && !curseforge_ids.is_empty()
    {
        match client.files_by_id(&curseforge_ids, cancel).await {
            Ok(files) => found.curseforge = files.into_iter().map(|f| (f.id, f)).collect(),
            Err(error) => found.error = Some(error.to_string()),
        }
    }
    found
}

/// Dependências novas e incompatibilidades que as versões novas declaram.
#[derive(Default)]
struct Findings {
    /// (é da CurseForge, projeto) → nomes dos itens que exigem.
    needed: BTreeMap<(bool, String), Vec<String>>,
    conflicts: Vec<NewConflict>,
}

impl Findings {
    fn modrinth(&mut self, name: &str, version: &Version, in_pack: &HashMap<&str, &InventoryItem>) {
        for dependency in &version.dependencies {
            let Some(project) = dependency.project_id.as_deref() else {
                continue;
            };
            match dependency.dependency_type {
                DependencyType::Required if !in_pack.contains_key(project) => {
                    self.needed
                        .entry((false, project.to_owned()))
                        .or_default()
                        .push(name.to_owned());
                }
                DependencyType::Incompatible => {
                    self.conflict(name, in_pack.get(project));
                }
                _ => {}
            }
        }
    }

    fn curseforge(&mut self, name: &str, file: &CfFile, in_pack: &HashMap<&str, &InventoryItem>) {
        for dependency in &file.dependencies {
            let id = dependency.mod_id.to_string();
            match dependency.relation_type {
                RelationType::RequiredDependency if !in_pack.contains_key(id.as_str()) => {
                    self.needed
                        .entry((true, id))
                        .or_default()
                        .push(name.to_owned());
                }
                RelationType::Incompatible => {
                    self.conflict(name, in_pack.get(id.as_str()));
                }
                _ => {}
            }
        }
    }

    fn conflict(&mut self, name: &str, other: Option<&&InventoryItem>) {
        if let Some(other) = other {
            let conflict = NewConflict {
                item: name.to_owned(),
                with: other.name.clone(),
            };
            if !self.conflicts.contains(&conflict) {
                self.conflicts.push(conflict);
            }
        }
    }
}

/// As notas do arquivo novo da CurseForge (uma entrada só, em HTML).
async fn curseforge_changelog(
    file: &CfFile,
    new_version: &NewVersion,
    ctx: &UpdateContext<'_>,
    cancel: Option<&CancellationToken>,
) -> Vec<ChangelogEntry> {
    let Some(client) = ctx.curseforge else {
        return Vec::new();
    };
    let text = client
        .file_changelog(file.mod_id, file.id, cancel)
        .await
        .ok()
        .filter(|text| !text.trim().is_empty());
    vec![ChangelogEntry {
        version: new_version.number.clone(),
        channel: new_version.channel,
        published: new_version.published.clone(),
        text,
        format: crate::details::DescriptionFormat::Html,
    }]
}

async fn resolve_names(
    ctx: &UpdateContext<'_>,
    needed: BTreeMap<(bool, String), Vec<String>>,
    cancel: Option<&CancellationToken>,
) -> Vec<NewDependency> {
    let modrinth_ids: Vec<String> = needed
        .keys()
        .filter(|(curseforge, _)| !*curseforge)
        .map(|(_, id)| id.clone())
        .collect();
    let modrinth_names: HashMap<String, String> = modrinth_projects(ctx.modrinth, &modrinth_ids)
        .await
        .into_iter()
        .map(|(id, project)| (id, project.title))
        .collect();
    let curseforge_ids: Vec<u64> = needed
        .keys()
        .filter(|(curseforge, _)| *curseforge)
        .filter_map(|(_, id)| id.parse().ok())
        .collect();
    let curseforge_names: HashMap<u64, String> = match ctx.curseforge {
        Some(client) if !curseforge_ids.is_empty() => client
            .projects(&curseforge_ids, cancel)
            .await
            .map(|projects| projects.into_iter().map(|p| (p.id, p.name)).collect())
            .unwrap_or_default(),
        _ => HashMap::new(),
    };
    needed
        .into_iter()
        .map(|((curseforge, id), mut needed_by)| {
            needed_by.dedup();
            let (source, name) = if curseforge {
                (
                    ItemSource::Curseforge,
                    id.parse::<u64>()
                        .ok()
                        .and_then(|key| curseforge_names.get(&key).cloned()),
                )
            } else {
                (ItemSource::Modrinth, modrinth_names.get(&id).cloned())
            };
            let name = name
                .filter(|name| !name.trim().is_empty())
                .unwrap_or_else(|| id.clone());
            NewDependency {
                project_id: id,
                name,
                source,
                needed_by,
            }
        })
        .collect()
}

/// Novidades das versões entre a instalada (exclusive) e a nova (inclusive). Se a lista do
/// projeto não vier, fica só a nova, com as notas que já temos.
async fn modrinth_changelog(
    entry: &InventoryItem,
    new_version: &NewVersion,
    version: Option<&Version>,
    target: &Target,
    ctx: &UpdateContext<'_>,
    cancel: Option<&CancellationToken>,
) -> Vec<ChangelogEntry> {
    let entry_of = |version: &Version| ChangelogEntry {
        version: version.version_number.clone(),
        channel: channel_of_modrinth(version.version_type),
        published: (!version.date_published.is_empty()).then(|| version.date_published.clone()),
        text: version
            .changelog
            .clone()
            .filter(|text| !text.trim().is_empty()),
        format: crate::details::DescriptionFormat::Markdown,
    };
    let fallback = || {
        version
            .map(|version| vec![entry_of(version)])
            .unwrap_or_default()
    };
    let (Some(project), Some(installed_id)) = (
        entry.project_id.as_deref(),
        entry.source_version_id.as_deref(),
    ) else {
        return fallback();
    };
    let filter = VersionFilter {
        loaders: target.modrinth_loaders(),
        game_versions: vec![target.minecraft.clone()],
        include_changelog: true,
        ..VersionFilter::default()
    };
    let Ok(versions) = ctx
        .modrinth
        .project_versions(project, &filter, cancel)
        .await
    else {
        return fallback();
    };
    let installed_date = versions
        .iter()
        .find(|version| version.id == installed_id)
        .map(|version| version.date_published.clone());
    // A versão instalada pode ser de outro Minecraft e não vir na lista: sem data, só a nova.
    let Some(installed_date) = installed_date else {
        return fallback();
    };
    let mut between: Vec<&Version> = versions
        .iter()
        .filter(|version| {
            version.id == new_version.id
                || (is_newer(&version.date_published, &installed_date)
                    && version.id != installed_id
                    && ctx.allows(channel_of_modrinth(version.version_type))
                    && new_version
                        .published
                        .as_deref()
                        .is_none_or(|new| !is_newer(&version.date_published, new)))
        })
        .collect();
    between.sort_by(|a, b| b.date_published.cmp(&a.date_published));
    between.truncate(CHANGELOG_LIMIT);
    if between.is_empty() {
        return fallback();
    }
    between.into_iter().map(entry_of).collect()
}

/// Um item que [`apply`] atualizou.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct AppliedUpdate {
    /// Caminho do metafile.
    pub path: String,
    /// Nome do item.
    pub name: String,
    /// Versão de antes, legível.
    pub from: Option<String>,
    /// Versão de agora, legível.
    pub to: String,
    /// ID da versão (ou arquivo) de agora.
    pub to_id: String,
}

/// O resultado de [`apply`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct AppliedUpdates {
    /// Itens atualizados.
    pub updated: Vec<AppliedUpdate>,
    /// Nome do ponto de segurança criado antes (quando pedido).
    pub safety_point: Option<String>,
}

/// Grava as novas referências: o `.pw.toml` de cada item passa a apontar para a versão nova
/// (lado, opcional e nome ficam como estavam) e o índice é atualizado, tudo numa transação
/// (um só `packwiz refresh`). Com `safety_author`, cria antes um ponto de segurança "Atualizar
/// todos" (ARCHITECTURE §11).
///
/// Erros: os de [`plan`] para um caminho; [`Code::InvalidInput`] se a versão pedida não é a
/// que a revisão mostrou ou a fonte não entrega o arquivo.
pub async fn apply(
    root: &Path,
    report: &ModUpdateReport,
    selections: &[UpdateSelection],
    ctx: &UpdateContext<'_>,
    safety_author: Option<&str>,
    packwiz: &Packwiz,
    cancel: &CancellationToken,
) -> Result<AppliedUpdates> {
    let scan = scan(root)?;
    if let Some(error) = &scan.inventory.index_error {
        return Err(Error::new(Code::InvalidPack, error.clone()));
    }
    let paths: Vec<String> = selections.iter().map(|s| s.path.clone()).collect();
    let chosen = selected(report, &scan.inventory.items, &paths)?;
    check_versions(&chosen, selections)?;
    let new_versions = fetch_targets(&chosen, ctx, Some(cancel)).await;
    if let Some(error) = &new_versions.error {
        return Err(Error::new(
            Code::Internal,
            format!("consulta da versão nova: {error}"),
        ));
    }

    let mut tx = PackTransaction::new(root.to_path_buf());
    let mut updated = Vec::new();
    for (reported, entry) in &chosen {
        let Some(new_version) = reported.new_version.as_ref() else {
            continue;
        };
        let old = scan
            .metafiles
            .get(&entry.path)
            .ok_or_else(|| not_updatable(&entry.path))?;
        let mut metafile = new_metafile(entry, old, new_version, &new_versions)?;
        metafile.option.clone_from(&old.option);
        tx.write(&entry.path, metafile.to_toml_string().into_bytes())?;
        updated.push(AppliedUpdate {
            path: entry.path.clone(),
            name: entry.name.clone(),
            from: reported.current.clone(),
            to: new_version.number.clone(),
            to_id: new_version.id.clone(),
        });
    }
    if updated.is_empty() {
        return Ok(AppliedUpdates {
            updated,
            safety_point: None,
        });
    }
    let safety_point = safety_author
        .map(|author| create_safety_point(root, author))
        .transpose()?;
    tx.commit(packwiz, cancel).await?;
    Ok(AppliedUpdates {
        updated,
        safety_point,
    })
}

/// Cada item escolhido precisa pedir a versão que a revisão mostrou.
fn check_versions(
    chosen: &[(&UpdateItem, &InventoryItem)],
    selections: &[UpdateSelection],
) -> Result<()> {
    let wanted: HashMap<String, &str> = selections
        .iter()
        .map(|selection| {
            (
                warden_packwiz::clean_path(&selection.path.replace('\\', "/")),
                selection.to_version_id.as_str(),
            )
        })
        .collect();
    for (reported, entry) in chosen {
        let expected = reported.new_version.as_ref().map(|v| v.id.as_str());
        if expected != wanted.get(&entry.path).copied() {
            return Err(Error::new(
                Code::InvalidInput,
                format!(
                    "{}: a versão pedida não é a que a revisão mostrou",
                    entry.path
                ),
            )
            .param("path", &entry.path));
        }
    }
    Ok(())
}

/// O `.pw.toml` novo de um item, a partir do antigo e da versão buscada na fonte.
fn new_metafile(
    entry: &InventoryItem,
    old: &Metafile,
    new_version: &NewVersion,
    versions: &NewVersions,
) -> Result<Metafile> {
    match entry.source {
        ItemSource::Modrinth => {
            let version = versions
                .modrinth
                .get(&new_version.id)
                .ok_or_else(|| missing_target(&entry.path))?;
            modrinth_metafile(old, version, &entry.path)
        }
        ItemSource::Curseforge => {
            let file = new_version
                .id
                .parse::<u64>()
                .ok()
                .and_then(|id| versions.curseforge.get(&id))
                .ok_or_else(|| missing_target(&entry.path))?;
            curseforge_metafile(old, file, &entry.path)
        }
        ItemSource::Url | ItemSource::Local => Err(not_updatable(&entry.path)),
    }
}

/// O ponto de segurança de "Atualizar todos" (ARCHITECTURE §11). Devolve o nome.
fn create_safety_point(root: &Path, author: &str) -> Result<String> {
    let repo = PackRepo::open(root).map_err(|e| Error::new(Code::Internal, e.to_string()))?;
    repo.check_writable()
        .map_err(|e| Error::new(Code::ReadOnly, e.to_string()))?;
    let point = repo
        .create_safety_point(
            &SafetyReason::UpdateAll,
            &Identity::for_pack(author, None),
            Moment::now_utc(),
        )
        .map_err(|e| Error::new(Code::Internal, e.to_string()))?;
    Ok(point.name)
}

fn missing_target(path: &str) -> Error {
    Error::new(
        Code::InvalidInput,
        format!("{path}: a fonte não devolveu a versão nova"),
    )
    .param("path", path)
}

fn modrinth_metafile(old: &Metafile, version: &Version, path: &str) -> Result<Metafile> {
    let file = version.primary_file().ok_or_else(|| missing_target(path))?;
    let (hash_format, hash) = if file.hashes.sha512.is_empty() {
        (HashFormat::Sha1, file.hashes.sha1.clone())
    } else {
        (HashFormat::Sha512, file.hashes.sha512.clone())
    };
    if hash.is_empty() {
        return Err(missing_target(path));
    }
    let side = old.side.clone();
    let mut metafile = Metafile::modrinth(
        &ModrinthFile {
            title: old.name.clone(),
            project_id: version.project_id.clone(),
            version_id: version.id.clone(),
            filename: file.filename.clone(),
            url: file.url.clone(),
            hash_format,
            hash,
        },
        side,
    );
    metafile.pin = old.pin;
    Ok(metafile)
}

fn curseforge_metafile(old: &Metafile, file: &CfFile, path: &str) -> Result<Metafile> {
    let (hash_format, hash) = if let Some(sha1) = file.hash(HashAlgo::Sha1) {
        (HashFormat::Sha1, sha1)
    } else if let Some(md5) = file.hash(HashAlgo::Md5) {
        (HashFormat::Md5, md5)
    } else {
        return Err(missing_target(path));
    };
    let project_id = u32::try_from(file.mod_id).map_err(|_| missing_target(path))?;
    let file_id = u32::try_from(file.id).map_err(|_| missing_target(path))?;
    let side: Side = old.side.clone();
    let mut metafile = Metafile::curseforge(
        &CurseForgeFile {
            name: old.name.clone(),
            project_id,
            file_id,
            filename: file.file_name.clone(),
            hash_format,
            hash,
        },
        side,
    );
    metafile.pin = old.pin;
    Ok(metafile)
}
