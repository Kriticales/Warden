//! Parâmetros das consultas: busca com facets, filtros de versões e de atualizações.
//!
//! Facets do Modrinth (R3 §3.1): uma lista de listas em JSON; dentro da mesma lista vale "ou",
//! entre listas vale "e". Cada item é `tipo<operador>valor`, com os operadores `:`, `!=`,
//! `>=`, `>`, `<=` e `<`. Loaders são categorias (`categories:fabric`). O JSON vai codificado
//! na URL: com colchetes crus a API responde 400 (ARCHITECTURE §17.1).

use serde::Serialize;

use crate::model::{HashAlgorithm, ProjectType, VersionType};

/// Ordem da busca.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SearchIndex {
    /// Relevância (padrão).
    #[default]
    Relevance,
    /// Mais baixados.
    Downloads,
    /// Mais seguidos.
    Follows,
    /// Mais novos.
    Newest,
    /// Atualizados há menos tempo.
    Updated,
}

impl SearchIndex {
    /// Texto da API.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Relevance => "relevance",
            Self::Downloads => "downloads",
            Self::Follows => "follows",
            Self::Newest => "newest",
            Self::Updated => "updated",
        }
    }
}

/// Facets da busca: cada grupo é um "ou"; os grupos se combinam por "e".
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Facets {
    groups: Vec<Vec<String>>,
}

impl Facets {
    /// Sem filtros.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Acrescenta um grupo "ou" de facets já montados (`"categories:fabric"`). Grupo vazio é
    /// ignorado.
    #[must_use]
    pub fn any_of<I, S>(mut self, facets: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let group: Vec<String> = facets.into_iter().map(Into::into).collect();
        if !group.is_empty() {
            self.groups.push(group);
        }
        self
    }

    /// Exige um facet (`tipo:valor`).
    #[must_use]
    pub fn require(self, kind: &str, value: &str) -> Self {
        self.any_of([format!("{kind}:{value}")])
    }

    /// Tipo de projeto (`project_type:mod`).
    #[must_use]
    pub fn project_type(self, project_type: ProjectType) -> Self {
        let name = match project_type {
            ProjectType::Mod => "mod",
            ProjectType::Modpack => "modpack",
            ProjectType::Resourcepack => "resourcepack",
            ProjectType::Shader => "shader",
            ProjectType::Datapack => "datapack",
            ProjectType::Plugin => "plugin",
            ProjectType::Unknown => return self,
        };
        self.require("project_type", name)
    }

    /// Algum dos loaders (`categories:fabric` ou `categories:quilt`…).
    #[must_use]
    pub fn loaders<I, S>(self, loaders: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        self.any_of(
            loaders
                .into_iter()
                .map(|loader| format!("categories:{}", loader.as_ref())),
        )
    }

    /// Alguma das versões do Minecraft (`versions:1.21.1`).
    #[must_use]
    pub fn game_versions<I, S>(self, versions: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        self.any_of(
            versions
                .into_iter()
                .map(|version| format!("versions:{}", version.as_ref())),
        )
    }

    /// Se não há filtro.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.groups.is_empty()
    }

    /// O JSON que a API recebe em `facets`.
    #[must_use]
    pub fn to_json(&self) -> String {
        // Uma lista de listas de textos sempre vira JSON.
        serde_json::to_string(&self.groups).unwrap_or_else(|_| "[]".to_owned())
    }
}

/// Uma busca (`GET /search`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchQuery {
    /// Texto buscado (vazio: só os filtros).
    pub query: String,
    /// Filtros.
    pub facets: Facets,
    /// Ordem.
    pub index: SearchIndex,
    /// Deslocamento.
    pub offset: u32,
    /// Tamanho da página (1 a 100; a API recusa mais de 100).
    pub limit: u32,
}

impl SearchQuery {
    /// Busca por texto, 20 por página, por relevância.
    #[must_use]
    pub fn new(query: impl Into<String>) -> Self {
        Self {
            query: query.into(),
            facets: Facets::new(),
            index: SearchIndex::Relevance,
            offset: 0,
            limit: 20,
        }
    }

    /// Troca os filtros.
    #[must_use]
    pub fn facets(mut self, facets: Facets) -> Self {
        self.facets = facets;
        self
    }

    /// Troca a ordem.
    #[must_use]
    pub fn index(mut self, index: SearchIndex) -> Self {
        self.index = index;
        self
    }

    /// Troca a página.
    #[must_use]
    pub fn page(mut self, offset: u32, limit: u32) -> Self {
        self.offset = offset;
        self.limit = limit;
        self
    }

    /// Parâmetros da URL, com o `limit` entre 1 e 100.
    #[must_use]
    pub fn to_params(&self) -> Vec<(&'static str, String)> {
        let mut params = Vec::new();
        if !self.query.trim().is_empty() {
            params.push(("query", self.query.trim().to_owned()));
        }
        if !self.facets.is_empty() {
            params.push(("facets", self.facets.to_json()));
        }
        params.push(("index", self.index.as_str().to_owned()));
        params.push(("offset", self.offset.to_string()));
        params.push(("limit", self.limit.clamp(1, 100).to_string()));
        params
    }
}

/// Filtro das versões de um projeto (`GET /project/{id}/version`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct VersionFilter {
    /// Loaders aceitos (vazio: todos).
    pub loaders: Vec<String>,
    /// Versões do Minecraft aceitas (vazio: todas).
    pub game_versions: Vec<String>,
    /// Só as em destaque.
    pub featured: Option<bool>,
    /// Trazer as notas de cada versão (deixa a resposta bem maior).
    pub include_changelog: bool,
}

impl VersionFilter {
    /// Parâmetros da URL.
    #[must_use]
    pub fn to_params(&self) -> Vec<(&'static str, String)> {
        let mut params = Vec::new();
        if !self.loaders.is_empty() {
            params.push(("loaders", json_list(&self.loaders)));
        }
        if !self.game_versions.is_empty() {
            params.push(("game_versions", json_list(&self.game_versions)));
        }
        if let Some(featured) = self.featured {
            params.push(("featured", featured.to_string()));
        }
        params.push(("include_changelog", self.include_changelog.to_string()));
        params
    }
}

/// Filtro da verificação de atualizações (`POST /version_files/update`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct UpdateFilter {
    /// Loaders do pack.
    pub loaders: Vec<String>,
    /// Versões do Minecraft do pack.
    pub game_versions: Vec<String>,
    /// Tipos aceitos (vazio: todos).
    pub version_types: Vec<VersionType>,
}

/// Corpo de `POST /version_files`.
#[derive(Debug, Serialize)]
pub(crate) struct HashesBody<'a> {
    pub(crate) hashes: &'a [String],
    pub(crate) algorithm: HashAlgorithm,
}

/// Corpo de `POST /version_files/update`.
#[derive(Debug, Serialize)]
pub(crate) struct UpdateBody<'a> {
    pub(crate) hashes: &'a [String],
    pub(crate) algorithm: HashAlgorithm,
    pub(crate) loaders: &'a [String],
    pub(crate) game_versions: &'a [String],
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub(crate) version_types: Vec<&'static str>,
}

/// Lista JSON de textos (`["a","b"]`).
#[must_use]
pub fn json_list<S: AsRef<str>>(items: &[S]) -> String {
    let items: Vec<&str> = items.iter().map(AsRef::as_ref).collect();
    serde_json::to_string(&items).unwrap_or_else(|_| "[]".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn facets_do_criterio_de_aceite() {
        let facets = Facets::new()
            .loaders(["fabric"])
            .game_versions(["1.21.1"])
            .project_type(ProjectType::Mod);
        assert_eq!(
            facets.to_json(),
            r#"[["categories:fabric"],["versions:1.21.1"],["project_type:mod"]]"#
        );
        let either = Facets::new()
            .loaders(["fabric", "quilt"])
            .any_of(Vec::<String>::new())
            .project_type(ProjectType::Unknown)
            .require("environment", "client_only");
        assert_eq!(
            either.to_json(),
            r#"[["categories:fabric","categories:quilt"],["environment:client_only"]]"#
        );
        assert!(Facets::new().is_empty());
        assert_eq!(Facets::new().to_json(), "[]");
    }

    #[test]
    fn parametros_da_busca() {
        let query = SearchQuery::new("  sodium ")
            .facets(Facets::new().loaders(["fabric"]))
            .index(SearchIndex::Downloads)
            .page(40, 500);
        assert_eq!(
            query.to_params(),
            vec![
                ("query", "sodium".to_owned()),
                ("facets", r#"[["categories:fabric"]]"#.to_owned()),
                ("index", "downloads".to_owned()),
                ("offset", "40".to_owned()),
                ("limit", "100".to_owned()),
            ]
        );
        let empty = SearchQuery::new("").page(0, 0);
        assert_eq!(
            empty.to_params(),
            vec![
                ("index", "relevance".to_owned()),
                ("offset", "0".to_owned()),
                ("limit", "1".to_owned()),
            ]
        );
        for (index, text) in [
            (SearchIndex::Follows, "follows"),
            (SearchIndex::Newest, "newest"),
            (SearchIndex::Updated, "updated"),
        ] {
            assert_eq!(index.as_str(), text);
        }
    }

    #[test]
    fn tipos_de_projeto_nos_facets() {
        for (kind, text) in [
            (ProjectType::Modpack, "modpack"),
            (ProjectType::Resourcepack, "resourcepack"),
            (ProjectType::Shader, "shader"),
            (ProjectType::Datapack, "datapack"),
            (ProjectType::Plugin, "plugin"),
        ] {
            assert_eq!(
                Facets::new().project_type(kind).to_json(),
                format!(r#"[["project_type:{text}"]]"#)
            );
        }
    }

    #[test]
    fn parametros_das_versoes() {
        let filter = VersionFilter {
            loaders: vec!["fabric".into()],
            game_versions: vec!["1.21.1".into(), "1.21".into()],
            featured: Some(true),
            include_changelog: false,
        };
        assert_eq!(
            filter.to_params(),
            vec![
                ("loaders", r#"["fabric"]"#.to_owned()),
                ("game_versions", r#"["1.21.1","1.21"]"#.to_owned()),
                ("featured", "true".to_owned()),
                ("include_changelog", "false".to_owned()),
            ]
        );
        assert_eq!(
            VersionFilter::default().to_params(),
            vec![("include_changelog", "false".to_owned())]
        );
    }
}
