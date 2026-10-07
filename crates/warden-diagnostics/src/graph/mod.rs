//! Grafo de dependências do pack e as suas consultas (D-07; ARCHITECTURE §9.6; R5A §6.1).
//!
//! O grafo nasce do mesmo modelo do diagnóstico (D-01): os metadados dos jars
//! ([`warden_jarmeta::JarMetadata`]) e, quando um jar não está disponível, as relações
//! obrigatórias que a API da fonte declarou ([`GraphItem::api_required`], ou
//! [`GraphItem::from_pack_items`] a partir dos [`crate::pretest::PackItem`]).
//!
//! - **Nós:** os itens do pack e os mods embutidos neles (jar-in-jar), que ficam *dentro* do
//!   nó do item que os traz.
//! - **Arestas tipadas** ([`RelationKind`]): obrigatória, opcional, recomendada, sugerida,
//!   incompatível (`breaks`), conflito (`conflicts`) e **inferida** (busca do culpado ou log,
//!   guardada por pack em [`InferredStore`]). Um mod que `provides` outro id satisfaz as
//!   dependências desse id; um mod embutido satisfaz as dependências de quem o traz e, se
//!   outro item precisa daquele id, a ligação vai para o item que o embute.
//! - **Ciclos** de dependência formam um só nó ([`Graph::cycles`]); as consultas tratam os
//!   membros de um ciclo como um grupo.
//!
//! Consultas ([`Graph`]):
//!
//! - [`Graph::dependents`]: o que para de funcionar se os itens saírem (fecho transitivo, com
//!   alternativas: uma dependência com dois fornecedores só quebra quando os dois saem), mais as
//!   relações diretas "Depende de" e "Usado por" (CA-T06-06).
//! - [`Graph::why_in_pack`]: "Você adicionou" ou a cadeia até um item adicionado pelo usuário
//!   (CA-T07-03).
//! - [`Graph::orphans`]: bibliotecas que nenhum item mantido usa mais.
//!
//! O histórico de adições ([`AdditionHistory`]) é opcional. Com ele, "adicionado pelo usuário"
//! é exato; sem ele, vale a regra da R5A: quem nenhum outro item exige foi escolhido pelo
//! usuário, **exceto** as bibliotecas conhecidas (o jar se declara `LIBRARY` ou o id está em
//! [`libraries`]), que sem dependentes são tratadas como sem uso.
//!
//! A crate não abre jars nem faz rede: o chamador monta a [`GraphInput`].

mod build;
mod inferred;
pub mod libraries;
mod query;

#[cfg(test)]
mod tests;

pub use inferred::{InferredEdge, InferredSource, InferredStore};
pub use query::{
    Affected, ChainLink, DependentsReport, DependsOn, DependsOnState, NodeRef, Orphan, ProviderRef,
    UnknownItem, UsedBy, Why, WhyReport,
};

use std::collections::{BTreeMap, HashMap};

use serde::{Deserialize, Serialize};
use warden_jarmeta::{JarMetadata, Loader};

use crate::pretest::PackItem;

/// Tipo de uma ligação entre itens.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, specta::Type,
)]
#[serde(rename_all = "camelCase")]
pub enum RelationKind {
    /// Obrigatória (`depends`, `required`, `mandatory`; ou relação obrigatória da API).
    Required,
    /// Opcional.
    Optional,
    /// Recomendada (`recommends`).
    Recommended,
    /// Sugerida (`suggests`).
    Suggested,
    /// Incompatível: impede o carregamento (`breaks`, `incompatible`).
    Breaks,
    /// Conflito: só avisa (`conflicts`, `discouraged`).
    Conflicts,
    /// Dependência não declarada, inferida pela busca do culpado ou pelo log.
    Inferred,
}

impl RelationKind {
    /// `true` se o item precisa do outro para funcionar (obrigatória ou inferida).
    #[must_use]
    pub fn is_needed(self) -> bool {
        matches!(self, Self::Required | Self::Inferred)
    }

    /// `true` se é uma incompatibilidade ou conflito.
    #[must_use]
    pub fn is_conflict(self) -> bool {
        matches!(self, Self::Breaks | Self::Conflicts)
    }
}

/// Um item do pack na entrada do grafo.
#[derive(Debug, Clone, Default)]
pub struct GraphItem {
    /// Caminho do item no pack (`mods/create.pw.toml`), a identidade dele nas respostas.
    pub path: String,
    /// Nome para mostrar.
    pub name: String,
    /// Metadados do jar, quando ele está na instância.
    pub jar: Option<JarMetadata>,
    /// Caminhos de outros itens que a API da fonte diz serem obrigatórios para este.
    /// Preenchido para os itens sem jar (e somado ao que o jar declara, sem repetir).
    pub api_required: Vec<String>,
}

impl GraphItem {
    /// Converte os itens do diagnóstico: `api_required` vem das relações `Required` da API,
    /// ligadas pelo `project_id` ao outro item da mesma fonte.
    #[must_use]
    pub fn from_pack_items(items: &[PackItem]) -> Vec<Self> {
        use crate::pretest::ApiRelationKind;

        let by_project: HashMap<(crate::Source, &str), &str> = items
            .iter()
            .filter_map(|item| {
                let api = item.api.as_ref()?;
                Some(((api.source, api.project_id.as_str()), item.path.as_str()))
            })
            .collect();
        items
            .iter()
            .map(|item| {
                let mut required: Vec<String> = Vec::new();
                if let Some(api) = &item.api {
                    for relation in &api.relations {
                        if relation.kind != ApiRelationKind::Required {
                            continue;
                        }
                        if let Some(path) =
                            by_project.get(&(api.source, relation.project_id.as_str()))
                            && *path != item.path
                            && !required.iter().any(|known| known == path)
                        {
                            required.push((*path).to_owned());
                        }
                    }
                }
                Self {
                    path: item.path.clone(),
                    name: item.filename.clone(),
                    jar: item.jar.clone(),
                    api_required: required,
                }
            })
            .collect()
    }
}

/// Quando e se o usuário adicionou um item.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct Addition {
    /// Data da adição, como texto já formatado para a interface (`2026-08-12`), se conhecida.
    #[serde(default)]
    pub at: Option<String>,
}

/// Histórico de adições: os itens que o **usuário** escolheu (por oposição aos que o Warden
/// trouxe como dependência). Quem monta a entrada garante que os itens anteriores ao histórico
/// também constem aqui; item fora do histórico é tratado como dependência.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AdditionHistory {
    /// Caminho do item → adição.
    pub entries: BTreeMap<String, Addition>,
}

/// Entrada do grafo.
#[derive(Debug, Clone)]
pub struct GraphInput {
    /// Loader do pack (decide quais descritores do jar valem).
    pub loader: Loader,
    /// Itens do pack.
    pub items: Vec<GraphItem>,
    /// Histórico de adições, quando existe.
    pub additions: Option<AdditionHistory>,
    /// Arestas inferidas guardadas para o pack.
    pub inferred: Vec<InferredEdge>,
}

/// Mod declarado por um item (no jar de cima).
#[derive(Debug, Clone)]
pub(crate) struct ModNode {
    pub(crate) id: String,
}

/// Mod embutido dentro de um item (jar-in-jar).
#[derive(Debug, Clone)]
pub(crate) struct EmbeddedNode {
    pub(crate) id: String,
}

/// Quem satisfaz uma dependência.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Provider {
    /// Índice do item que traz o mod (ele mesmo ou o que o embute).
    pub(crate) item: usize,
    /// Id do mod que satisfaz.
    pub(crate) mod_id: String,
    /// O mod está embutido no item (jar-in-jar).
    pub(crate) embedded: bool,
    /// Satisfaz por `provides`, não pelo id próprio.
    pub(crate) alias: bool,
}

/// Situação de uma dependência declarada.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum State {
    /// Algum item do pack satisfaz.
    InPack,
    /// O próprio item embute quem satisfaz.
    Own,
    /// Ninguém no pack satisfaz.
    Missing,
}

/// Uma relação declarada por um item (ou inferida).
#[derive(Debug, Clone)]
pub(crate) struct Requirement {
    pub(crate) id: String,
    pub(crate) kind: RelationKind,
    pub(crate) range: Option<String>,
    pub(crate) providers: Vec<Provider>,
    pub(crate) state: State,
    /// Nota da inferência, quando a relação é inferida.
    pub(crate) note: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct ItemNode {
    pub(crate) path: String,
    pub(crate) name: String,
    pub(crate) mods: Vec<ModNode>,
    pub(crate) embedded: Vec<EmbeddedNode>,
    pub(crate) library: bool,
    pub(crate) requirements: Vec<Requirement>,
}

/// O grafo de dependências de um pack. Construído por [`Graph::build`]; as consultas não
/// alteram nada.
#[derive(Debug, Clone)]
pub struct Graph {
    pub(crate) items: Vec<ItemNode>,
    pub(crate) by_path: HashMap<String, usize>,
    /// Itens que cada item exige (obrigatória ou inferida), sem repetir e sem ele mesmo.
    pub(crate) needs: Vec<Vec<usize>>,
    /// O inverso de `needs`.
    pub(crate) needed_by: Vec<Vec<usize>>,
    /// Componente fortemente conexo de cada item (ciclos).
    pub(crate) component: Vec<usize>,
    pub(crate) additions: Option<AdditionHistory>,
}

impl Graph {
    /// Monta o grafo.
    #[must_use]
    pub fn build(input: &GraphInput) -> Self {
        build::build(input)
    }

    /// Quantidade de itens.
    #[must_use]
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// `true` se o pack não tem itens.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Ciclos de dependência: grupos de dois ou mais itens que exigem uns aos outros. Cada
    /// grupo conta como um nó só nas consultas.
    #[must_use]
    pub fn cycles(&self) -> Vec<Vec<NodeRef>> {
        let mut groups: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
        for (index, component) in self.component.iter().enumerate() {
            groups.entry(*component).or_default().push(index);
        }
        let mut cycles: Vec<Vec<NodeRef>> = groups
            .into_values()
            .filter(|members| members.len() > 1)
            .map(|members| members.into_iter().map(|index| self.node(index)).collect())
            .collect();
        cycles.sort_by(|a, b| a[0].path.cmp(&b[0].path));
        cycles
    }
}
