//! `get_mixin_report(mod_id?)` (ARCHITECTURE §9.5; D-10; ROADMAP D-14): o raio-x de mixins para a
//! IA, sobre o mesmo relatório do bloco "O que este mod altera no jogo" (`xray_mod`) e o mesmo
//! aviso `W_MIXIN_OVERLAP` da seção Problemas (sem `mod_id`). Só leitura; evidência
//! `mixin:<mod>` e `mixin:<mod>#<n>` (cada sobreposição).

use std::fmt::Write as _;

use serde_json::Value;
use warden_ai::tools::{ToolItem, ToolOutput};
use warden_mixin::overlap::Overlap;

use super::PackTools;
use crate::commands::xray::{PretestXray, XrayState, xray_mod_impl};

/// Sobreposições por mod enviadas à IA (as de maior risco primeiro).
const MAX_OVERLAPS: usize = 15;

fn label(value: impl serde::Serialize) -> String {
    serde_json::to_value(value)
        .ok()
        .and_then(|value| value.as_str().map(ToOwned::to_owned))
        .unwrap_or_default()
}

fn overlap_line(overlap: &Overlap, names: &std::collections::BTreeMap<String, String>) -> String {
    let name = |path: &str| names.get(path).cloned().unwrap_or_else(|| path.to_owned());
    let mut line = format!(
        "{}: risco {} (regra {}); mods: {}",
        overlap.label(),
        label(overlap.risk),
        label(overlap.rule),
        overlap
            .participants
            .iter()
            .map(|p| format!(
                "{} ({}, {}, {})",
                name(&p.item),
                label(p.kind),
                p.config,
                p.mixin_class
            ))
            .collect::<Vec<_>>()
            .join("; ")
    );
    if overlap.risk != overlap.base {
        let _ = write!(line, "; risco pela regra: {}", label(overlap.base));
    }
    if !overlap.elevators.is_empty() {
        let _ = write!(
            line,
            "; elevadores: {}",
            serde_json::to_string(&overlap.elevators).unwrap_or_default()
        );
    }
    if !overlap.demoters.is_empty() {
        let _ = write!(
            line,
            "; rebaixadores: {}",
            serde_json::to_string(&overlap.demoters).unwrap_or_default()
        );
    }
    line
}

/// Executa a ferramenta.
pub(super) async fn mixin_report(
    tools: &PackTools<'_>,
    args: &Value,
) -> Result<ToolOutput, String> {
    let mod_id = super::text_arg(args, "mod_id");
    if mod_id.is_empty() {
        return pack_overlaps(tools).await;
    }
    let mods = tools.pack_mods().await;
    let item = mods.find_item(&mod_id).ok_or_else(|| {
        format!("O mod '{mod_id}' não está no pack; use um id devolvido por list_mods.")
    })?;
    let view = xray_mod_impl(tools.state, tools.pack_id, &item.path)
        .await
        .map_err(|error| super::failed(&error))?;
    let (XrayState::Ready, Some(report)) = (view.state, view.report) else {
        return Err(format!(
            "O jar de '{}' ainda não está no computador: o pack não foi testado depois que o mod entrou.",
            item.name
        ));
    };
    let mut items = vec![ToolItem::new(
        format!("mixin:{mod_id}"),
        format!(
            "{}: {} alterações ({} classes de mixin, {} desligadas por opções) em {} configs ({}); {} partes do jogo alteradas, {} também alteradas por outros mods; {} sobreposições.",
            item.name,
            report.change_count,
            report.class_count,
            report.disabled_class_count,
            report.configs.len(),
            report.configs.join(", "),
            report.point_count,
            report.shared_point_count,
            report.overlaps.len(),
        ),
    )];
    items.extend(
        report
            .overlaps
            .iter()
            .take(MAX_OVERLAPS)
            .enumerate()
            .map(|(index, overlap)| {
                ToolItem::new(
                    format!("mixin:{mod_id}#{}", index + 1),
                    overlap_line(overlap, &view.names),
                )
            }),
    );
    Ok(ToolOutput {
        items,
        ..ToolOutput::default()
    })
}

/// Sem `mod_id`: os avisos "alteram o mesmo ponto do jogo" do pack inteiro.
async fn pack_overlaps(tools: &PackTools<'_>) -> Result<ToolOutput, String> {
    let mods = tools.pack_mods().await;
    let items: Vec<_> = mods.items().cloned().collect();
    let loader = {
        let _lock = tools.state.locks.read(tools.pack_id).await;
        warden_project::meta::read_meta(&tools.root)
            .map_err(|error| super::failed(&crate::error::AppError::from_domain(&error)))?
            .loader
    };
    let loader = crate::commands::graph::loader_of(loader.as_deref());
    let xray = PretestXray::new(tools.state, tools.pack_id, &items, &tools.root);
    let findings = tauri::async_runtime::spawn_blocking(move || xray.findings(loader))
        .await
        .map_err(|error| error.to_string())?
        .map_err(|error| super::failed(&error))?;
    if findings.is_empty() {
        return Ok(ToolOutput {
            items: vec![ToolItem::new(
                "mixin:pack",
                "Nenhum ponto do jogo alterado por dois mods com risco médio ou alto (só os jars que já estão no computador contam).",
            )],
            ..ToolOutput::default()
        });
    }
    Ok(ToolOutput {
        items: findings
            .iter()
            .take(MAX_OVERLAPS)
            .enumerate()
            .map(|(index, finding)| {
                ToolItem::new(
                    format!("mixin:pack#{}", index + 1),
                    serde_json::to_string(finding).unwrap_or_default(),
                )
            })
            .collect(),
        ..ToolOutput::default()
    })
}
