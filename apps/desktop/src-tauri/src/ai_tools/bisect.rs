//! `get_bisect_result` (ARCHITECTURE §9.5; D-12; ROADMAP D-14): a busca do culpado guardada do
//! pack (rodando, pausada ou a última terminada), como a tela do teste a mostra. Só leitura;
//! evidência `bisect:<id>` (o resumo) e `bisect:<id>#R<n>` (cada rodada).

use std::fmt::Write as _;

use warden_ai::tools::{ToolItem, ToolOutput};

use super::PackTools;
use crate::bisect_session::{self, BisectItem};

fn label(value: impl serde::Serialize) -> String {
    match serde_json::to_value(value) {
        Ok(serde_json::Value::String(text)) => text,
        Ok(other) => other.to_string(),
        Err(_) => String::new(),
    }
}

fn names(items: &[BisectItem]) -> String {
    if items.is_empty() {
        return "nenhum".to_owned();
    }
    items
        .iter()
        .map(|item| item.name.as_str())
        .collect::<Vec<_>>()
        .join(", ")
}

/// Executa a ferramenta.
pub(super) fn bisect_result(tools: &PackTools<'_>) -> Result<ToolOutput, String> {
    let view = bisect_session::get(&tools.state.paths, tools.pack_id)
        .map_err(|error| super::failed(&error))?;
    let Some(view) = view else {
        return Ok(ToolOutput {
            items: vec![ToolItem::new(
                "bisect:nenhuma",
                "Nenhuma busca do culpado foi feita neste pack.",
            )],
            ..ToolOutput::default()
        });
    };
    let mut summary = format!(
        "Busca do culpado: situação {}; sintoma {}; {} mods na busca; {} rodadas feitas.",
        label(view.status),
        label(&view.symptom),
        view.searched,
        view.rounds.len(),
    );
    match &view.outcome {
        Some(outcome) => {
            let _ = write!(
                summary,
                " Resultado: {}; itens do resultado: {}.",
                label(outcome),
                names(&view.outcome_items)
            );
        }
        None => {
            let _ = write!(summary, " Suspeitos agora: {}.", names(&view.suspects));
        }
    }
    for inferred in &view.inferred {
        let _ = write!(
            summary,
            " {} usa {} sem declarar a dependência.",
            inferred.from.name, inferred.to.name
        );
    }
    let mut items = vec![ToolItem::new(format!("bisect:{}", view.id), summary)];
    items.extend(view.rounds.iter().map(|round| {
        let mut line = format!(
            "Rodada {} ({}): {} mods ligados; resultado: {}",
            round.number,
            label(&round.kind),
            round.enabled_count,
            label(&round.verdict),
        );
        if let Some(anchor) = &round.anchor {
            let _ = write!(line, "; sinal: {}", label(anchor));
        }
        ToolItem::new(format!("bisect:{}#R{}", view.id, round.number), line)
    }));
    Ok(ToolOutput {
        items,
        ..ToolOutput::default()
    })
}
