//! O que a tela "Importar modpack" mostra (SPEC T24) e o plano guardado até "Importar como
//! pack novo".

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use warden_packwiz::PackManifest;

/// Formato reconhecido pelo conteúdo (ARCHITECTURE §12.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum SourceKind {
    /// Pasta com `pack.toml`: segue para Abrir pack (T04).
    Packwiz,
    /// `.mrpack` (`modrinth.index.json`).
    Mrpack,
    /// Zip da CurseForge (`manifest.json`).
    CurseforgeZip,
    /// Instância do Prism Launcher ou do MultiMC (`instance.cfg` + `mmc-pack.json`), pasta ou
    /// zip.
    Prism,
    /// Instância do app da CurseForge (`minecraftinstance.json`).
    CurseforgeApp,
}

impl SourceKind {
    /// Nome curto gravado em `.warden/project.toml`.
    #[must_use]
    pub fn key(self) -> &'static str {
        match self {
            Self::Packwiz => "packwiz",
            Self::Mrpack => "mrpack",
            Self::CurseforgeZip => "curseforge-zip",
            Self::Prism => "prism",
            Self::CurseforgeApp => "curseforge-app",
        }
    }
}

/// De onde vem um item que virou referência.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum ItemSource {
    /// Modrinth (`[update.modrinth]`).
    Modrinth,
    /// CurseForge (`[update.curseforge]`).
    Curseforge,
    /// Link direto de outro site (metafile em modo `url`).
    Url,
}

/// Um item que vira `.pw.toml` (referência à fonte).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ReferenceItem {
    /// Caminho do `.pw.toml` no pack novo.
    pub path: String,
    /// Nome do mod.
    pub name: String,
    /// Fonte.
    pub source: ItemSource,
    /// Jar solto identificado pelo hash (não vinha como referência no modpack).
    pub by_hash: bool,
}

/// Por que um jar fica como arquivo local.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum LocalReason {
    /// Não está no Modrinth nem na CurseForge.
    NotFound,
    /// Não está no Modrinth e a CurseForge não foi consultada (sem chave).
    NoCurseforgeKey,
    /// A consulta pelo hash falhou (sem internet ou loja fora do ar).
    LookupFailed,
}

/// Um jar que entra no pack como arquivo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct LocalItem {
    /// Caminho no pack novo.
    pub path: String,
    /// Nome do arquivo.
    pub name: String,
    /// Motivo.
    pub reason: LocalReason,
}

/// Por que um item precisa de revisão.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum ReviewReason {
    /// `env` fora da especificação (`unknown`): entra como Cliente e servidor.
    UnknownSide,
    /// Opcional no modpack: entra opcional e marcado.
    Optional,
    /// Opcional desligado no zip da CurseForge.
    OptionalDisabled,
    /// Veio de `client-overrides/`: entra para todos.
    ClientOverride,
}

/// Um item para revisar.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ReviewItem {
    /// Caminho no pack novo.
    pub path: String,
    /// Nome.
    pub name: String,
    /// Motivo.
    pub reason: ReviewReason,
}

/// Por que algo não entra no pack.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum ExcludedReason {
    /// Dados de execução do jogo ou do launcher (logs, mundos, caches de mods).
    Runtime,
    /// Segredos e arquivos de ferramentas.
    Tools,
    /// Lixo em qualquer pasta (`*.bak`, `*.log`, `*.disabled`…).
    Junk,
    /// Dados de servidor ou do instalador.
    ServerData,
    /// Arquivo de outro launcher (fora das pastas do jogo: `xmcl.json`, `instance.cfg`…).
    OtherLauncher,
    /// Item desconhecido na raiz (pode entrar, se a pessoa marcar).
    UnknownRoot,
    /// Link simbólico ou junção.
    Symlink,
}

/// Algo que não entra no pack.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ExcludedItem {
    /// Caminho (relativo ao pack novo, ou ao modpack quando está fora das pastas do jogo).
    pub path: String,
    /// Se é uma pasta (cobre tudo dentro dela).
    pub is_dir: bool,
    /// Quantos arquivos.
    pub files: u32,
    /// Motivo.
    pub reason: ExcludedReason,
    /// Se a pessoa pode marcar para entrar mesmo assim.
    pub can_include: bool,
}

/// Licença do modpack, quando a fonte informa.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct PackLicense {
    /// Identificador SPDX (ou `LicenseRef-…`).
    pub id: String,
    /// Nome legível.
    pub name: String,
    /// Se é uma licença livre conhecida.
    pub free: bool,
}

/// Memória e argumentos que viram um perfil do teste (L-08).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ImportProfile {
    /// Memória máxima, em MB.
    pub memory_mb: Option<u32>,
    /// Argumentos extras da JVM.
    pub jvm_args: String,
}

/// O que foi reconhecido e o que acontece com cada arquivo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ImportSummary {
    /// Formato.
    pub kind: SourceKind,
    /// Nome do arquivo ou da pasta de origem (sem o caminho).
    pub file_name: String,
    /// Nome do modpack.
    pub name: String,
    /// Versão do modpack (pode ser vazia).
    pub version: String,
    /// Minecraft.
    pub minecraft: String,
    /// Loader (`forge`, `neoforge`, `fabric`) ou `null` para vanilla.
    pub loader: Option<String>,
    /// Versão do loader.
    pub loader_version: Option<String>,
    /// Licença do projeto, quando a fonte informa.
    pub license: Option<PackLicense>,
    /// Viram referência.
    pub references: Vec<ReferenceItem>,
    /// Ficam como arquivo local.
    pub local_files: Vec<LocalItem>,
    /// Para revisar.
    pub review: Vec<ReviewItem>,
    /// Não entram.
    pub excluded: Vec<ExcludedItem>,
    /// Arquivos copiados para o pack (configs, scripts…).
    pub files: u32,
    /// Arquivos que vão para `server-overrides/`.
    pub server_files: u32,
    /// Preferências pessoais encontradas (`options.txt`, `servers.dat`), perguntadas.
    pub preferences: Vec<String>,
    /// Perfil do teste que será criado.
    pub profile: Option<ImportProfile>,
    /// Lojas que não responderam à procura pelo hash.
    pub unavailable_sources: Vec<String>,
}

/// De onde vêm os bytes de um arquivo do pack novo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Content {
    /// Gerado pelo Warden (metafiles).
    Bytes(Vec<u8>),
    /// Entrada da origem (relativa à raiz lida).
    Entry(String),
    /// Arquivo da cópia de staging (zip da CurseForge).
    Staged(PathBuf),
}

/// Registro da importação em `.warden/project.toml` (gancho 1.1 da ARCHITECTURE §21.7: os
/// jars que ficam como arquivo local levam a origem).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub(crate) struct LocalJar {
    pub(crate) path: String,
    pub(crate) sha256: String,
}

/// O plano completo, guardado pelo app até "Importar como pack novo" ou "Cancelar". Nada foi
/// gravado no destino.
#[derive(Debug)]
pub struct ImportPlan {
    /// O resumo para a tela.
    pub summary: ImportSummary,
    pub(crate) origin: PathBuf,
    pub(crate) origin_stamp: Option<(u64, Option<std::time::SystemTime>)>,
    pub(crate) root: String,
    pub(crate) manifest: PackManifest,
    pub(crate) files: BTreeMap<String, Content>,
    pub(crate) includable: BTreeMap<String, Vec<(String, Content)>>,
    pub(crate) preferences: BTreeMap<String, Content>,
    pub(crate) staging: Option<PathBuf>,
    pub(crate) local_jars: Vec<LocalJar>,
}

impl ImportPlan {
    /// Apaga a cópia de staging (zip da CurseForge), se houver. Chamado ao esquecer o plano.
    pub fn discard(&self) {
        if let Some(staging) = &self.staging
            && let Err(error) = std::fs::remove_dir_all(staging)
            && error.kind() != std::io::ErrorKind::NotFound
        {
            tracing::warn!(error = %error.kind(), "cópia de staging da importação não apagada");
        }
    }

    /// Quantos itens o pack novo terá para revisar.
    #[must_use]
    pub fn review_count(&self) -> usize {
        self.summary.review.len() + self.summary.local_files.len()
    }
}
