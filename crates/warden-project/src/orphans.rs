//! Dependências que ficam sem uso ("órfãs") quando itens saem do pack (SPEC T06, Remover, P1;
//! P1-15).
//!
//! Quem decide o que é "sem uso" é o grafo da D-07 (`warden_diagnostics::graph::Graph::orphans`:
//! as bibliotecas que nenhum item escolhido pelo usuário mantém, considerando `provides` e
//! jar-in-jar). A `warden-app` consulta o grafo do pack como está e o grafo sem os itens que vão
//! sair; aqui fica a regra do diálogo de remover: oferecer só o que **passa** a ficar sem uso por
//! causa desta remoção (o que já estava sem uso aparece no modo Grafo, D-11), nunca um dos
//! próprios itens removidos e só o que ainda está no inventário.

use std::collections::HashSet;

use serde::Serialize;

use crate::inventory::Inventory;

/// Uma dependência que fica sem uso e pode sair junto.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct OrphanOffer {
    /// Chave estável do item.
    pub key: String,
    /// Caminho do item no pack (vai para `items_remove` junto com os alvos).
    pub path: String,
    /// Nome legível.
    pub name: String,
    /// Itens que ficam e só o usam como opcional, recomendado ou sugerido.
    pub optional_users: Vec<String>,
}

/// Uma biblioteca sem uso segundo o grafo: o caminho e os nomes dos usuários opcionais.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphOrphan {
    /// Caminho do item.
    pub path: String,
    /// Nomes dos itens que só o usam como opcional.
    pub optional_users: Vec<String>,
}

/// As dependências que ficam sem uso por causa da remoção de `targets`.
///
/// `before` são as órfãs do pack como está e `after`, as do pack sem os alvos (na ordem em que
/// o grafo as devolve, por nome). Fica de fora: o que já era órfão, os próprios alvos e o que
/// não está no inventário.
#[must_use]
pub fn orphans_after_removal(
    inventory: &Inventory,
    targets: &[String],
    before: &[GraphOrphan],
    after: Vec<GraphOrphan>,
) -> Vec<OrphanOffer> {
    let targets: HashSet<&str> = targets.iter().map(String::as_str).collect();
    let before: HashSet<&str> = before.iter().map(|orphan| orphan.path.as_str()).collect();
    after
        .into_iter()
        .filter(|orphan| {
            !targets.contains(orphan.path.as_str()) && !before.contains(orphan.path.as_str())
        })
        .filter_map(|orphan| {
            let item = inventory
                .items
                .iter()
                .find(|item| item.path == orphan.path)?;
            Some(OrphanOffer {
                key: item.key.clone(),
                path: item.path.clone(),
                name: item.name.clone(),
                optional_users: orphan.optional_users,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inventory::{InventoryItem, ItemKind, ItemSide, ItemSource, ItemState};

    fn item(name: &str) -> InventoryItem {
        InventoryItem {
            key: format!("modrinth:{name}"),
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

    fn orphan(name: &str) -> GraphOrphan {
        GraphOrphan {
            path: format!("mods/{name}.pw.toml"),
            optional_users: Vec::new(),
        }
    }

    #[test]
    fn so_oferece_o_que_ficou_sem_uso_agora() {
        let inventory = Inventory {
            items: ["A", "Kotlin", "Velha", "Cloth"].map(item).to_vec(),
            index_error: None,
        };
        let mut cloth = orphan("Cloth");
        cloth.optional_users = vec!["Outro".into()];
        let offers = orphans_after_removal(
            &inventory,
            &["mods/A.pw.toml".into()],
            &[orphan("Velha")],
            vec![
                cloth,
                orphan("Kotlin"),
                orphan("Velha"),
                orphan("A"),
                orphan("Sumiu"),
            ],
        );
        let names: Vec<&str> = offers.iter().map(|o| o.name.as_str()).collect();
        assert_eq!(names, ["Cloth", "Kotlin"]);
        assert_eq!(offers[0].optional_users, ["Outro"]);
        assert_eq!(offers[1].key, "modrinth:Kotlin");
        assert_eq!(offers[1].path, "mods/Kotlin.pw.toml");
    }
}
