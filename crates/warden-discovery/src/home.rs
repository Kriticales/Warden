//! Início da página de descoberta (SPEC T08; ARCHITECTURE §17.1): com o campo vazio, as listas
//! "Populares para <loader> <versão>" e "Atualizados recentemente", já filtradas para o pack.
//!
//! Cada lista é uma busca vazia da busca combinada, ordenada por downloads ou por atualização.
//! Cada fonte recebe **uma requisição por lista**, no total duas (o orçamento é de no máximo 3
//! por fonte). O que já está no pack não ocupa lugar nas listas: os nomes mais populares que já
//! foram adicionados vêm à parte para a nota "também estão entre os mais baixados, mas já estão
//! no seu pack".

use serde::Serialize;
use warden_core::CancellationToken;
use warden_project::Result;
use warden_project::search::{
    ActiveSource, InstalledKeys, PackTarget, ProjectKind, SearchPage, SearchRequest, SearchResult,
    SearchSort, SourceFilter, SourceId, SourceWarning, search,
};

/// Itens de cada lista do início.
pub const HOME_ITEMS: usize = 8;

/// Quantos nomes "já no pack" a nota do início cita.
const HIDDEN_NAMES: usize = 4;

/// O início da descoberta.
#[derive(Debug, Clone, PartialEq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct DiscoverHome {
    /// Os mais baixados para o pack.
    pub popular: Vec<SearchResult>,
    /// Os atualizados há menos tempo para o pack.
    pub updated: Vec<SearchResult>,
    /// Nomes dos mais baixados que já estão no pack (ficam fora de `popular`).
    pub popular_in_pack: Vec<String>,
    /// As fontes consultadas.
    pub sources: Vec<SourceId>,
    /// Fontes que ficaram de fora, e por quê.
    pub warnings: Vec<SourceWarning>,
}

fn request(kind: ProjectKind, sort: SearchSort) -> SearchRequest {
    SearchRequest {
        query: String::new(),
        kind,
        sort,
        source: SourceFilter::All,
        environment: None,
        include_incompatible: false,
        category: None,
        cursor: None,
    }
}

/// Monta o início para o tipo de projeto dado.
///
/// Erro [`warden_project::ProjectErrorCode::SearchSourceUnavailable`] só quando todas as fontes
/// falham; com uma só fora do ar, o início vem com a outra e o aviso.
pub async fn home(
    sources: &[ActiveSource],
    target: &PackTarget,
    installed: &InstalledKeys,
    kind: ProjectKind,
    cancel: &CancellationToken,
) -> Result<DiscoverHome> {
    let popular_request = request(kind, SearchSort::Downloads);
    let updated_request = request(kind, SearchSort::Updated);
    let (popular, updated) = tokio::join!(
        search(sources, target, installed, &popular_request, cancel),
        search(sources, target, installed, &updated_request, cancel),
    );
    let (popular, updated) = (popular?, updated?);
    let popular_in_pack = popular
        .items
        .iter()
        .filter(|item| item.in_pack)
        .map(|item| item.title.clone())
        .take(HIDDEN_NAMES)
        .collect();
    let warnings = merged_warnings(&popular, &updated);
    Ok(DiscoverHome {
        sources: popular.sources.clone(),
        popular: available(popular),
        updated: available(updated),
        popular_in_pack,
        warnings,
    })
}

/// Só o que ainda dá para adicionar, até [`HOME_ITEMS`].
fn available(page: SearchPage) -> Vec<SearchResult> {
    page.items
        .into_iter()
        .filter(|item| !item.in_pack && item.compatible)
        .take(HOME_ITEMS)
        .collect()
}

fn merged_warnings(first: &SearchPage, second: &SearchPage) -> Vec<SourceWarning> {
    let mut warnings: Vec<SourceWarning> = Vec::new();
    for warning in first.warnings.iter().chain(&second.warnings) {
        if !warnings
            .iter()
            .any(|known| known.source == warning.source && known.reason == warning.reason)
        {
            warnings.push(warning.clone());
        }
    }
    warnings
}
