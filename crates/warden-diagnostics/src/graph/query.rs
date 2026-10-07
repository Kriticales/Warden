//! Consultas do grafo: dependentes transitivos, "Por que está no pack" e bibliotecas sem uso.

use std::collections::{BTreeSet, HashMap, HashSet, VecDeque};
use std::fmt;

use serde::Serialize;

use super::{Graph, Provider, RelationKind, Requirement, State};

/// Quantas cadeias "Por que está no pack" a resposta traz, no máximo.
const MAX_CHAINS: usize = 3;

/// Um item do pack numa resposta.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct NodeRef {
    /// Caminho do item no pack.
    pub path: String,
    /// Nome para mostrar.
    pub name: String,
}

/// O caminho pedido não é de um item do grafo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownItem(pub String);

impl fmt::Display for UnknownItem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "o item {:?} não está no grafo do pack", self.0)
    }
}

impl std::error::Error for UnknownItem {}

/// Situação de uma dependência declarada.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum DependsOnState {
    /// Algum item do pack satisfaz (ou, numa incompatibilidade, o outro item está no pack).
    InPack,
    /// O próprio item traz quem satisfaz (jar-in-jar).
    Own,
    /// Nenhum item do pack satisfaz.
    Missing,
}

/// Quem satisfaz uma dependência.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ProviderRef {
    /// O item do pack (o que embute, se o mod está embutido).
    pub item: NodeRef,
    /// Id do mod que satisfaz.
    pub mod_id: String,
    /// O mod está embutido dentro do item ("dentro de X").
    pub embedded: bool,
    /// Satisfaz por `provides`, com outro id.
    pub alias: bool,
}

/// "Depende de": uma relação que o item declara.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct DependsOn {
    /// Id do mod pedido.
    pub id: String,
    /// Tipo da relação.
    pub kind: RelationKind,
    /// Faixa de versões pedida, quando há.
    pub range: Option<String>,
    /// Se está no pack.
    pub state: DependsOnState,
    /// Quem satisfaz (vazio se falta ou se é do próprio item).
    pub providers: Vec<ProviderRef>,
    /// Nota da inferência, nas relações inferidas.
    pub note: Option<String>,
}

/// "Usado por": um item que declara uma relação com o item consultado.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct UsedBy {
    /// O item que declara a relação.
    pub item: NodeRef,
    /// Tipo da relação.
    pub kind: RelationKind,
    /// Faixa de versões pedida, quando há.
    pub range: Option<String>,
    /// Id que o item pede (o mod consultado ou um id que ele fornece).
    pub id: String,
    /// Quem satisfaz é um mod embutido no item consultado.
    pub embedded: bool,
    /// Nota da inferência, nas relações inferidas.
    pub note: Option<String>,
}

/// Um item que deixa de funcionar quando os alvos saem.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct Affected {
    /// O item.
    pub item: NodeRef,
    /// 1 = depende direto de um alvo; 2 = depende de um item de nível 1; e assim por diante.
    pub depth: u32,
    /// Os itens que somem (alvos ou afetados de nível menor) de que ele precisa.
    pub needs: Vec<NodeRef>,
}

/// Resposta de [`Graph::dependents`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct DependentsReport {
    /// Os itens consultados, na ordem pedida.
    pub targets: Vec<NodeRef>,
    /// O que o item declara (só com um alvo; vazio com vários).
    pub depends_on: Vec<DependsOn>,
    /// Quem declara uma relação com os alvos, de qualquer tipo.
    pub used_by: Vec<UsedBy>,
    /// O que para de funcionar se os alvos saírem: o fecho transitivo das dependências
    /// obrigatórias e inferidas.
    pub affected: Vec<Affected>,
}

/// Um passo da cadeia "Por que está no pack".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ChainLink {
    /// O item.
    pub item: NodeRef,
    /// Os outros itens do ciclo de dependência a que ele pertence (vazio fora de ciclo).
    pub cycle: Vec<NodeRef>,
}

/// Por que um item está no pack.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Why {
    /// O usuário adicionou (pelo histórico).
    #[serde(rename_all = "camelCase")]
    UserAdded {
        /// Quando, se o histórico sabe.
        at: Option<String>,
    },
    /// Sem histórico: nenhum outro item o exige, então foi escolha do usuário.
    NoDependents,
    /// Exigido por outros itens: cada cadeia vai do item até um adicionado pelo usuário,
    /// da mais curta para a mais longa.
    #[serde(rename_all = "camelCase")]
    RequiredBy {
        /// As cadeias (cada uma começa no próprio item e termina no item do usuário).
        chains: Vec<Vec<ChainLink>>,
    },
    /// Nenhum item mantido o exige: pode ser removido.
    #[serde(rename_all = "camelCase")]
    Unused {
        /// Itens que só o usam como opcional, recomendado ou sugerido.
        optional_users: Vec<NodeRef>,
    },
}

/// Resposta de [`Graph::why_in_pack`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct WhyReport {
    /// O item consultado.
    pub item: NodeRef,
    /// A resposta.
    pub why: Why,
    /// Os outros itens do ciclo de dependência do item (vazio fora de ciclo).
    pub cycle: Vec<NodeRef>,
}

/// Uma biblioteca (ou item) que nenhum item mantido usa mais.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct Orphan {
    /// O item.
    pub item: NodeRef,
    /// Itens que só o usam como opcional, recomendado ou sugerido.
    pub optional_users: Vec<NodeRef>,
}

impl Graph {
    pub(super) fn node(&self, index: usize) -> NodeRef {
        NodeRef {
            path: self.items[index].path.clone(),
            name: self.items[index].name.clone(),
        }
    }

    fn sorted_nodes(&self, indexes: impl IntoIterator<Item = usize>) -> Vec<NodeRef> {
        let mut nodes: Vec<NodeRef> = indexes.into_iter().map(|index| self.node(index)).collect();
        nodes.sort_by(|a, b| {
            a.name
                .to_lowercase()
                .cmp(&b.name.to_lowercase())
                .then_with(|| a.path.cmp(&b.path))
        });
        nodes.dedup();
        nodes
    }

    fn index_of(&self, path: &str) -> std::result::Result<usize, UnknownItem> {
        self.by_path
            .get(path)
            .copied()
            .ok_or_else(|| UnknownItem(path.to_owned()))
    }

    fn provider_ref(&self, provider: &Provider) -> ProviderRef {
        ProviderRef {
            item: self.node(provider.item),
            mod_id: provider.mod_id.clone(),
            embedded: provider.embedded,
            alias: provider.alias,
        }
    }

    /// Os outros membros do ciclo de dependência do item.
    fn cycle_of(&self, index: usize) -> Vec<NodeRef> {
        let component = self.component[index];
        self.sorted_nodes(
            (0..self.items.len())
                .filter(|&other| other != index && self.component[other] == component),
        )
    }

    /// O que depende dos itens, de forma transitiva, e as relações diretas deles.
    ///
    /// `affected` é o ponto fixo de: um item não removido quebra quando alguma dependência
    /// obrigatória ou inferida dele tem fornecedores no pack e **todos** já saíram. Dependência
    /// que o próprio item satisfaz (jar-in-jar dele) ou que já faltava não conta. `provides` e
    /// jar-in-jar valem: um mod embutido em outro item sai junto com esse item.
    pub fn dependents(
        &self,
        paths: &[String],
    ) -> std::result::Result<DependentsReport, UnknownItem> {
        let mut targets: Vec<usize> = Vec::new();
        for path in paths {
            let index = self.index_of(path)?;
            if !targets.contains(&index) {
                targets.push(index);
            }
        }
        let target_set: HashSet<usize> = targets.iter().copied().collect();

        let depends_on = match targets.as_slice() {
            [only] => self.dependencies_of(*only),
            _ => Vec::new(),
        };
        let used_by = self.used_by(&target_set);
        let affected = self.affected(&target_set);
        Ok(DependentsReport {
            targets: targets.iter().map(|&index| self.node(index)).collect(),
            depends_on,
            used_by,
            affected,
        })
    }

    fn dependencies_of(&self, index: usize) -> Vec<DependsOn> {
        let mut list: Vec<DependsOn> = self.items[index]
            .requirements
            .iter()
            .map(|requirement| DependsOn {
                id: requirement.id.clone(),
                kind: requirement.kind,
                range: requirement.range.clone(),
                state: match requirement.state {
                    State::InPack => DependsOnState::InPack,
                    State::Own => DependsOnState::Own,
                    State::Missing => DependsOnState::Missing,
                },
                providers: requirement
                    .providers
                    .iter()
                    .map(|provider| self.provider_ref(provider))
                    .collect(),
                note: requirement.note.clone(),
            })
            .collect();
        list.sort_by(|a, b| {
            a.kind
                .cmp(&b.kind)
                .then_with(|| a.id.to_lowercase().cmp(&b.id.to_lowercase()))
        });
        list
    }

    fn used_by(&self, targets: &HashSet<usize>) -> Vec<UsedBy> {
        let mut list: Vec<UsedBy> = Vec::new();
        for (index, node) in self.items.iter().enumerate() {
            if targets.contains(&index) {
                continue;
            }
            for requirement in &node.requirements {
                let Some(provider) = requirement
                    .providers
                    .iter()
                    .find(|provider| targets.contains(&provider.item))
                else {
                    continue;
                };
                let dependent = UsedBy {
                    item: self.node(index),
                    kind: requirement.kind,
                    range: requirement.range.clone(),
                    id: requirement.id.clone(),
                    embedded: provider.embedded,
                    note: requirement.note.clone(),
                };
                if !list.contains(&dependent) {
                    list.push(dependent);
                }
            }
        }
        list.sort_by(|a, b| {
            a.kind
                .cmp(&b.kind)
                .then_with(|| a.item.name.to_lowercase().cmp(&b.item.name.to_lowercase()))
                .then_with(|| a.item.path.cmp(&b.item.path))
        });
        list
    }

    fn affected(&self, targets: &HashSet<usize>) -> Vec<Affected> {
        let mut gone: HashSet<usize> = targets.clone();
        let mut result: Vec<Affected> = Vec::new();
        let mut depth = 0_u32;
        loop {
            depth += 1;
            let mut round: Vec<(usize, BTreeSet<usize>)> = Vec::new();
            for (index, node) in self.items.iter().enumerate() {
                if gone.contains(&index) {
                    continue;
                }
                let mut missing: BTreeSet<usize> = BTreeSet::new();
                for requirement in &node.requirements {
                    if broken_by(requirement, &gone) {
                        missing.extend(
                            requirement
                                .providers
                                .iter()
                                .map(|provider| provider.item)
                                .filter(|item| gone.contains(item)),
                        );
                    }
                }
                if !missing.is_empty() {
                    round.push((index, missing));
                }
            }
            if round.is_empty() {
                break;
            }
            let mut batch: Vec<Affected> = round
                .iter()
                .map(|(index, missing)| Affected {
                    item: self.node(*index),
                    depth,
                    needs: self.sorted_nodes(missing.iter().copied()),
                })
                .collect();
            batch.sort_by(|a, b| {
                a.item
                    .name
                    .to_lowercase()
                    .cmp(&b.item.name.to_lowercase())
                    .then_with(|| a.item.path.cmp(&b.item.path))
            });
            result.extend(batch);
            gone.extend(round.into_iter().map(|(index, _)| index));
        }
        result
    }

    /// Quem o usuário escolheu: o histórico, ou, sem ele, os itens que nenhum outro exige
    /// (o grupo inteiro de um ciclo sem entradas de fora) e que não são bibliotecas.
    fn roots(&self) -> Vec<bool> {
        let count = self.items.len();
        if let Some(history) = &self.additions {
            return self
                .items
                .iter()
                .map(|node| history.entries.contains_key(&node.path))
                .collect();
        }
        let mut has_outside_user = vec![false; self.component.iter().max().map_or(0, |m| m + 1)];
        for (index, users) in self.needed_by.iter().enumerate() {
            if users
                .iter()
                .any(|&user| self.component[user] != self.component[index])
            {
                has_outside_user[self.component[index]] = true;
            }
        }
        (0..count)
            .map(|index| !has_outside_user[self.component[index]] && !self.items[index].library)
            .collect()
    }

    /// Itens que algum item escolhido pelo usuário mantém, direta ou indiretamente.
    fn kept(&self, roots: &[bool]) -> Vec<bool> {
        let mut kept = roots.to_vec();
        let mut queue: VecDeque<usize> = (0..roots.len()).filter(|&i| roots[i]).collect();
        while let Some(index) = queue.pop_front() {
            for &needed in &self.needs[index] {
                if !kept[needed] {
                    kept[needed] = true;
                    queue.push_back(needed);
                }
            }
        }
        kept
    }

    /// Itens que nenhum item escolhido pelo usuário mantém: bibliotecas que sobraram.
    #[must_use]
    pub fn orphans(&self) -> Vec<Orphan> {
        let roots = self.roots();
        let kept = self.kept(&roots);
        let mut orphans: Vec<Orphan> = (0..self.items.len())
            .filter(|&index| !kept[index])
            .map(|index| Orphan {
                item: self.node(index),
                optional_users: self.optional_users(index),
            })
            .collect();
        orphans.sort_by(|a, b| {
            a.item
                .name
                .to_lowercase()
                .cmp(&b.item.name.to_lowercase())
                .then_with(|| a.item.path.cmp(&b.item.path))
        });
        orphans
    }

    fn optional_users(&self, index: usize) -> Vec<NodeRef> {
        self.sorted_nodes(self.items.iter().enumerate().filter_map(|(user, node)| {
            (user != index
                && node.requirements.iter().any(|requirement| {
                    matches!(
                        requirement.kind,
                        RelationKind::Optional
                            | RelationKind::Recommended
                            | RelationKind::Suggested
                    ) && requirement
                        .providers
                        .iter()
                        .any(|provider| provider.item == index)
                }))
            .then_some(user)
        }))
    }

    /// Por que o item está no pack (CA-T07-03).
    pub fn why_in_pack(&self, path: &str) -> std::result::Result<WhyReport, UnknownItem> {
        let index = self.index_of(path)?;
        let roots = self.roots();
        let cycle = self.cycle_of(index);
        let why = if roots[index] {
            match &self.additions {
                Some(history) => Why::UserAdded {
                    at: history.entries.get(path).and_then(|a| a.at.clone()),
                },
                None => Why::NoDependents,
            }
        } else {
            let chains = self.chains_to_roots(index, &roots);
            if chains.is_empty() {
                Why::Unused {
                    optional_users: self.optional_users(index),
                }
            } else {
                Why::RequiredBy { chains }
            }
        };
        Ok(WhyReport {
            item: self.node(index),
            why,
            cycle,
        })
    }

    /// Cadeias mais curtas do item até os itens do usuário, subindo por quem exige quem.
    fn chains_to_roots(&self, start: usize, roots: &[bool]) -> Vec<Vec<ChainLink>> {
        let mut parent: HashMap<usize, usize> = HashMap::new();
        let mut seen: HashSet<usize> = HashSet::from([start]);
        let mut queue: VecDeque<usize> = VecDeque::from([start]);
        let mut found: Vec<usize> = Vec::new();
        while let Some(current) = queue.pop_front() {
            for &user in &self.needed_by[current] {
                if !seen.insert(user) {
                    continue;
                }
                parent.insert(user, current);
                if roots[user] {
                    // A cadeia termina no primeiro item do usuário.
                    found.push(user);
                } else {
                    queue.push_back(user);
                }
            }
            if found.len() >= MAX_CHAINS {
                break;
            }
        }
        found
            .into_iter()
            .take(MAX_CHAINS)
            .map(|root| {
                let mut path = vec![root];
                let mut at = root;
                while let Some(&below) = parent.get(&at) {
                    path.push(below);
                    at = below;
                }
                path.reverse();
                path.into_iter()
                    .map(|index| ChainLink {
                        item: self.node(index),
                        cycle: self.cycle_of(index),
                    })
                    .collect()
            })
            .collect()
    }
}

/// A relação quebra quando os itens em `gone` saem: é necessária, tem fornecedores no pack e
/// todos eles saíram.
fn broken_by(requirement: &Requirement, gone: &HashSet<usize>) -> bool {
    requirement.kind.is_needed()
        && requirement.state == State::InPack
        && !requirement.providers.is_empty()
        && requirement
            .providers
            .iter()
            .all(|provider| gone.contains(&provider.item))
}
