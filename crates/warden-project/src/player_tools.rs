//! Ferramentas do jogador: spark e Crash Assistant (ADR-0033; decisão D16).
//!
//! O pack marca quais itens dele são ferramentas do jogador na tabela `[player-tools]` de
//! `.warden/project.toml`: a chave é o papel (`spark`, `crash-assistant`) e o valor é o caminho
//! do `.pw.toml` no pack.
//!
//! ```toml
//! [player-tools]
//! spark = "mods/spark.pw.toml"
//! crash-assistant = "mods/crash-assistant.pw.toml"
//! ```
//!
//! A `warden-instance` lê a mesma tabela para decidir o que entra na instância de teste
//! (`warden-instance/src/player_tools.rs`). O leitor preserva tudo o que não conhece: as outras
//! tabelas, os comentários e a ordem (Gancho 1.1).
//!
//! **Crash Assistant (licença própria, ADR-0033).** O Warden só inclui o mod no pack por
//! referência e escreve uma config inicial com as chaves que o autor documenta (o histórico de
//! versões do mod no Modrinth e a própria config que o autor distribui em modpacks). O Warden não
//! lê o código do mod nem os arquivos que ele gera.

use std::fs;
use std::path::Path;

use toml_edit::{DocumentMut, Item, Table, value};

use crate::{Error, ProjectErrorCode as Code, Result};

/// Arquivo do Warden dentro do pack.
pub const PROJECT_FILE: &str = ".warden/project.toml";

/// Tabela das ferramentas do jogador.
pub const TABLE: &str = "player-tools";

/// Papel do spark.
pub const ROLE_SPARK: &str = "spark";

/// Papel do Crash Assistant.
pub const ROLE_CRASH_ASSISTANT: &str = "crash-assistant";

/// Caminho da config inicial do Crash Assistant, relativo ao pack.
pub const CRASH_ASSISTANT_CONFIG: &str = "config/crash_assistant/config.toml";

/// Uma ferramenta do jogador marcada no pack.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayerTool {
    /// Papel (`spark`, `crash-assistant`).
    pub role: String,
    /// Caminho do `.pw.toml` no pack, com `/`.
    pub path: String,
}

/// Lê as ferramentas do jogador de um texto de `.warden/project.toml`. Entradas que não são
/// texto são ignoradas; um arquivo sem a tabela devolve a lista vazia.
pub fn parse(text: &str) -> Result<Vec<PlayerTool>> {
    let doc: DocumentMut = text
        .parse()
        .map_err(|e| Error::new(Code::InvalidPack, format!("{PROJECT_FILE}: {e}")))?;
    let Some(table) = doc.get(TABLE).and_then(Item::as_table_like) else {
        return Ok(Vec::new());
    };
    Ok(table
        .iter()
        .filter_map(|(role, item)| {
            item.as_str().map(|path| PlayerTool {
                role: role.to_owned(),
                path: path.replace('\\', "/"),
            })
        })
        .collect())
}

/// Lê as ferramentas do jogador do pack. Pack sem `.warden/project.toml`: lista vazia.
pub fn read(root: &Path) -> Result<Vec<PlayerTool>> {
    match fs::read_to_string(root.join(PROJECT_FILE)) {
        Ok(text) => parse(&text),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(e) => Err(Error::new(
            Code::InvalidPack,
            format!("{PROJECT_FILE}: {e}"),
        )),
    }
}

/// Devolve o texto de `.warden/project.toml` com as ferramentas dadas na tabela
/// `[player-tools]` (as de mesmo papel são trocadas; as outras ficam). O resto do arquivo não
/// muda.
pub fn with_tools(text: &str, tools: &[PlayerTool]) -> Result<String> {
    let mut doc: DocumentMut = text
        .parse()
        .map_err(|e| Error::new(Code::InvalidPack, format!("{PROJECT_FILE}: {e}")))?;
    if !doc.contains_key(TABLE) {
        doc.insert(TABLE, Item::Table(Table::new()));
    }
    let table = doc
        .get_mut(TABLE)
        .and_then(Item::as_table_like_mut)
        .ok_or_else(|| {
            Error::new(
                Code::InvalidPack,
                format!("{PROJECT_FILE}: [{TABLE}] inválida"),
            )
        })?;
    for tool in tools {
        table.insert(&tool.role, value(tool.path.as_str()));
    }
    Ok(doc.to_string())
}

/// A config inicial do Crash Assistant (`config/crash_assistant/config.toml`).
///
/// Só chaves que o autor documenta:
/// - `general.send_uploaded_logs_data_to_kostromdan_dev = false`: não envia ao autor os metadados
///   dos logs enviados (opção criada na 1.11.12, ligada por padrão; changelog do autor);
/// - `general.wrap_link = false` (1.11.12 em diante) e `general.upload_to = "mclo.gs"` (versões
///   anteriores): o link do log fica o do `mclo.gs`, sem passar por um encurtador/visualizador
///   de terceiros;
/// - `piracy.enabled = false`: sem a verificação de pirataria (o launcher do Warden é offline e
///   o aviso confundiria o jogador; a 1.10.29 documenta a opção).
///
/// Chaves que a versão instalada não conhece são ignoradas pelo mod.
#[must_use]
pub fn crash_assistant_config() -> String {
    "\
# Config inicial escrita pelo Warden (só chaves documentadas pelo autor do Crash Assistant).
# Pode editar à vontade: o Warden não reescreve este arquivo.
[general]
\t# Não enviar ao autor do mod os metadados dos logs enviados.
\tsend_uploaded_logs_data_to_kostromdan_dev = false
\t# Não passar o link do log por um visualizador de terceiros (1.11.12 em diante).
\twrap_link = false
\t# Mesma ideia nas versões anteriores à 1.11.12.
\tupload_to = \"mclo.gs\"

[piracy]
\t# O Warden usa conta offline; sem a verificação de pirataria.
\tenabled = false
"
    .to_owned()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    #[test]
    fn grava_a_tabela_sem_perder_o_resto() {
        let original = "# do Warden\nschemaVersion = 1\nid = \"abc\"\n\n[futuro]\nx = 1\n";
        let tools = [PlayerTool {
            role: ROLE_CRASH_ASSISTANT.into(),
            path: "mods/crash-assistant.pw.toml".into(),
        }];
        let text = with_tools(original, &tools).unwrap();
        assert!(text.starts_with("# do Warden\nschemaVersion = 1\nid = \"abc\"\n"));
        assert!(text.contains("[futuro]\nx = 1\n"));
        assert_eq!(parse(&text).unwrap(), tools);
        // Gravar de novo não duplica nem muda.
        assert_eq!(with_tools(&text, &tools).unwrap(), text);
    }

    #[test]
    fn mantem_ferramentas_de_outros_papeis() {
        let text = with_tools(
            "id = \"a\"\n",
            &[PlayerTool {
                role: ROLE_SPARK.into(),
                path: "mods/spark.pw.toml".into(),
            }],
        )
        .unwrap();
        let text = with_tools(
            &text,
            &[PlayerTool {
                role: ROLE_CRASH_ASSISTANT.into(),
                path: "mods\\crash.pw.toml".into(),
            }],
        )
        .unwrap();
        let tools = parse(&text).unwrap();
        assert_eq!(tools.len(), 2);
        assert!(tools.iter().any(|t| t.role == ROLE_SPARK));
        assert!(tools.iter().any(|t| t.path == "mods/crash.pw.toml"));
    }

    #[test]
    fn sem_tabela_a_lista_e_vazia() {
        assert!(parse("id = \"a\"\n").unwrap().is_empty());
        assert!(
            parse("id = \"a\"\n[player-tools]\nspark = 3\n")
                .unwrap()
                .is_empty()
        );
        let error = parse("id = [").unwrap_err();
        assert_eq!(error.code, Code::InvalidPack);
    }

    #[test]
    fn config_inicial_desliga_o_envio_e_e_toml_valido() {
        let text = crash_assistant_config();
        let doc: DocumentMut = text.parse().unwrap();
        assert_eq!(
            doc["general"]["send_uploaded_logs_data_to_kostromdan_dev"].as_bool(),
            Some(false)
        );
        assert_eq!(doc["general"]["wrap_link"].as_bool(), Some(false));
        assert_eq!(doc["piracy"]["enabled"].as_bool(), Some(false));
    }
}
