//! O resultado do último **Testar** de cada chave (Configurações → Chaves e contas; ADR-0069 e
//! ADR-0070): `key-checks.json` na pasta de configuração.
//!
//! Guarda só se a chave funcionou ou foi recusada e quando; nunca o valor nem um pedaço dele.
//! Uma chave recusada no último teste faz o Sensor Sculk pular o provedor dela (ADR-0070) e
//! aparece como "Recusada" até ser testada de novo ou substituída. Gravar ou remover a chave
//! apaga o resultado (a chave nova ainda não foi testada).
//!
//! Leitura tolerante: arquivo ausente ou ilegível vale como "nenhum teste"; gravação atômica.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard, PoisonError};

use serde::{Deserialize, Serialize};
use warden_core::{CoreError, atomic_write};
use warden_secrets::SecretKind;

use crate::error::{AppError, ErrorCode};

/// O que o último teste disse da chave.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) enum KeyCheckOutcome {
    /// O serviço aceitou a chave.
    Working,
    /// O serviço recusou a chave (inválida, apagada, vencida).
    Refused,
}

/// O último teste de uma chave.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct KeyCheck {
    /// O resultado.
    pub(crate) outcome: KeyCheckOutcome,
    /// Quando (RFC 3339, UTC).
    pub(crate) at: String,
}

/// Se um erro do **Testar** quer dizer que o serviço recusou a chave (e não, por exemplo, que a
/// internet caiu): só esses erros marcam a chave como recusada.
#[must_use]
pub(crate) fn is_refusal(code: ErrorCode) -> bool {
    matches!(
        code,
        ErrorCode::Curseforge(warden_curseforge::CurseforgeErrorCode::KeyInvalid)
            | ErrorCode::Ai(warden_ai::AiErrorCode::KeyInvalid)
            | ErrorCode::Versioning(warden_versioning::VersioningErrorCode::GithubTokenRejected)
    )
}

/// `key-checks.json` em memória.
#[derive(Debug)]
pub(crate) struct KeyChecks {
    path: PathBuf,
    checks: Mutex<BTreeMap<SecretKind, KeyCheck>>,
}

impl KeyChecks {
    /// Abre o arquivo (ausente ou ilegível: nenhum teste).
    #[must_use]
    pub(crate) fn open(path: PathBuf) -> Self {
        let checks = std::fs::read(&path)
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default();
        Self {
            path,
            checks: Mutex::new(checks),
        }
    }

    /// O último teste da chave, se houver.
    pub(crate) fn get(&self, kind: SecretKind) -> Option<KeyCheck> {
        self.lock().get(&kind).cloned()
    }

    /// Se a chave foi recusada no último teste.
    pub(crate) fn refused(&self, kind: SecretKind) -> bool {
        self.get(kind)
            .is_some_and(|check| check.outcome == KeyCheckOutcome::Refused)
    }

    /// Grava o resultado de um teste feito agora.
    pub(crate) fn record(
        &self,
        kind: SecretKind,
        outcome: KeyCheckOutcome,
    ) -> Result<KeyCheck, AppError> {
        let check = KeyCheck {
            outcome,
            at: crate::server_session::now_rfc3339(),
        };
        self.change(|checks| {
            checks.insert(kind, check.clone());
        })?;
        Ok(check)
    }

    /// Esquece o teste da chave (gravada de novo ou removida).
    pub(crate) fn clear(&self, kind: SecretKind) -> Result<(), AppError> {
        if self.lock().contains_key(&kind) {
            self.change(|checks| {
                checks.remove(&kind);
            })?;
        }
        Ok(())
    }

    fn change(
        &self,
        edit: impl FnOnce(&mut BTreeMap<SecretKind, KeyCheck>),
    ) -> Result<(), AppError> {
        let mut checks = self.lock();
        let mut next = checks.clone();
        edit(&mut next);
        let mut text = serde_json::to_string_pretty(&next)
            .map_err(|error| AppError::internal(format!("key-checks.json: {error}")))?;
        text.push('\n');
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|error| CoreError::io("criar a pasta", parent, error))?;
        }
        atomic_write(&self.path, text.as_bytes())?;
        *checks = next;
        Ok(())
    }

    fn lock(&self) -> MutexGuard<'_, BTreeMap<SecretKind, KeyCheck>> {
        self.checks.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grava_le_e_esquece() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config").join("key-checks.json");
        let checks = KeyChecks::open(path.clone());
        assert_eq!(checks.get(SecretKind::Github), None);
        checks
            .record(SecretKind::Github, KeyCheckOutcome::Refused)
            .unwrap();
        checks
            .record(SecretKind::Gemini, KeyCheckOutcome::Working)
            .unwrap();
        assert!(checks.refused(SecretKind::Github));
        assert!(!checks.refused(SecretKind::Gemini));

        let reopened = KeyChecks::open(path.clone());
        assert!(reopened.refused(SecretKind::Github));
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("\"github\""), "{text}");
        assert!(text.contains("\"refused\""), "{text}");

        reopened.clear(SecretKind::Github).unwrap();
        assert!(!KeyChecks::open(path).refused(SecretKind::Github));
    }

    #[test]
    fn arquivo_ilegivel_vale_como_nenhum_teste() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("key-checks.json");
        std::fs::write(&path, b"{ quebrado").unwrap();
        let checks = KeyChecks::open(path);
        assert_eq!(checks.get(SecretKind::Openai), None);
        checks.clear(SecretKind::Openai).unwrap();
    }

    #[test]
    fn so_a_recusa_do_servico_conta_como_recusada() {
        assert!(is_refusal(ErrorCode::Ai(
            warden_ai::AiErrorCode::KeyInvalid
        )));
        assert!(is_refusal(ErrorCode::Curseforge(
            warden_curseforge::CurseforgeErrorCode::KeyInvalid
        )));
        assert!(is_refusal(ErrorCode::Versioning(
            warden_versioning::VersioningErrorCode::GithubTokenRejected
        )));
        assert!(!is_refusal(ErrorCode::Ai(warden_ai::AiErrorCode::Timeout)));
        assert!(!is_refusal(ErrorCode::Core(
            warden_core::CoreErrorCode::NetworkUnavailable
        )));
    }
}
