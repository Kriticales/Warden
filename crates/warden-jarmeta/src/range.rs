//! Faixa de versões com o dialeto certo de cada descritor (R3 §6.4 item 6).
//!
//! - Fabric e Quilt: predicados do Fabric; uma lista é um OU entre predicados
//!   ([`crate::version::fabric`]).
//! - Forge, NeoForge e `@Mod.dependencies`: faixas Maven ([`crate::version::maven`]).
//! - `mcmod.info`: `mcversion` é uma versão exata.

use serde::{Deserialize, Serialize};

use crate::version::fabric::{FabricVersion, VersionPredicate};
use crate::version::maven::{MavenFlavor, MavenRange};

/// Faixa de versões declarada por um mod, guardada como texto com o dialeto.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "dialect", rename_all = "camelCase")]
pub enum VersionRange {
    /// Qualquer versão (campo de faixa ausente).
    Any,
    /// Predicados do Fabric; a versão precisa satisfazer **pelo menos um** (lista vazia não
    /// aceita nada, como no fabric-loader).
    Fabric {
        /// Predicados, como `>=0.14.21` ou `1.20.x`.
        predicates: Vec<String>,
    },
    /// Faixa Maven, como `[1.20.1,1.21)`.
    Maven {
        /// Especificação da faixa.
        spec: String,
    },
    /// Versão exata (`mcversion` do `mcmod.info`).
    Exact {
        /// A versão.
        version: String,
    },
}

/// Dialeto de uma faixa, para mensagens de erro.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RangeDialect {
    /// Predicados do Fabric.
    Fabric,
    /// Faixas Maven.
    Maven,
}

/// Faixa que o loader recusaria.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("faixa de versões inválida ({dialect:?}) '{spec}': {message}")]
pub struct RangeError {
    /// Dialeto da faixa.
    pub dialect: RangeDialect,
    /// Texto da faixa.
    pub spec: String,
    /// Motivo.
    pub message: String,
}

impl VersionRange {
    /// Faixa Fabric com um predicado só.
    #[must_use]
    pub fn fabric(predicate: impl Into<String>) -> Self {
        Self::Fabric {
            predicates: vec![predicate.into()],
        }
    }

    /// Faixa Maven.
    #[must_use]
    pub fn maven(spec: impl Into<String>) -> Self {
        Self::Maven { spec: spec.into() }
    }

    /// Confere se a faixa é aceita pelo loader, sem avaliar nenhuma versão (faixas Maven com a
    /// variante padrão, [`MavenFlavor::V3_8_5`]).
    pub fn validate(&self) -> Result<(), RangeError> {
        self.validate_with(MavenFlavor::default())
    }

    /// Como [`VersionRange::validate`], com as regras Maven de uma variante.
    pub fn validate_with(&self, flavor: MavenFlavor) -> Result<(), RangeError> {
        match self {
            Self::Any | Self::Exact { .. } => Ok(()),
            Self::Fabric { predicates } => {
                for p in predicates {
                    VersionPredicate::parse(p).map_err(|e| RangeError {
                        dialect: RangeDialect::Fabric,
                        spec: p.clone(),
                        message: e.to_string(),
                    })?;
                }
                Ok(())
            }
            Self::Maven { spec } => MavenRange::parse_with(spec, flavor)
                .map(|_| ())
                .map_err(|e| RangeError {
                    dialect: RangeDialect::Maven,
                    spec: spec.clone(),
                    message: e.to_string(),
                }),
        }
    }

    /// `true` se `version` está na faixa, avaliada no dialeto da faixa (faixas Maven com a
    /// variante padrão, [`MavenFlavor::V3_8_5`]; para o loader do pack, use
    /// [`VersionRange::matches_with`] com [`MavenFlavor::for_forge`] ou
    /// [`MavenFlavor::for_neoforge`]).
    ///
    /// Erro quando a faixa é inválida ou, no Fabric, quando a versão é vazia.
    pub fn matches(&self, version: &str) -> Result<bool, RangeError> {
        self.matches_with(version, MavenFlavor::default())
    }

    /// Como [`VersionRange::matches`], com as regras Maven de uma variante.
    pub fn matches_with(&self, version: &str, flavor: MavenFlavor) -> Result<bool, RangeError> {
        match self {
            Self::Any => Ok(true),
            Self::Exact { version: expected } => Ok(expected == version),
            Self::Fabric { predicates } => {
                let fabric_error = |spec: &str, message: String| RangeError {
                    dialect: RangeDialect::Fabric,
                    spec: spec.to_owned(),
                    message,
                };
                let candidate = FabricVersion::parse(version, false)
                    .map_err(|e| fabric_error(version, e.to_string()))?;
                let mut any = false;
                for p in predicates {
                    let predicate =
                        VersionPredicate::parse(p).map_err(|e| fabric_error(p, e.to_string()))?;
                    any |= predicate.test(&candidate);
                }
                Ok(any)
            }
            Self::Maven { spec } => {
                let range = MavenRange::parse_with(spec, flavor).map_err(|e| RangeError {
                    dialect: RangeDialect::Maven,
                    spec: spec.clone(),
                    message: e.to_string(),
                })?;
                Ok(range.contains_text(version))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn avalia_cada_dialeto() {
        assert_eq!(VersionRange::Any.matches("qualquer"), Ok(true));
        assert_eq!(
            VersionRange::Exact {
                version: "1.7.10".into()
            }
            .matches("1.7.10"),
            Ok(true)
        );
        let or = VersionRange::Fabric {
            predicates: vec!["1.20.x".into(), ">=1.21 <1.21.2".into()],
        };
        assert_eq!(or.matches("1.20.4"), Ok(true));
        assert_eq!(or.matches("1.21.1"), Ok(true));
        assert_eq!(or.matches("1.21.2"), Ok(false));
        let empty = VersionRange::Fabric { predicates: vec![] };
        assert_eq!(empty.matches("1.0"), Ok(false));
        assert_eq!(
            VersionRange::maven("[1.20.1,1.21)").matches("1.20.1"),
            Ok(true)
        );
        assert_eq!(
            VersionRange::maven("[1.20.1,1.21)").matches("1.21"),
            Ok(false)
        );
    }

    #[test]
    fn faixas_invalidas_viram_erro() {
        let bad = VersionRange::fabric(">=1.x");
        assert!(bad.validate().is_err());
        assert!(bad.matches("1.0").is_err());
        assert!(VersionRange::fabric("1.0").matches("").is_err());
        let bad = VersionRange::maven("[1.0");
        let err = bad.validate().expect_err("inválida");
        assert_eq!(err.dialect, RangeDialect::Maven);
        assert!(err.to_string().contains("[1.0"));
        assert!(VersionRange::maven("[1.0,2.0)").validate().is_ok());
        assert!(VersionRange::Any.validate().is_ok());
        let open = VersionRange::maven("(,)");
        assert!(open.validate().is_err());
        assert!(open.validate_with(MavenFlavor::V3_8_8).is_ok());
        assert_eq!(open.matches_with("1.0", MavenFlavor::V3_8_8), Ok(true));
        assert!(open.matches("1.0").is_err());
    }

    #[test]
    fn serializa_com_o_dialeto() {
        let json = serde_json::to_string(&VersionRange::maven("[47,)")).expect("json");
        assert_eq!(json, r#"{"dialect":"maven","spec":"[47,)"}"#);
        let back: VersionRange = serde_json::from_str(&json).expect("volta");
        assert_eq!(back, VersionRange::maven("[47,)"));
        assert_eq!(
            serde_json::to_string(&VersionRange::Any).expect("json"),
            r#"{"dialect":"any"}"#
        );
    }
}
