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
//! referência e escreve uma config inicial ([`crash_assistant_config`]) com chaves que o autor
//! documenta. O app do Warden não usa, não lê nem porta o código do mod, e não lê os arquivos que
//! ele gera; o significado de cada chave, em cada faixa de versão, foi conferido uma vez na
//! correção da P1-18 (changelog no Modrinth e código público do mod, só para entender as chaves):
//!
//! - O mod lê `config/crash_assistant/config.toml` ao abrir. Chave com tipo diferente do padrão
//!   volta ao padrão (com um aviso no log); chave que a versão não conhece é apagada do arquivo
//!   da instância. Por isso cada chave vai com o tipo exato da versão que a usa, e as chaves de
//!   uma faixa não atrapalham a outra.
//! - **1.11.12 em diante** (todas as faixas de `data/initial-mods.toml` hoje, menos o Forge
//!   1.15 e 1.15.1, que param na 1.11.0): o log vai sempre para o `mclo.gs`, e
//!   `general.wrap_link` (texto: `"mclo.gs"`, `"gnomebot.dev"` ou `"kostromdan.dev"`, padrão
//!   `"kostromdan.dev"`) só escolhe o link copiado. Gravamos `"mclo.gs"`: o link do próprio
//!   serviço, sem visualizador no meio. `general.send_uploaded_logs_data_to_kostromdan_dev`
//!   (booleano, padrão `true`) manda os metadados dos logs enviados para `api.kostromdan.dev`;
//!   gravamos `false`.
//! - **Antes da 1.11.12:** não há envio ao autor. `general.upload_to` (texto: `"mclo.gs"` ou
//!   `"gnomebot.dev"`, padrão `"gnomebot.dev"`) escolhe o link copiado, e gravamos `"mclo.gs"`.
//!   Limite conhecido: enquanto `general.help_link` estiver no padrão (`"CHANGE_ME"`), essas
//!   versões ignoram `upload_to` e copiam o link do `gnomebot.dev` (que só mostra o log
//!   guardado no `mclo.gs`; o mod não manda nada para lá). O Warden não mexe em `help_link`:
//!   trocá-lo põe o mod em "modo modpack", com o botão de ajuda apontando para uma comunidade
//!   que o Warden não conhece. As chaves da 1.11.12 vão junto, para valerem se o mod do pack for
//!   atualizado depois (o Warden não reescreve a config).
//! - `piracy.enabled = false` (booleano, igual ao padrão): o aviso de pirataria fica desligado
//!   quando o autor do pack configura `help_link`; com `help_link` no padrão, o mod mostra o
//!   aviso de conta offline de qualquer jeito (vale para todas as versões).
//!
//! Versão que não dá para comparar (fora do formato numérico): vão as chaves das duas faixas.

use std::cmp::Ordering;
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

/// Primeira versão do Crash Assistant com `general.wrap_link` e
/// `general.send_uploaded_logs_data_to_kostromdan_dev` (e sem `general.upload_to`).
pub const CRASH_ASSISTANT_WRAP_LINK_SINCE: &str = "1.11.12";

/// A config inicial do Crash Assistant (`config/crash_assistant/config.toml`) para a versão do
/// mod que entrou no pack (`version_number` do Modrinth). As chaves e as faixas estão no início
/// do módulo.
#[must_use]
pub fn crash_assistant_config(version: &str) -> String {
    let wrap_link_era =
        crate::initial_mods::compare_versions(version, CRASH_ASSISTANT_WRAP_LINK_SINCE)
            .is_some_and(Ordering::is_ge);
    let mut text = String::from(
        "\
# Config inicial escrita pelo Warden (só chaves documentadas pelo autor do Crash Assistant).
# Pode editar à vontade: o Warden não reescreve este arquivo.
[general]
\t# Não enviar ao autor do mod os metadados dos logs enviados (1.11.12 em diante).
\tsend_uploaded_logs_data_to_kostromdan_dev = false
\t# Copiar o link do próprio mclo.gs, sem visualizador no meio (1.11.12 em diante).
\twrap_link = \"mclo.gs\"
",
    );
    if !wrap_link_era {
        text.push_str(
            "\
\t# Mesma ideia nas versões anteriores à 1.11.12 (só vale com help_link configurado).
\tupload_to = \"mclo.gs\"
",
        );
    }
    text.push_str(
        "
[piracy]
\t# Sem o aviso de pirataria (só vale com help_link configurado).
\tenabled = false
",
    );
    text
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

    /// Um valor como o mod o lê: o tipo do padrão e, para texto, os valores aceitos.
    enum Expected {
        Bool(bool),
        Text(&'static str, &'static [&'static str]),
    }

    fn assert_key(doc: &DocumentMut, path: &str, expected: &Expected, version: &str) {
        let (section, key) = path.split_once('.').unwrap();
        let item = &doc[section][key];
        match expected {
            Expected::Bool(want) => {
                assert_eq!(item.as_bool(), Some(*want), "{version}: {path} = {item:?}");
            }
            Expected::Text(want, accepted) => {
                let got = item.as_str();
                assert_eq!(got, Some(*want), "{version}: {path} = {item:?}");
                assert!(accepted.contains(want), "{version}: {path}");
            }
        }
    }

    /// O que cada faixa do mod lê (tipo do padrão e valores aceitos) e o valor que a decisão D16
    /// pede.
    fn wanted_keys(wrap_link_era: bool) -> Vec<(&'static str, Expected)> {
        let mut keys = vec![("piracy.enabled", Expected::Bool(false))];
        if wrap_link_era {
            keys.push((
                "general.wrap_link",
                Expected::Text("mclo.gs", &["mclo.gs", "gnomebot.dev", "kostromdan.dev"]),
            ));
            keys.push((
                "general.send_uploaded_logs_data_to_kostromdan_dev",
                Expected::Bool(false),
            ));
        } else {
            keys.push((
                "general.upload_to",
                Expected::Text("mclo.gs", &["mclo.gs", "gnomebot.dev"]),
            ));
        }
        keys
    }

    #[test]
    fn config_da_1_11_12_em_diante_usa_wrap_link_em_texto_e_desliga_o_envio() {
        for version in ["1.11.12", "1.11.14", "1.12", "2.0.0"] {
            let doc: DocumentMut = crash_assistant_config(version).parse().unwrap();
            for (path, expected) in wanted_keys(true) {
                assert_key(&doc, path, &expected, version);
            }
            // A chave antiga não existe mais nessas versões: não vai.
            assert!(doc["general"].get("upload_to").is_none(), "{version}");
        }
    }

    #[test]
    fn config_anterior_a_1_11_12_usa_upload_to_e_leva_as_chaves_novas() {
        for version in ["1.11.0", "1.11.11", "1.9.4"] {
            let doc: DocumentMut = crash_assistant_config(version).parse().unwrap();
            for (path, expected) in wanted_keys(false) {
                assert_key(&doc, path, &expected, version);
            }
            // As chaves da 1.11.12, com o tipo certo, para valerem se o mod for atualizado.
            for (path, expected) in wanted_keys(true) {
                assert_key(&doc, path, &expected, version);
            }
        }
    }

    #[test]
    fn versao_que_nao_compara_leva_as_chaves_das_duas_faixas() {
        let doc: DocumentMut = crash_assistant_config("1.11.12-beta").parse().unwrap();
        for wrap_link_era in [true, false] {
            for (path, expected) in wanted_keys(wrap_link_era) {
                assert_key(&doc, path, &expected, "1.11.12-beta");
            }
        }
    }
}
