//! Exportação nativa: a mesma lista do índice alimenta prévia, pasta e zip.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use warden_core::{CancellationToken, resolve_inside};
use warden_packwiz::{PACK_FILE, PackIndex, PackManifest, check_relative_path};
use warden_packwiz_cli::{Packwiz, RunContext, snapshot};
use warden_project::hygiene::{self, HygieneCause, HygieneFinding};
use warden_project::transaction::PackTransaction;
use warden_versioning::{PackRepo, Snapshot};
use zip::write::SimpleFileOptions;

use crate::{Error, Result};

/// Origem da exportação. Uma versão salva nunca lê a árvore de trabalho.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ExportSource {
    /// Estado atual do pack.
    Current,
    /// Tag `v<version>` validada por `warden-versioning`.
    Saved {
        /// Número `SemVer` sem o prefixo `v`.
        version: String,
    },
}

/// Saída nativa.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum ExportFormat {
    /// Diretório com a árvore do pack.
    Folder,
    /// Arquivo ZIP com a mesma árvore.
    Zip,
}

/// Motivo de um alerta da prévia; a interface escolhe o texto exibido.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ExportAlert {
    /// Arquivo distribuído com mais de 20 MiB.
    LargeFile,
    /// `options.txt` sem `preserve` pode sobrescrever as preferências do jogador.
    OptionsOverridesPreferences,
    /// Arquivo distribuído diretamente na raiz do pack.
    LooseRootFile,
    /// Motivo identificado pela varredura de higiene do pack.
    Hygiene {
        /// Causa original da higiene.
        cause: HygieneCause,
    },
}

/// Um arquivo distribuído, em ordem de caminho.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct PreviewFile {
    /// Caminho relativo, com `/`.
    pub path: String,
    /// Bytes reais da origem.
    #[specta(type = specta_typescript::Number)]
    pub bytes: u64,
    /// Referência `.pw.toml`, sem o jar do mod.
    pub reference: bool,
    /// Arquivo distribuído em bytes, sem ser referência nem controle.
    pub local: bool,
    /// Arquivo de `config/` ou `defaultconfigs/`.
    pub config: bool,
    /// Alertas deste arquivo.
    pub alerts: Vec<ExportAlert>,
}

/// Contagem e tamanho por pasta, incluindo a raiz (`.`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct PreviewFolder {
    /// Caminho relativo da pasta.
    pub path: String,
    /// Arquivos nesta pasta e nas subpastas.
    #[specta(type = specta_typescript::Number)]
    pub files: usize,
    /// Soma dos tamanhos nesta pasta e nas subpastas.
    #[specta(type = specta_typescript::Number)]
    pub bytes: u64,
}

/// Avisos anteriores à exportação.
#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct Preflight {
    /// Arquivos suspeitos encontrados na pasta, inclusive fora do índice.
    pub hygiene: Vec<HygieneFinding>,
    /// O estado atual diverge da última versão salva.
    pub unsaved_changes: bool,
    /// A contagem de erros de diagnóstico fica ausente até a API de D-01 existir.
    pub diagnostic_errors: Option<u32>,
}

/// Prévia lida dos mesmos bytes que a exportação vai considerar.
#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ExportPreview {
    /// Arquivos exatos da saída (inclusive manifesto e índice).
    pub files: Vec<PreviewFile>,
    /// Pastas da árvore, com seus totais.
    pub folders: Vec<PreviewFolder>,
    /// Total de bytes dos arquivos da lista.
    #[specta(type = specta_typescript::Number)]
    pub bytes: u64,
    /// Número de referências a mods/resource packs/shaders.
    #[specta(type = specta_typescript::Number)]
    pub references: usize,
    /// Conferências e avisos.
    pub preflight: Preflight,
}

/// Resultado persistido.
#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ExportResult {
    /// Pasta ou arquivo escolhido no diálogo nativo.
    pub path: PathBuf,
    /// Arquivos distribuídos, em ordem.
    pub files: Vec<String>,
    /// Bytes da saída final (zip comprimido ou soma dos arquivos).
    #[specta(type = specta_typescript::Number)]
    pub bytes: u64,
}

enum SourceReader {
    Current(PathBuf),
    Saved { repo: PackRepo, snapshot: Snapshot },
}

impl SourceReader {
    fn open(root: &Path, source: &ExportSource) -> Result<Self> {
        match source {
            ExportSource::Current => Ok(Self::Current(root.to_path_buf())),
            ExportSource::Saved { version } => {
                let repo = PackRepo::open(root).map_err(err)?;
                let snapshot = Snapshot::Version(version.clone());
                repo.files_at(&snapshot).map_err(err)?;
                Ok(Self::Saved { repo, snapshot })
            }
        }
    }

    fn read(&self, relative: &str) -> Result<Vec<u8>> {
        check_relative_path(relative).map_err(err)?;
        match self {
            Self::Current(root) => {
                let path = resolve_inside(root, relative).map_err(err)?;
                let mut component = root.clone();
                for segment in relative.split('/') {
                    component.push(segment);
                    if fs::symlink_metadata(&component)
                        .map_err(err)?
                        .file_type()
                        .is_symlink()
                    {
                        return Err(Error(format!("link simbólico no pack: {relative}")));
                    }
                }
                let metadata = fs::symlink_metadata(&path).map_err(err)?;
                if !metadata.is_file() || metadata.file_type().is_symlink() {
                    return Err(Error(format!("arquivo não é regular: {relative}")));
                }
                fs::read(path).map_err(err)
            }
            Self::Saved { repo, snapshot } => repo
                .read_file_at(snapshot, relative)
                .map_err(err)?
                .ok_or_else(|| Error(format!("arquivo ausente na versão: {relative}"))),
        }
    }
}

fn err(error: impl std::fmt::Display) -> Error {
    Error(error.to_string())
}

fn indexed_files(reader: &SourceReader) -> Result<BTreeMap<String, Vec<u8>>> {
    let manifest_bytes = reader.read(PACK_FILE)?;
    let manifest = PackManifest::parse(std::str::from_utf8(&manifest_bytes).map_err(err)?)
        .map_err(err)?
        .value;
    // A tela e o contrato de publicação usam os dois nomes canônicos.
    if manifest.index.file != "index.toml" {
        return Err(Error("o índice do pack precisa ser index.toml".into()));
    }
    let index_bytes = reader.read("index.toml")?;
    let index = PackIndex::parse(std::str::from_utf8(&index_bytes).map_err(err)?)
        .map_err(err)?
        .value;
    let mut files = BTreeMap::from([
        (PACK_FILE.to_owned(), manifest_bytes),
        ("index.toml".to_owned(), index_bytes),
    ]);
    for entry in index.normalized_entries() {
        check_relative_path(&entry.file).map_err(err)?;
        if entry.file.contains('\\') {
            return Err(Error(format!(
                "separador inválido no índice: {}",
                entry.file
            )));
        }
        if entry.file == "CHANGELOG.md"
            || entry.file == ".warden"
            || entry.file.starts_with(".warden/")
            || entry.file == ".git"
            || entry.file.starts_with(".git/")
        {
            return Err(Error(format!("arquivo privado no índice: {}", entry.file)));
        }
        if matches!(entry.file.as_str(), PACK_FILE | "index.toml") {
            return Err(Error(format!(
                "arquivo de controle repetido no índice: {}",
                entry.file
            )));
        }
        files
            .entry(entry.file.clone())
            .or_insert(reader.read(&entry.file)?);
    }
    Ok(files)
}

/// Lê a prévia sem escrever no pack nem no cache.
pub fn preview(root: &Path, source: &ExportSource) -> Result<ExportPreview> {
    let reader = SourceReader::open(root, source)?;
    let files = indexed_files(&reader)?;
    let index = PackIndex::parse(std::str::from_utf8(&files["index.toml"]).map_err(err)?)
        .map_err(err)?
        .value;
    let (hygiene, unsaved_changes) = match source {
        ExportSource::Current => {
            let hygiene = hygiene::scan(root).map_err(err)?;
            let unsaved = PackRepo::open(root)
                .and_then(|repo| repo.unsaved_changes())
                .map_or(true, |changes| !changes.is_empty());
            (hygiene, unsaved)
        }
        ExportSource::Saved { .. } => (Vec::new(), false),
    };
    let mut total = 0_u64;
    let mut references = 0;
    let mut rows = Vec::with_capacity(files.len());
    let mut folders = BTreeMap::<String, (usize, u64)>::new();
    for (path, data) in &files {
        let bytes = u64::try_from(data.len()).map_err(err)?;
        total = total.saturating_add(bytes);
        let reference = path.ends_with(".pw.toml");
        let local = !reference && !matches!(path.as_str(), PACK_FILE | "index.toml");
        references += usize::from(reference);
        let config = path.starts_with("config/") || path.starts_with("defaultconfigs/");
        let mut alerts = Vec::new();
        if bytes > 20 * 1024 * 1024 {
            alerts.push(ExportAlert::LargeFile);
        }
        if path == "options.txt" && !index.entry(path).is_some_and(|entry| entry.preserve) {
            alerts.push(ExportAlert::OptionsOverridesPreferences);
        }
        if !matches!(path.as_str(), PACK_FILE | "index.toml") && !path.contains('/') {
            alerts.push(ExportAlert::LooseRootFile);
        }
        for finding in &hygiene {
            if finding.path == *path
                || (finding.is_dir && path.starts_with(&format!("{}/", finding.path)))
            {
                alerts.extend(
                    finding
                        .reasons
                        .iter()
                        .cloned()
                        .map(|cause| ExportAlert::Hygiene { cause }),
                );
            }
        }
        let root_total = folders.entry(".".into()).or_default();
        root_total.0 += 1;
        root_total.1 = root_total.1.saturating_add(bytes);
        let mut folder = String::new();
        for segment in path
            .split('/')
            .take(path.split('/').count().saturating_sub(1))
        {
            if !folder.is_empty() {
                folder.push('/');
            }
            folder.push_str(segment);
            let sum = folders.entry(folder.clone()).or_default();
            sum.0 += 1;
            sum.1 = sum.1.saturating_add(bytes);
        }
        rows.push(PreviewFile {
            path: path.clone(),
            bytes,
            reference,
            local,
            config,
            alerts,
        });
    }
    Ok(ExportPreview {
        files: rows,
        folders: folders
            .into_iter()
            .map(|(path, (files, bytes))| PreviewFolder { path, files, bytes })
            .collect(),
        bytes: total,
        references,
        preflight: Preflight {
            hygiene,
            unsaved_changes,
            diagnostic_errors: None,
        },
    })
}

/// Exclui um arquivo indexado do pack: grava a regra e atualiza o índice transacionalmente.
pub async fn exclude_from_pack(
    root: &Path,
    relative: &str,
    packwiz: &Packwiz,
    cancel: &CancellationToken,
) -> Result<()> {
    check_relative_path(relative).map_err(err)?;
    if relative.contains(['[', ']']) {
        return Err(Error(
            "nome com colchetes exige exclusão manual no .packwizignore".into(),
        ));
    }
    if matches!(relative, PACK_FILE | "index.toml") {
        return Err(Error(
            "não é possível excluir pack.toml ou index.toml".into(),
        ));
    }
    let reader = SourceReader::open(root, &ExportSource::Current)?;
    let files = indexed_files(&reader)?;
    let is_file = files.contains_key(relative);
    let is_folder = files
        .keys()
        .any(|path| path.starts_with(&format!("{relative}/")));
    if !is_file && !is_folder {
        return Err(Error(format!("caminho fora do índice: {relative}")));
    }
    let path = root.join(".packwizignore");
    let mut ignore = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(error) => return Err(err(error)),
    };
    if !ignore.is_empty() && !ignore.ends_with('\n') {
        ignore.push('\n');
    }
    ignore.push('/');
    ignore.push_str(relative);
    if is_folder {
        ignore.push('/');
    }
    ignore.push('\n');
    let mut tx = PackTransaction::new(root.to_path_buf());
    tx.write(".packwizignore", ignore.into_bytes())
        .map_err(err)?;
    tx.commit(packwiz, cancel).await.map_err(err)?;
    Ok(())
}

/// Gera pasta ou zip determinístico. `destination` deve estar vazio/inexistente.
pub async fn export(
    root: &Path,
    source: &ExportSource,
    format: ExportFormat,
    destination: &Path,
    staging_parent: &Path,
    packwiz: &Packwiz,
    cancel: &CancellationToken,
) -> Result<ExportResult> {
    if destination.exists()
        && fs::symlink_metadata(destination)
            .map_err(err)?
            .file_type()
            .is_symlink()
    {
        return Err(Error("o destino não pode ser um link simbólico".into()));
    }
    if destination.exists()
        && (format != ExportFormat::Folder
            || !destination.is_dir()
            || fs::read_dir(destination).map_err(err)?.next().is_some())
    {
        return Err(Error(format!(
            "o destino já existe: {}",
            destination.display()
        )));
    }
    let root_real = root.canonicalize().map_err(err)?;
    let parent_real = destination
        .parent()
        .ok_or_else(|| Error("destino sem pasta pai".into()))?
        .canonicalize()
        .map_err(err)?;
    if parent_real.starts_with(root_real) {
        return Err(Error(
            "o destino precisa ficar fora da pasta do pack".into(),
        ));
    }
    fs::create_dir_all(staging_parent).map_err(err)?;
    let stage = tempfile::Builder::new()
        .prefix("export-")
        .tempdir_in(staging_parent)
        .map_err(err)?;
    let reader = SourceReader::open(root, source)?;
    let original = indexed_files(&reader)?;
    for (path, bytes) in &original {
        if cancel.is_cancelled() {
            return Err(Error("exportação cancelada".into()));
        }
        let target = resolve_inside(stage.path(), path).map_err(err)?;
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(err)?;
        }
        fs::write(target, bytes).map_err(err)?;
    }
    packwiz
        .refresh(stage.path(), true, RunContext::new(cancel))
        .await
        .map_err(err)?;
    let refreshed = indexed_files(&SourceReader::Current(stage.path().to_path_buf()))?;
    verify_refresh(&original, &refreshed)?;
    let before = snapshot(stage.path(), |_| true).map_err(err)?;
    packwiz
        .refresh(stage.path(), false, RunContext::new(cancel))
        .await
        .map_err(err)?;
    let after = snapshot(stage.path(), |_| true).map_err(err)?;
    if before != after {
        return Err(Error("a saída não é estável após packwiz refresh".into()));
    }
    if cancel.is_cancelled() {
        return Err(Error("exportação cancelada".into()));
    }
    let files = refreshed.keys().cloned().collect::<Vec<_>>();
    let bytes = match format {
        ExportFormat::Folder => write_folder(destination, &refreshed, cancel)?,
        ExportFormat::Zip => write_zip(destination, &refreshed, cancel)?,
    };
    Ok(ExportResult {
        path: destination.to_path_buf(),
        files,
        bytes,
    })
}

fn verify_refresh(
    original: &BTreeMap<String, Vec<u8>>,
    refreshed: &BTreeMap<String, Vec<u8>>,
) -> Result<()> {
    if original.keys().ne(refreshed.keys()) {
        return Err(Error(
            "o refresh mudou a lista de arquivos; atualize o pack antes de exportar".into(),
        ));
    }
    if original.iter().any(|(path, bytes)| {
        !matches!(path.as_str(), PACK_FILE | "index.toml") && refreshed.get(path) != Some(bytes)
    }) {
        return Err(Error("o refresh mudou um arquivo do pack".into()));
    }
    let parse_manifest = |files: &BTreeMap<String, Vec<u8>>| -> Result<PackManifest> {
        Ok(
            PackManifest::parse(std::str::from_utf8(&files[PACK_FILE]).map_err(err)?)
                .map_err(err)?
                .value,
        )
    };
    let mut old_manifest = parse_manifest(original)?;
    let new_manifest = parse_manifest(refreshed)?;
    old_manifest.index.hash.clone_from(&new_manifest.index.hash);
    if old_manifest != new_manifest {
        return Err(Error(
            "o refresh mudou o manifesto além do hash do índice".into(),
        ));
    }
    let parse_index = |files: &BTreeMap<String, Vec<u8>>| -> Result<PackIndex> {
        Ok(
            PackIndex::parse(std::str::from_utf8(&files["index.toml"]).map_err(err)?)
                .map_err(err)?
                .value,
        )
    };
    let mut old_entries = parse_index(original)?.normalized_entries();
    let mut new_entries = parse_index(refreshed)?.normalized_entries();
    for entry in &mut old_entries {
        entry.hash.clear();
    }
    for entry in &mut new_entries {
        entry.hash.clear();
    }
    if old_entries != new_entries {
        return Err(Error("o refresh mudou o índice além dos hashes".into()));
    }
    Ok(())
}

fn write_folder(
    destination: &Path,
    files: &BTreeMap<String, Vec<u8>>,
    cancel: &CancellationToken,
) -> Result<u64> {
    let created = !destination.exists();
    if created {
        fs::create_dir(destination).map_err(err)?;
    }
    let mut written = Vec::new();
    let result = (|| {
        let mut total = 0_u64;
        for (relative, bytes) in files {
            if cancel.is_cancelled() {
                return Err(Error("exportação cancelada".into()));
            }
            let path = resolve_inside(destination, relative).map_err(err)?;
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).map_err(err)?;
            }
            let mut file = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)
                .map_err(err)?;
            written.push(path);
            file.write_all(bytes).map_err(err)?;
            total = total.saturating_add(u64::try_from(bytes.len()).map_err(err)?);
        }
        Ok(total)
    })();
    if result.is_err() {
        for path in &written {
            let _ = fs::remove_file(path);
        }
        let mut dirs = BTreeSet::new();
        for path in &written {
            let mut dir = path.parent().map(Path::to_path_buf);
            while let Some(path) = dir {
                if path == destination {
                    break;
                }
                dirs.insert(path.clone());
                dir = path.parent().map(Path::to_path_buf);
            }
        }
        for dir in dirs.iter().rev() {
            let _ = fs::remove_dir(dir);
        }
        if created {
            let _ = fs::remove_dir(destination);
        }
    }
    result
}

fn write_zip(
    destination: &Path,
    files: &BTreeMap<String, Vec<u8>>,
    cancel: &CancellationToken,
) -> Result<u64> {
    let file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(destination)
        .map_err(err)?;
    let result = (|| {
        let mut zip = zip::ZipWriter::new(file);
        let options = SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .last_modified_time(zip::DateTime::default())
            .unix_permissions(0o644);
        let mut directories = BTreeSet::new();
        for path in files.keys() {
            let mut prefix = String::new();
            for segment in path
                .split('/')
                .take(path.split('/').count().saturating_sub(1))
            {
                prefix.push_str(segment);
                prefix.push('/');
                directories.insert(prefix.clone());
            }
        }
        for directory in directories {
            zip.add_directory(directory, options.unix_permissions(0o755))
                .map_err(err)?;
        }
        for (path, bytes) in files {
            if cancel.is_cancelled() {
                return Err(Error("exportação cancelada".into()));
            }
            zip.start_file(path, options).map_err(err)?;
            zip.write_all(bytes).map_err(err)?;
        }
        let file = zip.finish().map_err(err)?;
        file.metadata().map(|meta| meta.len()).map_err(err)
    })();
    if result.is_err() {
        let _ = fs::remove_file(destination);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use git2::Repository;
    use warden_packwiz::{IndexEntry, PackManifest};

    fn fixture(root: &Path) {
        fs::create_dir_all(root.join("config")).unwrap();
        fs::write(root.join("config/a.txt"), b"ajuste").unwrap();
        fs::create_dir_all(root.join("crash-reports")).unwrap();
        fs::write(root.join("crash-reports/x.txt"), b"falha").unwrap();
        let mut index = PackIndex::default();
        index.files.push(IndexEntry::new("config/a.txt", ""));
        fs::write(root.join("index.toml"), index.to_toml_string()).unwrap();
        fs::write(
            root.join("pack.toml"),
            PackManifest::new("Teste", "1.21.1").to_toml_string(),
        )
        .unwrap();
    }

    #[test]
    fn preview_lista_apenas_indice_e_alerta_higiene() {
        let temp = tempfile::tempdir().unwrap();
        fixture(temp.path());
        let preview = preview(temp.path(), &ExportSource::Current).unwrap();
        assert_eq!(
            preview
                .files
                .iter()
                .map(|item| item.path.as_str())
                .collect::<Vec<_>>(),
            ["config/a.txt", "index.toml", "pack.toml"]
        );
        assert!(
            preview
                .preflight
                .hygiene
                .iter()
                .any(|item| item.path.starts_with("crash-reports"))
        );
        assert_eq!(
            preview
                .folders
                .iter()
                .find(|item| item.path == "config")
                .unwrap()
                .files,
            1
        );
        assert!(
            preview
                .files
                .iter()
                .find(|item| item.path == "config/a.txt")
                .unwrap()
                .local
        );
    }

    #[test]
    fn zip_e_deterministico_e_tem_so_arquivos_do_indice() {
        let temp = tempfile::tempdir().unwrap();
        let files = BTreeMap::from([
            ("pack.toml".to_owned(), b"pack".to_vec()),
            ("index.toml".to_owned(), b"index".to_vec()),
            ("config/a.txt".to_owned(), b"ajuste".to_vec()),
        ]);
        let a = temp.path().join("a.zip");
        let b = temp.path().join("b.zip");
        write_zip(&a, &files, &CancellationToken::new()).unwrap();
        write_zip(&b, &files, &CancellationToken::new()).unwrap();
        assert_eq!(fs::read(&a).unwrap(), fs::read(&b).unwrap());
        let mut zip = zip::ZipArchive::new(fs::File::open(a).unwrap()).unwrap();
        let mut names = (0..zip.len())
            .map(|index| zip.by_index(index).unwrap().name().to_owned())
            .collect::<Vec<_>>();
        names.sort();
        assert_eq!(
            names,
            ["config/", "config/a.txt", "index.toml", "pack.toml"]
        );
        let occupied = temp.path().join("existente.zip");
        fs::write(&occupied, b"arquivo de outra pessoa").unwrap();
        assert!(write_zip(&occupied, &files, &CancellationToken::new()).is_err());
        assert_eq!(fs::read(occupied).unwrap(), b"arquivo de outra pessoa");
    }

    #[test]
    fn versao_salva_le_blobs_da_tag_sem_ler_pasta_atual() {
        let temp = tempfile::tempdir().unwrap();
        fixture(temp.path());
        let repo = Repository::init(temp.path()).unwrap();
        let mut index = repo.index().unwrap();
        index.add_path(Path::new("pack.toml")).unwrap();
        index.add_path(Path::new("index.toml")).unwrap();
        index.add_path(Path::new("config/a.txt")).unwrap();
        let tree_id = index.write_tree().unwrap();
        let tree = repo.find_tree(tree_id).unwrap();
        let signature = git2::Signature::now("Teste", "teste@example.invalid").unwrap();
        let commit = repo
            .commit(Some("HEAD"), &signature, &signature, "v1", &tree, &[])
            .unwrap();
        let object = repo.find_object(commit, None).unwrap();
        repo.tag("v1.0.0", &object, &signature, "versão", false)
            .unwrap();
        fs::write(temp.path().join("config/a.txt"), b"mudou").unwrap();
        let reader = SourceReader::open(
            temp.path(),
            &ExportSource::Saved {
                version: "1.0.0".into(),
            },
        )
        .unwrap();
        assert_eq!(reader.read("config/a.txt").unwrap(), b"ajuste");
        assert!(
            SourceReader::open(
                temp.path(),
                &ExportSource::Saved {
                    version: "2.0.0".into()
                }
            )
            .is_err()
        );
    }
}
