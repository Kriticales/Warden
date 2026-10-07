//! Formatos de outros launchers (E-02): `.mrpack` e zip da CurseForge, gerados pelo
//! `packwiz modrinth export` / `packwiz curseforge export` sobre a cópia limpa de staging e
//! conferidos depois de gerados (ARCHITECTURE §12; SPEC T19).
//!
//! - [`analyze_format`]: lê o pack e diz o que o formato perde, o que iria embutido (pede
//!   confirmação de licença), o que pode ser trocado pelo Modrinth (mesmo hash) e o que
//!   impede (mod da CurseForge bloqueado para terceiros).
//! - [`export_format`]: gera o arquivo com as escolhas da pessoa, valida e só então grava no
//!   destino.
//! - [`validate_mrpack`], [`validate_curseforge`] e [`read_mrpack`]: o validador reaproveitado
//!   pela E-04 e pela importação (P1-19).

mod model;
mod plan;
mod validate;

use std::collections::BTreeSet;
use std::fs;
use std::io::Write as _;
use std::path::Path;

use secrecy::SecretString;
use warden_core::{CancellationToken, DomainError as _};
use warden_packwiz::{HashFormat, Metafile, ModrinthFile};
use warden_packwiz_cli::{ExportSide, Packwiz, RunContext};

pub use model::{
    FormatAnalysis, FormatChoices, FormatExportResult, FormatItem, FormatLoss, FormatValidation,
    ItemOrigin, ItemOutcome, LauncherFormat, LossKind,
};
pub use plan::Lookups;
pub use validate::{
    CURSEFORGE_MANIFEST, CurseforgeRules, MRPACK_INDEX, MRPACK_OVERRIDE_DIRS, MrpackArchive,
    MrpackEnv, MrpackFile, MrpackIndex, MrpackRules, read_mrpack, validate_curseforge,
    validate_mrpack,
};

use crate::native::{
    ExportSource, SourceReader, check_destination, err, indexed_files, prepare_stage,
};
use crate::{Error, ExportErrorCode, Result};

/// Tamanho máximo do texto da versão digitada.
const MAX_VERSION_CHARS: usize = 64;

/// Lê o pack (sem escrever em lugar nenhum) e devolve o que o formato perde, o que pede
/// decisão e o que impede de gerar.
pub async fn analyze_format(
    root: &Path,
    source: &ExportSource,
    format: LauncherFormat,
    lookups: &Lookups<'_>,
    cancel: &CancellationToken,
) -> Result<FormatAnalysis> {
    let reader = SourceReader::open(root, source)?;
    let files = indexed_files(&reader)?;
    Ok(plan::plan(&files, format, lookups, cancel).await?.analysis)
}

/// Tudo o que [`export_format`] precisa.
pub struct FormatRequest<'a> {
    /// Pasta do pack.
    pub root: &'a Path,
    /// Estado atual ou versão salva.
    pub source: &'a ExportSource,
    /// O formato.
    pub format: LauncherFormat,
    /// As escolhas da pessoa.
    pub choices: &'a FormatChoices,
    /// O arquivo a criar (não pode existir).
    pub destination: &'a Path,
    /// Pasta das cópias temporárias (`cache/staging`).
    pub staging_parent: &'a Path,
    /// O sidecar.
    pub packwiz: &'a Packwiz,
    /// A chave da CurseForge, se houver; só vai ao packwiz quando o comando precisa dela.
    pub key: Option<&'a SecretString>,
    /// Quem responde às conferências.
    pub lookups: Lookups<'a>,
}

fn map_packwiz(error: &warden_packwiz_cli::Error) -> Error {
    use warden_packwiz_cli::Error as Cli;
    match error {
        Cli::CurseForgeKeyMissing { .. } => Error::new(
            ExportErrorCode::CurseforgeKeyMissing,
            "o packwiz precisa da chave da CurseForge",
        ),
        Cli::ManualDownloads { files, .. } => Error::new(
            ExportErrorCode::BlockedByAuthor,
            "mods da CurseForge só podem ser baixados à mão",
        )
        .with_param(
            "files",
            files
                .iter()
                .map(|file| file.name.as_str())
                .collect::<Vec<_>>()
                .join(", "),
        )
        .with_detail(error.detail().unwrap_or_default()),
        Cli::Cancelled { .. } => Error::internal("exportação cancelada"),
        other => Error::new(
            ExportErrorCode::FormatToolFailed,
            "o packwiz não conseguiu gerar o arquivo",
        )
        .with_param("command", other.command().unwrap_or("export"))
        .with_detail(other.detail().unwrap_or_default()),
    }
}

fn check_version(text: &str) -> Result<String> {
    let version = text.trim();
    if version.is_empty()
        || version.chars().count() > MAX_VERSION_CHARS
        || version.chars().any(char::is_control)
    {
        return Err(Error::new(
            ExportErrorCode::VersionRequired,
            "informe uma versão de até 64 caracteres, sem caracteres de controle",
        ));
    }
    Ok(version.to_owned())
}

fn is_cancelled(cancel: &CancellationToken) -> Result<()> {
    if cancel.is_cancelled() {
        return Err(Error::internal("exportação cancelada"));
    }
    Ok(())
}

/// Gera o `.mrpack` ou o zip da CurseForge, confere o arquivo e só então o grava em
/// `destination`. O pack no Warden não muda: trocas de fonte e a `version` informada valem só
/// para a cópia de staging.
#[allow(clippy::too_many_lines)]
pub async fn export_format(
    request: &FormatRequest<'_>,
    cancel: &CancellationToken,
) -> Result<FormatExportResult> {
    let FormatRequest {
        root,
        source,
        format,
        choices,
        destination,
        staging_parent,
        packwiz,
        key,
        lookups,
    } = request;
    check_destination(root, destination, false)?;
    let staged = prepare_stage(root, source, staging_parent, packwiz, cancel).await?;
    let plan = plan::plan(&staged.files, *format, lookups, cancel).await?;

    let version = match &plan.analysis.version {
        Some(version) => version.clone(),
        None => check_version(choices.version.as_deref().unwrap_or_default())?,
    };

    let swap_set: BTreeSet<&str> = choices.swap.iter().map(String::as_str).collect();
    if let Some(path) = swap_set
        .iter()
        .find(|path| !plan.swaps.contains_key(**path))
    {
        return Err(Error::new(
            ExportErrorCode::SwapNotAvailable,
            format!("{path} não tem um equivalente no Modrinth"),
        )
        .with_param("file", *path));
    }
    let mut embeds = plan.embeds.clone();
    let mut blocked = Vec::new();
    for item in &plan.analysis.items {
        match &item.outcome {
            ItemOutcome::Blocked { .. } | ItemOutcome::Unavailable => {
                blocked.push(item.name.clone());
            }
            ItemOutcome::Swap { .. } if !swap_set.contains(item.path.as_str()) => {
                match plan.swap_kept.get(&item.path) {
                    Some(plan::KeptOutcome::Embed) => {
                        if let Some(m) = plan.contents.mods.iter().find(|m| m.path == item.path) {
                            embeds.push(plan::Embed {
                                path: m.path.clone(),
                                dest: m.dest.clone(),
                            });
                        }
                    }
                    Some(plan::KeptOutcome::NeedsKey) => {
                        return Err(Error::new(
                            ExportErrorCode::CurseforgeKeyMissing,
                            "sem a troca pelo Modrinth, embutir o mod exige a chave da CurseForge",
                        ));
                    }
                    _ => blocked.push(item.name.clone()),
                }
            }
            _ => {}
        }
    }
    if !blocked.is_empty() {
        return Err(Error::new(
            ExportErrorCode::BlockedByAuthor,
            "há mods da CurseForge que o formato não pode levar",
        )
        .with_param("files", blocked.join(", ")));
    }
    if !embeds.is_empty() && !choices.confirm_embed {
        return Err(Error::new(
            ExportErrorCode::EmbedNeedsConfirmation,
            "arquivos de terceiros iriam dentro do arquivo gerado",
        )
        .with_param("count", embeds.len().to_string()));
    }

    let stage = staged.dir.path();
    for path in &swap_set {
        let (Some(target), Some(old)) = (
            plan.swaps.get(*path),
            plan.contents.mods.iter().find(|m| m.path == *path),
        ) else {
            continue;
        };
        let mut metafile = Metafile::modrinth(
            &ModrinthFile {
                title: target.title.clone(),
                project_id: target.project_id.clone(),
                version_id: target.version_id.clone(),
                filename: target.filename.clone(),
                url: target.url.clone(),
                hash_format: HashFormat::Sha512,
                hash: target.sha512.clone(),
            },
            old.meta.side.clone(),
        );
        metafile.pin = old.meta.pin;
        metafile.option.clone_from(&old.meta.option);
        fs::write(stage.join(path), metafile.to_toml_string()).map_err(err)?;
    }
    if plan.analysis.version.is_none() {
        let mut manifest = plan.contents.manifest.clone();
        manifest.version.clone_from(&version);
        fs::write(stage.join("pack.toml"), manifest.to_toml_string()).map_err(err)?;
    }
    is_cancelled(cancel)?;

    fs::create_dir_all(staging_parent).map_err(err)?;
    let out_dir = tempfile::Builder::new()
        .prefix("export-out-")
        .tempdir_in(staging_parent)
        .map_err(err)?;
    let output = out_dir.path().join(format!("pack.{}", format.extension()));
    let needs_key = *format == LauncherFormat::Mrpack
        && embeds.iter().any(|embed| {
            plan.contents
                .mods
                .iter()
                .any(|m| m.path == embed.path && m.meta.curseforge_ids().is_some())
        });
    let key = key.filter(|_| needs_key);
    let context = RunContext::new(cancel);
    match format {
        LauncherFormat::Mrpack => packwiz
            .modrinth_export(stage, &output, key, context)
            .await
            .map(|_| ()),
        // `client`: o jogador do app da CurseForge é cliente; mods só de servidor ficam de fora
        // e a tela avisa.
        LauncherFormat::Curseforge => packwiz
            .curseforge_export(stage, ExportSide::Client, &output, key, context)
            .await
            .map(|_| ()),
    }
    .map_err(|error| map_packwiz(&error))?;
    is_cancelled(cancel)?;

    let allowed_overrides = plan::allowed_overrides(&plan, &embeds);
    let allowed_jars: BTreeSet<String> = allowed_overrides
        .iter()
        .filter(|path| path.to_ascii_lowercase().ends_with(".jar"))
        .cloned()
        .collect();
    let validation = match format {
        LauncherFormat::Mrpack => validate_mrpack(
            &output,
            &MrpackRules {
                strict: true,
                allowed_overrides: Some(allowed_overrides),
                allowed_jars: Some(allowed_jars),
            },
        )?,
        LauncherFormat::Curseforge => validate_curseforge(
            &output,
            &CurseforgeRules {
                allowed_overrides: Some(allowed_overrides),
                allowed_jars: Some(allowed_jars),
            },
        )?,
    };
    let bytes = copy_new(&output, destination)?;
    Ok(FormatExportResult {
        path: destination.to_path_buf(),
        bytes,
        swapped: swap_set.len(),
        validation,
    })
}

/// Copia `from` para `to`, que não pode existir; se a cópia falhar, não deixa pedaço.
fn copy_new(from: &Path, to: &Path) -> Result<u64> {
    let mut source = fs::File::open(from).map_err(err)?;
    let mut target = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(to)
        .map_err(err)?;
    let result = std::io::copy(&mut source, &mut target)
        .and_then(|bytes| target.flush().map(|()| bytes))
        .map_err(err);
    if result.is_err() {
        drop(target);
        let _ = fs::remove_file(to);
    }
    result
}
