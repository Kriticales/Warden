//! Ferramentas do jogador na instância de teste (ADR-0033; decisão D16; SPEC T13).
//!
//! O pack marca o spark e o Crash Assistant como ferramentas do jogador na tabela
//! `[player-tools]` de `.warden/project.toml` (a `warden-project` escreve; aqui só se lê):
//!
//! ```toml
//! [player-tools]
//! spark = "mods/spark.pw.toml"
//! crash-assistant = "mods/crash-assistant.pw.toml"
//! ```
//!
//! O `index.toml` do pack continua com os dois: o que muda é o que o Warden **copia para a
//! instância**, conforme o modo ([`PlayerToolsMode`]):
//!
//! | Modo | spark | Crash Assistant |
//! |---|---|---|
//! | [`Normal`](PlayerToolsMode::Normal) (testes normais) | entra | **fora** (a janela dele atrapalharia o resultado do Warden) |
//! | [`AsPlayer`](PlayerToolsMode::AsPlayer) ("Testar como o jogador recebe") | entra | entra |
//! | [`Culprit`](PlayerToolsMode::Culprit) (busca do culpado) | fora | fora |
//!
//! Na busca do culpado, uma ferramenta que está entre os suspeitos continua valendo como um mod
//! qualquer e entra. Uma instância que já tinha o item o perde na próxima materialização, como
//! qualquer item que saiu do pack.
//!
//! O Warden nunca lê o código do Crash Assistant nem os arquivos que ele gera.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use warden_packwiz::clean_path;

/// Arquivo do Warden dentro do pack.
const PROJECT_FILE: &str = ".warden/project.toml";

/// Tabela das ferramentas do jogador.
const TABLE: &str = "player-tools";

/// Papel do Crash Assistant.
pub const ROLE_CRASH_ASSISTANT: &str = "crash-assistant";

/// Quais ferramentas do jogador entram na instância.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum PlayerToolsMode {
    /// Teste normal: o Crash Assistant fica fora; o spark entra.
    #[default]
    Normal,
    /// "Testar como o jogador recebe": as duas entram.
    AsPlayer,
    /// Busca do culpado: as duas ficam fora, a menos que estejam entre os suspeitos.
    Culprit {
        /// Caminhos (relativos ao pack, com `/`) dos metafiles suspeitos.
        suspects: BTreeSet<String>,
    },
}

/// Uma ferramenta marcada no pack.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Marked {
    role: String,
    path: String,
}

fn read_marked(pack_root: &Path) -> Vec<Marked> {
    let text = match fs::read_to_string(pack_root.join(PROJECT_FILE)) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Vec::new(),
        Err(error) => {
            tracing::warn!(%error, "não foi possível ler {PROJECT_FILE}; ferramentas do jogador ignoradas");
            return Vec::new();
        }
    };
    let table: toml::Table = match text.parse() {
        Ok(table) => table,
        Err(error) => {
            tracing::warn!(%error, "{PROJECT_FILE} inválido; ferramentas do jogador ignoradas");
            return Vec::new();
        }
    };
    table
        .get(TABLE)
        .and_then(toml::Value::as_table)
        .map(|tools| {
            tools
                .iter()
                .filter_map(|(role, path)| {
                    path.as_str().map(|path| Marked {
                        role: role.clone(),
                        path: clean_path(&path.replace('\\', "/")),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Caminhos dos metafiles (relativos ao pack) que ficam **fora** da instância neste modo.
///
/// Pack sem `[player-tools]`, sem `.warden/project.toml` ou com ele ilegível: nada fica fora.
#[must_use]
pub fn excluded_paths(pack_root: &Path, mode: &PlayerToolsMode) -> BTreeSet<String> {
    let marked = read_marked(pack_root);
    marked
        .into_iter()
        .filter(|tool| match mode {
            PlayerToolsMode::Normal => tool.role == ROLE_CRASH_ASSISTANT,
            PlayerToolsMode::AsPlayer => false,
            PlayerToolsMode::Culprit { suspects } => !suspects.contains(&tool.path),
        })
        .map(|tool| tool.path)
        .collect()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use crate::test_support::temp_dir;

    fn pack(project: &str) -> tempfile::TempDir {
        let dir = temp_dir();
        fs::create_dir_all(dir.path().join(".warden")).unwrap();
        fs::write(dir.path().join(PROJECT_FILE), project).unwrap();
        dir
    }

    const TOOLS: &str = "id = \"a\"\n[player-tools]\nspark = \"mods/spark.pw.toml\"\ncrash-assistant = \"mods\\\\crash.pw.toml\"\n";

    fn paths(set: &BTreeSet<String>) -> Vec<&str> {
        set.iter().map(String::as_str).collect()
    }

    #[test]
    fn teste_normal_tira_so_o_crash_assistant() {
        let dir = pack(TOOLS);
        let out = excluded_paths(dir.path(), &PlayerToolsMode::Normal);
        assert_eq!(paths(&out), ["mods/crash.pw.toml"]);
    }

    #[test]
    fn testar_como_o_jogador_recebe_traz_os_dois() {
        let dir = pack(TOOLS);
        assert!(excluded_paths(dir.path(), &PlayerToolsMode::AsPlayer).is_empty());
    }

    #[test]
    fn busca_do_culpado_tira_os_dois_menos_os_suspeitos() {
        let dir = pack(TOOLS);
        let none = PlayerToolsMode::Culprit {
            suspects: BTreeSet::new(),
        };
        assert_eq!(
            paths(&excluded_paths(dir.path(), &none)),
            ["mods/crash.pw.toml", "mods/spark.pw.toml"]
        );
        let spark_suspect = PlayerToolsMode::Culprit {
            suspects: BTreeSet::from(["mods/spark.pw.toml".to_owned()]),
        };
        assert_eq!(
            paths(&excluded_paths(dir.path(), &spark_suspect)),
            ["mods/crash.pw.toml"]
        );
    }

    #[test]
    fn sem_marcacao_ou_com_arquivo_ruim_nada_fica_fora() {
        let empty = temp_dir();
        assert!(excluded_paths(empty.path(), &PlayerToolsMode::Normal).is_empty());
        for text in ["id = \"a\"\n", "id = [", "[player-tools]\nspark = 3\n"] {
            let dir = pack(text);
            assert!(
                excluded_paths(dir.path(), &PlayerToolsMode::Normal).is_empty(),
                "{text}"
            );
        }
    }
}
