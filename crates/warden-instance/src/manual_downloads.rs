//! Downloads manuais da CurseForge (SPEC T20; ROADMAP L-06).
//!
//! A materialização devolve os mods bloqueados para apps de terceiros em [`Report::blocked`]
//! (`crate::blocked`). Este módulo cuida do que acontece depois:
//!
//! - [`ManualDownloads`]: a lista da tela T20 com o estado de cada item (aguardando,
//!   encontrado, arquivo errado). Só memória: nada daqui é gravado.
//! - [`provide_file`]: o arquivo escolhido pela pessoa. O hash é conferido **antes** de qualquer
//!   cópia; diferente, nada é copiado e o erro diz qual arquivo era o esperado
//!   ([`Error::ManualFileMismatch`]). Igual, o arquivo vai para o cache de downloads (nunca para
//!   a pasta do pack) e a próxima materialização o instala como qualquer outro.
//! - [`DownloadsWatch`] (P1): observa a pasta Downloads com o `notify` e reconhece sozinho, pelo
//!   hash, os arquivos esperados que aparecerem lá. Antes de observar, procura os que já estão
//!   lá com o nome esperado. Parar é soltar o valor (`Drop`). Os registros nunca levam o
//!   caminho dos arquivos (é a pasta pessoal de quem usa o Warden).
//!
//! [`Report::blocked`]: crate::Report::blocked

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use notify::event::{AccessKind, AccessMode, ModifyKind, RenameMode};
use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher as _};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio::time::Instant;
use warden_core::DomainError as _;
use warden_packwiz::hash::hash_matches;

use crate::blocked::{BlockedFile, accept_manual_file};
use crate::downloads::{CachedFile, DownloadCache, hash_file};
use crate::error::{Error, Result};

/// Maior arquivo da pasta Downloads que o vigia confere (os jars de mods ficam bem abaixo).
/// Um arquivo escolhido pela pessoa é sempre conferido, de qualquer tamanho.
pub const MAX_WATCHED_FILE_BYTES: u64 = 512 * 1024 * 1024;

/// Quanto tempo sem novidades num arquivo antes de conferir (o navegador ainda pode estar
/// gravando).
pub const SETTLE: Duration = Duration::from_millis(700);

/// Finais de arquivos de download ainda incompletos (Chrome/Edge, Firefox, Safari, Opera e
/// gerenciadores de download).
const PARTIAL_SUFFIXES: [&str; 7] = [
    ".crdownload",
    ".part",
    ".partial",
    ".download",
    ".opdownload",
    ".tmp",
    ".!ut",
];

/// Como um item foi encontrado.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum FoundVia {
    /// A pessoa escolheu o arquivo ("Selecionar arquivo baixado…").
    Selected,
    /// O Warden achou o arquivo na pasta Downloads.
    DownloadsFolder,
}

/// Estado de um item da lista.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase", tag = "state")]
pub enum ManualDownloadStatus {
    /// Esperando o arquivo.
    Waiting,
    /// Arquivo certo, já no cache de downloads.
    Found {
        /// Como foi encontrado.
        via: FoundVia,
    },
    /// O último arquivo escolhido não é o certo (hash diferente). Continua esperando.
    #[serde(rename_all = "camelCase")]
    WrongFile {
        /// Nome do arquivo escolhido (só o nome, sem a pasta).
        chosen_file: String,
    },
}

/// Um item da lista, como a interface mostra.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ManualDownloadItem {
    /// Caminho do metafile no pack (identifica o item).
    pub metafile: String,
    /// Nome do mod.
    pub name: String,
    /// Nome do arquivo esperado.
    pub file_name: String,
    /// Se há a página do download no site (o endereço não sai do backend).
    pub has_page: bool,
    /// Estado.
    pub status: ManualDownloadStatus,
}

/// A lista da T20 com o estado de cada item.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ManualDownloads {
    entries: Vec<(BlockedFile, ManualDownloadStatus)>,
}

impl ManualDownloads {
    /// Lista nova, com tudo aguardando. Um metafile repetido entra uma vez só.
    #[must_use]
    pub fn new(blocked: Vec<BlockedFile>) -> Self {
        let mut entries: Vec<(BlockedFile, ManualDownloadStatus)> = Vec::new();
        for file in blocked {
            if !entries
                .iter()
                .any(|(known, _)| known.metafile == file.metafile)
            {
                entries.push((file, ManualDownloadStatus::Waiting));
            }
        }
        Self { entries }
    }

    /// Os itens, na ordem da materialização.
    #[must_use]
    pub fn items(&self) -> Vec<ManualDownloadItem> {
        self.entries
            .iter()
            .map(|(file, status)| ManualDownloadItem {
                metafile: file.metafile.clone(),
                name: file.name.clone(),
                file_name: file.file_name.clone(),
                has_page: file.page_url.is_some(),
                status: status.clone(),
            })
            .collect()
    }

    /// O item do metafile.
    #[must_use]
    pub fn get(&self, metafile: &str) -> Option<&BlockedFile> {
        self.entries
            .iter()
            .find(|(file, _)| file.metafile == metafile)
            .map(|(file, _)| file)
    }

    /// O item do metafile como a interface mostra.
    #[must_use]
    pub fn item(&self, metafile: &str) -> Option<ManualDownloadItem> {
        self.items()
            .into_iter()
            .find(|item| item.metafile == metafile)
    }

    /// Os que ainda faltam.
    #[must_use]
    pub fn waiting(&self) -> Vec<BlockedFile> {
        self.entries
            .iter()
            .filter(|(_, status)| !matches!(status, ManualDownloadStatus::Found { .. }))
            .map(|(file, _)| file.clone())
            .collect()
    }

    /// Se todos foram encontrados.
    #[must_use]
    pub fn is_complete(&self) -> bool {
        self.entries
            .iter()
            .all(|(_, status)| matches!(status, ManualDownloadStatus::Found { .. }))
    }

    /// Marca o item como encontrado. Devolve se algo mudou (encontrado continua encontrado).
    pub fn mark_found(&mut self, metafile: &str, via: FoundVia) -> bool {
        self.set(metafile, |status| {
            (!matches!(status, ManualDownloadStatus::Found { .. }))
                .then_some(ManualDownloadStatus::Found { via })
        })
    }

    /// Marca que o arquivo escolhido não é o certo. Um item já encontrado não volta atrás.
    pub fn mark_wrong(&mut self, metafile: &str, chosen_file: String) -> bool {
        self.set(metafile, |status| {
            (!matches!(status, ManualDownloadStatus::Found { .. }))
                .then_some(ManualDownloadStatus::WrongFile { chosen_file })
        })
    }

    fn set(
        &mut self,
        metafile: &str,
        change: impl FnOnce(&ManualDownloadStatus) -> Option<ManualDownloadStatus>,
    ) -> bool {
        let Some((_, status)) = self
            .entries
            .iter_mut()
            .find(|(file, _)| file.metafile == metafile)
        else {
            return false;
        };
        match change(status) {
            Some(new) if new != *status => {
                *status = new;
                true
            }
            _ => false,
        }
    }
}

/// Se o arquivo é o esperado, pelo hash do metafile. Só lê: nada é copiado.
pub fn file_matches(blocked: &BlockedFile, file: &Path) -> Result<bool> {
    let expected = blocked.expected()?;
    let (_, hashes) = hash_file(file).map_err(|error| Error::io("ler o arquivo", file, error))?;
    let actual = hashes.get(expected.format).unwrap_or_default();
    Ok(hash_matches(expected.format, &expected.value, &actual))
}

/// O arquivo escolhido pela pessoa para um item. Confere o hash antes de copiar: diferente,
/// [`Error::ManualFileMismatch`] e nada é copiado. Igual, o arquivo vai para o cache (conferido
/// de novo durante a cópia, caso tenha mudado no meio) e continua onde estava.
pub fn provide_file(
    cache: &DownloadCache,
    blocked: &BlockedFile,
    file: &Path,
) -> Result<CachedFile> {
    if !file_matches(blocked, file)? {
        return Err(Error::ManualFileMismatch {
            expected_file: blocked.file_name.clone(),
            format: blocked.hash_format.clone(),
        });
    }
    accept_manual_file(cache, blocked, file)
}

/// Qual dos itens esperados é este arquivo (pelo hash, calculado uma vez para todos).
pub fn identify(waiting: &[BlockedFile], file: &Path) -> Result<Option<BlockedFile>> {
    let (_, hashes) = hash_file(file).map_err(|error| Error::io("ler o arquivo", file, error))?;
    for item in waiting {
        let Ok(expected) = item.expected() else {
            continue;
        };
        let actual = hashes.get(expected.format).unwrap_or_default();
        if hash_matches(expected.format, &expected.value, &actual) {
            return Ok(Some(item.clone()));
        }
    }
    Ok(None)
}

/// Só o nome do arquivo, para mostrar (nunca a pasta).
#[must_use]
pub fn display_name(file: &Path) -> String {
    file.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// Se o nome é de um download ainda incompleto.
fn is_partial(name: &str) -> bool {
    let lower = name.to_lowercase();
    PARTIAL_SUFFIXES
        .iter()
        .any(|suffix| lower.ends_with(suffix))
}

/// Extensão em minúsculas (`jar`), se houver.
fn extension(name: &str) -> Option<String> {
    Path::new(name)
        .extension()
        .map(|extension| extension.to_string_lossy().to_lowercase())
}

/// Se vale conferir o arquivo da pasta Downloads: arquivo comum, não oculto, download
/// terminado, com a extensão de algum item esperado e tamanho entre 1 byte e
/// [`MAX_WATCHED_FILE_BYTES`].
#[must_use]
pub fn is_candidate(waiting: &[BlockedFile], file: &Path) -> bool {
    let name = display_name(file);
    if name.is_empty() || name.starts_with('.') || name.starts_with('~') || is_partial(&name) {
        return false;
    }
    let ext = extension(&name);
    let wanted = waiting
        .iter()
        .any(|item| extension(&item.file_name).is_none_or(|expected| Some(expected) == ext));
    if !wanted {
        return false;
    }
    fs::metadata(file).is_ok_and(|metadata| {
        metadata.is_file() && metadata.len() > 0 && metadata.len() <= MAX_WATCHED_FILE_BYTES
    })
}

/// Se `name` é o nome esperado ou a cópia numerada que os navegadores criam
/// (`mod (1).jar`, `mod(2).jar`). Sem diferença de maiúsculas.
fn same_download_name(expected: &str, name: &str) -> bool {
    let expected = expected.to_lowercase();
    let name = name.to_lowercase();
    if name == expected {
        return true;
    }
    let (stem, ext) = match expected.rfind('.') {
        Some(dot) if dot > 0 => (&expected[..dot], &expected[dot..]),
        _ => (expected.as_str(), ""),
    };
    let Some(rest) = name.strip_prefix(stem) else {
        return false;
    };
    let Some(middle) = rest.strip_suffix(ext) else {
        return false;
    };
    let middle = middle.strip_prefix(' ').unwrap_or(middle);
    middle
        .strip_prefix('(')
        .and_then(|inner| inner.strip_suffix(')'))
        .is_some_and(|number| !number.is_empty() && number.chars().all(|c| c.is_ascii_digit()))
}

/// Arquivos da pasta (sem subpastas) com o nome de algum item esperado. Barato: não lê o
/// conteúdo de nada. Pasta que não existe ou não abre: lista vazia.
#[must_use]
pub fn find_by_name(folder: &Path, waiting: &[BlockedFile]) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(folder) else {
        return Vec::new();
    };
    let mut found: Vec<PathBuf> = entries
        .flatten()
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_file()))
        .filter(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            waiting
                .iter()
                .any(|item| same_download_name(&item.file_name, &name))
        })
        .map(|entry| entry.path())
        .collect();
    found.sort();
    found
}

/// Quem lista o que falta e quem recebe o que foi achado.
#[derive(Clone)]
pub struct WatchContext {
    /// Cache de downloads, onde o arquivo achado é guardado.
    pub cache: Arc<DownloadCache>,
    /// O que ainda falta (consultado a cada arquivo).
    pub waiting: Arc<dyn Fn() -> Vec<BlockedFile> + Send + Sync>,
    /// Um item foi achado e já está no cache.
    pub found: Arc<dyn Fn(&BlockedFile) + Send + Sync>,
}

impl std::fmt::Debug for WatchContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WatchContext").finish_non_exhaustive()
    }
}

/// O vigia da pasta Downloads (P1). Para de observar quando é solto.
#[derive(Debug)]
pub struct DownloadsWatch {
    task: JoinHandle<()>,
    _watcher: RecommendedWatcher,
}

impl DownloadsWatch {
    /// Começa a observar `folder` (sem subpastas). Antes, confere os arquivos que já estão lá
    /// com o nome esperado. Precisa de um runtime do Tokio.
    pub fn start(folder: &Path, context: WatchContext) -> Result<Self> {
        let (sender, receiver) = mpsc::unbounded_channel::<PathBuf>();
        let mut watcher =
            notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
                let Ok(event) = event else {
                    return;
                };
                if is_relevant(event.kind) {
                    for path in event.paths {
                        // Quem recebia já parou: não há o que fazer.
                        let _ = sender.send(path);
                    }
                }
            })
            .map_err(|error| watch_error(folder, &error))?;
        watcher
            .watch(folder, RecursiveMode::NonRecursive)
            .map_err(|error| watch_error(folder, &error))?;
        let initial = find_by_name(folder, &(context.waiting)());
        let task = tokio::spawn(run_watch(initial, receiver, context));
        Ok(Self {
            task,
            _watcher: watcher,
        })
    }
}

impl Drop for DownloadsWatch {
    fn drop(&mut self) {
        self.task.abort();
    }
}

fn watch_error(folder: &Path, error: &notify::Error) -> Error {
    Error::io(
        "observar a pasta",
        folder,
        std::io::Error::other(format!("{:?}", error.kind)),
    )
}

/// Eventos que podem significar "um arquivo terminou de chegar".
fn is_relevant(kind: EventKind) -> bool {
    matches!(
        kind,
        EventKind::Create(_)
            | EventKind::Modify(
                ModifyKind::Any
                    | ModifyKind::Data(_)
                    | ModifyKind::Name(RenameMode::To | RenameMode::Both | RenameMode::Any)
            )
            | EventKind::Access(AccessKind::Close(AccessMode::Write))
    )
}

/// Tamanho e data: o mesmo arquivo sem mudanças não é lido de novo.
type Signature = (u64, Option<SystemTime>);

fn signature(file: &Path) -> Option<Signature> {
    fs::metadata(file)
        .ok()
        .map(|metadata| (metadata.len(), metadata.modified().ok()))
}

/// O laço do vigia: junta os eventos de cada arquivo e confere quando ele sossega por
/// [`SETTLE`].
async fn run_watch(
    initial: Vec<PathBuf>,
    mut receiver: mpsc::UnboundedReceiver<PathBuf>,
    context: WatchContext,
) {
    let mut pending: HashMap<PathBuf, Instant> = HashMap::new();
    let mut checked: HashMap<PathBuf, Signature> = HashMap::new();
    let start = Instant::now()
        .checked_sub(SETTLE)
        .unwrap_or_else(Instant::now);
    for path in initial {
        pending.insert(path, start);
    }
    loop {
        let deadline = pending.values().min().map(|last| *last + SETTLE);
        tokio::select! {
            next = receiver.recv() => match next {
                Some(path) => {
                    pending.insert(path, Instant::now());
                }
                None => return,
            },
            () = sleep_until(deadline) => {
                let now = Instant::now();
                let ready: Vec<PathBuf> = pending
                    .iter()
                    .filter(|(_, last)| **last + SETTLE <= now)
                    .map(|(path, _)| path.clone())
                    .collect();
                for path in ready {
                    pending.remove(&path);
                    check(&context, &mut checked, path).await;
                }
            }
        }
    }
}

async fn sleep_until(deadline: Option<Instant>) {
    match deadline {
        Some(deadline) => tokio::time::sleep_until(deadline).await,
        None => std::future::pending().await,
    }
}

/// Confere um arquivo que sossegou e, se for um dos esperados, guarda no cache e avisa.
async fn check(context: &WatchContext, checked: &mut HashMap<PathBuf, Signature>, path: PathBuf) {
    let waiting = (context.waiting)();
    if waiting.is_empty() || !is_candidate(&waiting, &path) {
        return;
    }
    let Some(now) = signature(&path) else {
        return;
    };
    if checked.get(&path) == Some(&now) {
        return;
    }
    let cache = Arc::clone(&context.cache);
    let file = path.clone();
    let outcome = tokio::task::spawn_blocking(move || -> Result<Option<BlockedFile>> {
        let Some(item) = identify(&waiting, &file)? else {
            return Ok(None);
        };
        accept_manual_file(&cache, &item, &file)?;
        Ok(Some(item))
    })
    .await;
    match outcome {
        Ok(Ok(Some(item))) => {
            checked.insert(path, now);
            tracing::info!(item = %item.name, "download manual reconhecido na pasta Downloads");
            (context.found)(&item);
        }
        Ok(Ok(None)) => {
            checked.insert(path, now);
        }
        // Arquivo ainda preso pelo navegador ou apagado no meio: um novo evento traz de volta.
        // O erro tem o caminho do arquivo, por isso só o código vai para o registro.
        Ok(Err(error)) => {
            tracing::debug!(code = ?error.code(), "arquivo da pasta Downloads não conferido");
        }
        Err(error) => {
            tracing::warn!(%error, "conferência de arquivo da pasta Downloads interrompida");
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use warden_packwiz::HashFormat;
    use warden_packwiz::hash::hash_bytes;

    use super::*;
    use crate::downloads::INCOMING_DIR;

    fn blocked(metafile: &str, file_name: &str, data: &[u8]) -> BlockedFile {
        BlockedFile {
            metafile: metafile.to_owned(),
            name: format!("Mod {metafile}"),
            file_name: file_name.to_owned(),
            project_id: 448_233,
            file_id: 1,
            page_url: Some("https://www.curseforge.com/minecraft/mc-mods/x/files/1".to_owned()),
            hash_format: "sha1".to_owned(),
            hash: hash_bytes(HashFormat::Sha1, data),
        }
    }

    fn incoming_is_empty(cache: &DownloadCache) -> bool {
        fs::read_dir(cache.root().join(INCOMING_DIR))
            .map_or(true, |mut entries| entries.next().is_none())
    }

    #[test]
    fn lista_comeca_aguardando_e_termina_quando_todos_chegam() {
        let a = blocked("mods/a.pw.toml", "a.jar", b"a");
        let b = blocked("mods/b.pw.toml", "b.jar", b"b");
        let mut list = ManualDownloads::new(vec![a.clone(), b.clone(), a.clone()]);
        assert_eq!(list.items().len(), 2, "metafile repetido entra uma vez");
        assert!(!list.is_complete());
        assert_eq!(list.waiting(), vec![a.clone(), b.clone()]);

        assert!(list.mark_wrong("mods/a.pw.toml", "outro.jar".to_owned()));
        assert_eq!(
            list.item("mods/a.pw.toml").unwrap().status,
            ManualDownloadStatus::WrongFile {
                chosen_file: "outro.jar".to_owned()
            }
        );
        assert_eq!(list.waiting().len(), 2, "arquivo errado continua esperando");

        assert!(list.mark_found("mods/a.pw.toml", FoundVia::Selected));
        assert!(!list.mark_found("mods/a.pw.toml", FoundVia::DownloadsFolder));
        assert!(!list.mark_wrong("mods/a.pw.toml", "x.jar".to_owned()));
        assert_eq!(list.waiting(), vec![b]);
        assert!(list.mark_found("mods/b.pw.toml", FoundVia::DownloadsFolder));
        assert!(list.is_complete());
        assert!(!list.mark_found("mods/nao-existe.pw.toml", FoundVia::Selected));
        assert!(list.items()[0].has_page);
    }

    #[test]
    fn status_em_json_com_o_estado() {
        let json = serde_json::to_value(ManualDownloadStatus::WrongFile {
            chosen_file: "x.jar".to_owned(),
        })
        .unwrap();
        assert_eq!(
            json,
            serde_json::json!({ "state": "wrongFile", "chosenFile": "x.jar" })
        );
        let json = serde_json::to_value(ManualDownloadStatus::Found {
            via: FoundVia::DownloadsFolder,
        })
        .unwrap();
        assert_eq!(
            json,
            serde_json::json!({ "state": "found", "via": "downloadsFolder" })
        );
    }

    #[test]
    fn ca_t20_01_hash_errado_nao_copia_nada() {
        let dir = crate::test_support::temp_dir();
        let cache = DownloadCache::open(&dir.path().join("downloads")).unwrap();
        let item = blocked("mods/a.pw.toml", "a.jar", b"jar certo");
        let folder = dir.path().join("Meus Downloads").join("ação é");
        fs::create_dir_all(&folder).unwrap();
        let wrong = folder.join("cópia errada (1).jar");
        fs::write(&wrong, b"jar errado").unwrap();

        let error = provide_file(&cache, &item, &wrong).unwrap_err();
        assert!(
            matches!(&error, Error::ManualFileMismatch { expected_file, .. } if expected_file == "a.jar"),
            "{error:?}"
        );
        assert!(incoming_is_empty(&cache), "nada foi copiado");
        assert!(cache.find(&item.expected().unwrap()).unwrap().is_none());

        let right = folder.join("arquivo certo ç.jar");
        fs::write(&right, b"jar certo").unwrap();
        let stored = provide_file(&cache, &item, &right).unwrap();
        assert_eq!(fs::read(&stored.path).unwrap(), b"jar certo");
        assert!(stored.path.starts_with(cache.root()));
        assert!(right.exists(), "o arquivo escolhido fica onde estava");
    }

    #[test]
    fn arquivo_que_sumiu_e_erro_de_disco() {
        let dir = crate::test_support::temp_dir();
        let cache = DownloadCache::open(&dir.path().join("downloads")).unwrap();
        let item = blocked("mods/a.pw.toml", "a.jar", b"x");
        let error = provide_file(&cache, &item, &dir.path().join("sumiu.jar")).unwrap_err();
        assert!(matches!(error, Error::Io { .. }), "{error:?}");
    }

    #[test]
    fn identifica_pelo_hash_entre_varios() {
        let dir = crate::test_support::temp_dir();
        let a = blocked("mods/a.pw.toml", "a.jar", b"conteudo a");
        let mut b = blocked("mods/b.pw.toml", "b.jar", b"conteudo b");
        b.hash_format = "murmur2".to_owned();
        b.hash = hash_bytes(HashFormat::Murmur2, b"conteudo b");
        let file = dir.path().join("qualquer nome.jar");
        fs::write(&file, b"conteudo b").unwrap();
        assert_eq!(identify(&[a.clone(), b.clone()], &file).unwrap(), Some(b));
        fs::write(&file, b"outro").unwrap();
        assert_eq!(identify(&[a], &file).unwrap(), None);
    }

    #[test]
    fn nomes_dos_navegadores() {
        assert!(same_download_name("Mod-1.0.jar", "mod-1.0.jar"));
        assert!(same_download_name("mod-1.0.jar", "mod-1.0 (1).jar"));
        assert!(same_download_name("mod-1.0.jar", "mod-1.0(12).jar"));
        assert!(!same_download_name("mod-1.0.jar", "mod-1.0 ().jar"));
        assert!(!same_download_name("mod-1.0.jar", "mod-1.0 (a).jar"));
        assert!(!same_download_name("mod-1.0.jar", "mod-1.0.jar.crdownload"));
        assert!(!same_download_name("mod-1.0.jar", "outro-mod-1.0.jar"));
        assert!(same_download_name("semextensao", "semextensao (2)"));
    }

    #[test]
    fn candidatos_ignoram_downloads_incompletos_e_outras_extensoes() {
        let dir = crate::test_support::temp_dir();
        let waiting = [blocked("mods/a.pw.toml", "a.jar", b"a")];
        let write = |name: &str, data: &[u8]| {
            let path = dir.path().join(name);
            fs::write(&path, data).unwrap();
            path
        };
        assert!(is_candidate(&waiting, &write("baixado agora.JAR", b"x")));
        assert!(!is_candidate(&waiting, &write("a.jar.crdownload", b"x")));
        assert!(!is_candidate(&waiting, &write("a.jar.part", b"x")));
        assert!(!is_candidate(&waiting, &write("foto.png", b"x")));
        assert!(!is_candidate(&waiting, &write(".a.jar", b"x")));
        assert!(!is_candidate(&waiting, &write("vazio.jar", b"")));
        assert!(!is_candidate(&waiting, &dir.path().join("nao-existe.jar")));
        fs::create_dir(dir.path().join("pasta.jar")).unwrap();
        assert!(!is_candidate(&waiting, &dir.path().join("pasta.jar")));
    }

    #[test]
    fn procura_pelo_nome_sem_subpastas() {
        let dir = crate::test_support::temp_dir();
        let waiting = [blocked("mods/a.pw.toml", "Mod Ação.jar", b"a")];
        fs::write(dir.path().join("mod ação (1).jar"), b"x").unwrap();
        fs::write(dir.path().join("outro.jar"), b"x").unwrap();
        fs::create_dir(dir.path().join("sub")).unwrap();
        fs::write(dir.path().join("sub").join("Mod Ação.jar"), b"x").unwrap();
        let found = find_by_name(dir.path(), &waiting);
        assert_eq!(found, vec![dir.path().join("mod ação (1).jar")]);
        assert!(find_by_name(&dir.path().join("nao-existe"), &waiting).is_empty());
    }

    /// Contexto de teste: a lista em memória e o que foi achado.
    fn context(
        cache: Arc<DownloadCache>,
        list: Arc<Mutex<ManualDownloads>>,
    ) -> (WatchContext, Arc<Mutex<Vec<String>>>) {
        let found = Arc::new(Mutex::new(Vec::new()));
        let waiting_list = Arc::clone(&list);
        let found_list = Arc::clone(&found);
        let context = WatchContext {
            cache,
            waiting: Arc::new(move || waiting_list.lock().unwrap().waiting()),
            found: Arc::new(move |item: &BlockedFile| {
                list.lock()
                    .unwrap()
                    .mark_found(&item.metafile, FoundVia::DownloadsFolder);
                found_list.lock().unwrap().push(item.metafile.clone());
            }),
        };
        (context, found)
    }

    async fn wait_until(what: impl Fn() -> bool) -> bool {
        let limit = Instant::now() + Duration::from_secs(15);
        while Instant::now() < limit {
            if what() {
                return true;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        what()
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn vigia_reconhece_o_que_ja_estava_e_o_que_chega() {
        let dir = crate::test_support::temp_dir();
        let cache = Arc::new(DownloadCache::open(&dir.path().join("cache")).unwrap());
        let downloads = dir.path().join("Downloads do usuário");
        fs::create_dir_all(&downloads).unwrap();
        let a = blocked("mods/a.pw.toml", "a-1.0.jar", b"jar a");
        let b = blocked("mods/b.pw.toml", "b-2.0.jar", b"jar b");
        // Já estava lá com o nome esperado (cópia numerada do navegador).
        fs::write(downloads.join("a-1.0 (1).jar"), b"jar a").unwrap();
        let list = Arc::new(Mutex::new(ManualDownloads::new(vec![a.clone(), b.clone()])));
        let (context, found) = context(Arc::clone(&cache), Arc::clone(&list));
        let watch = DownloadsWatch::start(&downloads, context).unwrap();

        assert!(wait_until(|| found.lock().unwrap().len() == 1).await);
        assert_eq!(found.lock().unwrap()[0], "mods/a.pw.toml");

        // Um arquivo errado com a extensão certa é ignorado; o download do navegador chega
        // como `.crdownload` e é renomeado no fim.
        fs::write(downloads.join("errado.jar"), b"nao e o b").unwrap();
        let partial = downloads.join("nome diferente é.jar.crdownload");
        fs::write(&partial, b"jar b").unwrap();
        tokio::time::sleep(Duration::from_millis(200)).await;
        fs::rename(&partial, downloads.join("nome diferente é.jar")).unwrap();

        assert!(wait_until(|| list.lock().unwrap().is_complete()).await);
        assert_eq!(
            *found.lock().unwrap(),
            vec!["mods/a.pw.toml".to_owned(), "mods/b.pw.toml".to_owned()]
        );
        assert!(cache.find(&b.expected().unwrap()).unwrap().is_some());
        drop(watch);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn vigia_solto_para_de_observar() {
        let dir = crate::test_support::temp_dir();
        let cache = Arc::new(DownloadCache::open(&dir.path().join("cache")).unwrap());
        let downloads = dir.path().join("Downloads");
        fs::create_dir_all(&downloads).unwrap();
        let a = blocked("mods/a.pw.toml", "a.jar", b"jar a");
        let list = Arc::new(Mutex::new(ManualDownloads::new(vec![a.clone()])));
        let (context, found) = context(Arc::clone(&cache), Arc::clone(&list));
        let watch = DownloadsWatch::start(&downloads, context).unwrap();
        drop(watch);
        fs::write(downloads.join("a.jar"), b"jar a").unwrap();
        tokio::time::sleep(SETTLE * 3).await;
        assert!(found.lock().unwrap().is_empty());
        assert!(cache.find(&a.expected().unwrap()).unwrap().is_none());
    }

    #[test]
    fn pasta_que_nao_existe_nao_e_observada() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let _guard = runtime.enter();
        let dir = crate::test_support::temp_dir();
        let cache = Arc::new(DownloadCache::open(&dir.path().join("cache")).unwrap());
        let list = Arc::new(Mutex::new(ManualDownloads::default()));
        let (context, _) = context(cache, list);
        let error = DownloadsWatch::start(&dir.path().join("nao-existe"), context).unwrap_err();
        assert!(matches!(error, Error::Io { .. }), "{error:?}");
    }
}
