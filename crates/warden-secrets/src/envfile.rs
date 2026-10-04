//! Arquivo `.env` na pasta de configuração do Warden (ADR-0025; ARCHITECTURE §14).
//!
//! Formato: uma variável por linha, `NOME='valor'` (aspas simples, porque a chave da
//! CurseForge começa com `$2a$` e muitos leitores de `.env` expandem `$` dentro de aspas
//! duplas). A leitura é tolerante: aceita `export NOME=...`, aspas duplas com `\"`, `\\` e
//! `\n`, valor sem aspas (até um ` #` de comentário) e preserva, sem interpretar, comentários
//! e linhas desconhecidas. A escrita é atômica e o arquivo nasce `0600` no Linux.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use secrecy::{ExposeSecret as _, SecretString};
use warden_core::{CoreError, atomic_write_private};

use crate::error::SecretsError;
use crate::kind::{BackendKind, SecretKind};
use crate::store::SecretStore;

/// O `.env` das chaves.
#[derive(Debug)]
pub struct EnvFileStore {
    path: PathBuf,
    /// Serializa ler-alterar-gravar dentro do processo.
    lock: Mutex<()>,
}

impl EnvFileStore {
    /// Armazenamento no arquivo `path` (que pode ainda não existir).
    #[must_use]
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            lock: Mutex::new(()),
        }
    }

    /// Caminho do arquivo.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    fn read_document(&self) -> Result<EnvDocument, SecretsError> {
        match std::fs::read(&self.path) {
            Ok(bytes) => {
                let text =
                    String::from_utf8(bytes).map_err(|_| SecretsError::EnvFileUnreadable {
                        path: self.path.clone(),
                    })?;
                Ok(EnvDocument::parse(&text))
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                Ok(EnvDocument::default())
            }
            Err(error) => Err(CoreError::io("ler", &self.path, error).into()),
        }
    }

    fn write_document(&self, document: &EnvDocument) -> Result<(), SecretsError> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|error| CoreError::io("criar a pasta", parent, error))?;
        }
        atomic_write_private(&self.path, document.render().as_bytes())?;
        Ok(())
    }

    fn locked(&self) -> std::sync::MutexGuard<'_, ()> {
        // Uma thread que entrou em pânico com a trava não deixa o arquivo pela metade
        // (a escrita é atômica): seguir com a trava envenenada é seguro.
        self.lock
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

impl SecretStore for EnvFileStore {
    fn backend(&self) -> BackendKind {
        BackendKind::Envfile
    }

    fn get(&self, kind: SecretKind) -> Result<Option<SecretString>, SecretsError> {
        let _guard = self.locked();
        Ok(self
            .read_document()?
            .get(kind.env_var())
            .filter(|value| !value.is_empty())
            .map(SecretString::from))
    }

    fn set(&self, kind: SecretKind, value: &SecretString) -> Result<(), SecretsError> {
        let _guard = self.locked();
        let mut document = self.read_document()?;
        document.set(kind.env_var(), value.expose_secret());
        self.write_document(&document)
    }

    fn remove(&self, kind: SecretKind) -> Result<(), SecretsError> {
        let _guard = self.locked();
        let mut document = self.read_document()?;
        if document.remove(kind.env_var()) {
            self.write_document(&document)?;
        }
        Ok(())
    }

    /// Voltar para o cofre apaga o `.env` (ARCHITECTURE §14).
    fn purge(&self) -> Result<(), SecretsError> {
        let _guard = self.locked();
        match std::fs::remove_file(&self.path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(CoreError::io("apagar", &self.path, error).into()),
        }
    }
}

/// Uma linha do `.env`: variável reconhecida ou texto preservado como está.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Line {
    Entry { key: String, raw: String },
    Other(String),
}

/// Conteúdo do `.env`, linha a linha.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct EnvDocument {
    lines: Vec<Line>,
}

impl EnvDocument {
    pub(crate) fn parse(text: &str) -> Self {
        let text = text.strip_prefix('\u{feff}').unwrap_or(text);
        let lines = text
            .lines()
            .map(|raw| match parse_key(raw) {
                Some((key, _)) => Line::Entry {
                    key: key.to_owned(),
                    raw: raw.to_owned(),
                },
                None => Line::Other(raw.to_owned()),
            })
            .collect();
        Self { lines }
    }

    /// Valor da última definição de `key` (como o shell faria).
    pub(crate) fn get(&self, key: &str) -> Option<String> {
        self.lines.iter().rev().find_map(|line| match line {
            Line::Entry { key: found, raw } if found == key => {
                parse_key(raw).map(|(_, rest)| parse_value(rest))
            }
            _ => None,
        })
    }

    /// Define `key`: substitui a primeira definição e apaga as repetidas, ou acrescenta.
    pub(crate) fn set(&mut self, key: &str, value: &str) {
        let new_line = format!("{key}='{value}'");
        let mut replaced = false;
        self.lines.retain_mut(|line| match line {
            Line::Entry { key: found, raw } if found == key => {
                if replaced {
                    false
                } else {
                    raw.clone_from(&new_line);
                    replaced = true;
                    true
                }
            }
            _ => true,
        });
        if !replaced {
            self.lines.push(Line::Entry {
                key: key.to_owned(),
                raw: new_line,
            });
        }
    }

    /// Apaga todas as definições de `key`. Devolve se havia alguma.
    pub(crate) fn remove(&mut self, key: &str) -> bool {
        let before = self.lines.len();
        self.lines
            .retain(|line| !matches!(line, Line::Entry { key: found, .. } if found == key));
        self.lines.len() != before
    }

    /// Texto do arquivo, com `\n` e linha final.
    pub(crate) fn render(&self) -> String {
        let mut text = String::new();
        for line in &self.lines {
            let raw = match line {
                Line::Entry { raw, .. } | Line::Other(raw) => raw,
            };
            text.push_str(raw);
            text.push('\n');
        }
        text
    }
}

/// `NOME=resto` (com `export ` opcional). Devolve o nome e o que vem depois do `=`.
fn parse_key(raw: &str) -> Option<(&str, &str)> {
    let line = raw.trim_start();
    let line = line.strip_prefix("export ").map_or(line, str::trim_start);
    let (key, rest) = line.split_once('=')?;
    let key = key.trim_end();
    let valid = !key.is_empty()
        && !key.starts_with(|c: char| c.is_ascii_digit())
        && key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
    valid.then_some((key, rest))
}

fn parse_value(rest: &str) -> String {
    let rest = rest.trim_start();
    if let Some(inner) = rest.strip_prefix('\'') {
        return inner
            .split_once('\'')
            .map_or(inner, |(value, _)| value)
            .to_owned();
    }
    if let Some(inner) = rest.strip_prefix('"') {
        let mut value = String::new();
        let mut chars = inner.chars();
        while let Some(c) = chars.next() {
            match c {
                '"' => return value,
                '\\' => match chars.next() {
                    Some('n') => value.push('\n'),
                    Some('t') => value.push('\t'),
                    Some(other) => value.push(other),
                    None => value.push('\\'),
                },
                other => value.push(other),
            }
        }
        return value;
    }
    let value = rest.find(" #").map_or(rest, |index| &rest[..index]);
    value.trim_end().to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn secret(text: &str) -> SecretString {
        SecretString::from(text.to_owned())
    }

    #[test]
    fn le_os_formatos_comuns() {
        let document = EnvDocument::parse(
            "\u{feff}# chaves\n\
             CURSEFORGE_API_KEY='$2a$10$abc/def'\n\
             export GEMINI_API_KEY = \"AIza\\\"x\\\\y\"\n\
             GITHUB_TOKEN=ghp_123 # comentário\n\
             VAZIA=\n\
             linha qualquer\n\
             1NOME=x\n",
        );
        assert_eq!(
            document.get("CURSEFORGE_API_KEY").unwrap(),
            "$2a$10$abc/def"
        );
        assert_eq!(document.get("GEMINI_API_KEY").unwrap(), "AIza\"x\\y");
        assert_eq!(document.get("GITHUB_TOKEN").unwrap(), "ghp_123");
        assert_eq!(document.get("VAZIA").unwrap(), "");
        assert_eq!(document.get("1NOME"), None);
        assert_eq!(document.get("NAO_EXISTE"), None);
    }

    #[test]
    fn aceita_crlf_e_ultima_definicao_vale() {
        let document = EnvDocument::parse("A='1'\r\nA='2'\r\n");
        assert_eq!(document.get("A").unwrap(), "2");
    }

    #[test]
    fn grava_com_aspas_simples_e_preserva_o_resto() {
        let mut document = EnvDocument::parse("# topo\nOUTRA=x\nA='velho'\nA='repetida'\n");
        document.set("A", "$2a$novo");
        document.set("B", "b");
        assert_eq!(document.render(), "# topo\nOUTRA=x\nA='$2a$novo'\nB='b'\n");
        assert!(document.remove("A"));
        assert!(!document.remove("A"));
        assert_eq!(document.render(), "# topo\nOUTRA=x\nB='b'\n");
    }

    #[test]
    fn aspas_sem_fechamento_sao_toleradas() {
        let document = EnvDocument::parse("A='sem fim\nB=\"sem fim\\\n");
        assert_eq!(document.get("A").unwrap(), "sem fim");
        assert_eq!(document.get("B").unwrap(), "sem fim\\");
    }

    #[test]
    fn armazenamento_grava_le_e_apaga() {
        let dir = tempfile::tempdir().unwrap();
        let store = EnvFileStore::new(dir.path().join("config").join(".env"));
        assert_eq!(store.backend(), BackendKind::Envfile);
        assert!(store.get(SecretKind::Curseforge).unwrap().is_none());

        store
            .set(SecretKind::Curseforge, &secret("$2a$10$xyz"))
            .unwrap();
        store.set(SecretKind::Github, &secret("ghp_abc")).unwrap();
        let text = std::fs::read_to_string(store.path()).unwrap();
        assert_eq!(
            text,
            "CURSEFORGE_API_KEY='$2a$10$xyz'\nGITHUB_TOKEN='ghp_abc'\n"
        );
        assert_eq!(
            store
                .get(SecretKind::Curseforge)
                .unwrap()
                .unwrap()
                .expose_secret(),
            "$2a$10$xyz"
        );

        store.remove(SecretKind::Curseforge).unwrap();
        store.remove(SecretKind::Gemini).unwrap();
        assert!(store.get(SecretKind::Curseforge).unwrap().is_none());
        assert!(store.get(SecretKind::Github).unwrap().is_some());

        store.purge().unwrap();
        assert!(!store.path().exists());
        store.purge().unwrap();
    }

    #[test]
    fn arquivo_que_nao_e_texto_e_erro_proprio() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(".env");
        std::fs::write(&path, [0xff, 0xfe, 0x00, 0x41]).unwrap();
        let store = EnvFileStore::new(&path);
        assert!(matches!(
            store.get(SecretKind::Gemini),
            Err(SecretsError::EnvFileUnreadable { .. })
        ));
    }

    #[test]
    fn valor_vazio_conta_como_ausente() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(".env");
        std::fs::write(&path, "GEMINI_API_KEY=''\n").unwrap();
        assert!(
            EnvFileStore::new(&path)
                .get(SecretKind::Gemini)
                .unwrap()
                .is_none()
        );
    }

    proptest! {
        /// Todo valor aceito por `normalize_secret` volta igual depois de gravar e ler.
        #[test]
        fn ida_e_volta(value in "[^'\\p{Cc}]{1,80}", noise in "(# [a-z ]{0,10}\n|OUTRA=[a-z]{0,5}\n){0,3}") {
            let trimmed = value.trim();
            prop_assume!(!trimmed.is_empty());
            let mut document = EnvDocument::parse(&noise);
            document.set("CURSEFORGE_API_KEY", trimmed);
            let reparsed = EnvDocument::parse(&document.render());
            prop_assert_eq!(reparsed.get("CURSEFORGE_API_KEY").unwrap(), trimmed);
            prop_assert!(reparsed.render().starts_with(&noise));
        }

        /// Texto arbitrário nunca entra em pânico e a releitura é estável.
        #[test]
        fn texto_arbitrario(text in "\\PC{0,200}") {
            let document = EnvDocument::parse(&text);
            let again = EnvDocument::parse(&document.render());
            prop_assert_eq!(document.render(), again.render());
            let _ = document.get("A");
        }
    }
}
