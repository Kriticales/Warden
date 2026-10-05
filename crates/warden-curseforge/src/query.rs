//! Parâmetros da busca (`GET /v1/mods/search`) e da lista de arquivos
//! (`GET /v1/mods/{id}/files`), com os limites da API conferidos antes de pedir (R3 §3.2):
//! `pageSize` até 50, `index + pageSize` até 10 000, `gameVersions` até 4, `modLoaderTypes`
//! até 5 e `categoryIds` até 10. Fora disso a API responde 400; o Warden recusa antes, com
//! [`Error::InvalidQuery`].

use crate::error::{Error, Result};
use crate::model::{MINECRAFT_GAME_ID, ModLoaderType, ProjectClass};

/// Maior página da API.
pub const MAX_PAGE_SIZE: u32 = 50;
/// A busca só alcança os primeiros 10 mil resultados (`index + pageSize`).
pub const MAX_SEARCH_WINDOW: u32 = 10_000;
/// Versões do Minecraft por busca.
pub const MAX_GAME_VERSIONS: usize = 4;
/// Loaders por busca.
pub const MAX_MOD_LOADER_TYPES: usize = 5;
/// Categorias por busca.
pub const MAX_CATEGORY_IDS: usize = 10;

/// Ordem dos resultados (`sortField`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SortField {
    /// Destaques.
    Featured,
    /// Popularidade (padrão do Warden: na conferência de 05/10/2026, buscar "jei" sem ordem
    /// devolveu mods recentes antes do JEI; por popularidade, o JEI vem primeiro).
    Popularity,
    /// Última atualização.
    LastUpdated,
    /// Nome.
    Name,
    /// Autor.
    Author,
    /// Total de downloads.
    TotalDownloads,
    /// Categoria.
    Category,
    /// Versão do jogo.
    GameVersion,
    /// Data de publicação.
    ReleasedDate,
    /// Avaliação.
    Rating,
}

impl SortField {
    /// O número na API.
    #[must_use]
    pub const fn id(self) -> u32 {
        match self {
            Self::Featured => 1,
            Self::Popularity => 2,
            Self::LastUpdated => 3,
            Self::Name => 4,
            Self::Author => 5,
            Self::TotalDownloads => 6,
            Self::Category => 7,
            Self::GameVersion => 8,
            Self::ReleasedDate => 11,
            Self::Rating => 12,
        }
    }
}

/// Sentido da ordem.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SortOrder {
    /// Crescente.
    Ascending,
    /// Decrescente.
    Descending,
}

impl SortOrder {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Ascending => "asc",
            Self::Descending => "desc",
        }
    }
}

/// Uma busca na CurseForge.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SearchQuery {
    /// Texto buscado.
    pub text: String,
    /// Classe (mods, resource packs, shaders…); `None` busca em todas.
    pub class: Option<ProjectClass>,
    /// Versões do Minecraft (até 4). Uma vira `gameVersion`; várias, `gameVersions`.
    pub game_versions: Vec<String>,
    /// Loaders (até 5). Um vira `modLoaderType`; vários, `modLoaderTypes`.
    pub loaders: Vec<ModLoaderType>,
    /// Categorias (até 10).
    pub category_ids: Vec<u32>,
    /// Slug exato.
    pub slug: Option<String>,
    /// Ordem.
    pub sort_field: SortField,
    /// Sentido.
    pub sort_order: SortOrder,
    /// Posição do primeiro resultado.
    pub index: u32,
    /// Resultados por página (1 a 50).
    pub page_size: u32,
}

impl SearchQuery {
    /// Busca por `text` em todas as classes, por popularidade, 20 por página (a página da
    /// busca combinada, ADR-0027).
    #[must_use]
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            class: None,
            game_versions: Vec::new(),
            loaders: Vec::new(),
            category_ids: Vec::new(),
            slug: None,
            sort_field: SortField::Popularity,
            sort_order: SortOrder::Descending,
            index: 0,
            page_size: 20,
        }
    }

    /// Só a classe dada.
    #[must_use]
    pub fn class(mut self, class: ProjectClass) -> Self {
        self.class = Some(class);
        self
    }

    /// Só arquivos para esta versão do Minecraft (acumula).
    #[must_use]
    pub fn game_version(mut self, version: impl Into<String>) -> Self {
        self.game_versions.push(version.into());
        self
    }

    /// Só arquivos para este loader (acumula).
    #[must_use]
    pub fn loader(mut self, loader: ModLoaderType) -> Self {
        self.loaders.push(loader);
        self
    }

    /// Só esta categoria (acumula).
    #[must_use]
    pub fn category(mut self, id: u32) -> Self {
        self.category_ids.push(id);
        self
    }

    /// Só o projeto com este slug.
    #[must_use]
    pub fn slug(mut self, slug: impl Into<String>) -> Self {
        self.slug = Some(slug.into());
        self
    }

    /// Ordem dos resultados.
    #[must_use]
    pub fn sort(mut self, field: SortField, order: SortOrder) -> Self {
        self.sort_field = field;
        self.sort_order = order;
        self
    }

    /// Página: posição do primeiro resultado e tamanho.
    #[must_use]
    pub fn page(mut self, index: u32, page_size: u32) -> Self {
        self.index = index;
        self.page_size = page_size;
        self
    }

    /// Confere os limites da API.
    pub fn validate(&self) -> Result<()> {
        check_page(self.index, self.page_size)?;
        if self.index.saturating_add(self.page_size) > MAX_SEARCH_WINDOW {
            return Err(Error::InvalidQuery(format!(
                "a CurseForge só mostra os primeiros {MAX_SEARCH_WINDOW} resultados \
                 (index {} + pageSize {})",
                self.index, self.page_size
            )));
        }
        check_count(
            "versões do Minecraft",
            self.game_versions.len(),
            MAX_GAME_VERSIONS,
        )?;
        check_count("loaders", self.loaders.len(), MAX_MOD_LOADER_TYPES)?;
        check_count("categorias", self.category_ids.len(), MAX_CATEGORY_IDS)?;
        Ok(())
    }

    /// Parâmetros da URL (sem conferir os limites).
    #[must_use]
    pub fn to_params(&self) -> Vec<(&'static str, String)> {
        let mut params = vec![("gameId", MINECRAFT_GAME_ID.to_string())];
        if let Some(class) = self.class {
            params.push(("classId", class.id().to_string()));
        }
        let text = self.text.trim();
        if !text.is_empty() {
            params.push(("searchFilter", text.to_owned()));
        }
        let versions: Vec<&str> = self
            .game_versions
            .iter()
            .map(|version| version.trim())
            .filter(|version| !version.is_empty())
            .collect();
        match versions.as_slice() {
            [] => {}
            [version] => params.push(("gameVersion", (*version).to_owned())),
            many => params.push(("gameVersions", json_strings(many))),
        }
        match self.loaders.as_slice() {
            [] => {}
            [loader] => params.push(("modLoaderType", loader.id().to_string())),
            many => params.push((
                "modLoaderTypes",
                json_numbers(many.iter().map(|loader| loader.id())),
            )),
        }
        match self.category_ids.as_slice() {
            [] => {}
            [id] => params.push(("categoryId", id.to_string())),
            many => params.push(("categoryIds", json_numbers(many.iter().copied()))),
        }
        if let Some(slug) = self
            .slug
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            params.push(("slug", slug.to_owned()));
        }
        params.push(("sortField", self.sort_field.id().to_string()));
        params.push(("sortOrder", self.sort_order.as_str().to_owned()));
        params.push(("index", self.index.to_string()));
        params.push(("pageSize", self.page_size.to_string()));
        params
    }
}

/// Filtro da lista de arquivos de um projeto.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FilesQuery {
    /// Versão do Minecraft.
    pub game_version: Option<String>,
    /// Loader.
    pub loader: Option<ModLoaderType>,
    /// Posição do primeiro arquivo.
    pub index: u32,
    /// Arquivos por página (1 a 50).
    pub page_size: u32,
}

impl Default for FilesQuery {
    fn default() -> Self {
        Self {
            game_version: None,
            loader: None,
            index: 0,
            page_size: MAX_PAGE_SIZE,
        }
    }
}

impl FilesQuery {
    /// Só arquivos para esta versão do Minecraft.
    #[must_use]
    pub fn game_version(mut self, version: impl Into<String>) -> Self {
        self.game_version = Some(version.into());
        self
    }

    /// Só arquivos para este loader.
    #[must_use]
    pub fn loader(mut self, loader: ModLoaderType) -> Self {
        self.loader = Some(loader);
        self
    }

    /// Página.
    #[must_use]
    pub fn page(mut self, index: u32, page_size: u32) -> Self {
        self.index = index;
        self.page_size = page_size;
        self
    }

    /// Confere os limites da API.
    pub fn validate(&self) -> Result<()> {
        check_page(self.index, self.page_size)
    }

    /// Parâmetros da URL.
    #[must_use]
    pub fn to_params(&self) -> Vec<(&'static str, String)> {
        let mut params = Vec::new();
        if let Some(version) = self
            .game_version
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty())
        {
            params.push(("gameVersion", version.to_owned()));
        }
        if let Some(loader) = self.loader {
            params.push(("modLoaderType", loader.id().to_string()));
        }
        params.push(("index", self.index.to_string()));
        params.push(("pageSize", self.page_size.to_string()));
        params
    }
}

fn check_page(index: u32, page_size: u32) -> Result<()> {
    if page_size == 0 || page_size > MAX_PAGE_SIZE {
        return Err(Error::InvalidQuery(format!(
            "pageSize {page_size} fora de 1 a {MAX_PAGE_SIZE}"
        )));
    }
    if index.checked_add(page_size).is_none() {
        return Err(Error::InvalidQuery(format!("index {index} grande demais")));
    }
    Ok(())
}

fn check_count(what: &str, count: usize, max: usize) -> Result<()> {
    if count > max {
        return Err(Error::InvalidQuery(format!(
            "{count} {what}; a CurseForge aceita até {max}"
        )));
    }
    Ok(())
}

/// Lista JSON de textos, como a API pede em `gameVersions`.
fn json_strings(items: &[&str]) -> String {
    serde_json::Value::from(items.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>()).to_string()
}

/// Lista JSON de números.
fn json_numbers(items: impl Iterator<Item = u32>) -> String {
    serde_json::Value::from(items.collect::<Vec<_>>()).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn param<'a>(params: &'a [(&str, String)], name: &str) -> Option<&'a str> {
        params
            .iter()
            .find(|(key, _)| *key == name)
            .map(|(_, value)| value.as_str())
    }

    #[test]
    fn busca_simples() {
        let query = SearchQuery::new(" jei ")
            .class(ProjectClass::Mod)
            .game_version("1.20.1")
            .loader(ModLoaderType::Forge);
        query.validate().unwrap();
        let params = query.to_params();
        assert_eq!(param(&params, "gameId"), Some("432"));
        assert_eq!(param(&params, "classId"), Some("6"));
        assert_eq!(param(&params, "searchFilter"), Some("jei"));
        assert_eq!(param(&params, "gameVersion"), Some("1.20.1"));
        assert_eq!(param(&params, "modLoaderType"), Some("1"));
        assert_eq!(param(&params, "sortField"), Some("2"));
        assert_eq!(param(&params, "sortOrder"), Some("desc"));
        assert_eq!(param(&params, "index"), Some("0"));
        assert_eq!(param(&params, "pageSize"), Some("20"));
        assert_eq!(param(&params, "gameVersions"), None);
    }

    #[test]
    fn busca_com_listas() {
        let query = SearchQuery::new("")
            .class(ProjectClass::Shader)
            .game_version("1.20.1")
            .game_version("1.19.2")
            .loader(ModLoaderType::Forge)
            .loader(ModLoaderType::NeoForge)
            .category(1)
            .category(2)
            .slug("x")
            .sort(SortField::Name, SortOrder::Ascending)
            .page(40, 50);
        query.validate().unwrap();
        let params = query.to_params();
        assert_eq!(param(&params, "classId"), Some("6552"));
        assert_eq!(param(&params, "searchFilter"), None);
        assert_eq!(
            param(&params, "gameVersions"),
            Some(r#"["1.20.1","1.19.2"]"#)
        );
        assert_eq!(param(&params, "modLoaderTypes"), Some("[1,6]"));
        assert_eq!(param(&params, "categoryIds"), Some("[1,2]"));
        assert_eq!(param(&params, "slug"), Some("x"));
        assert_eq!(param(&params, "sortField"), Some("4"));
        assert_eq!(param(&params, "sortOrder"), Some("asc"));
        assert_eq!(
            param(&SearchQuery::new("a").category(9).to_params(), "categoryId"),
            Some("9")
        );
    }

    #[test]
    fn limites_da_api() {
        assert!(SearchQuery::new("a").page(0, 50).validate().is_ok());
        assert!(SearchQuery::new("a").page(9950, 50).validate().is_ok());
        for bad in [
            SearchQuery::new("a").page(0, 0),
            SearchQuery::new("a").page(0, 51),
            SearchQuery::new("a").page(9951, 50),
            SearchQuery::new("a").page(u32::MAX, 1),
            (0..5).fold(SearchQuery::new("a"), |q, i| q.game_version(i.to_string())),
            (0..6).fold(SearchQuery::new("a"), |q, _| q.loader(ModLoaderType::Forge)),
            (0..11).fold(SearchQuery::new("a"), SearchQuery::category),
        ] {
            let error = bad.validate().unwrap_err();
            assert!(matches!(error, Error::InvalidQuery(_)), "{bad:?}");
        }
        assert!(FilesQuery::default().validate().is_ok());
        assert!(FilesQuery::default().page(0, 51).validate().is_err());
    }

    #[test]
    fn filtro_de_arquivos() {
        let params = FilesQuery::default()
            .game_version("1.20.1")
            .loader(ModLoaderType::Fabric)
            .page(50, 10)
            .to_params();
        assert_eq!(param(&params, "gameVersion"), Some("1.20.1"));
        assert_eq!(param(&params, "modLoaderType"), Some("4"));
        assert_eq!(param(&params, "index"), Some("50"));
        assert_eq!(param(&params, "pageSize"), Some("10"));
        let params = FilesQuery::default().game_version(" ").to_params();
        assert_eq!(param(&params, "gameVersion"), None);
    }
}
