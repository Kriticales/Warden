//! Tipos que a tela "Exportar" troca com o backend nos formatos de outros launchers.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Formato de outro launcher, gerado pelo packwiz sobre uma cópia limpa do pack.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum LauncherFormat {
    /// `.mrpack`: app do Modrinth e launchers compatíveis.
    Mrpack,
    /// `.zip` da CurseForge.
    Curseforge,
}

impl LauncherFormat {
    /// Extensão do arquivo gerado, sem o ponto.
    #[must_use]
    pub fn extension(self) -> &'static str {
        match self {
            Self::Mrpack => "mrpack",
            Self::Curseforge => "zip",
        }
    }
}

/// De onde vem um arquivo de mod do pack.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum ItemOrigin {
    /// Referência do Modrinth.
    Modrinth,
    /// Referência da CurseForge.
    Curseforge,
    /// Link direto para outro site.
    Link,
    /// Arquivo local (`.jar` que está na pasta do pack).
    Local,
}

/// O que acontece com um arquivo que pede atenção.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ItemOutcome {
    /// O mesmo arquivo (mesmo hash) existe no Modrinth: dá para trocar a fonte e levá-lo por
    /// referência, sem embutir o jar.
    Swap {
        /// A CurseForge não deixa embutir este arquivo: a troca é o único caminho.
        required: bool,
        /// Projeto no Modrinth.
        project_id: String,
        /// Versão no Modrinth.
        version_id: String,
        /// Nome do arquivo no Modrinth.
        filename: String,
    },
    /// O autor bloqueou apps de terceiros e o arquivo não existe no Modrinth: o formato não
    /// pode levá-lo.
    Blocked {
        /// Página do arquivo na CurseForge, para o download manual.
        page_url: Option<String>,
    },
    /// O arquivo não existe mais na CurseForge.
    Unavailable,
    /// O jar vai dentro do arquivo gerado; pede a confirmação de licença.
    Embed,
}

/// Um arquivo do pack que pede decisão ou explicação antes de gerar.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct FormatItem {
    /// Caminho do `.pw.toml` (ou do arquivo local), relativo ao pack.
    pub path: String,
    /// Nome mostrado.
    pub name: String,
    /// Nome do jar.
    pub filename: String,
    /// Origem.
    pub origin: ItemOrigin,
    /// Site do link direto, quando há.
    pub host: Option<String>,
    /// O que acontece.
    pub outcome: ItemOutcome,
}

/// O que um formato não guarda; a interface escolhe o texto.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum LossKind {
    /// Mods marcados só para cliente ou só para servidor perdem a marca.
    SideNotKept,
    /// Mods só de servidor ficam de fora.
    ServerOnlyLeft,
    /// Texto e padrão dos opcionais não são guardados.
    OptionalTexts,
    /// Arquivos que o jogador pode manter (`preserve`) passam a ser sempre substituídos.
    Preserve,
    /// Mods fixados em uma versão perdem a fixação.
    Pinned,
    /// A fonte de atualização (Modrinth, CurseForge) não vai junto.
    UpdateSources,
    /// Quem joga não recebe atualizações pelo link: cada versão é um arquivo novo.
    NoAutoUpdate,
    /// Configs vão sempre para todos os lados.
    ConfigsEverywhere,
}

/// Uma perda, com a quantidade de itens do pack que ela atinge (0 = vale para o formato todo).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct FormatLoss {
    /// O que se perde.
    pub kind: LossKind,
    /// Quantos itens do pack são atingidos.
    #[specta(type = specta_typescript::Number)]
    pub count: usize,
}

/// A leitura do pack para um formato: o que se perde, o que pede decisão e o que impede.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct FormatAnalysis {
    /// O formato analisado.
    pub format: LauncherFormat,
    /// Nome do pack (nome do arquivo gerado).
    pub name: String,
    /// `version` do `pack.toml`; `None` se faltar (o formato exige).
    pub version: Option<String>,
    /// Mods levados por referência (endereço de download, sem o jar).
    #[specta(type = specta_typescript::Number)]
    pub references: usize,
    /// Arquivos de terceiros que iriam dentro do arquivo.
    #[specta(type = specta_typescript::Number)]
    pub embedded: usize,
    /// O que o formato não guarda.
    pub losses: Vec<FormatLoss>,
    /// Só os arquivos que pedem decisão ou explicação.
    pub items: Vec<FormatItem>,
}

impl FormatAnalysis {
    /// Se algum arquivo impede de gerar (bloqueado ou fora da CurseForge).
    #[must_use]
    pub fn has_blockers(&self) -> bool {
        self.items.iter().any(|item| {
            matches!(
                item.outcome,
                ItemOutcome::Blocked { .. } | ItemOutcome::Unavailable
            )
        })
    }
}

/// As escolhas da pessoa antes de gerar.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct FormatChoices {
    /// Caminhos dos mods da CurseForge a trocar pelo Modrinth (só no arquivo gerado).
    pub swap: Vec<String>,
    /// A pessoa confirmou que pode embutir os arquivos de terceiros.
    pub confirm_embed: bool,
    /// `version` a usar quando o `pack.toml` não tem (só no arquivo gerado; o pack não muda).
    pub version: Option<String>,
}

/// O que a validação do arquivo gerado conferiu.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct FormatValidation {
    /// Entradas do zip.
    #[specta(type = specta_typescript::Number)]
    pub entries: usize,
    /// Mods levados por referência.
    #[specta(type = specta_typescript::Number)]
    pub references: usize,
    /// Arquivos dentro de `overrides/` (e `client-overrides/`, `server-overrides/`).
    #[specta(type = specta_typescript::Number)]
    pub overrides: usize,
    /// Jars embutidos, com o caminho dentro do zip.
    pub embedded_jars: Vec<String>,
}

/// Resultado da exportação para outro launcher.
#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct FormatExportResult {
    /// O arquivo escolhido no diálogo nativo.
    pub path: PathBuf,
    /// Bytes do arquivo.
    #[specta(type = specta_typescript::Number)]
    pub bytes: u64,
    /// Mods da CurseForge trocados pelo Modrinth.
    #[specta(type = specta_typescript::Number)]
    pub swapped: usize,
    /// O que a validação conferiu.
    pub validation: FormatValidation,
}
