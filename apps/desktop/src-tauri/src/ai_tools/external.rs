//! As consultas externas da IA (ARCHITECTURE §9.5; ROADMAP D-14; CA-T14-13): `search_mod_issues`
//! e `get_mod_changelog`, sobre o que o app já tem.
//!
//! - **Issues:** o repositório vem dos links do projeto (`issues_url`/`source_url` do Modrinth,
//!   `links.issuesUrl`/`sourceUrl` da CurseForge), pela mesma pré-visualização da página
//!   Adicionar (`project_preview`). A busca é a [`GithubIssues`] da `warden-ai`, guardada no
//!   [`GithubHub`] do estado (o cache de 1 h vale para todas as conversas da execução), com o
//!   token do GitHub quando ele existe. Só chega aqui com a caixa do GitHub marcada: o laço
//!   recusa antes (`Needs::Github`). Uma busca que falha depois de sair (limite, repositório
//!   renomeado, rede) ainda leva o pedido exato, para o bloco "Enviado ao GitHub" aparecer.
//! - Cancelar a conversa para as consultas de rede no meio (o token de [`PackTools::cancel`]).
//! - **Changelog:** as versões compatíveis com o pack vêm do "Trocar de versão" (P1-15, as
//!   mesmas do diálogo, já filtradas pelo Minecraft e pelo loader); as notas, do cliente do
//!   Modrinth (com cache) ou da CurseForge (só em memória, termos da CurseForge), só das versões
//!   escolhidas.

use std::sync::{Arc, Mutex, PoisonError};

use serde_json::Value;
use warden_ai::external::{
    ChangelogVersion, GithubIssues, GithubRepo, changelog_items, changelog_range, html_to_text,
    search_terms,
};
use warden_ai::tools::{ToolItem, ToolOutput};
use warden_http::HttpConfig;
use warden_project::inventory::{InventoryItem, ItemSource};
use warden_project::search::{SourceId, project_preview};
use warden_secrets::SecretKind;

use super::PackTools;
use crate::commands::add::{add_sources, search_sources};

/// A busca de issues do app, criada na primeira consulta.
#[derive(Default)]
pub(crate) struct GithubHub {
    issues: Mutex<Option<Arc<GithubIssues>>>,
    /// API do GitHub nos testes (simulado).
    #[cfg(test)]
    pub(crate) api: Mutex<Option<String>>,
}

impl std::fmt::Debug for GithubHub {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GithubHub").finish_non_exhaustive()
    }
}

impl GithubHub {
    fn issues(&self) -> Result<Arc<GithubIssues>, String> {
        let mut slot = self.issues.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(issues) = slot.as_ref() {
            return Ok(Arc::clone(issues));
        }
        let config = HttpConfig::for_version(env!("CARGO_PKG_VERSION"));
        #[cfg(test)]
        let created = match self
            .api
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
        {
            Some(api) => GithubIssues::with_api(&config, &api),
            None => GithubIssues::new(&config),
        };
        #[cfg(not(test))]
        let created = GithubIssues::new(&config);
        let issues = Arc::new(created.map_err(|error| error.to_string())?);
        *slot = Some(Arc::clone(&issues));
        Ok(issues)
    }
}

fn source_of(item: &InventoryItem) -> Result<(SourceId, String), String> {
    let project = item.project_id.clone();
    match (item.source, project) {
        (ItemSource::Modrinth, Some(project)) => Ok((SourceId::Modrinth, project)),
        (ItemSource::Curseforge, Some(project)) => Ok((SourceId::Curseforge, project)),
        _ => Err(format!(
            "O mod '{}' não vem do Modrinth nem da CurseForge: não há página com changelog ou repositório.",
            item.name
        )),
    }
}

async fn item_of(tools: &PackTools<'_>, mod_id: &str) -> Result<InventoryItem, String> {
    tools
        .pack_mods()
        .await
        .find_item(mod_id)
        .cloned()
        .ok_or_else(|| {
            format!("O mod '{mod_id}' não está no pack; use um id devolvido por list_mods.")
        })
}

/// `search_mod_issues(mod_id, query)`.
pub(super) async fn search_mod_issues(
    tools: &PackTools<'_>,
    args: &Value,
) -> Result<ToolOutput, String> {
    let mod_id = super::text_arg(args, "mod_id");
    let query = super::text_arg(args, "query");
    let terms = search_terms(&query, &tools.redactor).map_err(|error| error.to_string())?;
    let item = item_of(tools, &mod_id).await?;
    let (source, project) = source_of(&item)?;
    let preview = project_preview(
        &search_sources(tools.state),
        source,
        &project,
        &tools.cancel,
    )
    .await
    .map_err(|error| format!("Não foi possível ler a página do mod ({error})."))?;
    let repository = GithubRepo::from_links(
        preview.links.issues.as_deref(),
        preview.links.source.as_deref(),
    )
    .ok_or_else(|| warden_ai::external::ExternalError::NoRepository.to_string())?;
    let token = tools.state.secrets.get(SecretKind::Github).ok().flatten();
    let issues = tools.state.ai.github.issues()?;
    let request = issues
        .request(&repository, &terms)
        .map_err(|error| error.to_string())?;
    match issues
        .search(
            &repository,
            &terms,
            token.as_ref(),
            &tools.redactor,
            Some(&tools.cancel),
        )
        .await
    {
        Ok(search) => Ok(ToolOutput {
            items: search.items(),
            third_party: true,
            external: Some(search.request),
            ..ToolOutput::default()
        }),
        // O pedido saiu (ou pode ter saído): o bloco "Enviado ao GitHub" aparece mesmo assim,
        // e a IA recebe o motivo da falha.
        Err(error) => Ok(ToolOutput {
            items: vec![ToolItem::new(
                format!("issue:{}", repository.label()),
                format!("A busca no GitHub falhou: {error}"),
            )],
            external: Some(request),
            ..ToolOutput::default()
        }),
    }
}

/// `get_mod_changelog(mod_id, from?, to?)`.
pub(super) async fn mod_changelog(
    tools: &PackTools<'_>,
    args: &Value,
) -> Result<ToolOutput, String> {
    let mod_id = super::text_arg(args, "mod_id");
    let item = item_of(tools, &mod_id).await?;
    let (source, project) = source_of(&item)?;
    let options = {
        let _lock = tools.state.locks.read(tools.pack_id).await;
        warden_project::change_version::options(
            &tools.root,
            &add_sources(tools.state),
            &item.path,
            &tools.cancel,
        )
        .await
        .map_err(|error| format!("Não foi possível ler as versões do mod ({error})."))?
    };
    let versions: Vec<ChangelogVersion> = options
        .versions
        .iter()
        .map(|version| ChangelogVersion {
            id: version.id.clone(),
            number: version.number.clone(),
            published: version.published.clone(),
            text: String::new(),
        })
        .collect();
    let from = match super::text_arg(args, "from") {
        from if !from.is_empty() => from,
        _ => options.current_id.clone().ok_or(
            "A versão instalada não está entre as compatíveis; diga a versão de partida (from).",
        )?,
    };
    let to = match super::text_arg(args, "to") {
        to if !to.is_empty() => to,
        _ => versions
            .first()
            .map(|version| version.id.clone())
            .ok_or("O mod não tem versões compatíveis com o pack.")?,
    };
    let (range, left_out) =
        changelog_range(&versions, &from, &to).map_err(|error| error.to_string())?;
    let mut with_notes: Vec<ChangelogVersion> = range.into_iter().cloned().collect();
    fill_notes(tools, source, &project, &mut with_notes).await?;
    let refs: Vec<&ChangelogVersion> = with_notes.iter().collect();
    Ok(ToolOutput {
        items: changelog_items(&mod_id, &refs, left_out, &tools.redactor),
        third_party: true,
        ..ToolOutput::default()
    })
}

/// As notas das versões escolhidas, pela fonte do mod.
async fn fill_notes(
    tools: &PackTools<'_>,
    source: SourceId,
    project: &str,
    versions: &mut [ChangelogVersion],
) -> Result<(), String> {
    match source {
        SourceId::Modrinth => {
            let ids: Vec<String> = versions.iter().map(|version| version.id.clone()).collect();
            let found = tools
                .state
                .modrinth
                .versions(&ids, Some(&tools.cancel))
                .await
                .map_err(|error| format!("O Modrinth não devolveu as notas ({error})."))?;
            for version in versions.iter_mut() {
                if let Some(notes) = found
                    .iter()
                    .find(|found| found.id == version.id)
                    .and_then(|found| found.changelog.clone())
                {
                    version.text = notes;
                }
            }
        }
        SourceId::Curseforge => {
            let client = crate::commands::inventory::curseforge(tools.state).map_err(|_| {
                "Sem a chave da CurseForge: não dá para ler o changelog.".to_owned()
            })?;
            let mod_id: u64 = project
                .parse()
                .map_err(|_| format!("Id da CurseForge inválido: {project}"))?;
            for version in versions.iter_mut() {
                let file_id: u64 = version
                    .id
                    .parse()
                    .map_err(|_| format!("Arquivo da CurseForge inválido: {}", version.id))?;
                let html = client
                    .file_changelog(mod_id, file_id, Some(&tools.cancel))
                    .await
                    .map_err(|error| format!("A CurseForge não devolveu as notas ({error})."))?;
                version.text = html_to_text(&html);
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
