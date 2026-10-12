//! Montagem do relato de problema (UI-02): o corpo em Markdown, o corte do registro para o
//! endereço caber no limite e o endereço da issue nova no GitHub.
//!
//! Tudo aqui recebe texto **já redigido** ([`ReportParts`]); a redação fica em `mod.rs`.
//! O corpo que vai no endereço é exatamente o que a pré-visualização mostra.

use std::fmt::Write as _;

/// Repositório que recebe os relatos. É o único lugar com este endereço.
pub(crate) const REPOSITORY_URL: &str = "https://github.com/Kriticales/Warden";

/// Rótulo das issues abertas pelo relato (já existe no repositório).
pub(crate) const REPORT_LABEL: &str = "relato-do-dono";

/// Maior endereço aceito. Endereços muito longos falham no navegador ou no GitHub.
pub(crate) const MAX_URL_CHARS: usize = 7_500;

/// Maior título de issue aceito pelo GitHub, em caracteres.
pub(crate) const MAX_TITLE_CHARS: usize = 256;

/// Linha do registro maior que isto é cortada (uma pilha enorme não come o espaço todo).
pub(crate) const MAX_LOG_LINE_CHARS: usize = 1_000;

/// Marca no lugar das linhas antigas do registro que não couberam.
pub(crate) const LOG_CUT_MARK: &str = "… (linhas anteriores cortadas)";

/// Marca no fim do relato quando nem o texto escrito coube.
pub(crate) const TEXT_CUT_MARK: &str =
    "… (relato cortado: a versão completa foi copiada para a área de transferência)";

/// Endereço da página de issue nova, sem os parâmetros.
pub(crate) fn new_issue_url() -> String {
    format!("{REPOSITORY_URL}/issues/new")
}

/// Os pedaços do relato, já redigidos.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct ReportParts {
    /// Título da issue.
    pub(crate) title: String,
    /// "O que você fez".
    pub(crate) what_you_did: String,
    /// "O que aconteceu".
    pub(crate) what_happened: String,
    /// "O que você esperava".
    pub(crate) what_you_expected: String,
    /// Dados coletados sozinhos: rótulo e valor, na ordem.
    pub(crate) environment: Vec<(String, String)>,
    /// Últimas linhas do registro, da mais antiga para a mais nova; `None` = não incluir.
    pub(crate) log: Option<Vec<String>>,
}

/// O que precisou ser cortado para o endereço caber.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Cut {
    /// Nada: o relato inteiro está no endereço.
    None,
    /// Linhas antigas do registro.
    Log,
    /// O registro inteiro e parte do texto escrito.
    Text,
}

/// O relato pronto para abrir.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FittedReport {
    /// Título que vai no endereço.
    pub(crate) title: String,
    /// Corpo que vai no endereço (o que a pré-visualização mostra).
    pub(crate) body: String,
    /// Corpo com o registro inteiro (vai para a área de transferência quando houve corte).
    pub(crate) full_body: String,
    /// Endereço da issue nova, com título, corpo e rótulo.
    pub(crate) url: String,
    /// O que foi cortado.
    pub(crate) cut: Cut,
    /// Linhas do registro que ficaram no corpo.
    pub(crate) log_lines: usize,
}

/// Monta o relato e corta o que for preciso para o endereço caber em [`MAX_URL_CHARS`]:
/// primeiro as linhas mais antigas do registro, depois (só se nem sem registro couber) o fim
/// do texto.
pub(crate) fn fit(parts: &ReportParts) -> FittedReport {
    let title = truncate_chars(parts.title.trim(), MAX_TITLE_CHARS);
    let log: Vec<String> = parts
        .log
        .iter()
        .flatten()
        .map(|line| truncate_line(line))
        .collect();
    let full_body = body(parts, parts.log.as_ref().map(|_| LogView::All(&log)));
    let full_url = issue_url(&title, &full_body);
    if full_url.chars().count() <= MAX_URL_CHARS {
        return FittedReport {
            title,
            body: full_body.clone(),
            full_body,
            url: full_url,
            cut: Cut::None,
            log_lines: log.len(),
        };
    }

    // Quantas linhas mais novas cabem (busca binária: mais linhas, endereço maior).
    let with_newest = |kept: usize| body(parts, Some(LogView::Newest(&log, kept)));
    let fits = |body: &str| issue_url(&title, body).chars().count() <= MAX_URL_CHARS;
    if parts.log.is_some() && fits(&with_newest(0)) {
        let (mut low, mut high) = (0, log.len());
        while low < high {
            let middle = (low + high).div_ceil(2);
            if fits(&with_newest(middle)) {
                low = middle;
            } else {
                high = middle - 1;
            }
        }
        let body = with_newest(low);
        return FittedReport {
            url: issue_url(&title, &body),
            title,
            body,
            full_body,
            cut: Cut::Log,
            log_lines: low,
        };
    }

    // Nem sem o registro coube: corta o fim do texto escrito.
    let without_log = body(parts, None);
    let chars: Vec<char> = without_log.chars().collect();
    let cut_at = |count: usize| {
        let mut text: String = chars[..count].iter().collect();
        text.truncate(text.trim_end().len());
        text.push_str("\n\n");
        text.push_str(TEXT_CUT_MARK);
        text.push('\n');
        text
    };
    let (mut low, mut high) = (0, chars.len());
    while low < high {
        let middle = (low + high).div_ceil(2);
        if fits(&cut_at(middle)) {
            low = middle;
        } else {
            high = middle - 1;
        }
    }
    let body = cut_at(low);
    FittedReport {
        url: issue_url(&title, &body),
        title,
        body,
        full_body,
        cut: Cut::Text,
        log_lines: 0,
    }
}

/// Que parte do registro entra no corpo.
enum LogView<'a> {
    /// Todas as linhas.
    All(&'a [String]),
    /// Só as `n` mais novas, com a marca de corte no lugar das outras.
    Newest(&'a [String], usize),
}

/// O corpo da issue em Markdown. Seções opcionais vazias não aparecem.
fn body(parts: &ReportParts, log: Option<LogView<'_>>) -> String {
    let mut text = String::new();
    for (heading, value) in [
        ("O que você fez", &parts.what_you_did),
        ("O que aconteceu", &parts.what_happened),
        ("O que você esperava", &parts.what_you_expected),
    ] {
        let value = value.trim();
        if !value.is_empty() {
            let _ = write!(text, "### {heading}\n\n{value}\n\n");
        }
    }
    text.push_str("### Ambiente\n\n");
    for (label, value) in &parts.environment {
        let _ = writeln!(text, "- **{label}:** {value}");
    }
    text.push('\n');
    if let Some(log) = log {
        let (lines, cut) = match log {
            LogView::All(lines) => (lines, false),
            LogView::Newest(lines, kept) => {
                let kept = kept.min(lines.len());
                (&lines[lines.len() - kept..], kept < lines.len())
            }
        };
        text.push_str("### Registros do Warden (últimas linhas)\n\n");
        if lines.is_empty() && !cut {
            text.push_str("_Sem linhas no registro._\n\n");
        } else {
            let fence = fence_for(lines);
            let _ = writeln!(text, "{fence}text");
            if cut {
                let _ = writeln!(text, "{LOG_CUT_MARK}");
            }
            for line in lines {
                let _ = writeln!(text, "{line}");
            }
            let _ = write!(text, "{fence}\n\n");
        }
    }
    text.push_str(
        "_Relato aberto pelo botão \"Relatar problema\" do Warden. Pastas pessoais, nomes e \
         chaves foram trocados por marcadores como `<usuário>` e `<segredo>`._\n",
    );
    text
}

/// Cerca de código maior que qualquer sequência de crases das linhas (o registro não fecha o
/// bloco antes da hora).
fn fence_for(lines: &[String]) -> String {
    let longest = lines
        .iter()
        .map(|line| {
            line.split(|c| c != '`')
                .map(str::len)
                .max()
                .unwrap_or(0)
        })
        .max()
        .unwrap_or(0);
    "`".repeat(longest.max(2) + 1)
}

/// Endereço da issue nova com título, corpo e rótulo.
pub(crate) fn issue_url(title: &str, body: &str) -> String {
    format!(
        "{}?title={}&labels={}&body={}",
        new_issue_url(),
        encode(title),
        encode(REPORT_LABEL),
        encode(body)
    )
}

/// Codifica para a query do endereço: tudo fora de `A-Z a-z 0-9 - . _ ~` vira `%XX` (UTF-8).
pub(crate) fn encode(text: &str) -> String {
    let mut encoded = String::with_capacity(text.len());
    for byte in text.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            encoded.push(char::from(byte));
        } else {
            let _ = write!(encoded, "%{byte:02X}");
        }
    }
    encoded
}

fn truncate_chars(text: &str, max: usize) -> String {
    match text.char_indices().nth(max) {
        Some((index, _)) => text[..index].to_owned(),
        None => text.to_owned(),
    }
}

fn truncate_line(line: &str) -> String {
    let line = line.trim_end();
    if line.chars().count() <= MAX_LOG_LINE_CHARS {
        line.to_owned()
    } else {
        format!("{}…", truncate_chars(line, MAX_LOG_LINE_CHARS - 1))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parts(log: Option<Vec<String>>) -> ReportParts {
        ReportParts {
            title: "O Testar travou".into(),
            what_you_did: "Cliquei em Testar".into(),
            what_happened: "Nada abriu".into(),
            what_you_expected: String::new(),
            environment: vec![
                ("Warden".into(), "0.1.0".into()),
                ("Tela".into(), "Pack › Mods".into()),
            ],
            log,
        }
    }

    fn decode(text: &str) -> String {
        let bytes = text.as_bytes();
        let mut out = Vec::new();
        let mut index = 0;
        while index < bytes.len() {
            if bytes[index] == b'%' {
                let hex = std::str::from_utf8(&bytes[index + 1..index + 3]).unwrap();
                out.push(u8::from_str_radix(hex, 16).unwrap());
                index += 3;
            } else {
                out.push(bytes[index]);
                index += 1;
            }
        }
        String::from_utf8(out).unwrap()
    }

    fn query(url: &str, name: &str) -> String {
        let query = url.split_once('?').unwrap().1;
        let value = query
            .split('&')
            .find_map(|pair| pair.strip_prefix(&format!("{name}=")))
            .unwrap();
        decode(value)
    }

    #[test]
    fn corpo_com_secoes_ambiente_e_registro() {
        let fitted = fit(&parts(Some(vec!["linha 1".into(), "linha 2".into()])));
        assert_eq!(fitted.cut, Cut::None);
        assert_eq!(fitted.body, fitted.full_body);
        assert_eq!(fitted.log_lines, 2);
        let expected = "### O que você fez\n\nCliquei em Testar\n\n\
             ### O que aconteceu\n\nNada abriu\n\n\
             ### Ambiente\n\n- **Warden:** 0.1.0\n- **Tela:** Pack › Mods\n\n\
             ### Registros do Warden (últimas linhas)\n\n```text\nlinha 1\nlinha 2\n```\n\n";
        assert!(fitted.body.starts_with(expected), "{}", fitted.body);
        assert!(!fitted.body.contains("O que você esperava"));
    }

    #[test]
    fn endereco_tem_titulo_corpo_e_rotulo() {
        let fitted = fit(&parts(None));
        assert!(
            fitted
                .url
                .starts_with("https://github.com/Kriticales/Warden/issues/new?title=")
        );
        assert_eq!(query(&fitted.url, "title"), "O Testar travou");
        assert_eq!(query(&fitted.url, "labels"), "relato-do-dono");
        assert_eq!(query(&fitted.url, "body"), fitted.body);
        assert!(!fitted.body.contains("Registros do Warden"));
        assert!(fitted.url.is_ascii());
        assert!(!fitted.url.contains(' '));
    }

    #[test]
    fn registro_vazio_aparece_como_vazio() {
        let fitted = fit(&parts(Some(Vec::new())));
        assert!(fitted.body.contains("_Sem linhas no registro._"));
    }

    #[test]
    fn registro_grande_perde_as_linhas_antigas() {
        let log: Vec<String> = (1..=400)
            .map(|n| format!("2026-10-12T10:00:00Z INFO warden: linha número {n:03}"))
            .collect();
        let fitted = fit(&parts(Some(log)));
        assert_eq!(fitted.cut, Cut::Log);
        assert!(fitted.url.chars().count() <= MAX_URL_CHARS);
        // Fica a mais nova, sai a mais antiga, e a marca avisa.
        assert!(fitted.body.contains("linha número 400"));
        assert!(!fitted.body.contains("linha número 001"));
        assert!(fitted.body.contains(LOG_CUT_MARK));
        assert!(fitted.log_lines > 0 && fitted.log_lines < 400);
        // O corpo completo continua com tudo.
        assert!(fitted.full_body.contains("linha número 001"));
        assert!(!fitted.full_body.contains(LOG_CUT_MARK));
        // É o máximo que cabe: uma linha a mais passaria do limite.
        let mut more = parts(None);
        more.log = Some(
            (1..=400)
                .map(|n| format!("2026-10-12T10:00:00Z INFO warden: linha número {n:03}"))
                .collect(),
        );
        let one_more = body(
            &more,
            Some(LogView::Newest(
                more.log.as_ref().unwrap(),
                fitted.log_lines + 1,
            )),
        );
        assert!(issue_url(&fitted.title, &one_more).chars().count() > MAX_URL_CHARS);
    }

    #[test]
    fn texto_enorme_corta_o_texto_e_tira_o_registro() {
        let mut big = parts(Some(vec!["linha".into()]));
        big.what_happened = "ação é ".repeat(2_000);
        let fitted = fit(&big);
        assert_eq!(fitted.cut, Cut::Text);
        assert!(fitted.url.chars().count() <= MAX_URL_CHARS);
        assert!(fitted.body.ends_with(&format!("{TEXT_CUT_MARK}\n")));
        assert!(!fitted.body.contains("Registros do Warden"));
        assert!(fitted.full_body.contains("Registros do Warden"));
        assert_eq!(query(&fitted.url, "body"), fitted.body);
    }

    #[test]
    fn titulo_longo_e_linha_longa_sao_cortados() {
        let mut long = parts(Some(vec!["x".repeat(5_000)]));
        long.title = "t".repeat(400);
        let fitted = fit(&long);
        assert_eq!(fitted.title.chars().count(), MAX_TITLE_CHARS);
        assert!(fitted.full_body.contains(&format!("{}…", "x".repeat(999))));
        assert!(!fitted.full_body.contains(&"x".repeat(1_000)));
    }

    #[test]
    fn crases_do_registro_nao_fecham_o_bloco() {
        let fitted = fit(&parts(Some(vec!["veja ```isto``` e ````aquilo````".into()])));
        assert!(fitted.body.contains("`````text\n"), "{}", fitted.body);
    }

    #[test]
    fn codifica_utf8_e_reservados() {
        assert_eq!(encode("a b&c=d"), "a%20b%26c%3Dd");
        assert_eq!(encode("ç›\n"), "%C3%A7%E2%80%BA%0A");
        assert_eq!(encode("A-z_0.9~"), "A-z_0.9~");
    }
}
