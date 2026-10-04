//! `cargo xtask check-docs`: confere os links internos dos arquivos Markdown versionados
//! (QUALITY §2.3).
//!
//! Para cada link relativo, o arquivo ou a pasta precisa existir; se o link tem âncora
//! (`#seção`) e aponta para Markdown, o título precisa existir no destino, com a mesma regra
//! de âncoras do GitHub. Links externos (`https:`, `mailto:`…) não são conferidos.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path, PathBuf};

use anyhow::{Context as _, Result, bail};
use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};

use crate::util::{Cmd, workspace_root};

/// Um link de um arquivo Markdown.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Link {
    /// Linha (a partir de 1) onde o link aparece.
    pub line: usize,
    /// Destino como está no texto.
    pub target: String,
}

/// O que interessa de um arquivo Markdown: links e âncoras dos títulos.
#[derive(Debug, Default)]
pub struct Document {
    /// Links e imagens.
    pub links: Vec<Link>,
    /// Âncoras dos títulos, como o GitHub as gera.
    pub anchors: BTreeSet<String>,
}

/// Lê links e títulos de um texto Markdown.
pub fn parse(text: &str) -> Document {
    let mut document = Document::default();
    let mut heading: Option<String> = None;
    let mut seen: BTreeMap<String, usize> = BTreeMap::new();
    let options = Options::ENABLE_TABLES
        | Options::ENABLE_FOOTNOTES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_HEADING_ATTRIBUTES;
    for (event, range) in Parser::new_ext(text, options).into_offset_iter() {
        match event {
            Event::Start(Tag::Heading { .. }) => {
                heading = Some(String::new());
            }
            Event::End(TagEnd::Heading(_)) => {
                if let Some(title) = heading.take() {
                    let base = slug(&title);
                    let count = seen.entry(base.clone()).or_insert(0);
                    let anchor = if *count == 0 {
                        base.clone()
                    } else {
                        format!("{base}-{count}")
                    };
                    *count += 1;
                    document.anchors.insert(anchor);
                }
            }
            Event::Text(part) | Event::Code(part) => {
                if let Some(title) = heading.as_mut() {
                    title.push_str(&part);
                }
            }
            Event::Start(Tag::Link { dest_url, .. } | Tag::Image { dest_url, .. }) => {
                let line = text[..range.start].matches('\n').count() + 1;
                document.links.push(Link {
                    line,
                    target: dest_url.into_string(),
                });
            }
            _ => {}
        }
    }
    document
}

/// Âncora de um título, como no GitHub: minúsculas, sem pontuação (letras com acento ficam),
/// espaços viram `-`.
pub fn slug(title: &str) -> String {
    title
        .trim()
        .to_lowercase()
        .chars()
        .filter_map(|c| match c {
            ' ' => Some('-'),
            '-' | '_' => Some(c),
            c if c.is_alphanumeric() => Some(c),
            _ => None,
        })
        .collect()
}

/// Decodifica `%XX` (UTF-8). Sequência inválida fica como está.
fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%'
            && let Some(hex) = text.get(index + 1..index + 3)
            && let Ok(byte) = u8::from_str_radix(hex, 16)
        {
            out.push(byte);
            index += 3;
            continue;
        }
        out.push(bytes[index]);
        index += 1;
    }
    String::from_utf8(out).unwrap_or_else(|_| text.to_owned())
}

/// Se o destino é externo (tem esquema, como `https:` ou `mailto:`).
fn is_external(target: &str) -> bool {
    let scheme_end = target.find(':');
    let path_start = target.find(['/', '#', '?']);
    match (scheme_end, path_start) {
        (Some(colon), Some(slash)) => colon < slash,
        (Some(_), None) => true,
        _ => false,
    }
}

/// Junta `base` e `relative` resolvendo `.` e `..` sem tocar no disco.
fn normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other),
        }
    }
    out
}

/// Um link quebrado: arquivo, linha, destino e motivo.
#[derive(Debug, PartialEq, Eq)]
pub struct Broken {
    /// Arquivo de origem, relativo à raiz.
    pub file: String,
    /// Linha do link.
    pub line: usize,
    /// Destino como está no texto.
    pub target: String,
    /// Motivo, em português.
    pub reason: String,
}

/// Confere os links de `files` (caminhos relativos a `root`, com `/`).
pub fn check_files(root: &Path, files: &[String]) -> Result<Vec<Broken>> {
    let mut documents = BTreeMap::new();
    for file in files {
        let text = std::fs::read_to_string(root.join(file))
            .with_context(|| format!("falha ao ler {file}"))?;
        documents.insert(PathBuf::from(file), parse(&text));
    }
    let mut extra = BTreeMap::new();
    let mut broken = Vec::new();
    for (file, document) in &documents {
        for link in &document.links {
            if let Some(reason) = check_link(root, file, &link.target, &documents, &mut extra) {
                broken.push(Broken {
                    file: file.to_string_lossy().replace('\\', "/"),
                    line: link.line,
                    target: link.target.clone(),
                    reason,
                });
            }
        }
    }
    Ok(broken)
}

fn check_link(
    root: &Path,
    file: &Path,
    target: &str,
    documents: &BTreeMap<PathBuf, Document>,
    extra: &mut BTreeMap<PathBuf, Option<Document>>,
) -> Option<String> {
    if target.is_empty() || is_external(target) {
        return None;
    }
    let (path_part, anchor) = match target.split_once('#') {
        Some((path, anchor)) => (path, Some(percent_decode(anchor))),
        None => (target, None),
    };
    let path_part = path_part.split('?').next().unwrap_or_default();
    let destination = if path_part.is_empty() {
        file.to_path_buf()
    } else {
        let decoded = percent_decode(path_part);
        let joined = match decoded.strip_prefix('/') {
            Some(from_root) => PathBuf::from(from_root),
            None => file.parent().unwrap_or(Path::new("")).join(&decoded),
        };
        normalize(&joined)
    };
    if !root.join(&destination).exists() {
        return Some("arquivo ou pasta não existe".to_owned());
    }
    let anchor = anchor?;
    if destination.extension().is_none_or(|ext| ext != "md") || anchor.is_empty() {
        return None;
    }
    let anchors = if let Some(document) = documents.get(&destination) {
        &document.anchors
    } else {
        let parsed = extra.entry(destination.clone()).or_insert_with(|| {
            std::fs::read_to_string(root.join(&destination))
                .ok()
                .map(|text| parse(&text))
        });
        match parsed {
            Some(document) => &document.anchors,
            None => return Some("não foi possível ler o destino".to_owned()),
        }
    };
    (!anchors.contains(&anchor.to_lowercase())).then(|| format!("âncora #{anchor} não existe"))
}

/// Arquivos Markdown versionados, relativos à raiz.
fn versioned_markdown(root: &Path) -> Result<Vec<String>> {
    let output = Cmd::new("git")
        .cwd(root)
        .args(["ls-files", "-z", "--", "*.md"])
        .read()?;
    Ok(output
        .split('\0')
        .filter(|name| !name.is_empty())
        .map(str::to_owned)
        .collect())
}

/// `cargo xtask check-docs`.
pub fn run() -> Result<()> {
    let root = workspace_root();
    let files = versioned_markdown(&root)?;
    let broken = check_files(&root, &files)?;
    if broken.is_empty() {
        println!(
            "check-docs: {} arquivos Markdown, nenhum link interno quebrado.",
            files.len()
        );
        return Ok(());
    }
    let lines: Vec<String> = broken
        .iter()
        .map(|item| {
            format!(
                "  {}:{}: {} ({})",
                item.file, item.line, item.target, item.reason
            )
        })
        .collect();
    bail!(
        "check-docs: {} link(s) interno(s) quebrado(s):\n{}",
        broken.len(),
        lines.join("\n")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slug_igual_ao_do_github() {
        assert_eq!(
            slug("2. Ações necessárias do dono"),
            "2-ações-necessárias-do-dono"
        );
        assert_eq!(slug("Fase 0 — Fundação"), "fase-0--fundação");
        assert_eq!(
            slug("F0-01 — Esqueleto do monorepo"),
            "f0-01--esqueleto-do-monorepo"
        );
        assert_eq!(
            slug("Fase 7 — Warden 1.1 \"Profissional\""),
            "fase-7--warden-11-profissional"
        );
        assert_eq!(slug("`warden-core` e o_resto"), "warden-core-e-o_resto");
    }

    #[test]
    fn parse_encontra_links_titulos_e_repetidos() {
        let text = "# Título\n\nVeja [a](outro.md#seção) e ![i](img.png).\n\n## Notas\n\n## Notas\n\n```\n[não](link.md)\n```\n";
        let document = parse(text);
        assert_eq!(
            document.links,
            [
                Link {
                    line: 3,
                    target: "outro.md#seção".into()
                },
                Link {
                    line: 3,
                    target: "img.png".into()
                },
            ]
        );
        let anchors: Vec<_> = document.anchors.iter().map(String::as_str).collect();
        assert_eq!(anchors, ["notas", "notas-1", "título"]);
    }

    #[test]
    fn externos_nao_sao_conferidos() {
        assert!(is_external("https://github.com/x"));
        assert!(is_external("mailto:a@b.c"));
        assert!(!is_external("docs/SPEC.md#10-decisões"));
        assert!(!is_external("#x:y"));
    }

    #[test]
    fn check_files_acha_arquivo_e_ancora_quebrados() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("docs/sub")).unwrap();
        std::fs::write(dir.path().join("README.md"), "# Leia\n").unwrap();
        std::fs::write(
            dir.path().join("docs/a.md"),
            "# Ações\n\n[ok](../README.md#leia) [ok2](#ações) [ok3](sub/) [cod](%C3%A7.md)\n\
             [x](falta.md)\n[y](../README.md#nada)\n[z](https://exemplo.com/falta.md)\n",
        )
        .unwrap();
        std::fs::write(dir.path().join("docs/ç.md"), "").unwrap();
        let files = vec!["README.md".to_owned(), "docs/a.md".to_owned()];
        let broken = check_files(dir.path(), &files).unwrap();
        let summary: Vec<_> = broken
            .iter()
            .map(|b| (b.file.as_str(), b.line, b.target.as_str()))
            .collect();
        assert_eq!(
            summary,
            [
                ("docs/a.md", 4, "falta.md"),
                ("docs/a.md", 5, "../README.md#nada")
            ]
        );
    }
}
