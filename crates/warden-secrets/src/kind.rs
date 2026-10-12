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
    /// Token do GitHub (publicar versão), colado pelo usuário ou gravado pelo login do GitHub
    /// (UI-03, ADR-0059).
    Github,
    /// Token de renovação do login do GitHub (UI-03, ADR-0059). Não aparece como linha em
    /// Chaves e contas: só existe com o login e sai junto com ele.
    GithubRefresh,
}

impl SecretKind {
    /// Todos: os três da tela de Configurações, na ordem dela, e o token de renovação do login.
    pub const ALL: [Self; 4] = [
        Self::Curseforge,
        Self::Gemini,
        Self::Github,
        Self::GithubRefresh,
    ];

    /// Conta no cofre do sistema (serviço `dev.kriticales.warden`).
    #[must_use]
    pub const fn account(self) -> &'static str {
        match self {
            Self::Curseforge => "curseforge-api-key",
            Self::Gemini => "gemini-api-key",
            Self::Github => "github-token",
            Self::GithubRefresh => "github-refresh-token",
        }
    }

    /// Nome da variável no `.env` e no ambiente de desenvolvimento.
    #[must_use]
    pub const fn env_var(self) -> &'static str {
        match self {
            Self::Curseforge => "CURSEFORGE_API_KEY",
            Self::Gemini => "GEMINI_API_KEY",
            Self::Github => "GITHUB_TOKEN",
            Self::GithubRefresh => "GITHUB_REFRESH_TOKEN",
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
#[allow(clippy::struct_excessive_bools)] // um campo por segredo: é o contrato com a interface
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SecretsStatus {
    /// Armazenamento em uso.
    pub backend: BackendKind,
    /// Chave da CurseForge configurada.
    pub curseforge: bool,
    /// Chave do Gemini configurada.
    pub gemini: bool,
    /// Token do GitHub configurado.
    pub github: bool,
    /// Token de renovação do login do GitHub guardado.
    pub github_refresh: bool,
}

impl SecretsStatus {
    /// Se `kind` está configurado.
    #[must_use]
    pub const fn is_configured(&self, kind: SecretKind) -> bool {
        match kind {
            SecretKind::Curseforge => self.curseforge,
            SecretKind::Gemini => self.gemini,
            SecretKind::Github => self.github,
            SecretKind::GithubRefresh => self.github_refresh,
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
            serde_json::json!(["curseforge", "gemini", "github", "githubRefresh"])
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
                "github-token",
                "github-refresh-token"
            ]
        );
        let vars: Vec<_> = SecretKind::ALL.iter().map(|kind| kind.env_var()).collect();
        assert_eq!(
            vars,
            [
                "CURSEFORGE_API_KEY",
                "GEMINI_API_KEY",
                "GITHUB_TOKEN",
                "GITHUB_REFRESH_TOKEN"
            ]
        );
    }

    #[test]
    fn estado_por_tipo() {
        let status = SecretsStatus {
            backend: BackendKind::Keyring,
            curseforge: true,
            gemini: false,
            github: true,
            github_refresh: false,
        };
        assert!(status.is_configured(SecretKind::Curseforge));
        assert!(!status.is_configured(SecretKind::GithubRefresh));
        assert!(!status.is_configured(SecretKind::Gemini));
        assert!(status.is_configured(SecretKind::Github));
        assert_eq!(BackendKind::default(), BackendKind::Keyring);
    }
}
