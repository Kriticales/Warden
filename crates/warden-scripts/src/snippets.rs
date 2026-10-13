//! Trechos prontos por versão (ARCHITECTURE §10.2): os eventos mais usados de cada linha do
//! KubeJS e do CraftTweaker, em **dados** (`data/snippets/*.toml`), não em código.
//!
//! Cada arquivo diz a ferramenta e a faixa de linhas (`min`, `max`, inclusivas) em que vale. O
//! nome que a interface mostra é do i18n (`scripts:trechos.<id>`); aqui ficam só o `id`, o tipo
//! de script em que o trecho funciona (KubeJS) e o corpo, com os campos do `CodeMirror`
//! (`${texto}`).

use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

use crate::detect::{KubejsScriptType, ScriptTool, ToolVersion};

/// Os arquivos de dados, embutidos no binário.
const FILES: [(&str, &str); 5] = [
    (
        "kubejs-5.toml",
        include_str!("../data/snippets/kubejs-5.toml"),
    ),
    (
        "kubejs-6.toml",
        include_str!("../data/snippets/kubejs-6.toml"),
    ),
    (
        "kubejs-7.toml",
        include_str!("../data/snippets/kubejs-7.toml"),
    ),
    (
        "crafttweaker-1.12.toml",
        include_str!("../data/snippets/crafttweaker-1.12.toml"),
    ),
    (
        "crafttweaker.toml",
        include_str!("../data/snippets/crafttweaker.toml"),
    ),
];

/// Um trecho pronto.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct Snippet {
    /// Identificador estável (nome na interface: `scripts:trechos.<id>`).
    pub id: String,
    /// Tipo de script em que o trecho funciona (só KubeJS).
    pub script_type: Option<KubejsScriptType>,
    /// O texto inserido, com os campos `${…}` do `CodeMirror`.
    pub body: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SnippetFile {
    ferramenta: String,
    min: Option<u32>,
    max: Option<u32>,
    trecho: Vec<RawSnippet>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawSnippet {
    id: String,
    tipo: Option<String>,
    corpo: String,
}

/// Um arquivo já lido.
#[derive(Debug)]
struct Set {
    tool: ScriptTool,
    min: u32,
    max: u32,
    snippets: Vec<Snippet>,
}

/// Lê um arquivo de trechos.
fn parse(name: &str, text: &str) -> Result<Set, String> {
    let file: SnippetFile = toml::from_str(text).map_err(|error| format!("{name}: {error}"))?;
    let tool = match file.ferramenta.as_str() {
        "kubejs" => ScriptTool::Kubejs,
        "crafttweaker" => ScriptTool::Crafttweaker,
        other => return Err(format!("{name}: ferramenta desconhecida {other}")),
    };
    let mut snippets = Vec::new();
    for raw in file.trecho {
        let script_type = match raw.tipo.as_deref() {
            None => None,
            Some("startup") => Some(KubejsScriptType::Startup),
            Some("server") => Some(KubejsScriptType::Server),
            Some("client") => Some(KubejsScriptType::Client),
            Some(other) => return Err(format!("{name}: tipo desconhecido {other}")),
        };
        if raw.id.is_empty() || raw.corpo.trim().is_empty() {
            return Err(format!("{name}: trecho sem id ou sem corpo"));
        }
        snippets.push(Snippet {
            id: raw.id,
            script_type,
            body: raw.corpo,
        });
    }
    Ok(Set {
        tool,
        min: file.min.unwrap_or(0),
        max: file.max.unwrap_or(u32::MAX),
        snippets,
    })
}

fn sets() -> &'static [Set] {
    static SETS: OnceLock<Vec<Set>> = OnceLock::new();
    SETS.get_or_init(|| {
        FILES
            .iter()
            .filter_map(|(name, text)| match parse(name, text) {
                Ok(set) => Some(set),
                Err(error) => {
                    // Os testes garantem que os arquivos embutidos são válidos.
                    tracing::error!(%error, "trechos prontos inválidos");
                    None
                }
            })
            .collect()
    })
}

/// Os trechos da ferramenta na versão detectada (nenhum se a linha não pôde ser lida).
#[must_use]
pub fn snippets_for(tool: &ToolVersion) -> Vec<Snippet> {
    let Some(major) = tool.major else {
        return Vec::new();
    };
    sets()
        .iter()
        .filter(|set| set.tool == tool.tool && (set.min..=set.max).contains(&major))
        .flat_map(|set| set.snippets.iter().cloned())
        .collect()
}

/// Todos os `id` dos arquivos de dados (para conferir o i18n).
#[must_use]
pub fn all_ids() -> Vec<&'static str> {
    let mut ids: Vec<&str> = sets()
        .iter()
        .flat_map(|set| set.snippets.iter().map(|snippet| snippet.id.as_str()))
        .collect();
    ids.sort_unstable();
    ids.dedup();
    ids
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tool(tool: ScriptTool, major: Option<u32>) -> ToolVersion {
        ToolVersion {
            tool,
            major,
            version: None,
        }
    }

    fn bodies(tool: &ToolVersion) -> Vec<(String, String)> {
        snippets_for(tool)
            .into_iter()
            .map(|snippet| (snippet.id, snippet.body))
            .collect()
    }

    #[test]
    fn arquivos_embutidos_sao_validos_e_sem_sobreposicao() {
        let mut parsed = Vec::new();
        for (name, text) in FILES {
            parsed.push(parse(name, text).unwrap());
        }
        assert_eq!(sets().len(), FILES.len());
        // Duas faixas da mesma ferramenta nunca se cruzam: cada versão tem um conjunto só.
        for (i, a) in parsed.iter().enumerate() {
            for b in &parsed[i + 1..] {
                if a.tool == b.tool {
                    assert!(a.max < b.min || b.max < a.min, "{a:?} x {b:?}");
                }
            }
            let mut ids: Vec<&str> = a.snippets.iter().map(|s| s.id.as_str()).collect();
            ids.sort_unstable();
            let total = ids.len();
            ids.dedup();
            assert_eq!(ids.len(), total, "id repetido");
        }
    }

    #[test]
    fn kubejs_6_tem_os_eventos_da_versao() {
        let found = bodies(&tool(ScriptTool::Kubejs, Some(6)));
        let ids: Vec<&str> = found.iter().map(|(id, _)| id.as_str()).collect();
        assert_eq!(
            ids,
            [
                "receita-com-forma",
                "receita-sem-forma",
                "remover-receita",
                "tag-de-item",
                "item-novo",
                "dica-de-item"
            ]
        );
        assert!(found[0].1.starts_with("ServerEvents.recipes(event => {"));
        assert!(found[3].1.starts_with("ServerEvents.tags('item'"));
        assert!(found[4].1.starts_with("StartupEvents.registry('item'"));
        assert!(found[5].1.starts_with("ItemEvents.tooltip("));
        let tipos: Vec<Option<KubejsScriptType>> = snippets_for(&tool(ScriptTool::Kubejs, Some(6)))
            .into_iter()
            .map(|s| s.script_type)
            .collect();
        assert_eq!(tipos[4], Some(KubejsScriptType::Startup));
        assert_eq!(tipos[5], Some(KubejsScriptType::Client));
    }

    #[test]
    fn versoes_diferentes_trocam_os_eventos() {
        let five = bodies(&tool(ScriptTool::Kubejs, Some(5)));
        assert!(five[0].1.starts_with("onEvent('recipes'"));
        let seven = bodies(&tool(ScriptTool::Kubejs, Some(7)));
        assert!(
            seven
                .iter()
                .any(|(_, body)| body.contains("ItemEvents.modifyTooltips"))
        );
        let eight = bodies(&tool(ScriptTool::Kubejs, Some(8)));
        assert_eq!(eight, seven);

        let modern = bodies(&tool(ScriptTool::Crafttweaker, Some(14)));
        assert!(modern[0].1.starts_with("craftingTable.addShaped("));
        let classic = bodies(&tool(ScriptTool::Crafttweaker, Some(4)));
        assert!(classic[0].1.starts_with("recipes.addShaped(<"));
        assert!(bodies(&tool(ScriptTool::Kubejs, None)).is_empty());
    }

    #[test]
    fn ids_para_o_i18n() {
        assert_eq!(
            all_ids(),
            [
                "dica-de-item",
                "fornalha",
                "item-novo",
                "receita-com-forma",
                "receita-sem-forma",
                "remover-receita",
                "tag-de-item"
            ]
        );
    }

    #[test]
    fn recusa_dados_invalidos() {
        assert!(parse("a", "ferramenta = \"x\"\n[[trecho]]\nid=\"a\"\ncorpo=\"b\"").is_err());
        assert!(
            parse(
                "a",
                "ferramenta = \"kubejs\"\n[[trecho]]\nid=\"a\"\ntipo=\"x\"\ncorpo=\"b\""
            )
            .is_err()
        );
        assert!(
            parse(
                "a",
                "ferramenta = \"kubejs\"\n[[trecho]]\nid=\"\"\ncorpo=\"b\""
            )
            .is_err()
        );
        assert!(parse("a", "ferramenta = \"kubejs\"\noutro = 1\ntrecho = []").is_err());
    }
}
