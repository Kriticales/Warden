//! Perfil offline (ARCHITECTURE §7.2): nome do jogador e UUID derivado como o próprio jogo.
//!
//! O UUID é o que o Java calcula em `UUID.nameUUIDFromBytes(("OfflinePlayer:" +
//! nome).getBytes(UTF_8))`: MD5 dos bytes, com a versão 3 e a variante IETF ajustadas. Não há
//! namespace (por isso **não** é `Uuid::new_v3`). Nenhum contorno de autenticação: o perfil só
//! leva o nome e o UUID; o token é `0` e os serviços da Mojang recusam o jogador, como
//! qualquer jogo offline.

use std::fmt::{self, Write as _};

use md5::{Digest, Md5};
use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

/// Prefixo que o jogo usa para o UUID offline.
const OFFLINE_PREFIX: &str = "OfflinePlayer:";

/// UUID fixo do Warden para `--clientId` (o jogo só repassa o valor; um UUID estável evita
/// argumento vazio, que alguns parsers recusam; R2 §4.5).
pub const WARDEN_CLIENT_ID: &str = "8c3c4f58-6a1e-4b52-9d0b-77a6f0e5d1c2";

/// Nome do jogador já validado: 3 a 16 caracteres `A-Z`, `a-z`, `0-9` ou `_`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, specta::Type)]
#[serde(try_from = "String", into = "String")]
pub struct PlayerName(#[specta(type = String)] String);

impl PlayerName {
    /// Valida o nome.
    ///
    /// Erros: [`Error::InvalidPlayerName`] fora do padrão `^[A-Za-z0-9_]{3,16}$`.
    pub fn new(name: &str) -> Result<Self> {
        let valid = (3..=16).contains(&name.len())
            && name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_');
        if valid {
            Ok(Self(name.to_owned()))
        } else {
            Err(Error::InvalidPlayerName {
                name: name.to_owned(),
            })
        }
    }

    /// O nome.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for PlayerName {
    type Error = Error;

    fn try_from(value: String) -> Result<Self> {
        Self::new(&value)
    }
}

impl From<PlayerName> for String {
    fn from(value: PlayerName) -> Self {
        value.0
    }
}

impl fmt::Display for PlayerName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// UUID de 128 bits, mostrado com ou sem hífens.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct OfflineUuid([u8; 16]);

impl OfflineUuid {
    /// O UUID offline do nome, igual ao do Java.
    #[must_use]
    pub fn for_name(name: &PlayerName) -> Self {
        Self::from_text(name.as_str())
    }

    /// O cálculo sem validar o nome (os testes comparam com o Java para nomes quaisquer).
    #[must_use]
    pub(crate) fn from_text(name: &str) -> Self {
        let mut hasher = Md5::new();
        hasher.update(OFFLINE_PREFIX.as_bytes());
        hasher.update(name.as_bytes());
        let mut bytes: [u8; 16] = hasher.finalize().into();
        // Versão 3 (nibble alto do byte 6) e variante IETF (bits 10xx no byte 8), como em
        // `java.util.UUID.nameUUIDFromBytes`.
        bytes[6] = (bytes[6] & 0x0f) | 0x30;
        bytes[8] = (bytes[8] & 0x3f) | 0x80;
        Self(bytes)
    }

    /// Os 32 dígitos hexadecimais sem hífens (o formato do `--uuid` do jogo).
    #[must_use]
    pub fn simple(&self) -> String {
        self.0
            .iter()
            .fold(String::with_capacity(32), |mut text, byte| {
                let _ = write!(text, "{byte:02x}");
                text
            })
    }

    /// O formato com hífens (`8-4-4-4-12`), igual ao `UUID.toString()` do Java.
    #[must_use]
    pub fn hyphenated(&self) -> String {
        let simple = self.simple();
        format!(
            "{}-{}-{}-{}-{}",
            &simple[0..8],
            &simple[8..12],
            &simple[12..16],
            &simple[16..20],
            &simple[20..32]
        )
    }
}

impl fmt::Display for OfflineUuid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.hyphenated())
    }
}

/// O jogador offline do teste.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OfflineProfile {
    /// Nome mostrado no jogo.
    pub name: PlayerName,
    /// UUID derivado do nome.
    pub uuid: OfflineUuid,
}

impl OfflineProfile {
    /// Perfil do nome, com o UUID derivado.
    #[must_use]
    pub fn new(name: PlayerName) -> Self {
        let uuid = OfflineUuid::for_name(&name);
        Self { name, uuid }
    }

    /// Valida o nome e monta o perfil.
    ///
    /// Erros: [`Error::InvalidPlayerName`].
    pub fn from_name(name: &str) -> Result<Self> {
        PlayerName::new(name).map(Self::new)
    }
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    #[test]
    fn uuid_do_spike_s1() {
        // Calculado de forma independente no S1 e visto no log do Forge 1.16.5.
        let profile = OfflineProfile::from_name("WardenTest").unwrap();
        assert_eq!(profile.uuid.simple(), "6c5aa2b1c08439d799a8edc5b372e5f9");
        assert_eq!(
            profile.uuid.hyphenated(),
            "6c5aa2b1-c084-39d7-99a8-edc5b372e5f9"
        );
    }

    #[test]
    fn nomes_validos_e_invalidos() {
        for ok in ["abc", "Steve", "a_b_c", "1234567890123456", "___"] {
            assert!(PlayerName::new(ok).is_ok(), "{ok}");
        }
        for bad in [
            "",
            "ab",
            "12345678901234567",
            "com espaço",
            "jogador-1",
            "Joaõ",
            "a.b",
        ] {
            let error = PlayerName::new(bad).unwrap_err();
            assert!(matches!(error, Error::InvalidPlayerName { .. }), "{bad}");
        }
    }

    #[test]
    fn nome_lido_de_json_e_validado() {
        let name: PlayerName = serde_json::from_str("\"Steve\"").unwrap();
        assert_eq!(name.as_str(), "Steve");
        assert!(serde_json::from_str::<PlayerName>("\"x\"").is_err());
    }

    proptest! {
        #[test]
        fn uuid_sempre_versao_3_e_variante_ietf(name in "[A-Za-z0-9_]{3,16}") {
            let uuid = OfflineUuid::from_text(&name).hyphenated();
            prop_assert_eq!(uuid.len(), 36);
            prop_assert_eq!(&uuid[14..15], "3");
            prop_assert!(matches!(&uuid[19..20], "8" | "9" | "a" | "b"));
        }
    }
}
