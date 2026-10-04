//! Leitura do `.env` de desenvolvimento do repositório principal (ARCHITECTURE §14).
//!
//! O arquivo fica na raiz do repositório principal (descoberta por
//! `git rev-parse --git-common-dir`, que funciona também nos worktrees). Nada é copiado e
//! nenhum valor é impresso: só os nomes das variáveis carregadas.
//!
//! Formato aceito, por linha: `NOME=valor`, com `export ` opcional e comentários com `#`.
//! Valores entre aspas simples são literais (a chave da CurseForge começa com `$2a$`); entre
//! aspas duplas aceitam `\n`, `\t`, `\"` e `\\`; sem aspas, vão até um ` #` de comentário.

use std::path::{Path, PathBuf};

use anyhow::{Context as _, Result, bail};

use crate::util::Cmd;

/// Variáveis lidas de um `.env`, na ordem do arquivo.
#[derive(Default)]
pub struct EnvVars(Vec<(String, String)>);

impl EnvVars {
    /// Nomes das variáveis (para mostrar no terminal).
    pub fn names(&self) -> Vec<&str> {
        self.0.iter().map(|(name, _)| name.as_str()).collect()
    }

    /// Pares nome e valor, para passar ao processo filho.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
        self.0
            .iter()
            .map(|(name, value)| (name.as_str(), value.as_str()))
    }

    /// Se não há nenhuma variável.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

// `Debug` nunca mostra valores.
impl std::fmt::Debug for EnvVars {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_list().entries(self.names()).finish()
    }
}

/// Lê o texto de um `.env`. Erros citam a linha, nunca o valor.
pub fn parse(text: &str) -> Result<EnvVars> {
    let mut vars = Vec::new();
    for (index, raw) in text.lines().enumerate() {
        let number = index + 1;
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let line = line.strip_prefix("export ").map_or(line, str::trim_start);
        let Some((name, rest)) = line.split_once('=') else {
            bail!("linha {number} do .env não tem `=`");
        };
        let name = name.trim();
        if name.is_empty()
            || name.starts_with(|c: char| c.is_ascii_digit())
            || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
        {
            bail!("linha {number} do .env tem um nome de variável inválido");
        }
        let value = parse_value(rest.trim_start())
            .with_context(|| format!("linha {number} do .env (variável {name})"))?;
        vars.push((name.to_owned(), value));
    }
    Ok(EnvVars(vars))
}

fn parse_value(rest: &str) -> Result<String> {
    if let Some(inner) = rest.strip_prefix('\'') {
        let Some(end) = inner.find('\'') else {
            bail!("aspas simples sem fechamento");
        };
        ensure_only_comment(&inner[end + 1..])?;
        return Ok(inner[..end].to_owned());
    }
    if let Some(inner) = rest.strip_prefix('"') {
        let mut value = String::new();
        let mut chars = inner.char_indices();
        while let Some((index, c)) = chars.next() {
            match c {
                '"' => {
                    ensure_only_comment(&inner[index + 1..])?;
                    return Ok(value);
                }
                '\\' => match chars.next() {
                    Some((_, 'n')) => value.push('\n'),
                    Some((_, 't')) => value.push('\t'),
                    Some((_, other)) => value.push(other),
                    None => bail!("barra invertida no fim do valor"),
                },
                other => value.push(other),
            }
        }
        bail!("aspas duplas sem fechamento");
    }
    let value = rest.find(" #").map_or(rest, |index| &rest[..index]);
    Ok(value.trim_end().to_owned())
}

fn ensure_only_comment(after: &str) -> Result<()> {
    let after = after.trim_start();
    if after.is_empty() || after.starts_with('#') {
        Ok(())
    } else {
        bail!("texto depois das aspas")
    }
}

/// Caminho do `.env` do repositório principal.
pub fn main_repo_env_path() -> Result<PathBuf> {
    let common = Cmd::new("git")
        .args(["rev-parse", "--path-format=absolute", "--git-common-dir"])
        .read()
        .context("não foi possível descobrir o repositório principal com o git")?;
    let common = PathBuf::from(common.trim());
    let repo = common
        .parent()
        .context("pasta comum do git sem pasta acima")?;
    Ok(repo.join(".env"))
}

/// Lê um `.env`; `Ok(None)` se o arquivo não existe.
pub fn load(path: &Path) -> Result<Option<EnvVars>> {
    match std::fs::read_to_string(path) {
        Ok(text) => parse(&text).map(Some),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error).with_context(|| format!("falha ao ler {}", path.display())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pairs(vars: &EnvVars) -> Vec<(&str, &str)> {
        vars.iter().collect()
    }

    #[test]
    fn aspas_simples_sao_literais() {
        let vars = parse("CURSEFORGE_API_KEY='$2a$10$abc\\n\"x\"'\n").unwrap();
        assert_eq!(pairs(&vars), [("CURSEFORGE_API_KEY", "$2a$10$abc\\n\"x\"")]);
    }

    #[test]
    fn aspas_duplas_com_escapes() {
        let vars = parse("A=\"um\\ndois \\\"tres\\\" \\\\\" # comentário\n").unwrap();
        assert_eq!(pairs(&vars), [("A", "um\ndois \"tres\" \\")]);
    }

    #[test]
    fn sem_aspas_comentarios_export_e_linhas_vazias() {
        let text = "# cabeçalho\n\nexport B = valor solto # comentário\r\nC=\nD=a#b\n";
        let vars = parse(text).unwrap();
        assert_eq!(
            pairs(&vars),
            [("B", "valor solto"), ("C", ""), ("D", "a#b")]
        );
    }

    #[test]
    fn erros_citam_a_linha_e_nunca_o_valor() {
        for text in [
            "X='segredo-123",
            "X=\"segredo-123",
            "X='segredo-123' sobra",
            "1X=a",
            "sem-igual",
        ] {
            let error = format!("{:#}", parse(text).err().unwrap());
            assert!(error.contains("linha 1"), "{error}");
            assert!(!error.contains("segredo-123"), "{error}");
        }
    }

    #[test]
    fn debug_nao_mostra_valores() {
        let vars = parse("GEMINI_API_KEY='AIza-segredo'").unwrap();
        let debug = format!("{vars:?}");
        assert!(debug.contains("GEMINI_API_KEY"));
        assert!(!debug.contains("segredo"));
    }

    #[test]
    fn arquivo_inexistente_e_none() {
        let dir = tempfile::tempdir().unwrap();
        assert!(load(&dir.path().join(".env")).unwrap().is_none());
        std::fs::write(dir.path().join(".env"), "A='1'\n").unwrap();
        let vars = load(&dir.path().join(".env")).unwrap().unwrap();
        assert_eq!(vars.names(), ["A"]);
        assert!(!vars.is_empty());
    }
}
