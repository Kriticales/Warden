//! `cargo xtask check-deps`: nenhuma crate do workspace fora da `warden-app` pode ter o
//! `tauri` na árvore de dependências (ARCHITECTURE §2; ADR-0002).
//!
//! Vale para dependências normais, de build e de desenvolvimento das crates do workspace,
//! diretas ou transitivas.

use std::collections::{BTreeMap, VecDeque};
use std::path::Path;

use anyhow::{Context as _, Result, bail};
use cargo_metadata::{Metadata, MetadataCommand, PackageId};

use crate::util::{cargo_program, workspace_root};

/// Pacote proibido fora das crates permitidas.
pub const FORBIDDEN: &str = "tauri";

/// Crates do workspace que podem depender do Tauri.
pub const ALLOWED: &[&str] = &["warden-app"];

/// Uma crate do workspace que chega ao pacote proibido.
#[derive(Debug, PartialEq, Eq)]
pub struct Violation {
    /// Nome da crate do workspace.
    pub krate: String,
    /// Caminho de dependências, da crate até o pacote proibido.
    pub chain: Vec<String>,
}

/// Procura crates do workspace (fora de `allowed`) que chegam a `forbidden`.
pub fn violations(
    metadata: &Metadata,
    forbidden: &str,
    allowed: &[&str],
) -> Result<Vec<Violation>> {
    let resolve = metadata
        .resolve
        .as_ref()
        .context("o cargo metadata veio sem o grafo de dependências")?;
    let nodes: BTreeMap<&PackageId, &cargo_metadata::Node> =
        resolve.nodes.iter().map(|node| (&node.id, node)).collect();
    let name_of = |id: &PackageId| -> String {
        metadata
            .packages
            .iter()
            .find(|package| &package.id == id)
            .map_or_else(|| id.repr.clone(), |package| package.name.to_string())
    };

    let mut found = Vec::new();
    for member in &metadata.workspace_members {
        let name = name_of(member);
        if allowed.contains(&name.as_str()) {
            continue;
        }
        // Busca em largura, guardando de onde veio cada pacote para montar o caminho.
        let mut parent: BTreeMap<&PackageId, &PackageId> = BTreeMap::new();
        let mut queue = VecDeque::from([member]);
        let mut hit = None;
        while let Some(id) = queue.pop_front() {
            if id != member && name_of(id) == forbidden {
                hit = Some(id);
                break;
            }
            let Some(node) = nodes.get(id) else { continue };
            for dep in &node.deps {
                if &dep.pkg != member && !parent.contains_key(&dep.pkg) {
                    parent.insert(&dep.pkg, id);
                    queue.push_back(&dep.pkg);
                }
            }
        }
        if let Some(mut id) = hit {
            let mut chain = vec![name_of(id)];
            while let Some(previous) = parent.get(id) {
                chain.push(name_of(previous));
                id = previous;
            }
            chain.reverse();
            found.push(Violation { krate: name, chain });
        }
    }
    Ok(found)
}

/// Lê o grafo do workspace em `manifest`.
pub fn metadata(manifest: &Path) -> Result<Metadata> {
    MetadataCommand::new()
        .cargo_path(cargo_program())
        .manifest_path(manifest)
        .exec()
        .context("falha ao rodar `cargo metadata`")
}

/// Confere o workspace em `manifest`; erro com a lista de violações, se houver.
pub fn check_manifest(manifest: &Path) -> Result<()> {
    let found = violations(&metadata(manifest)?, FORBIDDEN, ALLOWED)?;
    if found.is_empty() {
        println!("check-deps: nenhuma crate fora de {ALLOWED:?} depende de `{FORBIDDEN}`.");
        return Ok(());
    }
    let lines: Vec<String> = found
        .iter()
        .map(|violation| format!("  {}: {}", violation.krate, violation.chain.join(" -> ")))
        .collect();
    bail!(
        "check-deps: {} crate(s) dependem de `{FORBIDDEN}`, que só a warden-app pode usar \
         (ARCHITECTURE §2):\n{}",
        found.len(),
        lines.join("\n")
    )
}

/// `cargo xtask check-deps`.
pub fn run() -> Result<()> {
    check_manifest(&workspace_root().join("Cargo.toml"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// Workspace descartável, só com dependências por caminho (o `cargo metadata` não precisa
    /// de rede). O pacote `tauri` dele é falso: só o nome importa para a regra.
    fn fixture(domain_deps: &str) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let write = |path: &str, text: &str| {
            let path = dir.path().join(path);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, text).unwrap();
        };
        write(
            "Cargo.toml",
            "[workspace]\nresolver = \"3\"\nmembers = [\"crates/*\", \"apps/app\"]\n",
        );
        let package = |name: &str, deps: &str| {
            format!(
                "[package]\nname = \"{name}\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\
                 publish = false\n\n{deps}\n"
            )
        };
        write("vendor/tauri/Cargo.toml", &package("tauri", ""));
        write("vendor/tauri/src/lib.rs", "");
        write(
            "vendor/usa-tauri/Cargo.toml",
            &package(
                "usa-tauri",
                "[dependencies]\ntauri = { path = \"../tauri\" }",
            ),
        );
        write("vendor/usa-tauri/src/lib.rs", "");
        write(
            "apps/app/Cargo.toml",
            &package(
                "warden-app",
                "[dependencies]\ntauri = { path = \"../../vendor/tauri\" }",
            ),
        );
        write("apps/app/src/lib.rs", "");
        write("crates/warden-core/Cargo.toml", &package("warden-core", ""));
        write("crates/warden-core/src/lib.rs", "");
        write(
            "crates/warden-dominio/Cargo.toml",
            &package("warden-dominio", domain_deps),
        );
        write("crates/warden-dominio/src/lib.rs", "");
        dir
    }

    fn check(dir: &tempfile::TempDir) -> Vec<Violation> {
        let metadata = MetadataCommand::new()
            .cargo_path(cargo_program())
            .manifest_path(dir.path().join("Cargo.toml"))
            .other_options(vec!["--offline".to_owned()])
            .exec()
            .unwrap();
        violations(&metadata, FORBIDDEN, ALLOWED).unwrap()
    }

    #[test]
    fn f0_01_criterio_3_falha_quando_crate_de_dominio_declara_tauri() {
        let dir = fixture(
            "[dependencies]\nwarden-core = { path = \"../warden-core\" }\n\
             tauri = { path = \"../../vendor/tauri\" }",
        );
        assert_eq!(
            check(&dir),
            [Violation {
                krate: "warden-dominio".into(),
                chain: vec!["warden-dominio".into(), "tauri".into()],
            }]
        );
        let error = check_manifest(&dir.path().join("Cargo.toml")).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("warden-dominio: warden-dominio -> tauri"),
            "{error}"
        );
    }

    #[test]
    fn falha_com_tauri_transitivo_e_em_dev_dependencies() {
        let transitive =
            fixture("[dependencies]\nusa-tauri = { path = \"../../vendor/usa-tauri\" }");
        assert_eq!(
            check(&transitive)[0].chain,
            ["warden-dominio", "usa-tauri", "tauri"]
        );

        let dev = fixture("[dev-dependencies]\ntauri = { path = \"../../vendor/tauri\" }");
        assert_eq!(check(&dev)[0].krate, "warden-dominio");
    }

    #[test]
    fn passa_quando_so_a_warden_app_usa_tauri() {
        let dir = fixture("[dependencies]\nwarden-core = { path = \"../warden-core\" }");
        assert!(check(&dir).is_empty());
        check_manifest(&dir.path().join("Cargo.toml")).unwrap();
    }
}
