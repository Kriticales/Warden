//! Modelos da API da CurseForge (Core API v1; R3 §3.2), conferidos contra respostas reais em
//! 05/10/2026 (testes `rede_*`).
//!
//! Tolerantes: campos ausentes viram o padrão, campos novos são ignorados e números de enum
//! que o Warden não conhece viram `Other(n)` em vez de falhar. Nada daqui é gravado em disco
//! (termos da CurseForge, ARCHITECTURE §17): os tipos não implementam `Serialize`.

use std::collections::HashMap;

use serde::{Deserialize, Deserializer};

/// ID do Minecraft na CurseForge (`gameId`).
pub const MINECRAFT_GAME_ID: u32 = 432;

/// Enum numérico da API, com reserva para valores novos.
macro_rules! int_enum {
    (
        $(#[$meta:meta])*
        pub enum $name:ident { $($(#[$vmeta:meta])* $variant:ident = $value:literal,)+ }
    ) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum $name {
            $($(#[$vmeta])* $variant,)+
            /// Valor que o Warden ainda não conhece.
            Other(u32),
        }

        impl $name {
            /// O número na API.
            #[must_use]
            pub const fn id(self) -> u32 {
                match self {
                    $(Self::$variant => $value,)+
                    Self::Other(value) => value,
                }
            }

            /// O valor de um número da API.
            #[must_use]
            pub const fn from_id(id: u32) -> Self {
                match id {
                    $($value => Self::$variant,)+
                    other => Self::Other(other),
                }
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                Ok(Self::from_id(u32::deserialize(deserializer)?))
            }
        }
    };
}

int_enum! {
    /// Classe de projeto do Minecraft (`classId`). Confirmadas pela API em 05/10/2026
    /// (`GET /v1/categories?gameId=432&classesOnly=true`, teste
    /// `rede_classes_confirmadas_pela_api`): shaders são **6552**.
    pub enum ProjectClass {
        /// Mods (`mc-mods`).
        Mod = 6,
        /// Resource packs (`texture-packs`).
        ResourcePack = 12,
        /// Shaders (`shaders`).
        Shader = 6552,
        /// Modpacks (`modpacks`).
        Modpack = 4471,
        /// Data packs (`data-packs`).
        DataPack = 6945,
        /// Mundos (`worlds`).
        World = 17,
        /// Plugins do Bukkit (`bukkit-plugins`).
        BukkitPlugin = 5,
        /// Personalização (`customization`).
        Customization = 4546,
        /// Addons do Bedrock (`mc-addons`).
        Addon = 4559,
    }
}

int_enum! {
    /// Loader (`modLoaderType`).
    pub enum ModLoaderType {
        /// Qualquer.
        Any = 0,
        /// Forge.
        Forge = 1,
        /// Cauldron.
        Cauldron = 2,
        /// `LiteLoader`.
        LiteLoader = 3,
        /// Fabric.
        Fabric = 4,
        /// Quilt.
        Quilt = 5,
        /// NeoForge.
        NeoForge = 6,
    }
}

impl ModLoaderType {
    /// O loader pelo nome usado no `pack.toml` (`forge`, `neoforge`, `fabric`, `quilt`,
    /// `liteloader`), sem diferença de maiúsculas.
    #[must_use]
    pub fn from_loader_name(name: &str) -> Option<Self> {
        match name.trim().to_ascii_lowercase().as_str() {
            "forge" => Some(Self::Forge),
            "neoforge" => Some(Self::NeoForge),
            "fabric" => Some(Self::Fabric),
            "quilt" => Some(Self::Quilt),
            "liteloader" => Some(Self::LiteLoader),
            "cauldron" => Some(Self::Cauldron),
            _ => None,
        }
    }

    /// Nome do loader como a CurseForge escreve em `gameVersions` de um arquivo.
    #[must_use]
    pub const fn tag(self) -> Option<&'static str> {
        match self {
            Self::Forge => Some("Forge"),
            Self::NeoForge => Some("NeoForge"),
            Self::Fabric => Some("Fabric"),
            Self::Quilt => Some("Quilt"),
            Self::LiteLoader => Some("LiteLoader"),
            Self::Cauldron => Some("Cauldron"),
            Self::Any | Self::Other(_) => None,
        }
    }
}

int_enum! {
    /// Tipo de versão (`releaseType`).
    pub enum ReleaseType {
        /// Versão final.
        Release = 1,
        /// Beta.
        Beta = 2,
        /// Alfa.
        Alpha = 3,
    }
}

int_enum! {
    /// Algoritmo de um hash (`algo`).
    pub enum HashAlgo {
        /// SHA-1.
        Sha1 = 1,
        /// MD5.
        Md5 = 2,
    }
}

int_enum! {
    /// Relação de uma dependência (`relationType`).
    pub enum RelationType {
        /// Biblioteca embutida.
        EmbeddedLibrary = 1,
        /// Dependência opcional.
        OptionalDependency = 2,
        /// Dependência obrigatória.
        RequiredDependency = 3,
        /// Ferramenta.
        Tool = 4,
        /// Incompatível.
        Incompatible = 5,
        /// Incluída.
        Include = 6,
    }
}

/// Número que a API às vezes manda como decimal (`downloadCount`).
fn lenient_u64<'de, D: Deserializer<'de>>(deserializer: D) -> Result<u64, D::Error> {
    let value = Option::<serde_json::Number>::deserialize(deserializer)?;
    Ok(value
        .and_then(|number| {
            number.as_u64().or_else(|| {
                number
                    .as_f64()
                    .filter(|value| value.is_finite() && *value >= 0.0)
                    // Contadores cabem num u64; o arredondamento é o pretendido.
                    .map(|value| {
                        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                        let value = value as u64;
                        value
                    })
            })
        })
        .unwrap_or_default())
}

/// `null` vira o padrão.
fn null_default<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de> + Default,
{
    Ok(Option::<T>::deserialize(deserializer)?.unwrap_or_default())
}

/// Resposta da API: tudo vem dentro de `data`.
#[derive(Debug, Deserialize)]
pub(crate) struct Envelope<T> {
    pub(crate) data: T,
}

/// Resposta paginada.
#[derive(Debug, Deserialize)]
pub(crate) struct Paged<T> {
    pub(crate) data: Vec<T>,
    #[serde(default)]
    pub(crate) pagination: Pagination,
}

/// Paginação de uma resposta.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Pagination {
    /// Posição do primeiro item.
    pub index: u32,
    /// Tamanho pedido.
    pub page_size: u32,
    /// Itens nesta página.
    pub result_count: u32,
    /// Total de itens (a busca da CurseForge só deixa chegar aos primeiros 10 mil).
    pub total_count: u64,
}

/// Uma página de resultados da busca.
#[derive(Debug, Clone, PartialEq)]
pub struct SearchResults {
    /// Projetos, na ordem da CurseForge.
    pub mods: Vec<Mod>,
    /// Paginação.
    pub pagination: Pagination,
}

/// Uma página de arquivos de um projeto.
#[derive(Debug, Clone, PartialEq)]
pub struct FilePage {
    /// Arquivos, do mais novo para o mais antigo.
    pub files: Vec<File>,
    /// Paginação.
    pub pagination: Pagination,
}

/// Links de um projeto.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ModLinks {
    /// Página do projeto no site.
    pub website_url: Option<String>,
    /// Wiki.
    pub wiki_url: Option<String>,
    /// Relato de problemas.
    pub issues_url: Option<String>,
    /// Código-fonte.
    pub source_url: Option<String>,
}

/// Autor de um projeto.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Author {
    /// ID do autor.
    pub id: u64,
    /// Nome (usado na deduplicação da busca combinada, ADR-0027).
    pub name: String,
    /// Página do autor.
    pub url: Option<String>,
}

/// Imagem (logo ou captura de tela).
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Asset {
    /// ID.
    pub id: u64,
    /// Título.
    pub title: Option<String>,
    /// Descrição.
    pub description: Option<String>,
    /// Miniatura.
    pub thumbnail_url: Option<String>,
    /// Imagem.
    pub url: Option<String>,
}

/// Categoria (ou classe, com `is_class`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Category {
    /// ID.
    pub id: u32,
    /// Jogo.
    pub game_id: u32,
    /// Nome em inglês.
    pub name: String,
    /// Slug.
    pub slug: String,
    /// Página no site.
    pub url: Option<String>,
    /// Ícone.
    pub icon_url: Option<String>,
    /// Se é uma classe (Mods, Resource Packs, Shaders…).
    pub is_class: Option<bool>,
    /// Classe a que pertence.
    pub class_id: Option<u32>,
    /// Categoria-mãe.
    pub parent_category_id: Option<u32>,
}

/// Um projeto da CurseForge (mod, resource pack, shader, modpack…).
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Mod {
    /// ID do projeto (o `project-id` do `.pw.toml`).
    pub id: u64,
    /// Jogo (432).
    pub game_id: u32,
    /// Nome.
    pub name: String,
    /// Slug.
    pub slug: String,
    /// Links.
    #[serde(deserialize_with = "null_default")]
    pub links: ModLinks,
    /// Resumo de uma linha.
    pub summary: String,
    /// Estado do projeto (4 = aprovado).
    pub status: u32,
    /// Downloads.
    #[serde(deserialize_with = "lenient_u64")]
    pub download_count: u64,
    /// Classe (`classId`).
    pub class_id: Option<ProjectClass>,
    /// Categoria principal.
    pub primary_category_id: Option<u32>,
    /// Categorias.
    #[serde(deserialize_with = "null_default")]
    pub categories: Vec<Category>,
    /// Autores.
    #[serde(deserialize_with = "null_default")]
    pub authors: Vec<Author>,
    /// Logo.
    pub logo: Option<Asset>,
    /// Capturas de tela.
    #[serde(deserialize_with = "null_default")]
    pub screenshots: Vec<Asset>,
    /// Arquivo principal.
    pub main_file_id: u64,
    /// Arquivos mais recentes.
    #[serde(deserialize_with = "null_default")]
    pub latest_files: Vec<File>,
    /// Arquivo mais recente por versão do Minecraft e loader.
    #[serde(deserialize_with = "null_default")]
    pub latest_files_indexes: Vec<FileIndex>,
    /// Criação (ISO 8601).
    pub date_created: Option<String>,
    /// Última mudança (ISO 8601).
    pub date_modified: Option<String>,
    /// Último arquivo publicado (ISO 8601).
    pub date_released: Option<String>,
    /// Se o autor deixa apps de terceiros baixarem os arquivos (`false`: distribuição
    /// bloqueada; `null` em projetos antigos que nunca mexeram na opção).
    pub allow_mod_distribution: Option<bool>,
    /// Se aparece na busca.
    pub is_available: bool,
    /// Posição de popularidade.
    pub game_popularity_rank: Option<u64>,
}

impl Mod {
    /// Se o autor desligou a distribuição por terceiros (os arquivos vêm sem `downloadUrl`).
    #[must_use]
    pub fn is_distribution_blocked(&self) -> bool {
        self.allow_mod_distribution == Some(false)
    }

    /// Página de um arquivo no site (`<websiteUrl>/files/<fileId>`), para o download manual,
    /// como faz o packwiz.
    #[must_use]
    pub fn file_page_url(&self, file_id: u64) -> Option<String> {
        let website = self.links.website_url.as_deref()?.trim_end_matches('/');
        (!website.is_empty()).then(|| format!("{website}/files/{file_id}"))
    }
}

/// Arquivo mais recente de uma combinação de versão e loader.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct FileIndex {
    /// Versão do Minecraft.
    pub game_version: String,
    /// Arquivo.
    pub file_id: u64,
    /// Nome do arquivo.
    pub filename: String,
    /// Tipo de versão.
    pub release_type: Option<ReleaseType>,
    /// Loader.
    pub mod_loader: Option<ModLoaderType>,
}

/// Hash de um arquivo.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileHash {
    /// Valor em hexadecimal.
    pub value: String,
    /// Algoritmo.
    pub algo: HashAlgo,
}

/// Versão de jogo de um arquivo, com o tipo.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SortableGameVersion {
    /// Nome mostrado (`1.20.1`, `Forge`, `Client`…).
    pub game_version_name: String,
    /// Versão (vazia para loaders e lados).
    pub game_version: String,
    /// Tipo da versão.
    pub game_version_type_id: Option<u32>,
}

/// Dependência de um arquivo.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileDependency {
    /// Projeto de que depende.
    pub mod_id: u64,
    /// Relação.
    pub relation_type: RelationType,
}

/// Um arquivo de um projeto.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct File {
    /// ID do arquivo (o `file-id` do `.pw.toml`).
    pub id: u64,
    /// Jogo (432).
    pub game_id: u32,
    /// Projeto.
    pub mod_id: u64,
    /// Se está disponível.
    pub is_available: bool,
    /// Nome mostrado.
    pub display_name: String,
    /// Nome do arquivo.
    pub file_name: String,
    /// Tipo de versão.
    pub release_type: Option<ReleaseType>,
    /// Estado do arquivo (4 = aprovado).
    pub file_status: u32,
    /// Hashes (SHA-1 e MD5).
    #[serde(deserialize_with = "null_default")]
    pub hashes: Vec<FileHash>,
    /// Data de publicação (ISO 8601).
    pub file_date: Option<String>,
    /// Tamanho em bytes.
    #[serde(deserialize_with = "lenient_u64")]
    pub file_length: u64,
    /// Downloads.
    #[serde(deserialize_with = "lenient_u64")]
    pub download_count: u64,
    /// Endereço de download (`null` quando a distribuição por terceiros está desligada).
    /// Nunca gravado em disco (ARCHITECTURE §17).
    pub download_url: Option<String>,
    /// Versões do Minecraft, loaders e lados (`Client`, `Server`), misturados.
    #[serde(deserialize_with = "null_default")]
    pub game_versions: Vec<String>,
    /// As mesmas versões, com o tipo.
    #[serde(deserialize_with = "null_default")]
    pub sortable_game_versions: Vec<SortableGameVersion>,
    /// Dependências.
    #[serde(deserialize_with = "null_default")]
    pub dependencies: Vec<FileDependency>,
    /// Arquivo alternativo (pacote de servidor).
    pub alternate_file_id: Option<u64>,
    /// Se é um pacote de servidor.
    pub is_server_pack: Option<bool>,
    /// Pacote de servidor deste arquivo.
    pub server_pack_file_id: Option<u64>,
    /// Impressão digital (murmur2 sem espaços, semente 1).
    pub file_fingerprint: u32,
}

impl File {
    /// O hash no algoritmo dado, em minúsculas.
    #[must_use]
    pub fn hash(&self, algo: HashAlgo) -> Option<String> {
        self.hashes
            .iter()
            .find(|hash| hash.algo == algo && !hash.value.trim().is_empty())
            .map(|hash| hash.value.trim().to_ascii_lowercase())
    }

    /// SHA-1 (comparado com o do Modrinth na busca combinada, ADR-0027).
    #[must_use]
    pub fn sha1(&self) -> Option<String> {
        self.hash(HashAlgo::Sha1)
    }

    /// O endereço de download, quando a distribuição está liberada.
    #[must_use]
    pub fn download_url(&self) -> Option<&str> {
        self.download_url
            .as_deref()
            .map(str::trim)
            .filter(|url| !url.is_empty())
    }

    /// Se o arquivo não pode ser baixado por apps de terceiros (sem `downloadUrl`), como o
    /// packwiz, o packwiz-installer e o Prism decidem.
    #[must_use]
    pub fn is_distribution_blocked(&self) -> bool {
        self.download_url().is_none()
    }

    /// Versões do Minecraft do arquivo (só as que têm número, sem loaders e lados).
    #[must_use]
    pub fn minecraft_versions(&self) -> Vec<String> {
        if self.sortable_game_versions.is_empty() {
            return self
                .game_versions
                .iter()
                .filter(|version| version.starts_with(|c: char| c.is_ascii_digit()))
                .cloned()
                .collect();
        }
        self.sortable_game_versions
            .iter()
            .filter(|version| !version.game_version.is_empty())
            .map(|version| version.game_version.clone())
            .collect()
    }

    /// Loaders do arquivo, pelos nomes em `gameVersions`.
    #[must_use]
    pub fn loaders(&self) -> Vec<ModLoaderType> {
        let mut loaders = Vec::new();
        for tag in &self.game_versions {
            if let Some(loader) = ModLoaderType::from_loader_name(tag)
                && !loaders.contains(&loader)
            {
                loaders.push(loader);
            }
        }
        loaders
    }

    /// Dependências obrigatórias.
    pub fn required_dependencies(&self) -> impl Iterator<Item = u64> + '_ {
        self.dependencies
            .iter()
            .filter(|dependency| dependency.relation_type == RelationType::RequiredDependency)
            .map(|dependency| dependency.mod_id)
    }
}

/// Um arquivo reconhecido pela impressão digital.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FingerprintMatch {
    /// Projeto.
    pub id: u64,
    /// O arquivo reconhecido.
    pub file: File,
    /// Arquivos mais recentes do projeto.
    #[serde(default, deserialize_with = "null_default")]
    pub latest_files: Vec<File>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct FingerprintResponse {
    #[serde(default, deserialize_with = "null_default")]
    pub(crate) exact_matches: Vec<FingerprintMatch>,
}

/// Resultado da identificação por impressão digital.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FingerprintMatches {
    /// Impressão digital → arquivo reconhecido.
    pub matches: HashMap<u32, FingerprintMatch>,
    /// Impressões que a CurseForge não reconheceu, na ordem pedida (a API devolve `null`
    /// nesse campo, então a lista é calculada pelo Warden).
    pub unmatched: Vec<u32>,
}

/// Referência a um arquivo (`project-id` e `file-id` do `.pw.toml`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FileRef {
    /// Projeto.
    pub mod_id: u64,
    /// Arquivo.
    pub file_id: u64,
}

impl FileRef {
    /// Referência a `file_id` do projeto `mod_id`.
    #[must_use]
    pub const fn new(mod_id: u64, file_id: u64) -> Self {
        Self { mod_id, file_id }
    }
}

/// Se um arquivo pode ser baixado pelo Warden.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Distribution {
    /// Pode: o endereço (só em memória).
    Allowed {
        /// Endereço de download.
        url: String,
    },
    /// O autor bloqueou apps de terceiros: só o download manual pela página.
    Blocked {
        /// Página do arquivo no site, quando conhecida.
        page_url: Option<String>,
    },
    /// O arquivo não existe (ou não está disponível) na CurseForge.
    Missing,
}

/// Situação de um arquivo para o download.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileDistribution {
    /// O arquivo pedido.
    pub file: FileRef,
    /// Nome do arquivo, quando a CurseForge o devolveu.
    pub file_name: Option<String>,
    /// Situação.
    pub distribution: Distribution,
}

impl FileDistribution {
    /// Se só dá para baixar à mão.
    #[must_use]
    pub fn is_blocked(&self) -> bool {
        matches!(self.distribution, Distribution::Blocked { .. })
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn enums_numericos_com_reserva() {
        assert_eq!(ProjectClass::from_id(6552), ProjectClass::Shader);
        assert_eq!(ProjectClass::Shader.id(), 6552);
        assert_eq!(ProjectClass::from_id(99), ProjectClass::Other(99));
        assert_eq!(ProjectClass::Other(99).id(), 99);
        assert_eq!(ModLoaderType::from_id(6), ModLoaderType::NeoForge);
        let parsed: RelationType = serde_json::from_value(json!(5)).unwrap();
        assert_eq!(parsed, RelationType::Incompatible);
        let parsed: HashAlgo = serde_json::from_value(json!(7)).unwrap();
        assert_eq!(parsed, HashAlgo::Other(7));
        assert!(serde_json::from_value::<HashAlgo>(json!("x")).is_err());
    }

    #[test]
    fn loaders_pelo_nome() {
        assert_eq!(
            ModLoaderType::from_loader_name(" NeoForge "),
            Some(ModLoaderType::NeoForge)
        );
        assert_eq!(
            ModLoaderType::from_loader_name("fabric"),
            Some(ModLoaderType::Fabric)
        );
        assert_eq!(ModLoaderType::from_loader_name("1.20.1"), None);
        assert_eq!(ModLoaderType::Forge.tag(), Some("Forge"));
        assert_eq!(ModLoaderType::Any.tag(), None);
        for loader in [
            ModLoaderType::Forge,
            ModLoaderType::NeoForge,
            ModLoaderType::Fabric,
            ModLoaderType::Quilt,
            ModLoaderType::LiteLoader,
            ModLoaderType::Cauldron,
        ] {
            assert_eq!(
                ModLoaderType::from_loader_name(loader.tag().unwrap()),
                Some(loader)
            );
        }
    }

    #[test]
    fn arquivo_tolerante_a_nulos_e_campos_novos() {
        let file: File = serde_json::from_value(json!({
            "id": 10,
            "modId": 1,
            "fileName": "a.jar",
            "hashes": [{"value": "AB", "algo": 1}, {"value": "cd", "algo": 2}],
            "downloadUrl": null,
            "gameVersions": ["Client", "1.20.1", "Forge", "NeoForge", "Server"],
            "sortableGameVersions": null,
            "dependencies": [{"modId": 2, "relationType": 3}, {"modId": 3, "relationType": 2}],
            "fileLength": 12.0,
            "downloadCount": null,
            "campoNovo": {"x": 1}
        }))
        .unwrap();
        assert_eq!(file.sha1().as_deref(), Some("ab"));
        assert_eq!(file.hash(HashAlgo::Md5).as_deref(), Some("cd"));
        assert!(file.is_distribution_blocked());
        assert_eq!(file.minecraft_versions(), vec!["1.20.1"]);
        assert_eq!(
            file.loaders(),
            vec![ModLoaderType::Forge, ModLoaderType::NeoForge]
        );
        assert_eq!(file.required_dependencies().collect::<Vec<_>>(), vec![2]);
        assert_eq!(file.file_length, 12);
        assert_eq!(file.download_count, 0);

        let file: File = serde_json::from_value(json!({
            "id": 11,
            "downloadUrl": "  ",
            "sortableGameVersions": [
                {"gameVersionName": "Forge", "gameVersion": ""},
                {"gameVersionName": "1.20.1", "gameVersion": "1.20.1"}
            ]
        }))
        .unwrap();
        assert!(file.is_distribution_blocked());
        assert_eq!(file.minecraft_versions(), vec!["1.20.1"]);
        assert_eq!(file.sha1(), None);
    }

    #[test]
    fn projeto_e_pagina_do_arquivo() {
        let project: Mod = serde_json::from_value(json!({
            "id": 448_233,
            "name": "Entity Culling",
            "links": {"websiteUrl": "https://www.curseforge.com/minecraft/mc-mods/entityculling/"},
            "allowModDistribution": false,
            "classId": 6,
            "categories": null,
            "logo": null
        }))
        .unwrap();
        assert!(project.is_distribution_blocked());
        assert_eq!(project.class_id, Some(ProjectClass::Mod));
        assert_eq!(
            project.file_page_url(7).as_deref(),
            Some("https://www.curseforge.com/minecraft/mc-mods/entityculling/files/7")
        );
        let project: Mod =
            serde_json::from_value(json!({"id": 1, "links": null, "allowModDistribution": null}))
                .unwrap();
        assert!(!project.is_distribution_blocked());
        assert_eq!(project.file_page_url(7), None);
    }

    #[test]
    fn distribuicao() {
        let blocked = FileDistribution {
            file: FileRef::new(1, 2),
            file_name: None,
            distribution: Distribution::Blocked { page_url: None },
        };
        assert!(blocked.is_blocked());
        let missing = FileDistribution {
            distribution: Distribution::Missing,
            ..blocked
        };
        assert!(!missing.is_blocked());
    }
}
