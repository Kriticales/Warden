//! Instala, abre e desinstala o instalador NSIS e falha se sobrar algo fora dos dados do usuário (Windows).
//!
//! `cargo xtask verify-installer [--installer <arquivo>]` (A-02; ROADMAP: "instalar, abrir,
//! desinstalar sem resíduos fora dos dados do usuário"). Roda na CI do Windows, numa máquina
//! descartável, logo depois do `cargo tauri build --bundles nsis`; nunca no computador do dono.
//!
//! 1. Retrato do que existe antes: as pastas de primeiro nível de `%LOCALAPPDATA%`,
//!    `%APPDATA%`, `%ProgramFiles%`, `%ProgramFiles(x86)%`, do menu Iniciar e da Área de
//!    Trabalho, e as subchaves de `HKCU\Software` e das chaves de desinstalação (usuário e
//!    máquina).
//! 2. Instala em modo silencioso (`/S`), por usuário: a chave de desinstalação fica em `HKCU`,
//!    nada em `HKLM`, a pasta fica dentro de `%LOCALAPPDATA%`, com o executável e o sidecar do
//!    packwiz.
//! 3. Abre o Warden instalado e espera ele gravar o primeiro registro (`logs/warden.*.log` nos
//!    dados do app), sinal de que o executável instalado subiu de verdade; depois fecha.
//! 4. Desinstala em modo silencioso e espera a pasta e a chave de desinstalação sumirem.
//! 5. Retrato de novo: só podem ter surgido as pastas de dados do app (`dev.kriticales.warden`
//!    em `%APPDATA%` e `%LOCALAPPDATA%`), que a desinstalação mantém de propósito.
//!
//! As mensagens usam os nomes das variáveis (`%LOCALAPPDATA%\…`), nunca a pasta do usuário.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant, UNIX_EPOCH};

use anyhow::{Context as _, Result, bail, ensure};

use crate::preview::{
    IDENTIFIER, MAIN_BINARY, SIDECAR, UNINSTALL_KEY, install, newest_installer, reg_value,
    running_warden,
};
use crate::util::target_dir;

/// Argumentos de `cargo xtask verify-installer`.
#[derive(Debug, Default, clap::Args)]
pub struct Args {
    /// Instalador a testar (padrão: o mais novo em `<target>/release/bundle/nsis/`).
    #[arg(long)]
    pub installer: Option<PathBuf>,
}

/// Chave de desinstalação do usuário (onde o instalador por usuário grava a sua).
const USER_UNINSTALL: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Uninstall";

/// Chave de desinstalação da máquina (precisaria de administrador; não pode ganhar nada).
const MACHINE_UNINSTALL: &str = r"HKLM\Software\Microsoft\Windows\CurrentVersion\Uninstall";

/// Software do usuário (o gancho `nsis/hooks.nsh` apaga a chave que o modelo do Tauri deixava).
const USER_SOFTWARE: &str = r"HKCU\Software";

/// Quanto esperar o Warden instalado gravar o primeiro registro.
const OPEN_TIMEOUT: Duration = Duration::from_secs(60);

/// Quanto esperar o Warden fechar e a desinstalação terminar.
const CLOSE_TIMEOUT: Duration = Duration::from_secs(30);
const UNINSTALL_TIMEOUT: Duration = Duration::from_secs(120);

/// Lugares observados: rótulo exibido (sem a pasta do usuário) → nomes de primeiro nível.
pub type Inventory = BTreeMap<String, BTreeSet<String>>;

/// O que surgiu entre os dois retratos, fora do permitido (`allowed`: nomes que podem surgir
/// em qualquer lugar observado). Um lugar que só existe no segundo retrato conta inteiro.
pub fn residues(before: &Inventory, after: &Inventory, allowed: &[&str]) -> Vec<String> {
    let mut found = Vec::new();
    for (place, names) in after {
        let old = before.get(place);
        for name in names {
            let is_new = old.is_none_or(|old| !old.contains(name));
            let is_allowed = allowed
                .iter()
                .any(|allowed| allowed.eq_ignore_ascii_case(name));
            if is_new && !is_allowed {
                found.push(format!(r"{place}\{name}"));
            }
        }
    }
    found
}

/// Subchaves diretas de `key` na saída do `reg query <key>` (as linhas que começam com a
/// própria chave seguida de `\`), só com o último nome.
pub fn parse_reg_subkeys(output: &str, key: &str) -> BTreeSet<String> {
    // O `reg` escreve a raiz por extenso (`HKEY_CURRENT_USER`), mesmo pedida abreviada.
    let rest = |prefix: &str| key.strip_prefix(prefix).map(str::to_owned);
    let full = rest(r"HKCU\")
        .map(|rest| format!(r"HKEY_CURRENT_USER\{rest}"))
        .or_else(|| rest(r"HKLM\").map(|rest| format!(r"HKEY_LOCAL_MACHINE\{rest}")))
        .unwrap_or_else(|| key.to_owned());
    output
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            let name = line
                .get(full.len()..)
                .filter(|_| line[..full.len()].eq_ignore_ascii_case(&full))?
                .strip_prefix('\\')?;
            (!name.is_empty() && !name.contains('\\')).then(|| name.to_owned())
        })
        .collect()
}

/// Se o log mais recente do app existe e não está vazio (o app subiu e gravou o primeiro registro).
pub fn has_log(dir: &Path) -> bool {
    std::fs::read_dir(dir).is_ok_and(|entries| {
        entries.filter_map(Result::ok).any(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            let path = Path::new(&name);
            name.starts_with("warden.")
                && path
                    .extension()
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("log"))
                // `fs::metadata` abre o arquivo: o tamanho da listagem da pasta (`DirEntry`) só é
                // atualizado no NTFS quando quem grava fecha o arquivo, e o app ainda está aberto.
                && std::fs::metadata(entry.path()).is_ok_and(|meta| meta.len() > 0)
        })
    })
}

/// Se `path` fica dentro de `base`, sem diferenciar maiúsculas (o registro e as variáveis de
/// ambiente do Windows podem trazer a mesma pasta com outra caixa) nem o tipo da barra.
pub fn is_inside(path: &Path, base: &Path) -> bool {
    let normalize = |path: &Path| {
        path.to_string_lossy()
            .replace('/', "\\")
            .trim_end_matches('\\')
            .to_lowercase()
    };
    let (path, base) = (normalize(path), normalize(base));
    path.strip_prefix(&base)
        .is_some_and(|rest| rest.starts_with('\\'))
}

fn env_dir(name: &str) -> Option<PathBuf> {
    std::env::var_os(name).map(PathBuf::from)
}

/// Pastas observadas: rótulo → caminho.
fn watched_dirs() -> Vec<(String, PathBuf)> {
    let mut dirs = Vec::new();
    for name in [
        "LOCALAPPDATA",
        "APPDATA",
        "ProgramFiles",
        "ProgramFiles(x86)",
        "PUBLIC",
    ] {
        if let Some(dir) = env_dir(name) {
            dirs.push((format!("%{name}%"), dir));
        }
    }
    if let Some(appdata) = env_dir("APPDATA") {
        let programs = appdata.join(r"Microsoft\Windows\Start Menu\Programs");
        dirs.push((r"%APPDATA%\…\Start Menu\Programs".to_owned(), programs));
    }
    if let Some(profile) = env_dir("USERPROFILE") {
        dirs.push((r"%USERPROFILE%\Desktop".to_owned(), profile.join("Desktop")));
    }
    if let Some(public) = env_dir("PUBLIC") {
        dirs.push((r"%PUBLIC%\Desktop".to_owned(), public.join("Desktop")));
    }
    dirs
}

fn names_in(dir: &Path) -> Result<BTreeSet<String>> {
    match std::fs::read_dir(dir) {
        Ok(entries) => Ok(entries
            .filter_map(Result::ok)
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .collect()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(BTreeSet::new()),
        Err(error) => Err(error).context("falha ao listar uma pasta observada"),
    }
}

fn reg_subkeys(key: &str) -> Result<BTreeSet<String>> {
    let output = Command::new("reg")
        .args(["query", key])
        .stderr(Stdio::null())
        .output()
        .context("não foi possível executar `reg query`")?;
    // Chave inexistente: o `reg` sai com erro; para o retrato, é uma chave vazia.
    Ok(parse_reg_subkeys(
        &String::from_utf8_lossy(&output.stdout),
        key,
    ))
}

fn inventory() -> Result<Inventory> {
    let mut found = Inventory::new();
    for (label, dir) in watched_dirs() {
        found.insert(label, names_in(&dir)?);
    }
    for key in [USER_SOFTWARE, USER_UNINSTALL, MACHINE_UNINSTALL] {
        found.insert(key.to_owned(), reg_subkeys(key)?);
    }
    Ok(found)
}

/// Espera `done` ficar verdadeiro, conferindo a cada meio segundo.
fn wait_until(timeout: Duration, mut done: impl FnMut() -> Result<bool>) -> Result<bool> {
    let start = Instant::now();
    loop {
        if done()? {
            return Ok(true);
        }
        if start.elapsed() >= timeout {
            return Ok(false);
        }
        std::thread::sleep(Duration::from_millis(500));
    }
}

/// Pastas de dados do app (o que a desinstalação mantém): rótulo → caminho.
fn data_dirs() -> Vec<(String, PathBuf)> {
    ["APPDATA", "LOCALAPPDATA"]
        .into_iter()
        .filter_map(|name| Some((format!(r"%{name}%\{IDENTIFIER}"), env_dir(name)?.join(IDENTIFIER))))
        .collect()
}

/// O que há em cada pasta de registros, para a mensagem de erro (só nomes e tamanhos).
pub fn describe_logs(dirs: &[(String, PathBuf)]) -> String {
    dirs.iter()
        .map(|(label, dir)| {
            let Ok(entries) = std::fs::read_dir(dir) else {
                return format!(r"{label}\logs: não existe");
            };
            let mut files: Vec<String> = entries
                .filter_map(Result::ok)
                .map(|entry| {
                    let size = std::fs::metadata(entry.path()).map_or(0, |meta| meta.len());
                    format!("{} ({size} bytes)", entry.file_name().to_string_lossy())
                })
                .collect();
            files.sort();
            if files.is_empty() {
                format!(r"{label}\logs: vazia")
            } else {
                format!(r"{label}\logs: {}", files.join(", "))
            }
        })
        .collect::<Vec<_>>()
        .join("; ")
}

fn open_and_close(exe: &Path) -> Result<()> {
    let mut child = Command::new(exe)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .context("não foi possível abrir o Warden instalado")?;
    let logs: Vec<(String, PathBuf)> = data_dirs()
        .into_iter()
        .map(|(label, dir)| (label, dir.join("logs")))
        .collect();
    let opened = wait_until(OPEN_TIMEOUT, || {
        if let Some(status) = child.try_wait()? {
            bail!(
                "o Warden instalado fechou sozinho ao abrir ({status}); {}",
                describe_logs(&logs)
            );
        }
        Ok(logs.iter().any(|(_, dir)| has_log(dir)))
    })?;
    ensure!(
        opened,
        "o Warden instalado não gravou nenhum registro em {} s ({})",
        OPEN_TIMEOUT.as_secs(),
        describe_logs(&logs)
    );
    ensure!(
        !running_warden()?.is_empty(),
        "o Warden instalado não aparece no tasklist"
    );
    println!("verify-installer: o Warden instalado abriu e gravou o primeiro registro.");

    let _ = Command::new("taskkill")
        .args(["/F", "/T", "/IM", MAIN_BINARY])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    let _ = child.wait();
    let closed = wait_until(CLOSE_TIMEOUT, || Ok(running_warden()?.is_empty()))?;
    ensure!(closed, "o Warden não fechou em {} s", CLOSE_TIMEOUT.as_secs());
    Ok(())
}

fn uninstall(dir: &Path) -> Result<()> {
    let uninstaller = dir.join("uninstall.exe");
    ensure!(
        uninstaller.is_file(),
        r"faltou o desinstalador (uninstall.exe) na pasta instalada"
    );
    // O desinstalador do NSIS se copia para a pasta temporária e devolve na hora: quem diz que
    // terminou é a pasta e a chave de desinstalação sumirem.
    let status = Command::new(&uninstaller)
        .arg("/S")
        .status()
        .context("não foi possível executar o desinstalador")?;
    ensure!(status.success(), "o desinstalador terminou com {status}");
    let gone = wait_until(UNINSTALL_TIMEOUT, || {
        Ok(!dir.exists() && reg_value(UNINSTALL_KEY, "DisplayName").is_none())
    })?;
    ensure!(
        gone,
        "depois de {} s a pasta instalada ou a chave de desinstalação continuam lá",
        UNINSTALL_TIMEOUT.as_secs()
    );
    println!("verify-installer: desinstalado (pasta e chave de desinstalação removidas).");
    Ok(())
}

/// `cargo xtask verify-installer`.
pub fn run(args: &Args) -> Result<()> {
    if !cfg!(windows) {
        bail!("o verify-installer roda só no Windows (instalador NSIS)");
    }
    let installer = match &args.installer {
        Some(path) => path.clone(),
        None => newest_installer(
            &target_dir()?.join("release").join("bundle").join("nsis"),
            UNIX_EPOCH,
        )?,
    };
    ensure!(
        reg_value(UNINSTALL_KEY, "DisplayName").is_none() && running_warden()?.is_empty(),
        "o Warden já está instalado ou aberto nesta máquina; o verify-installer precisa de uma \
         máquina limpa (rode na CI)"
    );
    let before = inventory()?;

    install(&installer, None)?;
    let location = reg_value(UNINSTALL_KEY, "InstallLocation")
        .context("o instalador não gravou a chave de desinstalação do usuário (HKCU)")?;
    let dir = PathBuf::from(location);
    let local = env_dir("LOCALAPPDATA").context("LOCALAPPDATA não definida")?;
    ensure!(
        is_inside(&dir, &local),
        r"a instalação não ficou dentro de %LOCALAPPDATA% (instalação por usuário)"
    );
    for file in [MAIN_BINARY, SIDECAR] {
        ensure!(
            dir.join(file).is_file(),
            r"faltou {file} na pasta instalada (%LOCALAPPDATA%\…)"
        );
    }
    let machine = reg_subkeys(MACHINE_UNINSTALL)?;
    ensure!(
        machine == before[MACHINE_UNINSTALL],
        "o instalador mexeu na chave de desinstalação da máquina (HKLM): pediria administrador"
    );
    println!("verify-installer: instalado por usuário, sem administrador.");

    open_and_close(&dir.join(MAIN_BINARY))?;
    uninstall(&dir)?;

    let after = inventory()?;
    let left = residues(&before, &after, &[IDENTIFIER]);
    if !left.is_empty() {
        bail!(
            "sobrou depois de desinstalar (fora dos dados do usuário):\n  - {}",
            left.join("\n  - ")
        );
    }
    println!(
        "verify-installer: nada sobrou fora dos dados do usuário (%APPDATA% e %LOCALAPPDATA%\\{IDENTIFIER})."
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inventory_of(list: &[(&str, &[&str])]) -> Inventory {
        list.iter()
            .map(|(place, names)| {
                (
                    (*place).to_owned(),
                    names.iter().map(|name| (*name).to_owned()).collect(),
                )
            })
            .collect()
    }

    #[test]
    fn residuo_e_o_que_surgiu_fora_dos_dados_do_app() {
        let before = inventory_of(&[
            ("%LOCALAPPDATA%", &["Microsoft", "Temp"]),
            (USER_SOFTWARE, &["Microsoft"]),
        ]);
        let after = inventory_of(&[
            ("%LOCALAPPDATA%", &["Microsoft", "Temp", "dev.kriticales.warden", "Warden"]),
            (USER_SOFTWARE, &["Microsoft", "Kriticales"]),
            (r"%PUBLIC%\Desktop", &["Warden.lnk"]),
        ]);
        assert_eq!(
            residues(&before, &after, &[IDENTIFIER]),
            [
                r"%LOCALAPPDATA%\Warden",
                r"%PUBLIC%\Desktop\Warden.lnk",
                r"HKCU\Software\Kriticales",
            ]
        );
    }

    #[test]
    fn nada_novo_nada_sobra_e_sumir_nao_e_residuo() {
        let before = inventory_of(&[("%APPDATA%", &["Code", "npm"])]);
        let after = inventory_of(&[("%APPDATA%", &["npm", "DEV.KRITICALES.WARDEN"])]);
        assert!(residues(&before, &after, &[IDENTIFIER]).is_empty());
    }

    #[test]
    fn subchaves_do_reg_query() {
        let output = "\r\nHKEY_CURRENT_USER\\Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\r\n    \
                      (Padrão)    REG_SZ    \r\n\r\n\
                      HKEY_CURRENT_USER\\Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\Warden\r\n\
                      HKEY_CURRENT_USER\\Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\{ABC-123}\r\n";
        assert_eq!(
            parse_reg_subkeys(output, USER_UNINSTALL),
            ["Warden", "{ABC-123}"].map(String::from).into()
        );
        let machine = "HKEY_LOCAL_MACHINE\\Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\Git_is1\n";
        assert_eq!(
            parse_reg_subkeys(machine, MACHINE_UNINSTALL),
            ["Git_is1"].map(String::from).into()
        );
        assert!(parse_reg_subkeys("ERRO: chave não encontrada", USER_SOFTWARE).is_empty());
    }

    #[test]
    fn log_so_conta_quando_o_app_gravou_algo() {
        let dir = tempfile::tempdir().unwrap();
        assert!(!has_log(&dir.path().join("logs")));
        std::fs::write(dir.path().join("warden.2026-10-13.log"), "").unwrap();
        std::fs::write(dir.path().join("outro.log"), "x").unwrap();
        assert!(!has_log(dir.path()));
        std::fs::write(dir.path().join("warden.2026-10-13.log"), "INFO configurações lidas").unwrap();
        assert!(has_log(dir.path()));
    }

    #[test]
    fn pasta_instalada_dentro_de_localappdata_sem_diferenciar_maiusculas() {
        let local = Path::new(r"C:\Users\Runner\AppData\Local");
        assert!(is_inside(Path::new(r"c:\users\runner\appdata\local\Warden"), local));
        assert!(is_inside(Path::new("C:/Users/Runner/AppData/Local/Warden/"), local));
        assert!(!is_inside(Path::new(r"C:\Users\Runner\AppData\LocalLow\Warden"), local));
        assert!(!is_inside(Path::new(r"C:\Program Files\Warden"), local));
        assert!(!is_inside(local, local));
    }

    #[test]
    fn descricao_dos_registros_so_com_rotulo_nome_e_tamanho() {
        let dir = tempfile::tempdir().unwrap();
        let logs = dir.path().join("logs");
        std::fs::create_dir(&logs).unwrap();
        let dirs = vec![
            ("%LOCALAPPDATA%\\x".to_owned(), logs.clone()),
            ("%APPDATA%\\x".to_owned(), dir.path().join("nao-existe")),
        ];
        assert_eq!(
            describe_logs(&dirs),
            "%LOCALAPPDATA%\\x\\logs: vazia; %APPDATA%\\x\\logs: não existe"
        );
        std::fs::write(logs.join("warden.2026-10-13.log"), "abc").unwrap();
        let text = describe_logs(&dirs);
        assert!(text.starts_with("%LOCALAPPDATA%\\x\\logs: warden.2026-10-13.log (3 bytes)"));
        assert!(!text.contains(&*dir.path().to_string_lossy()), "{text}");
    }

    #[test]
    fn lugares_observados_nao_levam_a_pasta_do_usuario_no_rotulo() {
        for (label, _) in watched_dirs() {
            assert!(label.starts_with('%'), "{label}");
        }
    }
}
