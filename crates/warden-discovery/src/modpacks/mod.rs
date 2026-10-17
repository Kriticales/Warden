//! Navegar por modpacks (SPEC T08 "Navegar por modpacks"; R5B §6; ARCHITECTURE §17.1; P1-17).
//!
//! Com Tipo = **Modpacks**, a busca combinada da página de descoberta acha os modpacks
//! (`ProjectKind::Modpack`: Modrinth `project_type:modpack`, CurseForge `classId=4471`). Abrir um
//! deles chama [`modpack_contents`], que monta a lista **Mods deste modpack** com o estado de cada
//! mod para o pack aberto:
//!
//! - **Modrinth** ([`modrinth`]): as dependências `embedded` da versão do modpack (cada mod com
//!   `project_id` e `version_id`; arquivos fora do Modrinth só com `file_name`), com os nomes em
//!   lote por `/v2/projects?ids=`. São 3 requisições num modpack de até 199 mods (CA-T08-14:
//!   no máximo 4). Versão sem a lista (modpacks antigos): o `modrinth.index.json` lido **por
//!   partes** de dentro do `.mrpack` ([`archive`]).
//! - **CurseForge** ([`curseforge`]): a API não lista o conteúdo; o `manifest.json` é lido por
//!   partes do zip do modpack, só em memória (CA-T08-16), e os nomes vêm em lote por `POST
//!   /v1/mods` e `POST /v1/mods/files`. Modpack com `downloadUrl` nulo (autor bloqueou apps de
//!   terceiros): [`DiscoveryErrorCode::ModpackBlocked`].
//!
//! Os jars soltos em `overrides/mods/` são **Arquivo fora das lojas**: o Warden não copia jars de
//! pacotes de terceiros. Adicionar os escolhidos usa o `add_plan` de vários itens (um diálogo de
//! dependências só), com a versão do modpack (`version_id`) ou a mais nova compatível.

pub mod archive;
pub mod curseforge;
pub mod modrinth;

#[cfg(test)]
mod tests;

use std::collections::BTreeMap;

use serde::Serialize;
use warden_core::{DomainCode, DomainError};
use warden_project::dedup::{Identity, PackItems};
use warden_project::search::{PackTarget, SourceId};

use crate::DiscoveryErrorCode;

/// O estado de um mod do modpack para o pack aberto.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum ModpackModState {
    /// Tem versão para o Minecraft e o loader do pack: com caixa de seleção.
    Compatible,
    /// Já está no pack (pelo projeto na mesma fonte ou pela deduplicação entre fontes): sem caixa.
    InPack,
    /// Sem versão para o pack: desabilitado, com o motivo.
    NoVersion,
    /// Jar dentro do modpack, fora das lojas: desabilitado (o Warden não copia jars de terceiros).
    External,
}

/// Um mod do modpack.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ModpackMod {
    /// Chave estável na lista (`modrinth:<projeto>`, `curseforge:<projeto>` ou `arquivo:<nome>`).
    pub key: String,
    /// Fonte (nenhuma: arquivo fora das lojas).
    pub source: Option<SourceId>,
    /// ID do projeto na fonte.
    pub project_id: Option<String>,
    /// A versão que o modpack usa (Modrinth: ID da versão; CurseForge: ID do arquivo), quando
    /// conhecida. Vai para o `add_plan` com "a mesma do modpack".
    pub version_id: Option<String>,
    /// Nome.
    pub title: String,
    /// Resumo.
    pub summary: String,
    /// Ícone.
    pub icon_url: Option<String>,
    /// Nome do arquivo (o único dado de um arquivo fora das lojas).
    pub file_name: Option<String>,
    /// Estado para o pack aberto.
    pub state: ModpackModState,
    /// Caminho do item que já está no pack (com [`ModpackModState::InPack`]).
    pub pack_path: Option<String>,
}

/// A licença do projeto do modpack.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ModpackLicense {
    /// Nome ou identificador SPDX, quando a fonte informa (a CurseForge não informa).
    pub name: Option<String>,
    /// Licença livre conhecida (as configs e arquivos do modpack podem ser reaproveitados com as
    /// condições dela). Falso também quando a licença não é informada: a interface avisa.
    pub free: bool,
}

/// A lista de mods de um modpack, para o painel da direita da página de descoberta.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ModpackContents {
    /// Fonte do modpack.
    pub source: SourceId,
    /// ID do projeto do modpack.
    pub project_id: String,
    /// Nome do modpack.
    pub title: String,
    /// A versão do modpack lida (número legível).
    pub version_number: String,
    /// Versões do Minecraft da versão do modpack.
    pub game_versions: Vec<String>,
    /// Loaders da versão do modpack (`neoforge`, `fabric`…).
    pub loaders: Vec<String>,
    /// A versão do modpack é da mesma versão do Minecraft e do mesmo loader do pack: "a mesma do
    /// modpack (testada junto)" é o padrão do seletor **Versão**.
    pub same_target: bool,
    /// Licença do projeto do modpack.
    pub license: ModpackLicense,
    /// Página do modpack na fonte.
    pub page_url: Option<String>,
    /// Os mods, os compatíveis primeiro, depois os que já estão no pack, os sem versão e os
    /// arquivos fora das lojas (cada grupo em ordem alfabética).
    pub mods: Vec<ModpackMod>,
    /// O servidor não aceitou leitura por partes e o arquivo do modpack foi baixado inteiro (para
    /// a memória): a interface avisa.
    pub whole_download: bool,
}

/// Erro de um modpack (código do domínio `discovery`).
#[derive(Debug, thiserror::Error)]
#[error("{detail}")]
pub struct ModpackError {
    /// Código estável.
    pub code: DiscoveryErrorCode,
    /// Detalhe técnico, sem segredos.
    pub detail: String,
    /// Parâmetros da frase.
    pub params: BTreeMap<String, String>,
}

impl ModpackError {
    fn new(code: DiscoveryErrorCode, detail: impl Into<String>) -> Self {
        Self {
            code,
            detail: detail.into(),
            params: BTreeMap::new(),
        }
    }

    fn param(mut self, key: &str, value: impl Into<String>) -> Self {
        self.params.insert(key.to_owned(), value.into());
        self
    }

    pub(crate) fn unreadable(detail: impl Into<String>) -> Self {
        Self::new(DiscoveryErrorCode::ModpackUnreadable, detail)
    }
}

impl DomainError for ModpackError {
    type Code = DiscoveryErrorCode;

    fn code(&self) -> DomainCode<Self::Code> {
        DomainCode::Domain(self.code)
    }

    fn params(&self) -> BTreeMap<String, String> {
        self.params.clone()
    }
}

/// Por que um modpack não abriu: um erro da fonte (domínio `project`, como
/// `SEARCH_SOURCE_UNAVAILABLE`) ou do próprio modpack (domínio `discovery`).
#[derive(Debug, thiserror::Error)]
pub enum ModpackFailure {
    /// A fonte não respondeu ou não está ligada.
    #[error(transparent)]
    Source(#[from] warden_project::Error),
    /// O modpack não pôde ser lido.
    #[error(transparent)]
    Modpack(#[from] ModpackError),
}

/// Resultado de [`modpack_contents`].
pub type Result<T> = std::result::Result<T, ModpackFailure>;

/// O que o pack aberto aceita e o que já tem.
#[derive(Debug, Clone)]
pub struct PackView<'a> {
    /// Versões e loaders aceitos.
    pub target: &'a PackTarget,
    /// Itens do pack (para "Já no seu pack").
    pub items: &'a PackItems,
}

/// As fontes de modpacks ligadas no app.
#[derive(Debug, Clone)]
pub struct ModpackSources {
    /// O Modrinth (sempre ligado).
    pub modrinth: warden_modrinth::ModrinthClient,
    /// A CurseForge, só com chave válida.
    pub curseforge: Option<warden_curseforge::CurseforgeClient>,
    /// O cliente HTTP da leitura por partes dos arquivos dos modpacks.
    pub http: warden_http::HttpClient,
}

/// A lista de mods de um modpack, com o estado de cada um para o pack.
///
/// Erros: [`warden_project::ProjectErrorCode::SearchSourceUnavailable`] (fonte desligada ou sem
/// resposta), [`DiscoveryErrorCode::ModpackBlocked`], [`DiscoveryErrorCode::ModpackTooLarge`] e
/// [`DiscoveryErrorCode::ModpackUnreadable`].
pub async fn modpack_contents(
    sources: &ModpackSources,
    pack: &PackView<'_>,
    source: SourceId,
    project_id: &str,
    cancel: &warden_core::CancellationToken,
) -> Result<ModpackContents> {
    match source {
        SourceId::Modrinth => {
            modrinth::contents(&sources.modrinth, &sources.http, pack, project_id, cancel).await
        }
        SourceId::Curseforge => {
            let client = sources
                .curseforge
                .as_ref()
                .ok_or_else(|| unavailable(SourceId::Curseforge, "fonte não está ligada"))?;
            curseforge::contents(client, &sources.http, pack, project_id, cancel).await
        }
    }
}

/// `SEARCH_SOURCE_UNAVAILABLE` da fonte, com o detalhe técnico (sem segredos).
pub(crate) fn unavailable(source: SourceId, detail: impl std::fmt::Display) -> ModpackFailure {
    ModpackFailure::Source(
        warden_project::Error::new(
            warden_project::ProjectErrorCode::SearchSourceUnavailable,
            format!("{}: {detail}", source.label()),
        )
        .param("source", source.label()),
    )
}

/// O estado de um mod de uma loja: já no pack (mesma fonte ou outra), compatível ou sem versão.
fn store_state(
    pack: &PackView<'_>,
    source: SourceId,
    project_id: &str,
    identity: Identity<'_>,
    sha1: Option<&str>,
    compatible: bool,
) -> (ModpackModState, Option<String>) {
    let found = pack.items.same_source(source, project_id).or_else(|| {
        pack.items
            .equivalent(source, project_id, identity, sha1)
            .map(|(item, _)| item)
    });
    match found {
        Some(item) => (ModpackModState::InPack, Some(item.path.clone())),
        None if compatible => (ModpackModState::Compatible, None),
        None => (ModpackModState::NoVersion, None),
    }
}

/// Um jar fora das lojas.
fn external(file_name: &str) -> ModpackMod {
    let title = file_name
        .rsplit('/')
        .next()
        .unwrap_or(file_name)
        .trim_end_matches(".jar")
        .to_owned();
    ModpackMod {
        key: format!("arquivo:{file_name}"),
        source: None,
        project_id: None,
        version_id: None,
        title,
        summary: String::new(),
        icon_url: None,
        file_name: Some(file_name.rsplit('/').next().unwrap_or(file_name).to_owned()),
        state: ModpackModState::External,
        pack_path: None,
    }
}

/// Os jars soltos nas pastas de mods do modpack (`overrides/mods/`, `client-overrides/mods/`,
/// `server-overrides/mods/`), pelos nomes do diretório central.
fn override_jars(names: &[String]) -> Vec<String> {
    let mut jars: Vec<String> = names
        .iter()
        .filter(|name| {
            let lower = name.to_ascii_lowercase();
            std::path::Path::new(&lower)
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("jar"))
                && [
                    "overrides/mods/",
                    "client-overrides/mods/",
                    "server-overrides/mods/",
                ]
                .iter()
                .any(|prefix| lower.starts_with(prefix))
        })
        .cloned()
        .collect();
    jars.sort();
    jars.dedup();
    jars
}

/// Ordena: compatíveis, no pack, sem versão, fora das lojas; cada grupo pelo nome.
fn sort_mods(mods: &mut [ModpackMod]) {
    let rank = |state: ModpackModState| match state {
        ModpackModState::Compatible => 0,
        ModpackModState::InPack => 1,
        ModpackModState::NoVersion => 2,
        ModpackModState::External => 3,
    };
    mods.sort_by(|a, b| {
        rank(a.state)
            .cmp(&rank(b.state))
            .then_with(|| a.title.to_lowercase().cmp(&b.title.to_lowercase()))
    });
}

/// Licenças livres conhecidas (SPDX). Fora desta lista, ou sem licença, a interface avisa que
/// as configs e arquivos do modpack são trabalho do autor.
fn is_free_license(id: &str) -> bool {
    const FREE: [&str; 16] = [
        "mit",
        "apache-2.0",
        "bsd-2-clause",
        "bsd-3-clause",
        "isc",
        "zlib",
        "unlicense",
        "cc0-1.0",
        "mpl-2.0",
        "lgpl-2.1",
        "lgpl-3.0",
        "gpl-2.0",
        "gpl-3.0",
        "agpl-3.0",
        "cc-by-4.0",
        "cc-by-sa-4.0",
    ];
    let id = id.trim().to_ascii_lowercase();
    let id = id
        .trim_end_matches("-only")
        .trim_end_matches("-or-later")
        .trim_end_matches('+');
    FREE.contains(&id)
}
