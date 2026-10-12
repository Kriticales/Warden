//! `CHANGELOG.md` do pack (ARCHITECTURE §11): a entrada de cada versão, em Markdown, e a
//! inclusão dela no topo do arquivo.
//!
//! As funções só montam texto: quem grava o arquivo no pack é a `PackTransaction` (V-02 pela
//! P1-07; QUALITY §1 item 5). A mesma entrada serve de mensagem da tag anotada da versão.
//!
//! Formato de uma entrada:
//!
//! ```markdown
//! ## 1.3.0 — 2026-10-01
//!
//! Notas livres do usuário.
//!
//! ### Atenção
//! - Mods removidos podem apagar blocos ou itens de mundos existentes. Faça backup antes de atualizar.
//!
//! ### Minecraft e loader
//! - Minecraft 1.21.1 → 1.21.4
//!
//! ### Mods adicionados
//! - Sodium 0.6.0 (Modrinth)
//!
//! ### Mods removidos
//! - OptiFine HD U I6 (arquivo local)
//!
//! ### Mods atualizados
//! - Lithium 0.14.1 → 0.14.3
//!
//! ### Resource packs e shaders
//! - Adicionado: Faithful 32x 1.21 (Modrinth)
//!
//! ### Configs alteradas
//! - config/sodium-options.json
//! ```

use std::fmt::Write as _;

use crate::changes::{ChangeSet, ChangeSource, ItemCategory, ItemChange, ItemChangeKind};

/// Título de um `CHANGELOG.md` novo.
pub const CHANGELOG_TITLE: &str = "# Changelog";

/// Aviso da seção "Atenção" quando a versão remove mods que podem ter conteúdo nos mundos.
pub const WORLD_REMOVAL_WARNING: &str = "Mods removidos podem apagar blocos ou itens de mundos existentes. Faça backup antes de atualizar.";

/// Escapa o texto de um nome para não virar formatação no Markdown (nomes de mod podem ter
/// `*`, `_`, `[`, `<`…). As notas do usuário não passam por aqui: são Markdown dele.
#[must_use]
pub fn escape_markdown(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for c in text.chars() {
        if matches!(
            c,
            '\\' | '`' | '*' | '_' | '[' | ']' | '<' | '>' | '#' | '|' | '~'
        ) {
            escaped.push('\\');
        }
        if c == '\n' || c == '\r' {
            escaped.push(' ');
        } else {
            escaped.push(c);
        }
    }
    escaped
}

fn source_label(source: ChangeSource) -> &'static str {
    match source {
        ChangeSource::Modrinth => "Modrinth",
        ChangeSource::CurseForge => "CurseForge",
        ChangeSource::Url => "link direto",
        ChangeSource::Local => "arquivo local",
        ChangeSource::Unknown => "origem desconhecida",
    }
}

fn loader_label(key: &str) -> String {
    match key {
        "fabric" => "Fabric".to_owned(),
        "forge" => "Forge".to_owned(),
        "neoforge" => "NeoForge".to_owned(),
        "quilt" => "Quilt".to_owned(),
        "liteloader" => "LiteLoader".to_owned(),
        other => escape_markdown(other),
    }
}

/// "Sodium 0.6.0 (Modrinth)": nome, versão (se diferente do nome) e origem.
fn with_version(item: &ItemChange, label: Option<&str>) -> String {
    let name = escape_markdown(&item.name);
    let source = source_label(item.source);
    match label {
        Some(label) if label != item.name => {
            format!("{name} {} ({source})", escape_markdown(label))
        }
        _ => format!("{name} ({source})"),
    }
}

fn added_line(item: &ItemChange) -> String {
    with_version(
        item,
        item.new.as_ref().map(|version| version.label.as_str()),
    )
}

fn removed_line(item: &ItemChange) -> String {
    with_version(
        item,
        item.old.as_ref().map(|version| version.label.as_str()),
    )
}

fn updated_line(item: &ItemChange) -> String {
    let old = item
        .old
        .as_ref()
        .map_or("?", |version| version.label.as_str());
    let new = item
        .new
        .as_ref()
        .map_or("?", |version| version.label.as_str());
    format!(
        "{} {} → {}",
        escape_markdown(&item.name),
        escape_markdown(old),
        escape_markdown(new)
    )
}

fn adjusted_line(item: &ItemChange) -> String {
    format!("{} (ajustes do item)", escape_markdown(&item.name))
}

fn push_section(out: &mut String, title: &str, lines: &[String]) {
    if lines.is_empty() {
        return;
    }
    let _ = writeln!(out, "### {title}");
    for line in lines {
        let _ = writeln!(out, "- {line}");
    }
    out.push('\n');
}

/// Corpo da entrada (sem o cabeçalho `## versão — data`): notas, "Atenção" e as seções do
/// changelog. Seções vazias não aparecem.
#[must_use]
pub fn render_body(notes: &str, changes: &ChangeSet) -> String {
    let mut out = String::new();
    let notes = notes.trim();
    if !notes.is_empty() {
        out.push_str(&notes.replace("\r\n", "\n"));
        out.push_str("\n\n");
    }
    if changes.removed_world_mods().next().is_some() {
        push_section(&mut out, "Atenção", &[WORLD_REMOVAL_WARNING.to_owned()]);
    }

    let mut platform = Vec::new();
    if let Some(minecraft) = &changes.minecraft {
        platform.push(format!(
            "Minecraft {} → {}",
            escape_markdown(minecraft.old.as_deref().unwrap_or("?")),
            escape_markdown(minecraft.new.as_deref().unwrap_or("?"))
        ));
    }
    for loader in &changes.loaders {
        let name = loader_label(&loader.loader);
        platform.push(match (&loader.change.old, &loader.change.new) {
            (Some(old), Some(new)) => {
                format!("{name} {} → {}", escape_markdown(old), escape_markdown(new))
            }
            (None, Some(new)) => format!("{name} {} adicionado", escape_markdown(new)),
            (Some(old), None) => format!("{name} {} removido", escape_markdown(old)),
            (None, None) => continue,
        });
    }
    push_section(&mut out, "Minecraft e loader", &platform);

    let mods = |kind: ItemChangeKind| {
        changes
            .items_of(kind)
            .filter(|item| item.category == ItemCategory::Mod)
    };
    push_section(
        &mut out,
        "Mods adicionados",
        &mods(ItemChangeKind::Added)
            .map(added_line)
            .collect::<Vec<_>>(),
    );
    push_section(
        &mut out,
        "Mods removidos",
        &mods(ItemChangeKind::Removed)
            .map(removed_line)
            .collect::<Vec<_>>(),
    );
    push_section(
        &mut out,
        "Mods atualizados",
        &mods(ItemChangeKind::Updated)
            .map(updated_line)
            .collect::<Vec<_>>(),
    );

    let mut packs = Vec::new();
    for kind in [
        ItemChangeKind::Added,
        ItemChangeKind::Removed,
        ItemChangeKind::Updated,
        ItemChangeKind::Adjusted,
    ] {
        for item in changes
            .items_of(kind)
            .filter(|item| item.category != ItemCategory::Mod)
        {
            packs.push(match kind {
                ItemChangeKind::Added => format!("Adicionado: {}", added_line(item)),
                ItemChangeKind::Removed => format!("Removido: {}", removed_line(item)),
                ItemChangeKind::Updated => format!("Atualizado: {}", updated_line(item)),
                ItemChangeKind::Adjusted => format!("Ajustado: {}", adjusted_line(item)),
            });
        }
    }
    push_section(&mut out, "Resource packs e shaders", &packs);
    push_section(
        &mut out,
        "Mods ajustados",
        &mods(ItemChangeKind::Adjusted)
            .map(adjusted_line)
            .collect::<Vec<_>>(),
    );
    push_section(
        &mut out,
        "Configs alteradas",
        &changes
            .configs
            .iter()
            .map(|path| escape_markdown(path))
            .collect::<Vec<_>>(),
    );
    let trimmed = out.trim_end().len();
    out.truncate(trimmed);
    out.push('\n');
    if out == "\n" {
        out.clear();
    }
    out
}

/// Entrada completa de uma versão: `## <versão> — <data>` e o corpo ([`render_body`]).
#[must_use]
pub fn render_entry(version: &str, date: &str, notes: &str, changes: &ChangeSet) -> String {
    let body = render_body(notes, changes);
    if body.is_empty() {
        format!("## {version} — {date}\n")
    } else {
        format!("## {version} — {date}\n\n{body}")
    }
}

/// Põe `entry` no topo das versões de um `CHANGELOG.md` (`None` = arquivo novo).
///
/// A entrada entra antes do primeiro título de versão (`## `); o que vem antes dele (título,
/// apresentação) fica no topo. Sem nenhum título de versão, a entrada vai para o fim do texto
/// que já existe. O fim de linha do arquivo (`\n` ou `\r\n`) é mantido.
#[must_use]
pub fn prepend_entry(existing: Option<&str>, entry: &str) -> String {
    let entry = entry.trim_end_matches(['\n', '\r']);
    let Some(existing) = existing.filter(|text| !text.trim().is_empty()) else {
        return format!("{CHANGELOG_TITLE}\n\n{entry}\n");
    };
    let crlf = existing.contains("\r\n");
    let normalized = existing.replace("\r\n", "\n");
    let mut offset = 0;
    let mut split = None;
    let mut in_fence = false;
    for line in normalized.split_inclusive('\n') {
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            in_fence = !in_fence;
        }
        if !in_fence && line.starts_with("## ") {
            split = Some(offset);
            break;
        }
        offset += line.len();
    }
    let result = if let Some(at) = split {
        let (head, tail) = normalized.split_at(at);
        format!("{head}{entry}\n\n{tail}")
    } else {
        let head = normalized.trim_end_matches('\n');
        format!("{head}\n\n{entry}\n")
    };
    if crlf {
        result.replace('\n', "\r\n")
    } else {
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapa_formatacao_dos_nomes() {
        assert_eq!(
            escape_markdown("Mod *Top* [beta]"),
            "Mod \\*Top\\* \\[beta\\]"
        );
        assert_eq!(escape_markdown("<script>"), "\\<script\\>");
        assert_eq!(escape_markdown("a\nb"), "a b");
        assert_eq!(escape_markdown("Ação é_ok"), "Ação é\\_ok");
    }

    #[test]
    fn arquivo_novo_ganha_titulo() {
        assert_eq!(
            prepend_entry(None, "## 1.0.0 — 2026-10-01\n\n- x\n\n"),
            "# Changelog\n\n## 1.0.0 — 2026-10-01\n\n- x\n"
        );
        assert_eq!(
            prepend_entry(Some("  \n"), "## 1.0.0 — 2026-10-01\n"),
            "# Changelog\n\n## 1.0.0 — 2026-10-01\n"
        );
    }

    #[test]
    fn entra_antes_da_primeira_versao_e_preserva_o_topo() {
        let existing = "# Changelog do pack\n\nApresentação.\n\n## 1.0.0 — 2026-09-01\n\n- velho\n";
        assert_eq!(
            prepend_entry(Some(existing), "## 1.1.0 — 2026-10-01\n\n- novo\n"),
            "# Changelog do pack\n\nApresentação.\n\n## 1.1.0 — 2026-10-01\n\n- novo\n\n## 1.0.0 — 2026-09-01\n\n- velho\n"
        );
    }

    #[test]
    fn titulo_dentro_de_bloco_de_codigo_nao_conta() {
        let existing = "# C\n\n```\n## não é versão\n```\n";
        assert_eq!(
            prepend_entry(Some(existing), "## 1.0.0 — d\n"),
            "# C\n\n```\n## não é versão\n```\n\n## 1.0.0 — d\n"
        );
    }

    #[test]
    fn mantem_crlf() {
        let existing = "# Changelog\r\n\r\n## 1.0.0 — d\r\n";
        assert_eq!(
            prepend_entry(Some(existing), "## 1.1.0 — e\n\n- a\n"),
            "# Changelog\r\n\r\n## 1.1.0 — e\r\n\r\n- a\r\n\r\n## 1.0.0 — d\r\n"
        );
    }

    #[test]
    fn entrada_sem_mudancas_so_tem_cabecalho() {
        assert_eq!(
            render_entry("1.0.1", "2026-10-01", "  ", &ChangeSet::default()),
            "## 1.0.1 — 2026-10-01\n"
        );
        assert_eq!(
            render_entry(
                "1.0.1",
                "2026-10-01",
                "Notas\r\nmais",
                &ChangeSet::default()
            ),
            "## 1.0.1 — 2026-10-01\n\nNotas\nmais\n"
        );
    }
}
