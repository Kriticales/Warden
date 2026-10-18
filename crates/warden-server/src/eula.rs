//! A EULA do Minecraft no servidor local (ADR-0032; SPEC T27 e CA-T21-05, CA-T27-02).
//!
//! O aceite é da pessoa, dado uma vez no diálogo da interface, e fica no `settings.json` do
//! app (`eulaAcceptedAt`). Esta crate só traduz o aceite para o `eula.txt` do servidor:
//! [`write_eula`] grava `eula=true` **somente** com o aceite em mãos; sem ele, regrava
//! `eula=false` (um aceite revogado não deixa um `eula=true` velho na pasta) e recusa abrir.

use std::path::Path;

use warden_core::atomic_write;

use crate::error::{Error, Result};

/// O texto oficial da EULA (o link que o diálogo mostra).
pub const EULA_URL: &str = "https://aka.ms/MinecraftEULA";

/// O arquivo que o servidor do Minecraft lê.
pub const EULA_FILE: &str = "eula.txt";

/// Grava o `eula.txt` da pasta do servidor. `accepted_at` é a data do aceite (RFC 3339).
///
/// Erros: [`Error::EulaNotAccepted`] sem o aceite (depois de gravar `eula=false`); disco.
pub fn write_eula(server_dir: &Path, accepted_at: Option<&str>) -> Result<()> {
    let path = server_dir.join(EULA_FILE);
    std::fs::create_dir_all(server_dir)
        .map_err(|error| Error::io("criar a pasta do servidor", server_dir, error))?;
    let accepted = accepted_at.map(str::trim).filter(|date| !date.is_empty());
    let text = match accepted {
        Some(date) => format!(
            "# A EULA do Minecraft ({EULA_URL}) foi aceita no Warden em {date}.\neula=true\n"
        ),
        None => format!(
            "# A EULA do Minecraft ({EULA_URL}) ainda não foi aceita no Warden.\neula=false\n"
        ),
    };
    atomic_write(&path, text.as_bytes())?;
    if accepted.is_none() {
        return Err(Error::EulaNotAccepted);
    }
    Ok(())
}

/// Se o `eula.txt` da pasta diz `eula=true` (o que o servidor lê).
#[must_use]
pub fn eula_file_accepts(server_dir: &Path) -> bool {
    std::fs::read_to_string(server_dir.join(EULA_FILE)).is_ok_and(|text| {
        text.lines()
            .map(str::trim)
            .filter(|line| !line.starts_with('#'))
            .any(|line| {
                line.split_once('=').is_some_and(|(key, value)| {
                    key.trim() == "eula" && value.trim().eq_ignore_ascii_case("true")
                })
            })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ca_t27_02_sem_aceite_nunca_grava_eula_true() {
        let dir = tempfile::tempdir().unwrap();
        let error = write_eula(dir.path(), None).unwrap_err();
        assert!(matches!(error, Error::EulaNotAccepted));
        assert!(!eula_file_accepts(dir.path()));
        assert!(
            std::fs::read_to_string(dir.path().join(EULA_FILE))
                .unwrap()
                .contains("eula=false")
        );
        // Aceite em branco não vale.
        assert!(write_eula(dir.path(), Some("  ")).is_err());
        assert!(!eula_file_accepts(dir.path()));
    }

    #[test]
    fn ca_t21_05_aceite_grava_e_revogar_volta_a_false() {
        let dir = tempfile::tempdir().unwrap();
        write_eula(dir.path(), Some("2026-10-01T17:05:09Z")).unwrap();
        assert!(eula_file_accepts(dir.path()));
        let text = std::fs::read_to_string(dir.path().join(EULA_FILE)).unwrap();
        assert!(text.contains(EULA_URL) && text.contains("2026-10-01T17:05:09Z"));
        // Revogado: o eula=true velho sai da pasta.
        assert!(write_eula(dir.path(), None).is_err());
        assert!(!eula_file_accepts(dir.path()));
    }

    #[test]
    fn le_o_eula_txt_como_o_servidor() {
        let dir = tempfile::tempdir().unwrap();
        assert!(!eula_file_accepts(dir.path()));
        std::fs::write(dir.path().join(EULA_FILE), "#eula=true\neula = TRUE\n").unwrap();
        assert!(eula_file_accepts(dir.path()));
        std::fs::write(dir.path().join(EULA_FILE), "# eula=true\neula=false\n").unwrap();
        assert!(!eula_file_accepts(dir.path()));
    }
}
