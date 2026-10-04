//! Nomes dos metafiles (ARCHITECTURE §6.2).
//!
//! O nome base do `.pw.toml` precisa ser único no pack inteiro: o `FindMod` do packwiz procura
//! pelo nome base em qualquer pasta e devolve um qualquer quando há repetidos. O padrão é
//! `<slug>.pw.toml`; se o nome já existe, `<slug>-<fonte>.pw.toml`, depois `-2`, `-3`…
//! A comparação ignora maiúsculas, porque no Windows `Sodium.pw.toml` e `sodium.pw.toml` são o
//! mesmo arquivo.

use std::collections::HashSet;
use std::sync::OnceLock;

use regex::Regex;

use crate::index::METAFILE_SUFFIX;

/// De onde vem o arquivo descrito pelo metafile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MetafileSource {
    /// Modrinth.
    Modrinth,
    /// CurseForge.
    CurseForge,
    /// Link direto.
    Url,
    /// GitHub Releases.
    GitHub,
}

impl MetafileSource {
    /// Sufixo usado quando o nome base já existe.
    #[must_use]
    pub fn suffix(self) -> &'static str {
        match self {
            Self::Modrinth => "modrinth",
            Self::CurseForge => "curseforge",
            Self::Url => "url",
            Self::GitHub => "github",
        }
    }
}

/// Nome base de um metafile (`mods/sodium.pw.toml` → `sodium`); `None` se não for metafile.
#[must_use]
pub fn metafile_stem(path: &str) -> Option<&str> {
    let name = path.rsplit('/').next().unwrap_or(path);
    name.strip_suffix(METAFILE_SUFFIX)
}

/// Nome base livre para um metafile novo, dado o slug do projeto, a fonte e os nomes base
/// que já existem no pack.
#[must_use]
pub fn unique_metafile_stem<'a>(
    slug: &str,
    source: MetafileSource,
    existing: impl IntoIterator<Item = &'a str>,
) -> String {
    let taken: HashSet<String> = existing.into_iter().map(str::to_lowercase).collect();
    let free = |candidate: &str| !taken.contains(&candidate.to_lowercase());
    if free(slug) {
        return slug.to_owned();
    }
    let with_source = format!("{slug}-{}", source.suffix());
    if free(&with_source) {
        return with_source;
    }
    let mut number = 2_u64;
    loop {
        let candidate = format!("{with_source}-{number}");
        if free(&candidate) {
            return candidate;
        }
        number += 1;
    }
}

/// `SlugifyName` do packwiz (`core/mod.go`): nome do metafile para um link direto.
///
/// Minúsculas; tira o que está entre parênteses e o que vem depois de ` - `; troca tudo que
/// não é `a-z` ou `0-9` por `-`; junta hífens repetidos e tira os das pontas.
#[must_use]
pub fn slugify_name(name: &str) -> String {
    static RULES: OnceLock<[Regex; 5]> = OnceLock::new();
    let [brackets, suffix, chars, dashes, edges] = RULES.get_or_init(|| {
        [
            compile(r"\(.*\)"),
            compile(r" - .+"),
            compile(r"[^a-z0-9]"),
            compile(r"-+"),
            compile(r"^-|-$"),
        ]
    });
    let lower = name.to_lowercase();
    let text = brackets.replace_all(&lower, "");
    let text = suffix.replace_all(&text, "");
    let text = chars.replace_all(&text, "-");
    let text = dashes.replace_all(&text, "-");
    edges.replace_all(&text, "").into_owned()
}

#[allow(clippy::expect_used)] // só expressões literais deste módulo, conferidas pelos testes
fn compile(pattern: &str) -> Regex {
    Regex::new(pattern).expect("expressão fixa válida")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slug_como_no_packwiz() {
        let cases = [
            ("Text Placeholder", "text-placeholder"),
            ("Jade 🔍", "jade"),
            ("Mod (Fabric) - Edição Especial", "mod"),
            ("  Já_vi  --Isso!! ", "j-vi-isso"),
            ("ABC123", "abc123"),
            ("---", ""),
        ];
        for (name, slug) in cases {
            assert_eq!(slugify_name(name), slug, "{name}");
        }
    }

    #[test]
    fn nomes_unicos() {
        let existing = [
            "sodium",
            "Lithium",
            "lithium-modrinth",
            "lithium-modrinth-2",
        ];
        assert_eq!(
            unique_metafile_stem("jei", MetafileSource::CurseForge, existing),
            "jei"
        );
        assert_eq!(
            unique_metafile_stem("SODIUM", MetafileSource::CurseForge, existing),
            "SODIUM-curseforge"
        );
        assert_eq!(
            unique_metafile_stem("lithium", MetafileSource::Modrinth, existing),
            "lithium-modrinth-3"
        );
        assert_eq!(MetafileSource::Url.suffix(), "url");
        assert_eq!(MetafileSource::GitHub.suffix(), "github");
    }

    #[test]
    fn nome_base() {
        assert_eq!(metafile_stem("mods/sodium.pw.toml"), Some("sodium"));
        assert_eq!(metafile_stem("x.pw.toml"), Some("x"));
        assert_eq!(metafile_stem("mods/a.jar"), None);
    }
}
