//! Jogo simulado para o E2E (só em build de debug; ROADMAP L-04, "E2E com jogo simulado").
//!
//! Com `WARDEN_E2E_FAKE_GAME` apontando a pasta do `WardenFakeGame.class` (o jogo simulado
//! dos testes da `warden-launcher`) e `WARDEN_E2E_JAVA` apontando um `java`, o teste não baixa
//! Java nem Minecraft: o "jogo instalado" é o programa simulado, e todo o resto (um jogo por
//! vez, sincronização da instância, processo, console, parar, sessão gravada) é o de verdade.
//! O modo do jogo simulado vem do arquivo [`ARGS_FILE`] na pasta do pack (padrão: fica aberto
//! com um processo filho, como o jogo).

use std::path::{Path, PathBuf};

use warden_launcher::{GameSpec, InstalledGame, JavaRuntime, LoggingConfig};

/// Pasta com o `WardenFakeGame.class`.
pub(crate) const FAKE_GAME_ENV: &str = "WARDEN_E2E_FAKE_GAME";
/// O `java` que roda o jogo simulado.
pub(crate) const FAKE_JAVA_ENV: &str = "WARDEN_E2E_JAVA";
/// Major desse Java (padrão 17).
pub(crate) const FAKE_JAVA_MAJOR_ENV: &str = "WARDEN_E2E_JAVA_MAJOR";
/// Arquivo na pasta do pack com os argumentos do jogo simulado (`crash`, `acentos`…).
pub(crate) const ARGS_FILE: &str = ".warden-fake-game";
/// Argumentos padrão: fica aberto, com um processo filho, até "Parar jogo".
const DEFAULT_ARGS: [&str; 2] = ["filhos", "1"];

/// O jogo simulado.
#[derive(Debug, Clone)]
pub(crate) struct FakeGame {
    class_dir: PathBuf,
    java: PathBuf,
    major: u32,
}

impl FakeGame {
    /// O jogo simulado das variáveis de ambiente, só em build de debug.
    pub(crate) fn from_env() -> Option<Self> {
        if !cfg!(debug_assertions) {
            return None;
        }
        let class_dir = std::env::var_os(FAKE_GAME_ENV).filter(|value| !value.is_empty())?;
        let java = std::env::var_os(FAKE_JAVA_ENV).filter(|value| !value.is_empty())?;
        let major = std::env::var(FAKE_JAVA_MAJOR_ENV)
            .ok()
            .and_then(|text| text.trim().parse().ok())
            .unwrap_or(17);
        Some(Self {
            class_dir: class_dir.into(),
            java: java.into(),
            major,
        })
    }

    /// O jogo simulado com a classe e o Java dados (testes).
    #[cfg(test)]
    pub(crate) fn new(class_dir: PathBuf, java: PathBuf, major: u32) -> Self {
        Self {
            class_dir,
            java,
            major,
        }
    }

    /// O Java do jogo simulado (o jogo abre pelo `javaw` ao lado, quando existe, como o jogo
    /// de verdade no Windows).
    pub(crate) fn java(&self) -> JavaRuntime {
        let javaw = self.java.with_file_name("javaw.exe");
        let launcher = if cfg!(windows) && javaw.is_file() {
            javaw
        } else {
            self.java.clone()
        };
        JavaRuntime {
            java: self.java.clone(),
            launcher,
            major: self.major,
        }
    }

    /// O "jogo instalado": a classe do jogo simulado com os argumentos do pack.
    pub(crate) fn installed(&self, spec: &GameSpec, pack_root: &Path) -> InstalledGame {
        let args: Vec<String> = std::fs::read_to_string(pack_root.join(ARGS_FILE))
            .ok()
            .map(|text| text.split_whitespace().map(ToOwned::to_owned).collect())
            .filter(|args: &Vec<String>| !args.is_empty())
            .unwrap_or_else(|| DEFAULT_ARGS.iter().map(|&arg| arg.to_owned()).collect());
        InstalledGame {
            spec: spec.clone(),
            version_id: "warden-fake-game".into(),
            hierarchy: vec!["warden-fake-game".into()],
            // Um `releaseTime` da 1.20.1: sem argumentos de faixa antiga.
            release_time: "2023-06-12T13:25:51+00:00".into(),
            main_class: "WardenFakeGame".into(),
            jvm_args: vec!["-cp".into(), self.class_dir.display().to_string()],
            game_args: args,
            game_dir_placeholder: "<fake-game-dir>".into(),
            logging: LoggingConfig::None,
        }
    }
}

#[cfg(test)]
mod tests {
    use warden_launcher::LoaderSpec;

    use super::*;

    #[test]
    fn argumentos_vem_do_arquivo_do_pack() {
        let dir = tempfile::tempdir().unwrap();
        let fake = FakeGame {
            class_dir: PathBuf::from("classes"),
            java: PathBuf::from("java"),
            major: 21,
        };
        let spec = GameSpec::new("1.20.1", LoaderSpec::Vanilla).unwrap();
        assert_eq!(fake.installed(&spec, dir.path()).game_args, ["filhos", "1"]);
        std::fs::write(dir.path().join(ARGS_FILE), "crash\n").unwrap();
        let game = fake.installed(&spec, dir.path());
        assert_eq!(game.game_args, ["crash"]);
        assert_eq!(game.jvm_args, ["-cp", "classes"]);
        assert_eq!(fake.java().major, 21);
    }
}
