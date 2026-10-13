//! Remover itens do pack, avisando quem depende deles (SPEC T06; CA-T06-04).
//!
//! O Warden apaga o `.pw.toml` (ou o arquivo local) e o `packwiz refresh` tira a entrada do
//! índice; não usa `packwiz remove` (R4 §2.3).

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::Path;

use serde::Serialize;
use warden_core::CancellationToken;
use warden_modrinth::{DependencyType, ModrinthClient};
use warden_packwiz_cli::Packwiz;

use crate::inventory::{Inventory, InventoryItem, ItemSource, ItemState, modrinth_versions, scan};
use crate::orphans::OrphanOffer;
use crate::transaction::PackTransaction;
use crate::{Error, ProjectErrorCode as Code, Result};

/// Um item que será removido.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct RemovalTarget {
    /// Chave estável do item.
    pub key: String,
    /// Caminho do item relativo à pasta do pack.
    pub path: String,
    /// Nome legível.
    pub name: String,
    /// Arquivos que serão apagados (o `.pw.toml`, ou o arquivo local).
    pub files: Vec<String>,
}

/// Um item que fica no pack mas deixa de funcionar sem os removidos: depende de algum deles,
/// direta ou indiretamente (quem depende de quem depende dele).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct Dependent {
    /// Chave estável do item.
    pub key: String,
    /// Caminho do item no pack.
    pub path: String,
    /// Nome legível.
    pub name: String,
    /// Nomes dos itens que somem (removidos ou dependentes de nível menor) de que ele precisa.
    pub needs: Vec<String>,
    /// 1 = depende direto de um removido; 2 = depende de um item de nível 1; e assim por diante.
    pub depth: u32,
}

/// O que a confirmação de remover mostra.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct RemovalPlan {
    /// Itens removidos, na ordem do inventário.
    pub targets: Vec<RemovalTarget>,
    /// Itens que deixam de funcionar, da cadeia inteira: primeiro os diretos, depois os de
    /// nível 2 e assim por diante (cada nível em ordem de nome).
    pub dependents: Vec<Dependent>,
    /// Dependências que ficam sem uso e podem sair junto (P1).
    pub orphans: Vec<OrphanOffer>,
}

/// Um item que deixa de funcionar se os alvos saírem, como o grafo da D-07 responde
/// (`warden_diagnostics::graph::Affected`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AffectedItem {
    /// Caminho do item.
    pub path: String,
    /// Nível na cadeia (1 = direto).
    pub depth: u32,
    /// Nomes dos itens que somem de que ele precisa.
    pub needs: Vec<String>,
}

/// Dependências obrigatórias diretas: chave do item → chaves dos itens de que ele precisa.
pub type DependencyMap = BTreeMap<String, Vec<String>>;

/// Dependências obrigatórias diretas entre os itens do pack, pelos metadados do Modrinth
/// (versão instalada; cache primeiro, rede só para o que falta). Só entram as dependências que
/// estão no pack. Item sem metadados fica sem entrada; nunca falha.
pub async fn direct_dependencies(
    inventory: &Inventory,
    modrinth: Option<&ModrinthClient>,
) -> DependencyMap {
    let mut map = DependencyMap::new();
    let Some(client) = modrinth else {
        return map;
    };
    let modrinth_items: Vec<&InventoryItem> = inventory
        .items
        .iter()
        .filter(|item| item.source == ItemSource::Modrinth && item.state == ItemState::Ok)
        .collect();
    let by_project: HashMap<&str, &str> = modrinth_items
        .iter()
        .filter_map(|item| Some((item.project_id.as_deref()?, item.key.as_str())))
        .collect();
    let by_version: HashMap<&str, &str> = modrinth_items
        .iter()
        .filter_map(|item| Some((item.source_version_id.as_deref()?, item.key.as_str())))
        .collect();
    let ids: Vec<String> = by_version.keys().map(|id| (*id).to_owned()).collect();
    let versions = modrinth_versions(client, &ids).await;
    for (version_id, key) in &by_version {
        let Some(version) = versions.get(*version_id) else {
            continue;
        };
        let mut needs: Vec<String> = Vec::new();
        for dependency in version.dependencies_of(DependencyType::Required) {
            let target = dependency
                .project_id
                .as_deref()
                .and_then(|project| by_project.get(project))
                .or_else(|| {
                    dependency
                        .version_id
                        .as_deref()
                        .and_then(|id| by_version.get(id))
                });
            if let Some(target) = target
                && target != key
                && !needs.iter().any(|need| need == target)
            {
                needs.push((*target).to_owned());
            }
        }
        map.insert((*key).to_owned(), needs);
    }
    map
}

/// Monta a confirmação: os alvos, o que deixa de funcionar (a cadeia inteira) e as
/// dependências que ficam sem uso.
///
/// `affected` é o fecho transitivo das dependências do grafo da D-07 (`Graph::dependents`, que
/// considera `provides` e jar-in-jar; CA-T06-06) e `orphans`, o resultado de
/// [`crate::orphans::orphans_after_removal`]. Afetado que não está no inventário ou que também
/// é alvo fica de fora.
///
/// Erro [`Code::ItemNotFound`] (parâmetro `path`) se um caminho não está no inventário.
pub fn removal_plan(
    inventory: &Inventory,
    paths: &[String],
    affected: &[AffectedItem],
    orphans: Vec<OrphanOffer>,
) -> Result<RemovalPlan> {
    let mut wanted = BTreeSet::new();
    for path in paths {
        let item = find(inventory, path)?;
        wanted.insert(item.path.clone());
    }
    let targets: Vec<&InventoryItem> = inventory
        .items
        .iter()
        .filter(|item| wanted.contains(&item.path))
        .collect();
    let dependents = affected
        .iter()
        .filter(|affected| !wanted.contains(&affected.path))
        .filter_map(|affected| {
            let item = inventory
                .items
                .iter()
                .find(|item| item.path == affected.path)?;
            Some(Dependent {
                key: item.key.clone(),
                path: item.path.clone(),
                name: item.name.clone(),
                needs: affected.needs.clone(),
                depth: affected.depth,
            })
        })
        .collect();
    let orphans = orphans
        .into_iter()
        .filter(|orphan| !wanted.contains(&orphan.path))
        .collect();
    Ok(RemovalPlan {
        targets: targets
            .into_iter()
            .map(|item| RemovalTarget {
                key: item.key.clone(),
                path: item.path.clone(),
                name: item.name.clone(),
                files: vec![item.path.clone()],
            })
            .collect(),
        dependents,
        orphans,
    })
}

/// Apaga os itens (o `.pw.toml`, o arquivo local indexado ou o arquivo fora do índice) numa
/// única transação, e o `packwiz refresh` atualiza o índice. Devolve os arquivos apagados.
///
/// Erro [`Code::ItemNotFound`] (parâmetro `path`) se um caminho não está no inventário.
pub async fn remove(
    root: &Path,
    paths: &[String],
    packwiz: &Packwiz,
    cancel: &CancellationToken,
) -> Result<Vec<String>> {
    let scan = scan(root)?;
    if let Some(error) = &scan.inventory.index_error {
        return Err(Error::new(Code::InvalidPack, error.clone()));
    }
    let mut files = BTreeSet::new();
    for path in paths {
        files.insert(scan.require(path)?.path.clone());
    }
    if files.is_empty() {
        return Ok(Vec::new());
    }
    let mut tx = PackTransaction::new(root.to_path_buf());
    for file in &files {
        tx.delete(file)?;
    }
    tx.commit(packwiz, cancel).await
}

fn find<'a>(inventory: &'a Inventory, path: &str) -> Result<&'a InventoryItem> {
    let wanted = warden_packwiz::clean_path(&path.replace('\\', "/"));
    inventory
        .items
        .iter()
        .find(|item| item.path == wanted)
        .ok_or_else(|| crate::inventory::item_not_found(path))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inventory::{ItemKind, ItemSide};

    fn item(key: &str, name: &str) -> InventoryItem {
        InventoryItem {
            key: key.into(),
            path: format!("mods/{name}.pw.toml"),
            kind: ItemKind::Mod,
            state: ItemState::Ok,
            name: name.into(),
            file_name: Some(format!("{name}.jar")),
            version: None,
            source: ItemSource::Modrinth,
            side: ItemSide::Both,
            side_editable: true,
            pinned: false,
            optional: false,
            project_id: None,
            source_version_id: None,
            summary: None,
            icon_url: None,
            error: None,
        }
    }

    fn affected(path: &str, depth: u32, needs: &[&str]) -> AffectedItem {
        AffectedItem {
            path: path.into(),
            depth,
            needs: needs.iter().map(|n| (*n).to_owned()).collect(),
        }
    }

    #[test]
    fn plano_lista_a_cadeia_inteira_fora_dos_alvos() {
        let inventory = Inventory {
            items: vec![
                item("modrinth:a", "A"),
                item("modrinth:b", "B"),
                item("modrinth:c", "C"),
                item("modrinth:d", "D"),
            ],
            index_error: None,
        };
        // A → B → C e D → C: remover C quebra B e D (nível 1) e A (nível 2), na ordem do grafo.
        let chain = [
            affected("mods/B.pw.toml", 1, &["C"]),
            affected("mods/D.pw.toml", 1, &["C"]),
            affected("mods/A.pw.toml", 2, &["B"]),
            affected("mods/fora.pw.toml", 2, &["C"]),
        ];
        let plan =
            removal_plan(&inventory, &["mods/C.pw.toml".into()], &chain, Vec::new()).unwrap();
        assert_eq!(plan.targets.len(), 1);
        assert_eq!(plan.targets[0].files, ["mods/C.pw.toml"]);
        let names: Vec<(&str, u32, &[String])> = plan
            .dependents
            .iter()
            .map(|d| (d.name.as_str(), d.depth, d.needs.as_slice()))
            .collect();
        assert_eq!(
            names,
            [
                ("B", 1, &["C".to_owned()][..]),
                ("D", 1, &["C".to_owned()][..]),
                ("A", 2, &["B".to_owned()][..]),
            ]
        );
        assert_eq!(plan.dependents[2].key, "modrinth:a");
        // Removendo B e C juntos, B sai da lista de dependentes (é alvo).
        let plan = removal_plan(
            &inventory,
            &["mods/B.pw.toml".into(), "mods/C.pw.toml".into()],
            &chain,
            vec![OrphanOffer {
                key: "modrinth:b".into(),
                path: "mods/B.pw.toml".into(),
                name: "B".into(),
                optional_users: Vec::new(),
            }],
        )
        .unwrap();
        let names: Vec<&str> = plan.dependents.iter().map(|d| d.name.as_str()).collect();
        assert_eq!(names, ["D", "A"]);
        assert!(
            plan.orphans.is_empty(),
            "um alvo nunca é oferecido como órfão"
        );
        let error =
            removal_plan(&inventory, &["mods/X.pw.toml".into()], &chain, Vec::new()).unwrap_err();
        assert_eq!(error.code, Code::ItemNotFound);
        assert_eq!(error.params["path"], "mods/X.pw.toml");
    }
}
