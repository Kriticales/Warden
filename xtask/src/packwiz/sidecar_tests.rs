//! Testes com os executáveis reais gerados por `cargo xtask build-packwiz` (critérios da F0-03
//! no ROADMAP).
//!
//! Sem os executáveis, os testes pulam com aviso; com `WARDEN_REQUIRE_EXTERNALS=1` (CI),
//! falham (QUALITY §4.1). Os testes `rede_*` usam a CurseForge e o Mojang de verdade e só rodam
//! com `cargo xtask test-network`, que carrega o `.env` sem imprimir valores.
//!
//! Cada execução do packwiz usa uma pasta temporária própria, ambiente limpo (como o Warden
//! fará, ARCHITECTURE §6.3), `--cache` e `--config` isolados e entrada padrão fechada. A chave
//! nunca aparece em mensagem de teste: as saídas passam por [`redact`] antes.
//!
//! Com `--pack-file` absoluto, o packwiz precisa também de `--meta-folder-base` absoluto: sem
//! ele, todo `add` falha em `filepath.Rel` (bug do packwiz original, provado em
//! `rede_pack_file_absoluto_exige_meta_folder_base`).

use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use super::*;

/// Executáveis do build, ou `None` (com aviso) se ainda não foram gerados.
struct Built {
    layout: Layout,
    host: &'static Target,
}

impl Built {
    fn get() -> Option<Self> {
        let layout = Layout::current().expect("layout do workspace");
        let host = host_target().expect("sistema sem alvo do sidecar");
        let mut missing: Vec<PathBuf> = TARGETS
            .iter()
            .map(|target| layout.binary(target))
            .chain(layout.reference())
            .filter(|path| !path.is_file())
            .collect();
        if !layout.source().join(".git").is_dir() {
            missing.push(layout.source());
        }
        if missing.is_empty() {
            return Some(Self { layout, host });
        }
        let message = format!(
            "sidecar do packwiz não compilado (falta {}); rode `cargo xtask build-packwiz`",
            missing[0].display()
        );
        assert!(
            std::env::var("WARDEN_REQUIRE_EXTERNALS").as_deref() != Ok("1"),
            "{message}"
        );
        eprintln!("AVISO: teste pulado: {message}");
        None
    }

    fn patched(&self) -> PathBuf {
        self.layout.binary(self.host)
    }

    fn reference(&self) -> PathBuf {
        self.layout
            .reference()
            .expect("referência do sistema atual")
    }

    fn needles(&self) -> Vec<Vec<u8>> {
        let inputs = read_inputs(&self.layout.third_party).unwrap();
        let git = find_program("git").unwrap();
        original_key_needles(&git, &self.layout.source(), &inputs.commit).unwrap()
    }
}

/// Troca a chave por `***` (para mensagens de falha).
fn redact(text: &str, key: Option<&str>) -> String {
    match key {
        Some(key) if !key.trim().is_empty() => text.replace(key, "***"),
        _ => text.to_owned(),
    }
}

/// Pasta temporária com um pack, um "home", um cache e um config vazio só deste teste.
struct Sandbox {
    dir: tempfile::TempDir,
}

impl Sandbox {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        for sub in ["pack", "home", "cache", "tmp"] {
            std::fs::create_dir_all(dir.path().join(sub)).unwrap();
        }
        std::fs::write(dir.path().join("packwiz.toml"), "").unwrap();
        Self { dir }
    }

    fn pack(&self) -> PathBuf {
        self.dir.path().join("pack")
    }

    /// Roda o packwiz com o ambiente limpo e `--meta-folder-base` absoluto; `key` vira
    /// `WARDEN_CURSEFORGE_API_KEY`.
    fn run(&self, bin: &Path, args: &[&str], key: Option<&str>) -> Output {
        self.run_with(bin, args, key, true)
    }

    fn run_with(&self, bin: &Path, args: &[&str], key: Option<&str>, meta_base: bool) -> Output {
        let home = self.dir.path().join("home");
        let tmp = self.dir.path().join("tmp");
        let mut command = Command::new(bin);
        command.env_clear();
        for name in ["SYSTEMROOT", "PATH"] {
            if let Some(value) = std::env::var_os(name) {
                command.env(name, value);
            }
        }
        for name in [
            "HOME",
            "USERPROFILE",
            "APPDATA",
            "LOCALAPPDATA",
            "XDG_CONFIG_HOME",
            "XDG_CACHE_HOME",
        ] {
            command.env(name, &home);
        }
        command
            .env("TEMP", &tmp)
            .env("TMP", &tmp)
            .env("TMPDIR", &tmp);
        if let Some(key) = key {
            command.env(KEY_ENV, key);
        }
        if meta_base {
            command.arg("--meta-folder-base").arg(self.pack());
        }
        command
            .arg("--pack-file")
            .arg(self.pack().join("pack.toml"))
            .arg("--cache")
            .arg(self.dir.path().join("cache"))
            .arg("--config")
            .arg(self.dir.path().join("packwiz.toml"))
            .args(args)
            .current_dir(self.pack())
            .stdin(Stdio::null())
            .output()
            .unwrap()
    }

    /// Pack Forge 1.20.1 escrito à mão, para testes sem rede. O metadado do mod é sintético
    /// (o `refresh` não baixa nada; só calcula os hashes dos arquivos do pack).
    fn write_offline_pack(&self) {
        let pack = self.pack();
        let files: &[(&str, &str)] = &[
            (
                "pack.toml",
                "name = \"Teste do sidecar\"\nauthor = \"Warden\"\nversion = \"1.0.0\"\n\
                 pack-format = \"packwiz:1.1.0\"\n\n[index]\nfile = \"index.toml\"\n\
                 hash-format = \"sha256\"\nhash = \"\"\n\n[versions]\nforge = \"47.3.0\"\n\
                 minecraft = \"1.20.1\"\n",
            ),
            ("index.toml", "hash-format = \"sha256\"\n"),
            (
                "mods/exemplo.pw.toml",
                "name = \"Exemplo\"\nfilename = \"exemplo-1.0.0.jar\"\nside = \"both\"\n\n\
                 [download]\nurl = \"https://cdn.modrinth.com/data/AAAAAAAA/versions/BBBBBBBB/exemplo-1.0.0.jar\"\n\
                 hash-format = \"sha1\"\nhash = \"da39a3ee5e6b4b0d3255bfef95601890afd80709\"\n\n\
                 [update]\n[update.modrinth]\nmod-id = \"AAAAAAAA\"\nversion = \"BBBBBBBB\"\n",
            ),
            ("config/exemplo.toml", "[geral]\r\nativo = true\r\n"),
            (
                "config/pasta com espaço/opções.txt",
                "conteúdo em UTF-8: ção\n",
            ),
        ];
        for (path, text) in files {
            let path = pack.join(path);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, text).unwrap();
        }
    }

    fn read(&self, path: &str) -> String {
        std::fs::read_to_string(self.pack().join(path)).unwrap()
    }
}

fn text(output: &Output, key: Option<&str>) -> String {
    let mut text = String::from_utf8_lossy(&output.stdout).into_owned();
    text.push_str(&String::from_utf8_lossy(&output.stderr));
    redact(&text, key)
}

#[test]
fn ca1_executavel_do_sistema_roda_help() {
    let Some(built) = Built::get() else { return };
    let sandbox = Sandbox::new();
    let output = sandbox.run(&built.patched(), &["--help"], None);
    let out = text(&output, None);
    assert!(output.status.success(), "{out}");
    assert!(out.contains("packwiz"), "{out}");
    assert!(out.contains("curseforge"), "{out}");
    assert!(out.contains("refresh"), "{out}");
}

#[test]
fn ca1_exe_do_windows_e_pe32_plus_de_console() {
    let Some(built) = Built::get() else { return };
    let bytes = std::fs::read(built.layout.binary(&TARGETS[0])).unwrap();
    assert_eq!(
        exe::check(ExeFormat::Pe, &bytes).unwrap(),
        exe::ExeInfo::Pe {
            machine: exe::PE_MACHINE_AMD64,
            optional_magic: exe::PE32_PLUS_MAGIC,
            subsystem: exe::PE_SUBSYSTEM_CONSOLE,
        }
    );
}

#[test]
fn ca1_binario_do_linux_e_elf64_estatico() {
    let Some(built) = Built::get() else { return };
    let bytes = std::fs::read(built.layout.binary(&TARGETS[1])).unwrap();
    let info = exe::check(ExeFormat::Elf, &bytes).unwrap();
    assert!(matches!(
        info,
        exe::ExeInfo::Elf {
            class64: true,
            little_endian: true,
            machine: exe::ELF_MACHINE_X86_64,
            has_interp: false,
            ..
        }
    ));
}

#[test]
fn ca2_sem_chave_curseforge_add_falha_com_a_mensagem_do_patch() {
    let Some(built) = Built::get() else { return };
    // Sem a variável e com a variável só com espaços: os dois falham antes de usar a rede.
    for key in [None, Some("   ")] {
        let sandbox = Sandbox::new();
        sandbox.write_offline_pack();
        let index_before = sandbox.read("index.toml");
        let output = sandbox.run(&built.patched(), &["curseforge", "add", "jei", "-y"], key);
        let out = text(&output, None);
        assert!(!output.status.success(), "{key:?}: {out}");
        assert!(out.contains(MISSING_KEY_MESSAGE), "{key:?}: {out}");
        assert!(out.contains(KEY_ENV), "{out}");
        assert!(!sandbox.pack().join("mods").join("jei.pw.toml").exists());
        assert_eq!(sandbox.read("index.toml"), index_before);
    }
}

#[test]
fn ca2_chave_embutida_do_packwiz_nao_existe_nos_executaveis() {
    let Some(built) = Built::get() else { return };
    let needles = built.needles();
    assert_eq!(needles.len(), 2, "base64 e decodificada");
    // Controle positivo: a busca acha a chave no executável sem patch.
    let reference = std::fs::read(built.reference()).unwrap();
    assert!(contains(&reference, &needles[0]));
    assert!(!contains(&reference, MISSING_KEY_MESSAGE.as_bytes()));
    for target in TARGETS {
        let bytes = std::fs::read(built.layout.binary(target)).unwrap();
        for (index, needle) in needles.iter().enumerate() {
            assert!(
                !contains(&bytes, needle),
                "{} contém a chave (forma {index})",
                target.file_name()
            );
        }
        assert!(contains(&bytes, MISSING_KEY_MESSAGE.as_bytes()));
        verify_patched(target, &bytes, &needles).unwrap();
    }
}

/// Arquivos do pack, em ordem, com o conteúdo (para comparar dois packs).
fn snapshot(root: &Path) -> Vec<(String, Vec<u8>)> {
    fn walk(root: &Path, dir: &Path, files: &mut Vec<(String, Vec<u8>)>) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                walk(root, &path, files);
            } else {
                let name = path.strip_prefix(root).unwrap().to_string_lossy();
                files.push((name.replace('\\', "/"), std::fs::read(&path).unwrap()));
            }
        }
    }
    let mut files = Vec::new();
    walk(root, root, &mut files);
    files.sort();
    files
}

#[test]
fn ca3_refresh_igual_ao_executavel_sem_patch() {
    let Some(built) = Built::get() else { return };
    let mut results = Vec::new();
    for bin in [built.patched(), built.reference()] {
        let sandbox = Sandbox::new();
        sandbox.write_offline_pack();
        let output = sandbox.run(&bin, &["refresh"], None);
        let out = text(&output, None);
        assert!(output.status.success(), "{}: {out}", bin.display());
        results.push((snapshot(&sandbox.pack()), sandbox));
    }
    assert_eq!(results[0].0, results[1].0);

    let index = results[0].1.read("index.toml");
    for file in [
        "config/exemplo.toml",
        "config/pasta com espaço/opções.txt",
        "mods/exemplo.pw.toml",
    ] {
        assert!(index.contains(&format!("file = \"{file}\"")), "{index}");
    }
    assert!(index.contains("metafile = true"), "{index}");
    let pack = results[0].1.read("pack.toml");
    assert!(
        !pack.contains("hash = \"\""),
        "o refresh grava o hash do índice: {pack}"
    );
}

/// Chave da CurseForge do `.env` (carregado pelo `cargo xtask test-network`).
fn network_key() -> String {
    std::env::var(KEY_ENV)
        .or_else(|_| std::env::var("CURSEFORGE_API_KEY"))
        .ok()
        .filter(|key| !key.trim().is_empty())
        .expect("teste de rede sem CURSEFORGE_API_KEY: rode com `cargo xtask test-network`")
}

#[test]
#[ignore = "rede"]
fn rede_ca2_com_a_chave_do_env_adiciona_o_jei() {
    let Some(built) = Built::get() else { return };
    let key = network_key();
    let sandbox = Sandbox::new();
    sandbox.write_offline_pack();
    let output = sandbox.run(
        &built.patched(),
        &["curseforge", "add", "jei", "-y"],
        Some(&key),
    );
    let out = text(&output, Some(&key));
    assert!(output.status.success(), "{out}");
    let meta = sandbox.read("mods/jei.pw.toml");
    assert!(meta.contains("[update.curseforge]"), "{meta}");
    assert!(meta.contains("project-id = 238222"), "{meta}");
    assert!(!meta.contains(&key));
    assert!(sandbox.read("index.toml").contains("mods/jei.pw.toml"));
}

#[test]
#[ignore = "rede"]
fn rede_ca2_chave_invalida_e_recusada_sem_recuo_para_a_embutida() {
    let Some(built) = Built::get() else { return };
    let sandbox = Sandbox::new();
    sandbox.write_offline_pack();
    let key = "chave-invalida-do-teste-do-warden";
    let output = sandbox.run(
        &built.patched(),
        &["curseforge", "add", "jei", "-y"],
        Some(key),
    );
    let out = text(&output, None);
    assert!(!output.status.success(), "{out}");
    assert!(out.contains("invalid response status: 403"), "{out}");
    assert!(!sandbox.pack().join("mods").join("jei.pw.toml").exists());
}

#[test]
#[ignore = "rede"]
fn rede_ca3_init_igual_ao_executavel_sem_patch() {
    let Some(built) = Built::get() else { return };
    let args = [
        "init",
        "--name",
        "Teste do sidecar",
        "--author",
        "Warden",
        "--version",
        "1.0.0",
        "--mc-version",
        "1.20.1",
        "--modloader",
        "forge",
        "--forge-version",
        "47.3.0",
        "-y",
    ];
    let mut snapshots = Vec::new();
    for bin in [built.patched(), built.reference()] {
        let sandbox = Sandbox::new();
        let output = sandbox.run(&bin, &args, None);
        let out = text(&output, None);
        assert!(output.status.success(), "{}: {out}", bin.display());
        snapshots.push(snapshot(&sandbox.pack()));
    }
    assert_eq!(snapshots[0], snapshots[1]);
    let pack = String::from_utf8_lossy(
        &snapshots[0]
            .iter()
            .find(|(name, _)| name == "pack.toml")
            .expect("pack.toml criado")
            .1,
    )
    .into_owned();
    assert!(pack.contains("forge = \"47.3.0\""), "{pack}");
    assert!(pack.contains("minecraft = \"1.20.1\""), "{pack}");
}

#[test]
#[ignore = "rede"]
fn rede_pack_file_absoluto_exige_meta_folder_base() {
    let Some(built) = Built::get() else { return };
    // Mesmo comportamento com e sem patch: o defeito é do packwiz original.
    for bin in [built.patched(), built.reference()] {
        let sandbox = Sandbox::new();
        sandbox.write_offline_pack();
        let index_before = sandbox.read("index.toml");
        let args = ["modrinth", "add", "jei", "-y"];
        let output = sandbox.run_with(&bin, &args, None, false);
        let out = text(&output, None);
        assert!(!output.status.success(), "{}: {out}", bin.display());
        assert!(out.contains("Rel: can't make"), "{out}");
        // Um metadado novo (o da primeira dependência) fica gravado, mas o índice não muda:
        // o pack fica inconsistente.
        let orphans: Vec<_> = snapshot(&sandbox.pack())
            .into_iter()
            .map(|(name, _)| name)
            .filter(|name| name.ends_with(".pw.toml") && name != "mods/exemplo.pw.toml")
            .collect();
        assert!(!orphans.is_empty(), "nenhum metadado órfão");
        assert_eq!(sandbox.read("index.toml"), index_before);

        let sandbox = Sandbox::new();
        sandbox.write_offline_pack();
        let output = sandbox.run(&bin, &args, None);
        let out = text(&output, None);
        assert!(output.status.success(), "{}: {out}", bin.display());
        assert!(sandbox.read("index.toml").contains("mods/jei.pw.toml"));
    }
}

#[test]
fn redacao_tira_a_chave() {
    assert_eq!(redact("a SEGREDO b", Some("SEGREDO")), "a *** b");
    assert_eq!(redact("a b", Some("  ")), "a b");
    assert_eq!(redact("a b", None), "a b");
}
