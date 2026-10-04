//! Comandos do domínio `app`.

use serde::Serialize;

use crate::error::AppError;

/// Versão e commit do app em execução.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AppInfo {
    /// Versão do app (a do `Cargo.toml` do workspace).
    pub(crate) version: String,
    /// Commit do código compilado (12 caracteres), quando conhecido.
    pub(crate) commit: Option<String>,
}

impl AppInfo {
    fn current() -> Self {
        let commit = env!("WARDEN_COMMIT");
        Self {
            version: env!("CARGO_PKG_VERSION").to_owned(),
            commit: (!commit.is_empty()).then(|| commit.to_owned()),
        }
    }
}

/// Informa a versão e o commit do app.
#[tauri::command]
#[specta::specta]
// Todo comando devolve `Result<T, AppError>`, mesmo quando hoje não falha (ARCHITECTURE §4.1).
#[allow(clippy::unnecessary_wraps)]
pub(crate) fn app_info() -> Result<AppInfo, AppError> {
    Ok(AppInfo::current())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_info_informa_a_versao_do_workspace() {
        let info = app_info().unwrap();
        assert_eq!(info.version, env!("CARGO_PKG_VERSION"));
        if let Some(commit) = &info.commit {
            assert!(!commit.is_empty());
        }
    }

    #[test]
    fn app_info_em_camel_case() {
        let info = AppInfo {
            version: "1.2.3".into(),
            commit: Some("abc".into()),
        };
        let json = serde_json::to_value(info).unwrap();
        assert_eq!(
            json,
            serde_json::json!({ "version": "1.2.3", "commit": "abc" })
        );
    }
}
