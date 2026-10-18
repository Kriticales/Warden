//! Escrita atômica de arquivos (ARCHITECTURE §6.5 e §14).
//!
//! [`atomic_write`] grava num arquivo temporário ao lado do destino (sufixo `.warden-tmp`),
//! força os dados para o disco (`fsync`) e só então renomeia por cima do destino. Quem lê o
//! arquivo vê o conteúdo antigo ou o novo inteiro, nunca um pedaço. Se algo falhar antes da
//! renomeação, o destino fica intacto e o temporário é apagado; se o processo morrer no meio,
//! sobra um `.warden-tmp`, que [`remove_temp_files`] apaga na próxima abertura.
//!
//! No Windows, renomear por cima (ou apagar) um arquivo que outro programa mantém aberto sem
//! `FILE_SHARE_DELETE` falha com "acesso negado" ou violação de compartilhamento
//! (ARCHITECTURE §15). A renomeação e a remoção repetem só esse erro, poucas vezes e por pouco
//! tempo ([`retry_on_sharing_error`]); qualquer outro erro, e o de compartilhamento depois das
//! tentativas, chegam ao chamador como estão.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Write as _};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use crate::error::CoreError;
use crate::fault;

/// Sufixo dos arquivos temporários do Warden.
pub const TEMP_SUFFIX: &str = ".warden-tmp";

/// Contador para nomes temporários únicos dentro do processo.
static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Grava `contents` em `path` de forma atômica (`.warden-tmp` + fsync + renomeação).
///
/// A pasta de `path` precisa existir. Erros: [`CoreError::Io`] com o caminho e a etapa que
/// falhou; nesse caso o arquivo de destino continua como estava.
pub fn atomic_write(path: &Path, contents: &[u8]) -> Result<(), CoreError> {
    write_atomically(path, contents, false)
}

/// Como [`atomic_write`], mas o arquivo novo nasce legível só pelo usuário (`0600` no Linux;
/// no Windows vale a permissão herdada da pasta do usuário). Usado pelo `.env` das chaves.
pub fn atomic_write_private(path: &Path, contents: &[u8]) -> Result<(), CoreError> {
    write_atomically(path, contents, true)
}

fn write_atomically(path: &Path, contents: &[u8], private: bool) -> Result<(), CoreError> {
    let temp = temp_path_for(path)?;
    let result = write_temp_and_rename(&temp, path, contents, private);
    if result.is_err() {
        // O temporário pode nem ter sido criado; se a remoção falhar, a limpeza da próxima
        // abertura (`remove_temp_files`) apaga o que sobrou.
        let _ = fs::remove_file(&temp);
    }
    result
}

fn temp_path_for(path: &Path) -> Result<PathBuf, CoreError> {
    let name = path.file_name().ok_or_else(|| {
        CoreError::io(
            "gravar",
            path,
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "o caminho não tem nome de arquivo",
            ),
        )
    })?;
    let counter = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let mut temp_name = name.to_os_string();
    temp_name.push(format!(".{}-{counter}{TEMP_SUFFIX}", std::process::id()));
    Ok(path.with_file_name(temp_name))
}

fn write_temp_and_rename(
    temp: &Path,
    path: &Path,
    contents: &[u8],
    private: bool,
) -> Result<(), CoreError> {
    fault::check("atomic_write.before_write").map_err(|e| CoreError::io("gravar", temp, e))?;
    let mut file = open_new(temp, private).map_err(|e| CoreError::io("criar", temp, e))?;
    file.write_all(contents)
        .map_err(|e| CoreError::io("gravar", temp, e))?;
    file.sync_all()
        .map_err(|e| CoreError::io("sincronizar com o disco", temp, e))?;
    drop(file);

    fault::check("atomic_write.before_rename")
        .map_err(|e| CoreError::io("renomear para", path, e))?;
    rename_with_retry(temp, path).map_err(|e| CoreError::io("renomear para", path, e))?;
    sync_parent(path).map_err(|e| CoreError::io("sincronizar a pasta de", path, e))?;
    Ok(())
}

fn open_new(path: &Path, private: bool) -> io::Result<File> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    if private {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.mode(0o600);
    }
    #[cfg(not(unix))]
    let _ = private;
    options.open(path)
}

/// Se `error` é o Windows recusando mexer num arquivo que outro handle mantém aberto sem
/// `FILE_SHARE_DELETE` (ARCHITECTURE §15): `ERROR_ACCESS_DENIED` (5), que o `MoveFileEx`
/// devolve ao renomear por cima de um arquivo aberto assim; `ERROR_SHARING_VIOLATION` (32), que
/// o `DeleteFile` devolve ao apagá-lo e o `MoveFileEx` ao renomeá-lo; e `ERROR_LOCK_VIOLATION`
/// (33). A `std` só traduz o 5 para [`io::ErrorKind::PermissionDenied`]; o 32 e o 33 ficam
/// sem categoria, por isso a conferência é pelo código do sistema. Fora do Windows, nunca.
#[must_use]
pub fn is_windows_sharing_error(error: &io::Error) -> bool {
    const ERROR_ACCESS_DENIED: i32 = 5;
    const ERROR_SHARING_VIOLATION: i32 = 32;
    const ERROR_LOCK_VIOLATION: i32 = 33;
    cfg!(windows)
        && matches!(
            error.raw_os_error(),
            Some(ERROR_ACCESS_DENIED | ERROR_SHARING_VIOLATION | ERROR_LOCK_VIOLATION)
        )
}

/// Esperas entre as tentativas de [`retry_on_sharing_error`]: 6 tentativas em cerca de
/// 600 ms. Cobre o que o Warden não controla (antivírus, indexador, Explorer e outros
/// programas que abrem o arquivo por instantes); um arquivo preso de verdade falha em menos
/// de um segundo, com o erro do sistema.
const SHARING_RETRY_DELAYS: [Duration; 5] = [
    Duration::from_millis(20),
    Duration::from_millis(40),
    Duration::from_millis(80),
    Duration::from_millis(160),
    Duration::from_millis(300),
];

/// Repete `action` enquanto ela falhar com o erro de compartilhamento do Windows
/// ([`is_windows_sharing_error`]), por até [`SHARING_RETRY_DELAYS`] vezes; qualquer outro
/// erro, e o último de compartilhamento, voltam como estão. Só para passos idempotentes
/// (renomear um temporário já gravado, apagar um arquivo): o conteúdo nunca é regravado.
pub fn retry_on_sharing_error(action: impl FnMut() -> io::Result<()>) -> io::Result<()> {
    retry_with(action, is_windows_sharing_error, std::thread::sleep)
}

/// [`retry_on_sharing_error`] com a conferência do erro e a espera trocáveis, para os testes
/// provarem a política sem depender do Windows nem do relógio.
fn retry_with(
    mut action: impl FnMut() -> io::Result<()>,
    is_transient: impl Fn(&io::Error) -> bool,
    mut sleep: impl FnMut(Duration),
) -> io::Result<()> {
    let mut delays = SHARING_RETRY_DELAYS.iter();
    loop {
        match action() {
            Ok(()) => return Ok(()),
            Err(error) if is_transient(&error) => {
                let Some(delay) = delays.next() else {
                    tracing::warn!(
                        %error,
                        attempts = SHARING_RETRY_DELAYS.len() + 1,
                        "arquivo em uso por outro programa; desistindo"
                    );
                    return Err(error);
                };
                tracing::debug!(
                    %error,
                    ?delay,
                    "arquivo em uso por outro programa; nova tentativa"
                );
                sleep(*delay);
            }
            Err(error) => return Err(error),
        }
    }
}

/// `fs::rename` por cima do destino com as novas tentativas de [`retry_on_sharing_error`].
///
/// No Windows, renomear por cima de um arquivo que outro programa mantém aberto sem
/// `FILE_SHARE_DELETE` dá "acesso negado": antivírus e indexadores fazem isso por instantes
/// com arquivos recém-gravados. Os leitores do próprio Warden abrem com `FILE_SHARE_DELETE`
/// (a `std` sempre; a libgit2 por configuração da `warden-versioning`), e com ele a `std`
/// completa a renomeação pela semântica POSIX, sem esperar ninguém fechar.
pub fn rename_with_retry(from: &Path, to: &Path) -> io::Result<()> {
    retry_on_sharing_error(|| fs::rename(from, to))
}

/// `fs::remove_file` com as novas tentativas de [`retry_on_sharing_error`]: no Windows, apagar
/// um arquivo aberto sem `FILE_SHARE_DELETE` dá violação de compartilhamento até ele ser
/// fechado. "Não encontrado" volta como erro, para o chamador decidir.
pub fn remove_file_with_retry(path: &Path) -> io::Result<()> {
    retry_on_sharing_error(|| fs::remove_file(path))
}

/// No Linux, a renomeação só fica garantida no disco depois do `fsync` da pasta. No Windows,
/// não há como abrir uma pasta para isso; o `MoveFileEx` já grava a entrada nova.
#[cfg(unix)]
fn sync_parent(path: &Path) -> io::Result<()> {
    match path.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => File::open(parent)?.sync_all(),
        _ => Ok(()),
    }
}

#[cfg(not(unix))]
#[allow(clippy::unnecessary_wraps)] // mesma assinatura da versão do Linux
fn sync_parent(_path: &Path) -> io::Result<()> {
    Ok(())
}

/// Apaga os `.warden-tmp` que sobraram em `dir` (não desce em subpastas). Devolve quantos
/// apagou. Pasta inexistente não é erro, nem um temporário que sumiu entre a listagem e a
/// remoção (outro processo, como o antivírus, o apagou antes): ele já está limpo.
pub fn remove_temp_files(dir: &Path) -> Result<usize, CoreError> {
    remove_temp_files_with(dir, |path| fs::remove_file(path))
}

/// [`remove_temp_files`] com a remoção trocável, para os testes simularem o arquivo sumindo
/// entre a listagem e a remoção.
fn remove_temp_files_with(
    dir: &Path,
    mut remove: impl FnMut(&Path) -> io::Result<()>,
) -> Result<usize, CoreError> {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(0),
        Err(error) => return Err(CoreError::io("listar", dir, error)),
    };
    let mut removed = 0;
    for entry in entries {
        let entry = entry.map_err(|e| CoreError::io("listar", dir, e))?;
        let is_temp = entry
            .file_name()
            .to_str()
            .is_some_and(|name| name.ends_with(TEMP_SUFFIX));
        let is_file = entry.file_type().is_ok_and(|kind| kind.is_file());
        if is_temp && is_file {
            let path = entry.path();
            match remove(&path) {
                Ok(()) => removed += 1,
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => return Err(CoreError::io("apagar", &path, error)),
            }
        }
    }
    Ok(removed)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(windows)]
    use crate::error::error_chain;
    use crate::error::{CoreErrorCode, DomainCode, DomainError as _};

    fn temp_files(dir: &Path) -> Vec<String> {
        fs::read_dir(dir)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .filter(|name| name.ends_with(TEMP_SUFFIX))
            .collect()
    }

    #[test]
    fn cria_e_substitui() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        atomic_write(&path, b"um").unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"um");
        atomic_write(&path, b"dois, mais longo").unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"dois, mais longo");
        atomic_write(&path, b"").unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"");
        assert!(temp_files(dir.path()).is_empty());
    }

    /// Critério 1 da F0-05: falha entre a escrita e a renomeação deixa o original intacto e
    /// nenhum `.warden-tmp`.
    #[test]
    fn f0_05_ca1_falha_antes_de_renomear_preserva_o_original() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("pack.toml");
        atomic_write(&path, b"original").unwrap();

        let guard = fault::arm("atomic_write.before_rename");
        let error = atomic_write(&path, b"novo").unwrap_err();
        assert_eq!(fault::hits("atomic_write.before_rename"), 1);
        drop(guard);

        assert_eq!(error.code(), DomainCode::Core(CoreErrorCode::Io));
        assert!(error.to_string().contains("renomear"), "{error}");
        assert_eq!(fs::read(&path).unwrap(), b"original");
        assert!(temp_files(dir.path()).is_empty());
    }

    #[test]
    fn falha_antes_de_gravar_preserva_o_original() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.txt");
        atomic_write(&path, b"original").unwrap();
        let _guard = fault::arm("atomic_write.before_write");
        assert!(atomic_write(&path, b"novo").is_err());
        assert_eq!(fs::read(&path).unwrap(), b"original");
        assert!(temp_files(dir.path()).is_empty());
    }

    /// Queda do processo no meio (o temporário sobra): a limpeza da abertura apaga o
    /// `.warden-tmp` e o original continua intacto.
    #[test]
    fn f0_05_ca1_limpeza_apaga_temporarios_de_queda() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("pack.toml");
        atomic_write(&path, b"original").unwrap();
        fs::write(
            dir.path().join("pack.toml.123-0.warden-tmp"),
            b"pela metade",
        )
        .unwrap();
        fs::write(dir.path().join("outro.warden-tmp"), b"x").unwrap();
        fs::create_dir(dir.path().join("pasta.warden-tmp")).unwrap();
        fs::write(dir.path().join("mantido.txt"), b"x").unwrap();

        assert_eq!(remove_temp_files(dir.path()).unwrap(), 2);
        assert!(
            temp_files(dir.path())
                .iter()
                .all(|name| name == "pasta.warden-tmp")
        );
        assert_eq!(fs::read(&path).unwrap(), b"original");
        assert!(dir.path().join("mantido.txt").exists());
    }

    /// Um temporário que outro processo apaga entre a listagem e a remoção não impede a
    /// abertura: conta como já limpo, e os outros temporários são apagados.
    #[test]
    fn limpeza_aceita_temporario_que_sumiu_no_meio() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("a.json.1-0.warden-tmp"), b"x").unwrap();
        fs::write(dir.path().join("b.json.1-1.warden-tmp"), b"x").unwrap();
        let mut vanished = None;
        let removed = remove_temp_files_with(dir.path(), |path| {
            if vanished.is_none() {
                // O primeiro some por fora antes da remoção do Warden.
                fs::remove_file(path).unwrap();
                vanished = Some(path.to_path_buf());
            }
            fs::remove_file(path)
        })
        .unwrap();
        assert!(vanished.is_some());
        assert_eq!(removed, 1);
        assert!(temp_files(dir.path()).is_empty());
    }

    /// Outros erros na remoção continuam sendo erro, com o caminho.
    #[test]
    fn limpeza_mantem_os_outros_erros_de_remocao() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("a.json.1-0.warden-tmp"), b"x").unwrap();
        let error = remove_temp_files_with(dir.path(), |_| {
            Err(io::Error::from(io::ErrorKind::PermissionDenied))
        })
        .unwrap_err();
        assert_eq!(error.code(), DomainCode::Core(CoreErrorCode::Io));
        assert!(error.to_string().contains("apagar"), "{error}");
        assert_eq!(temp_files(dir.path()).len(), 1);
    }

    #[test]
    fn limpeza_de_pasta_inexistente_nao_e_erro() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(
            remove_temp_files(&dir.path().join("nao-existe")).unwrap(),
            0
        );
    }

    #[test]
    fn pasta_inexistente_e_erro_de_io() {
        let dir = tempfile::tempdir().unwrap();
        let error = atomic_write(&dir.path().join("nao-existe").join("a.txt"), b"x").unwrap_err();
        assert_eq!(error.code(), DomainCode::Core(CoreErrorCode::Io));
        assert!(error.to_string().contains("criar"), "{error}");
    }

    #[test]
    fn caminho_sem_nome_e_recusado() {
        let error = atomic_write(Path::new(""), b"x").unwrap_err();
        assert_eq!(error.code(), DomainCode::Core(CoreErrorCode::Io));
    }

    #[test]
    fn escritas_concorrentes_no_mesmo_arquivo_nao_misturam_conteudo() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("concorrente.txt");
        let payloads: Vec<Vec<u8>> = (0..8u8).map(|i| vec![b'a' + i; 64 * 1024]).collect();
        std::thread::scope(|scope| {
            for payload in &payloads {
                let path = &path;
                scope.spawn(move || {
                    for _ in 0..5 {
                        // No Windows, duas renomeações simultâneas para o mesmo destino podem
                        // dar "acesso negado" mesmo após as novas tentativas; o que importa é
                        // que o arquivo final nunca fique misturado.
                        let _ = atomic_write(path, payload);
                    }
                });
            }
        });
        let final_contents = fs::read(&path).unwrap();
        assert!(payloads.contains(&final_contents));
        assert!(temp_files(dir.path()).is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn arquivo_privado_nasce_0600() {
        use std::os::unix::fs::PermissionsExt as _;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(".env");
        atomic_write_private(&path, b"A='b'\n").unwrap();
        let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
    }

    #[test]
    fn arquivo_privado_grava_o_conteudo() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(".env");
        atomic_write_private(&path, b"A='b'\n").unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"A='b'\n");
    }

    // -----------------------------------------------------------------------------------
    // Arquivo aberto por outro programa no Windows (ARCHITECTURE §15)
    // -----------------------------------------------------------------------------------

    fn sharing_error(code: i32) -> io::Error {
        io::Error::from_raw_os_error(code)
    }

    #[test]
    fn erro_de_compartilhamento_so_no_windows_e_so_os_tres_codigos() {
        for code in [5, 32, 33] {
            assert_eq!(
                is_windows_sharing_error(&sharing_error(code)),
                cfg!(windows),
                "código {code}"
            );
        }
        assert!(!is_windows_sharing_error(&sharing_error(2)));
        assert!(!is_windows_sharing_error(&io::Error::other("x")));
        assert!(!is_windows_sharing_error(&io::Error::from(
            io::ErrorKind::PermissionDenied
        )));
    }

    #[test]
    fn repete_so_o_erro_passageiro_e_espera_entre_as_tentativas() {
        let mut failures_left = 2;
        let mut slept = Vec::new();
        retry_with(
            || {
                if failures_left == 0 {
                    return Ok(());
                }
                failures_left -= 1;
                Err(sharing_error(32))
            },
            |_| true,
            |delay| slept.push(delay),
        )
        .unwrap();
        assert_eq!(slept, SHARING_RETRY_DELAYS[..2]);
    }

    #[test]
    fn desiste_depois_das_tentativas_e_devolve_o_erro_do_sistema() {
        let mut calls = 0;
        let mut slept = Vec::new();
        let error = retry_with(
            || {
                calls += 1;
                Err(sharing_error(32))
            },
            |_| true,
            |delay| slept.push(delay),
        )
        .unwrap_err();
        assert_eq!(calls, SHARING_RETRY_DELAYS.len() + 1);
        assert_eq!(slept, SHARING_RETRY_DELAYS);
        assert_eq!(error.raw_os_error(), Some(32));
        // Em torno de 600 ms no total: curto para o usuário, largo para o antivírus.
        let total: Duration = SHARING_RETRY_DELAYS.iter().sum();
        assert!((Duration::from_millis(500)..=Duration::from_millis(700)).contains(&total));
    }

    #[test]
    fn outro_erro_nao_e_repetido() {
        let mut calls = 0;
        let error = retry_with(
            || {
                calls += 1;
                Err(io::Error::new(io::ErrorKind::NotFound, "sumiu"))
            },
            |_| false,
            |_| panic!("não devia esperar"),
        )
        .unwrap_err();
        assert_eq!(calls, 1);
        assert_eq!(error.kind(), io::ErrorKind::NotFound);
    }

    #[test]
    fn fora_do_windows_nenhum_erro_e_passageiro() {
        if cfg!(windows) {
            return;
        }
        let mut calls = 0;
        let error = retry_on_sharing_error(|| {
            calls += 1;
            Err(sharing_error(32))
        })
        .unwrap_err();
        assert_eq!(calls, 1);
        assert_eq!(error.raw_os_error(), Some(32));
    }

    /// Como a libgit2 abre arquivos no Windows por padrão (`FILE_SHARE_READ |
    /// FILE_SHARE_WRITE`, sem `FILE_SHARE_DELETE`; `git_win32__createfile_sharemode`).
    #[cfg(windows)]
    fn open_like_libgit2(path: &Path) -> File {
        use std::os::windows::fs::OpenOptionsExt as _;
        const FILE_SHARE_READ: u32 = 0x1;
        const FILE_SHARE_WRITE: u32 = 0x2;
        OpenOptions::new()
            .read(true)
            .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
            .open(path)
            .unwrap()
    }

    /// Como a `std` abre arquivos no Windows (`FILE_SHARE_READ | FILE_SHARE_WRITE |
    /// FILE_SHARE_DELETE`), e como a libgit2 passa a abrir com a configuração da
    /// `warden-versioning`.
    #[cfg(windows)]
    fn open_with_share_delete(path: &Path) -> File {
        File::open(path).unwrap()
    }

    /// A prova do problema: renomear por cima de um arquivo que um leitor mantém aberto sem
    /// `FILE_SHARE_DELETE` falha com `ERROR_ACCESS_DENIED`; com `FILE_SHARE_DELETE`, a `std`
    /// completa a renomeação pela semântica POSIX enquanto o leitor continua lendo o antigo.
    #[cfg(windows)]
    #[test]
    fn windows_renomear_por_cima_de_leitor_sem_share_delete_falha_e_com_ele_funciona() {
        use std::io::Read as _;

        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("pack.toml");
        fs::write(&target, b"antigo").unwrap();
        let temp = dir.path().join("pack.toml.novo");
        fs::write(&temp, b"novo").unwrap();

        let reader = open_like_libgit2(&target);
        let error = fs::rename(&temp, &target).unwrap_err();
        assert_eq!(error.raw_os_error(), Some(5), "{error}");
        assert!(is_windows_sharing_error(&error));
        assert_eq!(fs::read(&target).unwrap(), b"antigo");
        drop(reader);

        let mut reader = open_with_share_delete(&target);
        fs::rename(&temp, &target).unwrap();
        assert_eq!(fs::read(&target).unwrap(), b"novo");
        let mut still_old = String::new();
        reader.read_to_string(&mut still_old).unwrap();
        assert_eq!(still_old, "antigo");
    }

    /// Um leitor sem `FILE_SHARE_DELETE` que solta o arquivo em 350 ms (antivírus, indexador)
    /// não derruba a gravação: as tentativas duram mais que isso. Com as esperas antigas
    /// (200 ms no total) a gravação falhava.
    #[cfg(windows)]
    #[test]
    fn windows_gravacao_espera_leitor_sem_share_delete_soltar_o_arquivo() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("index.toml");
        fs::write(&target, b"antigo").unwrap();
        let reader = open_like_libgit2(&target);
        let holder = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(350));
            drop(reader);
        });
        atomic_write(&target, b"novo").unwrap();
        holder.join().unwrap();
        assert_eq!(fs::read(&target).unwrap(), b"novo");
        assert!(temp_files(dir.path()).is_empty());
    }

    /// Um arquivo preso de verdade falha com o erro do sistema, em menos de um segundo, sem
    /// mexer no destino e sem deixar temporário: o erro não é escondido.
    #[cfg(windows)]
    #[test]
    fn windows_gravacao_com_leitor_preso_falha_rapido_com_o_erro_do_sistema() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("mods/preso.pw.toml");
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        fs::write(&target, b"antigo").unwrap();
        let reader = open_like_libgit2(&target);
        let started = std::time::Instant::now();
        let error = atomic_write(&target, b"novo").unwrap_err();
        let elapsed = started.elapsed();
        drop(reader);
        assert_eq!(error.code(), DomainCode::Core(CoreErrorCode::Io));
        assert!(error.to_string().contains("renomear para"), "{error}");
        assert!(
            error_chain(&error).contains("os error 5"),
            "{}",
            error_chain(&error)
        );
        assert!(elapsed < Duration::from_secs(3), "{elapsed:?}");
        assert_eq!(fs::read(&target).unwrap(), b"antigo");
        assert!(temp_files(target.parent().unwrap()).is_empty());
    }

    /// Apagar um arquivo que um leitor sem `FILE_SHARE_DELETE` solta em 350 ms funciona;
    /// o `fs::remove_file` cru falha na hora com `ERROR_SHARING_VIOLATION`.
    #[cfg(windows)]
    #[test]
    fn windows_remocao_espera_leitor_sem_share_delete_soltar_o_arquivo() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("modmenu.pw.toml");
        fs::write(&target, b"x").unwrap();
        let reader = open_like_libgit2(&target);
        let error = fs::remove_file(&target).unwrap_err();
        assert_eq!(error.raw_os_error(), Some(32), "{error}");
        assert!(is_windows_sharing_error(&error));
        let holder = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(350));
            drop(reader);
        });
        remove_file_with_retry(&target).unwrap();
        holder.join().unwrap();
        assert!(!target.exists());
    }
}
