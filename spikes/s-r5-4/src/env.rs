//! Leitura de um `.env` respeitando aspas simples (o valor entre aspas simples é literal,
//! sem expansão de `$`). Nunca imprime valores.

use std::path::Path;

/// Lê `nome` do arquivo `.env` em `caminho`. Devolve `None` se a variável não existir.
pub fn ler_variavel(caminho: &Path, nome: &str) -> anyhow::Result<Option<String>> {
    let texto = std::fs::read_to_string(caminho)?;
    Ok(procurar(&texto, nome))
}

fn procurar(texto: &str, nome: &str) -> Option<String> {
    for linha in texto.lines() {
        let linha = linha.trim_end_matches('\r').trim_start();
        if linha.starts_with('#') {
            continue;
        }
        let linha = linha.strip_prefix("export ").unwrap_or(linha);
        let Some((chave, valor)) = linha.split_once('=') else { continue };
        if chave.trim() != nome {
            continue;
        }
        let valor = valor.trim();
        let valor = if let Some(resto) = valor.strip_prefix('\'') {
            // Literal até a próxima aspa simples.
            resto.split_once('\'').map(|(v, _)| v).unwrap_or(resto).to_string()
        } else if let Some(resto) = valor.strip_prefix('"') {
            resto.split_once('"').map(|(v, _)| v).unwrap_or(resto).to_string()
        } else {
            // Sem aspas: corta comentário no fim da linha.
            valor.split(" #").next().unwrap_or(valor).trim().to_string()
        };
        return Some(valor);
    }
    None
}

#[cfg(test)]
mod testes {
    use super::procurar;

    #[test]
    fn aspas_simples_sao_literais() {
        let t = "A=1\nCURSEFORGE_API_KEY='$2a$10$abc#def'\r\nGEMINI_API_KEY='AIzaXYZ'\n";
        assert_eq!(procurar(t, "CURSEFORGE_API_KEY").as_deref(), Some("$2a$10$abc#def"));
        assert_eq!(procurar(t, "GEMINI_API_KEY").as_deref(), Some("AIzaXYZ"));
        assert_eq!(procurar(t, "B"), None);
    }

    #[test]
    fn sem_aspas_e_comentario() {
        assert_eq!(procurar("# x\nB = valor # nota\n", "B").as_deref(), Some("valor"));
    }
}
