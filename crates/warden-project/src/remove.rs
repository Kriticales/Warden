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

/// Um item que fica no pack mas depende de algum dos removidos.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct Dependent {
    /// Chave estável do item.
    pub key: String,
    /// Nome legível.
    pub name: String,
    /// Nomes dos itens removidos de que ele precisa.
    pub needs: Vec<String>,
}

/// O que a confirmação de remover mostra.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct RemovalPlan {
    /// Itens removidos, na ordem do inventário.
    pub targets: Vec<RemovalTarget>,
    /// Itens que dependem diretamente de algum removido, na ordem do inventário.
    pub dependents: Vec<Dependent>,
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

/// Monta a confirmação: os alvos e os dependentes **diretos** que ficam no pack.
///
/// A P1-15 troca os dependentes diretos pela consulta transitiva do modelo de dependências da
/// D-07 (cadeia inteira, `provides` e jar-in-jar; CA-T06-06).
pub fn removal_plan(
    inventory: &Inventory,
    paths: &[String],
    dependencies: &DependencyMap,
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
    let target_names: HashMap<&str, &str> = targets
        .iter()
        .map(|item| (item.key.as_str(), item.name.as_str()))
        .collect();
    let dependents = inventory
        .items
        .iter()
        .filter(|item| !wanted.contains(&item.path))
        .filter_map(|item| {
            let needs: Vec<String> = dependencies
                .get(&item.key)?
                .iter()
                .filter_map(|key| {
                    target_names
                        .get(key.as_str())
                        .map(|name| (*name).to_owned())
                })
                .collect();
            (!needs.is_empty()).then(|| Dependent {
                key: item.key.clone(),
                name: item.name.clone(),
                needs,
            })
        })
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

    #[test]
    fn plano_lista_dependentes_diretos_fora_dos_alvos() {
        let inventory = Inventory {
            items: vec![
                item("modrinth:a", "A"),
                item("modrinth:b", "B"),
                item("modrinth:c", "C"),
                item("modrinth:d", "D"),
            ],
            index_error: None,
        };
        let mut deps = DependencyMap::new();
        deps.insert("modrinth:a".into(), vec!["modrinth:b".into()]);
        deps.insert("modrinth:b".into(), vec!["modrinth:c".into()]);
        deps.insert(
            "modrinth:d".into(),
            vec!["modrinth:c".into(), "modrinth:b".into()],
        );
        let plan = removal_plan(&inventory, &["mods/C.pw.toml".into()], &deps).unwrap();
        assert_eq!(plan.targets.len(), 1);
        assert_eq!(plan.targets[0].files, ["mods/C.pw.toml"]);
        let names: Vec<(&str, &[String])> = plan
            .dependents
            .iter()
            .map(|d| (d.name.as_str(), d.needs.as_slice()))
            .collect();
        assert_eq!(
            names,
            [("B", &["C".to_owned()][..]), ("D", &["C".to_owned()][..])]
        );
        // Removendo B e C juntos, só A e D dependem deles (B sai da lista de dependentes).
        let plan = removal_plan(
            &inventory,
            &["mods/B.pw.toml".into(), "mods/C.pw.toml".into()],
            &deps,
        )
        .unwrap();
        let names: Vec<&str> = plan.dependents.iter().map(|d| d.name.as_str()).collect();
        assert_eq!(names, ["A", "D"]);
        assert_eq!(plan.dependents[1].needs, ["C", "B"]);
        let error = removal_plan(&inventory, &["mods/X.pw.toml".into()], &deps).unwrap_err();
        assert_eq!(error.code, Code::ItemNotFound);
        assert_eq!(error.params["path"], "mods/X.pw.toml");
    }
}
