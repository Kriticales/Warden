//! Quais segredos existem e onde podem ficar.

use serde::{Deserialize, Serialize};

/// Um segredo que o Warden guarda. Não existe "ler segredo" pela interface.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, specta::Type,
)]
#[serde(rename_all = "camelCase")]
pub enum SecretKind {
    /// Chave da API da CurseForge.
    Curseforge,
    /// Chave da API do Gemini.
    Gemini,
    /// Chave da API da `OpenAI` (ADR-0060).
    Openai,
    /// Chave da API da Anthropic, o provedor Claude (ADR-0060).
    Anthropic,
    /// Token do GitHub (publicar versão).
    Github,
}

impl SecretKind {
    /// Todos, na ordem da tela de Configurações.
    pub const ALL: [Self; 5] = [
        Self::Curseforge,
        Self::Gemini,
        Self::Openai,
        Self::Anthropic,
        Self::Github,
    ];

    /// Conta no cofre do sistema (serviço `dev.kriticales.warden`).
    #[must_use]
    pub const fn account(self) -> &'static str {
        match self {
            Self::Curseforge => "curseforge-api-key",
            Self::Gemini => "gemini-api-key",
            Self::Openai => "openai-api-key",
            Self::Anthropic => "anthropic-api-key",
            Self::Github => "github-token",
        }
    }

    /// Nome da variável no `.env` e no ambiente de desenvolvimento.
    #[must_use]
    pub const fn env_var(self) -> &'static str {
        match self {
            Self::Curseforge => "CURSEFORGE_API_KEY",
            Self::Gemini => "GEMINI_API_KEY",
            Self::Openai => "OPENAI_API_KEY",
            Self::Anthropic => "ANTHROPIC_API_KEY",
            Self::Github => "GITHUB_TOKEN",
        }
    }
}

/// Onde as chaves ficam, gravado em `settings.json` como `secretsBackend`.
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize, specta::Type,
)]
#[serde(rename_all = "camelCase")]
pub enum BackendKind {
    /// Cofre do sistema (padrão).
    #[default]
    Keyring,
    /// Arquivo `.env` na pasta de configuração do Warden.
    Envfile,
}

/// O que a interface sabe das chaves: onde ficam e quais estão configuradas.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
#[allow(clippy::struct_excessive_bools)] // um campo por chave da tela: é o contrato com a interface
pub struct SecretsStatus {
    /// Armazenamento em uso.
    pub backend: BackendKind,
    /// Chave da CurseForge configurada.
    pub curseforge: bool,
    /// Chave do Gemini configurada.
    pub gemini: bool,
    /// Chave da `OpenAI` configurada.
    pub openai: bool,
    /// Chave da Anthropic configurada.
    pub anthropic: bool,
    /// Token do GitHub configurado.
    pub github: bool,
}

impl SecretsStatus {
    /// Se `kind` está configurado.
    #[must_use]
    pub const fn is_configured(&self, kind: SecretKind) -> bool {
        match kind {
            SecretKind::Curseforge => self.curseforge,
            SecretKind::Gemini => self.gemini,
            SecretKind::Openai => self.openai,
            SecretKind::Anthropic => self.anthropic,
            SecretKind::Github => self.github,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nomes_estaveis() {
        assert_eq!(
            serde_json::to_value(SecretKind::ALL).unwrap(),
            serde_json::json!(["curseforge", "gemini", "openai", "anthropic", "github"])
        );
        assert_eq!(
            serde_json::to_value([BackendKind::Keyring, BackendKind::Envfile]).unwrap(),
            serde_json::json!(["keyring", "envfile"])
        );
        let accounts: Vec<_> = SecretKind::ALL.iter().map(|kind| kind.account()).collect();
        assert_eq!(
            accounts,
            [
                "curseforge-api-key",
                "gemini-api-key",
                "openai-api-key",
                "anthropic-api-key",
                "github-token"
            ]
        );
        let vars: Vec<_> = SecretKind::ALL.iter().map(|kind| kind.env_var()).collect();
        assert_eq!(
            vars,
            [
                "CURSEFORGE_API_KEY",
                "GEMINI_API_KEY",
                "OPENAI_API_KEY",
                "ANTHROPIC_API_KEY",
                "GITHUB_TOKEN"
            ]
        );
    }

    #[test]
    fn estado_por_tipo() {
        let status = SecretsStatus {
            backend: BackendKind::Keyring,
            curseforge: true,
            gemini: false,
            openai: true,
            anthropic: false,
            github: true,
        };
        assert!(status.is_configured(SecretKind::Curseforge));
        assert!(!status.is_configured(SecretKind::Gemini));
        assert!(status.is_configured(SecretKind::Openai));
        assert!(!status.is_configured(SecretKind::Anthropic));
        assert!(status.is_configured(SecretKind::Github));
        assert_eq!(BackendKind::default(), BackendKind::Keyring);
    }
}
