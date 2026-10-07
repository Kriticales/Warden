//! Deduplicação entre fontes (ADR-0027; ARCHITECTURE §17; SPEC T09 "Duplicado entre fontes").
//!
//! - **Na lista da busca:** dois resultados são o mesmo projeto quando o autor e o slug (ou o
//!   nome) normalizados coincidem ([`same_project`]).
//! - **No pack:** um item que vai entrar por uma fonte e já está no pack por outra é achado
//!   pelo projeto equivalente (o mesmo projeto na outra fonte, quando a fonte informa) ou pelo
//!   hash do arquivo (o SHA-1 que o `.pw.toml` da CurseForge guarda, conferido no Modrinth), e
//!   em último caso pelo nome normalizado ([`PackItems::equivalent`]).
//!
//! Normalizar: minúsculas, sem acentos, sem espaços e sem pontuação ("Farmer's Delight" e
//! "farmers-delight" ficam iguais).

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use unicode_normalization::UnicodeNormalization as _;
use unicode_normalization::char::is_combining_mark;
use warden_packwiz::{Metafile, metafile_stem};

use crate::search::SourceId;

/// Texto normalizado para comparar nomes, slugs e autores.
#[must_use]
pub fn normalize(text: &str) -> String {
    text.nfd()
        .filter(|c| !is_combining_mark(*c))
        .flat_map(char::to_lowercase)
        .filter(|c| c.is_alphanumeric())
        .collect()
}

/// O que identifica um resultado na comparação entre fontes.
#[derive(Debug, Clone, Copy)]
pub struct Identity<'a> {
    /// Autor.
    pub author: &'a str,
    /// Slug.
    pub slug: &'a str,
    /// Nome.
    pub title: &'a str,
}

/// Como um item do pack foi reconhecido como o mesmo projeto vindo de outra fonte.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum DuplicateMatch {
    /// O mesmo projeto, identificado pelo hash do arquivo na outra fonte.
    Project,
    /// O mesmo arquivo (SHA-1).
    Hash,
    /// O mesmo nome ou slug.
    Name,
}

/// Se dois resultados (de fontes diferentes) são o mesmo projeto: mesmo autor e mesmo slug ou
/// mesmo nome, normalizados. Autor vazio nunca casa (sem autor não há como confirmar).
#[must_use]
pub fn same_project(a: Identity<'_>, b: Identity<'_>) -> bool {
    let author = normalize(a.author);
    if author.is_empty() || author != normalize(b.author) {
        return false;
    }
    let same = |x: &str, y: &str| {
        let x = normalize(x);
        !x.is_empty() && x == normalize(y)
    };
    same(a.slug, b.slug) || same(a.title, b.title) || same(a.slug, b.title) || same(a.title, b.slug)
}

/// Um item do pack, com o que serve para achar o mesmo projeto em outra fonte.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackItem {
    /// Caminho do `.pw.toml`, relativo à pasta do pack.
    pub path: String,
    /// Nome (`name` do metafile).
    pub name: String,
    /// Fonte (nenhuma: link direto).
    pub source: Option<SourceId>,
    /// ID do projeto na fonte.
    pub project_id: Option<String>,
    /// ID da versão (Modrinth) ou do arquivo (CurseForge).
    pub version_id: Option<String>,
    /// SHA-1 do arquivo, quando o metafile guarda (CurseForge e links com `sha1`).
    pub sha1: Option<String>,
}

impl PackItem {
    /// Do metafile em `path`.
    #[must_use]
    pub fn from_metafile(path: &str, metafile: &Metafile) -> Self {
        let (source, project_id, version_id) =
            if let Some((project, version)) = metafile.modrinth_ids() {
                (
                    Some(SourceId::Modrinth),
                    Some(project.to_owned()),
                    Some(version.to_owned()),
                )
            } else if let Some((project, file)) = metafile.curseforge_ids() {
                (
                    Some(SourceId::Curseforge),
                    Some(project.to_string()),
                    Some(file.to_string()),
                )
            } else {
                (None, None, None)
            };
        let sha1 = metafile
            .download
            .hash_format
            .eq_ignore_ascii_case("sha1")
            .then(|| metafile.download.hash.to_ascii_lowercase());
        Self {
            path: path.to_owned(),
            name: metafile.name.clone(),
            source,
            project_id,
            version_id,
            sha1,
        }
    }

    /// Chave estável do inventário (`modrinth:<id>`), se o item vem de uma fonte.
    #[must_use]
    pub fn key(&self) -> Option<String> {
        Some(self.source?.key(self.project_id.as_deref()?))
    }
}

/// Os itens do pack que vêm de uma fonte ou de um link, para achar duplicatas.
#[derive(Debug, Clone, Default)]
pub struct PackItems {
    items: Vec<PackItem>,
    /// Projetos equivalentes noutras fontes, achados pelo hash: caminho do item do pack →
    /// chaves nas outras fontes (`mods/jei.pw.toml` → `modrinth:u6dRKJwZ`).
    equivalents: HashMap<String, Vec<String>>,
}

impl PackItems {
    /// Lista dos itens do pack.
    #[must_use]
    pub fn new(items: Vec<PackItem>) -> Self {
        Self {
            items,
            equivalents: HashMap::new(),
        }
    }

    /// Os itens.
    #[must_use]
    pub fn items(&self) -> &[PackItem] {
        &self.items
    }

    /// Registra que o item `path` do pack é o projeto `key` noutra fonte (achado pelo hash do
    /// arquivo).
    pub fn set_equivalent(&mut self, path: &str, key: String) {
        let keys = self.equivalents.entry(path.to_owned()).or_default();
        if !keys.contains(&key) {
            keys.push(key);
        }
    }

    /// O item do pack que é este projeto por outra fonte, achado pelo hash do arquivo
    /// ([`PackItems::set_equivalent`]).
    #[must_use]
    pub fn by_equivalent_key(&self, key: &str) -> Option<&PackItem> {
        self.items.iter().find(|item| {
            self.equivalents
                .get(&item.path)
                .is_some_and(|keys| keys.iter().any(|known| known == key))
        })
    }

    /// O item do pack com este projeto, nesta mesma fonte.
    #[must_use]
    pub fn same_source(&self, source: SourceId, project_id: &str) -> Option<&PackItem> {
        self.items.iter().find(|item| {
            item.source == Some(source) && item.project_id.as_deref() == Some(project_id)
        })
    }

    /// O item do pack que é este projeto **por outra fonte** (ou por link), e como foi achado:
    /// pelo projeto equivalente achado pelo hash, pelo SHA-1 do arquivo, ou pelo nome/slug
    /// normalizado.
    #[must_use]
    pub fn equivalent(
        &self,
        source: SourceId,
        project_id: &str,
        identity: Identity<'_>,
        sha1: Option<&str>,
    ) -> Option<(&PackItem, DuplicateMatch)> {
        let key = source.key(project_id);
        let others = || {
            self.items
                .iter()
                .filter(move |item| item.source != Some(source))
        };
        if let Some(item) = self
            .by_equivalent_key(&key)
            .filter(|item| item.source != Some(source))
        {
            return Some((item, DuplicateMatch::Project));
        }
        if let Some(sha1) = sha1.filter(|sha1| !sha1.is_empty()) {
            let sha1 = sha1.to_ascii_lowercase();
            if let Some(item) = others().find(|item| item.sha1.as_deref() == Some(sha1.as_str())) {
                return Some((item, DuplicateMatch::Hash));
            }
        }
        let names = [normalize(identity.slug), normalize(identity.title)];
        others()
            .find(|item| {
                let stem = metafile_stem(&item.path).map(normalize).unwrap_or_default();
                let name = normalize(&item.name);
                names
                    .iter()
                    .any(|wanted| !wanted.is_empty() && (*wanted == stem || *wanted == name))
            })
            .map(|item| (item, DuplicateMatch::Name))
    }

    /// Chaves dos projetos do pack, inclusive as equivalentes achadas pelo hash.
    #[must_use]
    pub fn keys(&self) -> std::collections::HashSet<String> {
        self.items
            .iter()
            .filter_map(PackItem::key)
            .chain(self.equivalents.values().flatten().cloned())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn identity<'a>(author: &'a str, slug: &'a str, title: &'a str) -> Identity<'a> {
        Identity {
            author,
            slug,
            title,
        }
    }

    #[test]
    fn normaliza_acentos_espacos_e_pontuacao() {
        assert_eq!(normalize("Farmer's Delight"), "farmersdelight");
        assert_eq!(normalize("farmers-delight"), "farmersdelight");
        assert_eq!(normalize("  Ação Rápida! "), "acaorapida");
        assert_eq!(normalize("JEI_(Just Enough Items)"), "jeijustenoughitems");
        assert_eq!(normalize("---"), "");
    }

    #[test]
    fn mesmo_projeto_exige_mesmo_autor_e_nome_ou_slug() {
        let modrinth = identity("mezz", "jei", "Just Enough Items");
        assert!(same_project(
            modrinth,
            identity("Mezz", "jei", "Just Enough Items (JEI)")
        ));
        assert!(same_project(
            modrinth,
            identity("mezz", "just-enough-items", "Just Enough Items")
        ));
        // Mesmo nome, outro autor: são mods diferentes (fork, port).
        assert!(!same_project(
            modrinth,
            identity("outro", "jei", "Just Enough Items")
        ));
        // Sem autor não há como confirmar.
        assert!(!same_project(
            identity("", "jei", "JEI"),
            identity("", "jei", "JEI")
        ));
        // Nome vazio nunca casa.
        assert!(!same_project(identity("a", "", ""), identity("a", "", "")));
    }

    fn item(path: &str, name: &str, source: Option<SourceId>, id: Option<&str>) -> PackItem {
        PackItem {
            path: path.into(),
            name: name.into(),
            source,
            project_id: id.map(Into::into),
            version_id: None,
            sha1: None,
        }
    }

    #[test]
    fn acha_equivalente_por_hash_projeto_ou_nome() {
        let mut cf = item(
            "mods/jei.pw.toml",
            "Just Enough Items (JEI)",
            Some(SourceId::Curseforge),
            Some("238222"),
        );
        cf.sha1 = Some("abc".into());
        let local = item("mods/sodium.pw.toml", "Sodium", None, None);
        let mut pack = PackItems::new(vec![cf, local]);
        let jei = identity("mezz", "jei", "Just Enough Items");
        // Pelo SHA-1 do arquivo.
        assert_eq!(
            pack.equivalent(SourceId::Modrinth, "u6dRKJwZ", jei, Some("ABC"))
                .map(|(item, how)| (item.path.as_str(), how)),
            Some(("mods/jei.pw.toml", DuplicateMatch::Hash))
        );
        // Pelo nome do metafile (stem).
        assert_eq!(
            pack.equivalent(SourceId::Modrinth, "u6dRKJwZ", jei, None)
                .map(|(item, how)| (item.path.as_str(), how)),
            Some(("mods/jei.pw.toml", DuplicateMatch::Name))
        );
        // Pelo projeto equivalente registrado.
        pack.set_equivalent("mods/jei.pw.toml", "modrinth:XYZ".into());
        let other = identity("x", "nada", "Nada");
        assert_eq!(
            pack.equivalent(SourceId::Modrinth, "XYZ", other, None)
                .map(|(item, how)| (item.path.as_str(), how)),
            Some(("mods/jei.pw.toml", DuplicateMatch::Project))
        );
        assert_eq!(
            pack.by_equivalent_key("modrinth:XYZ").unwrap().path,
            "mods/jei.pw.toml"
        );
        assert!(pack.keys().contains("modrinth:XYZ"));
        assert!(pack.keys().contains("curseforge:238222"));
        // A mesma fonte não conta como duplicado entre fontes.
        assert!(
            pack.equivalent(SourceId::Curseforge, "1", jei, Some("abc"))
                .is_none()
        );
        assert!(pack.same_source(SourceId::Curseforge, "238222").is_some());
        // Link direto também conta (o mesmo mod por outro caminho).
        assert_eq!(
            pack.equivalent(
                SourceId::Modrinth,
                "AANobbMI",
                identity("jellysquid3", "sodium", "Sodium"),
                None
            )
            .unwrap()
            .0
            .path,
            "mods/sodium.pw.toml"
        );
    }
}
