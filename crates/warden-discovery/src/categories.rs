//! Mapeamento curado de categorias (SPEC T08; ARCHITECTURE §17.1).
//!
//! `data/categories.toml` junta as categorias do Modrinth e da CurseForge sob um nome em
//! português. A interface mostra a lista de [`categories`] e devolve só o `id`; o app o traduz
//! em [`CategoryFilter`] com [`resolve`] antes de buscar. Categoria sem par filtra só a fonte
//! que a tem.

use std::sync::OnceLock;

use serde::{Deserialize, Serialize};
use warden_project::search::{CategoryFilter, ProjectKind, SourceId};

/// O arquivo do mapeamento, embutido no app.
const DATA: &str = include_str!("../data/categories.toml");

/// Uma categoria da lista de filtros.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct DiscoverCategory {
    /// Identificador estável (o que a busca recebe).
    pub id: String,
    /// Nome em português.
    pub name: String,
    /// As fontes que têm par para a categoria (a lista nunca é vazia).
    pub sources: Vec<SourceId>,
}

/// Uma entrada do arquivo.
#[derive(Debug, Clone, Deserialize)]
struct Entry {
    id: String,
    name: String,
    kinds: Vec<ProjectKind>,
    #[serde(default)]
    modrinth: Vec<String>,
    #[serde(default)]
    curseforge: Vec<u32>,
}

#[derive(Debug, Deserialize)]
struct File {
    category: Vec<Entry>,
}

fn entries() -> &'static [Entry] {
    static PARSED: OnceLock<Vec<Entry>> = OnceLock::new();
    PARSED.get_or_init(|| {
        // O arquivo é embutido e conferido pelos testes: um erro aqui é bug de build.
        toml::from_str::<File>(DATA).map_or_else(
            |error| {
                tracing::error!(%error, "categories.toml inválido");
                Vec::new()
            },
            |file| file.category,
        )
    })
}

/// As categorias de um tipo de projeto, na ordem do arquivo. `curseforge_on` diz se a
/// CurseForge está ligada: sem ela, só as categorias que o Modrinth tem aparecem (uma categoria
/// só da CurseForge não filtraria nada).
#[must_use]
pub fn categories(kind: ProjectKind, curseforge_on: bool) -> Vec<DiscoverCategory> {
    entries()
        .iter()
        .filter(|entry| entry.kinds.contains(&kind))
        .filter_map(|entry| {
            let mut sources = Vec::new();
            if !entry.modrinth.is_empty() {
                sources.push(SourceId::Modrinth);
            }
            if curseforge_on && !entry.curseforge.is_empty() {
                sources.push(SourceId::Curseforge);
            }
            (!sources.is_empty()).then(|| DiscoverCategory {
                id: entry.id.clone(),
                name: entry.name.clone(),
                sources,
            })
        })
        .collect()
}

/// A categoria `id` do tipo dado, traduzida para as categorias de cada fonte. `None` se a
/// categoria não existe para o tipo.
#[must_use]
pub fn resolve(id: &str, kind: ProjectKind) -> Option<CategoryFilter> {
    entries()
        .iter()
        .find(|entry| entry.id == id && entry.kinds.contains(&kind))
        .map(|entry| CategoryFilter {
            modrinth: entry.modrinth.clone(),
            curseforge: entry.curseforge.clone(),
        })
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    const KINDS: [ProjectKind; 3] = [
        ProjectKind::Mod,
        ProjectKind::ResourcePack,
        ProjectKind::Shader,
    ];

    #[test]
    fn arquivo_valido_com_ids_unicos_e_ao_menos_uma_fonte() {
        let all = entries();
        assert!(all.len() > 20, "o mapeamento não carregou");
        let mut seen = HashSet::new();
        for entry in all {
            assert!(seen.insert(entry.id.as_str()), "id repetido: {}", entry.id);
            assert!(!entry.name.trim().is_empty());
            assert!(!entry.kinds.is_empty(), "{} sem tipo", entry.id);
            assert!(
                !entry.modrinth.is_empty() || !entry.curseforge.is_empty(),
                "{} sem nenhuma fonte",
                entry.id
            );
        }
    }

    #[test]
    fn cada_tipo_tem_categorias() {
        for kind in KINDS {
            assert!(
                !categories(kind, true).is_empty(),
                "{kind:?} sem categorias"
            );
        }
    }

    #[test]
    fn tecnologia_junta_as_duas_fontes() {
        let filter = resolve("tecnologia", ProjectKind::Mod).unwrap();
        assert_eq!(filter.modrinth, ["technology"]);
        assert_eq!(filter.curseforge, [412]);
        assert!(filter.applies_to(SourceId::Modrinth) && filter.applies_to(SourceId::Curseforge));
    }

    #[test]
    fn categoria_sem_par_filtra_so_a_fonte_que_a_tem() {
        let only_modrinth = resolve("desempenho", ProjectKind::Mod).unwrap();
        assert!(only_modrinth.applies_to(SourceId::Modrinth));
        assert!(!only_modrinth.applies_to(SourceId::Curseforge));
        let only_curseforge = resolve("redstone", ProjectKind::Mod).unwrap();
        assert!(!only_curseforge.applies_to(SourceId::Modrinth));
    }

    #[test]
    fn sem_curseforge_a_lista_so_traz_o_que_o_modrinth_tem() {
        let list = categories(ProjectKind::Mod, false);
        assert!(list.iter().any(|c| c.id == "tecnologia"));
        assert!(list.iter().all(|c| c.sources == [SourceId::Modrinth]));
        assert!(!list.iter().any(|c| c.id == "redstone"));
        let with = categories(ProjectKind::Mod, true);
        assert!(with.iter().any(|c| c.id == "redstone"));
    }

    #[test]
    fn categoria_de_outro_tipo_nao_resolve() {
        assert!(resolve("rp-combate", ProjectKind::Mod).is_none());
        assert!(resolve("não-existe", ProjectKind::Mod).is_none());
    }
}
