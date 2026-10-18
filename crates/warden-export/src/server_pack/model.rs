//! Tipos públicos do pacote para servidor (ARCHITECTURE §12.1; ADR-0035).

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use url::Url;

/// Como os mods chegam ao servidor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum ServerPackVariant {
    /// Os scripts rodam o packwiz-installer-bootstrap com `-s server` pelo link do pack
    /// publicado (padrão): o servidor se atualiza a cada versão publicada.
    Link,
    /// Os jars dos mods de servidor vão dentro do zip (funciona sem internet).
    Embedded,
}

/// O que a pessoa escolheu antes de gerar.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ServerPackOptions {
    /// A variante.
    pub variant: ServerPackVariant,
    /// Memória do servidor em MB, gravada em `user_jvm_args.txt` (`-Xms` e `-Xmx`).
    pub memory_mb: u32,
    /// Confirmou que pode distribuir os arquivos de terceiros que vão dentro do zip (mods,
    /// na variante com mods dentro).
    pub confirm_third_party: bool,
}

/// Como os scripts instalam o loader na primeira execução (montado pelo app a partir do
/// catálogo; os endereços são os mesmos que o servidor local usa).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoaderDownload {
    /// Sem loader: o `server.jar` do Minecraft.
    Vanilla {
        /// Endereço do `server.jar`.
        url: Url,
        /// SHA-1 publicado pela Mojang.
        sha1: Option<String>,
    },
    /// O lançador de servidor do Fabric (baixa o `server.jar` do Minecraft sozinho).
    Fabric {
        /// Endereço do lançador no Fabric meta.
        url: Url,
    },
    /// O instalador oficial do Forge ou do `NeoForge` (`--installServer`).
    Installer {
        /// Endereço do instalador no Maven.
        url: Url,
        /// SHA-1 publicado ao lado do arquivo no Maven.
        sha1: Option<String>,
    },
}

/// O Java que o servidor aceita (da política do Java do Warden).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JavaRange {
    /// Major mínimo.
    pub min: u32,
    /// Major máximo, quando o loader só abre num Java (Forge até a 1.12.2 só no 8).
    pub max: Option<u32>,
}

/// Uma pasta de primeiro nível do pacote e quantos arquivos dela vão.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ServerPackFolder {
    /// Caminho da pasta (`config`), ou `.` para os arquivos soltos na raiz.
    pub path: String,
    /// Arquivos.
    #[specta(type = specta_typescript::Number)]
    pub files: usize,
}

/// O conteúdo de uma das variantes, antes de gerar.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ServerPackContents {
    /// Pastas e arquivos do pack que vão no zip (sem os scripts e os jars dos mods).
    pub folders: Vec<ServerPackFolder>,
    /// Mods, datapacks e outros itens de referência do lado servidor: dentro do zip (com mods
    /// dentro) ou baixados pelo link.
    #[specta(type = specta_typescript::Number)]
    pub server_items: usize,
    /// Arquivos de terceiros que iriam dentro do zip e pedem confirmação de licença.
    #[specta(type = specta_typescript::Number)]
    pub third_party: usize,
}

/// O que a geração vai fazer. Só lê o pack (e, com mods dentro, consulta a CurseForge).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ServerPackAnalysis {
    /// Versão do pack (`pack.toml`).
    pub version: String,
    /// Versão do Minecraft.
    pub minecraft: String,
    /// Loader e versão exata, em texto (`Forge 47.3.0`); vazio no vanilla.
    pub loader: String,
    /// O loader tem scripts de servidor (Quilt e `LiteLoader` não têm na v1).
    pub loader_supported: bool,
    /// Itens "Só cliente" que ficam de fora (nomes, até 50).
    pub client_only: Vec<String>,
    /// Quantos são.
    #[specta(type = specta_typescript::Number)]
    pub client_only_count: usize,
    /// Arquivos do pack de cliente que ficam de fora (`options.txt`, `servers.dat`, resource
    /// packs, shaders, `kubejs/client_scripts/`).
    #[specta(type = specta_typescript::Number)]
    pub client_files: usize,
    /// O pack tem `kubejs/client_scripts/` (fica de fora).
    pub kubejs_client_scripts: bool,
    /// Arquivos em `server-overrides/` (substituem os do pack no servidor).
    #[specta(type = specta_typescript::Number)]
    pub server_overrides: usize,
    /// Pelo link do pack.
    pub link: ServerPackContents,
    /// Com mods dentro do zip.
    pub embedded: ServerPackContents,
    /// Mods da CurseForge que ainda não estão neste computador e cujo autor não deixa apps de
    /// terceiros baixarem: com mods dentro, o download manual (T20) abre antes de gerar.
    pub manual_downloads: Vec<String>,
    /// Há mods da CurseForge fora do cache e falta a chave para conferir e baixar.
    pub curseforge_key_missing: bool,
}

/// O pacote gerado.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ServerPackResult {
    /// O arquivo `.zip` escolhido.
    pub path: PathBuf,
    /// A variante.
    pub variant: ServerPackVariant,
    /// Tamanho do arquivo.
    #[specta(type = specta_typescript::Number)]
    pub bytes: u64,
    /// SHA-256 do arquivo, em hexadecimal.
    pub sha256: String,
    /// Entradas do zip, em ordem.
    pub files: Vec<String>,
    /// Jars em `mods/` dentro do zip (com mods dentro: os de referência e os locais).
    #[specta(type = specta_typescript::Number)]
    pub mods: usize,
    /// Memória gravada em `user_jvm_args.txt`, em MB.
    pub memory_mb: u32,
}
