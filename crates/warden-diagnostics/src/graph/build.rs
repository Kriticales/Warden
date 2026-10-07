//! Montagem do grafo a partir da entrada (jars, API e arestas inferidas).

use std::collections::{HashMap, HashSet};

use warden_jarmeta::{DependencyKind, JarMetadata, Loader, VersionRange};

use super::libraries::is_known_library;
use super::{
    EmbeddedNode, Graph, GraphInput, ItemNode, ModNode, Provider, RelationKind, Requirement, State,
};

/// Ids que o loader ou o jogo fornecem: nunca são itens do pack.
fn builtin(id: &str) -> bool {
    matches!(
        id.to_ascii_lowercase().as_str(),
        "minecraft"
            | "java"
            | "fabricloader"
            | "fabric-loader"
            | "quilt_loader"
            | "forge"
            | "neoforge"
            | "fml"
    )
}

fn relation_kind(kind: DependencyKind) -> RelationKind {
    match kind {
        DependencyKind::Required => RelationKind::Required,
        DependencyKind::Optional => RelationKind::Optional,
        DependencyKind::Recommends => RelationKind::Recommended,
        DependencyKind::Suggests => RelationKind::Suggested,
        DependencyKind::Breaks | DependencyKind::Incompatible => RelationKind::Breaks,
        DependencyKind::Conflicts | DependencyKind::Discouraged => RelationKind::Conflicts,
    }
}

/// A faixa como a interface mostra; `None` para "qualquer versão".
fn range_text(range: &VersionRange) -> Option<String> {
    match range {
        VersionRange::Any => None,
        VersionRange::Fabric { predicates } => {
            (!predicates.is_empty()).then(|| predicates.join(" ou "))
        }
        VersionRange::Maven { spec } => Some(spec.clone()),
        VersionRange::Exact { version } => Some(version.clone()),
    }
}

/// Uma relação como o descritor a declara.
struct Declared {
    id: String,
    kind: RelationKind,
    range: Option<String>,
}

/// O que se tira de um jar (e dos jars dentro dele).
#[derive(Default)]
struct Collected {
    /// Mods do jar de cima, com os ids que cada um `provides`.
    mods: Vec<(String, Vec<String>)>,
    /// Mods dos jars embutidos.
    embedded: Vec<(String, Vec<String>)>,
    declared: Vec<Declared>,
    library: bool,
}

fn collect(loader: Loader, jar: &JarMetadata) -> Collected {
    let mut out = Collected::default();
    let mut seen: HashSet<String> = HashSet::new();
    for (prefix, meta) in jar.walk() {
        let top = prefix.is_empty();
        let mods = if top {
            meta.effective_mods_for_loader(loader)
        } else {
            meta.mods_for_loader(loader)
        };
        for module in mods {
            if !seen.insert(module.id.to_ascii_lowercase()) {
                continue;
            }
            let entry = (module.id.clone(), module.provides.clone());
            if top {
                out.mods.push(entry);
            } else {
                out.embedded.push(entry);
            }
            for dependency in &module.dependencies {
                out.declared.push(Declared {
                    id: dependency.id.clone(),
                    kind: relation_kind(dependency.kind),
                    range: range_text(&dependency.range),
                });
            }
        }
    }
    out.library = jar
        .manifest
        .as_ref()
        .and_then(|manifest| manifest.fml_mod_type.as_deref())
        .is_some_and(|kind| matches!(kind, "LIBRARY" | "GAMELIBRARY"))
        || out.mods.iter().any(|(id, _)| is_known_library(id));
    out
}

pub(super) fn build(input: &GraphInput) -> Graph {
    let mut items: Vec<ItemNode> = Vec::new();
    let mut by_path: HashMap<String, usize> = HashMap::new();
    let mut declared: Vec<Vec<Declared>> = Vec::new();
    let mut aliases_of: Vec<Vec<Vec<String>>> = Vec::new();

    for item in &input.items {
        if by_path.contains_key(&item.path) {
            continue;
        }
        let collected = item
            .jar
            .as_ref()
            .map(|jar| collect(input.loader, jar))
            .unwrap_or_default();
        by_path.insert(item.path.clone(), items.len());
        aliases_of.push(
            collected
                .mods
                .iter()
                .chain(&collected.embedded)
                .map(|(_, aliases)| aliases.clone())
                .collect(),
        );
        items.push(ItemNode {
            path: item.path.clone(),
            name: item.name.clone(),
            mods: collected
                .mods
                .iter()
                .map(|(id, _)| ModNode { id: id.clone() })
                .collect(),
            embedded: collected
                .embedded
                .iter()
                .map(|(id, _)| EmbeddedNode { id: id.clone() })
                .collect(),
            library: collected.library,
            requirements: Vec::new(),
        });
        declared.push(collected.declared);
    }

    let index = provider_index(&items, &aliases_of);

    for (i, decls) in declared.into_iter().enumerate() {
        items[i].requirements = resolve_requirements(i, decls, &index);
    }

    // Relações obrigatórias da API: valem para o que o jar não declarou (ou não existe).
    for item in &input.items {
        let Some(&i) = by_path.get(&item.path) else {
            continue;
        };
        for target in &item.api_required {
            let Some(&j) = by_path.get(target) else {
                continue;
            };
            add_edge(&mut items, i, j, RelationKind::Required, None);
        }
    }
    for edge in &input.inferred {
        let (Some(&from), Some(&to)) = (by_path.get(&edge.from), by_path.get(&edge.to)) else {
            continue;
        };
        let note = (!edge.note.is_empty()).then(|| edge.note.clone());
        add_edge(&mut items, from, to, RelationKind::Inferred, note);
    }

    let (needs, needed_by) = adjacency(&items);
    let component = strongly_connected(&needs);

    let additions = input.additions.clone().map(|mut history| {
        history.entries.retain(|path, _| by_path.contains_key(path));
        history
    });
    Graph {
        items,
        by_path,
        needs,
        needed_by,
        component,
        additions,
    }
}

/// Índice id (minúsculas) → quem satisfaz, com os `provides` como apelidos.
fn provider_index(
    items: &[ItemNode],
    aliases_of: &[Vec<Vec<String>>],
) -> HashMap<String, Vec<Provider>> {
    let mut index: HashMap<String, Vec<Provider>> = HashMap::new();
    for (i, node) in items.iter().enumerate() {
        let own = node.mods.iter().map(|m| (&m.id, false));
        let nested = node.embedded.iter().map(|m| (&m.id, true));
        for (position, (id, embedded)) in own.chain(nested).enumerate() {
            let mut push = |key: &str, alias: bool| {
                let known = index.entry(key.to_ascii_lowercase()).or_default();
                let provider = Provider {
                    item: i,
                    mod_id: id.clone(),
                    embedded,
                    alias,
                };
                if !known.contains(&provider) {
                    known.push(provider);
                }
            };
            push(id, false);
            for alias in &aliases_of[i][position] {
                push(alias, true);
            }
        }
    }
    index
}

/// Resolve as relações declaradas por um item contra o índice de fornecedores.
fn resolve_requirements(
    i: usize,
    decls: Vec<Declared>,
    index: &HashMap<String, Vec<Provider>>,
) -> Vec<Requirement> {
    let mut requirements: Vec<Requirement> = Vec::new();
    for decl in decls {
        if builtin(&decl.id) {
            continue;
        }
        let key = decl.id.to_ascii_lowercase();
        if requirements
            .iter()
            .any(|known| known.kind == decl.kind && known.id.eq_ignore_ascii_case(&key))
        {
            continue;
        }
        let all = index.get(&key).map_or(&[][..], Vec::as_slice);
        let own = all.iter().any(|provider| provider.item == i);
        let mut found: Vec<Provider> = all
            .iter()
            .filter(|provider| provider.item != i)
            .cloned()
            .collect();
        found.sort();
        let state = if decl.kind.is_conflict() {
            // Incompatibilidade só interessa quando o outro item está no pack.
            if found.is_empty() {
                continue;
            }
            State::InPack
        } else if own {
            found.clear();
            State::Own
        } else if found.is_empty() {
            State::Missing
        } else {
            State::InPack
        };
        requirements.push(Requirement {
            id: decl.id,
            kind: decl.kind,
            range: decl.range,
            providers: found,
            state,
            note: None,
        });
    }
    requirements
}

/// Quem cada item exige (obrigatória ou inferida) e o inverso, sem repetir.
fn adjacency(items: &[ItemNode]) -> (Vec<Vec<usize>>, Vec<Vec<usize>>) {
    let n = items.len();
    let mut needs: Vec<Vec<usize>> = vec![Vec::new(); n];
    let mut needed_by: Vec<Vec<usize>> = vec![Vec::new(); n];
    for (i, node) in items.iter().enumerate() {
        for requirement in &node.requirements {
            if !requirement.kind.is_needed() || requirement.state != State::InPack {
                continue;
            }
            for provider in &requirement.providers {
                needs[i].push(provider.item);
            }
        }
        needs[i].sort_unstable();
        needs[i].dedup();
        for &j in &needs[i] {
            needed_by[j].push(i);
        }
    }
    for list in &mut needed_by {
        list.sort_unstable();
        list.dedup();
    }
    (needs, needed_by)
}

/// Acrescenta `from → to` a menos que `from` já exija `to` por uma relação declarada.
fn add_edge(
    items: &mut [ItemNode],
    from: usize,
    to: usize,
    kind: RelationKind,
    note: Option<String>,
) {
    if from == to {
        return;
    }
    let already = items[from].requirements.iter().any(|requirement| {
        requirement.kind.is_needed()
            && requirement.state == State::InPack
            && requirement
                .providers
                .iter()
                .any(|provider| provider.item == to)
    });
    if already {
        return;
    }
    let id = items[to]
        .mods
        .first()
        .map_or_else(|| items[to].name.clone(), |m| m.id.clone());
    items[from].requirements.push(Requirement {
        id: id.clone(),
        kind,
        range: None,
        providers: vec![Provider {
            item: to,
            mod_id: id,
            embedded: false,
            alias: false,
        }],
        state: State::InPack,
        note,
    });
}

/// Componentes fortemente conexos (Tarjan iterativo): o número de cada vértice.
fn strongly_connected(adjacency: &[Vec<usize>]) -> Vec<usize> {
    let n = adjacency.len();
    let unset = usize::MAX;
    let mut order = vec![unset; n];
    let mut low = vec![0; n];
    let mut on_stack = vec![false; n];
    let mut component = vec![unset; n];
    let mut stack: Vec<usize> = Vec::new();
    let mut counter = 0;
    let mut components = 0;
    for root in 0..n {
        if order[root] != unset {
            continue;
        }
        // (vértice, próximo vizinho a visitar)
        let mut call: Vec<(usize, usize)> = vec![(root, 0)];
        order[root] = counter;
        low[root] = counter;
        counter += 1;
        stack.push(root);
        on_stack[root] = true;
        while let Some(&(v, next)) = call.last() {
            if let Some(&w) = adjacency[v].get(next) {
                if let Some(top) = call.last_mut() {
                    top.1 += 1;
                }
                if order[w] == unset {
                    order[w] = counter;
                    low[w] = counter;
                    counter += 1;
                    stack.push(w);
                    on_stack[w] = true;
                    call.push((w, 0));
                } else if on_stack[w] {
                    low[v] = low[v].min(order[w]);
                }
            } else {
                call.pop();
                if let Some(&(parent, _)) = call.last() {
                    low[parent] = low[parent].min(low[v]);
                }
                if low[v] == order[v] {
                    while let Some(w) = stack.pop() {
                        on_stack[w] = false;
                        component[w] = components;
                        if w == v {
                            break;
                        }
                    }
                    components += 1;
                }
            }
        }
    }
    component
}
