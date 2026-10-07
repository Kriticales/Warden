//! Aceite de "Mods opcionais e fixar versão" (P1-14; SPEC T11): `[option]` e `pin` gravados
//! no `.pw.toml` com edição mínima, um só `packwiz refresh` e índice sempre conforme. Usa o
//! sidecar real do packwiz quando disponível.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::fs;
use std::path::{Path, PathBuf};

use warden_catalog::Loader;
use warden_core::CancellationToken;
use warden_packwiz::{HashFormat, Metafile, ModrinthFile, Side};
use warden_packwiz_cli::{Packwiz, RunContext};
use warden_project::ProjectErrorCode as Code;
use warden_project::create::{CreatePack, create};
use warden_project::inventory::inventory;
use warden_project::optional::{OptionSettings, option_of, optional_items, set_optional};
use warden_project::pin::{pinned_paths, set_pins};
use warden_project::registry::Registry;

fn binary() -> Option<PathBuf> {
    let name = if cfg!(windows) {
        "packwiz-x86_64-pc-windows-msvc.exe"
    } else {
        "packwiz-x86_64-unknown-linux-gnu"
    };
    let path = std::env::var_os("WARDEN_PACKWIZ_BIN").map_or_else(
        || {
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../apps/desktop/src-tauri/binaries")
                .join(name)
        },
        PathBuf::from,
    );
    if path.is_file() {
        Some(path)
    } else {
        assert_ne!(
            std::env::var_os("WARDEN_REQUIRE_EXTERNALS").as_deref(),
            Some(std::ffi::OsStr::new("1")),
            "sidecar packwiz ausente"
        );
        None
    }
}

fn metafile(n: usize) -> Metafile {
    Metafile::modrinth(
        &ModrinthFile {
            title: format!("Mod {n:02}"),
            project_id: format!("proj{n:04}"),
            version_id: format!("vers{n:04}"),
            filename: format!("mod{n:02}-1.0.{n}.jar"),
            url: format!("https://cdn.modrinth.com/data/proj{n:04}/mod{n:02}.jar"),
            hash_format: HashFormat::Sha512,
            hash: "ab".repeat(64),
        },
        Side::Both,
    )
}

/// Pack criado pelo Warden com `count` mods do Modrinth, já indexados pelo packwiz.
async fn real_pack(root: &Path, binary: PathBuf, count: usize) -> (PathBuf, Packwiz) {
    fs::create_dir_all(root.join("dados")).unwrap();
    let registry = Registry::open(root.join("dados/packs.json")).unwrap();
    let cli = Packwiz::new(
        binary,
        root.join("dados/cache"),
        root.join("dados/config.toml"),
    );
    let made = create(
        &CreatePack {
            name: "Pack Opcionais".into(),
            author: "Autor".into(),
            description: String::new(),
            destination: Some(root.join("pack")),
            minecraft: "1.20.1".into(),
            loader: Some(Loader::Fabric),
            loader_version: Some("0.16.14".into()),
        },
        root,
        &registry,
        &cli,
        &CancellationToken::new(),
    )
    .await
    .unwrap();
    fs::create_dir_all(made.path.join("mods")).unwrap();
    for n in 0..count {
        fs::write(
            made.path.join(format!("mods/mod{n:02}.pw.toml")),
            metafile(n).to_toml_string(),
        )
        .unwrap();
    }
    refresh(&cli, &made.path).await;
    (made.path, cli)
}

async fn refresh(cli: &Packwiz, path: &Path) {
    cli.refresh(path, false, RunContext::new(&CancellationToken::new()))
        .await
        .unwrap();
}

/// Um `refresh` seguinte não muda o manifesto nem o índice: o pack já está conforme.
async fn assert_refresh_is_noop(cli: &Packwiz, path: &Path) {
    let manifest = fs::read(path.join("pack.toml")).unwrap();
    let index = fs::read(path.join("index.toml")).unwrap();
    refresh(cli, path).await;
    assert_eq!(fs::read(path.join("pack.toml")).unwrap(), manifest);
    assert_eq!(fs::read(path.join("index.toml")).unwrap(), index);
}

fn changed_lines(before: &str, after: &str) -> Vec<String> {
    let before: Vec<&str> = before.lines().collect();
    after
        .lines()
        .filter(|line| !before.contains(line))
        .map(str::to_owned)
        .collect()
}

fn settings(description: &str, default: bool) -> OptionSettings {
    OptionSettings {
        description: description.into(),
        default,
    }
}

#[tokio::test]
async fn ca_t11_02_marcar_como_opcional_grava_option_com_descricao_e_padrao() {
    let Some(binary) = binary() else { return };
    let temp = tempfile::tempdir().unwrap();
    let (pack, cli) = real_pack(temp.path(), binary, 3).await;
    let cancel = CancellationToken::new();
    let before = fs::read_to_string(pack.join("mods/mod00.pw.toml")).unwrap();
    let ignore_before = fs::read(pack.join(".packwizignore")).unwrap();

    let written = set_optional(
        &pack,
        "mods/mod00.pw.toml",
        Some(&settings("Sombras bonitas", true)),
        &cli,
        &cancel,
    )
    .await
    .unwrap();
    assert!(written.contains(&"mods/mod00.pw.toml".to_owned()));
    let after = fs::read_to_string(pack.join("mods/mod00.pw.toml")).unwrap();
    // Só a tabela [option] foi acrescentada: nenhuma linha antiga mudou.
    assert_eq!(
        changed_lines(&before, &after),
        ["[option]", "optional = true", "description = \"Sombras bonitas\"", "default = true"]
    );
    for line in before.lines() {
        assert!(after.contains(line), "{line}");
    }
    assert_eq!(
        option_of(&pack, "mods/mod00.pw.toml").unwrap(),
        Some(settings("Sombras bonitas", true))
    );
    assert_eq!(fs::read(pack.join(".packwizignore")).unwrap(), ignore_before);
    assert_refresh_is_noop(&cli, &pack).await;

    // `default = false` não grava a chave `default` (o packwiz a omite).
    set_optional(&pack, "mods/mod01.pw.toml", Some(&settings("", false)), &cli, &cancel)
        .await
        .unwrap();
    let second = fs::read_to_string(pack.join("mods/mod01.pw.toml")).unwrap();
    assert!(second.contains("optional = true"));
    assert!(!second.contains("default"));
    assert!(!second.contains("description"));

    let listed = optional_items(&pack).unwrap();
    assert_eq!(
        listed
            .iter()
            .map(|i| (i.path.as_str(), i.default))
            .collect::<Vec<_>>(),
        [("mods/mod00.pw.toml", true), ("mods/mod01.pw.toml", false)]
    );
    let inv = inventory(&pack, None).await.unwrap();
    assert_eq!(inv.items.iter().filter(|i| i.optional).count(), 2);

    // Pedir o mesmo de novo não grava nada.
    let again = set_optional(
        &pack,
        "mods/mod00.pw.toml",
        Some(&settings("Sombras bonitas", true)),
        &cli,
        &cancel,
    )
    .await
    .unwrap();
    assert!(again.is_empty());

    // Desmarcar remove a tabela inteira e volta ao arquivo original.
    set_optional(&pack, "mods/mod00.pw.toml", None, &cli, &cancel)
        .await
        .unwrap();
    assert_eq!(
        fs::read_to_string(pack.join("mods/mod00.pw.toml")).unwrap(),
        before
    );
    assert_eq!(option_of(&pack, "mods/mod00.pw.toml").unwrap(), None);
    assert_refresh_is_noop(&cli, &pack).await;
}

#[tokio::test]
async fn descricao_invalida_e_caminho_desconhecido_sao_recusados_sem_gravar() {
    let Some(binary) = binary() else { return };
    let temp = tempfile::tempdir().unwrap();
    let (pack, cli) = real_pack(temp.path(), binary, 1).await;
    let cancel = CancellationToken::new();
    let before = fs::read(pack.join("mods/mod00.pw.toml")).unwrap();
    let index = fs::read(pack.join("index.toml")).unwrap();
    let error = set_optional(
        &pack,
        "mods/mod00.pw.toml",
        Some(&settings("a\nb", true)),
        &cli,
        &cancel,
    )
    .await
    .unwrap_err();
    assert_eq!(error.code, Code::InvalidInput);
    assert_eq!(error.params["field"], "description");
    let error = set_optional(
        &pack,
        "mods/nao-existe.pw.toml",
        Some(&settings("", true)),
        &cli,
        &cancel,
    )
    .await
    .unwrap_err();
    assert_eq!(error.code, Code::InvalidInput);
    assert_eq!(fs::read(pack.join("mods/mod00.pw.toml")).unwrap(), before);
    assert_eq!(fs::read(pack.join("index.toml")).unwrap(), index);
}

#[tokio::test]
async fn fixar_versao_grava_so_a_linha_pin_e_soltar_volta_ao_original() {
    let Some(binary) = binary() else { return };
    let temp = tempfile::tempdir().unwrap();
    let (pack, cli) = real_pack(temp.path(), binary, 4).await;
    let cancel = CancellationToken::new();
    let paths: Vec<String> = (0..3).map(|n| format!("mods/mod{n:02}.pw.toml")).collect();
    let before: Vec<String> = paths
        .iter()
        .map(|p| fs::read_to_string(pack.join(p)).unwrap())
        .collect();

    let written = set_pins(&pack, &paths, true, &cli, &cancel).await.unwrap();
    for path in &paths {
        assert!(written.contains(path), "{path}");
    }
    for (path, old) in paths.iter().zip(&before) {
        let new = fs::read_to_string(pack.join(path)).unwrap();
        assert_eq!(changed_lines(old, &new), ["pin = true"], "{path}");
    }
    assert_eq!(pinned_paths(&pack).unwrap(), paths);
    let inv = inventory(&pack, None).await.unwrap();
    assert_eq!(
        inv.items
            .iter()
            .filter(|i| i.pinned)
            .map(|i| i.path.clone())
            .collect::<Vec<_>>(),
        paths
    );
    assert_refresh_is_noop(&cli, &pack).await;

    // Repetir não grava; soltar um só deixa os outros fixados.
    assert!(set_pins(&pack, &paths, true, &cli, &cancel)
        .await
        .unwrap()
        .is_empty());
    set_pins(&pack, &paths[..1], false, &cli, &cancel)
        .await
        .unwrap();
    assert_eq!(
        fs::read_to_string(pack.join(&paths[0])).unwrap(),
        before[0]
    );
    assert_eq!(pinned_paths(&pack).unwrap(), paths[1..]);
    assert_refresh_is_noop(&cli, &pack).await;

    let error = set_pins(&pack, &["mods/x.pw.toml".into()], true, &cli, &cancel)
        .await
        .unwrap_err();
    assert_eq!(error.code, Code::InvalidInput);
    assert_eq!(error.params["path"], "mods/x.pw.toml");
}

#[tokio::test]
async fn opcional_e_fixado_convivem_no_mesmo_metafile() {
    let Some(binary) = binary() else { return };
    let temp = tempfile::tempdir().unwrap();
    let (pack, cli) = real_pack(temp.path(), binary, 1).await;
    let cancel = CancellationToken::new();
    let path = "mods/mod00.pw.toml";
    set_pins(&pack, &[path.into()], true, &cli, &cancel)
        .await
        .unwrap();
    set_optional(&pack, path, Some(&settings("Extra", false)), &cli, &cancel)
        .await
        .unwrap();
    let parsed = warden_packwiz::read_pack(&pack).unwrap();
    let (_, file) = parsed.valid_metafiles().next().unwrap();
    assert!(file.pin);
    assert!(file.option.as_ref().unwrap().optional);
    assert_refresh_is_noop(&cli, &pack).await;
}
