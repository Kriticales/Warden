//! `cargo xtask fixtures-packwiz`: gera as fixtures da `warden-packwiz` com o packwiz real
//! (ARCHITECTURE §6.1; QUALITY §4.1, "fixtures reais").
//!
//! Monta packs numa pasta temporária, só com comandos do packwiz (`init`, `modrinth add`,
//! `curseforge add`, `url add`, `pin`, `unpin`, `settings`, `refresh`) e edições de texto que
//! o próprio packwiz regrava em seguida, e copia o resultado para
//! `crates/warden-packwiz/tests/fixtures/packwiz-output/`. Cada mod é escolhido por ID fixo
//! de projeto e de versão, então rodar de novo dá os mesmos bytes enquanto as plataformas não
//! mudarem os dados daquelas versões.
//!
//! Também grava:
//! - `ignore-100/`: 100 caminhos, o `.packwizignore` usado e o que o `packwiz refresh` pôs no
//!   índice (chamado com `--pack-file` relativo e absoluto);
//! - `dados-api.json`: os dados das APIs usados em cada metafile (Modrinth, e da CurseForge só
//!   os campos que o próprio `.pw.toml` já guarda);
//! - `murmur2/vetores.txt`: impressões digitais murmur2 calculadas pela mesma biblioteca Go que
//!   o packwiz usa (`github.com/aviddiviner/go-murmur`), com `go run`;
//! - `FIXTURES.md`: origem de tudo.
//!
//! Rede: Modrinth, CurseForge (chave do `.env`, passada só ao packwiz e ao `curl` pela entrada
//! padrão, nunca impressa nem posta em argumento) e GitHub. Requer o packwiz compilado pela
//! F0-03 (`cargo xtask build-packwiz`), o `curl` e o Go.

use std::collections::BTreeSet;
use std::ffi::OsStr;
use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{Context as _, Result, bail, ensure};
use serde_json::{Value, json};

use crate::env_file;
use crate::util::workspace_root;

/// Pasta das fixtures, relativa à raiz do workspace.
const FIXTURES_DIR: &str = "crates/warden-packwiz/tests/fixtures";

/// Versão do go-murmur fixada no `go.mod` do packwiz `ef87d96`.
const GO_MURMUR_VERSION: &str = "v0.0.0-20150519214947-b9740d71e571";

/// Opções do subcomando.
pub struct Options {
    /// Caminho do binário do packwiz (padrão: `WARDEN_PACKWIZ_BIN` ou o da F0-03).
    pub packwiz: Option<PathBuf>,
    /// Commit do packwiz (padrão: `third_party/packwiz/COMMIT` ou `binaries/packwiz.commit`).
    pub commit: Option<String>,
}

/// `cargo xtask fixtures-packwiz`.
pub fn run(options: Options) -> Result<()> {
    let root = workspace_root();
    let bin = resolve_packwiz(&root, options.packwiz)?;
    let commit = resolve_commit(&root, options.commit)?;
    println!("fixtures-packwiz: packwiz {} ({commit})", bin.display());

    let env_path = env_file::main_repo_env_path()?;
    let vars = env_file::load(&env_path)?.with_context(|| {
        format!(
            "{} não existe (precisa da chave da CurseForge)",
            env_path.display()
        )
    })?;
    let cf_key = vars
        .iter()
        .find(|(name, _)| *name == "CURSEFORGE_API_KEY")
        .map(|(_, value)| value.to_owned())
        .context("CURSEFORGE_API_KEY não está no .env")?;

    let temp = tempfile::tempdir().context("falha ao criar pasta temporária")?;
    let config = temp.path().join("packwiz.toml");
    fs::write(&config, "")?;
    let ctx = Ctx {
        bin,
        config,
        cache: temp.path().join("cache"),
        cf_key,
    };

    let out = root.join(FIXTURES_DIR);
    let output_dir = out.join("packwiz-output");
    if output_dir.exists() {
        fs::remove_dir_all(&output_dir)
            .with_context(|| format!("falha ao limpar {}", output_dir.display()))?;
    }
    fs::create_dir_all(&output_dir)?;

    let mut api = ApiData::default();
    let packs = temp.path().join("packs");
    for scenario in scenarios() {
        println!("\nfixtures-packwiz: pack {}", scenario.name);
        let dir = packs.join(scenario.name);
        fs::create_dir_all(&dir)?;
        build_pack(&ctx, &dir, &scenario, &mut api)?;
        copy_tree(&dir, &output_dir.join(scenario.name))?;
    }
    fs::write(
        output_dir.join("dados-api.json"),
        serde_json::to_string_pretty(&api.to_json())? + "\n",
    )?;

    println!("\nfixtures-packwiz: ignore-100");
    build_ignore_fixture(&ctx, &packs.join("ignore-100"), &out.join("ignore-100"))?;

    println!("\nfixtures-packwiz: vetores do murmur2 (go run)");
    build_murmur2_vectors(temp.path(), &out.join("murmur2"))?;

    fs::write(out.join("FIXTURES.md"), fixtures_md(&commit))?;
    println!("\nfixtures-packwiz: pronto em {}", out.display());
    Ok(())
}

fn resolve_packwiz(root: &Path, explicit: Option<PathBuf>) -> Result<PathBuf> {
    let candidate = explicit
        .or_else(|| std::env::var_os("WARDEN_PACKWIZ_BIN").map(PathBuf::from))
        .unwrap_or_else(|| {
            let triple = if cfg!(windows) {
                "x86_64-pc-windows-msvc.exe"
            } else {
                "x86_64-unknown-linux-gnu"
            };
            root.join("apps/desktop/src-tauri/binaries")
                .join(format!("packwiz-{triple}"))
        });
    ensure!(
        candidate.is_file(),
        "packwiz não encontrado em {}. Rode `cargo xtask build-packwiz` (F0-03) ou passe \
         --packwiz <caminho> / WARDEN_PACKWIZ_BIN.",
        candidate.display()
    );
    Ok(candidate)
}

fn resolve_commit(root: &Path, explicit: Option<String>) -> Result<String> {
    if let Some(commit) = explicit {
        return Ok(commit);
    }
    for path in [
        root.join("third_party/packwiz/COMMIT"),
        root.join("apps/desktop/src-tauri/binaries/packwiz.commit"),
    ] {
        if let Ok(text) = fs::read_to_string(&path) {
            let commit = text.trim().to_owned();
            if !commit.is_empty() {
                return Ok(commit);
            }
        }
    }
    bail!("commit do packwiz desconhecido: passe --commit <hash> (o mesmo da F0-03)")
}

struct Ctx {
    bin: PathBuf,
    config: PathBuf,
    cache: PathBuf,
    cf_key: String,
}

impl Ctx {
    /// Roda o packwiz na pasta do pack, com `--pack-file pack.toml` relativo (o único modo em
    /// que os padrões ancorados do `.packwizignore` funcionam; ver FIXTURES.md).
    fn packwiz(&self, dir: &Path, args: &[&str], curseforge: bool) -> Result<String> {
        self.packwiz_with(dir, OsStr::new("pack.toml"), args, curseforge)
    }

    fn packwiz_with(
        &self,
        dir: &Path,
        pack_file: &OsStr,
        args: &[&str],
        curseforge: bool,
    ) -> Result<String> {
        println!("> packwiz {}", args.join(" "));
        let mut command = Command::new(&self.bin);
        command
            .arg("--config")
            .arg(&self.config)
            .arg("--cache")
            .arg(&self.cache)
            .arg("--pack-file")
            .arg(pack_file)
            .args(args)
            .current_dir(dir)
            .env_remove("WARDEN_CURSEFORGE_API_KEY")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if curseforge {
            command.env("WARDEN_CURSEFORGE_API_KEY", &self.cf_key);
        }
        let mut child = command.spawn().context("falha ao iniciar o packwiz")?;
        if let Some(mut stdin) = child.stdin.take() {
            // Responde "não" à pergunta de dependências: o Warden adiciona as próprias.
            stdin.write_all(b"n\n")?;
        }
        let output = child.wait_with_output()?;
        let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
        if !output.status.success() {
            bail!(
                "packwiz {} terminou com {}:\n{stdout}{}",
                args.join(" "),
                output.status,
                String::from_utf8_lossy(&output.stderr)
            );
        }
        Ok(stdout)
    }
}

/// Um passo na montagem de um pack.
enum Step {
    /// `packwiz init` com loader.
    Init {
        name: &'static str,
        minecraft: &'static str,
        loader: &'static str,
        loader_version: &'static str,
    },
    /// `modrinth add` por IDs; o metafile esperado fica em `metafile`.
    Modrinth {
        project: &'static str,
        version: &'static str,
        metafile: &'static str,
    },
    /// `curseforge add` por IDs.
    CurseForge {
        project: u32,
        file: u32,
        metafile: &'static str,
    },
    /// `url add <nome> <link>`.
    Url {
        name: &'static str,
        url: &'static str,
        metafile: &'static str,
    },
    /// Comando qualquer do packwiz.
    Packwiz(&'static [&'static str]),
    /// Cria um arquivo com o conteúdo dado.
    Write(&'static str, &'static str),
    /// Troca um trecho de texto num arquivo (o packwiz regrava o arquivo depois).
    Replace(&'static str, &'static str, &'static str),
    /// Acrescenta texto ao fim de um arquivo.
    Append(&'static str, &'static str),
    /// Grava o `.packwizignore` padrão do Warden.
    WardenPackwizignore,
}

struct Scenario {
    name: &'static str,
    steps: Vec<Step>,
}

fn init(
    name: &'static str,
    minecraft: &'static str,
    loader: &'static str,
    loader_version: &'static str,
) -> Step {
    Step::Init {
        name,
        minecraft,
        loader,
        loader_version,
    }
}

#[allow(clippy::too_many_lines)] // a lista de cenários é dado, não lógica
fn scenarios() -> Vec<Scenario> {
    vec![
        Scenario {
            name: "fabric-1.21.1",
            steps: vec![
                init("Fixture Fabric", "1.21.1", "fabric", "0.16.14"),
                Step::WardenPackwizignore,
                Step::Modrinth {
                    project: "mOgUt4GM",
                    version: "6lgOkclV",
                    metafile: "mods/modmenu.pw.toml",
                },
                Step::Modrinth {
                    project: "AANobbMI",
                    version: "SMxNOGZ6",
                    metafile: "mods/sodium.pw.toml",
                },
                Step::Modrinth {
                    project: "gvQqBUqZ",
                    version: "N08Z8wog",
                    metafile: "mods/lithium.pw.toml",
                },
                Step::Modrinth {
                    project: "HVnmMxH1",
                    version: "Bqen1mJX",
                    metafile: "shaderpacks/complementary-reimagined.pw.toml",
                },
                Step::Modrinth {
                    project: "50dA9Sha",
                    version: "xN57JJts",
                    metafile: "resourcepacks/fresh-animations.pw.toml",
                },
                Step::CurseForge {
                    project: 324_717,
                    file: 8_591_528,
                    metafile: "mods/jade.pw.toml",
                },
                Step::Url {
                    name: "Text Placeholder API",
                    url: "https://github.com/Patbox/TextPlaceholderAPI/releases/download/2.4.2%2B1.21/placeholder-api-2.4.2%2B1.21.jar",
                    metafile: "mods/text-placeholder-api.pw.toml",
                },
                Step::Packwiz(&["pin", "sodium"]),
                // `[option]` não tem comando: escrito à mão e regravado pelo `pin`/`unpin`.
                Step::Append(
                    "mods/lithium.pw.toml",
                    "\n[option]\noptional = true\ndescription = \"Otimizações do servidor \\\"interno\\\"\"\ndefault = true\n",
                ),
                Step::Packwiz(&["pin", "lithium"]),
                Step::Packwiz(&["unpin", "lithium"]),
                Step::Append("mods/modmenu.pw.toml", "\n[option]\noptional = true\n"),
                Step::Packwiz(&["pin", "modmenu"]),
                Step::Packwiz(&["unpin", "modmenu"]),
                Step::Write("config/exemplo.properties", "chave=valor\r\noutra = 2\n"),
                Step::Write("options.txt", "lang:pt_br\n"),
                Step::Packwiz(&["refresh"]),
                // `preserve` também não tem comando: editado no índice e mantido pelo refresh.
                Step::Replace(
                    "index.toml",
                    "file = \"options.txt\"\n",
                    "file = \"options.txt\"\npreserve = true\n",
                ),
                Step::Packwiz(&["refresh"]),
            ],
        },
        Scenario {
            name: "forge-1.20.1",
            steps: vec![
                init("Fixture Forge", "1.20.1", "forge", "47.4.0"),
                Step::CurseForge {
                    project: 238_222,
                    file: 9_009_995,
                    metafile: "mods/jei.pw.toml",
                },
                Step::CurseForge {
                    project: 223_794,
                    file: 7_148_487,
                    metafile: "mods/applied-energistics-2.pw.toml",
                },
                Step::Modrinth {
                    project: "uXXizFIs",
                    version: "DG5Fn9Sz",
                    metafile: "mods/ferrite-core.pw.toml",
                },
                Step::Packwiz(&["refresh"]),
            ],
        },
        Scenario {
            name: "neoforge-1.21.1",
            steps: vec![
                init("Fixture NeoForge", "1.21.1", "neoforge", "21.1.209"),
                Step::Modrinth {
                    project: "AANobbMI",
                    version: "uMOpc5uV",
                    metafile: "mods/sodium.pw.toml",
                },
                Step::Modrinth {
                    project: "uXXizFIs",
                    version: "x7kQWVju",
                    metafile: "mods/ferrite-core.pw.toml",
                },
                Step::Packwiz(&["refresh"]),
            ],
        },
        Scenario {
            name: "neoforge-1.20.1",
            steps: vec![
                init("Fixture NeoForge legado", "1.20.1", "neoforge", "47.1.106"),
                Step::Modrinth {
                    project: "sk9rgfiA",
                    version: "UTbfe5d1",
                    metafile: "mods/embeddium.pw.toml",
                },
                Step::Packwiz(&["refresh"]),
            ],
        },
        Scenario {
            name: "quilt-1.20.1",
            steps: vec![
                init("Fixture Quilt", "1.20.1", "quilt", "0.26.4"),
                Step::Modrinth {
                    project: "mOgUt4GM",
                    version: "lEkperf6",
                    metafile: "mods/modmenu.pw.toml",
                },
                Step::Packwiz(&["refresh"]),
            ],
        },
        Scenario {
            name: "forge-1.12.2",
            steps: vec![
                init("Fixture Forge 1.12.2", "1.12.2", "forge", "14.23.5.2860"),
                Step::CurseForge {
                    project: 238_222,
                    file: 8_963_414,
                    metafile: "mods/jei.pw.toml",
                },
                Step::Packwiz(&["refresh"]),
            ],
        },
        Scenario {
            // O `init` do packwiz falha com Forge 1.7.10 (issue #393): pack sem loader e a
            // versão escrita à mão, como o Warden fará; o `refresh` regrava o pack.toml.
            name: "forge-1.7.10",
            steps: vec![
                init("Fixture 1.7.10", "1.7.10", "none", ""),
                Step::Replace(
                    "pack.toml",
                    "[versions]\n",
                    "[versions]\nforge = \"10.13.4.1614\"\n",
                ),
                Step::Packwiz(&["refresh"]),
                Step::CurseForge {
                    project: 32_274,
                    file: 8_923_608,
                    metafile: "mods/journeymap.pw.toml",
                },
                Step::Packwiz(&["refresh"]),
            ],
        },
        Scenario {
            name: "opcoes",
            steps: vec![
                init("Fixture Opções", "1.21.1", "fabric", "0.16.14"),
                Step::Packwiz(&["settings", "acceptable-versions", "1.21,1.21.1"]),
                Step::Replace(
                    "pack.toml",
                    "pack-format = ",
                    "description = \"Pack de teste: acentuação, \\\"aspas\\\" e emoji 🔍\"\npack-format = ",
                ),
                Step::Append(
                    "pack.toml",
                    "datapack-folder = \"config/openloader/data\"\nmeta-folder-base = \".\"\n\n[export.curseforge]\nproject-id = 123456\n",
                ),
                Step::Write("config/openloader/data/exemplo/pack.mcmeta", "{}\n"),
                Step::Packwiz(&["refresh"]),
            ],
        },
        Scenario {
            name: "sem-hashes",
            steps: vec![
                init("Fixture sem hashes", "1.21.1", "fabric", "0.16.14"),
                Step::Append("pack.toml", "\n[options]\nno-internal-hashes = true\n"),
                Step::Write("config/a.toml", "a = 1\n"),
                Step::Packwiz(&["refresh"]),
            ],
        },
        Scenario {
            name: "indice-sha512",
            steps: vec![
                init("Fixture índice sha512", "1.21.1", "fabric", "0.16.14"),
                Step::Replace(
                    "index.toml",
                    "hash-format = \"sha256\"",
                    "hash-format = \"sha512\"",
                ),
                Step::Write("config/a.toml", "a = 1\n"),
                Step::Packwiz(&["refresh"]),
            ],
        },
    ]
}

#[derive(Default)]
struct ApiData {
    modrinth: Vec<Value>,
    curseforge: Vec<Value>,
    url: Vec<Value>,
}

impl ApiData {
    fn to_json(&self) -> Value {
        json!({
            "modrinth": self.modrinth,
            "curseforge": self.curseforge,
            "url": self.url,
        })
    }
}

#[allow(clippy::too_many_lines)] // um braço por tipo de passo; dividir esconderia a sequência
fn build_pack(ctx: &Ctx, dir: &Path, scenario: &Scenario, api: &mut ApiData) -> Result<()> {
    for step in &scenario.steps {
        match step {
            Step::Init {
                name,
                minecraft,
                loader,
                loader_version,
            } => {
                let loader_flag = format!("--{loader}-version");
                let mut args = vec![
                    "init",
                    "-y",
                    "--name",
                    name,
                    "--author",
                    "Warden",
                    "--version",
                    "1.0.0",
                    "--mc-version",
                    minecraft,
                    "--modloader",
                    loader,
                ];
                if !loader_version.is_empty() {
                    args.extend([loader_flag.as_str(), loader_version]);
                }
                ctx.packwiz(dir, &args, false)?;
            }
            Step::Modrinth {
                project,
                version,
                metafile,
            } => {
                ctx.packwiz(
                    dir,
                    &[
                        "modrinth",
                        "add",
                        "--project-id",
                        project,
                        "--version-id",
                        version,
                    ],
                    false,
                )?;
                expect_file(dir, metafile)?;
                api.modrinth
                    .push(modrinth_data(scenario.name, metafile, project, version)?);
            }
            Step::CurseForge {
                project,
                file,
                metafile,
            } => {
                let (project_text, file_text) = (project.to_string(), file.to_string());
                ctx.packwiz(
                    dir,
                    &[
                        "curseforge",
                        "add",
                        "--addon-id",
                        &project_text,
                        "--file-id",
                        &file_text,
                    ],
                    true,
                )?;
                expect_file(dir, metafile)?;
                api.curseforge.push(curseforge_data(
                    ctx,
                    scenario.name,
                    metafile,
                    *project,
                    *file,
                )?);
            }
            Step::Url {
                name,
                url,
                metafile,
            } => {
                ctx.packwiz(dir, &["url", "add", name, url], false)?;
                expect_file(dir, metafile)?;
                api.url.push(json!({
                    "pack": scenario.name,
                    "metafile": metafile,
                    "name": name,
                    "url": url,
                }));
            }
            Step::Packwiz(args) => {
                ctx.packwiz(dir, args, false)?;
            }
            Step::Write(path, text) => write_file(&dir.join(path), text)?,
            Step::Replace(path, from, to) => {
                let full = dir.join(path);
                let text = fs::read_to_string(&full)?;
                ensure!(text.contains(from), "{path} não contém {from:?}");
                fs::write(&full, text.replacen(from, to, 1))?;
            }
            Step::Append(path, text) => {
                let full = dir.join(path);
                let mut current = fs::read_to_string(&full)?;
                current.push_str(text);
                fs::write(&full, current)?;
            }
            Step::WardenPackwizignore => {
                write_file(&dir.join(".packwizignore"), &warden_packwizignore()?)?;
            }
        }
    }
    Ok(())
}

fn expect_file(dir: &Path, path: &str) -> Result<()> {
    ensure!(
        dir.join(path).is_file(),
        "o packwiz não criou {path} (o nome do metafile mudou na plataforma?)"
    );
    Ok(())
}

fn write_file(path: &Path, text: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, text).with_context(|| format!("falha ao gravar {}", path.display()))
}

fn copy_tree(from: &Path, to: &Path) -> Result<()> {
    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_tree(&entry.path(), &target)?;
        } else {
            fs::copy(entry.path(), &target)?;
        }
    }
    Ok(())
}

/// GET pelo `curl`, com o cabeçalho da chave (se houver) passado pela entrada padrão.
fn curl(url: &str, api_key: Option<&str>) -> Result<Vec<u8>> {
    let mut command = Command::new(if cfg!(windows) { "curl.exe" } else { "curl" });
    command
        .args([
            "-sSfL",
            "--max-time",
            "120",
            "-A",
            "Kriticales/Warden (desenvolvimento)",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if api_key.is_some() {
        command.args(["-H", "@-"]);
    }
    command.arg(url);
    let mut child = command.spawn().context("falha ao iniciar o curl")?;
    if let (Some(key), Some(mut stdin)) = (api_key, child.stdin.take()) {
        stdin.write_all(format!("x-api-key: {key}\n").as_bytes())?;
    }
    let output = child.wait_with_output()?;
    ensure!(
        output.status.success(),
        "curl {url} terminou com {}: {}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(output.stdout)
}

fn get_json(url: &str, api_key: Option<&str>) -> Result<Value> {
    serde_json::from_slice(&curl(url, api_key)?).with_context(|| format!("JSON inválido de {url}"))
}

fn modrinth_data(pack: &str, metafile: &str, project: &str, version: &str) -> Result<Value> {
    let project_json = get_json(
        &format!("https://api.modrinth.com/v2/project/{project}"),
        None,
    )?;
    let version_json = get_json(
        &format!("https://api.modrinth.com/v2/version/{version}"),
        None,
    )?;
    let files = version_json["files"]
        .as_array()
        .context("versão sem arquivos")?;
    let file = files
        .iter()
        .find(|file| file["primary"].as_bool() == Some(true))
        .or_else(|| files.first())
        .context("versão sem arquivos")?;
    Ok(json!({
        "pack": pack,
        "metafile": metafile,
        "title": project_json["title"],
        "slug": project_json["slug"],
        "project_type": project_json["project_type"],
        "project_id": project,
        "version_id": version,
        "environment": version_json["environment"],
        "loaders": version_json["loaders"],
        "filename": file["filename"],
        "url": file["url"],
        "sha512": file["hashes"]["sha512"],
    }))
}

/// Só os campos que o `.pw.toml` já guarda (respostas da CurseForge não são gravadas).
fn curseforge_data(
    ctx: &Ctx,
    pack: &str,
    metafile: &str,
    project: u32,
    file: u32,
) -> Result<Value> {
    let key = Some(ctx.cf_key.as_str());
    let mod_json = get_json(
        &format!("https://api.curseforge.com/v1/mods/{project}"),
        key,
    )?;
    let file_json = get_json(
        &format!("https://api.curseforge.com/v1/mods/{project}/files/{file}"),
        key,
    )?;
    let sha1 = file_json["data"]["hashes"]
        .as_array()
        .and_then(|hashes| hashes.iter().find(|hash| hash["algo"].as_i64() == Some(1)))
        .map(|hash| hash["value"].clone())
        .context("arquivo da CurseForge sem sha1")?;
    Ok(json!({
        "pack": pack,
        "metafile": metafile,
        "name": mod_json["data"]["name"],
        "slug": mod_json["data"]["slug"],
        "project_id": project,
        "file_id": file,
        "filename": file_json["data"]["fileName"],
        "sha1": sha1,
    }))
}

/// Os 100 caminhos comparados com o `packwiz refresh` (CA-4 da P1-01).
const IGNORE_PATHS: [&str; 100] = [
    // Dados de execução.
    "logs/latest.log",
    "logs/debug-1.log.gz",
    "crash-reports/crash-2026-10-04.txt",
    "saves/Mundo Novo/level.dat",
    "screenshots/2026-10-04_12.00.00.png",
    "debug/profile.txt",
    "natives/lwjgl.dll",
    "libraries/org/x/x.jar",
    "versions/1.21.1/1.21.1.json",
    "assets/indexes/17.json",
    "resources/sons/a.ogg",
    ".mixin.out/class/A.class",
    ".fabric/remappedJars/x.jar",
    ".quilt/x.bin",
    ".cache/y.bin",
    "mods/.connector/z.jar",
    "modernfix/cache.bin",
    "journeymap/data/sp/x.dat",
    "journeymap/config/5.9/journeymap.core.config",
    "XaeroWaypoints/x.txt",
    "XaeroWorldMap/y.txt",
    "xaero/minimap.txt",
    "kubejs/probe/x.json",
    "kubejs/exported/tags.json",
    // Segredos e ferramentas.
    "kubejs/config/web_server.json",
    "kubejs/config/common.json",
    ".probe/a.json",
    ".vscode/settings.json",
    "local/kubejs/a.js",
    "local/outro.txt",
    "config/spark/perfil.sparkprofile",
    "config/spark/heap.sparkheap",
    "config/spark/tmp-123/a.bin",
    "config/spark/config.json",
    // Servidor e instalador.
    "server-overrides/server.properties",
    "usercache.json",
    "usernamecache.json",
    "launcher_profiles_microsoft.json",
    "servers.dat_old",
    "command_history.txt",
    "packwiz.json",
    "packwiz-installer-bootstrap.jar",
    ".packwiz.toml",
    "config/usercache.json",
    // Em qualquer profundidade.
    "kubejs/jsconfig.json",
    "config/a.log",
    "config/b.log.gz",
    "hs_err_pid123.log",
    "replay_pid9.log",
    "java.heapdump",
    "config/Thumbs.db",
    "desktop.ini",
    "config/x.tmp",
    "config/y.bak",
    "config/z.old",
    "mods/foo.jar.disabled",
    "mods/keep.disabled",
    // Bloco obrigatório.
    ".warden/estado.json",
    "CHANGELOG.md",
    "README.md",
    "config/a.warden-tmp",
    "docs/README.md",
    // Padrões embutidos do packwiz.
    "export.zip",
    "mods/a.zip",
    "pack-1.0.0.mrpack",
    "packwiz.exe",
    "packwiz/x.txt",
    ".gitattributes",
    ".DS_Store",
    // Conteúdo que fica.
    "mods/sodium.pw.toml",
    "mods/local.jar",
    "config/sodium-options.json",
    "config/debug/x.toml",
    "config/logs/x.txt",
    "kubejs/assets/x/textures/y.png",
    "kubejs/server_scripts/a.js",
    "resourcepacks/pack.zip",
    "shaderpacks/s.zip",
    "defaultconfigs/a.toml",
    "options.txt",
    "servers.dat",
    // Padrões do usuário (semântica do go-gitignore).
    "config/private/a.json",
    "config/private/keep.json",
    "config/private/sub/b.json",
    "tools/node_modules/x.js",
    "node_modules/y.js",
    "secret1.txt",
    "data/a/b/cache/x.bin",
    "data/cache/y.bin",
    "notas1.txt",
    "notas3.txt",
    "build/out.txt",
    "build/keep.txt",
    "src/build/a.txt",
    "espaco.txt",
    "#hash.txt",
    "#comentario.txt",
    "tempx.txt",
    "temp{1}x.txt",
    "Ünïcödé/arquivo ção.txt",
];

/// Linhas do usuário acrescentadas ao `.packwizignore` padrão no teste dos 100 caminhos.
const IGNORE_USER_LINES: &str = "# Linhas do usuário:
config/private/*.json
!config/private/keep.json
**/node_modules
secret?.txt
data/**/cache
notas[12].txt
build
!build/keep.txt
  espaco.txt
#comentario.txt
\\#hash.txt
temp{1}x.txt
!mods/keep.disabled
";

fn build_ignore_fixture(ctx: &Ctx, dir: &Path, out: &Path) -> Result<()> {
    let unique: BTreeSet<&str> = IGNORE_PATHS.iter().copied().collect();
    ensure!(
        unique.len() == IGNORE_PATHS.len(),
        "caminhos repetidos na lista"
    );
    let ignore_text = warden_packwizignore()? + IGNORE_USER_LINES;

    let mut results = Vec::new();
    for absolute in [false, true] {
        let pack = dir.join(if absolute { "absoluto" } else { "relativo" });
        fs::create_dir_all(&pack)?;
        ctx.packwiz(
            &pack,
            &[
                "init",
                "-y",
                "--name",
                "Ignore",
                "--mc-version",
                "1.21.1",
                "--modloader",
                "none",
            ],
            false,
        )?;
        fs::write(pack.join(".packwizignore"), &ignore_text)?;
        fs::create_dir_all(pack.join(".git"))?;
        fs::write(pack.join(".git/config"), "x\n")?;
        for path in IGNORE_PATHS {
            write_file(&pack.join(path), "x\n")?;
        }
        if absolute {
            let pack_file = pack.join("pack.toml");
            ctx.packwiz_with(&pack, pack_file.as_os_str(), &["refresh"], false)?;
        } else {
            ctx.packwiz(&pack, &["refresh"], false)?;
        }
        let index = fs::read_to_string(pack.join("index.toml"))?;
        results.push(indexed_files(&index)?);
    }

    fs::create_dir_all(out)?;
    fs::write(out.join("caminhos.txt"), IGNORE_PATHS.join("\n") + "\n")?;
    fs::write(out.join("packwizignore.txt"), &ignore_text)?;
    let lines = |set: &BTreeSet<String>| set.iter().cloned().collect::<Vec<_>>().join("\n") + "\n";
    fs::write(
        out.join("indexados-pack-file-relativo.txt"),
        lines(&results[0]),
    )?;
    fs::write(
        out.join("indexados-pack-file-absoluto.txt"),
        lines(&results[1]),
    )?;
    println!(
        "ignore-100: {} de 100 no índice (relativo), {} (absoluto)",
        results[0].len(),
        results[1].len()
    );
    Ok(())
}

/// Arquivos listados num `index.toml`.
fn indexed_files(index: &str) -> Result<BTreeSet<String>> {
    let parsed: toml::Table = toml::from_str(index).context("index.toml do packwiz ilegível")?;
    let files = parsed
        .get("files")
        .and_then(toml::Value::as_array)
        .context("index.toml sem `files`")?;
    files
        .iter()
        .map(|entry| {
            entry
                .get("file")
                .and_then(toml::Value::as_str)
                .map(str::to_owned)
                .context("entrada do índice sem `file`")
        })
        .collect()
}

/// O `.packwizignore` padrão do Warden, lido do bloco da ARCHITECTURE §6.4 (a mesma fonte que
/// a `warden-packwiz` reproduz e confere nos testes).
fn warden_packwizignore() -> Result<String> {
    let path = workspace_root().join("docs/ARCHITECTURE.md");
    let text =
        fs::read_to_string(&path).with_context(|| format!("falha ao ler {}", path.display()))?;
    extract_packwizignore(&text)
        .context("bloco do `.packwizignore` não encontrado na ARCHITECTURE §6.4")
}

/// O primeiro bloco de código `gitignore` depois do título do `.packwizignore` na §6.4.
fn extract_packwizignore(architecture: &str) -> Option<String> {
    let section = &architecture[architecture.find("### 6.4")?..];
    let after_title = &section[section.find("`.packwizignore`")?..];
    let start = after_title.find(
        "```gitignore
",
    )? + "```gitignore
"
    .len();
    let block = &after_title[start..];
    let end = block.find("```")?;
    Some(block[..end].to_owned())
}

const MURMUR2_GO: &str = r##"// Gera vetores do murmur2 da CurseForge com a biblioteca que o packwiz usa.
package main

import (
	"encoding/hex"
	"fmt"

	murmur "github.com/aviddiviner/go-murmur"
)

// Igual a curseforge/murmur2 do packwiz: tira tab, LF, CR e espaço; semente 1.
func cf(data []byte) uint32 {
	var kept []byte
	for _, b := range data {
		if b != 9 && b != 10 && b != 13 && b != 32 {
			kept = append(kept, b)
		}
	}
	return murmur.MurmurHash2(kept, 1)
}

func main() {
	state := uint32(2463534242)
	next := func() byte {
		state ^= state << 13
		state ^= state >> 17
		state ^= state << 5
		return byte(state)
	}
	lengths := []int{}
	for n := 0; n <= 40; n++ {
		lengths = append(lengths, n)
	}
	lengths = append(lengths, 63, 64, 65, 127, 128, 255, 256, 1000, 1023)
	fmt.Println("# tamanho hex murmur2_curseforge murmur2_semente_1_sem_filtro")
	for _, n := range lengths {
		data := make([]byte, n)
		for i := range data {
			b := next()
			// Um terço dos bytes vira espaço em branco, para exercitar o filtro.
			if b%3 == 0 {
				b = []byte{9, 10, 13, 32}[b%4]
			}
			data[i] = b
		}
		fmt.Printf("%d %s %d %d\n", n, hex.EncodeToString(data), cf(data), murmur.MurmurHash2(data, 1))
	}
}
"##;

fn build_murmur2_vectors(temp: &Path, out: &Path) -> Result<()> {
    let dir = temp.join("murmur2-go");
    fs::create_dir_all(&dir)?;
    fs::write(dir.join("main.go"), MURMUR2_GO)?;
    fs::write(
        dir.join("go.mod"),
        format!(
            "module vetores\n\ngo 1.23\n\nrequire github.com/aviddiviner/go-murmur {GO_MURMUR_VERSION}\n"
        ),
    )?;
    let go = if cfg!(windows) { "go.exe" } else { "go" };
    let status = Command::new(go)
        .args(["mod", "download", "github.com/aviddiviner/go-murmur"])
        .current_dir(&dir)
        .status()
        .context("falha ao rodar o Go")?;
    ensure!(status.success(), "go mod download falhou");
    let tidy = Command::new(go)
        .args(["mod", "tidy"])
        .current_dir(&dir)
        .status()?;
    ensure!(tidy.success(), "go mod tidy falhou");
    let output = Command::new(go)
        .args(["run", "."])
        .current_dir(&dir)
        .output()?;
    ensure!(
        output.status.success(),
        "go run falhou: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    fs::create_dir_all(out)?;
    fs::write(out.join("vetores.txt"), &output.stdout)?;
    fs::write(out.join("gerar_vetores.go.txt"), MURMUR2_GO)?;
    Ok(())
}

fn fixtures_md(commit: &str) -> String {
    format!(
        "# Fixtures da `warden-packwiz`

Geradas por `cargo xtask fixtures-packwiz` (código em `xtask/src/fixtures_packwiz.rs`, com a
lista exata de comandos e IDs). Não edite à mão: rode o comando de novo. O `.gitattributes` da
raiz marca esta pasta com `-text`, para os bytes (e os hashes do `index.toml`) não mudarem.

## Origem

- **packwiz** `packwiz/packwiz` (MIT), commit `{commit}`, compilado pela F0-03 com o patch
  `0001-chave-curseforge-em-tempo-de-execucao.patch` (chave da CurseForge do `.env` do dono,
  passada por variável de ambiente; nenhuma chave fica gravada aqui).
- Chamado sempre de dentro da pasta do pack com `--pack-file pack.toml` relativo,
  `--config` vazio e `--cache` temporário.
- Os mods são escolhidos por ID de projeto e de versão (Modrinth) ou de projeto e de arquivo
  (CurseForge). Nenhum jar é versionado: os metafiles só têm nome, link, hash e IDs.

## Pastas

- `packwiz-output/<pack>/`: o pack inteiro como o packwiz deixou (`pack.toml`, `index.toml`,
  metafiles e os arquivos comuns usados). Cobre Fabric, Forge (1.7.10, 1.12.2, 1.20.1),
  NeoForge (1.20.1 e 1.21.1), Quilt, Modrinth (mod, resource pack, shader), CurseForge,
  link direto, `pin`, `[option]`, `preserve`, `[options]`, `[export.curseforge]`,
  `description`, `no-internal-hashes` e índice com `hash-format` diferente de `sha256`.
- `packwiz-output/dados-api.json`: dados das APIs usados em cada metafile. Do Modrinth, o
  necessário para montar o metafile (inclusive `environment`, que decide o lado); da
  CurseForge, só nome, slug, IDs, nome do arquivo e `sha1` (os termos da CurseForge não
  permitem guardar as respostas; esses campos já estão no próprio `.pw.toml`).
- `ignore-100/`: 100 caminhos (`caminhos.txt`), o `.packwizignore` usado
  (`packwizignore.txt`: o padrão do Warden mais linhas do usuário) e os arquivos que o
  `packwiz refresh` pôs no índice: `indexados-pack-file-relativo.txt` (chamado com
  `--pack-file pack.toml`) e `indexados-pack-file-absoluto.txt` (chamado com o caminho
  absoluto do `pack.toml`).
- `murmur2/vetores.txt`: tamanho, dados em hexadecimal, murmur2 da CurseForge e murmur2 puro
  (semente 1), calculados pela biblioteca Go que o packwiz usa
  (`github.com/aviddiviner/go-murmur` {GO_MURMUR_VERSION}, MIT) com o programa
  `murmur2/gerar_vetores.go.txt`.

## Achado: `--pack-file` absoluto quebra os padrões ancorados

O packwiz compara o `.packwizignore` com o caminho que o percurso da pasta produz. Com
`--pack-file pack.toml` (relativo), esse caminho é relativo à pasta do pack e `/logs/` funciona.
Com `--pack-file C:\\...\\pack.toml` (absoluto), o caminho é absoluto e todo padrão ancorado
(começando por `/`) deixa de casar: compare os dois arquivos `indexados-*` de `ignore-100/`.
O Warden deve chamar o packwiz com o `pack.toml` relativo e a pasta do pack como pasta atual.
"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modelo_lido_da_arquitetura() {
        let text = warden_packwizignore().unwrap();
        assert!(text.starts_with("# Gerado pelo Warden."));
        assert!(text.contains(
            "/kubejs/config/web_server.json
"
        ));
        assert!(text.ends_with(
            "*.disabled
"
        ));
        assert!(extract_packwizignore("sem seção").is_none());
    }

    #[test]
    fn indice_lido() {
        let files = indexed_files(
            "hash-format = \"sha256\"
[[files]]
file = \"a\"
",
        )
        .unwrap();
        assert_eq!(files.into_iter().collect::<Vec<_>>(), ["a"]);
        assert!(indexed_files("files = 1").is_err());
    }

    #[test]
    fn cem_caminhos_distintos() {
        let unique: BTreeSet<&str> = IGNORE_PATHS.iter().copied().collect();
        assert_eq!(unique.len(), 100);
    }
}
