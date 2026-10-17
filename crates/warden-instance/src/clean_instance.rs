//! Instância limpa do "Testar como o jogador recebe" (ROADMAP L-07; ARCHITECTURE §8.5
//! passo 3).
//!
//! Uma pasta temporária `cache/tmp/player-test-<id>/`, separada da instância de teste do pack:
//! começa vazia (sem os mundos e sem as configs da instância de trabalho), recebe o pack pelo
//! instalador do jogador e é apagada no fim do teste. Antes de apagar, os registros do jogo
//! (`logs/`, `crash-reports/` e `hs_err_pid*.log`) são copiados para a pasta da sessão
//! ([`KEPT_DIR`], por [`keep_logs`]), que fica com as outras sessões do pack.
//!
//! Cada teste recria a pasta do zero: sobras de um teste interrompido (o Warden fechou no meio)
//! são apagadas na criação da próxima.
//!
//! ```text
//! cache/tmp/player-test-<id>/
//!   minecraft/   gameDir do jogador (o installer escreve aqui)
//!   state/       estado do Warden (linha de base e afins; some junto)
//!   work/        pasta de trabalho do Java do instalador
//! ```

use std::fs;
use std::path::{Path, PathBuf};

use crate::error::{Error, Result};
use crate::materialize::InstanceDirs;

/// Prefixo das pastas temporárias.
pub const CLEAN_PREFIX: &str = "player-test-";

/// Pasta, dentro da sessão, com os registros do jogo copiados da instância limpa.
pub const KEPT_DIR: &str = "instancia-do-jogador";

/// Pastas do jogo copiadas para a sessão antes de apagar a instância.
const KEPT_FOLDERS: [&str; 2] = ["logs", "crash-reports"];

/// A instância limpa de um teste. Apagada em [`CleanInstance::remove`] ou, se o teste
/// terminar de outro jeito (erro, cancelamento), ao sair de escopo.
#[derive(Debug)]
pub struct CleanInstance {
    root: PathBuf,
    dirs: InstanceDirs,
    removed: bool,
}

impl CleanInstance {
    /// Cria a pasta `<tmp>/player-test-<id>/` vazia, apagando antes as sobras de testes
    /// anteriores.
    ///
    /// Erros: os de disco; [`Error::Internal`] se `id` não for um nome simples.
    pub fn create(tmp_dir: &Path, id: &str) -> Result<Self> {
        if id.is_empty()
            || !id
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        {
            return Err(Error::Internal(format!(
                "id de instância limpa inválido: {id:?}"
            )));
        }
        sweep(tmp_dir)?;
        let root = tmp_dir.join(format!("{CLEAN_PREFIX}{id}"));
        let dirs = InstanceDirs {
            game_dir: root.join("minecraft"),
            state_dir: root.join("state"),
        };
        for dir in [&dirs.game_dir, &dirs.state_dir, &root.join("work")] {
            fs::create_dir_all(dir).map_err(|e| Error::io("criar a pasta", shown(dir), e))?;
        }
        Ok(Self {
            root,
            dirs,
            removed: false,
        })
    }

    /// `gameDir` e `state` da instância limpa.
    #[must_use]
    pub fn dirs(&self) -> &InstanceDirs {
        &self.dirs
    }

    /// Pasta de trabalho do Java do instalador.
    #[must_use]
    pub fn work_dir(&self) -> PathBuf {
        self.root.join("work")
    }

    /// Apaga a instância limpa.
    ///
    /// Erros: os de disco (a pasta pode ficar; a próxima criação tenta de novo).
    pub fn remove(mut self) -> Result<()> {
        self.removed = true;
        remove_dir(&self.root)
    }
}

impl Drop for CleanInstance {
    fn drop(&mut self) {
        if !self.removed
            && let Err(error) = remove_dir(&self.root)
        {
            tracing::warn!(%error, "instância limpa não foi apagada; a próxima tenta de novo");
        }
    }
}

/// O caminho como aparece nas mensagens: a partir de `cache/tmp/player-test-…`, nunca com a
/// pasta do usuário (QUALITY §9).
fn shown(path: &Path) -> PathBuf {
    let components: Vec<_> = path.components().collect();
    let start = components
        .iter()
        .position(|part| part.as_os_str().to_string_lossy().starts_with(CLEAN_PREFIX));
    match start {
        Some(start) => components[start..]
            .iter()
            .fold(PathBuf::from("cache/tmp"), |out, part| out.join(part)),
        None => PathBuf::from(path.file_name().unwrap_or_default()),
    }
}

/// Copia os registros do jogo (da pasta do jogo da instância limpa) para
/// `<sessão>/instancia-do-jogador/`. Devolve os caminhos
/// copiados, relativos à pasta do jogo (com `/`).
///
/// Erros: os de disco.
pub fn keep_logs(game_dir: &Path, session_dir: &Path) -> Result<Vec<String>> {
    let target = session_dir.join(KEPT_DIR);
    let game = game_dir;
    let mut kept = Vec::new();
    for folder in KEPT_FOLDERS {
        copy_tree(&game.join(folder), &target.join(folder), folder, &mut kept)?;
    }
    if let Ok(entries) = fs::read_dir(game) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            let log = Path::new(&name)
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("log"));
            if name.starts_with("hs_err_pid") && log && entry.path().is_file() {
                copy_file(&entry.path(), &target.join(&name))?;
                kept.push(name);
            }
        }
    }
    kept.sort();
    Ok(kept)
}

fn remove_dir(dir: &Path) -> Result<()> {
    match fs::remove_dir_all(dir) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(Error::io("apagar a pasta", shown(dir), error)),
    }
}

/// Apaga as pastas `player-test-*` que sobraram em `tmp` (as que não der, ficam para a próxima).
fn sweep(tmp_dir: &Path) -> Result<()> {
    let entries = match fs::read_dir(tmp_dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(Error::io("ler a pasta", "cache/tmp", error)),
    };
    for entry in entries.flatten() {
        if entry
            .file_name()
            .to_string_lossy()
            .starts_with(CLEAN_PREFIX)
        {
            // Uma sobra presa (outro programa usando um arquivo dela no Windows) não impede o
            // teste novo, que usa outra pasta: fica para a próxima vez.
            match remove_dir(&entry.path()) {
                Ok(()) => tracing::info!("sobra de um teste como o jogador apagada"),
                Err(error) => {
                    tracing::warn!(%error, "sobra de um teste como o jogador não apagada");
                }
            }
        }
    }
    Ok(())
}

fn copy_file(from: &Path, to: &Path) -> Result<()> {
    if let Some(parent) = to.parent() {
        fs::create_dir_all(parent).map_err(|e| Error::io("criar a pasta", shown(parent), e))?;
    }
    fs::copy(from, to).map_err(|e| Error::io("copiar", shown(from), e))?;
    Ok(())
}

/// Copia os arquivos de `from` (se existir) para `to`, anotando os caminhos relativos.
fn copy_tree(from: &Path, to: &Path, relative: &str, kept: &mut Vec<String>) -> Result<()> {
    let Ok(entries) = fs::read_dir(from) else {
        return Ok(());
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let path = entry.path();
        let inner = format!("{relative}/{name}");
        // Sem seguir links: o jogo não cria links nessas pastas.
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        if kind.is_dir() {
            copy_tree(&path, &to.join(&name), &inner, kept)?;
        } else if kind.is_file() {
            copy_file(&path, &to.join(&name))?;
            kept.push(inner);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::temp_dir;

    #[test]
    fn cria_vazia_e_apaga_sobras() {
        let tmp = temp_dir();
        let old = tmp
            .path()
            .join("player-test-velho")
            .join("minecraft")
            .join("saves");
        fs::create_dir_all(&old).unwrap();
        fs::create_dir_all(tmp.path().join("outra-coisa")).unwrap();
        let clean = CleanInstance::create(tmp.path(), "01ABC").unwrap();
        assert!(!tmp.path().join("player-test-velho").exists());
        assert!(
            tmp.path().join("outra-coisa").exists(),
            "só as sobras do teste somem"
        );
        assert!(clean.dirs().game_dir.is_dir());
        assert_eq!(fs::read_dir(&clean.dirs().game_dir).unwrap().count(), 0);
        assert!(clean.work_dir().is_dir());
        let root = tmp.path().join("player-test-01ABC");
        assert!(clean.dirs().game_dir.starts_with(&root));
        clean.remove().unwrap();
        assert!(!root.exists());
    }

    #[test]
    fn apagada_ao_sair_de_escopo() {
        let tmp = temp_dir();
        {
            let clean = CleanInstance::create(tmp.path(), "x1").unwrap();
            fs::write(clean.dirs().game_dir.join("options.txt"), "a").unwrap();
        }
        assert!(!tmp.path().join("player-test-x1").exists());
    }

    #[test]
    fn caminho_das_mensagens_sem_a_pasta_do_usuario() {
        let path = Path::new("/home/fulano/.local/share/Warden/cache/tmp/player-test-1/minecraft");
        assert_eq!(shown(path), Path::new("cache/tmp/player-test-1/minecraft"));
        assert_eq!(shown(Path::new("/home/fulano/x.txt")), Path::new("x.txt"));
    }

    #[test]
    fn id_estranho_recusado() {
        let tmp = temp_dir();
        for id in ["", "../fora", "a/b", "a b"] {
            assert!(CleanInstance::create(tmp.path(), id).is_err(), "{id:?}");
        }
    }

    #[test]
    fn registros_copiados_para_a_sessao() {
        let tmp = temp_dir();
        let clean = CleanInstance::create(tmp.path(), "logs").unwrap();
        let game = &clean.dirs().game_dir;
        fs::create_dir_all(game.join("logs")).unwrap();
        fs::write(game.join("logs/latest.log"), "linha").unwrap();
        fs::create_dir_all(game.join("crash-reports")).unwrap();
        fs::write(game.join("crash-reports/crash-1.txt"), "travou").unwrap();
        fs::write(game.join("hs_err_pid42.log"), "nativo").unwrap();
        fs::create_dir_all(game.join("mods")).unwrap();
        fs::write(game.join("mods/a.jar"), "jar").unwrap();
        let session = tmp.path().join("sessao");
        let kept = keep_logs(game, &session).unwrap();
        assert_eq!(
            kept,
            [
                "crash-reports/crash-1.txt",
                "hs_err_pid42.log",
                "logs/latest.log"
            ]
        );
        let target = session.join(KEPT_DIR);
        assert_eq!(
            fs::read_to_string(target.join("logs/latest.log")).unwrap(),
            "linha"
        );
        assert!(target.join("crash-reports/crash-1.txt").is_file());
        assert!(target.join("hs_err_pid42.log").is_file());
        assert!(!target.join("mods").exists(), "só os registros");
        clean.remove().unwrap();
        assert!(
            target.join("logs/latest.log").is_file(),
            "os registros ficam"
        );
    }
}
