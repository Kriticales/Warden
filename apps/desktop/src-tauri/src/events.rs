//! Eventos globais e o canal das operações longas (ARCHITECTURE §4.2 e §4.3).
//!
//! - Eventos globais (`app.emit`) são tipados com `tauri-specta` e registrados em
//!   `commands::builder`. Esta tarefa cria `operation-updated` e `pack-changed`; as tarefas donas
//!   de `game-state`, `server-state` e `ai-conversation-updated` acrescentam os seus aqui;
//!   a UI-01 acrescentou `title-bar-maximize`.
//! - [`OperationEvent`] é o que passa pelo `Channel` de cada comando longo. As variantes
//!   `PerfSample`, `Round` e `ToolCall` da ARCHITECTURE §4.3 entram com as tarefas donas
//!   (L-08, D-10 e D-04): este arquivo é registro acréscimo-apenas para eventos e variantes.

use serde::Serialize;
use tauri_specta::Event;
use warden_core::{OperationId, PackId, Progress};

use crate::error::AppError;
use crate::operations::{OperationKind, OperationSnapshot};

/// `operation-updated`: uma operação mudou de estado, etapa ou progresso (painel Tarefas,
/// T22).
#[derive(Debug, Clone, Serialize, specta::Type, Event)]
#[serde(transparent)]
#[tauri_specta(event_name = "operation-updated")]
pub struct OperationUpdated(pub OperationSnapshot);

/// Parte do pack que mudou, para a interface invalidar só o necessário.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum PackArea {
    /// Mods, resource packs, shaders (lista do pack).
    Inventory,
    /// Configs.
    Configs,
    /// Informações do pack.
    Meta,
    /// Histórico de versões.
    History,
}

/// `pack-changed`: o pack mudou (escrita do Warden ou mudança externa).
#[derive(Debug, Clone, Serialize, specta::Type, Event)]
#[serde(rename_all = "camelCase")]
#[tauri_specta(event_name = "pack-changed")]
pub struct PackChanged {
    /// Pack alterado.
    pub pack_id: PackId,
    /// Partes alteradas.
    pub areas: Vec<PackArea>,
}

/// `title-bar-maximize`: o mouse entrou, saiu ou apertou o botão maximizar da barra de título
/// própria (UI-01). Sobre esse botão quem recebe o mouse é uma janela nativa transparente
/// (`window_chrome`), para o Windows 11 mostrar o menu de encaixe; o evento devolve à interface
/// o estado que ela desenha.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type, Event)]
#[serde(rename_all = "camelCase")]
#[tauri_specta(event_name = "title-bar-maximize")]
pub struct TitleBarMaximize {
    /// Mouse sobre o botão.
    pub hovered: bool,
    /// Botão do mouse apertado sobre o botão.
    pub pressed: bool,
}

/// Origem de uma linha de log no canal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum LogSource {
    /// O jogo.
    Game,
    /// O servidor local (D4).
    Server,
    /// O sidecar do packwiz.
    Packwiz,
}

/// Uma linha do console (jogo, servidor ou packwiz).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct LogLine {
    /// De onde veio.
    pub source: LogSource,
    /// Texto, sem códigos ANSI.
    pub text: String,
}

/// Evento do canal de uma operação longa (`Channel<OperationEvent>`).
#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum OperationEvent {
    /// A operação começou.
    #[serde(rename_all = "camelCase")]
    Started {
        /// Identificador no registro de operações.
        operation_id: OperationId,
        /// Tipo da operação.
        kind: OperationKind,
    },
    /// Começou uma etapa.
    #[serde(rename_all = "camelCase")]
    Stage {
        /// Identificador estável da etapa.
        stage: String,
        /// Chave do texto da etapa no catálogo.
        label_key: String,
    },
    /// Andamento.
    Progress(Progress),
    /// Linhas de console (em lotes).
    Log {
        /// As linhas.
        lines: Vec<LogLine>,
    },
    /// Problema que não interrompe a operação.
    Warning {
        /// O problema.
        error: AppError,
    },
    /// Terminou; o resultado vem no retorno do comando.
    Finished,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eventos_do_canal_em_camel_case_com_tipo() {
        let operation: OperationId = "01J9ZQ0000000000000000000A".parse().unwrap();
        let events = [
            OperationEvent::Started {
                operation_id: operation,
                kind: OperationKind::new("pack.create"),
            },
            OperationEvent::Stage {
                stage: "download".into(),
                label_key: "test.stage.download".into(),
            },
            OperationEvent::Progress(Progress::bytes(5, Some(10))),
            OperationEvent::Log {
                lines: vec![LogLine {
                    source: LogSource::Game,
                    text: "[main/INFO]".into(),
                }],
            },
            OperationEvent::Finished,
        ];
        let json = serde_json::to_value(events).unwrap();
        assert_eq!(
            json,
            serde_json::json!([
                { "type": "started", "operationId": "01J9ZQ0000000000000000000A", "kind": "pack.create" },
                { "type": "stage", "stage": "download", "labelKey": "test.stage.download" },
                { "type": "progress", "current": 5, "total": 10, "unit": "bytes" },
                { "type": "log", "lines": [{ "source": "game", "text": "[main/INFO]" }] },
                { "type": "finished" },
            ])
        );
    }

    #[test]
    fn pack_changed_em_camel_case() {
        let pack: PackId = "01J9ZQ0000000000000000000A".parse().unwrap();
        let json = serde_json::to_value(PackChanged {
            pack_id: pack,
            areas: vec![PackArea::Inventory, PackArea::History],
        })
        .unwrap();
        assert_eq!(
            json,
            serde_json::json!({ "packId": "01J9ZQ0000000000000000000A", "areas": ["inventory", "history"] })
        );
    }
}
