//! Os itens da busca, o papel de cada um e a ordem topológica estável (ARCHITECTURE §9.7
//! itens 1 e 2; ADR-0031).
//!
//! - **Papéis:** sempre ligado (bibliotecas de que todos os mods precisam), desligado
//!   (ferramentas do jogador, salvo suspeitas ou exigidas por outro item; S-R5-3 §14 item 12)
//!   e procurado (o resto).
//! - **Exigências:** cada item tem grupos "um destes" (uma dependência com dois fornecedores no
//!   pack é satisfeita por qualquer um deles, como no grafo da D-07).
//! - **Unidades:** um ciclo de dependências obrigatórias vira uma unidade só (os membros
//!   entram e saem juntos).
//! - **Ordem:** topológica estável: uma unidade só entra depois de algum fornecedor de cada
//!   exigência; entre as prontas, bibliotecas primeiro e depois o nome em ordem alfabética.
//!   Assim **todo prefixo da ordem já contém as dependências** dos seus itens (CA-T25-01).

use std::collections::{BTreeSet, HashMap};

use serde::{Deserialize, Serialize};

/// Um item na entrada da busca.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemSpec {
    /// Caminho do item no pack (`mods/create.pw.toml`), a identidade dele.
    pub path: String,
    /// Nome para mostrar.
    pub name: String,
    /// É uma biblioteca (vai antes na ordem e pode ficar sempre ligada).
    pub library: bool,
    /// Ferramenta do jogador (spark, Crash Assistant, Not Enough Crashes): desligada na busca.
    pub player_tool: bool,
}

/// Uma exigência: `from` precisa de **um** dos itens de `any_of`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Requirement {
    /// Caminho do item que precisa.
    pub from: String,
    /// Caminhos dos itens que satisfazem.
    pub any_of: Vec<String>,
}

/// Papel de um item na busca.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum Role {
    /// Entra nas rodadas.
    Searched,
    /// Ligado em todas as rodadas (bibliotecas que todos exigem).
    AlwaysOn,
    /// Desligado em todas as rodadas (ferramentas do jogador).
    Disabled,
}

/// Os itens, papéis, exigências e a ordem da busca.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Universe {
    items: Vec<ItemSpec>,
    roles: Vec<Role>,
    /// Por item: os grupos "um destes" (índices de itens), sem itens desligados.
    groups: Vec<Vec<Vec<usize>>>,
    /// Unidades (ciclos juntos), cada uma com os itens em ordem de índice.
    units: Vec<Vec<usize>>,
    /// Unidade de cada item.
    unit_of: Vec<usize>,
    /// As unidades procuradas, na ordem da busca.
    order: Vec<usize>,
}

impl Universe {
    /// Monta a busca. `suspects`: itens que o usuário quer manter na busca mesmo sendo
    /// ferramentas do jogador. Exigências que citam itens desconhecidos são ignoradas.
    #[must_use]
    pub fn new(items: Vec<ItemSpec>, requirements: &[Requirement], suspects: &[String]) -> Self {
        let mut items = items;
        // Sem repetir caminhos (o primeiro vale).
        let mut seen = BTreeSet::new();
        items.retain(|item| seen.insert(item.path.clone()));
        let index: HashMap<&str, usize> = items
            .iter()
            .enumerate()
            .map(|(position, item)| (item.path.as_str(), position))
            .collect();
        let mut raw: Vec<Vec<Vec<usize>>> = vec![Vec::new(); items.len()];
        for requirement in requirements {
            let Some(&from) = index.get(requirement.from.as_str()) else {
                continue;
            };
            let mut group: Vec<usize> = requirement
                .any_of
                .iter()
                .filter_map(|path| index.get(path.as_str()).copied())
                .filter(|&to| to != from)
                .collect();
            group.sort_unstable();
            group.dedup();
            if !group.is_empty() && !raw[from].contains(&group) {
                raw[from].push(group);
            }
        }
        let roles = roles(&items, &raw, suspects);
        // Fornecedor desligado não conta quando há outro; um grupo só de desligados não
        // acontece (quem é exigido não é desligado).
        let groups: Vec<Vec<Vec<usize>>> = raw
            .iter()
            .map(|groups| {
                groups
                    .iter()
                    .map(|group| {
                        group
                            .iter()
                            .copied()
                            .filter(|&to| roles[to] != Role::Disabled)
                            .collect::<Vec<_>>()
                    })
                    .filter(|group| !group.is_empty())
                    .collect()
            })
            .collect();
        let (units, unit_of) = components(items.len(), &groups);
        let mut universe = Self {
            items,
            roles,
            groups,
            units,
            unit_of,
            order: Vec::new(),
        };
        universe.order = universe.topological_order();
        universe
    }

    /// Os itens.
    #[must_use]
    pub fn items(&self) -> &[ItemSpec] {
        &self.items
    }

    /// O papel do item `index`.
    #[must_use]
    pub fn role(&self, index: usize) -> Role {
        self.roles.get(index).copied().unwrap_or(Role::Disabled)
    }

    /// Índice do item com o caminho.
    #[must_use]
    pub fn index_of(&self, path: &str) -> Option<usize> {
        self.items.iter().position(|item| item.path == path)
    }

    /// Quantas unidades a busca procura.
    #[must_use]
    pub fn searched_units(&self) -> usize {
        self.order.len()
    }

    /// A ordem da busca (unidades).
    #[must_use]
    pub fn order(&self) -> &[usize] {
        &self.order
    }

    /// Os itens de uma unidade.
    #[must_use]
    pub fn unit_items(&self, unit: usize) -> &[usize] {
        self.units.get(unit).map_or(&[], Vec::as_slice)
    }

    /// A unidade do item.
    #[must_use]
    pub fn unit_of(&self, item: usize) -> Option<usize> {
        self.unit_of.get(item).copied()
    }

    /// Os itens com o papel, em ordem de índice.
    #[must_use]
    pub fn with_role(&self, role: Role) -> Vec<usize> {
        (0..self.items.len())
            .filter(|&index| self.roles[index] == role)
            .collect()
    }

    /// Os grupos "um destes" do item.
    #[must_use]
    pub fn groups_of(&self, item: usize) -> &[Vec<usize>] {
        self.groups.get(item).map_or(&[], Vec::as_slice)
    }

    /// Acrescenta uma exigência (dependência inferida: `from` precisa de `to`) e recalcula
    /// as unidades e a ordem. Devolve `false` se ela já existia ou cita item desconhecido.
    pub fn add_requirement(&mut self, from: &str, to: &str) -> bool {
        let (Some(from), Some(to)) = (self.index_of(from), self.index_of(to)) else {
            return false;
        };
        if from == to || self.groups[from].iter().any(|group| group == &vec![to]) {
            return false;
        }
        self.groups[from].push(vec![to]);
        // Um item exigido não fica desligado.
        if self.roles[to] == Role::Disabled {
            self.roles[to] = Role::Searched;
        }
        let (units, unit_of) = components(self.items.len(), &self.groups);
        self.units = units;
        self.unit_of = unit_of;
        self.order = self.topological_order();
        true
    }

    /// Ordem topológica estável das unidades procuradas: uma unidade fica pronta quando cada
    /// exigência dos seus itens tem um fornecedor sempre ligado, na própria unidade ou já
    /// colocado; entre as prontas, bibliotecas primeiro, depois o nome.
    fn topological_order(&self) -> Vec<usize> {
        let searched: Vec<usize> = (0..self.units.len())
            .filter(|&unit| {
                self.units[unit]
                    .iter()
                    .all(|&item| self.roles[item] == Role::Searched)
            })
            .collect();
        let mut placed_items: Vec<bool> = self
            .roles
            .iter()
            .map(|role| *role == Role::AlwaysOn)
            .collect();
        let mut remaining: Vec<usize> = searched;
        let mut order = Vec::with_capacity(remaining.len());
        while !remaining.is_empty() {
            let ready = remaining
                .iter()
                .copied()
                .filter(|&unit| self.unit_ready(unit, &placed_items))
                .min_by(|&a, &b| self.unit_key(a).cmp(&self.unit_key(b)));
            // Sem nenhuma pronta (não acontece com as unidades fortemente conexas), a de
            // menor chave entra para a ordem não parar.
            let next = ready.or_else(|| {
                remaining
                    .iter()
                    .copied()
                    .min_by(|&a, &b| self.unit_key(a).cmp(&self.unit_key(b)))
            });
            let Some(next) = next else { break };
            remaining.retain(|&unit| unit != next);
            for &item in &self.units[next] {
                placed_items[item] = true;
            }
            order.push(next);
        }
        order
    }

    fn unit_ready(&self, unit: usize, placed: &[bool]) -> bool {
        self.units[unit].iter().all(|&item| {
            self.groups[item].iter().all(|group| {
                group
                    .iter()
                    .any(|&to| placed[to] || self.unit_of[to] == unit)
            })
        })
    }

    /// Chave de desempate: bibliotecas primeiro, depois o nome sem maiúsculas e o caminho.
    fn unit_key(&self, unit: usize) -> (bool, String, String) {
        let first = self.units[unit].first().copied().unwrap_or_default();
        let item = &self.items[first];
        let library = self.units[unit]
            .iter()
            .all(|&member| self.items[member].library);
        (!library, item.name.to_lowercase(), item.path.clone())
    }

    /// Os itens ligados com estas unidades: as unidades, os sempre ligados e as dependências
    /// que faltarem (o fornecedor mais cedo na ordem), em ordem de índice.
    #[must_use]
    pub fn closure(&self, units: &[usize]) -> Vec<usize> {
        let mut on: Vec<bool> = self
            .roles
            .iter()
            .map(|role| *role == Role::AlwaysOn)
            .collect();
        let mut queue: Vec<usize> = Vec::new();
        for &unit in units {
            for &item in self.unit_items(unit) {
                if !on[item] {
                    on[item] = true;
                    queue.push(item);
                }
            }
        }
        let always: Vec<usize> = self.with_role(Role::AlwaysOn);
        queue.extend(always);
        let position = self.positions();
        while let Some(item) = queue.pop() {
            for group in &self.groups[item] {
                if group.iter().any(|&to| on[to]) {
                    continue;
                }
                let Some(&provider) = group
                    .iter()
                    .min_by_key(|&&to| position[self.unit_of[to]].unwrap_or(usize::MAX))
                else {
                    continue;
                };
                for &member in &self.units[self.unit_of[provider]] {
                    if !on[member] {
                        on[member] = true;
                        queue.push(member);
                    }
                }
            }
        }
        (0..on.len()).filter(|&item| on[item]).collect()
    }

    /// Posição de cada unidade na ordem (`None` = não procurada).
    fn positions(&self) -> Vec<Option<usize>> {
        let mut position = vec![None; self.units.len()];
        for (at, &unit) in self.order.iter().enumerate() {
            position[unit] = Some(at);
        }
        position
    }

    /// As unidades procuradas que deixam de funcionar sem estas (fecho dos dependentes): uma
    /// unidade cai quando alguma exigência dela só tinha fornecedores que caíram.
    #[must_use]
    pub fn dependents(&self, units: &[usize]) -> Vec<usize> {
        let mut gone = vec![false; self.items.len()];
        for &unit in units {
            for &item in self.unit_items(unit) {
                gone[item] = true;
            }
        }
        let mut changed = true;
        while changed {
            changed = false;
            for item in 0..self.items.len() {
                if gone[item] || self.roles[item] != Role::Searched {
                    continue;
                }
                let broken = self.groups[item]
                    .iter()
                    .any(|group| group.iter().all(|&to| gone[to]));
                if broken {
                    for &member in &self.units[self.unit_of[item]] {
                        gone[member] = true;
                    }
                    changed = true;
                }
            }
        }
        let start: BTreeSet<usize> = units.iter().copied().collect();
        self.order
            .iter()
            .copied()
            .filter(|unit| !start.contains(unit))
            .filter(|&unit| self.units[unit].iter().any(|&item| gone[item]))
            .collect()
    }

    /// Nomes de uma lista de unidades, em ordem.
    #[must_use]
    pub fn unit_names(&self, units: &[usize]) -> Vec<String> {
        units
            .iter()
            .flat_map(|&unit| self.unit_items(unit).iter())
            .map(|&item| self.items[item].name.clone())
            .collect()
    }

    /// Caminhos de uma lista de unidades, em ordem.
    #[must_use]
    pub fn unit_paths(&self, units: &[usize]) -> Vec<String> {
        units
            .iter()
            .flat_map(|&unit| self.unit_items(unit).iter())
            .map(|&item| self.items[item].path.clone())
            .collect()
    }
}

/// Os papéis: ferramentas do jogador desligadas (salvo suspeitas ou exigidas por um item que
/// fica); bibliotecas que todos os mods procurados exigem (de forma transitiva) ficam sempre
/// ligadas; o resto é procurado.
fn roles(items: &[ItemSpec], groups: &[Vec<Vec<usize>>], suspects: &[String]) -> Vec<Role> {
    let mut roles: Vec<Role> = items
        .iter()
        .map(|item| {
            if item.player_tool && !suspects.contains(&item.path) {
                Role::Disabled
            } else {
                Role::Searched
            }
        })
        .collect();
    // Quem é exigido (só por ele) por um item que fica não pode ser desligado.
    let mut changed = true;
    while changed {
        changed = false;
        for from in 0..items.len() {
            if roles[from] == Role::Disabled {
                continue;
            }
            for group in &groups[from] {
                if group.iter().all(|&to| roles[to] == Role::Disabled)
                    && let Some(&to) = group.first()
                {
                    roles[to] = Role::Searched;
                    changed = true;
                }
            }
        }
    }
    // Bibliotecas exigidas por todos os mods procurados (precisa haver pelo menos dois).
    let mods: Vec<usize> = (0..items.len())
        .filter(|&index| roles[index] == Role::Searched && !items[index].library)
        .collect();
    if mods.len() >= 2 {
        let needs: Vec<BTreeSet<usize>> = mods
            .iter()
            .map(|&item| required_closure(item, groups))
            .collect();
        let mut common: BTreeSet<usize> = needs.first().cloned().unwrap_or_default();
        for set in needs.iter().skip(1) {
            common = common.intersection(set).copied().collect();
        }
        for item in common {
            if items[item].library && roles[item] == Role::Searched {
                roles[item] = Role::AlwaysOn;
            }
        }
    }
    roles
}

/// Itens que `item` exige com certeza (grupos de um fornecedor só), de forma transitiva.
fn required_closure(item: usize, groups: &[Vec<Vec<usize>>]) -> BTreeSet<usize> {
    let mut out = BTreeSet::new();
    let mut stack = vec![item];
    while let Some(next) = stack.pop() {
        for group in &groups[next] {
            if let [only] = group.as_slice()
                && out.insert(*only)
            {
                stack.push(*only);
            }
        }
    }
    out.remove(&item);
    out
}

/// Componentes fortemente conexos (Tarjan iterativo) sobre as ligações "precisa de" com todos
/// os fornecedores: cada ciclo vira uma unidade.
fn components(len: usize, groups: &[Vec<Vec<usize>>]) -> (Vec<Vec<usize>>, Vec<usize>) {
    let edges: Vec<Vec<usize>> = (0..len)
        .map(|item| {
            let mut out: Vec<usize> = groups[item].iter().flatten().copied().collect();
            out.sort_unstable();
            out.dedup();
            out
        })
        .collect();
    let mut index_of: Vec<Option<usize>> = vec![None; len];
    let mut low = vec![0usize; len];
    let mut on_stack = vec![false; len];
    let mut stack: Vec<usize> = Vec::new();
    let mut counter = 0usize;
    let mut units: Vec<Vec<usize>> = Vec::new();
    for root in 0..len {
        if index_of[root].is_some() {
            continue;
        }
        // (nó, próxima aresta)
        let mut work: Vec<(usize, usize)> = vec![(root, 0)];
        index_of[root] = Some(counter);
        low[root] = counter;
        counter += 1;
        stack.push(root);
        on_stack[root] = true;
        while let Some(&mut (node, ref mut next)) = work.last_mut() {
            if let Some(&to) = edges[node].get(*next) {
                *next += 1;
                match index_of[to] {
                    None => {
                        index_of[to] = Some(counter);
                        low[to] = counter;
                        counter += 1;
                        stack.push(to);
                        on_stack[to] = true;
                        work.push((to, 0));
                    }
                    Some(found) if on_stack[to] => low[node] = low[node].min(found),
                    Some(_) => {}
                }
                continue;
            }
            work.pop();
            if let Some(&(parent, _)) = work.last() {
                low[parent] = low[parent].min(low[node]);
            }
            if Some(low[node]) == index_of[node] {
                let mut unit = Vec::new();
                while let Some(member) = stack.pop() {
                    on_stack[member] = false;
                    unit.push(member);
                    if member == node {
                        break;
                    }
                }
                unit.sort_unstable();
                units.push(unit);
            }
        }
    }
    units.sort_by_key(|unit| unit.first().copied().unwrap_or_default());
    let mut unit_of = vec![0usize; len];
    for (position, unit) in units.iter().enumerate() {
        for &item in unit {
            unit_of[item] = position;
        }
    }
    (units, unit_of)
}

#[cfg(test)]
mod tests {
    use super::*;

    pub(crate) fn spec(path: &str, library: bool) -> ItemSpec {
        ItemSpec {
            path: path.into(),
            name: path.into(),
            library,
            player_tool: false,
        }
    }

    fn req(from: &str, any_of: &[&str]) -> Requirement {
        Requirement {
            from: from.into(),
            any_of: any_of.iter().map(|&path| path.to_owned()).collect(),
        }
    }

    fn order_paths(universe: &Universe) -> Vec<String> {
        universe.unit_paths(universe.order())
    }

    #[test]
    fn bibliotecas_primeiro_e_dependencia_antes_de_quem_precisa() {
        let items = vec![
            spec("zeta", false),
            spec("alfa", false),
            spec("lib", true),
            spec("base", false),
        ];
        let universe = Universe::new(items, &[req("alfa", &["base"])], &[]);
        assert_eq!(order_paths(&universe), ["lib", "base", "alfa", "zeta"]);
    }

    #[test]
    fn biblioteca_de_todos_fica_sempre_ligada() {
        let items = vec![spec("a", false), spec("b", false), spec("api", true)];
        let universe = Universe::new(items, &[req("a", &["api"]), req("b", &["api"])], &[]);
        assert_eq!(universe.role(2), Role::AlwaysOn);
        assert_eq!(order_paths(&universe), ["a", "b"]);
        // A rodada de controle liga só os sempre ligados.
        assert_eq!(universe.closure(&[]), [2]);
    }

    #[test]
    fn ferramentas_do_jogador_desligadas_salvo_suspeitas_ou_exigidas() {
        let mut items = vec![
            spec("spark", false),
            spec("nec", false),
            spec("addon", false),
            spec("mod", false),
        ];
        items[0].player_tool = true;
        items[1].player_tool = true;
        let universe = Universe::new(items.clone(), &[req("addon", &["spark"])], &[]);
        assert_eq!(universe.role(0), Role::Searched);
        assert_eq!(universe.role(1), Role::Disabled);
        let universe = Universe::new(items, &[], &["nec".to_owned()]);
        assert_eq!(universe.role(0), Role::Disabled);
        assert_eq!(universe.role(1), Role::Searched);
    }

    #[test]
    fn ciclo_vira_uma_unidade() {
        let items = vec![spec("a", false), spec("b", false), spec("c", false)];
        let universe = Universe::new(items, &[req("a", &["b"]), req("b", &["a"])], &[]);
        assert_eq!(universe.searched_units(), 2);
        assert_eq!(order_paths(&universe), ["a", "b", "c"]);
        let unit = universe.unit_of(0).unwrap();
        assert_eq!(universe.unit_items(unit), [0, 1]);
    }

    #[test]
    fn alternativas_e_dependentes() {
        let items = vec![
            spec("app", false),
            spec("fapi", false),
            spec("forgified", false),
        ];
        let universe = Universe::new(items, &[req("app", &["fapi", "forgified"])], &[]);
        let fapi = universe.unit_of(1).unwrap();
        let forgified = universe.unit_of(2).unwrap();
        // Sem um dos fornecedores, o app continua; sem os dois, cai.
        assert!(universe.dependents(&[fapi]).is_empty());
        let app = universe.unit_of(0).unwrap();
        assert_eq!(universe.dependents(&[fapi, forgified]), [app]);
        // Ligar o app sozinho traz o fornecedor mais cedo na ordem.
        assert_eq!(universe.closure(&[app]), [0, 1]);
    }

    #[test]
    fn exigencia_inferida_reordena() {
        let items = vec![spec("a", false), spec("b", false)];
        let mut universe = Universe::new(items, &[], &[]);
        assert_eq!(order_paths(&universe), ["a", "b"]);
        assert!(universe.add_requirement("a", "b"));
        assert!(!universe.add_requirement("a", "b"));
        assert!(!universe.add_requirement("a", "x"));
        assert_eq!(order_paths(&universe), ["b", "a"]);
    }
}
