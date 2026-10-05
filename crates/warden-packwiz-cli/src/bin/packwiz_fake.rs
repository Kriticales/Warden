//! Executável falso do packwiz, só para os testes de processo da crate (árvore de processos,
//! tempo-limite, ambiente e argv recebidos). Não entra no app.
//!
//! O comportamento vem do arquivo `fake.txt` na pasta de trabalho (a pasta do "pack"), uma
//! opção por linha:
//!
//! - `mode=dump`: grava `fake-dump.txt` com os argumentos (`arg=`), as variáveis de ambiente
//!   (`env=NOME=valor`) e a entrada padrão (`stdin=`);
//! - `mode=sleep`: cria um filho (que trava `fake-lock.txt` e dorme), espera ele ficar pronto,
//!   imprime `filho pronto` e dorme;
//! - `mode=orphan`: cria o mesmo filho, que herda a saída, e sai logo com código 0;
//! - `print=<texto>` / `stderr=<texto>`: linhas impressas (`\e` vira ESC);
//! - `file=<caminho>`: grava um metafile válido da CurseForge (como o `curseforge add`);
//! - `exit=<código>`: código de saída (padrão 0).

use std::error::Error;
use std::fmt::Write as _;
use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::thread;
use std::time::{Duration, Instant};

const CHILD_FLAG: &str = "--fake-child";
const LOCK_FILE: &str = "fake-lock.txt";
const READY_FILE: &str = "fake-ready.txt";
const SLEEP: Duration = Duration::from_secs(120);

fn main() -> ExitCode {
    match run() {
        Ok(code) => ExitCode::from(code),
        Err(error) => {
            let _ = writeln!(io::stderr(), "packwiz-fake: {error}");
            ExitCode::from(99)
        }
    }
}

fn run() -> Result<u8, Box<dyn Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some(CHILD_FLAG) {
        child(Path::new(args.get(1).ok_or("pasta do filho ausente")?))?;
        return Ok(0);
    }
    let cwd = std::env::current_dir()?;
    let config = fs::read_to_string(cwd.join("fake.txt")).unwrap_or_default();
    let mut mode = "";
    let mut exit = 0_u8;
    let mut stdout = io::stdout();
    let mut stderr = io::stderr();
    for line in config.lines() {
        let (key, value) = line.split_once('=').unwrap_or((line, ""));
        let text = value.replace("\\e", "\u{1b}");
        match key {
            "mode" => mode = value,
            "exit" => exit = value.parse()?,
            "print" => writeln!(stdout, "{text}")?,
            "stderr" => writeln!(stderr, "{text}")?,
            "file" => write_metafile(&cwd.join(value))?,
            _ => {}
        }
    }
    stdout.flush()?;
    match mode {
        "dump" => dump(&cwd, &args)?,
        "sleep" => {
            spawn_child(&cwd)?;
            writeln!(stdout, "filho pronto")?;
            stdout.flush()?;
            thread::sleep(SLEEP);
        }
        "orphan" => {
            spawn_child(&cwd)?;
            writeln!(stdout, "saindo sem esperar o filho")?;
        }
        _ => {}
    }
    Ok(exit)
}

fn dump(cwd: &Path, args: &[String]) -> Result<(), Box<dyn Error>> {
    let mut text = String::new();
    for arg in args {
        writeln!(text, "arg={arg}")?;
    }
    let mut env: Vec<(String, String)> = std::env::vars().collect();
    env.sort();
    for (name, value) in env {
        writeln!(text, "env={name}={value}")?;
    }
    let mut stdin = String::new();
    io::stdin().read_to_string(&mut stdin)?;
    writeln!(text, "stdin={}", stdin.escape_debug())?;
    fs::write(cwd.join("fake-dump.txt"), text)?;
    Ok(())
}

/// Grava um metafile válido (como o `curseforge add` faria).
fn write_metafile(path: &Path) -> Result<(), Box<dyn Error>> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(
        path,
        "name = \"Falso\"\nfilename = \"falso.jar\"\nside = \"both\"\n\n[download]\nhash-format = \"sha1\"\nhash = \"aa\"\nmode = \"metadata:curseforge\"\n\n[update]\n[update.curseforge]\nfile-id = 2\nproject-id = 1\n",
    )?;
    Ok(())
}

/// Cria o filho e espera ele travar o arquivo.
fn spawn_child(cwd: &Path) -> Result<(), Box<dyn Error>> {
    let ready = cwd.join(READY_FILE);
    let _ = fs::remove_file(&ready);
    Command::new(std::env::current_exe()?)
        .arg(CHILD_FLAG)
        .arg(cwd)
        .spawn()?;
    let started = Instant::now();
    while !ready.exists() {
        if started.elapsed() > Duration::from_secs(20) {
            return Err("o filho não ficou pronto".into());
        }
        thread::sleep(Duration::from_millis(10));
    }
    Ok(())
}

/// O filho: trava `fake-lock.txt` (no Windows, sem compartilhamento: ninguém abre nem apaga
/// enquanto ele viver), grava o próprio PID em `fake-ready.txt` e dorme.
fn child(dir: &Path) -> Result<(), Box<dyn Error>> {
    let lock: PathBuf = dir.join(LOCK_FILE);
    let mut options = fs::OpenOptions::new();
    options.create(true).truncate(true).write(true);
    #[cfg(windows)]
    std::os::windows::fs::OpenOptionsExt::share_mode(&mut options, 0);
    let _held = options.open(&lock)?;
    let pending = dir.join("fake-ready.tmp");
    fs::write(&pending, std::process::id().to_string())?;
    fs::rename(pending, dir.join(READY_FILE))?;
    thread::sleep(SLEEP);
    Ok(())
}
