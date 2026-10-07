//! O plano de "Adicionar" (SPEC T09): tudo o que a tela de dependências mostra, sem gravar
//! nada.
//!
//! O plano é um grafo pequeno. Cada [`PlanNode`] é um item que **pode** entrar, com o papel
//! dele ([`NodeRole`]) e quem o pede (`required_by`, `optional_for`). A tela decide o que fica
//! marcado (regra em `features/add/dependencies/model.ts`):
//!
//! - **escolhido:** entra se continuar marcado;
//! - **obrigatória:** entra quando algum item que a pede entra, salvo se o usuário desmarcar
//!   ("Sem esta dependência o jogo provavelmente não abre.");
//! - **opcional:** só entra se o usuário marcar.
//!
//! Ao lado dos nós: dependências que já estão no pack ([`PlanInstalled`]), sem versão
//! compatível ([`PlanMissing`]), incompatibilidades declaradas ([`PlanConflict`]) e itens que já
//! estão no pack por outra fonte ([`PlanDuplicate`], com a opção de substituir).

use std::path::Path;

use serde::{Deserialize, Serialize};
use warden_core::CancellationToken;

use super::{AddChoice, AddSources, ChannelPolicy, SideNote, VersionChannel};
pub use crate::dedup::DuplicateMatch;
use crate::dedup::{PackItem, PackItems};
use crate::inventory::{ItemState, scan};
use crate::search::{PackTarget, ProjectKind, SourceId};
use crate::side::SideChoice;
use crate::{Error, ProjectErrorCode as Code, Result};

/// Pedido de plano: um ou vários itens (seleção múltipla, kit, mods de um modpack…).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct AddPlanRequest {
    /// Os itens escolhidos, na ordem da tela.
    pub items: Vec<AddChoice>,
}

/// Papel de um item no plano.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum NodeRole {
    /// Escolhido pelo usuário.
    Chosen,
    /// Dependência obrigatória de algum item do plano.
    Required,
    /// Dependência opcional de um item escolhido.
    Optional,
}

/// Um item que pode entrar no pack.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct PlanNode {
    /// Chave estável (`modrinth:<projeto>`).
    pub key: String,
    /// Papel.
    pub role: NodeRole,
    /// Fonte.
    pub source: SourceId,
    /// ID do projeto.
    pub project_id: String,
    /// Slug.
    pub slug: String,
    /// Nome.
    pub title: String,
    /// Ícone.
    pub icon_url: Option<String>,
    /// Tipo (pasta do pack).
    pub kind: ProjectKind,
    /// Versão resolvida (ID na fonte; vai para o `add_apply`).
    pub version_id: String,
    /// Versão legível.
    pub version_number: String,
    /// Canal da versão.
    pub channel: VersionChannel,
    /// A versão não é do canal configurado (não havia nenhuma do canal).
    pub outside_channel: bool,
    /// A versão serve para o Minecraft e o loader do pack. Falso só quando o usuário escolheu
    /// uma versão incompatível (ela entra e aparece no diagnóstico).
    pub compatible: bool,
    /// Nome do arquivo.
    pub file_name: String,
    /// Lado sugerido.
    pub side: SideChoice,
    /// Observação sobre o lado.
    pub side_note: Option<SideNote>,
    /// Chaves dos itens do plano que exigem este.
    pub required_by: Vec<String>,
    /// Chaves dos itens escolhidos para os quais este é opcional.
    pub optional_for: Vec<String>,
}

/// Uma dependência (ou um item escolhido) que já está no pack.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct PlanInstalled {
    /// Chave do projeto pedido.
    pub key: String,
    /// Nome no pack.
    pub title: String,
    /// Caminho do item no pack.
    pub path: String,
    /// Fonte do item no pack (nenhuma: link direto). Diferente da pedida quando o mesmo mod
    /// está no pack por outra fonte.
    pub pack_source: Option<SourceId>,
    /// Era um dos itens escolhidos (ele não entra de novo).
    pub chosen: bool,
    /// Chaves dos itens do plano que pedem este.
    pub required_by: Vec<String>,
}

/// Um item sem versão para o pack ("Nenhuma versão de X para 1.20.1 Forge.").
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct PlanMissing {
    /// Chave do projeto.
    pub key: String,
    /// Nome (o ID quando a fonte não informou).
    pub title: String,
    /// Papel que ele teria.
    pub role: NodeRole,
    /// Quem o pede.
    pub required_by: Vec<String>,
}

/// Um lado de uma incompatibilidade.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ConflictParty {
    /// Chave do projeto.
    pub key: String,
    /// Nome.
    pub title: String,
    /// Caminho no pack, quando já está nele.
    pub path: Option<String>,
}

/// Incompatibilidade declarada (Modrinth `incompatible`, CurseForge "incompatível").
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct PlanConflict {
    /// O item do plano envolvido.
    pub item: ConflictParty,
    /// O outro: um item do pack ou outro item do plano.
    pub other: ConflictParty,
    /// O outro já está no pack (senão, é entre itens escolhidos agora).
    pub with_pack: bool,
    /// Chave de quem declarou a incompatibilidade.
    pub declared_by: String,
    /// Motivo, quando o autor informa.
    pub reason: Option<String>,
}

/// Um item do plano que já está no pack por outra fonte ou por link ("Este mod já está no
/// pack pela CurseForge.").
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct PlanDuplicate {
    /// Chave do item do plano.
    pub key: String,
    /// Caminho do item que já está no pack (vai no `replaces` do `add_apply`).
    pub existing_path: String,
    /// Nome no pack.
    pub existing_title: String,
    /// Fonte do item do pack (nenhuma: link direto).
    pub existing_source: Option<SourceId>,
    /// Como foi achado.
    pub matched_by: DuplicateMatch,
}

/// Versão do Minecraft e loader do pack, para as frases ("para 1.20.1 Forge").
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct PlanTarget {
    /// Versão do Minecraft.
    pub minecraft: String,
    /// Loader do pack (`fabric`, `forge`…), se houver.
    pub loader: Option<String>,
}

/// O plano.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct AddPlan {
    /// Para qual pack.
    pub target: PlanTarget,
    /// Itens que podem entrar: os escolhidos primeiro, na ordem pedida; depois as dependências.
    pub nodes: Vec<PlanNode>,
    /// Já no pack.
    pub installed: Vec<PlanInstalled>,
    /// Sem versão compatível.
    pub missing: Vec<PlanMissing>,
    /// Incompatibilidades declaradas.
    pub conflicts: Vec<PlanConflict>,
    /// Já no pack por outra fonte.
    pub duplicates: Vec<PlanDuplicate>,
    /// Quantos itens o pack tem ("com os 128 itens que já estão no pack").
    pub pack_item_count: u32,
}

/// O pack lido para planejar: o que ele aceita e o que já tem.
#[derive(Debug, Clone)]
pub struct PackState {
    /// O que o pack aceita.
    pub target: PackTarget,
    /// Itens que vêm de uma fonte ou de um link.
    pub items: PackItems,
    /// Quantos itens o pack tem (todos os do inventário).
    pub item_count: u32,
    /// Nomes base de todos os metafiles do índice (nomes únicos, ARCHITECTURE §6.2).
    pub stems: Vec<String>,
}

impl PackState {
    /// Lê o pack do disco. Erro [`Code::InvalidPack`] com o índice ilegível.
    pub fn read(root: &Path) -> Result<Self> {
        let target = PackTarget::read(root)?;
        let scan = scan(root)?;
        if let Some(error) = &scan.inventory.index_error {
            return Err(Error::new(Code::InvalidPack, error.clone()));
        }
        let mut items = Vec::new();
        let mut stems = Vec::new();
        for item in &scan.inventory.items {
            if let Some(stem) = warden_packwiz::metafile_stem(&item.path) {
                stems.push(stem.to_owned());
            }
            if item.state != ItemState::Ok {
                continue;
            }
            if let Some(metafile) = scan.metafiles.get(&item.path) {
                items.push(PackItem::from_metafile(&item.path, metafile));
            }
        }
        Ok(Self {
            target,
            items: PackItems::new(items),
            item_count: u32::try_from(scan.inventory.items.len()).unwrap_or(u32::MAX),
            stems,
        })
    }

    /// Chaves dos projetos do pack ("Já no pack" na busca).
    #[must_use]
    pub fn installed_keys(&self) -> crate::search::InstalledKeys {
        self.items.keys()
    }
}

/// Monta o plano para os itens escolhidos. Nada é gravado.
///
/// Erros: [`Code::InvalidInput`] sem itens; [`Code::InvalidPack`] com o pack ilegível;
/// [`Code::SearchSourceUnavailable`] (parâmetro `source`) se a fonte não responder.
pub async fn add_plan(
    root: &Path,
    sources: &AddSources,
    policy: ChannelPolicy,
    request: &AddPlanRequest,
    cancel: &CancellationToken,
) -> Result<AddPlan> {
    if request.items.is_empty() {
        return Err(Error::new(Code::InvalidInput, "nenhum item escolhido").param("field", "items"));
    }
    let mut pack = PackState::read(root)?;
    crate::deps::find_equivalents(sources, &mut pack.items, cancel).await?;
    crate::deps::resolve(sources, &pack, policy, &request.items, cancel).await
}
