//! Modelo dos achados do diagnóstico (ARCHITECTURE §9.1), usado pela análise pós-crash (D-02)
//! e pelas regras pré-teste (D-01).
//!
//! Um [`Finding`] **sempre** tem pelo menos uma [`Evidence`]: o único construtor exige a
//! primeira, os campos são privados e a leitura de JSON recusa a lista vazia. Assim "achado sem
//! evidência" não compila nem chega pela fronteira (teste de propriedade em `tests/`).

use std::collections::BTreeMap;
use std::fmt;

use serde::{Deserialize, Deserializer, Serialize};

/// Tamanho máximo, em caracteres, do trecho guardado numa evidência.
pub const MAX_EXCERPT_CHARS: usize = 500;

/// Código estável de uma regra (`E_MISSING_DEP`, `W_SIDE`, `I_PRERELEASE`…).
///
/// Formato: letra da gravidade padrão (`E`, `W` ou `I`), sublinhado e palavras em maiúsculas
/// separadas por sublinhado. O código é contrato com a interface e com as assinaturas de
/// travamento: renomear é mudança de contrato.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, specta::Type)]
#[serde(transparent)]
pub struct RuleCode(String);

impl RuleCode {
    /// Valida e cria o código. Devolve `None` se o texto não segue o formato.
    #[must_use]
    pub fn parse(code: &str) -> Option<Self> {
        let mut parts = code.split('_');
        let prefix = parts.next()?;
        if !matches!(prefix, "E" | "W" | "I") {
            return None;
        }
        let mut words = 0;
        for word in parts {
            if word.is_empty()
                || !word
                    .bytes()
                    .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit())
            {
                return None;
            }
            words += 1;
        }
        (words > 0).then(|| Self(code.to_owned()))
    }

    /// O código como texto.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for RuleCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for RuleCode {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        Self::parse(&text)
            .ok_or_else(|| serde::de::Error::custom(format!("código de regra inválido: {text:?}")))
    }
}

/// Gravidade do achado.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, specta::Type,
)]
#[serde(rename_all = "camelCase")]
pub enum Severity {
    /// Bloqueia o teste por padrão (pré-teste) ou é a causa provável do travamento.
    Error,
    /// Merece atenção, mas não impede o jogo de abrir.
    Warning,
    /// Informação.
    Info,
}

/// Fonte de metadados de uma API.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, specta::Type,
)]
#[serde(rename_all = "camelCase")]
pub enum Source {
    /// Modrinth.
    Modrinth,
    /// CurseForge.
    CurseForge,
}

/// Item envolvido num achado.
///
/// O pré-teste conhece o metafile do pack; a análise de um log só conhece o que o log diz (id
/// do mod ou nome do jar). Quem mostra o achado liga o id ou o jar ao item do pack.
#[derive(
    Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, specta::Type,
)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ItemRef {
    /// Item do pack pelo caminho do metafile (`mods/sodium.pw.toml`) ou do arquivo.
    Metafile {
        /// Caminho relativo à raiz do pack, com `/`.
        path: String,
    },
    /// Mod pelo id declarado no jar (`modid`, `id` do `fabric.mod.json`).
    ModId {
        /// O id.
        id: String,
    },
    /// Arquivo de mod pelo nome do jar, como aparece no log.
    JarFile {
        /// Nome do arquivo, sem pastas.
        name: String,
    },
}

/// Lado escolhido numa correção de lado.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, specta::Type,
)]
#[serde(rename_all = "camelCase")]
pub enum SideChoice {
    /// Só cliente.
    Client,
    /// Só servidor.
    Server,
    /// Cliente e servidor.
    Both,
}

/// Correção sugerida. Nunca tem efeito sozinha: a interface leva aos fluxos normais, com
/// confirmação (SPEC T14).
#[derive(
    Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, specta::Type,
)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum SuggestedFix {
    /// Adicionar a dependência (busca pelo id nas fontes).
    #[serde(rename_all = "camelCase")]
    AddDependency {
        /// Id do mod que falta.
        mod_id: String,
        /// Faixa de versões pedida, como o log ou o jar escreve.
        version_range: Option<String>,
    },
    /// Remover o item.
    RemoveItem {
        /// O item.
        item: ItemRef,
    },
    /// Atualizar o item (ou trocar de versão).
    UpdateItem {
        /// O item.
        item: ItemRef,
    },
    /// Mudar o lado do item.
    SetSide {
        /// O item.
        item: ItemRef,
        /// O lado novo.
        side: SideChoice,
    },
    /// Trocar o Java do teste.
    ChangeJava {
        /// Versão principal exigida (8, 17, 21…), quando o log diz.
        major: Option<u32>,
    },
    /// Aumentar a memória do teste.
    ChangeMemory,
    /// Tirar um argumento da JVM dos ajustes do teste.
    RemoveJvmArgument {
        /// O argumento, como aparece no log.
        argument: String,
    },
    /// Restaurar uma config a partir da versão do pack.
    RestoreConfig {
        /// Caminho da config, como aparece no log.
        path: String,
    },
    /// Restaurar o padrão de uma chave de config (C-06).
    #[serde(rename_all = "camelCase")]
    RestoreDefault {
        /// Caminho da config, relativo ao pack.
        path: String,
        /// Caminho da chave.
        key: String,
    },
}

/// O que sustenta um achado. Toda evidência aponta o trecho exato.
#[derive(
    Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, specta::Type,
)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Evidence {
    /// Metadados lidos de um jar.
    #[serde(rename_all = "camelCase")]
    JarMetadata {
        /// O item.
        item: ItemRef,
        /// Arquivo dentro do jar (`fabric.mod.json`, `META-INF/mods.toml`…).
        file_in_jar: String,
        /// O trecho.
        excerpt: String,
    },
    /// Campo de uma resposta de API.
    Api {
        /// A fonte.
        source: Source,
        /// Endereço consultado.
        url: String,
        /// Campo que sustenta o achado.
        field: String,
    },
    /// Linha de um log, crash report ou relatório da JVM.
    Log {
        /// Nome do arquivo relativo à sessão (`logs/latest.log`, `output.log`,
        /// `crash-reports/crash-….txt`), nunca um caminho absoluto.
        file: String,
        /// Número da linha no arquivo original (começa em 1).
        line: u32,
        /// A linha, limpa de códigos `§` e ANSI, com no máximo [`MAX_EXCERPT_CHARS`].
        excerpt: String,
    },
    /// Regra da lista curada.
    #[serde(rename_all = "camelCase")]
    Curated {
        /// Id da regra na lista.
        rule_id: String,
        /// Nota da lista.
        note: String,
    },
    /// Trecho de um arquivo do pack.
    PackFile {
        /// Caminho relativo ao pack.
        path: String,
        /// O trecho.
        excerpt: String,
    },
}

impl Evidence {
    /// Evidência de log com o trecho aparado e cortado em [`MAX_EXCERPT_CHARS`].
    #[must_use]
    pub fn log(file: impl Into<String>, line: u32, excerpt: &str) -> Self {
        Self::Log {
            file: file.into(),
            line,
            excerpt: truncate_excerpt(excerpt.trim()),
        }
    }
}

/// Corta o texto em [`MAX_EXCERPT_CHARS`] caracteres, com `…` no fim quando cortou.
#[must_use]
pub fn truncate_excerpt(text: &str) -> String {
    match text.char_indices().nth(MAX_EXCERPT_CHARS) {
        Some((cut, _)) => {
            let mut short = text[..cut].to_owned();
            short.push('…');
            short
        }
        None => text.to_owned(),
    }
}

/// Um achado do diagnóstico, com evidência obrigatória.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct Finding {
    rule: RuleCode,
    severity: Severity,
    title_key: String,
    params: BTreeMap<String, String>,
    items: Vec<ItemRef>,
    evidence: Vec<Evidence>,
    fixes: Vec<SuggestedFix>,
}

impl Finding {
    /// Cria o achado com a primeira evidência (obrigatória).
    #[must_use]
    pub fn new(
        rule: RuleCode,
        severity: Severity,
        title_key: impl Into<String>,
        evidence: Evidence,
    ) -> Self {
        Self {
            rule,
            severity,
            title_key: title_key.into(),
            params: BTreeMap::new(),
            items: Vec::new(),
            evidence: vec![evidence],
            fixes: Vec::new(),
        }
    }

    /// Acrescenta (ou troca) um parâmetro da frase.
    #[must_use]
    pub fn with_param(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.params.insert(name.into(), value.into());
        self
    }

    /// Acrescenta um item envolvido (sem repetir).
    #[must_use]
    pub fn with_item(mut self, item: ItemRef) -> Self {
        self.push_item(item);
        self
    }

    /// Acrescenta mais uma evidência (sem repetir).
    #[must_use]
    pub fn with_evidence(mut self, evidence: Evidence) -> Self {
        self.push_evidence(evidence);
        self
    }

    /// Acrescenta uma correção sugerida (sem repetir).
    #[must_use]
    pub fn with_fix(mut self, fix: SuggestedFix) -> Self {
        if !self.fixes.contains(&fix) {
            self.fixes.push(fix);
        }
        self
    }

    pub(crate) fn push_item(&mut self, item: ItemRef) {
        if !self.items.contains(&item) {
            self.items.push(item);
        }
    }

    pub(crate) fn push_evidence(&mut self, evidence: Evidence) {
        if !self.evidence.contains(&evidence) {
            self.evidence.push(evidence);
        }
    }

    pub(crate) fn truncate_evidence(&mut self, max: usize) {
        self.evidence.truncate(max.max(1));
    }

    /// Código da regra.
    #[must_use]
    pub fn rule(&self) -> &RuleCode {
        &self.rule
    }

    /// Gravidade.
    #[must_use]
    pub fn severity(&self) -> Severity {
        self.severity
    }

    /// Chave i18n do título.
    #[must_use]
    pub fn title_key(&self) -> &str {
        &self.title_key
    }

    /// Parâmetros da frase.
    #[must_use]
    pub fn params(&self) -> &BTreeMap<String, String> {
        &self.params
    }

    /// Valor de um parâmetro.
    #[must_use]
    pub fn param(&self, name: &str) -> Option<&str> {
        self.params.get(name).map(String::as_str)
    }

    /// Itens envolvidos.
    #[must_use]
    pub fn items(&self) -> &[ItemRef] {
        &self.items
    }

    /// Evidências, a principal primeiro. Nunca vazia.
    #[must_use]
    pub fn evidence(&self) -> &[Evidence] {
        &self.evidence
    }

    /// A evidência principal.
    #[must_use]
    pub fn primary_evidence(&self) -> &Evidence {
        // O construtor exige uma evidência, nada remove a última (`truncate_evidence` guarda
        // ao menos uma) e a leitura de JSON recusa lista vazia.
        &self.evidence[0]
    }

    /// Correções sugeridas.
    #[must_use]
    pub fn fixes(&self) -> &[SuggestedFix] {
        &self.fixes
    }
}

/// Forma serializada de [`Finding`], só para a leitura validada.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FindingData {
    rule: RuleCode,
    severity: Severity,
    title_key: String,
    #[serde(default)]
    params: BTreeMap<String, String>,
    #[serde(default)]
    items: Vec<ItemRef>,
    evidence: Vec<Evidence>,
    #[serde(default)]
    fixes: Vec<SuggestedFix>,
}

impl<'de> Deserialize<'de> for Finding {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let data = FindingData::deserialize(deserializer)?;
        if data.evidence.is_empty() {
            return Err(serde::de::Error::custom(
                "achado sem evidência: todo achado precisa de pelo menos uma",
            ));
        }
        Ok(Self {
            rule: data.rule,
            severity: data.severity,
            title_key: data.title_key,
            params: data.params,
            items: data.items,
            evidence: data.evidence,
            fixes: data.fixes,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn code(text: &str) -> RuleCode {
        RuleCode::parse(text).unwrap()
    }

    #[test]
    fn codigo_de_regra_valida_o_formato() {
        for valid in ["E_MISSING_DEP", "W_SIDE", "I_PRERELEASE", "E_JAVA", "W_X1"] {
            assert_eq!(code(valid).as_str(), valid);
            assert_eq!(code(valid).to_string(), valid);
        }
        for invalid in [
            "", "E", "E_", "X_JAVA", "e_java", "E_java", "E__JAVA", "E_JAVA_", "EJAVA",
        ] {
            assert!(RuleCode::parse(invalid).is_none(), "{invalid:?}");
        }
        assert!(serde_json::from_str::<RuleCode>("\"E_JAVA\"").is_ok());
        assert!(serde_json::from_str::<RuleCode>("\"java\"").is_err());
    }

    #[test]
    fn trecho_e_cortado_em_500_caracteres() {
        let long = "á".repeat(MAX_EXCERPT_CHARS + 10);
        let Evidence::Log { excerpt, .. } = Evidence::log("output.log", 1, &long) else {
            unreachable!()
        };
        assert_eq!(excerpt.chars().count(), MAX_EXCERPT_CHARS + 1);
        assert!(excerpt.ends_with('…'));
        assert_eq!(truncate_excerpt("  curto  ".trim()), "curto");
    }

    #[test]
    fn achado_guarda_evidencia_itens_e_correcoes_sem_repetir() {
        let evidence = Evidence::log("logs/latest.log", 10, "  linha  ");
        let finding = Finding::new(
            code("E_MISSING_DEP"),
            Severity::Error,
            "t",
            evidence.clone(),
        )
        .with_param("mod", "a")
        .with_item(ItemRef::ModId { id: "a".into() })
        .with_item(ItemRef::ModId { id: "a".into() })
        .with_evidence(evidence.clone())
        .with_evidence(Evidence::log("output.log", 2, "outra"))
        .with_fix(SuggestedFix::ChangeMemory)
        .with_fix(SuggestedFix::ChangeMemory);
        assert_eq!(finding.rule().as_str(), "E_MISSING_DEP");
        assert_eq!(finding.severity(), Severity::Error);
        assert_eq!(finding.title_key(), "t");
        assert_eq!(finding.param("mod"), Some("a"));
        assert_eq!(finding.param("x"), None);
        assert_eq!(finding.params().len(), 1);
        assert_eq!(finding.items().len(), 1);
        assert_eq!(finding.evidence().len(), 2);
        assert_eq!(finding.primary_evidence(), &evidence);
        assert_eq!(finding.fixes(), &[SuggestedFix::ChangeMemory]);

        let mut short = finding.clone();
        short.truncate_evidence(0);
        assert_eq!(short.evidence().len(), 1);
    }

    #[test]
    fn json_de_ida_e_volta_e_recusa_achado_sem_evidencia() {
        let finding = Finding::new(
            code("E_JAVA"),
            Severity::Error,
            "diagnostics.finding.java",
            Evidence::log("output.log", 3, "UnsupportedClassVersionError"),
        )
        .with_fix(SuggestedFix::ChangeJava { major: Some(17) })
        .with_fix(SuggestedFix::AddDependency {
            mod_id: "fabric-api".into(),
            version_range: None,
        });
        let json = serde_json::to_value(&finding).unwrap();
        assert_eq!(json["rule"], "E_JAVA");
        assert_eq!(json["titleKey"], "diagnostics.finding.java");
        assert_eq!(json["evidence"][0]["kind"], "log");
        assert_eq!(json["fixes"][1]["modId"], "fabric-api");
        let back: Finding = serde_json::from_value(json.clone()).unwrap();
        assert_eq!(back, finding);

        let mut empty = json;
        empty["evidence"] = serde_json::json!([]);
        let error = serde_json::from_value::<Finding>(empty).unwrap_err();
        assert!(error.to_string().contains("sem evidência"));
    }
}
