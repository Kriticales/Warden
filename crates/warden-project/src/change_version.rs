//! "Trocar de versão" de um item (SPEC T07, **Mais opções**; P1-15).
//!
//! O usuário escolhe **qualquer** versão compatível com o pack (a versão do Minecraft e o loader
//! do pack), inclusive uma mais antiga que a instalada. O fluxo tem três passos, como o de
//! atualizar (T10):
//!
//! 1. [`options`]: as versões compatíveis, da mais nova para a mais antiga, com a instalada
//!    marcada e as mais antigas que ela marcadas como rebaixamento;
//! 2. [`plan`]: o que muda com a versão escolhida (rebaixamento, dependências obrigatórias que
//!    faltam no pack e incompatibilidades com itens do pack). Só lê;
//! 3. [`apply`]: grava o `.pw.toml` novo (lado, opcional, fixar e nome ficam como estavam) numa
//!    transação, com um `packwiz refresh`.
//!
//! Funciona com o Modrinth e a CurseForge pelas mesmas fontes do "Adicionar"
//! ([`crate::add::AddSource`]): as versões compatíveis, as versões pelos IDs e o metafile que o
//! packwiz escreveria. Arquivo local e link direto não têm versões para escolher.

use std::collections::{BTreeSet, HashMap};
use std::path::Path;

use serde::Serialize;
use warden_core::CancellationToken;
use warden_packwiz_cli::Packwiz;

use crate::add::{
    AddSource, AddSources, DependencyKind, SourceProject, SourceVersion, VersionChannel,
};
use crate::inventory::{InventoryItem, ItemSource, ItemState, Scan, scan};
use crate::search::{PackTarget, SourceId};
use crate::side::SideChoice;
use crate::transaction::PackTransaction;
use crate::updates::{NewConflict, NewDependency};
use crate::{Error, ProjectErrorCode as Code, Result};

/// Uma versão na lista de "Trocar de versão".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct VersionChoice {
    /// ID da versão (Modrinth) ou do arquivo (CurseForge). Volta em [`plan`] e [`apply`].
    pub id: String,
    /// Número legível.
    pub number: String,
    /// Canal.
    pub channel: VersionChannel,
    /// Data de publicação (RFC 3339).
    pub published: String,
    /// Nome do arquivo.
    pub file_name: String,
    /// É a versão instalada.
    pub installed: bool,
    /// É mais antiga que a instalada (trocar para ela é um rebaixamento).
    pub older: bool,
}

/// O que o diálogo "Trocar de versão" mostra.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ChangeVersionOptions {
    /// Caminho do `.pw.toml`.
    pub path: String,
    /// Nome do item.
    pub name: String,
    /// Fonte.
    pub source: SourceId,
    /// ID da versão instalada (volta em [`apply`] para conferir que o item não mudou).
    pub current_id: Option<String>,
    /// Versão instalada, legível (nome do arquivo quando a fonte não a devolve).
    pub current: Option<String>,
    /// A versão instalada não serve mais para o pack (outro Minecraft ou loader).
    pub current_incompatible: bool,
    /// O item está fixado (`pin = true`): continua fixado na versão escolhida.
    pub pinned: bool,
    /// As versões compatíveis com o pack, da mais nova para a mais antiga (todos os canais).
    pub versions: Vec<VersionChoice>,
}

/// O que muda com a versão escolhida.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ChangeVersionPlan {
    /// Caminho do `.pw.toml`.
    pub path: String,
    /// Nome do item.
    pub name: String,
    /// Versão instalada, legível.
    pub current: Option<String>,
    /// A versão escolhida.
    pub to: VersionChoice,
    /// É a mesma versão instalada: nada a fazer.
    pub same: bool,
    /// A versão escolhida é mais antiga que a instalada.
    pub downgrade: bool,
    /// Dependências obrigatórias da versão escolhida que não estão no pack.
    pub new_dependencies: Vec<NewDependency>,
    /// Itens do pack com que a versão escolhida se declara incompatível.
    pub conflicts: Vec<NewConflict>,
}

/// Resultado de [`apply`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct VersionChanged {
    /// Caminho do `.pw.toml`.
    pub path: String,
    /// Nome do item.
    pub name: String,
    /// Versão de antes, legível.
    pub from: Option<String>,
    /// Versão de agora, legível.
    pub to: String,
    /// ID da versão (ou arquivo) de agora.
    pub to_id: String,
    /// Foi um rebaixamento.
    pub downgrade: bool,
}

/// O item, a fonte dele e o projeto na fonte.
struct Subject<'a> {
    item: InventoryItem,
    source: &'a dyn AddSource,
    project: SourceProject,
    target: PackTarget,
}

fn not_changeable(path: &str) -> Error {
    Error::new(
        Code::InvalidInput,
        format!("{path}: só itens do Modrinth e da CurseForge têm versões para escolher"),
    )
    .param("path", path)
}

fn source_of(item: &InventoryItem) -> Option<SourceId> {
    match item.source {
        ItemSource::Modrinth => Some(SourceId::Modrinth),
        ItemSource::Curseforge => Some(SourceId::Curseforge),
        ItemSource::Url | ItemSource::Local => None,
    }
}

async fn subject<'a>(
    root: &Path,
    scan: &Scan,
    sources: &'a AddSources,
    path: &str,
    cancel: &CancellationToken,
) -> Result<Subject<'a>> {
    let item = scan.require(path)?.clone();
    let source_id = source_of(&item)
        .filter(|_| item.state == ItemState::Ok)
        .ok_or_else(|| not_changeable(path))?;
    let project_id = item
        .project_id
        .clone()
        .ok_or_else(|| not_changeable(path))?;
    let source = sources
        .get(source_id)
        .ok_or_else(|| crate::add::source_off(source_id))?;
    let project = source
        .projects(std::slice::from_ref(&project_id), cancel)
        .await?
        .into_iter()
        .next()
        .ok_or_else(|| {
            Error::new(
                Code::ItemNotFound,
                format!("{path}: o projeto não está mais na {}", source_id.label()),
            )
            .param("path", path)
        })?;
    let target = PackTarget::read(root)?;
    Ok(Subject {
        item,
        source,
        project,
        target,
    })
}

/// A versão instalada, como a fonte a devolve (nenhuma: removida ou sem rede para ela).
async fn installed(subject: &Subject<'_>, cancel: &CancellationToken) -> Option<SourceVersion> {
    let id = subject.item.source_version_id.clone()?;
    subject
        .source
        .versions(&[id], cancel)
        .await
        .ok()?
        .into_iter()
        .next()
}

/// `true` se `candidate` é mais antiga que `installed`: pela data de publicação (RFC 3339 em
/// UTC, até o segundo) e, com a mesma data, pela posição na lista (mais nova primeiro).
fn is_older(candidate: &SourceVersion, installed: &SourceVersion) -> bool {
    let date = |text: &str| text.get(..19).map(str::to_owned);
    match (date(&candidate.published), date(&installed.published)) {
        (Some(candidate), Some(installed)) => candidate < installed,
        _ => false,
    }
}

fn choice(version: &SourceVersion, installed: Option<&SourceVersion>) -> VersionChoice {
    VersionChoice {
        id: version.id.clone(),
        number: version.number.clone(),
        channel: version.channel,
        published: version.published.clone(),
        file_name: version
            .file
            .as_ref()
            .map(|file| file.name.clone())
            .unwrap_or_default(),
        installed: installed.is_some_and(|installed| installed.id == version.id),
        older: installed
            .is_some_and(|installed| installed.id != version.id && is_older(version, installed)),
    }
}

fn readable_current(item: &InventoryItem, installed: Option<&SourceVersion>) -> Option<String> {
    installed
        .map(|version| version.number.clone())
        .or_else(|| item.version.clone())
        .or_else(|| {
            item.file_name
                .as_deref()
                .map(crate::inventory::readable_file_version)
        })
}

/// As versões compatíveis do item, para o diálogo "Trocar de versão". Só lê.
///
/// Erros: [`Code::ItemNotFound`] (caminho fora do inventário ou projeto removido da fonte),
/// [`Code::InvalidInput`] (arquivo local, link direto ou item inválido) e
/// [`Code::SearchSourceUnavailable`] (fonte desligada ou sem resposta).
pub async fn options(
    root: &Path,
    sources: &AddSources,
    path: &str,
    cancel: &CancellationToken,
) -> Result<ChangeVersionOptions> {
    let scan = scan(root)?;
    let subject = subject(root, &scan, sources, path, cancel).await?;
    let kind = subject.project.kind;
    let versions = subject
        .source
        .compatible_versions(&subject.project, &subject.target, cancel)
        .await?;
    let installed = installed(&subject, cancel).await;
    let installed_ref = installed.as_ref().or_else(|| {
        let id = subject.item.source_version_id.as_deref()?;
        versions.iter().find(|version| version.id == id)
    });
    let current_incompatible =
        installed_ref.is_some_and(|version| !version.compatible(kind, &subject.target));
    Ok(ChangeVersionOptions {
        path: subject.item.path.clone(),
        name: subject.item.name.clone(),
        source: subject.project.source,
        current_id: subject.item.source_version_id.clone(),
        current: readable_current(&subject.item, installed_ref),
        current_incompatible,
        pinned: subject.item.pinned,
        versions: versions
            .iter()
            .filter(|version| version.file.is_some())
            .map(|version| choice(version, installed_ref))
            .collect(),
    })
}

/// A versão pedida, que precisa ser do mesmo projeto, ter arquivo e servir para o pack.
async fn wanted(
    subject: &Subject<'_>,
    version_id: &str,
    cancel: &CancellationToken,
) -> Result<SourceVersion> {
    let invalid = || {
        Error::new(
            Code::InvalidInput,
            format!(
                "{}: a versão {version_id} não serve para este pack",
                subject.item.path
            ),
        )
        .param("path", &subject.item.path)
        .param("version", version_id)
    };
    let version = subject
        .source
        .versions(&[version_id.to_owned()], cancel)
        .await?
        .into_iter()
        .next()
        .ok_or_else(invalid)?;
    if version.project_id != subject.project.id
        || version.file.is_none()
        || !version.compatible(subject.project.kind, &subject.target)
    {
        return Err(invalid());
    }
    Ok(version)
}

/// O que muda trocando o item para `version_id`. Só lê.
///
/// Erros: os de [`options`] e [`Code::InvalidInput`] se a versão não é do projeto ou não serve
/// para o pack.
pub async fn plan(
    root: &Path,
    sources: &AddSources,
    path: &str,
    version_id: &str,
    cancel: &CancellationToken,
) -> Result<ChangeVersionPlan> {
    let scan = scan(root)?;
    let subject = subject(root, &scan, sources, path, cancel).await?;
    let version = wanted(&subject, version_id, cancel).await?;
    let installed = installed(&subject, cancel).await;
    let to = choice(&version, installed.as_ref());
    let same = subject.item.source_version_id.as_deref() == Some(version.id.as_str());

    // Itens do pack da mesma fonte, pelo projeto.
    let in_pack: HashMap<&str, &InventoryItem> = scan
        .inventory
        .items
        .iter()
        .filter(|item| item.source == subject.item.source && item.state == ItemState::Ok)
        .filter_map(|item| Some((item.project_id.as_deref()?, item)))
        .collect();
    let mut missing: BTreeSet<String> = BTreeSet::new();
    let mut conflicts: Vec<NewConflict> = Vec::new();
    for dependency in &version.dependencies {
        let Some(project) = dependency.project_id.as_deref() else {
            continue;
        };
        if project == subject.project.id {
            continue;
        }
        match dependency.kind {
            DependencyKind::Required if !in_pack.contains_key(project) => {
                missing.insert(project.to_owned());
            }
            DependencyKind::Incompatible => {
                if let Some(other) = in_pack.get(project) {
                    let conflict = NewConflict {
                        item: subject.item.name.clone(),
                        with: other.name.clone(),
                    };
                    if !conflicts.contains(&conflict) {
                        conflicts.push(conflict);
                    }
                }
            }
            _ => {}
        }
    }
    let ids: Vec<String> = missing.into_iter().collect();
    // Os nomes são só para mostrar: sem rede, fica o ID.
    let names: HashMap<String, String> = if ids.is_empty() {
        HashMap::new()
    } else {
        subject
            .source
            .projects(&ids, cancel)
            .await
            .map(|projects| projects.into_iter().map(|p| (p.id, p.title)).collect())
            .unwrap_or_default()
    };
    let new_dependencies = ids
        .into_iter()
        .map(|id| NewDependency {
            name: names
                .get(&id)
                .filter(|name| !name.trim().is_empty())
                .cloned()
                .unwrap_or_else(|| id.clone()),
            project_id: id,
            source: subject.item.source,
            needed_by: vec![subject.item.name.clone()],
        })
        .collect();
    Ok(ChangeVersionPlan {
        path: subject.item.path.clone(),
        name: subject.item.name.clone(),
        current: readable_current(&subject.item, installed.as_ref()),
        downgrade: to.older,
        to,
        same,
        new_dependencies,
        conflicts,
    })
}

/// Troca o item para `version_id`: o `.pw.toml` passa a apontar para a versão escolhida (lado,
/// opcional, fixar e nome ficam como estavam) e o índice é atualizado numa transação.
///
/// `expected_current_id` é a versão instalada que o diálogo mostrou: se o item mudou depois,
/// nada é gravado ([`Code::PackChangedExternally`]). Escolher a versão instalada não grava nada.
pub async fn apply(
    root: &Path,
    sources: &AddSources,
    path: &str,
    expected_current_id: Option<&str>,
    version_id: &str,
    packwiz: &Packwiz,
    cancel: &CancellationToken,
) -> Result<VersionChanged> {
    let scan = scan(root)?;
    if let Some(error) = &scan.inventory.index_error {
        return Err(Error::new(Code::InvalidPack, error.clone()));
    }
    let subject = subject(root, &scan, sources, path, cancel).await?;
    if subject.item.source_version_id.as_deref() != expected_current_id {
        return Err(Error::new(
            Code::PackChangedExternally,
            format!(
                "{} mudou depois de abrir \"Trocar de versão\"",
                subject.item.path
            ),
        )
        .param("path", &subject.item.path));
    }
    let version = wanted(&subject, version_id, cancel).await?;
    let installed = installed(&subject, cancel).await;
    let from = readable_current(&subject.item, installed.as_ref());
    let downgrade = installed
        .as_ref()
        .is_some_and(|installed| installed.id != version.id && is_older(&version, installed));
    let changed = VersionChanged {
        path: subject.item.path.clone(),
        name: subject.item.name.clone(),
        from,
        to: version.number.clone(),
        to_id: version.id.clone(),
        downgrade,
    };
    if subject.item.source_version_id.as_deref() == Some(version.id.as_str()) {
        return Ok(changed);
    }
    let old = scan
        .metafiles
        .get(&subject.item.path)
        .ok_or_else(|| not_changeable(path))?;
    let mut metafile = subject
        .source
        .metafile(&subject.project, &version, SideChoice::Both)?;
    metafile.name.clone_from(&old.name);
    metafile.side = old.side.clone();
    metafile.pin = old.pin;
    metafile.option.clone_from(&old.option);
    let mut tx = PackTransaction::new(root.to_path_buf());
    tx.write(&subject.item.path, metafile.to_toml_string().into_bytes())?;
    tx.commit(packwiz, cancel).await?;
    Ok(changed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::side::SideChoice;

    fn version(id: &str, published: &str) -> SourceVersion {
        SourceVersion {
            source: SourceId::Modrinth,
            id: id.into(),
            project_id: "p".into(),
            number: id.into(),
            channel: VersionChannel::Release,
            published: published.into(),
            game_versions: vec!["1.20.1".into()],
            loaders: vec!["fabric".into()],
            file: None,
            side: SideChoice::Both,
            side_note: None,
            dependencies: Vec::new(),
        }
    }

    #[test]
    fn mais_antiga_pela_data_ate_o_segundo() {
        let installed = version("b", "2024-05-01T10:00:00.123456Z");
        assert!(is_older(&version("a", "2024-04-30T23:59:59Z"), &installed));
        assert!(!is_older(&version("c", "2024-05-02T00:00:00Z"), &installed));
        // Mesma data até o segundo (frações diferentes entre as fontes): não é rebaixamento.
        assert!(!is_older(
            &version("d", "2024-05-01T10:00:00.9Z"),
            &installed
        ));
        // Sem data, nunca se afirma que é mais antiga.
        assert!(!is_older(&version("e", ""), &installed));
        let choice = choice(&version("a", "2024-01-01T00:00:00Z"), Some(&installed));
        assert!(choice.older && !choice.installed);
        let same = super::choice(&installed, Some(&installed));
        assert!(same.installed && !same.older);
        assert!(!super::choice(&installed, None).older);
    }
}
