//! Salvar um script do pack com o jogo aberto (SPEC T26): o texto vai para o pack e é copiado
//! para a instância de teste, para que "Recarregar no jogo" use o que acabou de ser salvo.
//!
//! A cópia nunca apaga, sem perguntar, uma mudança feita direto na instância: ela só substitui
//! o arquivo da instância quando ele ainda é igual ao do pack **antes** de salvar (o `hash` da
//! leitura em que o editor se baseou), quando não existe ou quando a pessoa escolhe substituir.
//! Se a instância tem um texto próprio, nada é gravado e quem chamou pergunta.

use std::fs;
use std::io::ErrorKind;
use std::path::Path;

use serde::Serialize;
use sha2::{Digest as _, Sha256};
use warden_core::{atomic_write, resolve_inside};

use crate::detect::ScriptFile;
use crate::{Error, Result};

/// O que aconteceu com o arquivo da instância de teste.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum InstanceCopy {
    /// A instância recebeu o texto do pack.
    Copied,
    /// A instância já tinha o mesmo texto.
    Unchanged,
    /// O arquivo da instância foi mudado direto nela: nada foi gravado.
    InstanceChanged,
    /// A instância de teste ainda não existe.
    NoInstance,
}

/// SHA-256 em hexadecimal (o mesmo `hash` do editor de Configs).
fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn io(relative: &str, error: &std::io::Error) -> Error {
    Error::Internal(format!("{relative}: {error}"))
}

/// Copia o script `relative` (relativo ao pack, com `/`) do pack para a pasta do jogo da
/// instância (`game`).
///
/// - `previous_hash`: o `hash` do arquivo do pack antes de salvar. A instância igual a ele não
///   tem mudança própria e recebe o texto novo.
/// - `overwrite`: a pessoa escolheu substituir o texto da instância.
///
/// # Errors
///
/// [`Error::Internal`] se o caminho não é de um script ou se a leitura ou a gravação falha (a
/// mensagem leva só o caminho relativo).
pub fn copy_to_instance(
    pack_root: &Path,
    game: &Path,
    relative: &str,
    previous_hash: &str,
    overwrite: bool,
) -> Result<InstanceCopy> {
    if ScriptFile::classify(relative).is_none() {
        return Err(Error::Internal(format!("{relative} não é um script")));
    }
    if !game.is_dir() {
        return Ok(InstanceCopy::NoInstance);
    }
    // As mensagens levam só o caminho relativo: a pasta do usuário não sai daqui.
    let outside = |_| Error::Internal(format!("{relative}: caminho fora da pasta"));
    let from = resolve_inside(pack_root, relative).map_err(outside)?;
    let to = resolve_inside(game, relative).map_err(outside)?;
    let bytes = fs::read(&from).map_err(|error| io(relative, &error))?;
    match fs::read(&to) {
        Ok(current) => {
            if current == bytes {
                return Ok(InstanceCopy::Unchanged);
            }
            if !overwrite && hash(&current) != previous_hash {
                return Ok(InstanceCopy::InstanceChanged);
            }
        }
        Err(error) if error.kind() == ErrorKind::NotFound => {
            if let Some(parent) = to.parent() {
                fs::create_dir_all(parent).map_err(|error| io(relative, &error))?;
            }
        }
        Err(error) => return Err(io(relative, &error)),
    }
    atomic_write(&to, &bytes).map_err(|error| match error {
        warden_core::CoreError::Io { source, .. } => io(relative, &source),
        _ => Error::Internal(format!("{relative}: falha ao gravar na instância")),
    })?;
    Ok(InstanceCopy::Copied)
}

#[cfg(test)]
mod tests {
    use super::*;

    const PATH: &str = "kubejs/server_scripts/receitas.js";

    struct Dirs {
        _dir: tempfile::TempDir,
        pack: std::path::PathBuf,
        game: std::path::PathBuf,
    }

    fn dirs() -> Dirs {
        let dir = tempfile::tempdir().unwrap();
        let pack = dir.path().join("pack");
        let game = dir.path().join("instancia/minecraft");
        fs::create_dir_all(pack.join("kubejs/server_scripts")).unwrap();
        fs::create_dir_all(game.join("kubejs/server_scripts")).unwrap();
        Dirs {
            _dir: dir,
            pack,
            game,
        }
    }

    fn put(root: &Path, text: &str) {
        fs::write(root.join(PATH), text).unwrap();
    }

    fn read(root: &Path) -> String {
        fs::read_to_string(root.join(PATH)).unwrap()
    }

    #[test]
    fn copia_quando_a_instancia_era_igual_ao_pack_de_antes() {
        let d = dirs();
        put(&d.game, "antigo");
        put(&d.pack, "novo");
        let before = hash(b"antigo");
        let result = copy_to_instance(&d.pack, &d.game, PATH, &before, false).unwrap();
        assert_eq!(result, InstanceCopy::Copied);
        assert_eq!(read(&d.game), "novo");
        // De novo: já igual.
        let result = copy_to_instance(&d.pack, &d.game, PATH, &before, false).unwrap();
        assert_eq!(result, InstanceCopy::Unchanged);
    }

    #[test]
    fn nao_apaga_mudanca_feita_direto_na_instancia() {
        let d = dirs();
        put(&d.game, "mexido na instância");
        put(&d.pack, "novo");
        let before = hash(b"antigo");
        let result = copy_to_instance(&d.pack, &d.game, PATH, &before, false).unwrap();
        assert_eq!(result, InstanceCopy::InstanceChanged);
        assert_eq!(read(&d.game), "mexido na instância");
        // Só com a escolha da pessoa.
        let result = copy_to_instance(&d.pack, &d.game, PATH, &before, true).unwrap();
        assert_eq!(result, InstanceCopy::Copied);
        assert_eq!(read(&d.game), "novo");
    }

    #[test]
    fn script_novo_e_instancia_que_nao_existe() {
        let d = dirs();
        let path = "kubejs/startup_scripts/novo/itens.js";
        fs::create_dir_all(d.pack.join("kubejs/startup_scripts/novo")).unwrap();
        fs::write(d.pack.join(path), "x").unwrap();
        let result = copy_to_instance(&d.pack, &d.game, path, "", false).unwrap();
        assert_eq!(result, InstanceCopy::Copied);
        assert_eq!(fs::read_to_string(d.game.join(path)).unwrap(), "x");

        let missing = d.game.parent().unwrap().join("outra");
        let result = copy_to_instance(&d.pack, &missing, PATH, "", false).unwrap();
        assert_eq!(result, InstanceCopy::NoInstance);
        assert!(!missing.exists());
    }

    #[test]
    fn recusa_o_que_nao_e_script_e_caminho_para_fora() {
        let d = dirs();
        for path in [
            "config/a.toml",
            "kubejs/config/web_server.json",
            "../kubejs/server_scripts/a.js",
            "kubejs/server_scripts/../../../fora.js",
        ] {
            let error = copy_to_instance(&d.pack, &d.game, path, "", true);
            assert!(error.is_err(), "{path}");
        }
    }
}
