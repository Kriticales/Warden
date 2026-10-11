//! Link da CurseForge colado no campo único de "Adicionar" (SPEC T08 "Link"; ARCHITECTURE
//! §6.1; ADR-0006).
//!
//! Dois tipos de link:
//!
//! - **Projeto** (`https://www.curseforge.com/minecraft/mc-mods/jei`): [`resolve_project`]
//!   acha o projeto pelo slug na API e a tela abre a pré-visualização com o seletor de versão,
//!   igual à busca. O que entra no pack é o que o usuário escolher, pelo caminho normal
//!   (`add_plan` e `add_apply`, que escrevem o mesmo `.pw.toml` do packwiz).
//! - **Arquivo** (`…/jei/files/4712345`): quem lê o link é o **packwiz**, numa cópia do pack
//!   ([`import_file_link`]). O packwiz sobrescreve arquivos em silêncio, então ele nunca roda
//!   no pack de verdade (ARCHITECTURE §6.1); responde `n` à pergunta das dependências, que o
//!   Warden resolve sozinho pela API. Da cópia sai o projeto e o arquivo exatos, que seguem
//!   pelo caminho normal: o diálogo de dependências, os conflitos e a gravação atômica.
//!
//! Só links `https://` são aceitos; qualquer outro texto devolve `None` e quem chama tenta os
//! outros formatos (Modrinth, link direto, P1-11).

use std::path::Path;

use secrecy::SecretString;
use serde::Serialize;
use warden_core::CancellationToken;
use warden_curseforge::{CurseforgeClient, ProjectClass, SearchQuery};
use warden_packwiz::Metafile;
use warden_packwiz_cli::{CurseForgeTarget, Packwiz, RunContext, Staging};

use super::AddChoice;
use super::curseforge::kind_of;
use crate::search::{ProjectKind, SourceId};
use crate::{Error, ProjectErrorCode as Code};

/// Um link da CurseForge, já classificado.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CurseforgeLink {
    /// Link de projeto.
    Project {
        /// Tipo de projeto, pelo trecho do caminho (`mc-mods`, `texture-packs`, `shaders`).
        class: ProjectClass,
        /// Slug do projeto.
        slug: String,
    },
    /// Link de arquivo (`/files/<id>` ou `/download/<id>`).
    File {
        /// Tipo de projeto.
        class: ProjectClass,
        /// Slug do projeto.
        slug: String,
        /// ID do arquivo.
        file_id: u64,
    },
}

impl CurseforgeLink {
    fn class(&self) -> ProjectClass {
        match self {
            Self::Project { class, .. } | Self::File { class, .. } => *class,
        }
    }

    fn slug(&self) -> &str {
        match self {
            Self::Project { slug, .. } | Self::File { slug, .. } => slug,
        }
    }
}

/// Classe da CurseForge pelo trecho do caminho do site.
fn class_by_path(segment: &str) -> Option<ProjectClass> {
    Some(match segment {
        "mc-mods" => ProjectClass::Mod,
        "texture-packs" => ProjectClass::ResourcePack,
        "shaders" => ProjectClass::Shader,
        "modpacks" => ProjectClass::Modpack,
        "customization" => ProjectClass::Customization,
        "data-packs" => ProjectClass::DataPack,
        "worlds" => ProjectClass::World,
        "bukkit-plugins" => ProjectClass::BukkitPlugin,
        "mc-addons" => ProjectClass::Addon,
        _ => return None,
    })
}

/// Se o texto é um link de projeto ou de arquivo da CurseForge. `None`: não é (ou é `http://`,
/// recusado por quem cuida dos links, P1-11).
#[must_use]
pub fn parse_link(text: &str) -> Option<CurseforgeLink> {
    let rest = text.trim().strip_prefix("https://")?;
    let (host, path) = rest.split_once('/')?;
    let host = host.to_ascii_lowercase();
    if !matches!(
        host.as_str(),
        "www.curseforge.com" | "curseforge.com" | "legacy.curseforge.com"
    ) {
        return None;
    }
    let path = path.split(['?', '#']).next().unwrap_or_default();
    let mut segments = path.split('/').filter(|segment| !segment.is_empty());
    if segments.next()? != "minecraft" {
        return None;
    }
    let class = class_by_path(segments.next()?)?;
    let slug = segments.next()?.to_owned();
    match (segments.next(), segments.next(), segments.next()) {
        (Some("files" | "download"), Some(id), None) => Some(CurseforgeLink::File {
            class,
            slug,
            file_id: id.parse().ok()?,
        }),
        // Sem nada depois do slug, ou nas abas do projeto (`/files`, `/relations/dependencies`…).
        _ => Some(CurseforgeLink::Project { class, slug }),
    }
}

/// O que o link aponta, resolvido: a tela abre a pré-visualização (link de projeto) ou o
/// diálogo de dependências com o arquivo exato (link de arquivo).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct LinkTarget {
    /// ID do projeto na CurseForge.
    pub project_id: String,
    /// Nome.
    pub title: String,
    /// Tipo.
    pub kind: ProjectKind,
    /// ID do arquivo, quando o link era de arquivo.
    pub file_id: Option<String>,
}

impl LinkTarget {
    /// A escolha para o caminho normal (`add_plan`): o arquivo do link, ou a versão padrão.
    #[must_use]
    pub fn choice(&self) -> AddChoice {
        AddChoice {
            source: SourceId::Curseforge,
            project_id: self.project_id.clone(),
            version_id: self.file_id.clone(),
        }
    }
}

fn unsupported_type(link: &CurseforgeLink) -> Error {
    Error::new(
        Code::InvalidInput,
        format!(
            "o Warden só adiciona mods, resource packs e shaders da CurseForge ({})",
            link.slug()
        ),
    )
    .param("field", "link")
}

/// Acha o projeto do link na API (pelo slug e pela classe). Erros: [`Code::InvalidInput`] para
/// tipos que a página Adicionar não aceita; [`Code::ItemNotFound`] se o projeto não existe;
/// [`Code::SearchSourceUnavailable`] se a CurseForge não responder.
pub async fn resolve_project(
    client: &CurseforgeClient,
    link: &CurseforgeLink,
    cancel: &CancellationToken,
) -> crate::Result<LinkTarget> {
    let kind = kind_of(Some(link.class())).ok_or_else(|| unsupported_type(link))?;
    let query = SearchQuery::new("")
        .class(link.class())
        .slug(link.slug())
        .page(0, 5);
    let found = client
        .search(&query, Some(cancel))
        .await
        .map_err(|e| super::curseforge::unavailable(&e))?;
    let project = found
        .mods
        .into_iter()
        .find(|project| project.slug.eq_ignore_ascii_case(link.slug()))
        .ok_or_else(|| {
            Error::new(
                Code::ItemNotFound,
                format!("projeto {} não encontrado na CurseForge", link.slug()),
            )
            .param("path", link.slug())
        })?;
    let file_id = match link {
        CurseforgeLink::File { file_id, .. } => Some(file_id.to_string()),
        CurseforgeLink::Project { .. } => None,
    };
    Ok(LinkTarget {
        project_id: project.id.to_string(),
        title: project.name,
        kind,
        file_id,
    })
}

/// O que o packwiz escreveu na cópia ao ler o link de arquivo.
#[derive(Debug, Clone, PartialEq)]
pub struct ImportedLink {
    /// ID do projeto na CurseForge.
    pub project_id: String,
    /// ID do arquivo.
    pub file_id: String,
    /// Tipo, pela pasta onde o packwiz pôs o `.pw.toml`.
    pub kind: ProjectKind,
    /// O `.pw.toml` que o packwiz escreveu (na cópia; o pack de verdade não foi tocado).
    pub metafile: Metafile,
}

impl ImportedLink {
    /// O alvo do link para a tela.
    #[must_use]
    pub fn target(&self) -> LinkTarget {
        LinkTarget {
            project_id: self.project_id.clone(),
            title: self.metafile.name.clone(),
            kind: self.kind,
            file_id: Some(self.file_id.clone()),
        }
    }

    /// A escolha para o caminho normal (`add_plan`), com o arquivo exato do link.
    #[must_use]
    pub fn choice(&self) -> AddChoice {
        AddChoice {
            source: SourceId::Curseforge,
            project_id: self.project_id.clone(),
            version_id: Some(self.file_id.clone()),
        }
    }
}

/// Falha da importação de um link: do Warden ou do packwiz.
#[derive(Debug, thiserror::Error)]
pub enum LinkError {
    /// Erro do domínio de packs.
    #[error(transparent)]
    Project(#[from] Error),
    /// Erro do packwiz (sem chave, chave recusada, link que ele não entende…).
    #[error(transparent)]
    Packwiz(#[from] warden_packwiz_cli::Error),
}

/// Lê um link de **arquivo** pelo packwiz, numa cópia do pack em `staging_dir` (que não pode
/// ter conteúdo; é apagada ao fim). Nada é gravado no pack: devolve o projeto e o arquivo
/// exatos para seguirem pelo caminho normal.
///
/// Erros: os do packwiz ([`LinkError::Packwiz`]: chave ausente ou recusada, link inválido,
/// tempo esgotado) e [`Code::Internal`] se o packwiz não escreveu um `.pw.toml` da CurseForge.
pub async fn import_file_link(
    root: &Path,
    staging_dir: &Path,
    packwiz: &Packwiz,
    key: Option<&SecretString>,
    url: &str,
    cancel: &CancellationToken,
) -> Result<ImportedLink, LinkError> {
    let staging = Staging::copy_from(root, staging_dir)?;
    let report = packwiz
        .curseforge_add(
            staging.path(),
            &CurseForgeTarget::Url(url.to_owned()),
            key,
            RunContext::new(cancel),
        )
        .await?;
    imported(staging.path(), &report.metafiles).map_err(LinkError::Project)
}

/// O primeiro `.pw.toml` da CurseForge entre os que o packwiz criou.
fn imported(staging: &Path, metafiles: &[String]) -> crate::Result<ImportedLink> {
    let internal = |detail: String| Error::new(Code::Internal, detail);
    for relative in metafiles {
        let text = std::fs::read_to_string(staging.join(relative))
            .map_err(|e| internal(format!("{relative}: {e}")))?;
        let parsed = Metafile::parse(&text)
            .map_err(|e| internal(format!("{relative}: {e}")))?
            .value;
        let Some((project, file)) = parsed.curseforge_ids() else {
            continue;
        };
        let kind = [
            ProjectKind::Mod,
            ProjectKind::ResourcePack,
            ProjectKind::Shader,
        ]
        .into_iter()
        .find(|kind| relative.starts_with(&format!("{}/", kind.folder())))
        .ok_or_else(|| internal(format!("{relative}: pasta que o Warden não conhece")))?;
        return Ok(ImportedLink {
            project_id: project.to_string(),
            file_id: file.to_string(),
            kind,
            metafile: parsed,
        });
    }
    Err(internal(
        "o packwiz não criou um item da CurseForge para o link".to_owned(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn project(class: ProjectClass, slug: &str) -> CurseforgeLink {
        CurseforgeLink::Project {
            class,
            slug: slug.into(),
        }
    }

    #[test]
    fn reconhece_links_de_projeto() {
        let jei = "https://www.curseforge.com/minecraft/mc-mods/jei";
        assert_eq!(parse_link(jei), Some(project(ProjectClass::Mod, "jei")));
        // Barra final, espaços, consulta e as abas do projeto são o mesmo projeto.
        for text in [
            "https://www.curseforge.com/minecraft/mc-mods/jei/",
            "  https://www.curseforge.com/minecraft/mc-mods/jei  ",
            "https://curseforge.com/minecraft/mc-mods/jei?utm=1#x",
            "https://legacy.curseforge.com/minecraft/mc-mods/jei",
            "https://www.curseforge.com/minecraft/mc-mods/jei/files",
            "https://www.curseforge.com/minecraft/mc-mods/jei/relations/dependencies",
        ] {
            assert_eq!(
                parse_link(text),
                Some(project(ProjectClass::Mod, "jei")),
                "{text}"
            );
        }
        assert_eq!(
            parse_link("https://www.curseforge.com/minecraft/texture-packs/faithful-32x"),
            Some(project(ProjectClass::ResourcePack, "faithful-32x"))
        );
        assert_eq!(
            parse_link("https://www.curseforge.com/minecraft/shaders/complementary"),
            Some(project(ProjectClass::Shader, "complementary"))
        );
    }

    #[test]
    fn reconhece_links_de_arquivo() {
        let file = |class, slug: &str, file_id| {
            Some(CurseforgeLink::File {
                class,
                slug: slug.into(),
                file_id,
            })
        };
        assert_eq!(
            parse_link("https://www.curseforge.com/minecraft/mc-mods/jei/files/4712345"),
            file(ProjectClass::Mod, "jei", 4_712_345)
        );
        assert_eq!(
            parse_link("https://www.curseforge.com/minecraft/mc-mods/jei/download/4712345"),
            file(ProjectClass::Mod, "jei", 4_712_345)
        );
        // Arquivo que não é número não é link de arquivo.
        assert_eq!(
            parse_link("https://www.curseforge.com/minecraft/mc-mods/jei/files/latest"),
            None
        );
    }

    #[test]
    fn recusa_o_que_nao_e_link_da_curseforge() {
        for text in [
            "jei",
            "",
            "http://www.curseforge.com/minecraft/mc-mods/jei",
            "https://modrinth.com/mod/jei",
            "https://curseforge.com.evil.example/minecraft/mc-mods/jei",
            "https://www.curseforge.com/minecraft/mc-mods",
            "https://www.curseforge.com/minecraft/nada/jei",
            "https://www.curseforge.com/other-game/mc-mods/jei",
            "https://www.curseforge.com/",
        ] {
            assert_eq!(parse_link(text), None, "{text}");
        }
    }

    #[test]
    fn tipo_que_a_pagina_nao_aceita_e_recusado() {
        let link = parse_link("https://www.curseforge.com/minecraft/modpacks/create-live").unwrap();
        assert_eq!(kind_of(Some(link.class())), None);
        assert_eq!(unsupported_type(&link).code, Code::InvalidInput);
    }
}
