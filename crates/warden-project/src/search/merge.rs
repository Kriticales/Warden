//! Combinação dos resultados das fontes numa página só (ADR-0027; ARCHITECTURE §17 e §17.1).
//!
//! - **Relevância:** intercalada pela posição em cada fonte (1º de cada, 2º de cada…). A
//!   relevância de uma fonte não se compara com a da outra, então nenhuma é favorecida; no
//!   empate de posição vem primeiro a fonte preferida (Modrinth).
//! - **Downloads, atualização e novos:** mescla pelos números de cada fonte (`downloads` ×
//!   `downloadCount`; `date_modified` × `dateModified`; criação), sempre escolhendo a cabeça
//!   maior entre as fontes. Como cada fonte já vem ordenada, cada uma é consumida em ordem.
//! - **Duplicatas:** um resultado que é o mesmo projeto de outro já escolhido (mesmo autor e
//!   slug ou nome, [`crate::dedup::same_project`]) entra no item já escolhido como a segunda
//!   fonte, em vez de virar outro item.
//!
//! Cada fonte é sempre consumida pelo começo, então a página seguinte de cada uma começa em
//! `offset + consumidos`. Um resultado de mais adiante que foi juntado a um item desta página
//! pode voltar a aparecer quando a fonte chegar nele; a interface descarta quem já mostrou
//! (pelas referências das fontes).

use super::{InstalledKeys, SearchResult, SearchSort, SourceHit, SourceId, SourceOffset};
use crate::dedup::{Identity, same_project};

/// Os resultados de uma fonte para combinar.
#[derive(Debug, Clone, Copy)]
pub struct SourceBatch<'a> {
    /// A fonte.
    pub source: SourceId,
    /// Deslocamento pedido à fonte.
    pub offset: u32,
    /// Total que a fonte diz ter.
    pub total: u64,
    /// Resultados, na ordem da fonte.
    pub hits: &'a [SourceHit],
}

/// Um item combinado: uma ou mais referências do mesmo projeto, a preferida primeiro.
#[derive(Debug, Clone, PartialEq)]
pub struct MergedItem {
    /// Os resultados de cada fonte.
    pub hits: Vec<SourceHit>,
}

impl MergedItem {
    /// O resultado como a interface recebe.
    #[must_use]
    pub fn into_result(self, installed: &InstalledKeys) -> SearchResult {
        let mut hits = self.hits;
        hits.sort_by_key(|hit| hit.reference.source);
        let main = hits[0].clone();
        let key = main.reference.source.key(&main.reference.project_id);
        let in_pack = hits
            .iter()
            .any(|hit| installed.contains(&hit.reference.source.key(&hit.reference.project_id)));
        let icon_url = hits.iter().find_map(|hit| hit.icon_url.clone());
        let updated = hits
            .iter()
            .map(|hit| hit.updated.as_str())
            .max()
            .unwrap_or_default()
            .to_owned();
        SearchResult {
            key,
            title: main.title,
            author: main.author,
            summary: if main.summary.is_empty() {
                hits.iter()
                    .find(|hit| !hit.summary.is_empty())
                    .map(|hit| hit.summary.clone())
                    .unwrap_or_default()
            } else {
                main.summary
            },
            icon_url,
            downloads: hits.iter().map(|hit| hit.reference.downloads).sum(),
            updated,
            in_pack,
            compatible: hits.iter().any(|hit| hit.compatible),
            manual_download: hits.iter().all(|hit| hit.manual_download),
            sources: hits.into_iter().map(|hit| hit.reference).collect(),
        }
    }
}

/// Resultado da combinação.
#[derive(Debug, Clone, PartialEq)]
pub struct Combined {
    /// Até `page_size` itens.
    pub items: Vec<MergedItem>,
    /// Onde cada fonte continua.
    pub offsets: Vec<SourceOffset>,
}

/// Combina as páginas das fontes (cada uma pedida com `limit = page_size`).
#[must_use]
pub fn combine(batches: &[SourceBatch<'_>], sort: SearchSort, page_size: usize) -> Combined {
    let order = sequence(batches, sort);
    let mut taken = vec![vec![false; 0]; batches.len()];
    for (index, batch) in batches.iter().enumerate() {
        taken[index] = vec![false; batch.hits.len()];
    }
    let mut consumed = vec![0_usize; batches.len()];
    let mut items: Vec<MergedItem> = Vec::new();
    for (batch_index, hit_index) in order {
        if items.len() >= page_size {
            break;
        }
        consumed[batch_index] = hit_index + 1;
        if taken[batch_index][hit_index] {
            continue;
        }
        taken[batch_index][hit_index] = true;
        let hit = &batches[batch_index].hits[hit_index];
        let mut item = MergedItem {
            hits: vec![hit.clone()],
        };
        // O mesmo projeto noutra fonte, em qualquer posição desta página da fonte.
        for (other_index, other) in batches.iter().enumerate() {
            if other.source == batches[batch_index].source {
                continue;
            }
            if let Some(found) = other
                .hits
                .iter()
                .enumerate()
                .position(|(index, candidate)| {
                    !taken[other_index][index] && same_project(identity(hit), identity(candidate))
                })
            {
                taken[other_index][found] = true;
                item.hits.push(other.hits[found].clone());
            }
        }
        items.push(item);
    }
    let offsets = batches
        .iter()
        .zip(&consumed)
        .map(|(batch, &used)| {
            let fetched = batch.hits.len();
            let next = u64::from(batch.offset) + used as u64;
            let source_exhausted = fetched < page_size || next >= batch.total;
            SourceOffset {
                source: batch.source,
                offset: u32::try_from(next).unwrap_or(u32::MAX),
                done: used == fetched && source_exhausted,
            }
        })
        .collect();
    Combined { items, offsets }
}

fn identity(hit: &SourceHit) -> Identity<'_> {
    Identity {
        author: &hit.author,
        slug: &hit.reference.slug,
        title: &hit.title,
    }
}

/// A ordem em que os resultados são considerados: `(fonte, posição)`.
fn sequence(batches: &[SourceBatch<'_>], sort: SearchSort) -> Vec<(usize, usize)> {
    let total: usize = batches.iter().map(|batch| batch.hits.len()).sum();
    let mut order = Vec::with_capacity(total);
    match sort {
        SearchSort::Relevance => {
            let longest = batches
                .iter()
                .map(|batch| batch.hits.len())
                .max()
                .unwrap_or(0);
            for position in 0..longest {
                for (index, batch) in batches.iter().enumerate() {
                    if position < batch.hits.len() {
                        order.push((index, position));
                    }
                }
            }
        }
        SearchSort::Downloads | SearchSort::Updated | SearchSort::Newest => {
            let mut heads = vec![0_usize; batches.len()];
            while order.len() < total {
                let mut best: Option<usize> = None;
                for (index, batch) in batches.iter().enumerate() {
                    let Some(hit) = batch.hits.get(heads[index]) else {
                        continue;
                    };
                    let better = match best {
                        None => true,
                        Some(current) => {
                            let other = &batches[current].hits[heads[current]];
                            greater(hit, other, sort)
                        }
                    };
                    if better {
                        best = Some(index);
                    }
                }
                let Some(index) = best else { break };
                order.push((index, heads[index]));
                heads[index] += 1;
            }
        }
    }
    order
}

/// Se `a` vem antes de `b` nesta ordem (estritamente maior; no empate fica a fonte anterior).
fn greater(a: &SourceHit, b: &SourceHit, sort: SearchSort) -> bool {
    match sort {
        SearchSort::Downloads => a.reference.downloads > b.reference.downloads,
        SearchSort::Updated => timestamp(&a.updated) > timestamp(&b.updated),
        SearchSort::Newest => timestamp(&a.created) > timestamp(&b.created),
        SearchSort::Relevance => false,
    }
}

/// Data RFC 3339 comparável como texto: só `AAAA-MM-DDTHH:MM:SS` (as fontes variam nas
/// frações de segundo e no fuso `Z`).
fn timestamp(value: &str) -> &str {
    value.get(..19).unwrap_or(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::search::SourceRef;

    fn hit(source: SourceId, id: &str, author: &str, downloads: f64, updated: &str) -> SourceHit {
        SourceHit {
            reference: SourceRef {
                source,
                project_id: id.into(),
                slug: id.to_lowercase(),
                downloads,
            },
            title: id.into(),
            author: author.into(),
            summary: String::new(),
            icon_url: None,
            updated: updated.into(),
            created: updated.into(),
            compatible: true,
            manual_download: source == SourceId::Curseforge,
        }
    }

    fn ids(combined: &Combined) -> Vec<String> {
        combined
            .items
            .iter()
            .map(|item| {
                item.hits
                    .iter()
                    .map(|hit| hit.reference.project_id.as_str())
                    .collect::<Vec<_>>()
                    .join("+")
            })
            .collect()
    }

    const M: SourceId = SourceId::Modrinth;
    const C: SourceId = SourceId::Curseforge;

    #[test]
    fn relevancia_intercala_pela_posicao() {
        let m = [hit(M, "M1", "a", 1.0, ""), hit(M, "M2", "b", 1.0, "")];
        let c = [
            hit(C, "C1", "c", 1.0, ""),
            hit(C, "C2", "d", 1.0, ""),
            hit(C, "C3", "e", 1.0, ""),
        ];
        let combined = combine(
            &[
                SourceBatch {
                    source: M,
                    offset: 0,
                    total: 2,
                    hits: &m,
                },
                SourceBatch {
                    source: C,
                    offset: 0,
                    total: 3,
                    hits: &c,
                },
            ],
            SearchSort::Relevance,
            20,
        );
        assert_eq!(ids(&combined), ["M1", "C1", "M2", "C2", "C3"]);
        assert!(combined.offsets.iter().all(|offset| offset.done));
    }

    #[test]
    fn downloads_mescla_pelos_numeros_de_cada_fonte() {
        let m = [hit(M, "M1", "a", 900.0, ""), hit(M, "M2", "b", 100.0, "")];
        let c = [hit(C, "C1", "c", 1000.0, ""), hit(C, "C2", "d", 500.0, "")];
        let combined = combine(
            &[
                SourceBatch {
                    source: M,
                    offset: 0,
                    total: 2,
                    hits: &m,
                },
                SourceBatch {
                    source: C,
                    offset: 0,
                    total: 2,
                    hits: &c,
                },
            ],
            SearchSort::Downloads,
            20,
        );
        assert_eq!(ids(&combined), ["C1", "M1", "C2", "M2"]);
    }

    #[test]
    fn atualizacao_compara_datas_com_formatos_diferentes() {
        let m = [hit(M, "M1", "a", 0.0, "2026-09-01T10:00:00.123456Z")];
        let c = [hit(C, "C1", "c", 0.0, "2026-09-02T09:00:00Z")];
        let combined = combine(
            &[
                SourceBatch {
                    source: M,
                    offset: 0,
                    total: 1,
                    hits: &m,
                },
                SourceBatch {
                    source: C,
                    offset: 0,
                    total: 1,
                    hits: &c,
                },
            ],
            SearchSort::Updated,
            20,
        );
        assert_eq!(ids(&combined), ["C1", "M1"]);
    }

    #[test]
    fn duplicata_vira_um_item_com_as_duas_fontes() {
        let m = [hit(M, "jei", "mezz", 10.0, ""), hit(M, "M2", "b", 1.0, "")];
        let c = [hit(C, "C1", "c", 1.0, ""), hit(C, "JEI", "Mezz", 20.0, "")];
        let combined = combine(
            &[
                SourceBatch {
                    source: M,
                    offset: 0,
                    total: 2,
                    hits: &m,
                },
                SourceBatch {
                    source: C,
                    offset: 0,
                    total: 2,
                    hits: &c,
                },
            ],
            SearchSort::Relevance,
            20,
        );
        assert_eq!(ids(&combined), ["jei+JEI", "C1", "M2"]);
        let result = combined.items[0].clone().into_result(&InstalledKeys::new());
        assert_eq!(result.key, "modrinth:jei");
        assert!((result.downloads - 30.0).abs() < f64::EPSILON);
        assert_eq!(result.sources.len(), 2);
        assert!(!result.manual_download, "o Modrinth baixa sozinho");
    }

    #[test]
    fn pagina_cheia_guarda_onde_cada_fonte_continua() {
        let m: Vec<SourceHit> = (0..20)
            .map(|i| hit(M, &format!("M{i}"), "a", 1.0, ""))
            .collect();
        let c: Vec<SourceHit> = (0..20)
            .map(|i| hit(C, &format!("C{i}"), "c", 1.0, ""))
            .collect();
        let combined = combine(
            &[
                SourceBatch {
                    source: M,
                    offset: 40,
                    total: 500,
                    hits: &m,
                },
                SourceBatch {
                    source: C,
                    offset: 40,
                    total: 61,
                    hits: &c,
                },
            ],
            SearchSort::Relevance,
            20,
        );
        assert_eq!(combined.items.len(), 20);
        assert_eq!(combined.offsets[0].offset, 50);
        assert_eq!(combined.offsets[1].offset, 50);
        assert!(!combined.offsets[0].done && !combined.offsets[1].done);
    }

    #[test]
    fn ja_no_pack_por_qualquer_fonte() {
        let item = MergedItem {
            hits: vec![
                hit(C, "238222", "mezz", 1.0, ""),
                hit(M, "u6dRKJwZ", "mezz", 1.0, ""),
            ],
        };
        let installed = InstalledKeys::from(["curseforge:238222".to_owned()]);
        let result = item.into_result(&installed);
        assert!(result.in_pack);
        assert_eq!(
            result.key, "modrinth:u6dRKJwZ",
            "o Modrinth é a fonte preferida"
        );
    }
}
