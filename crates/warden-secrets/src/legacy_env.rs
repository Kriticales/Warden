//! O arquivo `.env` do antigo modo "guardar num arquivo" (ADR-0025, substituída pela
//! ADR-0069): só leitura, para a migração das chaves para o cofre do sistema
//! ([`crate::Secrets::migrate_legacy_env`]).
//!
//! Formato que o Warden gravava: uma variável por linha, `NOME='valor'` (aspas simples, porque a
//! chave da CurseForge começa com `$2a$`). A leitura é tolerante, como era: aceita
//! `export NOME=...`, aspas duplas com `\"`, `\\` e `\n`, valor sem aspas (até um ` #` de
//! comentário) e ignora comentários e linhas desconhecidas.

use std::path::Path;

use secrecy::SecretString;
use warden_core::CoreError;

use crate::error::SecretsError;
use crate::kind::SecretKind;

/// Lê as chaves do `.env` antigo. `None` quando o arquivo não existe; valor vazio conta como
/// ausente.
pub(crate) fn read_legacy_env(
    path: &Path,
) -> Result<Option<Vec<(SecretKind, SecretString)>>, SecretsError> {
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(CoreError::io("ler", path, error).into()),
    };
    let text = String::from_utf8(bytes).map_err(|_| SecretsError::EnvFileUnreadable {
        path: path.to_path_buf(),
    })?;
    let document = EnvDocument::parse(&text);
    Ok(Some(
        SecretKind::ALL
            .iter()
            .filter_map(|&kind| {
                document
                    .get(kind.env_var())
                    .map(|value| value.trim().to_owned())
                    .filter(|value| !value.is_empty())
                    .map(|value| (kind, SecretString::from(value)))
            })
            .collect(),
    ))
}

/// Apaga o `.env` antigo (apagar o que não existe não é erro).
pub(crate) fn remove_legacy_env(path: &Path) -> Result<(), SecretsError> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(CoreError::io("apagar", path, error).into()),
    }
}

/// As variáveis do `.env`, na ordem do arquivo (o resto das linhas é ignorado).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct EnvDocument {
    entries: Vec<(String, String)>,
}

impl EnvDocument {
    pub(crate) fn parse(text: &str) -> Self {
        let text = text.strip_prefix('\u{feff}').unwrap_or(text);
        let entries = text
            .lines()
            .filter_map(parse_key)
            .map(|(key, rest)| (key.to_owned(), parse_value(rest)))
            .collect();
        Self { entries }
    }

    /// Valor da última definição de `key` (como o shell faria).
    pub(crate) fn get(&self, key: &str) -> Option<String> {
        self.entries
            .iter()
            .rev()
            .find(|(found, _)| found == key)
            .map(|(_, value)| value.clone())
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
    use secrecy::ExposeSecret as _;

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
    fn aspas_sem_fechamento_sao_toleradas() {
        let document = EnvDocument::parse("A='sem fim\nB=\"sem fim\\\n");
        assert_eq!(document.get("A").unwrap(), "sem fim");
        assert_eq!(document.get("B").unwrap(), "sem fim\\");
    }

    #[test]
    fn le_as_chaves_do_arquivo_e_ignora_vazias() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(".env");
        assert!(read_legacy_env(&path).unwrap().is_none());
        std::fs::write(
            &path,
            "CURSEFORGE_API_KEY='$2a$10$xyz'\nGEMINI_API_KEY=''\nOUTRA=x\nGITHUB_TOKEN='ghp_abc'\n",
        )
        .unwrap();
        let read: Vec<_> = read_legacy_env(&path)
            .unwrap()
            .unwrap()
            .into_iter()
            .map(|(kind, value)| (kind, value.expose_secret().to_owned()))
            .collect();
        assert_eq!(
            read,
            [
                (SecretKind::Curseforge, "$2a$10$xyz".to_owned()),
                (SecretKind::Github, "ghp_abc".to_owned()),
            ]
        );
        remove_legacy_env(&path).unwrap();
        assert!(!path.exists());
        remove_legacy_env(&path).unwrap();
    }

    #[test]
    fn arquivo_que_nao_e_texto_e_erro_proprio() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(".env");
        std::fs::write(&path, [0xff, 0xfe, 0x00, 0x41]).unwrap();
        assert!(matches!(
            read_legacy_env(&path),
            Err(SecretsError::EnvFileUnreadable { .. })
        ));
    }

    proptest! {
        /// Todo valor que o Warden gravava (`NOME='valor'`) volta igual na leitura.
        #[test]
        fn le_o_que_o_warden_gravava(value in "[^'\\p{Cc}]{1,80}", noise in "(# [a-z ]{0,10}\n|OUTRA=[a-z]{0,5}\n){0,3}") {
            let trimmed = value.trim();
            prop_assume!(!trimmed.is_empty());
            let text = format!("{noise}CURSEFORGE_API_KEY='{trimmed}'\n");
            prop_assert_eq!(EnvDocument::parse(&text).get("CURSEFORGE_API_KEY").unwrap(), trimmed);
        }

        /// Texto arbitrário nunca entra em pânico.
        #[test]
        fn texto_arbitrario(text in "\\PC{0,200}") {
            let _ = EnvDocument::parse(&text).get("A");
        }
    }
}
