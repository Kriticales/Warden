//! Onde fica o `hsperfdata` de uma JVM (S-R5-2 §2, §6 e §8; ARCHITECTURE §7.7).
//!
//! A JVM grava `<pasta temporária>/hsperfdata_<usuário>/<pid>`. No Windows a pasta é a do
//! `GetTempPath` do **processo do jogo** (`TMP`, depois `TEMP`) e o usuário é o `USERNAME`
//! dele, convertido para a code page ANSI ("João" continua "João", "Łukasz" vira "Lukasz"). Por
//! isso o Warden nunca monta o nome a partir do usuário: procura as pastas `hsperfdata_*` que
//! têm o arquivo `<pid>` (como o `jstat` faz) e usa o sufixo da pasta no nome do *file
//! mapping* (`hsperfdata_<sufixo>_<pid>`). No Linux a JVM usa sempre `/tmp`.
//!
//! Com mais de uma pasta candidata (um arquivo velho de outra sessão com o mesmo pid), a lista
//! vem da mais nova para a mais velha; quem abre aceita a primeira que funcionar.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// Prefixo das pastas.
pub const DIR_PREFIX: &str = "hsperfdata_";

/// Um `hsperfdata` encontrado.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Location {
    /// O arquivo `<pasta>/<pid>`.
    pub file: PathBuf,
    /// Nome do *file mapping* do Windows: `hsperfdata_<sufixo da pasta>_<pid>`.
    pub mapping_name: OsString,
}

/// As pastas temporárias em que a JVM do jogo pode ter gravado, sem repetir.
///
/// `game_env` são as variáveis que o Warden acrescentou ao processo do jogo; o resto o jogo
/// herda do Warden. No Windows: `TMP`, `TEMP` (nessa ordem, como o `GetTempPath`) e a pasta
/// temporária do Warden. No Linux: `/tmp` (fixa na `HotSpot`) e a do Warden.
#[must_use]
pub fn game_temp_dirs(game_env: &[(String, String)]) -> Vec<PathBuf> {
    let lookup = |name: &str| -> Option<PathBuf> {
        game_env
            .iter()
            .rev()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| PathBuf::from(value))
            .or_else(|| std::env::var_os(name).map(PathBuf::from))
            .filter(|path| !path.as_os_str().is_empty())
    };
    let mut dirs: Vec<PathBuf> = Vec::new();
    if cfg!(windows) {
        dirs.extend(lookup("TMP"));
        dirs.extend(lookup("TEMP"));
    } else {
        dirs.push(PathBuf::from("/tmp"));
    }
    dirs.push(std::env::temp_dir());
    let mut unique: Vec<PathBuf> = Vec::new();
    for dir in dirs {
        if !unique.contains(&dir) {
            unique.push(dir);
        }
    }
    unique
}

/// Os `hsperfdata` do processo `pid` nas pastas dadas, do mais novo para o mais velho.
#[must_use]
pub fn candidates(temp_dirs: &[PathBuf], pid: u32) -> Vec<Location> {
    let name = pid.to_string();
    let mut found: Vec<(SystemTime, Location)> = Vec::new();
    for temp in temp_dirs {
        let Ok(entries) = std::fs::read_dir(temp) else {
            continue;
        };
        for entry in entries.flatten() {
            let dir_name = entry.file_name();
            let Some(suffix) = dir_name.to_str().and_then(|n| n.strip_prefix(DIR_PREFIX)) else {
                continue;
            };
            let file = entry.path().join(&name);
            let Some(when) = file_time(&file) else {
                continue;
            };
            if found.iter().any(|(_, known)| known.file == file) {
                continue;
            }
            let mut mapping_name = OsString::from(DIR_PREFIX);
            mapping_name.push(suffix);
            mapping_name.push("_");
            mapping_name.push(&name);
            found.push((when, Location { file, mapping_name }));
        }
    }
    // Do mais novo para o mais velho; no empate, a ordem das pastas.
    found.sort_by_key(|(when, _)| std::cmp::Reverse(*when));
    found.into_iter().map(|(_, location)| location).collect()
}

/// Quando o arquivo mudou pela última vez (ou foi criado, onde o sistema não guarda a mudança).
/// O arquivo de uma JVM viva foi criado depois de qualquer arquivo velho com o mesmo pid ter
/// mudado pela última vez (dois processos vivos não têm o mesmo pid). `None` se não for um
/// arquivo.
fn file_time(file: &Path) -> Option<SystemTime> {
    let meta = std::fs::metadata(file).ok()?;
    if !meta.is_file() {
        return None;
    }
    Some(
        meta.modified()
            .or_else(|_| meta.created())
            .unwrap_or(SystemTime::UNIX_EPOCH),
    )
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use std::time::Duration;

    use super::*;

    fn touch(path: &Path, when: SystemTime) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let file = std::fs::File::create(path).unwrap();
        file.set_modified(when).unwrap();
    }

    #[test]
    fn acha_pastas_com_acento_e_monta_o_nome_pelo_sufixo() {
        let temp = tempfile::tempdir().unwrap();
        let now = SystemTime::now();
        touch(&temp.path().join("hsperfdata_João").join("4242"), now);
        touch(
            &temp.path().join("hsperfdata_Lukasz").join("777"),
            now - Duration::from_secs(5),
        );
        // Outra pasta qualquer e um pid que não é o pedido.
        touch(&temp.path().join("outra").join("4242"), now);
        touch(&temp.path().join("hsperfdata_Ana").join("1"), now);

        let found = candidates(&[temp.path().to_path_buf()], 4242);
        assert_eq!(found.len(), 1);
        assert_eq!(
            found[0].mapping_name,
            OsString::from("hsperfdata_João_4242")
        );
        assert_eq!(
            found[0].file,
            temp.path().join("hsperfdata_João").join("4242")
        );

        let found = candidates(&[temp.path().to_path_buf()], 777);
        assert_eq!(
            found[0].mapping_name,
            OsString::from("hsperfdata_Lukasz_777")
        );
    }

    #[test]
    fn varias_pastas_do_mais_novo_para_o_mais_velho() {
        let first = tempfile::tempdir().unwrap();
        let second = tempfile::tempdir().unwrap();
        let now = SystemTime::now();
        touch(
            &first.path().join("hsperfdata_velho").join("10"),
            now - Duration::from_secs(3600),
        );
        touch(&second.path().join("hsperfdata_novo").join("10"), now);
        touch(
            &second.path().join("hsperfdata_meio").join("10"),
            now - Duration::from_secs(60),
        );
        let names: Vec<OsString> = candidates(
            &[first.path().to_path_buf(), second.path().to_path_buf()],
            10,
        )
        .into_iter()
        .map(|location| location.mapping_name)
        .collect();
        assert_eq!(
            names,
            [
                "hsperfdata_novo_10",
                "hsperfdata_meio_10",
                "hsperfdata_velho_10"
            ]
            .map(OsString::from)
        );
    }

    #[test]
    fn pasta_inexistente_ou_pid_que_e_pasta_nao_conta() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(temp.path().join("hsperfdata_x").join("5")).unwrap();
        assert!(candidates(&[temp.path().join("nao-existe"), temp.path().into()], 5).is_empty());
    }

    #[test]
    fn pastas_temporarias_do_jogo_sem_repetir() {
        let dirs = game_temp_dirs(&[]);
        assert!(dirs.contains(&std::env::temp_dir()));
        let mut unique = dirs.clone();
        unique.dedup();
        assert_eq!(unique, dirs);
        #[cfg(windows)]
        {
            let dirs = game_temp_dirs(&[("TMP".into(), r"C:\jogo\tmp".into())]);
            assert_eq!(dirs[0], PathBuf::from(r"C:\jogo\tmp"));
        }
        #[cfg(not(windows))]
        assert_eq!(dirs[0], PathBuf::from("/tmp"));
    }
}
