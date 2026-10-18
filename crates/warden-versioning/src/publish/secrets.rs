//! Varredura de segredos no conteúdo a publicar (ARCHITECTURE §11.1 item 1; SPEC T18,
//! CA-T18-06): algo com cara de chave ou token **bloqueia** a publicação, apontando arquivo e
//! linha. O valor encontrado nunca é guardado nem mostrado.
//!
//! Procura:
//! - as formas conhecidas da tabela da ARCHITECTURE §9.4 (chave da CurseForge `$2a$…`, tokens do
//!   GitHub `ghp_…`/`gho_…`/`github_pat_…`, chave do Google `AIza…`, chaves da OpenAI e da
//!   Anthropic `sk-…`, JWT);
//! - valores longos depois de nomes de chave (`api_key = "…"`, `"accessToken": "…"`,
//!   `password=…`), só quando o valor tem 16 caracteres ou mais com letras e números (para
//!   `token = 5` ou `secret = true` de configs comuns não bloquearem);
//! - os valores atuais do cofre (todas as chaves: CurseForge, provedores de IA, GitHub), em
//!   qualquer lugar;
//! - pelo nome, `kubejs/config/web_server.json` (guarda o token do servidor web do KubeJS).
//!
//! Arquivos binários (com byte nulo no começo) ficam de fora: jars e imagens não guardam
//! configuração digitada.

use std::sync::LazyLock;

use regex::Regex;
use secrecy::{ExposeSecret as _, SecretString};
use serde::{Deserialize, Serialize};

use super::PublishTree;

/// Arquivos que guardam segredo pelo próprio propósito, achados pelo nome.
const SECRET_FILES: [&str; 1] = ["kubejs/config/web_server.json"];

/// Menor valor do cofre procurado (valores curtos casariam com texto comum).
const MIN_STORED_SECRET: usize = 8;

/// Quanto do começo do arquivo decide se ele é binário.
const BINARY_PROBE: usize = 8 * 1024;

/// Algo com cara de chave no conteúdo a publicar.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SecretFinding {
    /// Arquivo, relativo à raiz do pack.
    pub path: String,
    /// Linha (a partir de 1).
    pub line: u32,
}

#[allow(clippy::expect_used)] // expressões fixas, conferidas pelos testes
fn fixed(pattern: &str) -> Regex {
    Regex::new(pattern).expect("expressão fixa válida")
}

static SHAPES: LazyLock<Regex> = LazyLock::new(|| {
    fixed(concat!(
        r"(?:\$2[abxy]?\$\d{2}\$[./A-Za-z0-9]{20,}",
        r"|\bgh[pousr]_[A-Za-z0-9]{20,}",
        r"|\bgithub_pat_[A-Za-z0-9_]{20,}",
        r"|\bAIza[0-9A-Za-z_\-]{20,}",
        r"|\bsk-[A-Za-z0-9_\-]{20,}",
        r"|\beyJ[A-Za-z0-9_\-]{8,}\.eyJ[A-Za-z0-9_\-]{8,}\.[A-Za-z0-9_\-]{8,}",
        r")",
    ))
});

static KEY_VALUES: LazyLock<Regex> = LazyLock::new(|| {
    fixed(concat!(
        r#"(?i)(?:x-api-key|api[_-]?key|apikey|access[_-]?token|auth[_-]?token|refresh[_-]?token"#,
        r#"|client[_-]?secret|bot[_-]?token|webhook[_-]?url|password|passwd|secret|token)"#,
        r#"["']?\s*[:=]\s*["']?(?P<value>[A-Za-z0-9_\-./+=:]{16,})"#,
    ))
});

fn looks_secret(value: &str) -> bool {
    value.chars().any(|c| c.is_ascii_alphabetic()) && value.chars().any(|c| c.is_ascii_digit())
}

fn line_of(text: &str, offset: usize) -> u32 {
    let line = text[..offset].matches('\n').count() + 1;
    u32::try_from(line).unwrap_or(u32::MAX)
}

fn is_binary(bytes: &[u8]) -> bool {
    bytes[..bytes.len().min(BINARY_PROBE)].contains(&0)
}

/// Procura segredos em todos os arquivos de `tree`. `stored` são os valores do cofre.
/// Devolve um achado por arquivo e linha, em ordem.
#[must_use]
pub fn scan_secrets(tree: &PublishTree, stored: &[SecretString]) -> Vec<SecretFinding> {
    let stored: Vec<&str> = stored
        .iter()
        .map(|secret| secret.expose_secret().trim())
        .filter(|value| value.len() >= MIN_STORED_SECRET)
        .collect();
    let mut found = std::collections::BTreeSet::new();
    for (path, bytes) in tree {
        if SECRET_FILES.contains(&path.as_str()) {
            found.insert(SecretFinding {
                path: path.clone(),
                line: 1,
            });
        }
        if is_binary(bytes) {
            continue;
        }
        let text = String::from_utf8_lossy(bytes);
        let mut add = |offset: usize| {
            found.insert(SecretFinding {
                path: path.clone(),
                line: line_of(&text, offset),
            });
        };
        for hit in SHAPES.find_iter(&text) {
            add(hit.start());
        }
        for captures in KEY_VALUES.captures_iter(&text) {
            if let Some(value) = captures.name("value")
                && looks_secret(value.as_str())
            {
                add(value.start());
            }
        }
        for value in &stored {
            for (offset, _) in text.match_indices(value) {
                add(offset);
            }
        }
    }
    found.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tree(files: &[(&str, &[u8])]) -> PublishTree {
        files
            .iter()
            .map(|(path, bytes)| ((*path).to_owned(), bytes.to_vec()))
            .collect()
    }

    fn at(path: &str, line: u32) -> SecretFinding {
        SecretFinding {
            path: path.into(),
            line,
        }
    }

    #[test]
    fn token_ghp_aponta_arquivo_e_linha() {
        let files = tree(&[(
            "config/x.toml",
            b"[geral]\nnome = \"a\"\ntoken = \"ghp_abcdefghijklmnopqrstuvwxyz0123456789\"\n",
        )]);
        assert_eq!(scan_secrets(&files, &[]), vec![at("config/x.toml", 3)]);
    }

    #[test]
    fn formas_conhecidas() {
        let cases: [&[u8]; 7] = [
            b"key: $2a$10$abcdefghijklmnopqrstuvwxyzABCD",
            b"github_pat_11ABCDEFG0123456789_abcdefghijklmnop",
            b"gho_ABCDEFGHIJKLMNOPQRSTUVWX",
            b"x AIzaSyA-0123456789abcdefghijklmn",
            b"OPENAI_API_KEY=sk-proj-0123456789abcdefghij_ABCDEF",
            b"chave: sk-ant-api03-0123456789abcdefghij_ABCDEF",
            b"eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.abcdefghijk",
        ];
        for case in cases {
            let files = tree(&[("config/a.txt", case)]);
            assert_eq!(
                scan_secrets(&files, &[]),
                vec![at("config/a.txt", 1)],
                "{}",
                String::from_utf8_lossy(case)
            );
        }
    }

    #[test]
    fn nomes_de_chave_com_valor_longo() {
        let files = tree(&[(
            "config/bot.json",
            b"{\n  \"botToken\": \"MTIzNDU2Nzg5MDEyMzQ1Njc4.abc\",\n  \"apiKey\": \"a1b2c3d4e5f6a7b8c9d0\"\n}",
        )]);
        assert_eq!(
            scan_secrets(&files, &[]),
            vec![at("config/bot.json", 2), at("config/bot.json", 3)]
        );
    }

    #[test]
    fn configs_comuns_nao_bloqueiam() {
        let files = tree(&[(
            "config/mod.toml",
            b"token = 5\nsecret = true\npassword = \"\"\nsecretRooms = 3\napi_key = \"COLOQUE_SUA_CHAVE_AQUI\"\ncolor = \"0123456789abcdef0123\"\n",
        )]);
        assert!(scan_secrets(&files, &[]).is_empty());
    }

    #[test]
    fn valor_do_cofre_em_qualquer_lugar_e_curto_ignorado() {
        let stored = [
            SecretString::from("minha-chave-gemini-123".to_owned()),
            SecretString::from("abc".to_owned()),
        ];
        let files = tree(&[(
            "kubejs/startup_scripts/a.js",
            b"// abc\nconst k = 'xx minha-chave-gemini-123 yy'\n",
        )]);
        assert_eq!(
            scan_secrets(&files, &stored),
            vec![at("kubejs/startup_scripts/a.js", 2)]
        );
    }

    #[test]
    fn arquivo_de_segredo_pelo_nome_e_binario_ignorado() {
        let files = tree(&[
            ("kubejs/config/web_server.json", b"{}"),
            (
                "mods/local.jar",
                b"PK\x03\x04\0\0ghp_abcdefghijklmnopqrstuvwxyz0123456789",
            ),
        ]);
        assert_eq!(
            scan_secrets(&files, &[]),
            vec![at("kubejs/config/web_server.json", 1)]
        );
    }

    #[test]
    fn crlf_conta_as_linhas_certo() {
        let files = tree(&[(
            "config/c.cfg",
            b"a\r\nb\r\nS:token=ghp_abcdefghijklmnopqrstuvwxyz0123456789\r\n",
        )]);
        assert_eq!(scan_secrets(&files, &[]), vec![at("config/c.cfg", 3)]);
    }
}
