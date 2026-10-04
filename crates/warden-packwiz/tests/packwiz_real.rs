//! Integração com o packwiz real (QUALITY §4.1): `WARDEN_PACKWIZ_BIN` aponta o binário
//! compilado pela F0-03. Sem ele, os testes avisam e passam; com `WARDEN_REQUIRE_EXTERNALS=1`
//! (CI), a falta do binário é falha.
//!
//! O packwiz é chamado como o Warden deve chamá-lo: pasta do pack como pasta atual e
//! `--pack-file pack.toml` relativo (ver `tests/fixtures/FIXTURES.md`).

// Nos auxiliares dos testes, `unwrap`, `expect` e `panic!` são a forma idiomática de falhar
// (o `clippy.toml` só os libera dentro das funções `#[test]`).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::print_stderr
)]

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use warden_packwiz::hash::hash_bytes;
use warden_packwiz::hygiene::{HygieneReason, PatternCategory, packwizignore_template, scan_dir};
use warden_packwiz::{
    HashFormat, IndexEntry, Metafile, ModrinthFile, PackIndex, PackManifest, PackwizIgnore, Side,
    edit,
};

fn packwiz() -> Option<PathBuf> {
    match std::env::var_os("WARDEN_PACKWIZ_BIN") {
        Some(path) if Path::new(&path).is_file() => Some(PathBuf::from(path)),
        _ => {
            assert!(
                std::env::var_os("WARDEN_REQUIRE_EXTERNALS").is_none_or(|value| value != "1"),
                "WARDEN_REQUIRE_EXTERNALS=1, mas WARDEN_PACKWIZ_BIN não aponta um binário"
            );
            eprintln!("AVISO: WARDEN_PACKWIZ_BIN ausente; teste com o packwiz real pulado.");
            None
        }
    }
}

fn refresh(bin: &Path, pack: &Path, temp: &Path) {
    let config = temp.join("packwiz.toml");
    fs::write(&config, "").unwrap();
    let output = Command::new(bin)
        .arg("--config")
        .arg(&config)
        .arg("--cache")
        .arg(temp.join("cache"))
        .args(["--pack-file", "pack.toml", "refresh"])
        .current_dir(pack)
        .env_remove("WARDEN_CURSEFORGE_API_KEY")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "packwiz refresh falhou: {}",
        String::from_utf8_lossy(&output.stdout)
    );
}

fn write(root: &Path, path: &str, text: &str) {
    let full = root.join(path);
    fs::create_dir_all(full.parent().unwrap()).unwrap();
    fs::write(full, text).unwrap();
}

fn indexed(pack: &Path) -> BTreeSet<String> {
    let text = fs::read_to_string(pack.join("index.toml")).unwrap();
    PackIndex::parse(&text)
        .unwrap()
        .value
        .files
        .into_iter()
        .map(|entry| entry.file)
        .collect()
}

/// Pack novo escrito só pelo Warden: `pack.toml`, `index.toml` vazio e o `.packwizignore`.
fn new_pack(root: &Path) {
    let mut pack = PackManifest::new("Teste ao vivo", "1.21.1");
    "Warden".clone_into(&mut pack.author);
    "1.0.0".clone_into(&mut pack.version);
    pack.set_loader_version(warden_packwiz::Loader::Fabric, "0.16.14");
    fs::create_dir_all(root).unwrap();
    fs::write(root.join("pack.toml"), pack.to_toml_string()).unwrap();
    fs::write(
        root.join("index.toml"),
        PackIndex::default().to_toml_string(),
    )
    .unwrap();
    fs::write(root.join(".packwizignore"), packwizignore_template()).unwrap();
}

#[test]
fn ca_4_cem_caminhos_ao_vivo() {
    let Some(bin) = packwiz() else { return };
    let temp = tempfile::tempdir().unwrap();
    let pack = temp.path().join("pack");
    new_pack(&pack);
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/ignore-100");
    let ignore_text = fs::read_to_string(fixtures.join("packwizignore.txt")).unwrap();
    fs::write(pack.join(".packwizignore"), &ignore_text).unwrap();
    let paths = fs::read_to_string(fixtures.join("caminhos.txt")).unwrap();
    for path in paths.lines() {
        write(&pack, path, "x\n");
    }
    refresh(&bin, &pack, temp.path());
    let ignore = PackwizIgnore::for_pack(Some(&ignore_text));
    let ours: BTreeSet<String> = paths
        .lines()
        .filter(|path| !ignore.is_excluded(path))
        .map(str::to_owned)
        .collect();
    assert_eq!(indexed(&pack), ours);
}

#[test]
fn ca_t04_05_modelo_tira_segredos_do_indice_ao_vivo() {
    let Some(bin) = packwiz() else { return };
    let temp = tempfile::tempdir().unwrap();
    let pack = temp.path().join("pack");
    new_pack(&pack);
    let secrets = [
        "kubejs/config/web_server.json",
        ".probe/sessao.json",
        ".vscode/settings.json",
        "local/kubejs/a.js",
        "config/spark/perfil.sparkprofile",
    ];
    for path in secrets {
        write(&pack, path, "segredo\n");
    }
    write(&pack, "config/ok.toml", "a = 1\n");
    refresh(&bin, &pack, temp.path());
    assert_eq!(
        indexed(&pack),
        BTreeSet::from(["config/ok.toml".to_owned()])
    );
    let items = scan_dir(&pack, None).unwrap();
    for expected in [
        "kubejs/config/web_server.json",
        ".probe",
        ".vscode",
        "local/kubejs",
        "config/spark/perfil.sparkprofile",
    ] {
        let item = items.iter().find(|item| item.path == expected);
        let item = item.unwrap_or_else(|| panic!("{expected} não apontado: {items:?}"));
        assert!(item.reasons.iter().any(|reason| matches!(
            reason,
            HygieneReason::MatchesPattern {
                category: PatternCategory::SecretsAndTools,
                ..
            }
        )));
    }
}

#[test]
fn pack_escrito_pelo_warden_nao_muda_no_refresh() {
    let Some(bin) = packwiz() else { return };
    let temp = tempfile::tempdir().unwrap();
    let pack = temp.path().join("pack");
    new_pack(&pack);
    let modmenu = Metafile::modrinth(
        &ModrinthFile {
            title: "Mod Menu".to_owned(),
            project_id: "mOgUt4GM".to_owned(),
            version_id: "6lgOkclV".to_owned(),
            filename: "modmenu-11.0.5.jar".to_owned(),
            url: "https://cdn.modrinth.com/data/mOgUt4GM/versions/6lgOkclV/modmenu-11.0.5.jar"
                .to_owned(),
            hash_format: HashFormat::Sha512,
            hash: "92e8e96267ff18d320903090f2ad9b32408a218919f5a0d578e232ad774c89d49a34a8271806652be7d6283f24e98432bc4af7f95855e78b843f9100d68696c7".to_owned(),
        },
        Side::Client,
    );
    let modmenu_text = edit::set_metafile_pin(&modmenu.to_toml_string(), true).unwrap();
    write(&pack, "mods/modmenu.pw.toml", &modmenu_text);
    write(&pack, "options.txt", "lang:pt_br\n");
    write(&pack, "config/a.toml", "a = 1\n");

    // O Warden calcula o índice e o hash dele, como o refresh faria.
    let mut index = PackIndex::default();
    for path in ["config/a.toml", "mods/modmenu.pw.toml", "options.txt"] {
        let bytes = fs::read(pack.join(path)).unwrap();
        index.files.push(IndexEntry::new(
            path,
            &hash_bytes(HashFormat::Sha256, &bytes),
        ));
    }
    let index_text = index.to_toml_string();
    let index_text = edit::set_index_preserve(&index_text, "options.txt", true)
        .unwrap()
        .unwrap();
    fs::write(pack.join("index.toml"), &index_text).unwrap();
    let mut manifest = PackManifest::parse(&fs::read_to_string(pack.join("pack.toml")).unwrap())
        .unwrap()
        .value;
    manifest.index.hash = hash_bytes(HashFormat::Sha256, index_text.as_bytes());
    let pack_text = manifest.to_toml_string();
    fs::write(pack.join("pack.toml"), &pack_text).unwrap();

    refresh(&bin, &pack, temp.path());

    assert_eq!(
        fs::read_to_string(pack.join("index.toml")).unwrap(),
        index_text
    );
    assert_eq!(
        fs::read_to_string(pack.join("pack.toml")).unwrap(),
        pack_text
    );
    assert_eq!(
        fs::read_to_string(pack.join("mods/modmenu.pw.toml")).unwrap(),
        modmenu_text
    );
}
