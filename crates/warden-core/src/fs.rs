//! Escrita atômica de arquivos (ARCHITECTURE §6.5 e §14).
//!
//! [`atomic_write`] grava num arquivo temporário ao lado do destino (sufixo `.warden-tmp`),
//! força os dados para o disco (`fsync`) e só então renomeia por cima do destino. Quem lê o
//! arquivo vê o conteúdo antigo ou o novo inteiro, nunca um pedaço. Se algo falhar antes da
//! renomeação, o destino fica intacto e o temporário é apagado; se o processo morrer no meio,
//! sobra um `.warden-tmp`, que [`remove_temp_files`] apaga na próxima abertura.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Write as _};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

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

/// No Windows, antivírus e indexadores abrem o arquivo recém-gravado por alguns instantes, e a
/// renomeação falha com "acesso negado". Algumas tentativas curtas resolvem; em outros erros,
/// não insiste.
fn rename_with_retry(from: &Path, to: &Path) -> io::Result<()> {
    const ATTEMPTS: u32 = 5;
    let mut attempt = 1;
    loop {
        match fs::rename(from, to) {
            Ok(()) => return Ok(()),
            Err(error)
                if cfg!(windows)
                    && error.kind() == io::ErrorKind::PermissionDenied
                    && attempt < ATTEMPTS =>
            {
                std::thread::sleep(std::time::Duration::from_millis(20 * u64::from(attempt)));
                attempt += 1;
            }
            Err(error) => return Err(error),
        }
    }
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
}
