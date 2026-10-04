//! Árvore de uma config: valores, entradas e os metadados que o próprio arquivo traz.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::format::ConfigFormat;
use crate::path::KeyPath;

/// Valor de uma chave, já sem a sintaxe do formato (aspas, escapes, prefixo de tipo).
///
/// Nos formatos sem tipos (`.properties`, `options.txt`) todo valor simples é texto; inferir
/// número ou liga/desliga a partir do texto é a camada 2 do formulário (C-04).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "camelCase")]
pub enum ConfigValue {
    /// Liga/desliga.
    Bool(bool),
    /// Número inteiro.
    Integer(i64),
    /// Número com casas decimais (pode ser `NaN` ou infinito em TOML, JSON5 e `.cfg`).
    Float(f64),
    /// Texto.
    String(String),
    /// Data e hora do TOML, no texto original (RFC 3339).
    Datetime(String),
    /// `null` do JSON.
    Null,
    /// Lista de valores simples (listas com objetos viram seções, com um item por posição).
    List(Vec<ConfigValue>),
}

impl ConfigValue {
    /// Tipo do valor.
    #[must_use]
    pub fn kind(&self) -> ValueKind {
        match self {
            Self::Bool(_) => ValueKind::Bool,
            Self::Integer(_) => ValueKind::Integer,
            Self::Float(_) => ValueKind::Float,
            Self::String(_) => ValueKind::String,
            Self::Datetime(_) => ValueKind::Datetime,
            Self::Null => ValueKind::Null,
            Self::List(_) => ValueKind::List,
        }
    }

    /// Igualdade semântica: como [`PartialEq`], mas `NaN` é igual a `NaN` (o mesmo valor no
    /// arquivo) e `0.0` é igual a `-0.0` só se tiverem o mesmo sinal.
    #[must_use]
    pub fn same_as(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Float(a), Self::Float(b)) => {
                (a.is_nan() && b.is_nan()) || a.to_bits() == b.to_bits()
            }
            (Self::List(a), Self::List(b)) => {
                a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.same_as(y))
            }
            _ => self == other,
        }
    }

    /// Texto do valor como ele seria lido num formato sem tipos (`true`, `42`, `1.5`).
    /// Listas e `null` não têm forma de texto simples.
    #[must_use]
    pub fn as_plain_text(&self) -> Option<String> {
        match self {
            Self::Bool(value) => Some(value.to_string()),
            Self::Integer(value) => Some(value.to_string()),
            Self::Float(value) => Some(float_text(*value)),
            Self::String(value) | Self::Datetime(value) => Some(value.clone()),
            Self::Null | Self::List(_) => None,
        }
    }
}

/// Número com casas decimais no formato mais curto que volta ao mesmo valor, sempre com `.` ou
/// expoente (`1.0`, `0.25`, `1e300`), aceito por TOML, JSON e `Double.parseDouble` do Java.
/// Valores não finitos saem como no Java (`NaN`, `Infinity`, `-Infinity`).
#[must_use]
pub(crate) fn float_text(value: f64) -> String {
    if value.is_nan() {
        "NaN".to_owned()
    } else if value.is_infinite() {
        if value.is_sign_negative() {
            "-Infinity"
        } else {
            "Infinity"
        }
        .to_owned()
    } else {
        format!("{value:?}")
    }
}

impl fmt::Display for ConfigValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Null => f.write_str("null"),
            Self::List(items) => {
                f.write_str("[")?;
                for (position, item) in items.iter().enumerate() {
                    if position > 0 {
                        f.write_str(", ")?;
                    }
                    write!(f, "{item}")?;
                }
                f.write_str("]")
            }
            Self::String(text) => write!(f, "{text:?}"),
            other => f.write_str(&other.as_plain_text().unwrap_or_default()),
        }
    }
}

/// Tipo de um [`ConfigValue`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ValueKind {
    /// Liga/desliga.
    Bool,
    /// Inteiro.
    Integer,
    /// Número com casas decimais.
    Float,
    /// Texto.
    String,
    /// Data e hora (TOML).
    Datetime,
    /// `null` (JSON).
    Null,
    /// Lista de valores simples.
    List,
}

/// Faixa permitida que o próprio arquivo declara num comentário: `# Range: 1 ~ 64` (Forge e
/// NeoForge), `# Range: > 0`, `[range: 0 ~ 10, default: 5]` (`.cfg` do Forge antigo).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ValueRange {
    /// Mínimo, se o comentário der um número legível.
    pub min: Option<f64>,
    /// Máximo, se o comentário der um número legível.
    pub max: Option<f64>,
    /// Texto da faixa como está no arquivo (`1 ~ 64`).
    pub raw: String,
}

/// Prefixo de tipo de uma propriedade do `.cfg` do Forge antigo (`B:`, `I:`, `D:`, `S:`...).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CfgType {
    /// `S:` texto.
    String,
    /// `I:` inteiro.
    Integer,
    /// `B:` liga/desliga.
    Boolean,
    /// `D:` número com casas decimais.
    Double,
    /// `C:` cor.
    Color,
    /// `M:` id de mod.
    ModId,
    /// Sem prefixo, ou prefixo desconhecido (o Forge trata como texto).
    Untyped,
}

impl CfgType {
    /// Tipo pelo caractere do prefixo, como o `Property.Type.tryParse` do Forge.
    #[must_use]
    pub fn from_prefix(prefix: char) -> Self {
        match prefix {
            'S' => Self::String,
            'I' => Self::Integer,
            'B' => Self::Boolean,
            'D' => Self::Double,
            'C' => Self::Color,
            'M' => Self::ModId,
            _ => Self::Untyped,
        }
    }
}

/// O que uma entrada da árvore é.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum EntryKind {
    /// Seção, tabela, objeto, categoria ou lista que contém objetos: agrupa outras entradas.
    Section,
    /// Chave com valor editável.
    Value,
}

/// Faixa de bytes no arquivo (incluindo o BOM, se houver): `start..end`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TextSpan {
    /// Primeiro byte.
    pub start: usize,
    /// Byte seguinte ao último.
    pub end: usize,
}

impl TextSpan {
    /// Faixa `start..end`.
    #[must_use]
    pub fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    /// Faixa como `Range`.
    #[must_use]
    pub fn range(self) -> std::ops::Range<usize> {
        self.start..self.end
    }
}

/// Uma entrada da árvore: uma seção ou uma chave com valor.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigEntry {
    /// Caminho da chave.
    pub path: KeyPath,
    /// Seção ou valor.
    pub kind: EntryKind,
    /// Valor (só em [`EntryKind::Value`]).
    pub value: Option<ConfigValue>,
    /// Linha da chave no arquivo (começa em 1).
    pub line: usize,
    /// Bytes do valor no arquivo (o trecho que uma edição troca); `None` em seções.
    pub value_span: Option<TextSpan>,
    /// Comentário logo acima da chave, sem os marcadores (`#`, `//`), uma linha por linha.
    pub comment: Option<String>,
    /// Faixa declarada no comentário.
    pub range: Option<ValueRange>,
    /// Valores permitidos declarados no comentário (`# Allowed Values: A, B, C`).
    pub allowed_values: Option<Vec<String>>,
    /// Prefixo de tipo, nos `.cfg` do Forge antigo.
    pub cfg_type: Option<CfgType>,
}

/// Árvore de uma config, com as entradas na ordem do arquivo (pai antes dos filhos).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigTree {
    /// Formato lido.
    pub format: ConfigFormat,
    /// `true` se o arquivo começa com BOM UTF-8 (preservado nas edições).
    pub has_bom: bool,
    /// Fim de linha predominante do arquivo (`"\r\n"` ou `"\n"`), usado quando uma edição
    /// precisa criar linhas (listas do `.cfg`).
    pub line_ending: String,
    /// Entradas.
    pub entries: Vec<ConfigEntry>,
}

impl ConfigTree {
    /// Entrada com esse caminho. Se o arquivo repete a chave (permitido em `.properties` e
    /// `options.txt`), devolve a última, que é a que o jogo usa.
    #[must_use]
    pub fn get(&self, path: &KeyPath) -> Option<&ConfigEntry> {
        self.entries.iter().rev().find(|entry| &entry.path == path)
    }

    /// Só as entradas com valor.
    #[must_use = "o iterador não faz nada sozinho"]
    pub fn values(&self) -> impl DoubleEndedIterator<Item = &ConfigEntry> {
        self.entries
            .iter()
            .filter(|entry| entry.kind == EntryKind::Value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn texto_de_numeros_volta_ao_mesmo_valor() {
        for value in [
            0.0,
            1.0,
            -1.5,
            0.1,
            1e300,
            1e-7,
            123_456.789,
            f64::MIN_POSITIVE,
        ] {
            let text = float_text(value);
            assert!(text.contains('.') || text.contains('e'), "{text}");
            assert_eq!(text.parse::<f64>().unwrap().to_bits(), value.to_bits());
        }
        assert_eq!(float_text(f64::NAN), "NaN");
        assert_eq!(float_text(f64::INFINITY), "Infinity");
        assert_eq!(float_text(f64::NEG_INFINITY), "-Infinity");
    }

    #[test]
    fn igualdade_semantica() {
        assert!(ConfigValue::Float(f64::NAN).same_as(&ConfigValue::Float(f64::NAN)));
        assert!(!ConfigValue::Float(0.0).same_as(&ConfigValue::Float(-0.0)));
        assert!(ConfigValue::Float(1.50).same_as(&ConfigValue::Float(1.5)));
        assert!(!ConfigValue::Integer(1).same_as(&ConfigValue::Float(1.0)));
        let list = ConfigValue::List(vec![ConfigValue::Float(f64::NAN), ConfigValue::Bool(true)]);
        assert!(list.same_as(&list.clone()));
        assert!(!list.same_as(&ConfigValue::List(vec![])));
    }

    #[test]
    fn tipos_e_texto_simples() {
        let samples = [
            (ConfigValue::Bool(true), ValueKind::Bool, Some("true")),
            (ConfigValue::Integer(-3), ValueKind::Integer, Some("-3")),
            (ConfigValue::Float(2.0), ValueKind::Float, Some("2.0")),
            (
                ConfigValue::String("a b".into()),
                ValueKind::String,
                Some("a b"),
            ),
            (
                ConfigValue::Datetime("1979-05-27".into()),
                ValueKind::Datetime,
                Some("1979-05-27"),
            ),
            (ConfigValue::Null, ValueKind::Null, None),
            (ConfigValue::List(vec![]), ValueKind::List, None),
        ];
        for (value, kind, text) in samples {
            assert_eq!(value.kind(), kind);
            assert_eq!(value.as_plain_text().as_deref(), text);
        }
        let list = ConfigValue::List(vec![ConfigValue::String("x".into()), ConfigValue::Null]);
        assert_eq!(list.to_string(), "[\"x\", null]");
        assert_eq!(ConfigValue::Integer(4).to_string(), "4");
    }

    #[test]
    fn prefixos_do_cfg() {
        let pairs = [
            ('S', CfgType::String),
            ('I', CfgType::Integer),
            ('B', CfgType::Boolean),
            ('D', CfgType::Double),
            ('C', CfgType::Color),
            ('M', CfgType::ModId),
            ('X', CfgType::Untyped),
        ];
        for (prefix, expected) in pairs {
            assert_eq!(CfgType::from_prefix(prefix), expected);
        }
    }

    #[test]
    fn busca_devolve_a_ultima_repetida() {
        let entry = |value: i64| ConfigEntry {
            path: KeyPath::from_keys(["k"]),
            kind: EntryKind::Value,
            value: Some(ConfigValue::Integer(value)),
            line: 1,
            value_span: Some(TextSpan::new(0, 1)),
            comment: None,
            range: None,
            allowed_values: None,
            cfg_type: None,
        };
        let tree = ConfigTree {
            format: ConfigFormat::Properties,
            has_bom: false,
            line_ending: "\n".into(),
            entries: vec![entry(1), entry(2)],
        };
        assert_eq!(
            tree.get(&KeyPath::from_keys(["k"]))
                .and_then(|e| e.value.clone()),
            Some(ConfigValue::Integer(2))
        );
        assert_eq!(tree.values().count(), 2);
        assert_eq!(TextSpan::new(2, 5).range(), 2..5);
    }
}
