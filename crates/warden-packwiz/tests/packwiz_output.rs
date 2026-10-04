//! Testes dourados contra as fixtures geradas pelo packwiz real (`cargo xtask
//! fixtures-packwiz`; origem em `tests/fixtures/FIXTURES.md`).
//!
//! - CA-1 da P1-01: ler e escrever cada `pack.toml`, `index.toml` e `.pw.toml` dá os mesmos
//!   bytes.
//! - CA-2: o `.pw.toml` que o Warden monta a partir dos dados da API é igual ao do packwiz
//!   (base de CA-T08-01 e CA-T08-03).
//! - Coerência: os hashes do índice conferem com os arquivos, o hash do índice no `pack.toml`
//!   confere, e o índice lista exatamente o que o matcher do `.packwizignore` deixa entrar.

// Nos auxiliares dos testes, `unwrap`, `expect` e `panic!` são a forma idiomática de falhar
// (o `clippy.toml` só os libera dentro das funções `#[test]`).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use warden_packwiz::hash::{hash_bytes, hash_matches};
use warden_packwiz::hygiene::packwizignore_template;
use warden_packwiz::{
    CurseForgeFile, HashFormat, Metafile, ModrinthFile, PackIndex, PackManifest, PackwizIgnore,
    Side, side_from_modrinth, slugify_name,
};

fn output_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/packwiz-output")
}

fn packs() -> Vec<PathBuf> {
    let mut packs: Vec<PathBuf> = fs::read_dir(output_dir())
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.is_dir())
        .collect();
    packs.sort();
    assert!(packs.len() >= 10, "fixtures faltando: {packs:?}");
    packs
}

/// Arquivos de uma pasta, relativos a ela, com `/`.
fn files(root: &Path) -> Vec<String> {
    fn walk(root: &Path, dir: &Path, out: &mut Vec<String>) {
        for entry in fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                walk(root, &path, out);
            } else {
                let relative = path.strip_prefix(root).unwrap();
                out.push(relative.to_string_lossy().replace('\\', "/"));
            }
        }
    }
    let mut out = Vec::new();
    walk(root, root, &mut out);
    out.sort();
    out
}

fn read(path: &Path) -> String {
    let text = fs::read_to_string(path).unwrap();
    assert!(
        !text.contains('\r'),
        "{} tem CR: o git mexeu no fim de linha",
        path.display()
    );
    text
}

#[test]
fn ca_1_ler_e_escrever_da_os_mesmos_bytes() {
    let mut checked = 0;
    for pack in packs() {
        let name = pack.file_name().unwrap().to_string_lossy().into_owned();
        let text = read(&pack.join("pack.toml"));
        let parsed = PackManifest::parse(&text).unwrap();
        assert!(
            parsed.unknown_keys.is_empty(),
            "{name}: {:?}",
            parsed.unknown_keys
        );
        assert!(
            parsed.migrations.is_empty(),
            "{name}: {:?}",
            parsed.migrations
        );
        assert_eq!(parsed.value.to_toml_string(), text, "{name}/pack.toml");

        let text = read(&pack.join("index.toml"));
        let parsed = PackIndex::parse(&text).unwrap();
        assert!(
            parsed.unknown_keys.is_empty(),
            "{name}: {:?}",
            parsed.unknown_keys
        );
        assert_eq!(parsed.value.to_toml_string(), text, "{name}/index.toml");

        for file in files(&pack).into_iter().filter(|f| f.ends_with(".pw.toml")) {
            let text = read(&pack.join(&file));
            let parsed = Metafile::parse(&text).unwrap();
            assert!(parsed.unknown_keys.is_empty(), "{name}/{file}");
            assert_eq!(parsed.value.to_toml_string(), text, "{name}/{file}");
            checked += 1;
        }
    }
    assert!(checked >= 16, "só {checked} metafiles");
}

#[test]
fn hashes_do_indice_e_do_pack_conferem() {
    for pack in packs() {
        let name = pack.file_name().unwrap().to_string_lossy().into_owned();
        let manifest = PackManifest::parse(&read(&pack.join("pack.toml")))
            .unwrap()
            .value;
        let index_bytes = fs::read(pack.join(&manifest.index.file)).unwrap();
        let index = PackIndex::parse(&String::from_utf8(index_bytes.clone()).unwrap())
            .unwrap()
            .value;
        if manifest.index.hash.is_empty() {
            assert_eq!(name, "sem-hashes");
        } else {
            assert_eq!(
                manifest.index.hash,
                hash_bytes(HashFormat::Sha256, &index_bytes),
                "{name}"
            );
        }
        for entry in &index.files {
            let format = if entry.hash_format.is_empty() {
                &index.hash_format
            } else {
                &entry.hash_format
            };
            let format = HashFormat::parse(format).unwrap();
            let bytes = fs::read(pack.join(&entry.file)).unwrap();
            if entry.hash.is_empty() {
                assert_eq!(name, "sem-hashes", "{}", entry.file);
                continue;
            }
            assert!(
                hash_matches(format, &entry.hash, &hash_bytes(format, &bytes)),
                "{name}/{}",
                entry.file
            );
            assert_eq!(entry.metafile, entry.file.ends_with(".pw.toml"));
        }
    }
}

#[test]
fn indice_lista_o_que_o_matcher_deixa_entrar() {
    for pack in packs() {
        let name = pack.file_name().unwrap().to_string_lossy().into_owned();
        let ignore_text = fs::read_to_string(pack.join(".packwizignore")).ok();
        let ignore = PackwizIgnore::for_pack(ignore_text.as_deref());
        let expected: BTreeSet<String> = files(&pack)
            .into_iter()
            .filter(|file| !["pack.toml", "index.toml", ".packwizignore"].contains(&file.as_str()))
            .filter(|file| !ignore.is_excluded(file))
            .collect();
        let index = PackIndex::parse(&read(&pack.join("index.toml")))
            .unwrap()
            .value;
        let indexed: BTreeSet<String> = index.files.into_iter().map(|entry| entry.file).collect();
        assert_eq!(indexed, expected, "{name}");
    }
}

#[test]
fn packwizignore_da_fixture_e_o_modelo_do_warden() {
    let text = read(&output_dir().join("fabric-1.21.1/.packwizignore"));
    assert_eq!(text, packwizignore_template());
}

#[test]
fn campos_especiais_presentes() {
    let fabric = output_dir().join("fabric-1.21.1");
    let sodium = Metafile::parse(&read(&fabric.join("mods/sodium.pw.toml")))
        .unwrap()
        .value;
    assert!(sodium.pin);
    let lithium = Metafile::parse(&read(&fabric.join("mods/lithium.pw.toml")))
        .unwrap()
        .value;
    let option = lithium.option.unwrap();
    assert!(option.optional && option.default);
    assert_eq!(option.description, "Otimizações do servidor \"interno\"");
    let modmenu = Metafile::parse(&read(&fabric.join("mods/modmenu.pw.toml")))
        .unwrap()
        .value;
    assert!(
        modmenu
            .option
            .is_some_and(|o| o.optional && !o.default && o.description.is_empty())
    );
    let jade = Metafile::parse(&read(&fabric.join("mods/jade.pw.toml")))
        .unwrap()
        .value;
    assert!(jade.is_curseforge_mode());
    assert_eq!(jade.curseforge_ids(), Some((324_717, 8_591_528)));
    let index = PackIndex::parse(&read(&fabric.join("index.toml")))
        .unwrap()
        .value;
    assert!(index.entry("options.txt").unwrap().preserve);

    let options = PackManifest::parse(&read(&output_dir().join("opcoes/pack.toml")))
        .unwrap()
        .value;
    assert_eq!(options.curseforge_project_id(), Some(123_456));
    assert_eq!(options.acceptable_game_versions(), ["1.21", "1.21.1"]);
    assert!(options.description.contains('🔍'));

    let old = PackManifest::parse(&read(&output_dir().join("forge-1.7.10/pack.toml")))
        .unwrap()
        .value;
    assert_eq!(old.minecraft_version(), Some("1.7.10"));
    assert_eq!(
        old.loaders(),
        [(warden_packwiz::Loader::Forge, "10.13.4.1614")]
    );
}

fn api_data() -> serde_json::Value {
    serde_json::from_str(&fs::read_to_string(output_dir().join("dados-api.json")).unwrap()).unwrap()
}

fn text(value: &serde_json::Value) -> String {
    value.as_str().unwrap().to_owned()
}

fn folder_for_modrinth(project_type: &str) -> &'static str {
    match project_type {
        "mod" => "mods",
        "resourcepack" => "resourcepacks",
        "shader" => "shaderpacks",
        other => panic!("tipo de projeto inesperado: {other}"),
    }
}

#[test]
fn ca_2_metafile_do_modrinth_igual_ao_do_packwiz() {
    let data = api_data();
    let entries = data["modrinth"].as_array().unwrap();
    assert!(entries.len() >= 10);
    for entry in entries {
        let (side, _) = side_from_modrinth(entry["environment"].as_str());
        let metafile = Metafile::modrinth(
            &ModrinthFile {
                title: text(&entry["title"]),
                project_id: text(&entry["project_id"]),
                version_id: text(&entry["version_id"]),
                filename: text(&entry["filename"]),
                url: text(&entry["url"]),
                hash_format: HashFormat::Sha512,
                hash: text(&entry["sha512"]),
            },
            side,
        );
        let path = format!(
            "{}/{}.pw.toml",
            folder_for_modrinth(entry["project_type"].as_str().unwrap()),
            text(&entry["slug"])
        );
        assert_eq!(path, text(&entry["metafile"]));
        let fixture = output_dir().join(text(&entry["pack"])).join(&path);
        let mut expected = read(&fixture);
        // `pin` e `[option]` foram acrescentados depois, por comandos do packwiz.
        let parsed = Metafile::parse(&expected).unwrap().value;
        if parsed.pin || parsed.option.is_some() {
            let mut base = parsed;
            base.pin = false;
            base.option = None;
            expected = base.to_toml_string();
        }
        assert_eq!(metafile.to_toml_string(), expected, "{path}");
    }
}

#[test]
fn ca_2_metafile_da_curseforge_igual_ao_do_packwiz() {
    let data = api_data();
    let entries = data["curseforge"].as_array().unwrap();
    assert!(entries.len() >= 5);
    for entry in entries {
        let metafile = Metafile::curseforge(
            &CurseForgeFile {
                name: text(&entry["name"]),
                project_id: u32::try_from(entry["project_id"].as_u64().unwrap()).unwrap(),
                file_id: u32::try_from(entry["file_id"].as_u64().unwrap()).unwrap(),
                filename: text(&entry["filename"]),
                hash_format: HashFormat::Sha1,
                hash: text(&entry["sha1"]),
            },
            Side::Both,
        );
        let path = format!("mods/{}.pw.toml", text(&entry["slug"]));
        assert_eq!(path, text(&entry["metafile"]));
        let expected = read(&output_dir().join(text(&entry["pack"])).join(&path));
        assert_eq!(metafile.to_toml_string(), expected, "{path}");
    }
}

#[test]
fn ca_2_metafile_de_link_igual_ao_do_packwiz() {
    let data = api_data();
    let entries = data["url"].as_array().unwrap();
    assert!(!entries.is_empty());
    for entry in entries {
        let name = text(&entry["name"]);
        let path = format!("mods/{}.pw.toml", slugify_name(&name));
        assert_eq!(path, text(&entry["metafile"]));
        let expected = read(&output_dir().join(text(&entry["pack"])).join(&path));
        // O sha256 vem do download (feito pelo packwiz); o resto é montado pelo Warden.
        let hash = Metafile::parse(&expected).unwrap().value.download.hash;
        let metafile = Metafile::url(&name, &text(&entry["url"]), &hash).unwrap();
        assert_eq!(metafile.to_toml_string(), expected, "{path}");
    }
}
