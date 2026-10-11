//! Adicionar ao pack: o plano (o que vai entrar, com dependências e conflitos) e a gravação
//! atômica de vários itens (SPEC T08 e T09; ARCHITECTURE §6.1 e §6.5).
//!
//! Contrato usado por várias telas (busca, link, arquivo, kits, mods iniciais, modpacks,
//! trocar mod): **planejar** com [`plan::add_plan`] e **gravar** com [`add_apply`].
//!
//! - O plano nunca grava nada. Ele resolve, para cada item escolhido, a versão (a pedida ou a
//!   mais nova compatível do canal), as dependências obrigatórias em cadeia (as comuns a
//!   vários itens aparecem uma vez), as opcionais, as que já estão no pack, as sem versão
//!   compatível, as incompatibilidades declaradas (com o pack e entre os próprios itens) e os
//!   itens que já estão no pack por outra fonte.
//! - A gravação recebe as versões exatas que a tela confirmou, monta um `.pw.toml` por item
//!   como o packwiz escreveria e grava tudo numa única [`crate::transaction::PackTransaction`]
//!   (um `packwiz refresh`): ou todos entram, ou nenhum.
//!
//! Cada fonte implementa [`AddSource`]: o Modrinth ([`modrinth::ModrinthAdd`]) e a CurseForge
//! ([`curseforge::CurseforgeAdd`]). O link da CurseForge está em [`link_curseforge`].

pub mod curseforge;
pub mod link_curseforge;
pub mod modrinth;
pub mod plan;

use std::collections::{HashMap, HashSet};
use std::path::Path;

use serde::{Deserialize, Serialize};
use warden_core::CancellationToken;
use warden_packwiz::{
    METAFILE_SUFFIX, Metafile, MetafileSource, slugify_name, unique_metafile_stem,
};
use warden_packwiz_cli::Packwiz;

use crate::search::{PackTarget, ProjectKind, SourceId};
use crate::side::SideChoice;
use crate::transaction::PackTransaction;
use crate::{Error, ProjectErrorCode as Code, Result};
use plan::PackState;

/// Um item escolhido pelo usuário.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct AddChoice {
    /// A fonte escolhida.
    pub source: SourceId,
    /// ID do projeto na fonte.
    pub project_id: String,
    /// Versão escolhida no seletor; nenhuma: a mais nova compatível do canal.
    #[serde(default)]
    pub version_id: Option<String>,
}

/// Canal de uma versão.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, specta::Type,
)]
#[serde(rename_all = "camelCase")]
pub enum VersionChannel {
    /// Estável.
    Release,
    /// Beta.
    Beta,
    /// Alpha.
    Alpha,
}

/// Observação sobre o lado sugerido (ARCHITECTURE §6.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum SideNote {
    /// "Funciona em qualquer um dos lados".
    EitherSide,
    /// "Lado desconhecido — confira se é só de cliente".
    Unknown,
}

/// Quais canais entram por padrão (Configurações: "Mostrar versões beta e alpha").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ChannelPolicy {
    /// Beta e alpha também contam como "mais nova".
    pub allow_prerelease: bool,
}

impl ChannelPolicy {
    /// Se o canal entra por padrão.
    #[must_use]
    pub fn allows(self, channel: VersionChannel) -> bool {
        self.allow_prerelease || channel == VersionChannel::Release
    }

    /// A versão padrão de uma lista (mais nova primeiro): a mais nova do canal; sem nenhuma
    /// do canal, a mais nova de qualquer canal (marcada `outside_channel` no plano).
    #[must_use]
    pub fn pick(self, versions: &[SourceVersion]) -> Option<&SourceVersion> {
        versions
            .iter()
            .find(|version| self.allows(version.channel))
            .or_else(|| versions.first())
    }
}

/// Um projeto como a fonte informa.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceProject {
    /// A fonte.
    pub source: SourceId,
    /// ID na fonte.
    pub id: String,
    /// Slug (nome do `.pw.toml`).
    pub slug: String,
    /// Nome.
    pub title: String,
    /// Tipo (nenhum: um tipo que não entra pela página Adicionar, como modpack ou datapack).
    pub kind: Option<ProjectKind>,
    /// Ícone.
    pub icon_url: Option<String>,
    /// Lado informado pelo projeto (reserva para versões sem esse dado).
    pub side: Option<(SideChoice, Option<SideNote>)>,
}

/// Arquivo de uma versão.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceFile {
    /// Nome do arquivo.
    pub name: String,
    /// Link (vazio na CurseForge, que baixa pela API).
    pub url: String,
    /// SHA-1 (comparação entre fontes, ADR-0027).
    pub sha1: Option<String>,
    /// SHA-512 (o hash que o Warden grava para o Modrinth).
    pub sha512: Option<String>,
}

/// Tipo de uma dependência declarada.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum DependencyKind {
    /// Obrigatória.
    Required,
    /// Opcional.
    Optional,
    /// Incompatível.
    Incompatible,
}

/// Uma dependência declarada por uma versão.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceDependency {
    /// Projeto (pode faltar quando só a versão é informada).
    pub project_id: Option<String>,
    /// Versão exata pedida, se houver.
    pub version_id: Option<String>,
    /// Tipo.
    pub kind: DependencyKind,
}

/// Uma versão como a fonte informa.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceVersion {
    /// A fonte.
    pub source: SourceId,
    /// ID da versão (Modrinth) ou do arquivo (CurseForge).
    pub id: String,
    /// Projeto.
    pub project_id: String,
    /// Número legível ("0.6.0").
    pub number: String,
    /// Canal.
    pub channel: VersionChannel,
    /// Data de publicação (RFC 3339).
    pub published: String,
    /// Versões do Minecraft.
    pub game_versions: Vec<String>,
    /// Loaders.
    pub loaders: Vec<String>,
    /// Arquivo principal (nenhum: a versão não tem arquivo utilizável).
    pub file: Option<SourceFile>,
    /// Lado sugerido.
    pub side: SideChoice,
    /// Observação sobre o lado.
    pub side_note: Option<SideNote>,
    /// Dependências declaradas.
    pub dependencies: Vec<SourceDependency>,
}

impl SourceVersion {
    /// Se a versão serve para o pack (versão do Minecraft e, para mods, loader).
    #[must_use]
    pub fn compatible(&self, kind: Option<ProjectKind>, target: &PackTarget) -> bool {
        let game = self
            .game_versions
            .iter()
            .any(|version| target.game_versions.contains(version));
        let loaders = kind
            .map(|kind| target.loader_names(kind))
            .unwrap_or_default();
        let loader = loaders.is_empty()
            || self
                .loaders
                .iter()
                .any(|loader| loaders.contains(&loader.to_ascii_lowercase()));
        game && loader
    }
}

/// O que uma fonte precisa saber fazer para o plano e a gravação. Implementada pelo Modrinth
/// (P1-09) e pela CurseForge (P1-10).
#[async_trait::async_trait]
pub trait AddSource: Send + Sync {
    /// Qual fonte é.
    fn id(&self) -> SourceId;

    /// Projetos pelos IDs, em lote. Os que não existem ficam de fora.
    async fn projects(
        &self,
        ids: &[String],
        cancel: &CancellationToken,
    ) -> Result<Vec<SourceProject>>;

    /// Versões do projeto que servem para o pack, de todos os canais, a mais nova primeiro.
    async fn compatible_versions(
        &self,
        project: &SourceProject,
        target: &PackTarget,
        cancel: &CancellationToken,
    ) -> Result<Vec<SourceVersion>>;

    /// Versões pelos IDs, em lote. As que não existem ficam de fora.
    async fn versions(
        &self,
        ids: &[String],
        cancel: &CancellationToken,
    ) -> Result<Vec<SourceVersion>>;

    /// Projetos desta fonte pelos SHA-1 de arquivos (achar no pack o mesmo mod vindo de
    /// outra fonte): SHA-1 → ID do projeto. Os desconhecidos ficam de fora.
    async fn projects_by_sha1(
        &self,
        hashes: &[String],
        cancel: &CancellationToken,
    ) -> Result<HashMap<String, String>>;

    /// O `.pw.toml` desta versão, como o packwiz escreveria, com o lado dado.
    fn metafile(
        &self,
        project: &SourceProject,
        version: &SourceVersion,
        side: SideChoice,
    ) -> Result<Metafile>;
}

/// As fontes ligadas no app, pela fonte.
#[derive(Clone, Default)]
pub struct AddSources {
    sources: Vec<std::sync::Arc<dyn AddSource>>,
}

impl AddSources {
    /// Sem fontes.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Acrescenta uma fonte.
    #[must_use]
    pub fn with(mut self, source: std::sync::Arc<dyn AddSource>) -> Self {
        self.sources.push(source);
        self
    }

    /// A fonte, se estiver ligada.
    #[must_use]
    pub fn get(&self, id: SourceId) -> Option<&dyn AddSource> {
        self.sources
            .iter()
            .find(|source| source.id() == id)
            .map(AsRef::as_ref)
    }

    /// Todas.
    pub fn all(&self) -> impl Iterator<Item = &dyn AddSource> {
        self.sources.iter().map(AsRef::as_ref)
    }
}

/// Erro de fonte não ligada (sem chave da CurseForge, por exemplo).
pub(crate) fn source_off(source: SourceId) -> crate::Error {
    crate::Error::new(
        crate::ProjectErrorCode::SearchSourceUnavailable,
        format!("fonte {} não está ligada", source.label()),
    )
    .param("source", source.label())
}

/// Um item confirmado na tela de dependências, com a versão exata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ApplyItem {
    /// Fonte.
    pub source: SourceId,
    /// ID do projeto.
    pub project_id: String,
    /// ID da versão (Modrinth) ou do arquivo (CurseForge), como veio no plano.
    pub version_id: String,
    /// Lado escolhido; nenhum: o sugerido pela fonte.
    #[serde(default)]
    pub side: Option<SideChoice>,
    /// Caminho do item do pack que este substitui ("Substituir pela versão do Modrinth").
    #[serde(default)]
    pub replaces: Option<String>,
}

/// O que gravar.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct AddApplyRequest {
    /// Os itens, na ordem da tela.
    pub items: Vec<ApplyItem>,
}

/// Um item gravado.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct AddedItem {
    /// Chave estável (`modrinth:<projeto>`).
    pub key: String,
    /// Nome.
    pub title: String,
    /// Caminho do `.pw.toml` criado.
    pub path: String,
}

/// Resultado da gravação.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct AddResult {
    /// Itens que entraram.
    pub added: Vec<AddedItem>,
    /// Chaves dos itens que já estavam no pack pela mesma fonte (nada foi feito com eles).
    pub skipped: Vec<String>,
    /// Itens substituídos (apagados).
    pub removed: Vec<String>,
}

fn metafile_source(source: SourceId) -> MetafileSource {
    match source {
        SourceId::Modrinth => MetafileSource::Modrinth,
        SourceId::Curseforge => MetafileSource::CurseForge,
    }
}

/// Nomes base dos `.pw.toml` que já existem nas pastas de conteúdo, mesmo fora do índice
/// (um arquivo novo nunca pode sobrescrever outro).
fn stems_on_disk(root: &Path) -> Vec<String> {
    let mut stems = Vec::new();
    for kind in [
        ProjectKind::Mod,
        ProjectKind::ResourcePack,
        ProjectKind::Shader,
    ] {
        let Ok(entries) = std::fs::read_dir(root.join(kind.folder())) else {
            continue;
        };
        for entry in entries.filter_map(std::result::Result::ok) {
            if let Some(stem) = entry
                .file_name()
                .to_str()
                .and_then(|name| name.strip_suffix(METAFILE_SUFFIX))
            {
                stems.push(stem.to_owned());
            }
        }
    }
    stems
}

/// Projetos e versões confirmados, em lote por fonte: chave do projeto → projeto e chave da
/// versão (`modrinth:<versão>`) → versão.
async fn fetch_chosen(
    sources: &AddSources,
    wanted: &[&ApplyItem],
    cancel: &CancellationToken,
) -> Result<(
    HashMap<String, SourceProject>,
    HashMap<String, SourceVersion>,
)> {
    let mut projects = HashMap::new();
    let mut versions = HashMap::new();
    for source_id in [SourceId::Modrinth, SourceId::Curseforge] {
        let of_source: Vec<&&ApplyItem> = wanted.iter().filter(|i| i.source == source_id).collect();
        if of_source.is_empty() {
            continue;
        }
        let source = sources
            .get(source_id)
            .ok_or_else(|| source_off(source_id))?;
        let ids: Vec<String> = of_source.iter().map(|i| i.project_id.clone()).collect();
        for project in source.projects(&ids, cancel).await? {
            projects.insert(source_id.key(&project.id), project);
        }
        let ids: Vec<String> = of_source.iter().map(|i| i.version_id.clone()).collect();
        for version in source.versions(&ids, cancel).await? {
            versions.insert(source_id.key(&version.id), version);
        }
    }
    Ok((projects, versions))
}

/// Planeja apagar o item do pack que um item novo substitui (duplicado entre fontes).
fn delete_replaced(pack: &PackState, tx: &mut PackTransaction, old: &str) -> Result<String> {
    let old = old.replace('\\', "/");
    if !pack.items.items().iter().any(|known| known.path == old) {
        return Err(
            Error::new(Code::InvalidInput, format!("{old} não é um item do pack"))
                .param("path", &old),
        );
    }
    tx.delete(&old)?;
    Ok(old)
}

/// Grava os itens confirmados: um `.pw.toml` por item, como o packwiz escreveria, e os
/// substituídos apagados, numa única transação com um só `packwiz refresh`. Ou todos entram,
/// ou nenhum (CA-T08-11, CA-T09-03).
///
/// Itens que já estão no pack pela mesma fonte são pulados (vão em `skipped`).
///
/// Erros: [`Code::InvalidInput`] (sem itens, versão que não é do projeto, item a substituir
/// que não está no pack); [`Code::SearchSourceUnavailable`] se a fonte não responder (nada é
/// gravado); [`Code::PackChangedExternally`] se o pack mudou durante a operação; os da
/// transação (refresh com falha: tudo volta como estava).
pub async fn add_apply(
    root: &Path,
    sources: &AddSources,
    request: &AddApplyRequest,
    packwiz: &Packwiz,
    cancel: &CancellationToken,
) -> Result<AddResult> {
    if request.items.is_empty() {
        return Err(
            Error::new(Code::InvalidInput, "nenhum item para gravar").param("field", "items")
        );
    }
    let pack = PackState::read(root)?;
    let mut seen = HashSet::new();
    let mut skipped = Vec::new();
    let mut wanted = Vec::new();
    for item in &request.items {
        let key = item.source.key(&item.project_id);
        if !seen.insert(key.clone()) {
            continue;
        }
        if pack
            .items
            .same_source(item.source, &item.project_id)
            .is_some()
        {
            skipped.push(key);
        } else {
            wanted.push(item);
        }
    }
    let (projects, versions) = fetch_chosen(sources, &wanted, cancel).await?;
    let mut taken: Vec<String> = pack.stems.clone();
    taken.extend(stems_on_disk(root));
    let mut tx = PackTransaction::new(root.to_path_buf());
    let mut added = Vec::new();
    let mut removed = Vec::new();
    for item in wanted {
        let key = item.source.key(&item.project_id);
        let project = projects.get(&key).ok_or_else(|| {
            Error::new(Code::InvalidInput, format!("projeto {key} não encontrado"))
                .param("field", "projectId")
        })?;
        let version = versions
            .get(&item.source.key(&item.version_id))
            .filter(|version| version.project_id == project.id)
            .ok_or_else(|| {
                Error::new(
                    Code::InvalidInput,
                    format!("a versão {} não é de {}", item.version_id, project.title),
                )
                .param("field", "versionId")
            })?;
        let kind = project.kind.ok_or_else(|| {
            Error::new(
                Code::InvalidInput,
                format!("{} não é mod, resource pack nem shader", project.title),
            )
            .param("field", "kind")
        })?;
        let source = sources
            .get(item.source)
            .ok_or_else(|| source_off(item.source))?;
        let side = item
            .side
            .unwrap_or(match (version.side_note, project.side) {
                (Some(SideNote::Unknown), Some((side, _))) => side,
                _ => version.side,
            });
        let metafile = source.metafile(project, version, side)?;
        let slug = if project.slug.trim().is_empty() {
            slugify_name(&project.title)
        } else {
            project.slug.trim().to_lowercase()
        };
        let stem = unique_metafile_stem(
            &slug,
            metafile_source(item.source),
            taken.iter().map(String::as_str),
        );
        taken.push(stem.clone());
        let path = format!("{}/{stem}{METAFILE_SUFFIX}", kind.folder());
        tx.write(&path, metafile.to_toml_string().into_bytes())?;
        if let Some(old) = &item.replaces {
            removed.push(delete_replaced(&pack, &mut tx, old)?);
        }
        added.push(AddedItem {
            key,
            title: project.title.clone(),
            path,
        });
    }
    if !added.is_empty() {
        tx.commit(packwiz, cancel).await?;
    }
    Ok(AddResult {
        added,
        skipped,
        removed,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn version(id: &str, channel: VersionChannel) -> SourceVersion {
        SourceVersion {
            source: SourceId::Modrinth,
            id: id.into(),
            project_id: "p".into(),
            number: id.into(),
            channel,
            published: String::new(),
            game_versions: vec!["1.20.1".into()],
            loaders: vec!["Forge".into()],
            file: None,
            side: SideChoice::Both,
            side_note: None,
            dependencies: Vec::new(),
        }
    }

    #[test]
    fn versao_padrao_respeita_o_canal() {
        let versions = [
            version("3-beta", VersionChannel::Beta),
            version("2", VersionChannel::Release),
        ];
        assert_eq!(ChannelPolicy::default().pick(&versions).unwrap().id, "2");
        let prerelease = ChannelPolicy {
            allow_prerelease: true,
        };
        assert_eq!(prerelease.pick(&versions).unwrap().id, "3-beta");
        // Sem nenhuma estável, a mais nova de qualquer canal.
        assert_eq!(
            ChannelPolicy::default().pick(&versions[..1]).unwrap().id,
            "3-beta"
        );
        assert!(ChannelPolicy::default().pick(&[]).is_none());
    }

    #[test]
    fn compatibilidade_pela_versao_e_pelo_loader() {
        let target = PackTarget {
            minecraft: "1.20.1".into(),
            game_versions: vec!["1.20.1".into()],
            loaders: vec![
                warden_packwiz::Loader::NeoForge,
                warden_packwiz::Loader::Forge,
            ],
        };
        let forge = version("1", VersionChannel::Release);
        assert!(forge.compatible(Some(ProjectKind::Mod), &target));
        let mut fabric = version("2", VersionChannel::Release);
        fabric.loaders = vec!["fabric".into()];
        assert!(!fabric.compatible(Some(ProjectKind::Mod), &target));
        // Resource pack não depende do loader.
        assert!(fabric.compatible(Some(ProjectKind::ResourcePack), &target));
        let mut old = version("3", VersionChannel::Release);
        old.game_versions = vec!["1.19.2".into()];
        assert!(!old.compatible(Some(ProjectKind::Mod), &target));
    }
}
