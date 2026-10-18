//! A entrada da busca a partir do grafo da D-07 (ARCHITECTURE §9.6 e §9.7 item 1; S-R5-3 §14
//! item 8).
//!
//! As exigências vêm de [`Graph::dependents`] ("Depende de" de cada item): só as obrigatórias e
//! as inferidas que algum item do pack satisfaz, com todos os fornecedores (qualquer um
//! basta). O grafo já trata o que o spike pediu dos descritores: no `neoforge.mods.toml`, a
//! dependência sem `type` é obrigatória (e no `mods.toml` do Forge vale `mandatory`), e num
//! pack NeoForge com o Sinytra Connector o `fabric.mod.json` dos mods Fabric é lido. O que
//! falta lá e entra aqui: `fabric-api` e `fabric_api` são o mesmo mod (a Forgified Fabric API
//! declara `fabric_api`; os mods Fabric pedem `fabric-api`), então uma dependência que o grafo
//! deu como faltando é procurada de novo pelo id normalizado (minúsculas, `-` = `_`), nos mods
//! de cima e nos embutidos de cada jar.

use std::collections::{BTreeMap, HashSet};

use crate::universe::{ItemSpec, Requirement};
use warden_diagnostics::graph::{DependsOnState, Graph, GraphInput};

/// Ids de mods que são ferramentas do jogador (desligadas na busca, salvo suspeitas; R5A
/// §3.5, S-R5-3 §14 item 12), além das marcadas em `.warden/project.toml`.
pub const KNOWN_PLAYER_TOOLS: &[&str] = &["spark", "crash_assistant", "notenoughcrashes"];

/// O id como o Connector compara (`fabric-api` = `fabric_api`).
#[must_use]
pub fn normalize_id(id: &str) -> String {
    id.trim().to_ascii_lowercase().replace('-', "_")
}

/// Ids normalizados dos mods de cada item (o jar de cima e os embutidos, de todos os
/// descritores: o Connector carrega mods Fabric num pack NeoForge).
#[must_use]
pub fn mod_ids(input: &GraphInput) -> BTreeMap<String, Vec<String>> {
    let mut by_id: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for item in &input.items {
        let Some(jar) = &item.jar else { continue };
        let mut seen: HashSet<String> = HashSet::new();
        for (_, meta) in jar.walk() {
            for module in &meta.mods {
                let ids = std::iter::once(&module.id).chain(&module.provides);
                for id in ids {
                    let id = normalize_id(id);
                    if seen.insert(id.clone()) {
                        by_id.entry(id).or_default().push(item.path.clone());
                    }
                }
            }
        }
    }
    by_id
}

/// Itens e exigências da busca. `player_tools`: caminhos marcados em `.warden/project.toml`.
#[must_use]
pub fn from_graph(
    graph: &Graph,
    input: &GraphInput,
    player_tools: &[String],
) -> (Vec<ItemSpec>, Vec<Requirement>) {
    let whole = graph.whole();
    let library: HashSet<&str> = whole
        .nodes
        .iter()
        .filter(|node| node.library)
        .filter_map(|node| node.item.as_ref().map(|item| item.path.as_str()))
        .collect();
    let ids = mod_ids(input);
    let tool_paths: HashSet<String> = KNOWN_PLAYER_TOOLS
        .iter()
        .filter_map(|id| ids.get(*id))
        .flatten()
        .cloned()
        .chain(player_tools.iter().cloned())
        .collect();
    let mut items = Vec::new();
    let mut requirements = Vec::new();
    for item in &input.items {
        items.push(ItemSpec {
            path: item.path.clone(),
            name: item.name.clone(),
            library: library.contains(item.path.as_str()),
            player_tool: tool_paths.contains(&item.path),
        });
        let Ok(report) = graph.dependents(std::slice::from_ref(&item.path)) else {
            continue;
        };
        for dependency in report.depends_on {
            if !dependency.kind.is_needed() {
                continue;
            }
            let any_of: Vec<String> = match dependency.state {
                DependsOnState::InPack => dependency
                    .providers
                    .iter()
                    .map(|provider| provider.item.path.clone())
                    .collect(),
                DependsOnState::Own => continue,
                DependsOnState::Missing => ids
                    .get(&normalize_id(&dependency.id))
                    .cloned()
                    .unwrap_or_default(),
            };
            let any_of: Vec<String> = any_of
                .into_iter()
                .filter(|path| *path != item.path)
                .collect();
            if !any_of.is_empty() {
                requirements.push(Requirement {
                    from: item.path.clone(),
                    any_of,
                });
            }
        }
    }
    (items, requirements)
}

#[cfg(test)]
mod tests {
    use std::io::Write as _;

    use warden_diagnostics::graph::GraphItem;
    use warden_jarmeta::{JarMetadata, Limits, Loader, read_jar_bytes};

    use super::*;

    /// Um jar com os arquivos dados (descritores reais lidos pela `warden-jarmeta`).
    fn jar(files: &[(&str, &str)]) -> JarMetadata {
        let mut buffer = std::io::Cursor::new(Vec::new());
        {
            let mut zip = zip::ZipWriter::new(&mut buffer);
            for (name, content) in files {
                zip.start_file(*name, zip::write::SimpleFileOptions::default())
                    .unwrap();
                zip.write_all(content.as_bytes()).unwrap();
            }
            zip.finish().unwrap();
        }
        read_jar_bytes(&buffer.into_inner(), &Limits::default()).unwrap()
    }

    fn neoforge(id: &str, deps: &str) -> JarMetadata {
        jar(&[(
            "META-INF/neoforge.mods.toml",
            &format!(
                "modLoader = \"javafml\"\nloaderVersion = \"[4,)\"\nlicense = \"MIT\"\n[[mods]]\nmodId = \"{id}\"\nversion = \"1.0\"\n{deps}"
            ),
        )])
    }

    fn fabric(id: &str, depends: &str) -> JarMetadata {
        jar(&[(
            "fabric.mod.json",
            &format!(
                "{{\"schemaVersion\":1,\"id\":\"{id}\",\"version\":\"1.0\",\"depends\":{{{depends}}}}}"
            ),
        )])
    }

    fn item(path: &str, jar: JarMetadata) -> GraphItem {
        GraphItem {
            path: path.into(),
            name: path.into(),
            jar: Some(jar),
            api_required: Vec::new(),
        }
    }

    /// CA-T25-08 (grafo): dependência sem `type` no `neoforge.mods.toml` é obrigatória; num
    /// pack com o Sinytra Connector, o `fabric.mod.json` entra e `fabric-api` = `fabric_api`.
    #[test]
    fn ca_t25_08_dependencia_sem_type_e_connector() {
        let input = GraphInput {
            loader: Loader::NeoForge,
            items: vec![
                item(
                    "mods/supplementaries.pw.toml",
                    neoforge(
                        "supplementaries",
                        "[[dependencies.supplementaries]]\nmodId = \"moonlight\"\nversionRange = \"[1,)\"\n",
                    ),
                ),
                item("mods/moonlight.pw.toml", neoforge("moonlight", "")),
                item(
                    "mods/continuity.pw.toml",
                    fabric("continuity", "\"fabric-api\": \"*\""),
                ),
                item("mods/forgified.pw.toml", neoforge("fabric_api", "")),
                item("mods/spark.pw.toml", neoforge("spark", "")),
            ],
            additions: None,
            inferred: Vec::new(),
        };
        let graph = Graph::build(&input);
        let (items, requirements) = from_graph(&graph, &input, &[]);
        assert!(requirements.contains(&Requirement {
            from: "mods/supplementaries.pw.toml".into(),
            any_of: vec!["mods/moonlight.pw.toml".into()],
        }));
        assert!(requirements.contains(&Requirement {
            from: "mods/continuity.pw.toml".into(),
            any_of: vec!["mods/forgified.pw.toml".into()],
        }));
        let spark = items
            .iter()
            .find(|item| item.path == "mods/spark.pw.toml")
            .unwrap();
        assert!(spark.player_tool);

        // Na ordem da busca, quem é exigido vem antes; todo prefixo tem as dependências.
        let universe = crate::universe::Universe::new(items, &requirements, &[]);
        let order = universe.unit_paths(universe.order());
        let position = |path: &str| order.iter().position(|known| known == path).unwrap();
        assert!(position("mods/moonlight.pw.toml") < position("mods/supplementaries.pw.toml"));
        assert!(position("mods/forgified.pw.toml") < position("mods/continuity.pw.toml"));
        assert!(!order.contains(&"mods/spark.pw.toml".to_owned()));
    }

    #[test]
    fn ids_normalizados() {
        assert_eq!(normalize_id("Fabric-API"), "fabric_api");
    }
}
