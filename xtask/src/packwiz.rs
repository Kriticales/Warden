//! `cargo xtask build-packwiz`: compila o sidecar do packwiz a partir do commit fixado, com os
//! patches do Warden (ARCHITECTURE §6.3, ADR-0007).
//!
//! Passos:
//! 1. lê `third_party/packwiz/COMMIT` e os `patches/*.patch` (em ordem de nome);
//! 2. se o registro do último build (`binaries/packwiz.build.toml`) tem o mesmo commit, os
//!    mesmos patches e a mesma receita, e os executáveis gravados não mudaram, não faz nada;
//! 3. senão, prepara o código num cache (`<target>/packwiz/src`, um clone raso do commit),
//!    compila um executável **sem patch** do sistema atual (`<target>/packwiz/referencia/`,
//!    só para os testes compararem o comportamento), aplica os patches e compila Windows e
//!    Linux com `CGO_ENABLED=0 go build -trimpath -buildvcs=false -ldflags="-s -w"`;
//! 4. confere cada executável (cabeçalho PE32+ ou ELF estático, a mensagem do patch presente
//!    e a chave embutida do packwiz ausente, em base64 e decodificada) e só então grava
//!    `binaries/packwiz.commit` e o registro do build.
//!
//! Os executáveis ficam em `apps/desktop/src-tauri/binaries/packwiz-<triplo>[.exe]`, o nome que
//! o Tauri espera para `bundle.externalBin: ["binaries/packwiz"]`. Nada aqui imprime chaves.

mod exe;
#[cfg(test)]
mod sidecar_tests;

use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use anyhow::{Context as _, Result, bail, ensure};
use base64::Engine as _;
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};

pub use exe::ExeFormat;

use crate::util::{
    Cmd, desktop_dir, find_program, format_duration, target_dir, timed, workspace_root,
};

/// Repositório oficial do packwiz.
pub const REPO_URL: &str = "https://github.com/packwiz/packwiz";
/// Variável de ambiente de onde o packwiz com patch lê a chave da CurseForge.
pub const KEY_ENV: &str = "WARDEN_CURSEFORGE_API_KEY";
/// Começo da mensagem de erro do patch quando a chave falta (P1-02 reconhece por ela).
pub const MISSING_KEY_MESSAGE: &str = "WARDEN_CURSEFORGE_API_KEY ausente";
/// Versão da receita de build: muda quando os argumentos do `go build` mudam, para forçar
/// uma nova compilação.
pub const RECIPE: u32 = 1;
/// Argumentos do `go build` (ARCHITECTURE §6.3), além de `-o`.
pub const GO_BUILD_FLAGS: &[&str] = &["-trimpath", "-buildvcs=false", "-ldflags=-s -w"];
/// Go mínimo (ADR-0007).
pub const MIN_GO: (u32, u32) = (1, 24);
/// Nome da constante da chave embutida no código original do packwiz.
const EMBEDDED_KEY_CONST: &str = "cfApiKeyDefault";
/// Arquivo do código original que contém a chave embutida.
const EMBEDDED_KEY_FILE: &str = "curseforge/request.go";

/// Um alvo de compilação do sidecar.
#[derive(Debug)]
pub struct Target {
    /// Triplo do Rust, usado no nome do arquivo que o Tauri procura.
    pub triple: &'static str,
    /// `GOOS` do Go.
    pub goos: &'static str,
    /// Formato de executável esperado.
    pub format: ExeFormat,
}

impl Target {
    /// Nome do arquivo em `binaries/`: `packwiz-<triplo>[.exe]`.
    pub fn file_name(&self) -> String {
        format!("packwiz-{}{}", self.triple, self.extension())
    }

    fn extension(&self) -> &'static str {
        match self.format {
            ExeFormat::Pe => ".exe",
            ExeFormat::Elf => "",
        }
    }
}

/// Alvos compilados (ARCHITECTURE §6.3), sempre para `GOARCH=amd64`.
pub const TARGETS: &[Target] = &[
    Target {
        triple: "x86_64-pc-windows-msvc",
        goos: "windows",
        format: ExeFormat::Pe,
    },
    Target {
        triple: "x86_64-unknown-linux-gnu",
        goos: "linux",
        format: ExeFormat::Elf,
    },
];

/// Alvo do sistema em que o xtask roda (para o executável de referência e os testes).
pub fn host_target() -> Option<&'static Target> {
    let goos = if cfg!(windows) {
        "windows"
    } else if cfg!(target_os = "linux") {
        "linux"
    } else {
        return None;
    };
    if !cfg!(target_arch = "x86_64") {
        return None;
    }
    TARGETS.iter().find(|target| target.goos == goos)
}

/// Onde ficam as entradas, o cache e as saídas do build.
#[derive(Debug, Clone)]
pub struct Layout {
    /// `third_party/packwiz/` (COMMIT, patches, LICENSE).
    pub third_party: PathBuf,
    /// `apps/desktop/src-tauri/binaries/`.
    pub binaries: PathBuf,
    /// `<target>/packwiz/`: código clonado e executável de referência.
    pub cache: PathBuf,
}

impl Layout {
    /// Pastas reais do workspace.
    pub fn current() -> Result<Self> {
        Ok(Self {
            third_party: workspace_root().join("third_party").join("packwiz"),
            binaries: desktop_dir().join("src-tauri").join("binaries"),
            cache: target_dir()?.join("packwiz"),
        })
    }

    /// Clone do packwiz no cache.
    pub fn source(&self) -> PathBuf {
        self.cache.join("src")
    }

    /// Executável sem patch do sistema atual, para os testes de comparação.
    pub fn reference(&self) -> Option<PathBuf> {
        host_target().map(|target| {
            self.cache
                .join("referencia")
                .join(format!("packwiz-sem-patch{}", target.extension()))
        })
    }

    /// Executável com patch de um alvo.
    pub fn binary(&self, target: &Target) -> PathBuf {
        self.binaries.join(target.file_name())
    }

    /// `binaries/packwiz.commit`, lido pelo app para mostrar a versão do packwiz.
    pub fn commit_file(&self) -> PathBuf {
        self.binaries.join("packwiz.commit")
    }

    /// Registro do último build, usado para pular builds repetidos.
    pub fn stamp_file(&self) -> PathBuf {
        self.binaries.join("packwiz.build.toml")
    }

    /// Executável de referência como saída do registro: (nome no registro, caminho).
    fn reference_output(&self) -> Option<(String, PathBuf)> {
        let path = self.reference()?;
        let name = format!("referencia/{}", path.file_name()?.to_string_lossy());
        Some((name, path))
    }

    /// Saídas conferidas pelo registro: (nome no registro, caminho).
    fn outputs(&self) -> Vec<(String, PathBuf)> {
        TARGETS
            .iter()
            .map(|target| (target.file_name(), self.binary(target)))
            .chain(self.reference_output())
            .collect()
    }
}

/// O que determina o resultado do build.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Inputs {
    /// [`RECIPE`].
    pub recipe: u32,
    /// Hash completo do commit do packwiz.
    pub commit: String,
    /// Nome de cada patch e o SHA-256 do conteúdo.
    pub patches: BTreeMap<String, String>,
}

/// Registro do último build bem-sucedido (`binaries/packwiz.build.toml`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Stamp {
    /// Entradas usadas.
    pub inputs: Inputs,
    /// SHA-256 de cada saída.
    pub outputs: BTreeMap<String, String>,
}

/// Lê o hash de commit: exatamente 40 dígitos hexadecimais minúsculos (espaços nas pontas
/// são ignorados).
pub fn parse_commit(text: &str) -> Result<String> {
    let commit = text.trim();
    ensure!(
        commit.len() == 40
            && commit
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)),
        "third_party/packwiz/COMMIT precisa ter o hash completo do commit (40 dígitos \
         hexadecimais minúsculos)"
    );
    Ok(commit.to_owned())
}

/// Lê a versão do Go de `go env GOVERSION` (`go1.26.5`, `devel go1.27-abc...`).
pub fn parse_go_version(text: &str) -> Option<(u32, u32)> {
    let token = text
        .split_whitespace()
        .find_map(|token| token.strip_prefix("go"))?;
    let mut parts = token.split(|c: char| !c.is_ascii_digit());
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    Some((major, minor))
}

/// SHA-256 em hexadecimal.
pub fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut text = String::with_capacity(64);
    for byte in digest {
        // Escrever numa String não falha.
        let _ = write!(text, "{byte:02x}");
    }
    text
}

/// Patches em ordem de nome: (nome, conteúdo).
pub fn read_patches(third_party: &Path) -> Result<Vec<(String, Vec<u8>)>> {
    let dir = third_party.join("patches");
    let mut patches = Vec::new();
    for entry in
        std::fs::read_dir(&dir).with_context(|| format!("falha ao ler {}", dir.display()))?
    {
        let path = entry?.path();
        if path.extension().is_some_and(|ext| ext == "patch") {
            let name = path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default();
            let bytes =
                std::fs::read(&path).with_context(|| format!("falha ao ler {}", path.display()))?;
            patches.push((name, bytes));
        }
    }
    patches.sort_by(|a, b| a.0.cmp(&b.0));
    ensure!(
        !patches.is_empty(),
        "nenhum patch em {}: o da chave da CurseForge é obrigatório (ADR-0007)",
        dir.display()
    );
    Ok(patches)
}

/// Entradas atuais do build.
pub fn read_inputs(third_party: &Path) -> Result<Inputs> {
    let commit_path = third_party.join("COMMIT");
    let commit = parse_commit(
        &std::fs::read_to_string(&commit_path)
            .with_context(|| format!("falha ao ler {}", commit_path.display()))?,
    )?;
    let patches = read_patches(third_party)?
        .into_iter()
        .map(|(name, bytes)| (name, sha256_hex(&bytes)))
        .collect();
    Ok(Inputs {
        recipe: RECIPE,
        commit,
        patches,
    })
}

fn short(commit: &str) -> &str {
    commit.get(..7).unwrap_or(commit)
}

/// Por que o build precisa rodar, ou `None` se as saídas já correspondem às entradas.
pub fn stale_reason(layout: &Layout, inputs: &Inputs) -> Result<Option<String>> {
    let stamp_path = layout.stamp_file();
    let text = match std::fs::read_to_string(&stamp_path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(Some("primeiro build".to_owned()));
        }
        Err(error) => {
            return Err(error).with_context(|| format!("falha ao ler {}", stamp_path.display()));
        }
    };
    let Ok(stamp) = toml::from_str::<Stamp>(&text) else {
        return Ok(Some("o registro do último build está ilegível".to_owned()));
    };
    if stamp.inputs.commit != inputs.commit {
        return Ok(Some(format!(
            "o commit mudou ({} → {})",
            short(&stamp.inputs.commit),
            short(&inputs.commit)
        )));
    }
    if stamp.inputs.patches != inputs.patches {
        return Ok(Some("os patches mudaram".to_owned()));
    }
    if stamp.inputs.recipe != inputs.recipe {
        return Ok(Some("a receita do build mudou".to_owned()));
    }
    match std::fs::read_to_string(layout.commit_file()) {
        Ok(text) if text.trim() == inputs.commit => {}
        _ => return Ok(Some("packwiz.commit falta ou não confere".to_owned())),
    }
    for (name, path) in layout.outputs() {
        let Some(expected) = stamp.outputs.get(&name) else {
            return Ok(Some(format!("{name} não está no registro do build")));
        };
        let bytes = match std::fs::read(&path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Some(format!("{name} não existe")));
            }
            Err(error) => {
                return Err(error).with_context(|| format!("falha ao ler {}", path.display()));
            }
        };
        if &sha256_hex(&bytes) != expected {
            return Ok(Some(format!("{name} foi alterado depois do build")));
        }
    }
    Ok(None)
}

/// Extrai o valor (base64) da constante da chave embutida do código original do packwiz.
pub fn embedded_key_from_source(source: &str) -> Option<String> {
    let start = source.find(&format!("{EMBEDDED_KEY_CONST} = \""))?;
    let rest = &source[start + EMBEDDED_KEY_CONST.len() + 4..];
    let value = &rest[..rest.find('"')?];
    let valid = value.len() >= 16
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'+' | b'/' | b'='));
    valid.then(|| value.to_owned())
}

/// Sequências proibidas no executável com patch: a chave embutida em base64 e decodificada.
pub fn forbidden_needles(base64_key: &str) -> Result<Vec<Vec<u8>>> {
    let decoded = base64::engine::general_purpose::STANDARD
        .decode(base64_key)
        .context("a chave embutida do packwiz não é base64 válido")?;
    ensure!(
        decoded.len() >= 16,
        "a chave embutida decodificada é curta demais"
    );
    Ok(vec![base64_key.as_bytes().to_vec(), decoded])
}

/// Se `needle` aparece em `haystack` (sequência vazia sempre aparece).
pub fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    needle.is_empty()
        || haystack
            .windows(needle.len())
            .any(|window| window == needle)
}

/// Lê, do clone no cache, a chave embutida no código **original** do commit (antes dos
/// patches), e devolve as sequências que não podem existir no executável com patch.
pub fn original_key_needles(git: &Path, source: &Path, commit: &str) -> Result<Vec<Vec<u8>>> {
    let original = cache_git(git, source)
        .args(["show", &format!("{commit}:{EMBEDDED_KEY_FILE}")])
        .read()
        .context("falha ao ler o código original do packwiz no cache")?;
    let key = embedded_key_from_source(&original).with_context(|| {
        format!("a constante {EMBEDDED_KEY_CONST} não foi encontrada em {EMBEDDED_KEY_FILE}")
    })?;
    forbidden_needles(&key)
}

/// Confere um executável com patch: formato, mensagem do patch presente e chave ausente.
pub fn verify_patched(target: &Target, bytes: &[u8], needles: &[Vec<u8>]) -> Result<()> {
    let name = target.file_name();
    exe::check(target.format, bytes).with_context(|| format!("{name} com formato inesperado"))?;
    ensure!(
        contains(bytes, MISSING_KEY_MESSAGE.as_bytes()),
        "{name} não tem a mensagem do patch: os patches não foram aplicados?"
    );
    for (index, needle) in needles.iter().enumerate() {
        ensure!(
            !contains(bytes, needle),
            "{name} contém a chave embutida do packwiz (forma {})",
            index + 1
        );
    }
    Ok(())
}

/// `cargo xtask build-packwiz [--force]`.
pub fn run(force: bool) -> Result<()> {
    let (result, elapsed) = timed(|| build(force));
    if result.is_ok() {
        println!("build-packwiz: terminou em {}.", format_duration(elapsed));
    }
    result
}

fn build(force: bool) -> Result<()> {
    let layout = Layout::current()?;
    let inputs = read_inputs(&layout.third_party)?;
    let reason = if force {
        Some("--force".to_owned())
    } else {
        stale_reason(&layout, &inputs)?
    };
    let Some(reason) = reason else {
        println!(
            "build-packwiz: sidecar em dia (commit {}, {} patch(es)); nada a fazer.",
            short(&inputs.commit),
            inputs.patches.len()
        );
        return Ok(());
    };
    println!(
        "build-packwiz: compilando o packwiz {} ({reason}).",
        short(&inputs.commit)
    );

    let go = find_program("go").context("o Go ≥ 1.24 é pré-requisito (ADR-0007)")?;
    let git = find_program("git")?;
    check_go_version(&go)?;

    // Sem registro durante o build: uma interrupção no meio força o próximo a recomeçar.
    remove_if_exists(&layout.stamp_file())?;
    let source = layout.source();
    prepare_source(&git, &source, &inputs.commit)?;
    let needles = original_key_needles(&git, &source, &inputs.commit)?;

    let mut outputs = BTreeMap::new();
    if let (Some(host), Some((name, reference))) = (host_target(), layout.reference_output()) {
        go_build(&go, &source, host, &reference)?;
        let bytes = read(&reference)?;
        exe::check(host.format, &bytes).context("executável sem patch com formato inesperado")?;
        // Controle positivo: a busca acha a chave onde ela existe.
        ensure!(
            needles.first().is_some_and(|key| contains(&bytes, key)),
            "o executável sem patch não contém a chave embutida: a conferência da chave \
             não está funcionando"
        );
        outputs.insert(name, sha256_hex(&bytes));
    }

    for (name, _) in read_patches(&layout.third_party)? {
        let patch = layout.third_party.join("patches").join(&name);
        cache_git(&git, &source)
            .args(["apply", "--whitespace=error-all"])
            .args([patch.as_os_str()])
            .run()
            .with_context(|| format!("o patch {name} não se aplica ao commit"))?;
    }

    std::fs::create_dir_all(&layout.binaries)
        .with_context(|| format!("falha ao criar {}", layout.binaries.display()))?;
    for target in TARGETS {
        let path = layout.binary(target);
        go_build(&go, &source, target, &path)?;
        let bytes = read(&path)?;
        verify_patched(target, &bytes, &needles)?;
        println!(
            "build-packwiz: {} conferido ({:.1} MB, sem a chave embutida).",
            target.file_name(),
            mebibytes(bytes.len())
        );
        outputs.insert(target.file_name(), sha256_hex(&bytes));
    }

    write(&layout.commit_file(), &format!("{}\n", inputs.commit))?;
    let stamp = Stamp { inputs, outputs };
    let text = format!(
        "# Gerado por `cargo xtask build-packwiz`; não edite.\n{}",
        toml::to_string(&stamp).context("falha ao gerar o registro do build")?
    );
    write(&layout.stamp_file(), &text)?;
    println!("build-packwiz: o sidecar lê a chave da CurseForge de {KEY_ENV}.");
    Ok(())
}

#[allow(clippy::cast_precision_loss)] // tamanhos de executável cabem com folga num f64
fn mebibytes(len: usize) -> f64 {
    len as f64 / (1024.0 * 1024.0)
}

fn read(path: &Path) -> Result<Vec<u8>> {
    std::fs::read(path).with_context(|| format!("falha ao ler {}", path.display()))
}

fn write(path: &Path, text: &str) -> Result<()> {
    std::fs::write(path, text).with_context(|| format!("falha ao gravar {}", path.display()))
}

fn remove_if_exists(path: &Path) -> Result<()> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error).with_context(|| format!("falha ao apagar {}", path.display())),
    }
}

fn check_go_version(go: &Path) -> Result<()> {
    let text = Cmd::new(go)
        .args(["env", "GOVERSION"])
        .env("GOTOOLCHAIN", "local")
        .read()?;
    let version = parse_go_version(&text)
        .with_context(|| format!("versão do Go não reconhecida: {}", text.trim()))?;
    if version < MIN_GO {
        bail!(
            "o packwiz precisa do Go {}.{} ou mais novo; encontrado {}",
            MIN_GO.0,
            MIN_GO.1,
            text.trim()
        );
    }
    println!("build-packwiz: Go {}", text.trim());
    Ok(())
}

/// Git preso ao cache do packwiz: `--git-dir` e `--work-tree` explícitos e
/// `GIT_CEILING_DIRECTORIES` no pai, para que um `.git` estragado no cache nunca faça o git
/// subir até o repositório do Warden (onde `checkout --force` e `clean -fdx` apagariam o `.env`).
fn cache_git(git: &Path, source: &Path) -> Cmd {
    let mut git_dir = std::ffi::OsString::from("--git-dir=");
    git_dir.push(source.join(".git"));
    let mut work_tree = std::ffi::OsString::from("--work-tree=");
    work_tree.push(source);
    let ceiling = source.parent().unwrap_or(source);
    Cmd::new(git)
        .cwd(source)
        .env("GIT_CEILING_DIRECTORIES", ceiling)
        .args([git_dir, work_tree])
}

/// Diz se `source` é um repositório git sadio cuja raiz é a própria pasta do cache.
fn cache_is_healthy(git: &Path, source: &Path) -> bool {
    if !source.join(".git").is_dir() {
        return false;
    }
    let Ok(top) = cache_git(git, source)
        .args(["rev-parse", "--show-toplevel"])
        .read()
    else {
        return false;
    };
    match (
        std::fs::canonicalize(top.trim()),
        std::fs::canonicalize(source),
    ) {
        (Ok(top), Ok(source)) => top == source,
        _ => false,
    }
}

/// Garante em `source` um repositório git próprio; apaga e recria o cache se estiver estragado.
fn ensure_cache_repo(git: &Path, source: &Path) -> Result<()> {
    if !cache_is_healthy(git, source) {
        if source.exists() {
            std::fs::remove_dir_all(source)
                .with_context(|| format!("falha ao limpar {}", source.display()))?;
        }
        std::fs::create_dir_all(source)
            .with_context(|| format!("falha ao criar {}", source.display()))?;
        Cmd::new(git)
            .cwd(source)
            .env("GIT_CEILING_DIRECTORIES", source.parent().unwrap_or(source))
            .args(["init", "-q"])
            .run()?;
        ensure!(
            cache_is_healthy(git, source),
            "o repositório git recriado em {} não tem a própria pasta como raiz",
            source.display()
        );
    }
    // Fim de linha fixo no cache, para os patches se aplicarem igual em qualquer máquina.
    cache_git(git, source)
        .args(["config", "core.autocrlf", "false"])
        .run()?;
    cache_git(git, source)
        .args(["config", "core.eol", "lf"])
        .run()?;
    Ok(())
}

/// Deixa `source` com o commit pedido, sem mudanças locais. Baixa só esse commit (clone raso)
/// quando ele ainda não está no cache.
fn prepare_source(git: &Path, source: &Path, commit: &str) -> Result<()> {
    prepare_source_from(git, source, REPO_URL, commit)
}

fn prepare_source_from(git: &Path, source: &Path, url: &str, commit: &str) -> Result<()> {
    ensure_cache_repo(git, source)?;
    let has_commit = cache_git(git, source)
        .args(["cat-file", "-e", &format!("{commit}^{{commit}}")])
        .read()
        .is_ok();
    if !has_commit {
        cache_git(git, source)
            .args(["fetch", "-q", "--depth", "1", "--no-tags", url, commit])
            .run()
            .context("falha ao baixar o packwiz (a primeira compilação precisa de rede)")?;
    }
    cache_git(git, source)
        .args(["checkout", "-q", "--force", "--detach", commit])
        .run()?;
    cache_git(git, source)
        .args(["clean", "-q", "-f", "-d", "-x"])
        .run()?;
    Ok(())
}

/// `go build` de um alvo, gravando primeiro num arquivo temporário ao lado do destino.
fn go_build(go: &Path, source: &Path, target: &Target, output: &Path) -> Result<()> {
    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("falha ao criar {}", parent.display()))?;
    }
    let mut temporary = output.as_os_str().to_os_string();
    temporary.push(".tmp");
    let temporary = PathBuf::from(temporary);
    let result = Cmd::new(go)
        .cwd(source)
        .args(["build"])
        .args(GO_BUILD_FLAGS)
        .args([OsStr::new("-o"), temporary.as_os_str(), OsStr::new(".")])
        .env("CGO_ENABLED", "0")
        .env("GOOS", target.goos)
        .env("GOARCH", "amd64")
        .env("GOTOOLCHAIN", "local")
        .env("GOFLAGS", "")
        .run()
        .and_then(|()| {
            std::fs::rename(&temporary, output).with_context(|| {
                format!(
                    "falha ao gravar {} (o app está aberto usando o sidecar?)",
                    output.display()
                )
            })
        });
    if result.is_err() {
        // A falha original é o que importa; o temporário pode nem existir.
        let _ = std::fs::remove_file(&temporary);
    }
    result
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    fn fake_layout(root: &Path) -> Layout {
        let layout = Layout {
            third_party: root.join("third_party"),
            binaries: root.join("binaries"),
            cache: root.join("cache"),
        };
        std::fs::create_dir_all(layout.third_party.join("patches")).unwrap();
        std::fs::create_dir_all(&layout.binaries).unwrap();
        std::fs::write(
            layout.third_party.join("COMMIT"),
            "ef87d964f8cbd52b3b13ea42453ef322290e2b9e\n",
        )
        .unwrap();
        std::fs::write(
            layout.third_party.join("patches").join("0001-a.patch"),
            "diff a\n",
        )
        .unwrap();
        layout
    }

    /// Simula um build concluído: grava saídas, packwiz.commit e o registro.
    fn fake_build(layout: &Layout) -> Inputs {
        let inputs = read_inputs(&layout.third_party).unwrap();
        let mut outputs = BTreeMap::new();
        for (name, path) in layout.outputs() {
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, name.as_bytes()).unwrap();
            outputs.insert(name.clone(), sha256_hex(name.as_bytes()));
        }
        std::fs::write(layout.commit_file(), format!("{}\n", inputs.commit)).unwrap();
        let stamp = Stamp {
            inputs: inputs.clone(),
            outputs,
        };
        std::fs::write(layout.stamp_file(), toml::to_string(&stamp).unwrap()).unwrap();
        inputs
    }

    #[test]
    fn commit_completo_e_aceito() {
        let commit = "ef87d964f8cbd52b3b13ea42453ef322290e2b9e";
        assert_eq!(parse_commit(&format!("  {commit}\r\n")).unwrap(), commit);
    }

    #[test]
    fn commit_curto_maiusculo_ou_invalido_e_recusado() {
        for text in [
            "ef87d96",
            "EF87D964F8CBD52B3B13EA42453EF322290E2B9E",
            "ef87d964f8cbd52b3b13ea42453ef322290e2b9g",
            "ef87d964f8cbd52b3b13ea42453ef322290e2b9e0",
            "ef87d964f8cbd52b3b13 a42453ef322290e2b9e",
            "",
        ] {
            assert!(parse_commit(text).is_err(), "{text:?}");
        }
    }

    #[test]
    fn versao_do_go_estavel_e_de_desenvolvimento() {
        assert_eq!(parse_go_version("go1.26.5\n"), Some((1, 26)));
        assert_eq!(parse_go_version("go1.24"), Some((1, 24)));
        assert_eq!(parse_go_version("go1.25rc2"), Some((1, 25)));
        assert_eq!(
            parse_go_version("devel go1.27-4f2a7e Fri Oct 2"),
            Some((1, 27))
        );
        assert_eq!(parse_go_version(""), None);
        assert_eq!(parse_go_version("gox"), None);
        assert!(parse_go_version("go1.23.9").unwrap() < MIN_GO);
        assert!(parse_go_version("go1.24.0").unwrap() >= MIN_GO);
    }

    #[test]
    fn sha256_conhecido() {
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn nomes_que_o_tauri_espera() {
        let names: Vec<_> = TARGETS.iter().map(Target::file_name).collect();
        assert_eq!(
            names,
            [
                "packwiz-x86_64-pc-windows-msvc.exe",
                "packwiz-x86_64-unknown-linux-gnu"
            ]
        );
        let host = host_target().unwrap();
        assert_eq!(host.goos, if cfg!(windows) { "windows" } else { "linux" });
    }

    #[test]
    fn chave_embutida_e_extraida_do_codigo() {
        let source = "const cfApiServer = \"x\"\n\
                      const cfApiKeyDefault = \"QUJDREVGR0hJSktMTU5PUFFSU1RVVldY\"\n";
        let key = embedded_key_from_source(source).unwrap();
        assert_eq!(key, "QUJDREVGR0hJSktMTU5PUFFSU1RVVldY");
        let needles = forbidden_needles(&key).unwrap();
        assert_eq!(needles[1], b"ABCDEFGHIJKLMNOPQRSTUVWX");
        assert_eq!(embedded_key_from_source("var cfApiKey = \"\""), None);
        assert_eq!(
            embedded_key_from_source("const cfApiKeyDefault = \"curta\""),
            None
        );
        assert_eq!(
            embedded_key_from_source("const cfApiKeyDefault = \"sem fim"),
            None
        );
        assert!(forbidden_needles("QUJD").is_err());
        assert!(forbidden_needles("!!!!!!!!!!!!!!!!").is_err());
    }

    #[test]
    fn busca_de_bytes() {
        assert!(contains(b"abcdef", b"cde"));
        assert!(!contains(b"abcdef", b"ced"));
        assert!(!contains(b"ab", b"abc"));
        assert!(contains(b"ab", b""));
    }

    #[test]
    fn executavel_com_patch_e_conferido() {
        let windows = &TARGETS[0];
        let mut pe = exe::tests::synthetic_pe(
            exe::PE_MACHINE_AMD64,
            exe::PE32_PLUS_MAGIC,
            exe::PE_SUBSYSTEM_CONSOLE,
        );
        let needles = vec![b"CHAVE-BASE64".to_vec(), b"chave-decodificada".to_vec()];
        let error = verify_patched(windows, &pe, &needles).unwrap_err();
        assert!(format!("{error}").contains("mensagem do patch"));
        pe.extend_from_slice(MISSING_KEY_MESSAGE.as_bytes());
        verify_patched(windows, &pe, &needles).unwrap();
        let mut with_key = pe.clone();
        with_key.extend_from_slice(b"..chave-decodificada..");
        let error = format!(
            "{}",
            verify_patched(windows, &with_key, &needles).unwrap_err()
        );
        assert!(error.contains("chave embutida"), "{error}");
        assert!(!error.contains("chave-decodificada"), "{error}");
        let linux = &TARGETS[1];
        assert!(verify_patched(linux, &pe, &needles).is_err());
    }

    #[test]
    fn patches_em_ordem_e_so_os_patch() {
        let dir = tempfile::tempdir().unwrap();
        let layout = fake_layout(dir.path());
        let patches = layout.third_party.join("patches");
        std::fs::write(patches.join("0002-b.patch"), "b").unwrap();
        std::fs::write(patches.join("LEIA.md"), "x").unwrap();
        let names: Vec<_> = read_patches(&layout.third_party)
            .unwrap()
            .into_iter()
            .map(|(name, _)| name)
            .collect();
        assert_eq!(names, ["0001-a.patch", "0002-b.patch"]);
        std::fs::remove_file(patches.join("0001-a.patch")).unwrap();
        std::fs::remove_file(patches.join("0002-b.patch")).unwrap();
        assert!(read_patches(&layout.third_party).is_err());
    }

    #[test]
    fn sem_registro_e_o_primeiro_build() {
        let dir = tempfile::tempdir().unwrap();
        let layout = fake_layout(dir.path());
        let inputs = read_inputs(&layout.third_party).unwrap();
        assert_eq!(
            stale_reason(&layout, &inputs).unwrap().as_deref(),
            Some("primeiro build")
        );
    }

    #[test]
    fn build_igual_e_pulado() {
        let dir = tempfile::tempdir().unwrap();
        let layout = fake_layout(dir.path());
        let inputs = fake_build(&layout);
        assert_eq!(stale_reason(&layout, &inputs).unwrap(), None);
    }

    #[test]
    fn commit_patch_ou_receita_diferentes_refazem_o_build() {
        let dir = tempfile::tempdir().unwrap();
        let layout = fake_layout(dir.path());
        let inputs = fake_build(&layout);

        let mut other = inputs.clone();
        other.commit = "9a2ca2e1a7a1ce2e93d93705f330a1976de6a92e".to_owned();
        let reason = stale_reason(&layout, &other).unwrap().unwrap();
        assert!(reason.contains("ef87d96 → 9a2ca2e"), "{reason}");

        std::fs::write(
            layout.third_party.join("patches").join("0001-a.patch"),
            "diff a mudado\n",
        )
        .unwrap();
        let changed = read_inputs(&layout.third_party).unwrap();
        assert_eq!(
            stale_reason(&layout, &changed).unwrap().as_deref(),
            Some("os patches mudaram")
        );

        let mut recipe = inputs.clone();
        recipe.recipe += 1;
        assert_eq!(
            stale_reason(&layout, &recipe).unwrap().as_deref(),
            Some("a receita do build mudou")
        );
    }

    #[test]
    fn saida_apagada_ou_alterada_refaz_o_build() {
        let dir = tempfile::tempdir().unwrap();
        let layout = fake_layout(dir.path());
        let inputs = fake_build(&layout);
        let windows = layout.binary(&TARGETS[0]);

        std::fs::write(&windows, b"outro conteudo").unwrap();
        let reason = stale_reason(&layout, &inputs).unwrap().unwrap();
        assert!(reason.contains("foi alterado"), "{reason}");

        std::fs::remove_file(&windows).unwrap();
        let reason = stale_reason(&layout, &inputs).unwrap().unwrap();
        assert!(reason.contains("não existe"), "{reason}");

        let inputs = fake_build(&layout);
        std::fs::remove_file(layout.reference().unwrap()).unwrap();
        let reason = stale_reason(&layout, &inputs).unwrap().unwrap();
        assert!(reason.contains("referencia/"), "{reason}");
    }

    #[test]
    fn packwiz_commit_ou_registro_estragados_refazem_o_build() {
        let dir = tempfile::tempdir().unwrap();
        let layout = fake_layout(dir.path());
        let inputs = fake_build(&layout);
        std::fs::write(layout.commit_file(), "outro\n").unwrap();
        assert!(stale_reason(&layout, &inputs).unwrap().is_some());

        let inputs = fake_build(&layout);
        std::fs::write(layout.stamp_file(), "isto não é = toml [").unwrap();
        assert_eq!(
            stale_reason(&layout, &inputs).unwrap().as_deref(),
            Some("o registro do último build está ilegível")
        );

        let inputs = fake_build(&layout);
        let text = std::fs::read_to_string(layout.stamp_file()).unwrap();
        let mut stamp: Stamp = toml::from_str(&text).unwrap();
        stamp.outputs.clear();
        std::fs::write(layout.stamp_file(), toml::to_string(&stamp).unwrap()).unwrap();
        let reason = stale_reason(&layout, &inputs).unwrap().unwrap();
        assert!(reason.contains("não está no registro"), "{reason}");
    }

    // Arquivos versionados da tarefa.

    #[test]
    fn commit_versionado_e_um_hash_completo() {
        let layout = Layout::current().unwrap();
        let inputs = read_inputs(&layout.third_party).unwrap();
        assert_eq!(inputs.commit.len(), 40);
        assert!(
            inputs
                .patches
                .contains_key("0001-chave-curseforge-em-tempo-de-execucao.patch")
        );
    }

    #[test]
    fn patch_da_chave_remove_a_chave_embutida_sem_recuo() {
        let layout = Layout::current().unwrap();
        let path = layout
            .third_party
            .join("patches")
            .join("0001-chave-curseforge-em-tempo-de-execucao.patch");
        let text = std::fs::read_to_string(path).unwrap();
        let added: Vec<_> = text
            .lines()
            .filter(|line| line.starts_with('+') && !line.starts_with("+++"))
            .collect();
        let removed: Vec<_> = text
            .lines()
            .filter(|line| line.starts_with('-') && !line.starts_with("---"))
            .collect();
        for removed_code in [
            "func decodeDefaultKey()",
            "var cfApiKey = \"\"",
            "\"encoding/base64\"",
        ] {
            assert!(
                removed.iter().any(|line| line.contains(removed_code)),
                "{removed_code}"
            );
        }
        let new_code: Vec<_> = added
            .iter()
            .filter(|line| !line[1..].trim_start().starts_with("//"))
            .collect();
        assert!(
            new_code
                .iter()
                .any(|line| line.contains(&format!("\"{KEY_ENV}\"")))
        );
        assert!(
            new_code
                .iter()
                .any(|line| line.contains(MISSING_KEY_MESSAGE))
        );
        // Nenhum código acrescentado usa a chave embutida (só o comentário a cita).
        for line in &new_code {
            assert!(!line.contains("cfApiKeyDefault"), "{line}");
            assert!(!line.contains("decodeDefaultKey"), "{line}");
            assert!(!line.contains("base64"), "{line}");
        }
        // O texto da chave nunca entra no repositório do Warden, nem como contexto do diff.
        assert!(!text.contains("cfApiKeyDefault = \""));
    }

    #[test]
    fn licenca_do_packwiz_e_mit() {
        let layout = Layout::current().unwrap();
        let license = std::fs::read_to_string(layout.third_party.join("LICENSE")).unwrap();
        assert!(license.starts_with("MIT License"));
    }

    #[test]
    fn tauri_conf_declara_o_sidecar() {
        let conf = desktop_dir().join("src-tauri").join("tauri.conf.json");
        let conf: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(conf).unwrap()).unwrap();
        assert_eq!(
            conf["bundle"]["externalBin"],
            serde_json::json!(["binaries/packwiz"])
        );
    }

    #[test]
    fn binarios_gerados_ficam_fora_do_git() {
        let layout = Layout::current().unwrap();
        let mut paths: Vec<PathBuf> = TARGETS.iter().map(|t| layout.binary(t)).collect();
        paths.push(layout.commit_file());
        paths.push(layout.stamp_file());
        for path in paths {
            let output = std::process::Command::new("git")
                .args(["check-ignore", "-q", "--no-index"])
                .arg(&path)
                .current_dir(workspace_root())
                .status()
                .unwrap();
            assert!(output.success(), "{} não é ignorado", path.display());
        }
    }

    /// Git de teste numa pasta descartável, sem depender da configuração global de quem roda.
    fn test_git(dir: &Path, args: &[&str]) -> String {
        let output = std::process::Command::new("git")
            .current_dir(dir)
            .args([
                "-c",
                "user.name=Teste",
                "-c",
                "user.email=teste@example.invalid",
                "-c",
                "commit.gpgsign=false",
            ])
            .args(args)
            .output()
            .unwrap();
        assert!(output.status.success(), "git {args:?}: {output:?}");
        String::from_utf8(output.stdout).unwrap()
    }

    #[test]
    fn cache_com_git_estragado_e_recriado_sem_tocar_no_repositorio_pai() {
        let git = find_program("git").unwrap();
        // Tudo numa pasta temporária: o "repositório pai" faz o papel do Warden.
        let dir = tempfile::tempdir().unwrap();
        let upstream = dir.path().join("upstream");
        std::fs::create_dir_all(&upstream).unwrap();
        test_git(&upstream, &["init", "-q"]);
        std::fs::write(
            upstream.join("main.go"),
            "package main
",
        )
        .unwrap();
        test_git(&upstream, &["add", "main.go"]);
        test_git(&upstream, &["commit", "-q", "-m", "upstream"]);
        let commit = test_git(&upstream, &["rev-parse", "HEAD"])
            .trim()
            .to_owned();

        // Cada caso estraga o `.git` do cache de um jeito.
        let corruptions: [(&str, fn(&Path)); 3] = [
            ("pasta .git vazia", |git_dir| {
                std::fs::create_dir_all(git_dir).unwrap();
            }),
            ("HEAD inválido", |git_dir| {
                std::fs::create_dir_all(git_dir.join("objects")).unwrap();
                std::fs::create_dir_all(git_dir.join("refs")).unwrap();
                std::fs::write(
                    git_dir.join("HEAD"),
                    "lixo
",
                )
                .unwrap();
            }),
            (".git é um arquivo", |git_dir| {
                std::fs::write(
                    git_dir,
                    "gitdir: /nao/existe
",
                )
                .unwrap();
            }),
        ];
        for (case, corrupt) in corruptions {
            let parent = dir.path().join(format!("pai-{}", case.replace(' ', "-")));
            std::fs::create_dir_all(&parent).unwrap();
            test_git(&parent, &["init", "-q"]);
            std::fs::write(
                parent.join("versionado.txt"),
                "original
",
            )
            .unwrap();
            test_git(&parent, &["add", "versionado.txt"]);
            test_git(&parent, &["commit", "-q", "-m", "pai"]);
            let parent_head = test_git(&parent, &["rev-parse", "HEAD"]);
            std::fs::write(
                parent.join(".env"),
                "SEGREDO='nao apagar'
",
            )
            .unwrap();
            std::fs::write(
                parent.join("versionado.txt"),
                "mudança local
",
            )
            .unwrap();

            let source = parent.join("target").join("packwiz").join("src");
            std::fs::create_dir_all(&source).unwrap();
            std::fs::write(source.join("sobra.txt"), "x").unwrap();
            corrupt(&source.join(".git"));

            prepare_source_from(&git, &source, upstream.to_str().unwrap(), &commit)
                .unwrap_or_else(|error| panic!("{case}: {error:#}"));

            assert_eq!(
                std::fs::read_to_string(parent.join(".env")).unwrap(),
                "SEGREDO='nao apagar'
",
                "{case}: o arquivo não versionado do pai sumiu"
            );
            assert_eq!(
                std::fs::read_to_string(parent.join("versionado.txt")).unwrap(),
                "mudança local
",
                "{case}: a mudança local do pai foi desfeita"
            );
            assert_eq!(
                test_git(&parent, &["rev-parse", "HEAD"]),
                parent_head,
                "{case}: o HEAD do pai mudou"
            );
            assert!(
                source.join("main.go").is_file(),
                "{case}: o cache não foi refeito"
            );
            assert!(!source.join("sobra.txt").exists(), "{case}: sobra no cache");
            assert!(
                cache_is_healthy(&git, &source),
                "{case}: cache não ficou sadio"
            );
            // Segunda passada sobre o cache já sadio: reaproveita sem rede nem recriação.
            prepare_source_from(&git, &source, "url-que-nao-deve-ser-usada", &commit).unwrap();
        }
    }

    proptest! {
        #[test]
        fn parse_commit_nunca_entra_em_panico(text in ".{0,80}") {
            let _ = parse_commit(&text);
        }

        #[test]
        fn todo_hash_de_40_digitos_e_aceito(commit in "[0-9a-f]{40}") {
            prop_assert_eq!(parse_commit(&commit).unwrap(), commit);
        }

        #[test]
        fn parse_go_version_nunca_entra_em_panico(text in ".{0,40}") {
            let _ = parse_go_version(&text);
        }

        #[test]
        fn versao_do_go_e_lida_de_volta(major in 0_u32..100, minor in 0_u32..1000, patch in 0_u32..100) {
            let text = format!("go{major}.{minor}.{patch}");
            prop_assert_eq!(parse_go_version(&text), Some((major, minor)));
        }

        #[test]
        fn extracao_da_chave_nunca_entra_em_panico(text in ".{0,200}") {
            let _ = embedded_key_from_source(&text);
            let _ = embedded_key_from_source(&format!("cfApiKeyDefault = \"{text}"));
        }

        #[test]
        fn busca_igual_a_ingenua(haystack in proptest::collection::vec(0_u8..4, 0..64), needle in proptest::collection::vec(0_u8..4, 1..6)) {
            let naive = (0..haystack.len()).any(|start| haystack[start..].starts_with(&needle));
            prop_assert_eq!(contains(&haystack, &needle), naive);
        }
    }
}
