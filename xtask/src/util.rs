//! Utilitários comuns do xtask: raiz do workspace, execução de processos e avisos.
//!
//! Processos sempre com argv em vetor, nunca por `cmd /c` ou `sh -c` (QUALITY §9 e §13.7).
//! O xtask imprime o programa e os argumentos de cada comando, nunca o ambiente.

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use anyhow::{Context as _, Result, bail};

/// Limite do caminho da pasta `target` (QUALITY §13.3).
pub const TARGET_PATH_LIMIT: usize = 100;

/// Raiz do workspace (a pasta acima de `xtask/`).
pub fn workspace_root() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .parent()
        .map_or(manifest_dir.clone(), Path::to_path_buf)
}

/// Pasta do app desktop (frontend e `src-tauri`).
pub fn desktop_dir() -> PathBuf {
    workspace_root().join("apps").join("desktop")
}

/// Executável do cargo que rodou o xtask (o da toolchain fixada).
pub fn cargo_program() -> OsString {
    std::env::var_os("CARGO").unwrap_or_else(|| OsString::from("cargo"))
}

/// Resolve um programa pelo `PATH`, inclusive `.cmd` no Windows (o `pnpm` é `pnpm.cmd`).
pub fn find_program(name: &str) -> Result<PathBuf> {
    which::which(name).with_context(|| {
        format!("`{name}` não foi encontrado no PATH; confira os pré-requisitos no README.md")
    })
}

/// Comando a executar, com a descrição que o xtask imprime antes.
pub struct Cmd {
    program: OsString,
    args: Vec<OsString>,
    cwd: PathBuf,
    envs: Vec<(OsString, OsString)>,
}

impl Cmd {
    /// Comando com a raiz do workspace como pasta atual.
    pub fn new(program: impl AsRef<OsStr>) -> Self {
        Self {
            program: program.as_ref().to_os_string(),
            args: Vec::new(),
            cwd: workspace_root(),
            envs: Vec::new(),
        }
    }

    /// `cargo` da toolchain fixada.
    pub fn cargo() -> Self {
        Self::new(cargo_program())
    }

    /// `pnpm`, resolvido pelo `PATH`.
    pub fn pnpm() -> Result<Self> {
        Ok(Self::new(find_program("pnpm")?))
    }

    /// Acrescenta argumentos.
    pub fn args<I, S>(mut self, args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        self.args
            .extend(args.into_iter().map(|arg| arg.as_ref().to_os_string()));
        self
    }

    /// Define a pasta atual.
    pub fn cwd(mut self, cwd: impl Into<PathBuf>) -> Self {
        self.cwd = cwd.into();
        self
    }

    /// Define uma variável de ambiente (o valor nunca é impresso).
    pub fn env(mut self, key: impl AsRef<OsStr>, value: impl AsRef<OsStr>) -> Self {
        self.envs
            .push((key.as_ref().to_os_string(), value.as_ref().to_os_string()));
        self
    }

    /// Texto do comando para o terminal: programa (só o nome) e argumentos.
    pub fn display(&self) -> String {
        let program = Path::new(&self.program)
            .file_stem()
            .unwrap_or(&self.program)
            .to_string_lossy()
            .into_owned();
        let mut text = program;
        for arg in &self.args {
            text.push(' ');
            text.push_str(&arg.to_string_lossy());
        }
        text
    }

    fn command(&self) -> Command {
        let mut command = Command::new(&self.program);
        command.args(&self.args).current_dir(&self.cwd);
        for (key, value) in &self.envs {
            command.env(key, value);
        }
        command
    }

    /// Executa com a saída no terminal; erro se o código de saída não for 0.
    pub fn run(&self) -> Result<()> {
        println!("> {}", self.display());
        let status = self
            .command()
            .status()
            .with_context(|| format!("não foi possível executar `{}`", self.display()))?;
        if !status.success() {
            bail!("`{}` terminou com {status}", self.display());
        }
        Ok(())
    }

    /// Executa em silêncio e devolve a saída padrão; erro se o código de saída não for 0.
    pub fn read(&self) -> Result<String> {
        let output = self
            .command()
            .output()
            .with_context(|| format!("não foi possível executar `{}`", self.display()))?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            bail!(
                "`{}` terminou com {}:\n{}",
                self.display(),
                output.status,
                stderr.trim()
            );
        }
        String::from_utf8(output.stdout)
            .with_context(|| format!("saída de `{}` não é UTF-8", self.display()))
    }
}

/// Pasta `target` do workspace, como o cargo a resolve (inclui `CARGO_TARGET_DIR`).
pub fn target_dir() -> Result<PathBuf> {
    let metadata = cargo_metadata::MetadataCommand::new()
        .cargo_path(cargo_program())
        .manifest_path(workspace_root().join("Cargo.toml"))
        .no_deps()
        .exec()
        .context("falha ao rodar `cargo metadata`")?;
    Ok(metadata.target_directory.into_std_path_buf())
}

/// Texto do aviso quando o caminho da pasta `target` é longo demais, ou `None`.
pub fn long_target_warning(target: &Path) -> Option<String> {
    let length = target.as_os_str().to_string_lossy().chars().count();
    (length > TARGET_PATH_LIMIT).then(|| {
        format!(
            "AVISO: o caminho da pasta target tem {length} caracteres (limite: {TARGET_PATH_LIMIT}).\n\
             No Windows, o link.exe falha com caminhos acima de 260 caracteres (LNK1104).\n\
             Solução: use uma pasta target curta só deste worktree, por exemplo\n\
             \x20 $env:CARGO_TARGET_DIR = 'C:\\wt\\<nome-do-worktree>'\n\
             e rode o comando de novo (QUALITY §13.3).\n\
             Pasta atual: {}",
            target.display()
        )
    })
}

/// Imprime o aviso da pasta `target`, se houver. Falha ao descobrir a pasta não interrompe.
pub fn warn_long_target() {
    match target_dir() {
        Ok(target) => {
            if let Some(warning) = long_target_warning(&target) {
                println!("{warning}\n");
            }
        }
        Err(error) => println!("AVISO: não foi possível conferir a pasta target: {error:#}\n"),
    }
}

/// Duração em texto curto: "42 s" ou "3 min 05 s".
pub fn format_duration(duration: Duration) -> String {
    let seconds = duration.as_secs();
    if seconds < 60 {
        format!("{seconds} s")
    } else {
        format!("{} min {:02} s", seconds / 60, seconds % 60)
    }
}

/// Mede quanto tempo `f` leva.
pub fn timed<T>(f: impl FnOnce() -> T) -> (T, Duration) {
    let start = Instant::now();
    let value = f();
    (value, start.elapsed())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aviso_so_quando_passa_do_limite() {
        let short = PathBuf::from("C:\\wt\\x\\target");
        assert!(long_target_warning(&short).is_none());
        let exact = PathBuf::from("a".repeat(TARGET_PATH_LIMIT));
        assert!(long_target_warning(&exact).is_none());
        let long = PathBuf::from("a".repeat(TARGET_PATH_LIMIT + 1));
        let warning = long_target_warning(&long).unwrap();
        assert!(warning.contains("CARGO_TARGET_DIR"));
        assert!(warning.contains("101 caracteres"));
    }

    #[test]
    fn aviso_conta_caracteres_e_nao_bytes() {
        let path = PathBuf::from("ç".repeat(TARGET_PATH_LIMIT));
        assert!(long_target_warning(&path).is_none());
    }

    #[test]
    fn duracao_curta_e_longa() {
        assert_eq!(format_duration(Duration::from_secs(42)), "42 s");
        assert_eq!(format_duration(Duration::from_secs(185)), "3 min 05 s");
    }

    #[test]
    fn display_mostra_nome_do_programa_e_argumentos_sem_ambiente() {
        // Caminho montado com o separador da plataforma, para valer no Linux também.
        let program = Path::new("x").join("pnpm.cmd");
        let cmd = Cmd::new(&program)
            .args(["install", "--frozen-lockfile"])
            .env("SEGREDO", "valor");
        assert_eq!(cmd.display(), "pnpm install --frozen-lockfile");
    }
}
