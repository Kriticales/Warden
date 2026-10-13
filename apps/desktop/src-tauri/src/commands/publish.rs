//! Comandos do domínio `publish` (SPEC T18; ARCHITECTURE §11.1): "Publicar a versão X para os
//! jogadores".
//!
//! - `publish_plan`: a árvore da versão montada pela exportação (E-01, origem "versão salva"),
//!   as conferências plugáveis (gancho 1.1, ADR-0039: [`PublishFinding`] e a lista de
//!   [`checks`]), as notas desde a última versão publicada e a situação da `main` do GitHub;
//! - `publish_run`: confere de novo o que bloqueia, publica (linha de publicação, envio
//!   protegido, Release) e devolve o link do pack, as notas e o id da Release (a E-04 anexa a
//!   instância pronta para o Prism por ele);
//! - `publish_retry_release`: "Tentar de novo" de uma versão "publicada, Release pendente".
//!
//! "Trocar pelo Modrinth" usa o `add_apply` que já existe (o mesmo arquivo, ou a versão
//! compatível do mesmo projeto mostrada antes), com `replaces` apontando o metafile da
//! CurseForge: muda o pack, e a versão precisa ser salva e publicada de novo.
#![allow(clippy::needless_pass_by_value)] // argumentos pertencem ao contrato do Tauri

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State};
use warden_core::{AppPaths, CancellationToken, PackId};
use warden_curseforge::{CurseforgeClient, Distribution, FileRef};
use warden_export::{ExportFormat, ExportSource};
use warden_modrinth::{HashAlgorithm, ModrinthClient};
use warden_packwiz::{Metafile, PackManifest};
use warden_packwiz_cli::Packwiz;
use warden_project::add::modrinth::ModrinthAdd;
use warden_project::add::{AddSource as _, ChannelPolicy};
use warden_project::dedup::{Identity as DedupIdentity, same_project};
use warden_project::hygiene::HygieneFinding;
use warden_project::meta::read_meta;
use warden_project::search::modrinth::{hit_compatible, modrinth_query};
use warden_project::search::{PackTarget, ProjectKind, SearchSort, SourceQuery};
use warden_project::side::SideChoice;
use warden_secrets::{SecretKind, SecretString};
use warden_versioning::github::GithubClient;
use warden_versioning::publish::{
    PublishCheck, PublishChecks, PublishContext, PublishFindingKind, PublishRequest, PublishTree,
    Published, RemoteStatus, ReplaceRemote, run_checks, scan_secrets,
};
use warden_versioning::{Identity, PackRepo, Snapshot, changelog};

use crate::commands::github::{GithubLinkView, StoredLink, client, stored_link};
use crate::commands::inventory::{self, notify_changed, pack_root};
use crate::commands::versioning::{changes_between, moment_now};
use crate::error::AppError;
use crate::events::PackArea;
use crate::operations::OperationKind;
use crate::state::AppState;

/// "Publicar versão" (gaveta de Tarefas).
const PUBLISH_RUN: OperationKind = OperationKind::new("publish.run");

/// Uma troca possível pelo Modrinth.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ModrinthSwap {
    /// Projeto no Modrinth.
    pub(crate) project_id: String,
    /// Versão no Modrinth.
    pub(crate) version_id: String,
    /// Número da versão (`0.5.1`).
    pub(crate) version_number: String,
    /// Nome do arquivo.
    pub(crate) file_name: String,
    /// O mesmo arquivo (mesmo SHA-1); falso: a versão compatível do mesmo projeto, mostrada
    /// antes de trocar.
    pub(crate) same_file: bool,
    /// O lado do item hoje (a troca mantém).
    pub(crate) side: SideChoice,
}

/// Um mod da CurseForge citado num aviso.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PublishMod {
    /// Metafile, relativo à pasta do pack.
    pub(crate) path: String,
    /// Nome.
    pub(crate) name: String,
    /// A troca pelo Modrinth, quando existe.
    pub(crate) swap: Option<ModrinthSwap>,
}

/// Qual conferência não pôde rodar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) enum PublishCheckId {
    /// Distribuição dos mods da CurseForge.
    CurseforgeDistribution,
    /// Mods só da CurseForge (equivalentes no Modrinth).
    CurseforgeOnly,
}

/// Por que uma conferência não rodou.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) enum CheckFailure {
    /// Sem a chave da CurseForge em Configurações.
    KeyMissing,
    /// Sem conexão ou o servidor não respondeu.
    Unreachable,
}

/// Um achado das conferências antes de publicar. Os que bloqueiam são
/// [`PublishFinding::SecretFound`] e [`PublishFinding::Hygiene`]; os demais são avisos
/// ("Publicar mesmo assim"). Registro acréscimo-apenas: a W-03 acrescenta os da 1.1.
#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub(crate) enum PublishFinding {
    /// Algo com cara de chave ou token: arquivo e linha.
    SecretFound {
        /// Arquivo.
        path: String,
        /// Linha.
        line: u32,
    },
    /// Arquivo que a higiene reprova (os mesmos do T04 e do T19).
    Hygiene {
        /// O achado.
        finding: HygieneFinding,
    },
    /// Mods da CurseForge com distribuição bloqueada.
    CurseforgeBlocked {
        /// Os mods.
        mods: Vec<PublishMod>,
    },
    /// Mods que só existem na CurseForge (D35, ADR-0051).
    CurseforgeOnly {
        /// Os mods.
        mods: Vec<PublishMod>,
    },
    /// Nenhum teste que fechou normalmente usou exatamente o conteúdo da versão.
    Untested,
    /// Uma conferência não pôde rodar.
    CheckFailed {
        /// Qual.
        check: PublishCheckId,
        /// Por quê.
        reason: CheckFailure,
    },
}

impl PublishFindingKind for PublishFinding {
    fn blocks(&self) -> bool {
        matches!(self, Self::SecretFound { .. } | Self::Hygiene { .. })
    }
}

/// O que o diálogo "Publicar a versão X para os jogadores" mostra.
#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PublishPlan {
    /// Versão.
    pub(crate) version: String,
    /// Última versão publicada (base das notas).
    pub(crate) last_published: Option<String>,
    /// Notas geradas (sem o recado para os jogadores).
    pub(crate) notes: String,
    /// Avisos e bloqueios, na ordem das conferências.
    pub(crate) findings: Vec<PublishFinding>,
    /// Algum achado bloqueia.
    pub(crate) blocked: bool,
    /// Arquivos que vão para o GitHub (sem `CHANGELOG.md` e `.gitattributes`).
    pub(crate) files: u32,
    /// O repositório (`None`: primeira publicação).
    pub(crate) link: Option<GithubLinkView>,
    /// O token do GitHub está salvo.
    pub(crate) token_configured: bool,
    /// A `main` do GitHub comparada com a última publicação daqui (com repositório e token).
    pub(crate) remote: Option<RemoteStatus>,
}

/// Pedido de `publish_run`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PublishRunRequest {
    /// Versão final.
    pub(crate) version: String,
    /// "Recado para os jogadores" (opcional), no topo das notas.
    pub(crate) player_notes: String,
    /// "Substituir o GitHub pelo que está aqui", para o commit que a pessoa viu.
    pub(crate) replace_remote: Option<ReplaceRemote>,
    /// Fuso do computador, em minutos (data do `CHANGELOG.md`).
    pub(crate) utc_offset_minutes: i32,
}

fn domain(error: warden_versioning::Error) -> AppError {
    AppError::from_domain(&error)
}

// ---------------------------------------------------------------------------------------
// A árvore da versão
// ---------------------------------------------------------------------------------------

/// A versão montada pela exportação numa pasta temporária, e o que a higiene reprova nela.
struct VersionTree {
    files: PublishTree,
    hygiene: Vec<HygieneFinding>,
}

async fn version_tree(
    state: &AppState,
    packwiz: &Packwiz,
    root: &Path,
    version: &str,
    cancel: &CancellationToken,
) -> Result<VersionTree, AppError> {
    let staging = state.paths.cache_dir().join("staging");
    std::fs::create_dir_all(&staging)
        .map_err(|error| AppError::internal(format!("pasta temporária: {error}")))?;
    let temp = tempfile::Builder::new()
        .prefix("publish-")
        .tempdir_in(&staging)
        .map_err(|error| AppError::internal(format!("pasta temporária: {error}")))?;
    let destination = temp.path().join("versao");
    let exported = warden_export::export(
        root,
        &ExportSource::Saved {
            version: version.to_owned(),
        },
        ExportFormat::Folder,
        &destination,
        &staging,
        packwiz,
        cancel,
    )
    .await
    .map_err(|error| AppError::from_domain(&error))?;
    let mut files = PublishTree::new();
    for relative in exported.files {
        let bytes = std::fs::read(destination.join(&relative))
            .map_err(|error| AppError::internal(format!("ler {relative}: {error}")))?;
        files.insert(relative, bytes);
    }
    let hygiene = warden_project::hygiene::scan(&destination)
        .map_err(|error| AppError::from_domain(&error))?
        .into_iter()
        .filter(|finding| finding.in_index)
        .collect();
    Ok(VersionTree { files, hygiene })
}

// ---------------------------------------------------------------------------------------
// Conferências
// ---------------------------------------------------------------------------------------

/// Varredura de segredos (bloqueia), com os valores atuais do cofre.
struct SecretsCheck {
    stored: Vec<SecretString>,
}

#[async_trait::async_trait]
impl PublishCheck for SecretsCheck {
    type Finding = PublishFinding;
    fn id(&self) -> &'static str {
        "secrets"
    }
    async fn run(&self, context: &PublishContext<'_>) -> Vec<PublishFinding> {
        scan_secrets(context.tree, &self.stored)
            .into_iter()
            .map(|found| PublishFinding::SecretFound {
                path: found.path,
                line: found.line,
            })
            .collect()
    }
}

/// Higiene (bloqueia): o resultado da varredura da pasta exportada.
struct HygieneCheck {
    findings: Vec<HygieneFinding>,
}

#[async_trait::async_trait]
impl PublishCheck for HygieneCheck {
    type Finding = PublishFinding;
    fn id(&self) -> &'static str {
        "hygiene"
    }
    async fn run(&self, _context: &PublishContext<'_>) -> Vec<PublishFinding> {
        self.findings
            .iter()
            .cloned()
            .map(|finding| PublishFinding::Hygiene { finding })
            .collect()
    }
}

/// Um item da CurseForge na versão.
#[derive(Debug, Clone)]
struct CfItem {
    path: String,
    name: String,
    project: u32,
    file: u32,
    sha1: Option<String>,
    side: SideChoice,
}

fn side_choice(side: &warden_packwiz::Side) -> SideChoice {
    match side {
        warden_packwiz::Side::Client => SideChoice::Client,
        warden_packwiz::Side::Server => SideChoice::Server,
        _ => SideChoice::Both,
    }
}

fn curseforge_items(tree: &PublishTree) -> Vec<CfItem> {
    tree.iter()
        .filter(|(path, _)| path.ends_with(".pw.toml"))
        .filter_map(|(path, bytes)| {
            let metafile = Metafile::parse(std::str::from_utf8(bytes).ok()?).ok()?.value;
            let (project, file) = metafile.curseforge_ids()?;
            let sha1 = metafile
                .download
                .hash_format
                .eq_ignore_ascii_case("sha1")
                .then(|| metafile.download.hash.trim().to_ascii_lowercase());
            Some(CfItem {
                path: path.clone(),
                name: metafile.name.clone(),
                project,
                file,
                sha1,
                side: side_choice(&metafile.side),
            })
        })
        .collect()
}

/// Mods da CurseForge: bloqueados para terceiros e os que só existem na CurseForge, com a
/// troca pelo Modrinth (o mesmo arquivo pelo SHA-1; senão, a versão compatível do mesmo
/// projeto achada pela regra de deduplicação da busca combinada, ADR-0027).
struct CurseforgeCheck {
    curseforge: CurseforgeClient,
    modrinth: ModrinthClient,
}

impl CurseforgeCheck {
    /// O mesmo arquivo no Modrinth, pelo SHA-1.
    async fn same_files(
        &self,
        items: &[CfItem],
        cancel: &CancellationToken,
    ) -> Result<BTreeMap<String, ModrinthSwap>, ()> {
        let hashes: Vec<String> = items.iter().filter_map(|item| item.sha1.clone()).collect();
        if hashes.is_empty() {
            return Ok(BTreeMap::new());
        }
        let found = self
            .modrinth
            .version_files(&hashes, HashAlgorithm::Sha1, Some(cancel))
            .await
            .map_err(|error| tracing::warn!(%error, "Modrinth: arquivos por SHA-1"))?;
        let mut swaps = BTreeMap::new();
        for item in items {
            let Some(hash) = &item.sha1 else { continue };
            let Some(version) = found.get(hash) else {
                continue;
            };
            let Some(file) = version
                .files
                .iter()
                .find(|file| file.hashes.sha1.eq_ignore_ascii_case(hash))
            else {
                continue;
            };
            swaps.insert(
                item.path.clone(),
                ModrinthSwap {
                    project_id: version.project_id.clone(),
                    version_id: version.id.clone(),
                    version_number: version.version_number.clone(),
                    file_name: file.filename.clone(),
                    same_file: true,
                    side: item.side,
                },
            );
        }
        Ok(swaps)
    }

    /// A versão compatível do mesmo projeto no Modrinth (precisa da chave da CurseForge para
    /// saber o autor e o slug do projeto).
    async fn same_project(
        &self,
        items: &[&CfItem],
        target: &PackTarget,
        cancel: &CancellationToken,
    ) -> BTreeMap<String, ModrinthSwap> {
        let mut swaps = BTreeMap::new();
        if items.is_empty() || !self.curseforge.has_key() {
            return swaps;
        }
        let ids: Vec<u64> = items.iter().map(|item| u64::from(item.project)).collect();
        let Ok(projects) = self.curseforge.projects(&ids, Some(cancel)).await else {
            return swaps;
        };
        let add = ModrinthAdd::new(self.modrinth.clone());
        for item in items {
            let Some(project) = projects.iter().find(|p| p.id == u64::from(item.project)) else {
                continue;
            };
            let author = project.authors.first().map(|a| a.name.as_str()).unwrap_or_default();
            let query = SourceQuery {
                query: project.name.clone(),
                kind: ProjectKind::Mod,
                sort: SearchSort::Relevance,
                environment: None,
                include_incompatible: false,
                target: target.clone(),
                offset: 0,
                limit: 10,
            };
            let Ok(results) = self.modrinth.search(&modrinth_query(&query), Some(cancel)).await
            else {
                continue;
            };
            let identity = DedupIdentity {
                author,
                slug: &project.slug,
                title: &project.name,
            };
            let Some(hit) = results.hits.iter().find(|hit| {
                hit_compatible(hit, ProjectKind::Mod, target)
                    && same_project(
                        identity,
                        DedupIdentity {
                            author: &hit.author,
                            slug: &hit.slug,
                            title: &hit.title,
                        },
                    )
            }) else {
                continue;
            };
            let Ok(found) = add.projects(std::slice::from_ref(&hit.project_id), cancel).await else {
                continue;
            };
            let Some(source_project) = found.first() else {
                continue;
            };
            let Ok(versions) = add.compatible_versions(source_project, target, cancel).await
            else {
                continue;
            };
            let Some(version) = ChannelPolicy::default().pick(&versions) else {
                continue;
            };
            swaps.insert(
                item.path.clone(),
                ModrinthSwap {
                    project_id: source_project.id.clone(),
                    version_id: version.id.clone(),
                    version_number: version.number.clone(),
                    file_name: version
                        .file
                        .as_ref()
                        .map(|file| file.name.clone())
                        .unwrap_or_default(),
                    same_file: false,
                    side: item.side,
                },
            );
        }
        swaps
    }
}

#[async_trait::async_trait]
impl PublishCheck for CurseforgeCheck {
    type Finding = PublishFinding;
    fn id(&self) -> &'static str {
        "curseforge"
    }
    async fn run(&self, context: &PublishContext<'_>) -> Vec<PublishFinding> {
        let items = curseforge_items(context.tree);
        if items.is_empty() {
            return Vec::new();
        }
        let mut findings = Vec::new();
        let Ok(mut swaps) = self.same_files(&items, context.cancel).await else {
            return vec![PublishFinding::CheckFailed {
                check: PublishCheckId::CurseforgeOnly,
                reason: CheckFailure::Unreachable,
            }];
        };
        // Bloqueados para terceiros: precisa da chave da CurseForge.
        let mut blocked_paths = Vec::new();
        if self.curseforge.has_key() {
            let refs: Vec<FileRef> = items
                .iter()
                .map(|item| FileRef::new(u64::from(item.project), u64::from(item.file)))
                .collect();
            match self.curseforge.distribution(&refs, Some(context.cancel)).await {
                Ok(answers) => {
                    for (item, answer) in items.iter().zip(answers) {
                        if !matches!(answer.distribution, Distribution::Allowed { .. }) {
                            blocked_paths.push(item.path.clone());
                        }
                    }
                }
                Err(error) => {
                    tracing::warn!(%error, "CurseForge: distribuição");
                    findings.push(PublishFinding::CheckFailed {
                        check: PublishCheckId::CurseforgeDistribution,
                        reason: CheckFailure::Unreachable,
                    });
                }
            }
        } else {
            findings.push(PublishFinding::CheckFailed {
                check: PublishCheckId::CurseforgeDistribution,
                reason: CheckFailure::KeyMissing,
            });
        }
        let without_same: Vec<&CfItem> = items
            .iter()
            .filter(|item| !swaps.contains_key(&item.path))
            .collect();
        let manifest = context
            .tree
            .get("pack.toml")
            .and_then(|bytes| std::str::from_utf8(bytes).ok())
            .and_then(|text| PackManifest::parse(text).ok())
            .map(|parsed| parsed.value);
        if let Some(target) = manifest.and_then(|m| PackTarget::from_manifest(&m).ok()) {
            swaps.extend(self.same_project(&without_same, &target, context.cancel).await);
        }
        let to_mod = |item: &CfItem| PublishMod {
            path: item.path.clone(),
            name: item.name.clone(),
            swap: swaps.get(&item.path).cloned(),
        };
        let blocked: Vec<PublishMod> = items
            .iter()
            .filter(|item| blocked_paths.contains(&item.path))
            .map(to_mod)
            .collect();
        if !blocked.is_empty() {
            findings.insert(0, PublishFinding::CurseforgeBlocked { mods: blocked });
        }
        // Só da CurseForge (D35, CA-T18-08): todo item que os jogadores baixam pela CurseForge,
        // com a troca onde o Modrinth tem o mesmo arquivo ou o mesmo projeto. Os bloqueados já
        // aparecem no aviso acima.
        let only: Vec<PublishMod> = items
            .iter()
            .filter(|item| !blocked_paths.contains(&item.path))
            .map(to_mod)
            .collect();
        if !only.is_empty() {
            findings.push(PublishFinding::CurseforgeOnly { mods: only });
        }
        findings
    }
}

/// Hashes da árvore testada nas sessões que fecharam normalmente.
fn tested_trees(paths: &AppPaths, pack_id: PackId) -> Vec<String> {
    let dir = warden_instance::InstanceDirs::for_pack(paths, pack_id)
        .state_dir
        .join("sessions");
    let Ok(sessions) = warden_launcher::session::list_sessions(&dir) else {
        return Vec::new();
    };
    sessions
        .iter()
        .filter_map(|session| session.record.as_ref())
        .filter(|record| record.outcome == warden_launcher::outcome::Outcome::ClosedNormally)
        .filter_map(|record| {
            record
                .extra
                .get("packTree")?
                .get("hash")?
                .as_str()
                .map(ToOwned::to_owned)
        })
        .collect()
}

/// Versão não testada: nenhuma sessão que fechou normalmente tem a árvore da versão
/// (o mesmo hash do Testar, L-04).
struct UntestedCheck {
    paths: AppPaths,
    pack_id: PackId,
}

#[async_trait::async_trait]
impl PublishCheck for UntestedCheck {
    type Finding = PublishFinding;
    fn id(&self) -> &'static str {
        "untested"
    }
    async fn run(&self, context: &PublishContext<'_>) -> Vec<PublishFinding> {
        let (Some(pack), Some(index)) = (context.tree.get("pack.toml"), context.tree.get("index.toml"))
        else {
            return vec![PublishFinding::Untested];
        };
        let hash = crate::test_session::pack_tree_hash_of(pack, "index.toml", index);
        let (paths, pack_id) = (self.paths.clone(), self.pack_id);
        let tested = tauri::async_runtime::spawn_blocking(move || tested_trees(&paths, pack_id))
            .await
            .unwrap_or_default();
        if tested.contains(&hash) {
            Vec::new()
        } else {
            vec![PublishFinding::Untested]
        }
    }
}

/// Valores do cofre que a varredura procura (CurseForge, Gemini e GitHub).
fn stored_secrets(state: &AppState) -> Vec<SecretString> {
    SecretKind::ALL
        .into_iter()
        .filter_map(|kind| state.secrets.get(kind).ok().flatten())
        .collect()
}

/// As conferências da v1, na ordem do diálogo. Registro acréscimo-apenas (gancho 1.1): a W-03
/// acrescenta "Segurança dos mods" e "Manutenção dos mods" aqui.
fn checks(state: &AppState, pack_id: PackId, hygiene: Vec<HygieneFinding>) -> Result<PublishChecks<PublishFinding>, AppError> {
    Ok(vec![
        Arc::new(SecretsCheck {
            stored: stored_secrets(state),
        }),
        Arc::new(HygieneCheck { findings: hygiene }),
        Arc::new(CurseforgeCheck {
            curseforge: inventory::curseforge(state)?,
            modrinth: state.modrinth.clone(),
        }),
        Arc::new(UntestedCheck {
            paths: state.paths.clone(),
            pack_id,
        }),
    ])
}

/// Notas geradas desde a última versão publicada (CA-T18-05), com o recado no topo.
async fn release_notes(
    state: &AppState,
    root: &Path,
    version: &str,
    player_notes: &str,
) -> Result<(Option<String>, String), AppError> {
    let base = {
        let root = root.to_path_buf();
        tauri::async_runtime::spawn_blocking(move || -> Result<_, AppError> {
            let repo = PackRepo::open(&root).map_err(domain)?;
            Ok((
                repo.last_published().map_err(domain)?,
                repo.publication_base().map_err(domain)?,
            ))
        })
        .await
        .map_err(|error| AppError::internal(format!("notas da versão: {error}")))??
    };
    let changes = changes_between(
        Some(&state.modrinth),
        root,
        base.1,
        Snapshot::Version(version.to_owned()),
    )
    .await?;
    Ok((
        base.0.map(|v| v.to_string()),
        changelog::render_body(player_notes, &changes),
    ))
}

async fn check_publishable(root: &Path, version: &str) -> Result<(), AppError> {
    let (root, version) = (root.to_path_buf(), version.to_owned());
    tauri::async_runtime::spawn_blocking(move || {
        PackRepo::open(&root)
            .and_then(|repo| repo.check_publishable(&version))
            .map(|_| ())
            .map_err(domain)
    })
    .await
    .map_err(|error| AppError::internal(format!("conferir a versão: {error}")))?
}

/// O diálogo "Publicar a versão X para os jogadores": avisos, bloqueios, notas e a situação
/// do GitHub. Não muda nada.
#[tauri::command]
#[specta::specta]
pub(crate) async fn publish_plan(
    state: State<'_, AppState>,
    pack_id: PackId,
    version: String,
) -> Result<PublishPlan, AppError> {
    plan_impl(&state, &inventory::packwiz(&state)?, pack_id, version).await
}

async fn plan_impl(
    state: &AppState,
    packwiz: &Packwiz,
    pack_id: PackId,
    version: String,
) -> Result<PublishPlan, AppError> {
    let _lock = state.locks.read(pack_id).await;
    let root = pack_root(state, pack_id)?;
    check_publishable(&root, &version).await?;
    let cancel = CancellationToken::new();
    let tree = version_tree(state, packwiz, &root, &version, &cancel).await?;
    let context = PublishContext {
        root: &root,
        version: &version,
        tree: &tree.files,
        cancel: &cancel,
    };
    let findings = run_checks(&checks(state, pack_id, tree.hygiene.clone())?, &context).await;
    let (last_published, notes) = release_notes(state, &root, &version, "").await?;
    let link = stored_link(state, pack_id)?;
    let token_configured = state.secrets.status()?.github;
    let remote = match (&link, token_configured) {
        (Some(link), true) => Some(
            warden_versioning::publish::remote_status(
                &client(state).await?,
                &root,
                &link.repo(),
                &cancel,
            )
            .await
            .map_err(domain)?,
        ),
        _ => None,
    };
    Ok(PublishPlan {
        version,
        last_published,
        notes,
        blocked: findings.iter().any(PublishFindingKind::blocks),
        findings,
        files: u32::try_from(tree.files.len()).unwrap_or(u32::MAX),
        link: link.as_ref().map(crate::commands::github::view),
        token_configured,
        remote,
    })
}

/// Publica a versão final: confere de novo o que bloqueia, envia e cria a Release.
///
/// Erros: `versioning.PUBLISH_SECRET_FOUND` / `PUBLISH_BLOCKED` (nada enviado),
/// `GITHUB_NOT_LINKED`, `REMOTE_DIVERGED`, `RELEASE_PENDING` (enviado; só a Release falhou) e
/// os do GitHub.
#[tauri::command]
#[specta::specta]
pub(crate) async fn publish_run(
    app: AppHandle,
    state: State<'_, AppState>,
    pack_id: PackId,
    request: PublishRunRequest,
) -> Result<Published, AppError> {
    let operation = state.operations.start(PUBLISH_RUN, Some(pack_id), false);
    let result = async {
        let _lock = state.locks.write_for(pack_id, &operation).await;
        let root = pack_root(&state, pack_id)?;
        let when = moment_now(request.utc_offset_minutes)?;
        let link = stored_link(&state, pack_id)?.ok_or_else(|| {
            domain(warden_versioning::Error::GithubNotLinked)
        })?;
        check_publishable(&root, &request.version).await?;
        let cancel = operation.token().clone();
        let tree = version_tree(&state, &inventory::packwiz(&state)?, &root, &request.version, &cancel).await?;
        if let Some(found) = scan_secrets(&tree.files, &stored_secrets(&state)).first() {
            return Err(domain(warden_versioning::Error::PublishSecretFound {
                path: found.path.clone(),
                line: found.line,
            }));
        }
        if !tree.hygiene.is_empty() {
            return Err(domain(warden_versioning::Error::PublishBlocked {
                count: tree.hygiene.len(),
            }));
        }
        let (_, notes) = release_notes(&state, &root, &request.version, &request.player_notes).await?;
        let client = client(&state).await?;
        let login = client.user(Some(&cancel)).await.map_err(domain)?;
        let meta = read_meta(&root).map_err(|error| AppError::from_domain(&error))?;
        let identity = Identity::for_pack(&meta.author, Some(&login));
        let publish_request = PublishRequest {
            version: &request.version,
            tree: &tree.files,
            notes: &notes,
            pack_name: &meta.name,
            identity: &identity,
            when,
        };
        publish_with(
            &client,
            &root,
            &link,
            &publish_request,
            request.replace_remote.as_ref(),
            &cancel,
        )
        .await
    }
    .await;
    // A versão passa a "publicada" também com a Release pendente.
    notify_changed(&app, pack_id, vec![PackArea::History]);
    operation.finish(result)
}

async fn publish_with(
    client: &GithubClient,
    root: &Path,
    link: &StoredLink,
    request: &PublishRequest<'_>,
    replace: Option<&ReplaceRemote>,
    cancel: &CancellationToken,
) -> Result<Published, AppError> {
    warden_versioning::publish::publish(client, root, &link.repo(), request, replace, cancel)
        .await
        .map_err(domain)
}

/// "Tentar de novo" de uma versão publicada com a Release pendente: cria só a Release.
#[tauri::command]
#[specta::specta]
pub(crate) async fn publish_retry_release(
    app: AppHandle,
    state: State<'_, AppState>,
    pack_id: PackId,
    version: String,
) -> Result<Published, AppError> {
    let _lock = state.locks.write(pack_id).await;
    let root = pack_root(&state, pack_id)?;
    let link = stored_link(&state, pack_id)?
        .ok_or_else(|| domain(warden_versioning::Error::GithubNotLinked))?;
    let client = client(&state).await?;
    let result = warden_versioning::publish::retry_release(&client, &root, &link.repo(), &version)
        .await
        .map_err(domain);
    notify_changed(&app, pack_id, vec![PackArea::History]);
    result
}


#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::time::{Duration, UNIX_EPOCH};

    use serde_json::json;
    use sha1::{Digest as _, Sha1};
    use warden_http::{HttpClient, HttpConfig};
    use warden_launcher::outcome::{CrashArtifacts, Outcome};
    use warden_launcher::process::GameExit;
    use warden_launcher::session::{SessionDir, SessionRecord};
    use warden_launcher::{GameSpec, LoaderSpec};
    use warden_packwiz::{CurseForgeFile, HashFormat, Side};
    use warden_project::transaction::PackTransaction;
    use warden_versioning::{InitialPoint, SaveVersion};
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;
    use crate::commands::inventory::tests::{packwiz_binary, registered_pack, test_packwiz};
    use crate::state::tests::test_state;

    const GHP: &str = "ghp_abcdefghijklmnopqrstuvwxyz0123456789";

    fn sha1_hex(bytes: &[u8]) -> String {
        hex::encode(Sha1::digest(bytes))
    }

    fn cf_metafile(name: &str, project: u32, file: u32, bytes: &[u8]) -> String {
        Metafile::curseforge(
            &CurseForgeFile {
                name: name.into(),
                project_id: project,
                file_id: file,
                filename: format!("{name}.jar"),
                hash_format: HashFormat::Sha1,
                hash: sha1_hex(bytes),
            },
            Side::Client,
        )
        .to_toml_string()
    }

    fn when() -> warden_versioning::Moment {
        warden_versioning::Moment::new(1_790_866_800, -180)
    }

    /// Pack registrado, com histórico e a versão final 1.0.0 com `files` (relativos).
    async fn final_version(
        state: &AppState,
        dir: &std::path::Path,
        files: &[(&str, &[u8])],
    ) -> Option<(PackId, PathBuf, Packwiz)> {
        let cli = test_packwiz(state, packwiz_binary()?);
        let (pack_id, root) = registered_pack(state, dir, &cli, 0).await;
        let identity = Identity::for_pack("Autor", None);
        let repo = PackRepo::init(&root, InitialPoint::Created, &identity, when()).unwrap();
        for (relative, bytes) in files {
            let path = root.join(relative);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, bytes).unwrap();
        }
        PackTransaction::new(root.clone())
            .commit(&cli, &CancellationToken::new())
            .await
            .unwrap();
        repo.save_version(&SaveVersion {
            version: "1.0.0",
            tag_message: "## 1.0.0\n",
            identity: &identity,
            when: when(),
            mark_final: true,
        })
        .unwrap();
        Some((pack_id, root, cli))
    }

    fn record_closed_session(state: &AppState, pack_id: PackId, hash: &str, outcome: Outcome) {
        let dir = warden_instance::InstanceDirs::for_pack(&state.paths, pack_id)
            .state_dir
            .join("sessions");
        fs::create_dir_all(&dir).unwrap();
        let started = UNIX_EPOCH + Duration::from_secs(1_790_874_309);
        let session = SessionDir::create(&dir, started).unwrap();
        let spec = GameSpec::new(
            "1.20.1",
            LoaderSpec::Fabric {
                version: "0.16.9".into(),
            },
        )
        .unwrap();
        let exit = GameExit {
            outcome,
            exit_code: Some(0),
            stop_requested: false,
            duration_ms: 1000,
            artifacts: CrashArtifacts::default(),
        };
        let mut record =
            SessionRecord::from_exit(started, &exit, &spec, 17, None, std::path::Path::new("jogo"));
        record
            .extra
            .insert("packTree".into(), json!({ "hash": hash, "head": null, "unsavedFiles": 0 }));
        session.write_record(&record).unwrap();
    }

    #[tokio::test]
    async fn ca_t18_06_token_no_config_bloqueia_com_arquivo_e_linha_e_versao_nao_testada_avisa() {
        let dir = tempfile::tempdir().unwrap();
        let state = test_state(dir.path());
        let token = format!("[geral]\nchave = \"{GHP}\"\n");
        let Some((pack_id, _root, cli)) = final_version(
            &state,
            dir.path(),
            &[
                ("config/x.toml", token.as_bytes()),
                ("config/ok.toml", b"a = 1\n"),
            ],
        )
        .await
        else {
            return;
        };
        let plan = plan_impl(&state, &cli, pack_id, "1.0.0".into()).await.unwrap();
        assert!(plan.blocked);
        assert!(plan.findings.iter().any(|finding| matches!(
            finding,
            PublishFinding::SecretFound { path, line } if path == "config/x.toml" && *line == 2
        )));
        assert!(plan.findings.iter().any(|f| matches!(f, PublishFinding::Untested)));
        assert_eq!(plan.files, 4, "pack.toml, index.toml e as duas configs");
        assert!(plan.notes.contains("config/ok.toml"), "{}", plan.notes);
        assert_eq!(plan.last_published, None);
        assert!(plan.link.is_none() && !plan.token_configured && plan.remote.is_none());
        let json = serde_json::to_value(&plan.findings[0]).unwrap();
        assert_eq!(json["kind"], "secretFound");
        assert!(!serde_json::to_string(&plan).unwrap().contains(GHP), "o valor nunca vai à interface");
    }

    #[tokio::test]
    async fn ca_t18_06_teste_que_fechou_normalmente_com_o_mesmo_conteudo_tira_o_aviso() {
        let dir = tempfile::tempdir().unwrap();
        let state = test_state(dir.path());
        let Some((pack_id, root, cli)) =
            final_version(&state, dir.path(), &[("config/ok.toml", b"a = 1\n")]).await
        else {
            return;
        };
        let pack = fs::read(root.join("pack.toml")).unwrap();
        let index = fs::read(root.join("index.toml")).unwrap();
        let hash = crate::test_session::pack_tree_hash_of(&pack, "index.toml", &index);
        // Um teste que travou com o mesmo conteúdo não conta.
        record_closed_session(&state, pack_id, &hash, Outcome::Crashed);
        let plan = plan_impl(&state, &cli, pack_id, "1.0.0".into()).await.unwrap();
        assert!(plan.findings.iter().any(|f| matches!(f, PublishFinding::Untested)));
        record_closed_session(&state, pack_id, &hash, Outcome::ClosedNormally);
        let plan = plan_impl(&state, &cli, pack_id, "1.0.0".into()).await.unwrap();
        assert!(
            !plan.findings.iter().any(|f| matches!(f, PublishFinding::Untested)),
            "{:?}",
            plan.findings
        );
        assert!(!plan.blocked);
    }

    #[tokio::test]
    async fn versao_que_nao_e_final_nem_chega_a_exportar() {
        let dir = tempfile::tempdir().unwrap();
        let state = test_state(dir.path());
        let Some((pack_id, root, cli)) =
            final_version(&state, dir.path(), &[("config/ok.toml", b"a = 1\n")]).await
        else {
            return;
        };
        PackRepo::open(&root).unwrap().set_final("1.0.0", false).unwrap();
        let error = plan_impl(&state, &cli, pack_id, "1.0.0".into()).await.unwrap_err();
        assert_eq!(
            serde_json::to_value(error.code).unwrap(),
            json!({ "domain": "versioning", "code": "NOT_FINAL" })
        );
    }

    /// Modrinth e CurseForge simulados: `bloqueado` (CurseForge recusa terceiros, o mesmo
    /// arquivo existe no Modrinth), `jei` (permitido, o mesmo arquivo no Modrinth) e `xaero`
    /// (permitido, só na CurseForge).
    async fn servers(blocked: bool) -> (MockServer, CurseforgeCheck) {
        let server = MockServer::start().await;
        let files = json!({"data": [
            {"id": 1, "modId": 10, "fileName": "bloqueado.jar", "downloadUrl": serde_json::Value::Null, "fileLength": 1, "hashes": []},
            {"id": 2, "modId": 20, "fileName": "jei.jar", "downloadUrl": "https://edge.forgecdn.net/files/0/2/jei.jar", "fileLength": 1, "hashes": []},
            {"id": 3, "modId": 30, "fileName": "xaero.jar", "downloadUrl": "https://edge.forgecdn.net/files/0/3/xaero.jar", "fileLength": 1, "hashes": []},
        ]});
        let mut files = files;
        if !blocked {
            files["data"].as_array_mut().unwrap().remove(0);
        }
        Mock::given(method("POST"))
            .and(path("/v1/mods/files"))
            .respond_with(ResponseTemplate::new(200).set_body_json(files))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/v1/mods"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"data": [
                {"id": 10, "links": {"websiteUrl": "https://www.curseforge.com/minecraft/mc-mods/b"}, "allowModDistribution": false}
            ]})))
            .mount(&server)
            .await;
        let version = |id: &str, project: &str, bytes: &[u8], name: &str| {
            json!({
                "id": id, "project_id": project, "name": name, "version_number": "1.0.0",
                "version_type": "release",
                "files": [{"hashes": {"sha1": sha1_hex(bytes), "sha512": "00"},
                           "url": format!("https://cdn.modrinth.com/data/{project}/{name}.jar"),
                           "filename": format!("{name}.jar"), "primary": true, "size": 1}]
            })
        };
        Mock::given(method("POST"))
            .and(path("/v2/version_files"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                sha1_hex(b"BLOQUEADO"): version("vB", "PB", b"BLOQUEADO", "bloqueado"),
                sha1_hex(b"JEI"): version("vJ", "PJ", b"JEI", "jei"),
            })))
            .mount(&server)
            .await;
        let http = HttpClient::new(HttpConfig::for_version("1")).unwrap();
        // Sem chave: a busca pelo mesmo projeto (precisa do autor na CurseForge) não roda;
        // com chave, a distribuição é conferida.
        let key = Some(SecretString::from("chave".to_owned()));
        let check = CurseforgeCheck {
            curseforge: CurseforgeClient::with_base_url(
                http.clone(),
                &format!("{}/v1", server.uri()),
                key,
            )
            .unwrap(),
            modrinth: ModrinthClient::with_base_url(http, &format!("{}/v2", server.uri()), None)
                .unwrap(),
        };
        (server, check)
    }

    fn tree(items: &[(&str, &str, u32, u32, &[u8])]) -> PublishTree {
        let mut tree = PublishTree::new();
        for (file, name, project, id, bytes) in items {
            tree.insert(
                format!("mods/{file}.pw.toml"),
                cf_metafile(name, *project, *id, bytes).into_bytes(),
            );
        }
        tree
    }

    async fn run(check: &CurseforgeCheck, tree: &PublishTree) -> Vec<PublishFinding> {
        let cancel = CancellationToken::new();
        check
            .run(&PublishContext {
                root: std::path::Path::new("."),
                version: "1.0.0",
                tree,
                cancel: &cancel,
            })
            .await
    }

    #[tokio::test]
    async fn ca_t18_06_mod_bloqueado_avisa_com_a_troca_pelo_mesmo_arquivo() {
        let (_server, check) = servers(true).await;
        let tree = tree(&[("bloqueado", "Bloqueado", 10, 1, b"BLOQUEADO")]);
        let findings = run(&check, &tree).await;
        let [PublishFinding::CurseforgeBlocked { mods }] = findings.as_slice() else {
            panic!("{findings:?}");
        };
        assert_eq!(mods.len(), 1);
        assert_eq!(mods[0].name, "Bloqueado");
        let swap = mods[0].swap.as_ref().unwrap();
        assert!(swap.same_file);
        assert_eq!(swap.project_id, "PB");
        assert_eq!(swap.version_id, "vB");
        assert_eq!(swap.side, SideChoice::Client, "a troca mantém o lado");
        assert!(!findings[0].blocks(), "aviso, nunca bloqueio");
    }

    #[tokio::test]
    async fn ca_t18_08_dois_mods_so_da_curseforge_e_a_troca_so_no_que_existe_no_modrinth() {
        let (_server, check) = servers(false).await;
        let tree = tree(&[
            ("jei", "Just Enough Items", 20, 2, b"JEI"),
            ("xaero", "Xaero's Minimap", 30, 3, b"XAERO"),
        ]);
        let findings = run(&check, &tree).await;
        let [PublishFinding::CurseforgeOnly { mods }] = findings.as_slice() else {
            panic!("{findings:?}");
        };
        let names: Vec<&str> = mods.iter().map(|m| m.name.as_str()).collect();
        assert_eq!(names, vec!["Just Enough Items", "Xaero's Minimap"]);
        assert!(mods[0].swap.as_ref().is_some_and(|swap| swap.same_file));
        assert!(mods[1].swap.is_none());
        assert!(!findings.iter().any(PublishFindingKind::blocks), "não bloqueia");
    }

    #[tokio::test]
    async fn ca_t18_08_a_troca_grava_o_modrinth_e_deixa_a_versao_para_salvar_de_novo() {
        let dir = tempfile::tempdir().unwrap();
        let state = test_state(dir.path());
        let jei = cf_metafile("Just Enough Items", 20, 2, b"JEI");
        let Some((_, root, cli)) =
            final_version(&state, dir.path(), &[("mods/jei.pw.toml", jei.as_bytes())]).await
        else {
            return;
        };
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v2/projects"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([{
                "id": "PJ", "slug": "jei", "title": "Just Enough Items", "project_type": "mod",
                "description": "", "client_side": "required", "server_side": "optional",
                "categories": [], "versions": ["vJ"], "downloads": 1, "followers": 1,
                "updated": "2026-01-01T00:00:00Z", "published": "2026-01-01T00:00:00Z",
                "team": "t", "license": {"id": "MIT"}, "status": "approved",
                "game_versions": ["1.20.1"], "loaders": ["fabric"]
            }])))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/v2/versions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([{
                "id": "vJ", "project_id": "PJ", "author_id": "a", "name": "JEI",
                "version_number": "1.0.0", "version_type": "release",
                "game_versions": ["1.20.1"], "loaders": ["fabric"], "featured": true,
                "date_published": "2026-01-01T00:00:00Z", "downloads": 1,
                "files": [{"hashes": {"sha1": sha1_hex(b"JEI"), "sha512": "ab".repeat(64)},
                           "url": "https://cdn.modrinth.com/data/PJ/versions/vJ/jei.jar",
                           "filename": "jei.jar", "primary": true, "size": 3}],
                "dependencies": []
            }])))
            .mount(&server)
            .await;
        let http = HttpClient::new(HttpConfig::for_version("1")).unwrap();
        let modrinth =
            ModrinthClient::with_base_url(http, &format!("{}/v2", server.uri()), None).unwrap();
        let sources = warden_project::add::AddSources::new()
            .with(Arc::new(ModrinthAdd::new(modrinth)));
        // O que a interface manda no "Trocar pelo Modrinth" (o mesmo `add_apply` do Adicionar).
        let request: warden_project::add::AddApplyRequest = serde_json::from_value(json!({
            "items": [{"source": "modrinth", "projectId": "PJ", "versionId": "vJ",
                       "side": "client", "replaces": "mods/jei.pw.toml"}]
        }))
        .unwrap();
        let result = warden_project::add::add_apply(
            &root,
            &sources,
            &request,
            &cli,
            &CancellationToken::new(),
        )
        .await
        .unwrap();
        assert_eq!(result.removed, vec!["mods/jei.pw.toml"]);
        let added = &result.added[0].path;
        let metafile = Metafile::parse(&fs::read_to_string(root.join(added)).unwrap())
            .unwrap()
            .value;
        assert_eq!(metafile.modrinth_ids(), Some(("PJ", "vJ")));
        assert_eq!(metafile.side, Side::Client);
        let unsaved = PackRepo::open(&root).unwrap().unsaved_changes().unwrap();
        assert!(!unsaved.is_empty(), "a versão precisa ser salva de novo");
    }
}
