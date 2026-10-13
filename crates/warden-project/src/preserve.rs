//! Como uma config do pack chega a quem joga (SPEC T12, "Avisos contextuais"; C-04).
//!
//! - [`preserve_of`] e [`set_preserve`]: a opção **Não substituir se o jogador já tiver**, que
//!   grava `preserve = true` na entrada do arquivo no índice. O packwiz-installer então só cria
//!   o arquivo quando ele não existe na pasta do jogador; nas atualizações, o do jogador fica.
//!   O `packwiz refresh` que a [`PackTransaction`] roda depois mantém o campo (teste de
//!   integração com o packwiz e o packwiz-installer reais).
//! - [`copy_to_defaultconfigs`]: **Copiar para defaultconfigs/**. Um `*-server.toml` em
//!   `config/` vale por mundo (Forge/NeoForge 1.13+); em `defaultconfigs/` ele vale para os
//!   mundos novos.

use std::path::Path;

use warden_core::CancellationToken;
use warden_packwiz::edit::set_index_preserve;
use warden_packwiz::{PACK_FILE, PackIndex, PackManifest, clean_path};
use warden_packwiz_cli::Packwiz;

use crate::configs::{ConfigOrigin, ReadOnlyReason, read_file};
use crate::transaction::PackTransaction;
use crate::{Error, ProjectErrorCode as Code, Result};

/// A opção "não substituir" de um arquivo do pack.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct PreserveState {
    /// O arquivo está no índice (só então a opção vale; um arquivo novo entra no índice ao
    /// salvar).
    pub in_index: bool,
    /// `preserve = true` no índice.
    pub preserve: bool,
}

/// O índice do pack: o caminho dele (relativo ao pack) e o texto.
struct IndexFile {
    path: String,
    /// Pasta do índice, relativa ao pack (vazia na raiz), com `/` no fim quando não vazia.
    dir: String,
    text: String,
}

fn read_index(root: &Path) -> Result<IndexFile> {
    let invalid = |detail: String| Error::new(Code::InvalidPack, detail);
    let pack_text = std::fs::read_to_string(root.join(PACK_FILE))
        .map_err(|error| invalid(format!("{PACK_FILE}: {error}")))?;
    let manifest = PackManifest::parse(&pack_text).map_err(|error| invalid(error.to_string()))?;
    let path = clean_path(&manifest.value.index.file);
    let text = std::fs::read_to_string(
        warden_core::resolve_inside(root, &path).map_err(|error| invalid(error.to_string()))?,
    )
    .map_err(|error| invalid(format!("{path}: {error}")))?;
    let dir = path
        .rsplit_once('/')
        .map_or_else(String::new, |(dir, _)| format!("{dir}/"));
    Ok(IndexFile { path, dir, text })
}

/// O caminho do arquivo como o índice escreve (relativo à pasta do índice).
fn index_entry_path(index: &IndexFile, relative: &str) -> Result<String> {
    let relative = clean_path(relative);
    relative
        .strip_prefix(&index.dir)
        .map(str::to_owned)
        .ok_or_else(|| {
            Error::new(
                Code::InvalidInput,
                format!("{relative} fica fora da pasta do índice"),
            )
            .param("field", "path")
        })
}

/// Confere que o caminho é um arquivo de config do pack que o editor mostra.
fn check_config(root: &Path, relative: &str) -> Result<String> {
    Ok(read_file(root, ConfigOrigin::Pack, relative)?.path)
}

/// Se o arquivo está no índice e com `preserve`.
///
/// Erros: `INVALID_INPUT` (caminho fora das configs do pack), `CONFIG_NOT_FOUND`,
/// `INVALID_PACK` (`pack.toml` ou índice ilegíveis).
pub fn preserve_of(root: &Path, relative: &str) -> Result<PreserveState> {
    let normalized = check_config(root, relative)?;
    let index = read_index(root)?;
    let entry = index_entry_path(&index, &normalized)?;
    let parsed = PackIndex::parse(&index.text)
        .map_err(|error| Error::new(Code::InvalidPack, error.to_string()))?;
    Ok(parsed.value.entry(&entry).map_or(
        PreserveState {
            in_index: false,
            preserve: false,
        },
        |found| PreserveState {
            in_index: true,
            preserve: found.preserve,
        },
    ))
}

/// Liga ou desliga `preserve` na entrada do arquivo no índice, pela [`PackTransaction`] (com o
/// `packwiz refresh`). Devolve `false` quando já estava assim (nada é gravado). O chamador
/// segura a trava de escrita do pack.
///
/// Erros: os de [`preserve_of`]; `INVALID_INPUT` se o arquivo não está no índice (ele entra
/// no índice quando é salvo pela primeira vez); `PACK_CHANGED_EXTERNALLY`.
pub async fn set_preserve(
    root: &Path,
    relative: &str,
    preserve: bool,
    packwiz: &Packwiz,
    cancel: &CancellationToken,
) -> Result<bool> {
    let current = preserve_of(root, relative)?;
    if !current.in_index {
        return Err(Error::new(
            Code::InvalidInput,
            format!("{relative} ainda não está no índice do pack"),
        )
        .param("field", "path"));
    }
    if current.preserve == preserve {
        return Ok(false);
    }
    let normalized = check_config(root, relative)?;
    let index = read_index(root)?;
    let entry = index_entry_path(&index, &normalized)?;
    let edited = set_index_preserve(&index.text, &entry, preserve)
        .map_err(|error| Error::new(Code::InvalidPack, error.to_string()))?
        .ok_or_else(|| Error::new(Code::PackChangedExternally, index.path.clone()))?;
    let mut transaction = PackTransaction::new(root.to_path_buf());
    transaction.write(&index.path, edited.into_bytes())?;
    transaction.commit(packwiz, cancel).await?;
    Ok(true)
}

/// `config/<nome>-server.toml` → `defaultconfigs/<nome>-server.toml`; `None` para os demais.
#[must_use]
pub fn defaultconfigs_target(relative: &str) -> Option<String> {
    let relative = clean_path(relative);
    let name = relative.strip_prefix("config/")?;
    (!name.contains('/') && name.to_ascii_lowercase().ends_with("-server.toml"))
        .then(|| format!("defaultconfigs/{name}"))
}

/// Resultado de "Copiar para defaultconfigs/".
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum CopyOutcome {
    /// Copiado (e o índice atualizado).
    Copied {
        /// Caminho do arquivo criado ou substituído.
        target: String,
    },
    /// O destino já tem exatamente o mesmo conteúdo; nada foi gravado.
    AlreadySame {
        /// Caminho do destino.
        target: String,
    },
    /// O destino existe com outro conteúdo; nada foi gravado. Repita com `overwrite`.
    NeedsOverwrite {
        /// Caminho do destino.
        target: String,
    },
}

/// Copia um `config/*-server.toml` do pack para `defaultconfigs/`, pela [`PackTransaction`].
/// Sem `overwrite`, um destino diferente não é tocado ([`CopyOutcome::NeedsOverwrite`]).
///
/// Erros: `INVALID_INPUT` (não é um `config/*-server.toml` do pack), `CONFIG_NOT_FOUND`,
/// `CONFIG_NOT_EDITABLE` (binário, grande demais ou fora de UTF-8), `PACK_CHANGED_EXTERNALLY`.
pub async fn copy_to_defaultconfigs(
    root: &Path,
    relative: &str,
    overwrite: bool,
    packwiz: &Packwiz,
    cancel: &CancellationToken,
) -> Result<CopyOutcome> {
    let source = read_file(root, ConfigOrigin::Pack, relative)?;
    let target = defaultconfigs_target(&source.path).ok_or_else(|| {
        Error::new(
            Code::InvalidInput,
            format!("{} não é um config/*-server.toml", source.path),
        )
        .param("field", "path")
    })?;
    if let Some(reason) = source.read_only {
        let name = match reason {
            ReadOnlyReason::Binary => "binary",
            ReadOnlyReason::TooLarge => "tooLarge",
            ReadOnlyReason::NotUtf8 => "notUtf8",
        };
        return Err(Error::new(
            Code::ConfigNotEditable,
            format!("o arquivo só abre para leitura ({name})"),
        )
        .param("reason", name));
    }
    let source_path = warden_core::resolve_inside(root, &source.path)
        .map_err(|error| Error::new(Code::InvalidInput, error.to_string()))?;
    let bytes = std::fs::read(&source_path)
        .map_err(|error| Error::new(Code::Internal, format!("{}: {error}", source.path)))?;
    let target_path = warden_core::resolve_inside(root, &target)
        .map_err(|error| Error::new(Code::InvalidInput, error.to_string()))?;
    match std::fs::read(&target_path) {
        Ok(existing) if existing == bytes => return Ok(CopyOutcome::AlreadySame { target }),
        Ok(_) if !overwrite => return Ok(CopyOutcome::NeedsOverwrite { target }),
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(Error::new(Code::Internal, format!("{target}: {error}"))),
    }
    let mut transaction = PackTransaction::new(root.to_path_buf());
    transaction.write(&target, bytes)?;
    transaction.commit(packwiz, cancel).await?;
    Ok(CopyOutcome::Copied { target })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn destino_em_defaultconfigs() {
        assert_eq!(
            defaultconfigs_target("config/create-server.toml").as_deref(),
            Some("defaultconfigs/create-server.toml")
        );
        assert_eq!(
            defaultconfigs_target("./config/A-SERVER.TOML").as_deref(),
            Some("defaultconfigs/A-SERVER.TOML")
        );
        assert_eq!(defaultconfigs_target("config/create-common.toml"), None);
        assert_eq!(defaultconfigs_target("config/sub/x-server.toml"), None);
        assert_eq!(defaultconfigs_target("defaultconfigs/x-server.toml"), None);
    }

    fn pack(index_file: &str, index: &str) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let mut manifest = PackManifest::new("Teste", "1.21.1");
        manifest.index.file = index_file.to_owned();
        std::fs::write(dir.path().join(PACK_FILE), manifest.to_toml_string()).unwrap();
        let index_path = dir.path().join(index_file);
        std::fs::create_dir_all(index_path.parent().unwrap()).unwrap();
        std::fs::write(index_path, index).unwrap();
        dir
    }

    #[test]
    fn le_o_preserve_do_indice() {
        let index = "hash-format = \"sha256\"\n\n[[files]]\nfile = \"options.txt\"\nhash = \"a\"\npreserve = true\n\n[[files]]\nfile = \"config/a.toml\"\nhash = \"b\"\n";
        let dir = pack("index.toml", index);
        std::fs::write(dir.path().join("options.txt"), "fov:0.0\n").unwrap();
        std::fs::create_dir_all(dir.path().join("config")).unwrap();
        std::fs::write(dir.path().join("config/a.toml"), "a = 1\n").unwrap();
        std::fs::write(dir.path().join("config/novo.toml"), "a = 1\n").unwrap();
        assert_eq!(
            preserve_of(dir.path(), "options.txt").unwrap(),
            PreserveState {
                in_index: true,
                preserve: true
            }
        );
        assert_eq!(
            preserve_of(dir.path(), "config/a.toml").unwrap(),
            PreserveState {
                in_index: true,
                preserve: false
            }
        );
        assert_eq!(
            preserve_of(dir.path(), "config/novo.toml").unwrap(),
            PreserveState {
                in_index: false,
                preserve: false
            }
        );
        let outside = preserve_of(dir.path(), "../x.txt").unwrap_err();
        assert_eq!(outside.code, Code::InvalidInput);
    }

    #[test]
    fn indice_em_subpasta() {
        let index = "hash-format = \"sha256\"\n\n[[files]]\nfile = \"config/a.toml\"\nhash = \"b\"\npreserve = true\n";
        let dir = pack("dados/index.toml", index);
        std::fs::create_dir_all(dir.path().join("dados/config")).unwrap();
        std::fs::write(dir.path().join("dados/config/a.toml"), "a = 1\n").unwrap();
        std::fs::write(dir.path().join("options.txt"), "fov:0.0\n").unwrap();
        assert!(
            preserve_of(dir.path(), "dados/config/a.toml")
                .unwrap()
                .preserve
        );
        assert_eq!(
            preserve_of(dir.path(), "options.txt").unwrap_err().code,
            Code::InvalidInput
        );
    }

    #[test]
    fn indice_ilegivel() {
        let dir = pack("index.toml", "isto não é toml = = \n");
        std::fs::write(dir.path().join("options.txt"), "fov:0.0\n").unwrap();
        assert_eq!(
            preserve_of(dir.path(), "options.txt").unwrap_err().code,
            Code::InvalidPack
        );
        std::fs::remove_file(dir.path().join(PACK_FILE)).unwrap();
        assert_eq!(
            preserve_of(dir.path(), "options.txt").unwrap_err().code,
            Code::InvalidPack
        );
    }
}
