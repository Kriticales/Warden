//! Contrato de um armazenamento de segredos.

use secrecy::{ExposeSecret as _, SecretString};

use crate::error::SecretsError;
use crate::kind::{BackendKind, SecretKind};

/// Maior valor aceito: chaves e tokens reais têm menos de 200 caracteres.
const MAX_SECRET_LEN: usize = 4096;

/// Um lugar onde os segredos ficam. As operações são síncronas e rápidas (cofre do sistema
/// ou arquivo pequeno); quem chama de código assíncrono usa `spawn_blocking`.
pub trait SecretStore: Send + Sync + std::fmt::Debug {
    /// Qual modo este armazenamento representa (o cofre de teste se apresenta como
    /// [`BackendKind::Keyring`], porque o substitui).
    fn backend(&self) -> BackendKind;

    /// Lê o segredo, se existir.
    fn get(&self, kind: SecretKind) -> Result<Option<SecretString>, SecretsError>;

    /// Grava o segredo, substituindo o anterior. `value` já passou por [`normalize_secret`].
    fn set(&self, kind: SecretKind, value: &SecretString) -> Result<(), SecretsError>;

    /// Apaga o segredo. Apagar o que não existe não é erro.
    fn remove(&self, kind: SecretKind) -> Result<(), SecretsError>;

    /// Apaga tudo o que este armazenamento guarda do Warden (fim de uma troca de modo).
    fn purge(&self) -> Result<(), SecretsError> {
        for kind in SecretKind::ALL {
            self.remove(kind)?;
        }
        Ok(())
    }
}

/// Limpa e valida um valor digitado: tira espaços e quebras de linha das pontas (comuns ao
/// colar) e recusa vazio, caracteres de controle, aspas simples (o `.env` usa aspas simples
/// para não expandir `$`, ADR-0025) e valores longos demais.
pub fn normalize_secret(
    kind: SecretKind,
    value: &SecretString,
) -> Result<SecretString, SecretsError> {
    let trimmed = value.expose_secret().trim();
    let reject = |reason| Err(SecretsError::InvalidValue { kind, reason });
    if trimmed.is_empty() {
        return reject("vazio");
    }
    if trimmed.len() > MAX_SECRET_LEN {
        return reject("longo demais");
    }
    if trimmed.chars().any(char::is_control) {
        return reject("tem quebra de linha ou caractere de controle");
    }
    if trimmed.contains('\'') {
        return reject("tem aspas simples");
    }
    Ok(SecretString::from(trimmed.to_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn normalize(text: &str) -> Result<String, SecretsError> {
        normalize_secret(SecretKind::Curseforge, &SecretString::from(text.to_owned()))
            .map(|value| value.expose_secret().to_owned())
    }

    #[test]
    fn tira_espacos_das_pontas() {
        assert_eq!(normalize("  $2a$10$abc \r\n").unwrap(), "$2a$10$abc");
        assert_eq!(normalize("AIza-x_y").unwrap(), "AIza-x_y");
        assert_eq!(
            normalize("com espaço no meio").unwrap(),
            "com espaço no meio"
        );
    }

    #[test]
    fn recusa_valores_ruins() {
        for text in [
            "",
            "   ",
            "a\nb",
            "a\tb",
            "a'b",
            &"x".repeat(MAX_SECRET_LEN + 1),
        ] {
            let error = normalize(text).unwrap_err();
            assert!(
                matches!(error, SecretsError::InvalidValue { .. }),
                "{text:?}"
            );
            assert!(!error.to_string().contains(text.trim()) || text.trim().is_empty());
        }
    }
}
