//! Configs pessoais (ADR-0067; SPEC T12 e T15): preferências de quem testa o pack, que ficam só
//! neste computador.
//!
//! O `options.txt` (vídeo, som, teclas, idioma) e parentes são de quem joga, não do pack: cada
//! jogador tem o seu. Por isso o Warden os guarda nos próprios dados, por pack
//! (`<dados>/personal/<pack-id>/`), e:
//!
//! - [`PersonalConfigs::restore_into`] copia a cópia guardada para a instância de teste em todo
//!   teste (logo antes de abrir o jogo), mesmo depois de a instância ser apagada;
//! - [`PersonalConfigs::save_from`] guarda o que o jogo deixou na instância quando ele fecha;
//! - a materialização nunca escreve, apaga nem pede revisão desses caminhos
//!   ([`crate::MaterializeOptions::personal`]) e a captura nunca os mostra como candidatos ao
//!   pack.
//!
//! A lista começa com [`DEFAULT_PERSONAL`]; "Marcar como pessoal" acrescenta um caminho e
//! "Deixar de ser pessoal" tira (inclusive um dos padrões). As escolhas ficam em
//! `personal.json`; as cópias, em `files/<caminho>`.

use std::collections::BTreeSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use warden_core::{atomic_write, resolve_inside};

use crate::error::Error;

/// As escolhas (`personal.json`).
pub const PERSONAL_FILE: &str = "personal.json";

/// As cópias guardadas, pelo caminho na instância.
pub const PERSONAL_FILES_DIR: &str = "files";

/// Versão do formato de `personal.json`.
const PERSONAL_SCHEMA: u32 = 1;

/// Preferências pessoais conhecidas: as opções do jogo (vídeo, som, teclas), as do `OptiFine`
/// e dos shaders, e o minimapa e o mapa do Xaero (zoom, tamanho, pontos marcados).
pub const DEFAULT_PERSONAL: [&str; 5] = [
    "options.txt",
    "optionsof.txt",
    "optionsshaders.txt",
    "config/xaerominimap.txt",
    "config/xaeroworldmap.txt",
];

/// O que a pessoa mudou na lista padrão.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Choices {
    schema_version: u32,
    /// Marcados como pessoais por quem testa.
    #[serde(default)]
    added: BTreeSet<String>,
    /// Padrões que deixaram de ser pessoais.
    #[serde(default)]
    removed: BTreeSet<String>,
}

/// As configs pessoais de um pack.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PersonalConfigs {
    dir: PathBuf,
    choices: Choices,
}

/// Caminho para mensagens: relativo à pasta das configs pessoais.
fn shown(relative: &str) -> String {
    format!("configs pessoais/{relative}")
}

/// Caminho relativo com `/`, sem `./` no começo. Vazio, absoluto ou com `..`: erro.
fn normalized(path: &str) -> Result<String, Error> {
    let clean = path.replace('\\', "/");
    let clean = clean.trim_start_matches("./");
    // Confere o caminho (absoluto, `..`, vazio) do mesmo jeito que o resto do app.
    resolve_inside(Path::new("."), clean)?;
    Ok(clean.to_owned())
}

/// Grava criando a pasta antes.
fn write_creating(path: &Path, bytes: &[u8], relative: &str) -> Result<(), Error> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| Error::io("criar a pasta", shown(relative), e))?;
    }
    atomic_write(path, bytes)?;
    Ok(())
}

/// Lê um arquivo; `None` se não existe.
fn read_optional(path: &Path, relative: &str) -> Result<Option<Vec<u8>>, Error> {
    match fs::read(path) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(Error::io("ler", relative.to_owned(), error)),
    }
}

impl PersonalConfigs {
    /// Lê as escolhas de `dir` (`<dados>/personal/<pack-id>`). Sem arquivo, ou ilegível: só
    /// os padrões (com aviso no registro, no segundo caso).
    pub fn load(dir: &Path) -> Result<Self, Error> {
        let file = dir.join(PERSONAL_FILE);
        let choices = match read_optional(&file, &shown(PERSONAL_FILE))? {
            None => Choices::default(),
            Some(bytes) => match serde_json::from_slice::<Choices>(&bytes) {
                Ok(choices) if choices.schema_version <= PERSONAL_SCHEMA => choices,
                Ok(_) => Choices::default(),
                Err(error) => {
                    tracing::warn!(%error, "configs pessoais ilegíveis; valem os padrões");
                    Choices::default()
                }
            },
        };
        Ok(Self {
            dir: dir.to_path_buf(),
            choices,
        })
    }

    /// Os caminhos pessoais (relativos à pasta do jogo, com `/`).
    #[must_use]
    pub fn paths(&self) -> BTreeSet<String> {
        DEFAULT_PERSONAL
            .iter()
            .map(|path| (*path).to_owned())
            .filter(|path| !self.choices.removed.contains(path))
            .chain(self.choices.added.iter().cloned())
            .collect()
    }

    /// Se o caminho é pessoal.
    #[must_use]
    pub fn contains(&self, path: &str) -> bool {
        self.paths().contains(path)
    }

    /// Se o caminho é um dos padrões ([`DEFAULT_PERSONAL`]).
    #[must_use]
    pub fn is_default(path: &str) -> bool {
        DEFAULT_PERSONAL.contains(&path)
    }

    fn save_choices(&self) -> Result<(), Error> {
        let mut choices = self.choices.clone();
        choices.schema_version = PERSONAL_SCHEMA;
        let mut bytes = serde_json::to_vec_pretty(&choices)
            .map_err(|error| Error::Internal(format!("configs pessoais em JSON: {error}")))?;
        bytes.push(b'\n');
        write_creating(&self.dir.join(PERSONAL_FILE), &bytes, PERSONAL_FILE)
    }

    fn copy_path(&self, relative: &str) -> Result<PathBuf, Error> {
        Ok(resolve_inside(
            &self.dir.join(PERSONAL_FILES_DIR),
            relative,
        )?)
    }

    /// Marca `path` como pessoal ("Marcar como pessoal"). Devolve `false` se já era.
    pub fn mark(&mut self, path: &str) -> Result<bool, Error> {
        let path = normalized(path)?;
        if self.contains(&path) {
            return Ok(false);
        }
        if !self.choices.removed.remove(&path) {
            self.choices.added.insert(path);
        }
        self.save_choices()?;
        Ok(true)
    }

    /// Tira `path` das pessoais ("Deixar de ser pessoal") e apaga a cópia guardada. O arquivo
    /// da instância fica onde está. Devolve `false` se não era pessoal.
    pub fn unmark(&mut self, path: &str) -> Result<bool, Error> {
        let path = normalized(path)?;
        if !self.contains(&path) {
            return Ok(false);
        }
        if !self.choices.added.remove(&path) {
            self.choices.removed.insert(path.clone());
        }
        self.save_choices()?;
        let copy = self.copy_path(&path)?;
        match fs::remove_file(&copy) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(Error::io("apagar", shown(&path), error)),
        }
        Ok(true)
    }

    /// A cópia guardada de `path`, se houver.
    pub fn stored(&self, path: &str) -> Result<Option<Vec<u8>>, Error> {
        let path = normalized(path)?;
        read_optional(&self.copy_path(&path)?, &shown(&path))
    }

    /// Guarda `bytes` como a cópia de `path` (que precisa ser pessoal). Igual ao que já estava
    /// guardado: não grava e devolve `false`.
    pub fn store(&self, path: &str, bytes: &[u8]) -> Result<bool, Error> {
        let path = normalized(path)?;
        if !self.contains(&path) {
            return Err(Error::Internal(format!("{path} não é uma config pessoal")));
        }
        let copy = self.copy_path(&path)?;
        if read_optional(&copy, &shown(&path))?.as_deref() == Some(bytes) {
            return Ok(false);
        }
        write_creating(&copy, bytes, &path)?;
        Ok(true)
    }

    /// Guarda as configs pessoais que estão na instância (`game_dir`), quando o jogo fecha ou
    /// quando uma é editada no Warden. Devolve os caminhos que mudaram na cópia guardada.
    pub fn save_from(&self, game_dir: &Path) -> Result<Vec<String>, Error> {
        let mut saved = Vec::new();
        for path in self.paths() {
            let full = resolve_inside(game_dir, &path)?;
            let Some(bytes) = read_optional(&full, &format!("instância/{path}"))? else {
                continue;
            };
            if self.store(&path, &bytes)? {
                saved.push(path);
            }
        }
        Ok(saved)
    }

    /// Copia as cópias guardadas para a instância (`game_dir`), em todo teste. Só grava o que
    /// está diferente; o que não tem cópia guardada fica como está. Devolve os caminhos
    /// gravados.
    pub fn restore_into(&self, game_dir: &Path) -> Result<Vec<String>, Error> {
        let mut restored = Vec::new();
        for path in self.paths() {
            let Some(bytes) = self.stored(&path)? else {
                continue;
            };
            let full = resolve_inside(game_dir, &path)?;
            let current = read_optional(&full, &format!("instância/{path}"))?;
            if current.as_deref() == Some(bytes.as_slice()) {
                continue;
            }
            write_creating(&full, &bytes, &path)?;
            restored.push(path);
        }
        Ok(restored)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::temp_dir;

    #[test]
    fn padroes_e_escolhas() {
        let dir = temp_dir();
        let mut personal = PersonalConfigs::load(dir.path()).unwrap();
        assert!(personal.contains("options.txt"));
        assert!(personal.contains("config/xaerominimap.txt"));
        assert!(!personal.contains("config/jei/jei-client.ini"));

        assert!(personal.mark("config\\jei/jei-client.ini").unwrap());
        assert!(!personal.mark("config/jei/jei-client.ini").unwrap());
        assert!(personal.unmark("options.txt").unwrap());
        assert!(!personal.unmark("options.txt").unwrap());

        // As escolhas ficam gravadas.
        let again = PersonalConfigs::load(dir.path()).unwrap();
        assert!(again.contains("config/jei/jei-client.ini"));
        assert!(!again.contains("options.txt"));
        assert!(again.contains("optionsof.txt"));

        // Voltar a marcar um padrão tira a escolha, sem acrescentar.
        let mut again = again;
        assert!(again.mark("options.txt").unwrap());
        assert_eq!(again.choices.added.len(), 1);
        assert!(again.choices.removed.is_empty());
    }

    #[test]
    fn caminho_fora_da_instancia_e_recusado() {
        let dir = temp_dir();
        let mut personal = PersonalConfigs::load(dir.path()).unwrap();
        assert!(personal.mark("../fora.txt").is_err());
        assert!(personal.mark("").is_err());
    }

    #[test]
    fn guarda_ao_fechar_e_devolve_em_todo_teste() {
        let dir = temp_dir();
        let game = dir.path().join("minecraft");
        let store = dir.path().join("personal");
        fs::create_dir_all(game.join("config")).unwrap();
        fs::write(game.join("options.txt"), "renderDistance:12\n").unwrap();
        fs::write(game.join("config/xaerominimap.txt"), "zoom:2\n").unwrap();
        fs::write(game.join("config/outra.toml"), "a = 1\n").unwrap();
        let personal = PersonalConfigs::load(&store).unwrap();

        let mut saved = personal.save_from(&game).unwrap();
        saved.sort();
        assert_eq!(saved, ["config/xaerominimap.txt", "options.txt"]);
        // Sem mudança, nada é regravado.
        assert!(personal.save_from(&game).unwrap().is_empty());
        assert_eq!(
            personal.stored("options.txt").unwrap().as_deref(),
            Some(&b"renderDistance:12\n"[..])
        );
        assert_eq!(personal.stored("config/outra.toml").unwrap(), None);

        // A instância foi apagada (ou o jogo mudou o arquivo): o próximo teste devolve a cópia.
        fs::remove_dir_all(&game).unwrap();
        let restored = personal.restore_into(&game).unwrap();
        assert_eq!(restored.len(), 2);
        assert_eq!(
            fs::read_to_string(game.join("options.txt")).unwrap(),
            "renderDistance:12\n"
        );
        assert!(personal.restore_into(&game).unwrap().is_empty());
    }

    #[test]
    fn deixar_de_ser_pessoal_apaga_a_copia_e_mantem_a_instancia() {
        let dir = temp_dir();
        let game = dir.path().join("minecraft");
        fs::create_dir_all(&game).unwrap();
        fs::write(game.join("options.txt"), "lang:pt_br\n").unwrap();
        let mut personal = PersonalConfigs::load(&dir.path().join("personal")).unwrap();
        personal.save_from(&game).unwrap();
        personal.unmark("options.txt").unwrap();
        assert_eq!(personal.stored("options.txt").unwrap(), None);
        assert!(game.join("options.txt").exists());
        assert!(personal.store("options.txt", b"x").is_err());
    }
}
