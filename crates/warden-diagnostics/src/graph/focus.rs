//! Subgrafo para o desenho (D-11; ARCHITECTURE §9.6 e §18; SPEC T06, CA-T06-07).
//!
//! - [`Graph::focus`]: um item no centro, o que ele exige à esquerda (níveis −1 e −2) e quem
//!   depende dele à direita (níveis 1 e 2), até 2 níveis de cada lado.
//! - [`Graph::whole`]: o pack inteiro, para "Ver o pack inteiro".
//!
//! As ligações vão sempre de quem declara para quem satisfaz (`from` depende de `to`), com o
//! tipo da relação. Uma dependência que nenhum item do pack satisfaz vira um nó "falta no pack"
//! (sem item); a que o próprio item satisfaz com um mod embutido não vira ligação: o mod
//! embutido aparece **dentro** do nó ([`FocusNode::embedded`]).

use std::collections::{BTreeMap, HashMap, HashSet};

use serde::Serialize;

use super::query::NodeRef;
use super::{Graph, RelationKind, Requirement, State, UnknownItem};

/// Quantos níveis de cada lado o desenho focado aceita.
pub const MAX_FOCUS_DEPTH: u8 = 2;

/// Prefixo da chave dos nós de dependências que faltam no pack.
const MISSING_PREFIX: &str = "falta:";

/// Um nó do desenho.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct FocusNode {
    /// Identidade do nó nas ligações: o caminho do item ou `falta:<id>`.
    pub key: String,
    /// O item do pack; `None` para uma dependência que falta.
    pub item: Option<NodeRef>,
    /// Nome para mostrar (o do item ou o id do mod que falta).
    pub label: String,
    /// 0 = centro (ou todos, no pack inteiro); negativo = o que o centro exige (−1 direto,
    /// −2 do nível −1); positivo = quem depende do centro (1 direto, 2 do nível 1).
    pub level: i8,
    /// Ids dos mods embutidos no item (jar-in-jar), que ficam dentro do nó.
    pub embedded: Vec<String>,
    /// O item é uma biblioteca (agrupadas no pack inteiro).
    pub library: bool,
}

/// Uma ligação do desenho: `from` declara a relação com `to`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct FocusEdge {
    /// Chave do nó que declara.
    pub from: String,
    /// Chave do nó que satisfaz (ou que falta, ou com que é incompatível).
    pub to: String,
    /// Tipo da relação.
    pub kind: RelationKind,
    /// Faixa de versões pedida, quando há.
    pub range: Option<String>,
    /// Id do mod embutido em `to` que satisfaz a relação ("dentro de X"), quando é o caso.
    pub embedded: Option<String>,
    /// Nota da inferência, nas relações inferidas.
    pub note: Option<String>,
}

/// Resposta de [`Graph::focus`] e [`Graph::whole`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct FocusReport {
    /// O item no centro; `None` no pack inteiro.
    pub center: Option<NodeRef>,
    /// Níveis de cada lado (0 no pack inteiro).
    pub depth: u8,
    /// Os nós, por nível e depois por nome.
    pub nodes: Vec<FocusNode>,
    /// As ligações, sem repetir.
    pub edges: Vec<FocusEdge>,
    /// Quantos itens o grafo do pack tem.
    pub total_items: u32,
}

/// Junta nós e ligações sem repetir.
struct Builder<'g> {
    graph: &'g Graph,
    nodes: BTreeMap<String, FocusNode>,
    edges: Vec<FocusEdge>,
    seen_edges: HashSet<(String, String, RelationKind)>,
}

impl<'g> Builder<'g> {
    fn new(graph: &'g Graph) -> Self {
        Self {
            graph,
            nodes: BTreeMap::new(),
            edges: Vec::new(),
            seen_edges: HashSet::new(),
        }
    }

    /// Acrescenta o item no nível dado, se ainda não está. Devolve `true` se é novo.
    fn add_item(&mut self, index: usize, level: i8) -> bool {
        let key = self.graph.items[index].path.clone();
        if self.nodes.contains_key(&key) {
            return false;
        }
        let item = &self.graph.items[index];
        self.nodes.insert(
            key.clone(),
            FocusNode {
                key,
                item: Some(self.graph.node(index)),
                label: item.name.clone(),
                level,
                embedded: item.embedded.iter().map(|m| m.id.clone()).collect(),
                library: item.library,
            },
        );
        true
    }

    fn add_missing(&mut self, id: &str, level: i8) -> String {
        let key = format!("{MISSING_PREFIX}{}", id.to_ascii_lowercase());
        self.nodes.entry(key.clone()).or_insert_with(|| FocusNode {
            key: key.clone(),
            item: None,
            label: id.to_owned(),
            level,
            embedded: Vec::new(),
            library: false,
        });
        key
    }

    fn add_edge(
        &mut self,
        from: String,
        to: String,
        requirement: &Requirement,
        embedded: Option<String>,
    ) {
        if !self
            .seen_edges
            .insert((from.clone(), to.clone(), requirement.kind))
        {
            return;
        }
        self.edges.push(FocusEdge {
            from,
            to,
            kind: requirement.kind,
            range: requirement.range.clone(),
            embedded,
            note: requirement.note.clone(),
        });
    }

    fn finish(self, center: Option<NodeRef>, depth: u8) -> FocusReport {
        let mut nodes: Vec<FocusNode> = self.nodes.into_values().collect();
        nodes.sort_by(|a, b| {
            a.level
                .cmp(&b.level)
                .then_with(|| a.label.to_lowercase().cmp(&b.label.to_lowercase()))
                .then_with(|| a.key.cmp(&b.key))
        });
        let order: HashMap<&str, usize> = nodes
            .iter()
            .enumerate()
            .map(|(position, node)| (node.key.as_str(), position))
            .collect();
        let mut edges = self.edges;
        edges.sort_by(|a, b| {
            order[a.from.as_str()]
                .cmp(&order[b.from.as_str()])
                .then_with(|| order[a.to.as_str()].cmp(&order[b.to.as_str()]))
                .then_with(|| a.kind.cmp(&b.kind))
        });
        FocusReport {
            center,
            depth,
            nodes,
            edges,
            total_items: u32::try_from(self.graph.items.len()).unwrap_or(u32::MAX),
        }
    }
}

impl Graph {
    /// O subgrafo em volta do item, com até `depth` níveis de cada lado (1 ou 2; fora disso,
    /// vale o limite mais próximo).
    ///
    /// À esquerda, tudo o que o item declara (obrigatórias, opcionais, incompatíveis,
    /// inferidas), e o que esses declaram no nível seguinte; à direita, quem declara alguma
    /// relação com o item, e quem declara com esses. Um item aparece uma vez só, no primeiro
    /// nível em que é encontrado (o centro vence; depois o mais perto dele).
    pub fn focus(&self, path: &str, depth: u8) -> Result<FocusReport, UnknownItem> {
        let center = self.index_of(path)?;
        let depth = depth.clamp(1, MAX_FOCUS_DEPTH);
        let mut builder = Builder::new(self);
        builder.add_item(center, 0);

        // Esquerda: o que o centro exige e, depois, o que isso exige.
        let mut frontier = vec![center];
        for step in 1..=depth {
            let level = -i8::try_from(step).unwrap_or(i8::MAX);
            let mut next = Vec::new();
            for &index in &frontier {
                let from = self.items[index].path.clone();
                for requirement in &self.items[index].requirements {
                    match requirement.state {
                        State::Own => {}
                        State::Missing => {
                            let to = builder.add_missing(&requirement.id, level);
                            builder.add_edge(from.clone(), to, requirement, None);
                        }
                        State::InPack => {
                            for provider in &requirement.providers {
                                if builder.add_item(provider.item, level) {
                                    next.push(provider.item);
                                }
                                let to = self.items[provider.item].path.clone();
                                let embedded = provider.embedded.then(|| provider.mod_id.clone());
                                builder.add_edge(from.clone(), to, requirement, embedded);
                            }
                        }
                    }
                }
            }
            frontier = next;
        }

        // Direita: quem declara uma relação com o centro e, depois, com esses.
        let mut frontier: HashSet<usize> = HashSet::from([center]);
        for step in 1..=depth {
            let level = i8::try_from(step).unwrap_or(i8::MAX);
            let mut next = HashSet::new();
            for (index, node) in self.items.iter().enumerate() {
                if frontier.contains(&index) {
                    continue;
                }
                for requirement in &node.requirements {
                    for provider in &requirement.providers {
                        if !frontier.contains(&provider.item) {
                            continue;
                        }
                        if builder.add_item(index, level) {
                            next.insert(index);
                        }
                        let from = node.path.clone();
                        let to = self.items[provider.item].path.clone();
                        let embedded = provider.embedded.then(|| provider.mod_id.clone());
                        builder.add_edge(from, to, requirement, embedded);
                    }
                }
            }
            frontier = next;
        }

        Ok(builder.finish(Some(self.node(center)), depth))
    }

    /// O pack inteiro: todos os itens e todas as ligações entre eles (as dependências que
    /// faltam ficam de fora; elas aparecem em Problemas e no grafo focado).
    #[must_use]
    pub fn whole(&self) -> FocusReport {
        let mut builder = Builder::new(self);
        for index in 0..self.items.len() {
            builder.add_item(index, 0);
        }
        for node in &self.items {
            for requirement in &node.requirements {
                if requirement.state != State::InPack {
                    continue;
                }
                for provider in &requirement.providers {
                    let to = self.items[provider.item].path.clone();
                    let embedded = provider.embedded.then(|| provider.mod_id.clone());
                    builder.add_edge(node.path.clone(), to, requirement, embedded);
                }
            }
        }
        builder.finish(None, 0)
    }
}

#[cfg(test)]
mod tests {
    use warden_jarmeta::JarMetadata;

    use super::super::tests::{input, item, jar_json, library, module, path};
    use super::super::{Graph, InferredEdge, InferredSource, RelationKind};
    use super::{FocusEdge, FocusReport};

    /// O Create embute Flywheel e Registrate; exige Flywheel (dentro dele), Ponder (falta) e
    /// usa o JEI como opcional, que exige o JEI Core. Dois addons exigem o Create, um terceiro
    /// exige um addon, um mod exige o Flywheel (embutido no Create) e outro é incompatível.
    fn create_pack() -> Graph {
        let create: JarMetadata = serde_json::from_value(jar_json(
            &module(
                "create",
                &[
                    ("flywheel", "required"),
                    ("ponder", "required"),
                    ("jei", "optional"),
                    ("minecraft", "required"),
                ],
                &[],
            ),
            &[
                jar_json(&module("flywheel", &[], &[]), &[]),
                jar_json(&module("registrate", &[], &[]), &[]),
            ],
        ))
        .unwrap();
        let mut create_item = item("create", &[]);
        create_item.jar = Some(create);
        let mut data = input(vec![
            create_item,
            item("jei", &[("jeicore", "required")]),
            library("jeicore", &[]),
            item("slice", &[("create", "required")]),
            item("crafts", &[("create", "required")]),
            item("slice_addon", &[("slice", "required")]),
            item("shader", &[("flywheel", "required")]),
            item("rival", &[("create", "breaks")]),
            item("solto", &[]),
        ]);
        data.inferred = vec![InferredEdge {
            from: path("solto"),
            to: path("create"),
            source: InferredSource::Log,
            note: "o log pediu o Create".into(),
        }];
        Graph::build(&data)
    }

    fn levels(report: &FocusReport) -> Vec<(&str, i8)> {
        report
            .nodes
            .iter()
            .map(|node| (node.label.as_str(), node.level))
            .collect()
    }

    fn edge<'r>(report: &'r FocusReport, from: &str, to: &str) -> &'r FocusEdge {
        report
            .edges
            .iter()
            .find(|edge| edge.from == from && edge.to == to)
            .unwrap_or_else(|| panic!("sem ligação {from} → {to}"))
    }

    #[test]
    fn ca_t06_07_create_no_centro_com_dependencias_a_esquerda_e_dependentes_a_direita() {
        let graph = create_pack();
        let report = graph.focus(&path("create"), 2).unwrap();
        assert_eq!(report.center.as_ref().unwrap().name, "create");
        assert_eq!(report.depth, 2);
        assert_eq!(
            levels(&report),
            vec![
                ("jeicore", -2),
                ("jei", -1),
                ("ponder", -1),
                ("create", 0),
                ("crafts", 1),
                ("rival", 1),
                ("shader", 1),
                ("slice", 1),
                ("solto", 1),
                ("slice_addon", 2),
            ]
        );
        // As embutidas ficam dentro do nó do Create, sem ligação própria; o jogo não é nó.
        let center = &report.nodes[3];
        assert_eq!(center.embedded, vec!["flywheel", "registrate"]);
        assert!(
            report
                .edges
                .iter()
                .all(|edge| !edge.to.contains("flywheel"))
        );
        assert!(report.nodes.iter().all(|node| node.label != "minecraft"));
        // O tipo de cada ligação.
        assert_eq!(
            edge(&report, &path("create"), &path("jei")).kind,
            RelationKind::Optional
        );
        let missing = edge(&report, &path("create"), "falta:ponder");
        assert_eq!(missing.kind, RelationKind::Required);
        assert!(report.nodes[2].item.is_none(), "ponder falta: nó sem item");
        assert_eq!(
            edge(&report, &path("jei"), &path("jeicore")).kind,
            RelationKind::Required
        );
        assert!(report.nodes[0].library);
        assert_eq!(
            edge(&report, &path("slice"), &path("create")).kind,
            RelationKind::Required
        );
        assert_eq!(
            edge(&report, &path("rival"), &path("create")).kind,
            RelationKind::Breaks
        );
        let inferred = edge(&report, &path("solto"), &path("create"));
        assert_eq!(inferred.kind, RelationKind::Inferred);
        assert_eq!(inferred.note.as_deref(), Some("o log pediu o Create"));
        // Quem exige um mod embutido no Create depende do Create, "dentro do Create".
        assert_eq!(
            edge(&report, &path("shader"), &path("create"))
                .embedded
                .as_deref(),
            Some("flywheel")
        );
        assert_eq!(
            edge(&report, &path("slice_addon"), &path("slice")).kind,
            RelationKind::Required
        );
        assert_eq!(report.total_items, 9);
    }

    #[test]
    fn um_nivel_so_mostra_os_vizinhos_diretos_e_a_profundidade_tem_limite() {
        let graph = create_pack();
        let one = graph.focus(&path("create"), 1).unwrap();
        assert!(one.nodes.iter().all(|node| node.level.abs() <= 1));
        assert!(!one.nodes.iter().any(|node| node.label == "jeicore"));
        assert!(!one.nodes.iter().any(|node| node.label == "slice_addon"));
        assert_eq!(graph.focus(&path("create"), 0).unwrap().depth, 1);
        assert_eq!(graph.focus(&path("create"), 9).unwrap().depth, 2);
    }

    #[test]
    fn ciclo_aparece_uma_vez_com_as_duas_ligacoes() {
        let graph = Graph::build(&input(vec![
            item("a", &[("b", "required")]),
            item("b", &[("a", "required")]),
        ]));
        let report = graph.focus(&path("a"), 2).unwrap();
        assert_eq!(levels(&report), vec![("b", -1), ("a", 0)]);
        assert_eq!(report.edges.len(), 2);
        edge(&report, &path("a"), &path("b"));
        edge(&report, &path("b"), &path("a"));
    }

    #[test]
    fn item_desconhecido_e_erro() {
        let graph = create_pack();
        assert_eq!(
            graph.focus("mods/nao-existe.pw.toml", 2).unwrap_err().0,
            "mods/nao-existe.pw.toml"
        );
    }

    #[test]
    fn pack_inteiro_traz_todos_os_itens_e_as_ligacoes_entre_eles() {
        let graph = create_pack();
        let whole = graph.whole();
        assert!(whole.center.is_none());
        assert_eq!(whole.nodes.len(), 9);
        assert!(
            whole
                .nodes
                .iter()
                .all(|node| node.level == 0 && node.item.is_some())
        );
        // Sem nós de dependências que faltam; cada ligação liga dois itens do pack.
        assert!(
            whole
                .edges
                .iter()
                .all(|edge| !edge.to.starts_with("falta:"))
        );
        assert_eq!(whole.edges.len(), 8);
        assert!(
            whole
                .nodes
                .iter()
                .find(|node| node.label == "jeicore")
                .unwrap()
                .library
        );
    }
}
