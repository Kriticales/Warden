//! "Importar como pack novo": grava o plano revisado como um pack novo, pela mesma rotina do
//! T03 (arquivos de controle, `.warden/`, repositório git com o ponto inicial "Pack
//! importado"), seguida de `packwiz refresh` e `check_conformance` (ARCHITECTURE §12.2).
//!
//! Tudo é montado numa pasta irmã temporária (`.warden-creating-<id>`) e só vira o pack no fim,
//! com uma troca de nome: cancelar ou falhar no meio apaga a pasta temporária e não deixa pack
//! pela metade nem registro. A origem nunca é alterada.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;
use warden_core::{CancellationToken, PackId, Progress, ProgressSink};
use warden_packwiz::PackIndex;
use warden_packwiz::hygiene::{GITATTRIBUTES_TEMPLATE, gitignore_template, packwizignore_template};
use warden_packwiz_cli::{Packwiz, RunContext, check_conformance};
use warden_project::create::project_toml;
use warden_project::registry::{PackRecord, Registry};
use warden_versioning::{Identity, InitialPoint, Moment, PackRepo};

use crate::archive::Source;
use crate::build::{read_content, read_detached};
use crate::common::redact;
use crate::error::{Error, ImportErrorCode as Code, Result};
use crate::model::{Content, ImportPlan, LocalJar};

/// O que a pessoa confirmou na tela.
#[derive(Debug, Clone)]
pub struct ImportRequest {
    /// Nome do pack novo (1 a 64 caracteres; já conferido).
    pub name: String,
    /// Pasta final, absoluta, inexistente ou vazia (já conferida por
    /// `warden_project::create::resolve_destination`).
    pub destination: PathBuf,
    /// Itens de "não entram" que a pessoa marcou para entrar (só os que permitem).
    pub include: Vec<String>,
    /// "Levar suas preferências para o pack?" (`options.txt`, `servers.dat`).
    pub take_preferences: bool,
    /// Pasta para a cópia da conferência (`cache/staging`).
    pub scratch: PathBuf,
}

/// O pack criado.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportedPack {
    /// Identificador.
    pub id: PackId,
    /// Pasta.
    pub path: PathBuf,
}

#[derive(Serialize)]
struct ReviewRecord<'a> {
    path: &'a str,
    name: &'a str,
    reason: &'a str,
}

#[derive(Serialize)]
struct ImportRecord<'a> {
    source: &'a str,
    file: &'a str,
    #[serde(rename = "local-files", skip_serializing_if = "<[_]>::is_empty")]
    local_files: &'a [LocalJar],
    #[serde(skip_serializing_if = "Vec::is_empty")]
    review: Vec<ReviewRecord<'a>>,
}

#[derive(Serialize)]
struct ImportTable<'a> {
    import: ImportRecord<'a>,
}

/// `.warden/project.toml` do pack importado: o documento mínimo do T03 mais a tabela
/// `[import]` com a origem, os jars que ficaram como arquivo local (gancho 1.2 da ARCHITECTURE
/// §21.7, usado pela W-02) e os itens para revisar.
pub(crate) fn project_document(id: PackId, plan: &ImportPlan) -> Result<String> {
    let review = plan
        .summary
        .review
        .iter()
        .map(|item| ReviewRecord {
            path: &item.path,
            name: &item.name,
            reason: match item.reason {
                crate::model::ImportReviewReason::UnknownSide => "unknownSide",
                crate::model::ImportReviewReason::Optional => "optional",
                crate::model::ImportReviewReason::OptionalDisabled => "optionalDisabled",
                crate::model::ImportReviewReason::ClientOverride => "clientOverride",
            },
        })
        .collect();
    let table = ImportTable {
        import: ImportRecord {
            source: plan.summary.kind.key(),
            file: &plan.summary.file_name,
            local_files: &plan.local_jars,
            review,
        },
    };
    let extra = toml::to_string(&table)
        .map_err(|error| Error::internal(format!(".warden/project.toml: {error}")))?;
    Ok(format!("{}\n{extra}", project_toml(id)))
}

fn io(action: &str, path: &str, error: &std::io::Error) -> Error {
    Error::internal(format!("{action} {path}: {}", error.kind()))
}

fn cancelled() -> Error {
    Error::internal("cancelado")
}

fn write_file(stage: &Path, relative: &str, bytes: &[u8]) -> Result<()> {
    let path = warden_core::resolve_inside(stage, relative)
        .map_err(|error| Error::internal(format!("{relative}: {error}")))?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| io("criar a pasta de", relative, &error))?;
    }
    fs::write(&path, bytes).map_err(|error| io("gravar", relative, &error))
}

fn source_unchanged(plan: &ImportPlan) -> Result<()> {
    let Some((size, modified)) = plan.origin_stamp else {
        return Ok(());
    };
    let now = fs::metadata(&plan.origin).ok();
    let same = now.is_some_and(|m| m.len() == size && m.modified().ok() == modified);
    if same {
        Ok(())
    } else {
        Err(Error::new(
            Code::SourceChanged,
            format!("{} mudou depois da leitura", plan.summary.file_name),
        )
        .with_param("name", &plan.summary.file_name))
    }
}

/// Grava o pack importado e o registra.
///
/// Erros: [`Code::SourceChanged`], [`Code::Internal`] (gravação, `packwiz refresh`,
/// conferência, git ou registro; nada fica gravado), cancelamento como `Internal` com o token
/// cancelado.
#[allow(clippy::too_many_lines)]
pub async fn apply(
    plan: &ImportPlan,
    request: &ImportRequest,
    registry: &Registry,
    packwiz: &Packwiz,
    progress: &dyn ProgressSink,
    cancel: &CancellationToken,
) -> Result<ImportedPack> {
    source_unchanged(plan)?;
    let path = request.destination.clone();
    let parent = path
        .parent()
        .ok_or_else(|| Error::internal("destino sem pasta pai"))?
        .to_path_buf();
    fs::create_dir_all(&parent).map_err(|error| io("criar", "a pasta do pack", &error))?;
    let id = PackId::new();
    let stage = parent.join(format!(".warden-creating-{id}"));
    fs::create_dir(&stage).map_err(|error| io("criar", "a pasta temporária", &error))?;

    let result = async {
        // O conteúdo: arquivos do plano, os itens marcados e, se a pessoa quis, as preferências.
        let mut files: BTreeMap<&str, &Content> = plan
            .files
            .iter()
            .map(|(path, content)| (path.as_str(), content))
            .collect();
        for item in &request.include {
            if let Some(list) = plan.includable.get(item) {
                for (path, content) in list {
                    files.insert(path.as_str(), content);
                }
            }
        }
        if request.take_preferences {
            for (path, content) in &plan.preferences {
                files.insert(path.as_str(), content);
            }
        }
        let mut source: Option<Source> = None;
        let total = files.len() as u64;
        for (done, (relative, content)) in files.into_iter().enumerate() {
            if cancel.is_cancelled() {
                return Err(cancelled());
            }
            let bytes = if let Some(bytes) = read_detached(content) {
                bytes?
            } else {
                if source.is_none() {
                    let opened = if plan.origin.is_dir() {
                        Source::open_dir(&plan.origin)?
                    } else {
                        Source::open_zip(&plan.origin)?
                    };
                    source = Some(opened.within(&plan.root));
                }
                let Some(source) = source.as_mut() else {
                    return Err(Error::internal("origem não aberta"));
                };
                read_content(source, content)?
            };
            write_file(&stage, relative, &bytes)?;
            progress.progress(Progress::items(done as u64 + 1, Some(total)));
        }

        let mut manifest = plan.manifest.clone();
        request.name.trim().clone_into(&mut manifest.name);
        write_file(&stage, "pack.toml", manifest.to_toml_string().as_bytes())?;
        write_file(
            &stage,
            "index.toml",
            PackIndex::default().to_toml_string().as_bytes(),
        )?;
        write_file(&stage, ".gitattributes", GITATTRIBUTES_TEMPLATE.as_bytes())?;
        write_file(
            &stage,
            ".packwizignore",
            packwizignore_template().as_bytes(),
        )?;
        write_file(&stage, ".gitignore", gitignore_template().as_bytes())?;
        write_file(
            &stage,
            ".warden/project.toml",
            project_document(id, plan)?.as_bytes(),
        )?;
        write_file(
            &stage,
            "CHANGELOG.md",
            "# Histórico de versões\n".as_bytes(),
        )?;
        if cancel.is_cancelled() {
            return Err(cancelled());
        }

        packwiz
            .refresh(&stage, false, RunContext::new(cancel))
            .await
            .map_err(|error| {
                if cancel.is_cancelled() {
                    cancelled()
                } else {
                    Error::internal("o packwiz refresh falhou no pack importado")
                        .with_detail(redact(&error.to_string(), &[&stage, &parent]))
                }
            })?;
        let report = check_conformance(
            packwiz,
            &stage,
            &request.scratch.join(format!("conformance-{id}")),
            RunContext::new(cancel),
        )
        .await
        .map_err(|error| {
            if cancel.is_cancelled() {
                cancelled()
            } else {
                Error::internal("a conferência do pack importado falhou")
                    .with_detail(redact(&error.to_string(), &[&stage, &request.scratch]))
            }
        })?;
        if !report.is_conforming() {
            return Err(
                Error::internal("o pack importado não passou na conferência do packwiz")
                    .with_detail(format!("{:?}", report.differences)),
            );
        }
        if cancel.is_cancelled() {
            return Err(cancelled());
        }
        PackRepo::init(
            &stage,
            InitialPoint::Imported,
            &Identity::for_pack(&manifest.author, None),
            Moment::now_utc(),
        )
        .map_err(|error| {
            Error::internal(format!(
                "histórico do pack: {}",
                redact(&error.to_string(), &[&stage, &parent])
            ))
        })?;
        let existed = path.is_dir();
        if existed {
            fs::remove_dir(&path).map_err(|error| io("trocar", "a pasta do pack", &error))?;
        }
        if let Err(error) = fs::rename(&stage, &path) {
            if existed {
                let _ = fs::create_dir(&path);
            }
            return Err(io("mover", "o pack para a pasta final", &error));
        }
        let record = PackRecord {
            id,
            name: request.name.trim().to_owned(),
            path: path.clone(),
            last_test: None,
            extra: BTreeMap::new(),
        };
        if let Err(error) = registry.insert(record) {
            let _ = fs::rename(&path, &stage);
            if existed {
                let _ = fs::create_dir(&path);
            }
            return Err(Error::internal(format!(
                "registro do pack: {}",
                redact(&error.to_string(), &[&path, &parent])
            )));
        }
        Ok(ImportedPack {
            id,
            path: path.clone(),
        })
    }
    .await;
    if result.is_err() {
        let _ = fs::remove_dir_all(&stage);
    }
    result
}
