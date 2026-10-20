//! Conferência da cópia emendada do `portablemc` em `third_party/portablemc/` (ADR-0062).
//!
//! A cópia é o crate do crates.io com os patches de `patches/`: desfazendo os patches (do último
//! para o primeiro, `git apply -R`), cada arquivo do crate tem de voltar ao SHA-256 registrado
//! em `ORIGINAL.sha256`, e nenhum arquivo pode sobrar ou faltar. Assim nenhuma mudança entra na
//! cópia sem virar patch versionado.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context as _, Result};
use sha2::{Digest as _, Sha256};

use crate::util::{Cmd, workspace_root};

/// Arquivos do Warden na pasta da cópia (não são do crate).
const OURS: &[&str] = &["LICENSE", "ORIGINAL.sha256", "WARDEN.md", "patches"];

/// A pasta da cópia.
fn dir() -> PathBuf {
    workspace_root().join("third_party").join("portablemc")
}

/// `caminho → SHA-256` registrado em `ORIGINAL.sha256` (formato do `sha256sum`).
fn recorded(text: &str) -> Result<BTreeMap<String, String>> {
    text.lines()
        .filter(|line| !line.trim().is_empty() && !line.starts_with('#'))
        .map(|line| {
            let (hash, path) = line
                .split_once("  ")
                .with_context(|| format!("linha inválida: {line}"))?;
            Ok((path.trim_start_matches("./").to_owned(), hash.to_owned()))
        })
        .collect()
}

/// Copia os arquivos do crate (sem os do Warden) de `from` para `to`.
fn copy_crate(from: &Path, to: &Path, relative: &Path) -> Result<()> {
    for entry in fs::read_dir(from.join(relative))? {
        let entry = entry?;
        let name = relative.join(entry.file_name());
        if relative.as_os_str().is_empty()
            && OURS.contains(&entry.file_name().to_string_lossy().as_ref())
        {
            continue;
        }
        if entry.file_type()?.is_dir() {
            fs::create_dir_all(to.join(&name))?;
            copy_crate(from, to, &name)?;
        } else {
            fs::copy(entry.path(), to.join(&name))?;
        }
    }
    Ok(())
}

/// `caminho → SHA-256` de todos os arquivos em `root`.
fn digests(root: &Path, relative: &Path, out: &mut BTreeMap<String, String>) -> Result<()> {
    for entry in fs::read_dir(root.join(relative))? {
        let entry = entry?;
        let name = relative.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            digests(root, &name, out)?;
        } else {
            let bytes = fs::read(entry.path())?;
            let key = name.to_string_lossy().replace('\\', "/");
            out.insert(key, format!("{:x}", Sha256::digest(&bytes)));
        }
    }
    Ok(())
}

/// A cópia com os patches desfeitos.
fn unpatched() -> Result<BTreeMap<String, String>> {
    let source = dir();
    let temp = tempfile::tempdir()?;
    copy_crate(&source, temp.path(), Path::new(""))?;
    let mut patches: Vec<PathBuf> = fs::read_dir(source.join("patches"))?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<Result<_, _>>()?;
    patches.sort();
    for patch in patches.iter().rev() {
        // Sem conversão de fim de linha: com `core.autocrlf=true` (o padrão do Git para
        // Windows), o `git apply` gravaria os arquivos desfeitos com CRLF.
        Cmd::new("git")
            .args(["-c", "core.autocrlf=false", "-c", "core.eol=lf"])
            .args(["apply", "-R", "--whitespace=nowarn"])
            .args([patch])
            .cwd(temp.path())
            .read()
            .with_context(|| format!("desfazer {}", patch.display()))?;
    }
    let mut out = BTreeMap::new();
    digests(temp.path(), Path::new(""), &mut out)?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copia_e_o_crate_original_mais_os_patches() {
        let text = fs::read_to_string(dir().join("ORIGINAL.sha256")).unwrap();
        let expected = recorded(&text).unwrap();
        assert!(expected.contains_key("src/base/mod.rs"));
        assert_eq!(unpatched().unwrap(), expected);
    }

    #[test]
    fn leitura_do_registro() {
        let text = "# comentário\nabc  ./src/lib.rs\n\ndef  Cargo.toml\n";
        let map = recorded(text).unwrap();
        assert_eq!(map.get("src/lib.rs").map(String::as_str), Some("abc"));
        assert_eq!(map.get("Cargo.toml").map(String::as_str), Some("def"));
        assert!(recorded("sem-separador").is_err());
    }
}
