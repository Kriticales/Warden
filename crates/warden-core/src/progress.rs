//! Progresso das operações longas (ARCHITECTURE §4.3 e §15).
//!
//! As crates de domínio informam o andamento por um [`ProgressSink`] sem saber quem escuta.
//! Na `warden-app`, a implementação encaminha para o canal da operação
//! (`Channel<OperationEvent>`) e para o registro de operações (painel Tarefas, T22).

use serde::{Deserialize, Serialize};

/// Unidade do progresso, para a interface formatar o número.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum ProgressUnit {
    /// Itens (mods, arquivos, etapas).
    Items,
    /// Bytes (downloads, cópias).
    Bytes,
}

/// Quanto já foi feito.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    /// Quantidade feita.
    #[specta(type = specta_typescript::Number)]
    pub current: u64,
    /// Total, quando conhecido.
    #[specta(type = Option<specta_typescript::Number>)]
    pub total: Option<u64>,
    /// Unidade de `current` e `total`.
    pub unit: ProgressUnit,
}

impl Progress {
    /// Progresso em itens.
    #[must_use]
    pub const fn items(current: u64, total: Option<u64>) -> Self {
        Self {
            current,
            total,
            unit: ProgressUnit::Items,
        }
    }

    /// Progresso em bytes.
    #[must_use]
    pub const fn bytes(current: u64, total: Option<u64>) -> Self {
        Self {
            current,
            total,
            unit: ProgressUnit::Bytes,
        }
    }

    /// Fração concluída entre 0 e 1, quando o total é conhecido e maior que zero.
    #[must_use]
    pub fn fraction(&self) -> Option<f64> {
        let total = self.total.filter(|total| *total > 0)?;
        // Precisão de `f64` é suficiente para uma barra de progresso.
        #[allow(clippy::cast_precision_loss)]
        let fraction = self.current.min(total) as f64 / total as f64;
        Some(fraction)
    }
}

/// Identificador estável de uma etapa ("prepareMinecraft", "downloadMods"…), usado pela
/// interface para saber em que etapa está; o texto vem do `label_key`.
pub type StageId = String;

/// Quem recebe o progresso de uma operação. As chamadas são baratas e podem ser frequentes:
/// quem implementa decide se agrega.
pub trait ProgressSink: Send + Sync {
    /// Começou uma etapa. `label_key` é a chave do texto no catálogo da interface
    /// (ex.: `"test.stage.prepareMinecraft"`).
    fn stage(&self, stage: &str, label_key: &str);

    /// Andamento dentro da etapa atual.
    fn progress(&self, progress: Progress);
}

/// Ignora o progresso (testes e chamadas internas sem interface).
#[derive(Debug, Clone, Copy, Default)]
pub struct NoProgress;

impl ProgressSink for NoProgress {
    fn stage(&self, _stage: &str, _label_key: &str) {}

    fn progress(&self, _progress: Progress) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fracao_so_com_total_positivo() {
        assert_eq!(Progress::items(1, Some(4)).fraction(), Some(0.25));
        assert_eq!(Progress::bytes(9, Some(4)).fraction(), Some(1.0));
        assert_eq!(Progress::items(1, None).fraction(), None);
        assert_eq!(Progress::items(0, Some(0)).fraction(), None);
    }

    #[test]
    fn json_em_camel_case() {
        let json = serde_json::to_value(Progress::bytes(10, Some(20))).unwrap();
        assert_eq!(
            json,
            serde_json::json!({ "current": 10, "total": 20, "unit": "bytes" })
        );
    }

    #[test]
    fn sem_progresso_aceita_tudo() {
        let sink: &dyn ProgressSink = &NoProgress;
        sink.stage("x", "y");
        sink.progress(Progress::items(1, None));
    }
}
