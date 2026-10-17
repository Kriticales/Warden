//! `W_CONFIG_RANGE`: valor de config fora da faixa ou dos valores permitidos que o próprio
//! arquivo informa (SPEC T12 e T14, CA-T14-10; ARCHITECTURE §9.2 e §10.1; R5B §2.3; C-06).
//!
//! O Forge e o NeoForge conferem cada valor contra o spec ao carregar a config e trocam o que
//! estiver fora pelo padrão, sem avisar na tela (no NeoForge, também guardam uma cópia
//! `<nome>-1.toml.bak`). Antes de testar, o Warden avisa: o valor, o limite declarado e, quando
//! o arquivo informa o padrão, a correção **Restaurar padrão**.
//!
//! Quem lê as configs é o chamador (a `warden-configs` acha as chaves; o app junta os arquivos
//! do pack): aqui só se monta o achado, com a linha do arquivo como evidência.

use crate::model::{Evidence, Finding, RuleCode, Severity, SuggestedFix, truncate_excerpt};

/// Código da regra.
pub const RULE: &str = "W_CONFIG_RANGE";

/// Chave da frase quando o arquivo informa o padrão.
pub const TITLE_WITH_DEFAULT: &str = "diagnostico.achados.configForaDaFaixa";

/// Chave da frase quando o arquivo não informa o padrão.
pub const TITLE_WITHOUT_DEFAULT: &str = "diagnostico.achados.configForaDaFaixaSemPadrao";

/// O que o arquivo declara como permitido.
#[derive(Debug, Clone, PartialEq)]
pub enum ConfigLimit {
    /// Faixa, como o arquivo escreve (`1 ~ 512`, `> 5`).
    Range(String),
    /// Valores permitidos.
    Values(Vec<String>),
}

/// Uma chave fora do permitido.
#[derive(Debug, Clone, PartialEq)]
pub struct ConfigRangeIssue {
    /// Caminho da config, relativo ao pack, com `/`.
    pub file: String,
    /// Caminho da chave como o formulário mostra (`general.maxChunks`).
    pub key: String,
    /// A linha da chave no arquivo, aparada.
    pub excerpt: String,
    /// O valor como texto.
    pub value: String,
    /// O limite declarado.
    pub limit: ConfigLimit,
    /// O padrão que o arquivo informa (camada A), se informa.
    pub default: Option<String>,
}

/// Os achados, um por chave, na ordem recebida.
#[must_use]
pub fn findings(issues: &[ConfigRangeIssue]) -> Vec<Finding> {
    issues.iter().map(finding).collect()
}

#[allow(clippy::expect_used)] // Código literal deste módulo, conferido pelos testes.
fn finding(issue: &ConfigRangeIssue) -> Finding {
    let rule = RuleCode::parse(RULE).expect("código fixo de regra");
    let title = if issue.default.is_some() {
        TITLE_WITH_DEFAULT
    } else {
        TITLE_WITHOUT_DEFAULT
    };
    let limit = match &issue.limit {
        ConfigLimit::Range(text) => text.clone(),
        ConfigLimit::Values(values) => values.join(", "),
    };
    let evidence = Evidence::PackFile {
        path: issue.file.clone(),
        excerpt: truncate_excerpt(&issue.excerpt),
    };
    let mut found = Finding::new(rule, Severity::Warning, title, evidence)
        .with_param("config_file", issue.file.clone())
        .with_param("config_key", issue.key.clone())
        .with_param("config_value", truncate_excerpt(&issue.value))
        .with_param("config_limit", truncate_excerpt(&limit));
    if let Some(default) = &issue.default {
        found = found
            .with_param("config_default", truncate_excerpt(default))
            .with_fix(SuggestedFix::RestoreDefault {
                path: issue.file.clone(),
                key: issue.key.clone(),
            });
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chunks(default: Option<&str>) -> ConfigRangeIssue {
        ConfigRangeIssue {
            file: "config/chunks-server.toml".into(),
            key: "general.maxChunks".into(),
            excerpt: "maxChunks = 900".into(),
            value: "900".into(),
            limit: ConfigLimit::Range("1 ~ 512".into()),
            default: default.map(str::to_owned),
        }
    }

    #[test]
    fn ca_t14_10_fora_da_faixa_com_restaurar_padrao() {
        let found = findings(&[chunks(Some("64"))]);
        assert_eq!(found.len(), 1);
        let finding = &found[0];
        assert_eq!(finding.rule().as_str(), RULE);
        assert_eq!(finding.severity(), Severity::Warning);
        assert_eq!(finding.title_key(), TITLE_WITH_DEFAULT);
        assert_eq!(finding.param("config_key"), Some("general.maxChunks"));
        assert_eq!(finding.param("config_value"), Some("900"));
        assert_eq!(finding.param("config_limit"), Some("1 ~ 512"));
        assert_eq!(finding.param("config_default"), Some("64"));
        assert_eq!(
            finding.evidence(),
            [Evidence::PackFile {
                path: "config/chunks-server.toml".into(),
                excerpt: "maxChunks = 900".into(),
            }]
        );
        assert_eq!(
            finding.fixes(),
            [SuggestedFix::RestoreDefault {
                path: "config/chunks-server.toml".into(),
                key: "general.maxChunks".into(),
            }]
        );
    }

    #[test]
    fn sem_padrao_conhecido_nao_oferece_restaurar() {
        let mut issue = chunks(None);
        issue.limit = ConfigLimit::Values(vec!["A".into(), "B".into()]);
        let found = findings(&[issue]);
        assert_eq!(found[0].title_key(), TITLE_WITHOUT_DEFAULT);
        assert_eq!(found[0].param("config_limit"), Some("A, B"));
        assert_eq!(found[0].param("config_default"), None);
        assert!(found[0].fixes().is_empty());
    }
}
