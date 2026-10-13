//! "Abrir no VS Code" (SPEC T26; ARCHITECTURE §10.2): procura o comando `code` no PATH e nos
//! caminhos padrão de instalação e abre uma pasta com ele.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::error::{Error, Result};

/// Nomes do comando no PATH.
#[cfg(windows)]
const NAMES: [&str; 2] = ["code.cmd", "code.exe"];
#[cfg(not(windows))]
const NAMES: [&str; 1] = ["code"];

/// Caminhos padrão de instalação (além do PATH).
#[must_use]
pub fn default_locations() -> Vec<PathBuf> {
    let mut found = Vec::new();
    if cfg!(windows) {
        for (variable, tail) in [
            ("LOCALAPPDATA", r"Programs\Microsoft VS Code\bin\code.cmd"),
            ("ProgramFiles", r"Microsoft VS Code\bin\code.cmd"),
            ("ProgramFiles(x86)", r"Microsoft VS Code\bin\code.cmd"),
        ] {
            if let Some(base) = std::env::var_os(variable) {
                found.push(PathBuf::from(base).join(tail));
            }
        }
    } else {
        for path in [
            "/usr/bin/code",
            "/usr/local/bin/code",
            "/snap/bin/code",
            "/usr/share/code/bin/code",
            "/Applications/Visual Studio Code.app/Contents/Resources/app/bin/code",
        ] {
            found.push(PathBuf::from(path));
        }
    }
    found
}

/// O `code`: primeiro no PATH dado, depois nos caminhos extras.
#[must_use]
pub fn find_vscode(path_var: Option<&OsStr>, extra: &[PathBuf]) -> Option<PathBuf> {
    let from_path = path_var
        .into_iter()
        .flat_map(std::env::split_paths)
        .flat_map(|dir| NAMES.iter().map(move |name| dir.join(name)));
    from_path
        .chain(extra.iter().cloned())
        .find(|candidate| candidate.is_file())
}

/// Abre a pasta no VS Code (não espera ele fechar).
///
/// # Errors
///
/// [`Error::VsCodeFailed`] se o processo não pôde ser criado.
pub fn open_folder(code: &Path, folder: &Path) -> Result<()> {
    let mut command = Command::new(code);
    command
        .arg(folder)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt as _;
        /// Sem a janela do console do `code.cmd`.
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    command
        .spawn()
        .map(drop)
        .map_err(|error| Error::VsCodeFailed {
            detail: error.to_string(),
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn acha_no_path_e_nos_caminhos_extras() {
        let dir = tempfile::tempdir().unwrap();
        let bin = dir.path().join("bin");
        std::fs::create_dir_all(&bin).unwrap();
        assert_eq!(find_vscode(Some(bin.as_os_str()), &[]), None);
        let extra = dir.path().join("extra-code");
        std::fs::write(&extra, "").unwrap();
        assert_eq!(
            find_vscode(Some(bin.as_os_str()), std::slice::from_ref(&extra)),
            Some(extra.clone())
        );
        let in_path = bin.join(NAMES[0]);
        std::fs::write(&in_path, "").unwrap();
        let path_var = std::env::join_paths([dir.path().join("vazio"), bin.clone()]).unwrap();
        assert_eq!(find_vscode(Some(&path_var), &[extra]), Some(in_path));
        assert!(!default_locations().is_empty() || cfg!(windows));
    }

    #[cfg(unix)]
    #[test]
    fn abre_a_pasta_com_o_comando() {
        use std::os::unix::fs::PermissionsExt as _;

        let dir = tempfile::tempdir().unwrap();
        let out = dir.path().join("abriu.txt");
        let code = dir.path().join("code");
        std::fs::write(
            &code,
            format!("#!/bin/sh\nprintf '%s' \"$1\" > '{}'\n", out.display()),
        )
        .unwrap();
        std::fs::set_permissions(&code, std::fs::Permissions::from_mode(0o755)).unwrap();
        let folder = dir.path().join("instância com espaço");
        open_folder(&code, &folder).unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while !out.is_file() && std::time::Instant::now() < deadline {
            std::thread::yield_now();
        }
        // O arquivo pode existir antes de terminar de ser escrito: espera o conteúdo.
        let mut text = String::new();
        while std::time::Instant::now() < deadline {
            text = std::fs::read_to_string(&out).unwrap_or_default();
            if !text.is_empty() {
                break;
            }
            std::thread::yield_now();
        }
        assert_eq!(text, folder.display().to_string());
        assert!(matches!(
            open_folder(&dir.path().join("nada"), &folder),
            Err(Error::VsCodeFailed { .. })
        ));
    }
}
