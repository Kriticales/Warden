//! Tipos públicos da instância pronta para o Prism (ARCHITECTURE §12.3).

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// O que a pessoa decidiu antes de gerar.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct PrismOptions {
    /// Confirmou que pode distribuir os arquivos de terceiros que vão dentro do arquivo.
    pub confirm_local_files: bool,
    /// Caminhos dos `.pw.toml` de mods bloqueados que a pessoa quer trocar pelo Modrinth
    /// (a troca vale só para o arquivo gerado; o pack não muda).
    pub swaps: Vec<String>,
}

/// O mesmo arquivo, encontrado no Modrinth.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct PrismSwap {
    /// Projeto no Modrinth.
    pub project_id: String,
    /// Versão no Modrinth.
    pub version_id: String,
    /// Número da versão, para mostrar.
    pub version_number: String,
    /// Nome do arquivo no Modrinth.
    pub file_name: String,
}

/// Mod da CurseForge que o Prism não conseguiria baixar.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct PrismBlockedMod {
    /// Caminho do `.pw.toml` no pack.
    pub path: String,
    /// Nome mostrado.
    pub name: String,
    /// Nome do arquivo.
    pub file_name: String,
    /// Página do arquivo na CurseForge, quando conhecida.
    pub page_url: Option<String>,
    /// O mesmo arquivo no Modrinth, quando existe.
    pub swap: Option<PrismSwap>,
}

/// Arquivo de terceiros que iria dentro do arquivo gerado.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct PrismLocalFile {
    /// Caminho no pack.
    pub path: String,
    /// Tamanho em bytes.
    #[specta(type = specta_typescript::Number)]
    pub bytes: u64,
}

/// O que a geração vai fazer, antes de baixar nada além das conferências.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct PrismAnalysis {
    /// Versão do pack (`pack.toml`); vazia = a geração recusa.
    pub version: String,
    /// Versão do Minecraft.
    pub minecraft: String,
    /// Loader e versão exata, em texto (`Fabric 0.16.5`); vazio no vanilla.
    pub loader: String,
    /// Arquivos do Modrinth.
    #[specta(type = specta_typescript::Number)]
    pub modrinth: usize,
    /// Arquivos da CurseForge.
    #[specta(type = specta_typescript::Number)]
    pub curseforge: usize,
    /// Arquivos de link direto.
    #[specta(type = specta_typescript::Number)]
    pub links: usize,
    /// Arquivos que vão dentro do arquivo (configs, scripts e arquivos locais).
    #[specta(type = specta_typescript::Number)]
    pub overrides: usize,
    /// Quantos itens o Prism vai listar como "não confiáveis".
    #[specta(type = specta_typescript::Number)]
    pub untrusted: usize,
    /// Há mods da CurseForge e falta a chave para conferir a distribuição.
    pub curseforge_key_missing: bool,
    /// Mods que o Prism não conseguiria baixar.
    pub blocked: Vec<PrismBlockedMod>,
    /// Arquivos de terceiros (mods, resource packs e shaders locais) que pedem confirmação.
    pub local_files: Vec<PrismLocalFile>,
}

/// Arquivo gerado.
#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct PrismResult {
    /// Arquivo `.mrpack` escolhido.
    pub path: PathBuf,
    /// Tamanho do arquivo.
    #[specta(type = specta_typescript::Number)]
    pub bytes: u64,
    /// SHA-256 do arquivo, em hexadecimal.
    pub sha256: String,
    /// Arquivos baixados pelo Prism (`files[]`).
    #[specta(type = specta_typescript::Number)]
    pub files: usize,
    /// Arquivos dentro de `overrides/`.
    #[specta(type = specta_typescript::Number)]
    pub overrides: usize,
    /// Quantos itens o Prism vai listar como "não confiáveis".
    #[specta(type = specta_typescript::Number)]
    pub untrusted: usize,
}
