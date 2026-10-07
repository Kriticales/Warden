//! `cargo xtask check [--fast]`: o portão de qualidade local (QUALITY §12).
//!
//! Roda todas as etapas mesmo quando uma falha e mostra no fim um resumo com o tempo de cada
//! uma; o código de saída só é 0 se todas passarem.
//!
//! `--fast` (a cada commit, QUALITY §7.2): formatação, clippy, testes unitários das crates
//! alteradas em relação à `main` (inclui o que ainda não foi commitado), lint e typecheck.

use std::collections::BTreeSet;
use std::path::Path;

use anyhow::{Result, bail};

use crate::tasks::check_integration;
use crate::util::{Cmd, desktop_dir, format_duration, timed, warn_long_target, workspace_root};
use crate::{bindings, deps, docs};

type Step = (&'static str, Box<dyn FnOnce() -> Result<()>>);

fn pnpm_script(script: &'static str) -> Box<dyn FnOnce() -> Result<()>> {
    Box::new(move || Cmd::pnpm()?.cwd(desktop_dir()).args(["run", script]).run())
}

fn full_steps() -> Vec<Step> {
    vec![
        (
            "cargo fmt",
            Box::new(|| Cmd::cargo().args(["fmt", "--all", "--check"]).run()),
        ),
        ("cargo clippy", Box::new(clippy)),
        (
            "cargo nextest",
            Box::new(|| {
                Cmd::cargo()
                    .args(["nextest", "run", "--workspace", "--locked"])
                    .run()
            }),
        ),
        (
            "cargo test --doc",
            Box::new(|| {
                Cmd::cargo()
                    .args(["test", "--doc", "--workspace", "--locked"])
                    .run()
            }),
        ),
        (
            "cargo deny",
            Box::new(|| {
                Cmd::cargo()
                    .args(["deny", "--locked", "check", "--hide-inclusion-graph"])
                    .run()
            }),
        ),
        ("check-deps", Box::new(deps::run)),
        ("check-docs", Box::new(docs::run)),
        ("bindings --check", Box::new(|| bindings::run(true))),
        (
            "check-integration",
            Box::new(|| check_integration::run(check_integration::Args { compile: false })),
        ),
        ("pnpm format:check", pnpm_script("format:check")),
        ("pnpm lint", pnpm_script("lint")),
        ("pnpm typecheck", pnpm_script("typecheck")),
        ("pnpm test", pnpm_script("test")),
    ]
}

fn clippy() -> Result<()> {
    Cmd::cargo()
        .args([
            "clippy",
            "--workspace",
            "--all-targets",
            "--all-features",
            "--locked",
        ])
        .args(["--", "-D", "warnings"])
        .run()
}

fn fast_steps() -> Result<Vec<Step>> {
    let packages = changed_packages()?;
    let tests: Box<dyn FnOnce() -> Result<()>> = match packages {
        ChangedPackages::All => Box::new(|| {
            Cmd::cargo()
                .args([
                    "nextest",
                    "run",
                    "--workspace",
                    "--locked",
                    "--lib",
                    "--bins",
                ])
                .run()
        }),
        ChangedPackages::Some(names) if names.is_empty() => Box::new(|| {
            println!("check --fast: nenhuma crate alterada em relação à main.");
            Ok(())
        }),
        ChangedPackages::Some(names) => Box::new(move || {
            let mut cmd = Cmd::cargo().args(["nextest", "run", "--locked", "--lib", "--bins"]);
            cmd = cmd.args(["--no-tests=warn"]);
            for name in &names {
                cmd = cmd.args(["-p", name]);
            }
            cmd.run()
        }),
    };
    Ok(vec![
        (
            "cargo fmt",
            Box::new(|| Cmd::cargo().args(["fmt", "--all", "--check"]).run()),
        ),
        ("cargo clippy", Box::new(clippy)),
        ("testes unitários", tests),
        ("pnpm lint", pnpm_script("lint")),
        ("pnpm typecheck", pnpm_script("typecheck")),
    ])
}

/// Crates cujos testes o `--fast` roda.
#[derive(Debug, PartialEq, Eq)]
pub enum ChangedPackages {
    /// Mudou algo que afeta todas (manifesto da raiz, lockfile, toolchain) ou não há `main`.
    All,
    /// Só estas crates.
    Some(BTreeSet<String>),
}

/// Arquivos da raiz que, alterados, valem como mudança em todas as crates.
const GLOBAL_FILES: &[&str] = &[
    "Cargo.toml",
    "Cargo.lock",
    "rust-toolchain.toml",
    "clippy.toml",
    ".cargo/config.toml",
];

/// Decide as crates a partir dos arquivos alterados e das pastas de cada crate (relativas à
/// raiz, com `/`).
pub fn packages_for(changed: &[String], packages: &[(String, String)]) -> ChangedPackages {
    if changed
        .iter()
        .any(|file| GLOBAL_FILES.contains(&file.as_str()))
    {
        return ChangedPackages::All;
    }
    let mut names = BTreeSet::new();
    for file in changed {
        // A pasta mais específica ganha (a `warden-app` fica dentro de `apps/desktop`).
        let owner = packages
            .iter()
            .filter(|(_, dir)| file.starts_with(&format!("{dir}/")))
            .max_by_key(|(_, dir)| dir.len());
        if let Some((name, _)) = owner {
            names.insert(name.clone());
        }
    }
    ChangedPackages::Some(names)
}

fn changed_packages() -> Result<ChangedPackages> {
    let root = workspace_root();
    let Ok(base) = Cmd::new("git").args(["merge-base", "HEAD", "main"]).read() else {
        println!("check --fast: sem branch main local; rodando os testes de todas as crates.");
        return Ok(ChangedPackages::All);
    };
    let mut changed: Vec<String> = Vec::new();
    for args in [
        vec!["diff", "--name-only", base.trim()],
        vec!["ls-files", "--others", "--exclude-standard"],
    ] {
        let output = Cmd::new("git").args(args).read()?;
        changed.extend(output.lines().map(str::to_owned));
    }
    let metadata = deps::metadata(&root.join("Cargo.toml"))?;
    let packages: Vec<(String, String)> = metadata
        .workspace_packages()
        .iter()
        .filter_map(|package| {
            let dir = package.manifest_path.parent()?.as_std_path();
            let relative = dir.strip_prefix(&root).unwrap_or(Path::new(""));
            Some((
                package.name.to_string(),
                relative.to_string_lossy().replace('\\', "/"),
            ))
        })
        .collect();
    Ok(packages_for(&changed, &packages))
}

/// `cargo xtask check [--fast]`.
pub fn run(fast: bool) -> Result<()> {
    warn_long_target();
    let steps = if fast { fast_steps()? } else { full_steps() };
    let mut results = Vec::new();
    for (name, step) in steps {
        println!("\n=== {name}");
        let (result, duration) = timed(step);
        if let Err(error) = &result {
            println!("FALHOU: {error:#}");
        }
        results.push((name, result.is_ok(), duration));
    }

    let total: std::time::Duration = results.iter().map(|(_, _, duration)| *duration).sum();
    println!("\n=== Resumo do check{}", if fast { " --fast" } else { "" });
    for (name, ok, duration) in &results {
        let status = if *ok { "ok" } else { "FALHOU" };
        println!("  {status:<7} {name:<20} {}", format_duration(*duration));
    }
    println!("  total: {}", format_duration(total));

    let failed: Vec<_> = results
        .iter()
        .filter(|(_, ok, _)| !ok)
        .map(|(name, _, _)| *name)
        .collect();
    if !failed.is_empty() {
        bail!("check: falhou em {}", failed.join(", "));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn packages() -> Vec<(String, String)> {
        [
            ("warden-app", "apps/desktop/src-tauri"),
            ("warden-core", "crates/warden-core"),
            ("warden-packwiz", "crates/warden-packwiz"),
            ("warden-packwiz-cli", "crates/warden-packwiz-cli"),
            ("xtask", "xtask"),
        ]
        .map(|(name, dir)| (name.to_owned(), dir.to_owned()))
        .to_vec()
    }

    fn files(list: &[&str]) -> Vec<String> {
        list.iter().map(|file| (*file).to_owned()).collect()
    }

    #[test]
    fn so_as_crates_alteradas() {
        let changed = files(&[
            "crates/warden-packwiz-cli/src/lib.rs",
            "apps/desktop/src-tauri/src/lib.rs",
            "apps/desktop/src/main.tsx",
            "docs/ROADMAP.md",
        ]);
        let expected: BTreeSet<String> = ["warden-app", "warden-packwiz-cli"]
            .map(String::from)
            .into_iter()
            .collect();
        assert_eq!(
            packages_for(&changed, &packages()),
            ChangedPackages::Some(expected)
        );
    }

    #[test]
    fn manifesto_da_raiz_vale_para_todas() {
        let changed = files(&["Cargo.lock", "crates/warden-core/src/lib.rs"]);
        assert_eq!(packages_for(&changed, &packages()), ChangedPackages::All);
    }

    #[test]
    fn nada_alterado() {
        assert_eq!(
            packages_for(&files(&["README.md"]), &packages()),
            ChangedPackages::Some(BTreeSet::new())
        );
    }
}
