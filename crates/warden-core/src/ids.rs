//! Identificadores estáveis: [`PackId`] e [`OperationId`].
//!
//! Os dois são ULIDs (ordenáveis pela data de criação) e atravessam o IPC e os arquivos como
//! texto de 26 caracteres em Crockford base32 (ARCHITECTURE §13: "instâncias, caches e
//! registros usam esse id, não o caminho").

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use ulid::Ulid;

/// Texto que não é um ULID válido.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("identificador inválido: {value:?}")]
pub struct InvalidId {
    /// O texto recebido (truncado em 64 caracteres).
    pub value: String,
}

fn parse_ulid(text: &str) -> Result<Ulid, InvalidId> {
    // `Ulid::from_string` aceita minúsculas; a forma canônica é a maiúscula, que é a que
    // `Display` produz. Aceitar as duas evita erro bobo em arquivo editado à mão.
    Ulid::from_string(text).map_err(|_| InvalidId {
        value: text.chars().take(64).collect(),
    })
}

macro_rules! ulid_id {
    ($(#[$doc:meta])* $name:ident) => {
        $(#[$doc])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, specta::Type)]
        #[specta(transparent)]
        pub struct $name(#[specta(type = String)] Ulid);

        impl $name {
            /// Gera um identificador novo, com a hora atual.
            #[must_use]
            pub fn new() -> Self {
                Self(Ulid::generate())
            }

            /// Cria a partir de um ULID existente (testes e conversões).
            #[must_use]
            pub const fn from_ulid(ulid: Ulid) -> Self {
                Self(ulid)
            }

            /// O ULID por baixo.
            #[must_use]
            pub const fn as_ulid(&self) -> Ulid {
                self.0
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                fmt::Display::fmt(&self.0, f)
            }
        }

        impl FromStr for $name {
            type Err = InvalidId;

            fn from_str(text: &str) -> Result<Self, Self::Err> {
                parse_ulid(text).map(Self)
            }
        }

        impl Serialize for $name {
            fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                serializer.collect_str(&self.0)
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                let text = <std::borrow::Cow<'de, str>>::deserialize(deserializer)?;
                text.parse().map_err(serde::de::Error::custom)
            }
        }
    };
}

ulid_id! {
    /// Identificador de um pack: gerado na criação ou importação e gravado em
    /// `.warden/project.toml`. Todo comando que atua num pack o recebe como primeiro argumento
    /// (ARCHITECTURE §4.1).
    PackId
}

ulid_id! {
    /// Identificador de uma operação longa no registro de operações (ARCHITECTURE §15).
    OperationId
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn texto_de_ida_e_volta() {
        let id = PackId::new();
        let text = id.to_string();
        assert_eq!(text.len(), 26);
        assert_eq!(text.parse::<PackId>().unwrap(), id);
    }

    #[test]
    fn json_e_texto_simples() {
        let id: OperationId = "01J9ZQ0000000000000000000A".parse().unwrap();
        let json = serde_json::to_value(id).unwrap();
        assert_eq!(json, serde_json::json!("01J9ZQ0000000000000000000A"));
        let back: OperationId = serde_json::from_value(json).unwrap();
        assert_eq!(back, id);
    }

    #[test]
    fn aceita_minusculas_e_grava_maiusculas() {
        let id: PackId = "01j9zq0000000000000000000a".parse().unwrap();
        assert_eq!(id.to_string(), "01J9ZQ0000000000000000000A");
    }

    #[test]
    fn recusa_texto_invalido() {
        for text in [
            "",
            "abc",
            "01J9ZQ0000000000000000000",
            "01J9ZQ0000000000000000000AA",
            "../x",
        ] {
            let error = text.parse::<PackId>().unwrap_err();
            assert_eq!(error.value, text);
        }
        assert!(serde_json::from_str::<PackId>("\"nao-e-ulid\"").is_err());
        assert!(serde_json::from_str::<PackId>("42").is_err());
    }

    #[test]
    fn texto_invalido_longo_e_truncado() {
        let long = "x".repeat(500);
        let error = long.parse::<PackId>().unwrap_err();
        assert_eq!(error.value.len(), 64);
    }

    #[test]
    fn ids_novos_sao_ordenados_pela_criacao() {
        let first = PackId::new();
        let second = PackId::from_ulid(Ulid::from_parts(first.as_ulid().timestamp_ms() + 1, 0));
        assert!(first < second);
        assert_ne!(PackId::new(), PackId::new());
        assert_ne!(PackId::default(), PackId::default());
    }
}
