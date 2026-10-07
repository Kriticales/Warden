//! Modelos da API v2 do Modrinth (R3 §3.1), conferidos com respostas reais gravadas em
//! 04/10/2026 (`tests/fixtures/http/`).
//!
//! Leitura tolerante: campos ausentes viram o padrão, e valores de enumeração desconhecidos
//! viram `Unknown`, para uma mudança na API não derrubar a leitura. O cache guarda o JSON
//! cru, então nada se perde ao reler com uma versão mais nova do Warden.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Tipo de projeto.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProjectType {
    /// Mod.
    Mod,
    /// Modpack.
    Modpack,
    /// Resource pack.
    Resourcepack,
    /// Shader.
    Shader,
    /// Datapack.
    Datapack,
    /// Plugin de servidor.
    Plugin,
    /// Outro tipo ainda desconhecido.
    #[serde(other)]
    Unknown,
}

/// Situação do projeto no Modrinth (`status`; usado pela manutenção dos mods da 1.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProjectStatus {
    /// Aprovado e público.
    Approved,
    /// Arquivado pelo autor.
    Archived,
    /// Recusado pela moderação.
    Rejected,
    /// Rascunho.
    Draft,
    /// Público, mas fora das buscas.
    Unlisted,
    /// Em análise.
    Processing,
    /// Retido pela moderação.
    Withheld,
    /// Publicação agendada.
    Scheduled,
    /// Privado.
    Private,
    /// Outra situação ainda desconhecida.
    #[serde(other)]
    Unknown,
}

/// Suporte de um lado (campos antigos `client_side`/`server_side`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SideSupport {
    /// Obrigatório.
    Required,
    /// Opcional.
    Optional,
    /// Não funciona nesse lado.
    Unsupported,
    /// Não informado.
    #[default]
    #[serde(other)]
    Unknown,
}

/// Ambiente em que o projeto ou a versão roda (campo novo `environment`, que substitui
/// `client_side`/`server_side`). No projeto vem como lista; na versão, como um valor
/// (R7 §9 item 4, conferido em 04/10/2026).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Environment {
    /// Cliente e servidor.
    ClientAndServer,
    /// Só cliente.
    ClientOnly,
    /// Cliente; opcional no servidor.
    ClientOnlyServerOptional,
    /// Só um jogador.
    SingleplayerOnly,
    /// Só servidor.
    ServerOnly,
    /// Servidor; opcional no cliente.
    ServerOnlyClientOptional,
    /// Só servidor dedicado.
    DedicatedServerOnly,
    /// Cliente ou servidor (qualquer um basta).
    ClientOrServer,
    /// Cliente ou servidor, de preferência os dois.
    ClientOrServerPrefersBoth,
    /// Não informado ou desconhecido.
    #[serde(other)]
    Unknown,
}

impl Environment {
    /// Texto da API (`client_only`…), o que o packwiz grava e a
    /// `warden_packwiz::side_from_modrinth` lê.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ClientAndServer => "client_and_server",
            Self::ClientOnly => "client_only",
            Self::ClientOnlyServerOptional => "client_only_server_optional",
            Self::SingleplayerOnly => "singleplayer_only",
            Self::ServerOnly => "server_only",
            Self::ServerOnlyClientOptional => "server_only_client_optional",
            Self::DedicatedServerOnly => "dedicated_server_only",
            Self::ClientOrServer => "client_or_server",
            Self::ClientOrServerPrefersBoth => "client_or_server_prefers_both",
            Self::Unknown => "unknown",
        }
    }
}

/// Licença.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct License {
    /// Identificador SPDX (ou `LicenseRef-…`).
    #[serde(default)]
    pub id: String,
    /// Nome, quando a licença é personalizada.
    #[serde(default)]
    pub name: String,
    /// Link do texto.
    #[serde(default)]
    pub url: Option<String>,
}

/// Imagem da galeria.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GalleryImage {
    /// Link da miniatura (350 px); a imagem inteira está em `raw_url`.
    pub url: String,
    /// Link da imagem em tamanho original.
    #[serde(default)]
    pub raw_url: Option<String>,
    /// Destaque.
    #[serde(default)]
    pub featured: bool,
    /// Título.
    #[serde(default)]
    pub title: Option<String>,
    /// Descrição.
    #[serde(default)]
    pub description: Option<String>,
}

/// Projeto (`GET /project/{id}` e `GET /projects?ids=`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Project {
    /// Identificador estável (`AANobbMI`).
    pub id: String,
    /// Slug (`sodium`); pode mudar.
    pub slug: String,
    /// Tipo.
    pub project_type: ProjectType,
    /// Nome.
    pub title: String,
    /// Resumo.
    #[serde(default)]
    pub description: String,
    /// Descrição completa, em Markdown.
    #[serde(default)]
    pub body: String,
    /// Categorias principais.
    #[serde(default)]
    pub categories: Vec<String>,
    /// Categorias adicionais.
    #[serde(default)]
    pub additional_categories: Vec<String>,
    /// Loaders.
    #[serde(default)]
    pub loaders: Vec<String>,
    /// Versões do Minecraft de todas as versões do projeto.
    #[serde(default)]
    pub game_versions: Vec<String>,
    /// IDs das versões.
    #[serde(default)]
    pub versions: Vec<String>,
    /// Ambientes (campo novo).
    #[serde(default)]
    pub environment: Vec<Environment>,
    /// Lado cliente (campo antigo).
    #[serde(default)]
    pub client_side: SideSupport,
    /// Lado servidor (campo antigo).
    #[serde(default)]
    pub server_side: SideSupport,
    /// Situação.
    pub status: ProjectStatus,
    /// Data de publicação (RFC 3339).
    #[serde(default)]
    pub published: String,
    /// Última atualização (RFC 3339).
    #[serde(default)]
    pub updated: String,
    /// Downloads.
    #[serde(default)]
    pub downloads: u64,
    /// Seguidores.
    #[serde(default)]
    pub followers: u64,
    /// Ícone.
    #[serde(default)]
    pub icon_url: Option<String>,
    /// Licença.
    #[serde(default)]
    pub license: Option<License>,
    /// Código-fonte.
    #[serde(default)]
    pub source_url: Option<String>,
    /// Página de problemas.
    #[serde(default)]
    pub issues_url: Option<String>,
    /// Wiki.
    #[serde(default)]
    pub wiki_url: Option<String>,
    /// Discord.
    #[serde(default)]
    pub discord_url: Option<String>,
    /// Equipe.
    #[serde(default)]
    pub team: String,
    /// Organização, se houver.
    #[serde(default)]
    pub organization: Option<String>,
    /// Galeria.
    #[serde(default)]
    pub gallery: Vec<GalleryImage>,
}

/// Tipo da versão.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum VersionType {
    /// Versão final.
    Release,
    /// Beta.
    Beta,
    /// Alfa.
    Alpha,
    /// Desconhecido.
    #[serde(other)]
    Unknown,
}

impl VersionType {
    /// Texto da API.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Release => "release",
            Self::Beta => "beta",
            Self::Alpha => "alpha",
            Self::Unknown => "unknown",
        }
    }
}

/// Tipo de dependência.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DependencyType {
    /// Obrigatória.
    Required,
    /// Opcional.
    Optional,
    /// Incompatível (usada na prática: o Embeddium declara o Sodium e o Rubidium).
    Incompatible,
    /// Embutida no arquivo.
    Embedded,
    /// Desconhecida.
    #[serde(other)]
    Unknown,
}

/// Dependência de uma versão.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Dependency {
    /// Versão exigida, se a dependência fixa uma.
    #[serde(default)]
    pub version_id: Option<String>,
    /// Projeto exigido.
    #[serde(default)]
    pub project_id: Option<String>,
    /// Nome do arquivo, para dependências fora do Modrinth.
    #[serde(default)]
    pub file_name: Option<String>,
    /// Tipo.
    pub dependency_type: DependencyType,
}

/// Hashes de um arquivo de versão.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct VersionFileHashes {
    /// SHA-1.
    #[serde(default)]
    pub sha1: String,
    /// SHA-512.
    #[serde(default)]
    pub sha512: String,
    /// Outros algoritmos que a API venha a mandar.
    #[serde(flatten)]
    pub other: BTreeMap<String, String>,
}

/// Arquivo de uma versão.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VersionFile {
    /// Hashes.
    #[serde(default)]
    pub hashes: VersionFileHashes,
    /// Link de download (`cdn.modrinth.com`).
    pub url: String,
    /// Nome do arquivo.
    pub filename: String,
    /// Se é o arquivo principal.
    #[serde(default)]
    pub primary: bool,
    /// Tamanho em bytes.
    #[serde(default)]
    pub size: u64,
    /// Tipo (`required-resource-pack`, `sources-jar`…; `None` no arquivo comum).
    #[serde(default)]
    pub file_type: Option<String>,
}

impl VersionFile {
    /// Arquivo auxiliar (código-fonte, desenvolvimento, documentação ou assinatura), que nunca
    /// vai para o pack.
    #[must_use]
    pub fn is_auxiliary(&self) -> bool {
        matches!(
            self.file_type.as_deref(),
            Some("sources-jar" | "dev-jar" | "javadoc-jar" | "signature")
        )
    }
}

/// Versão de um projeto (`GET /version/{id}`, `/versions?ids=`, `/project/{id}/version` e os
/// mapas de `/version_files`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Version {
    /// Identificador estável.
    pub id: String,
    /// Projeto.
    pub project_id: String,
    /// Autor da publicação.
    #[serde(default)]
    pub author_id: String,
    /// Nome mostrado.
    #[serde(default)]
    pub name: String,
    /// Número da versão (`mc1.21.1-0.8.13-fabric`).
    pub version_number: String,
    /// Tipo.
    pub version_type: VersionType,
    /// Versões do Minecraft.
    #[serde(default)]
    pub game_versions: Vec<String>,
    /// Loaders.
    #[serde(default)]
    pub loaders: Vec<String>,
    /// Ambiente desta versão (pode diferir do projeto).
    #[serde(default)]
    pub environment: Option<Environment>,
    /// Situação (`listed`, `archived`…).
    #[serde(default)]
    pub status: Option<String>,
    /// Destaque.
    #[serde(default)]
    pub featured: bool,
    /// Data de publicação (RFC 3339).
    #[serde(default)]
    pub date_published: String,
    /// Downloads.
    #[serde(default)]
    pub downloads: u64,
    /// Notas da versão, quando pedidas.
    #[serde(default)]
    pub changelog: Option<String>,
    /// Arquivos.
    #[serde(default)]
    pub files: Vec<VersionFile>,
    /// Dependências.
    #[serde(default)]
    pub dependencies: Vec<Dependency>,
}

impl Version {
    /// O arquivo que vai para o pack: o `primary`; sem ele, o primeiro que não seja auxiliar
    /// (R3 §3.1).
    #[must_use]
    pub fn primary_file(&self) -> Option<&VersionFile> {
        self.files
            .iter()
            .find(|file| file.primary)
            .or_else(|| self.files.iter().find(|file| !file.is_auxiliary()))
    }

    /// Dependências de um tipo.
    pub fn dependencies_of(&self, kind: DependencyType) -> impl Iterator<Item = &Dependency> {
        self.dependencies
            .iter()
            .filter(move |dependency| dependency.dependency_type == kind)
    }
}

/// Resultado da busca.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchHit {
    /// Projeto.
    pub project_id: String,
    /// Tipo.
    pub project_type: ProjectType,
    /// Slug.
    pub slug: String,
    /// Nome de quem publicou.
    #[serde(default)]
    pub author: String,
    /// Nome.
    pub title: String,
    /// Resumo.
    #[serde(default)]
    pub description: String,
    /// Categorias (com os loaders).
    #[serde(default)]
    pub categories: Vec<String>,
    /// Categorias para mostrar.
    #[serde(default)]
    pub display_categories: Vec<String>,
    /// Versões do Minecraft.
    #[serde(default)]
    pub versions: Vec<String>,
    /// Downloads.
    #[serde(default)]
    pub downloads: u64,
    /// Seguidores.
    #[serde(default)]
    pub follows: u64,
    /// Ícone.
    #[serde(default)]
    pub icon_url: Option<String>,
    /// Criação (RFC 3339).
    #[serde(default)]
    pub date_created: String,
    /// Última modificação (RFC 3339).
    #[serde(default)]
    pub date_modified: String,
    /// Versão mais recente.
    #[serde(default)]
    pub latest_version: Option<String>,
    /// Licença (SPDX).
    #[serde(default)]
    pub license: String,
    /// Lado cliente (campo antigo).
    #[serde(default)]
    pub client_side: SideSupport,
    /// Lado servidor (campo antigo).
    #[serde(default)]
    pub server_side: SideSupport,
    /// Ambientes (campo novo).
    #[serde(default)]
    pub environment: Vec<Environment>,
    /// Imagens da galeria.
    #[serde(default)]
    pub gallery: Vec<String>,
}

/// Página de resultados da busca.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchResults {
    /// Resultados.
    pub hits: Vec<SearchHit>,
    /// Deslocamento pedido.
    pub offset: u32,
    /// Tamanho da página.
    pub limit: u32,
    /// Total de resultados.
    pub total_hits: u64,
}

/// Loader (`GET /tag/loader`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoaderTag {
    /// Nome (`fabric`, `neoforge`…).
    pub name: String,
    /// Ícone em SVG.
    #[serde(default)]
    pub icon: String,
    /// Tipos de projeto que usam o loader.
    #[serde(default)]
    pub supported_project_types: Vec<String>,
}

/// Versão do Minecraft (`GET /tag/game_version`), na ordem da API (mais nova primeiro).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GameVersionTag {
    /// Versão (`1.21.1`, `26.4-snapshot-2`…).
    pub version: String,
    /// Tipo (`release`, `snapshot`, `alpha`, `beta`).
    pub version_type: String,
    /// Data de lançamento.
    #[serde(default)]
    pub date: String,
    /// Versão principal.
    #[serde(default)]
    pub major: bool,
}

/// Categoria (`GET /tag/category`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CategoryTag {
    /// Nome (`optimization`…).
    pub name: String,
    /// Ícone em SVG.
    #[serde(default)]
    pub icon: String,
    /// Tipo de projeto.
    pub project_type: String,
    /// Grupo (`categories`, `features`, `resolutions`…).
    #[serde(default)]
    pub header: String,
}

/// Algoritmo dos hashes em `/version_files`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HashAlgorithm {
    /// SHA-1.
    Sha1,
    /// SHA-512.
    Sha512,
}

impl HashAlgorithm {
    /// Texto da API.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Sha1 => "sha1",
            Self::Sha512 => "sha512",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valores_desconhecidos_nao_derrubam_a_leitura() {
        let version: Version = serde_json::from_value(serde_json::json!({
            "id": "v", "project_id": "p", "version_number": "1",
            "version_type": "nightly", "environment": "em_marte",
            "dependencies": [{"dependency_type": "sugerida"}],
            "files": [{"url": "u", "filename": "f", "hashes": {"sha1": "a", "blake3": "b"}}]
        }))
        .unwrap();
        assert_eq!(version.version_type, VersionType::Unknown);
        assert_eq!(version.environment, Some(Environment::Unknown));
        assert_eq!(
            version.dependencies[0].dependency_type,
            DependencyType::Unknown
        );
        assert_eq!(version.files[0].hashes.other["blake3"], "b");
        let project: Project = serde_json::from_value(serde_json::json!({
            "id": "p", "slug": "s", "project_type": "jogo", "title": "T", "status": "sumiu",
            "client_side": "talvez"
        }))
        .unwrap();
        assert_eq!(project.project_type, ProjectType::Unknown);
        assert_eq!(project.status, ProjectStatus::Unknown);
        assert_eq!(project.client_side, SideSupport::Unknown);
        assert!(project.environment.is_empty());
    }

    #[test]
    fn arquivo_principal_e_auxiliares() {
        let file = |name: &str, primary: bool, kind: Option<&str>| VersionFile {
            hashes: VersionFileHashes::default(),
            url: format!("https://cdn/{name}"),
            filename: name.into(),
            primary,
            size: 1,
            file_type: kind.map(Into::into),
        };
        let mut version: Version = serde_json::from_value(serde_json::json!({
            "id": "v", "project_id": "p", "version_number": "1", "version_type": "release"
        }))
        .unwrap();
        assert!(version.primary_file().is_none());
        version.files = vec![
            file("a-sources.jar", false, Some("sources-jar")),
            file("a.jar", false, None),
        ];
        assert_eq!(version.primary_file().unwrap().filename, "a.jar");
        version.files.push(file("b.jar", true, None));
        assert_eq!(version.primary_file().unwrap().filename, "b.jar");
    }

    #[test]
    fn textos_da_api() {
        assert_eq!(Environment::ClientOnly.as_str(), "client_only");
        assert_eq!(
            serde_json::to_value(Environment::ClientOrServerPrefersBoth).unwrap(),
            "client_or_server_prefers_both"
        );
        for environment in [
            Environment::ClientAndServer,
            Environment::ClientOnly,
            Environment::ClientOnlyServerOptional,
            Environment::SingleplayerOnly,
            Environment::ServerOnly,
            Environment::ServerOnlyClientOptional,
            Environment::DedicatedServerOnly,
            Environment::ClientOrServer,
            Environment::ClientOrServerPrefersBoth,
        ] {
            assert_eq!(
                serde_json::to_value(environment).unwrap(),
                environment.as_str()
            );
        }
        assert_eq!(VersionType::Beta.as_str(), "beta");
        assert_eq!(HashAlgorithm::Sha512.as_str(), "sha512");
        assert!(VersionType::Release < VersionType::Beta);
    }
}
