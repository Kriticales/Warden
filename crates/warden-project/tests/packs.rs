//! Aceite do serviço de packs com o sidecar real, quando disponível.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::fs;
use std::path::{Path, PathBuf};

use warden_catalog::Loader;
use warden_core::CancellationToken;
use warden_packwiz_cli::{Packwiz, RunContext};
use warden_project::ProjectErrorCode as Code;
use warden_project::create::{CreatePack, create, validate};
use warden_project::hygiene;
use warden_project::open::{import, preview};
use warden_project::registry::{PackRecord, PackStatus, Registry};

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

fn request(
    root: &Path,
    minecraft: &str,
    loader: Option<Loader>,
    version: Option<&str>,
) -> CreatePack {
    CreatePack {
        name: "Pack Teste".into(),
        author: "Autor".into(),
        description: "Descrição".into(),
        destination: Some(root.join("pack")),
        minecraft: minecraft.into(),
        loader,
        loader_version: version.map(str::to_owned),
    }
}

fn services(root: &Path, binary: PathBuf) -> (Registry, Packwiz) {
    fs::create_dir_all(root.join("dados")).unwrap();
    (
        Registry::open(root.join("dados/packs.json")).unwrap(),
        Packwiz::new(
            binary,
            root.join("dados/cache"),
            root.join("dados/config.toml"),
        ),
    )
}

#[test]
fn destino_nao_vazio_e_recusado_antes_de_escrever() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("pack");
    fs::create_dir(&path).unwrap();
    fs::write(path.join("original"), b"intacto").unwrap();
    let error = validate(&request(temp.path(), "1.20.1", None, None), temp.path()).unwrap_err();
    assert_eq!(error.code, Code::DestinationNotEmpty);
    assert_eq!(fs::read(path.join("original")).unwrap(), b"intacto");
    assert_eq!(fs::read_dir(&path).unwrap().count(), 1);
}

#[tokio::test]
async fn cria_variantes_e_refresh_nao_muda_bytes() {
    let Some(binary) = binary() else { return };
    for (mc, loader, version) in [
        ("1.7.10", Some(Loader::Forge), Some("10.13.4.1614")),
        ("1.12.2", Some(Loader::Forge), Some("14.23.5.2860")),
        ("1.20.1", Some(Loader::Forge), Some("47.4.0")),
        ("1.20.1", Some(Loader::NeoForge), Some("47.1.106")),
        ("1.21.1", Some(Loader::NeoForge), Some("21.1.200")),
        ("1.20.1", Some(Loader::Fabric), Some("0.16.14")),
        ("1.21.11", Some(Loader::Fabric), Some("0.18.4")),
        ("1.20.1", None, None),
    ] {
        let temp = tempfile::tempdir().unwrap();
        let (registry, cli) = services(temp.path(), binary.clone());
        let created = create(
            &request(temp.path(), mc, loader, version),
            temp.path(),
            &registry,
            &cli,
            &CancellationToken::new(),
        )
        .await
        .unwrap();
        let manifest = fs::read(created.path.join("pack.toml")).unwrap();
        let index = fs::read(created.path.join("index.toml")).unwrap();
        cli.refresh(
            &created.path,
            false,
            RunContext::new(&CancellationToken::new()),
        )
        .await
        .unwrap();
        assert_eq!(
            fs::read(created.path.join("pack.toml")).unwrap(),
            manifest,
            "{mc} {loader:?}"
        );
        assert_eq!(
            fs::read(created.path.join("index.toml")).unwrap(),
            index,
            "{mc} {loader:?}"
        );
        let mut names: Vec<_> = fs::read_dir(&created.path)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        assert_eq!(
            names,
            [
                ".git",
                ".gitattributes",
                ".gitignore",
                ".packwizignore",
                ".warden",
                "CHANGELOG.md",
                "index.toml",
                "pack.toml"
            ]
        );
        let pack = warden_packwiz::read_pack(&created.path).unwrap();
        assert_eq!(pack.pack.value.minecraft_version(), Some(mc));
        assert_eq!(pack.pack.value.loaders().first().map(|(_, v)| *v), version);
        assert_eq!(registry.list().len(), 1);
    }
}

#[tokio::test]
async fn importa_copia_com_outro_id_e_preserva_tabela_futura() {
    let Some(binary) = binary() else { return };
    let temp = tempfile::tempdir().unwrap();
    let (registry, cli) = services(temp.path(), binary);
    let made = create(
        &request(temp.path(), "1.20.1", None, None),
        temp.path(),
        &registry,
        &cli,
        &CancellationToken::new(),
    )
    .await
    .unwrap();
    let copy = temp.path().join("copy");
    copy_dir(&made.path, &copy);
    let original = fs::read_to_string(copy.join(".warden/project.toml")).unwrap();
    fs::write(
        copy.join(".warden/project.toml"),
        format!("{original}\n[future]\nflag = true\n"),
    )
    .unwrap();
    let result = import(&copy, false, &registry).await.unwrap();
    assert!(result.copied_id_replaced);
    assert_ne!(result.id, made.id);
    assert!(
        fs::read_to_string(copy.join(".warden/project.toml"))
            .unwrap()
            .contains("[future]\nflag = true")
    );
    assert_eq!(registry.list().len(), 2);
    assert!(preview(&copy).unwrap().read_only_reason.is_none());
}

#[tokio::test]
async fn head_destacado_abre_para_leitura_sem_alterar_arquivos() {
    let Some(binary) = binary() else { return };
    let temp = tempfile::tempdir().unwrap();
    let (registry, cli) = services(temp.path(), binary);
    let made = create(
        &request(temp.path(), "1.20.1", None, None),
        temp.path(),
        &registry,
        &cli,
        &CancellationToken::new(),
    )
    .await
    .unwrap();
    let copy = temp.path().join("detached");
    copy_dir(&made.path, &copy);
    let repo = git2::Repository::open(&copy).unwrap();
    let commit = repo.head().unwrap().target().unwrap();
    repo.set_head_detached(commit).unwrap();
    let before = fs::read(copy.join(".warden/project.toml")).unwrap();
    let result = import(&copy, true, &registry).await.unwrap();
    assert!(result.read_only_reason.is_some());
    assert!(result.copied_id_replaced);
    assert_eq!(fs::read(copy.join(".warden/project.toml")).unwrap(), before);
    assert_eq!(registry.list().len(), 2);
    assert!(
        registry
            .list()
            .iter()
            .any(|row| row.id == result.id && row.read_only_reason.is_some())
    );
}

fn copy_dir(from: &Path, to: &Path) {
    fs::create_dir(to).unwrap();
    for item in fs::read_dir(from).unwrap() {
        let item = item.unwrap();
        let target = to.join(item.file_name());
        if item.file_type().unwrap().is_dir() {
            copy_dir(&item.path(), &target);
        } else {
            fs::copy(item.path(), target).unwrap();
        }
    }
}

#[tokio::test]
async fn agora_nao_preserva_manifesto_e_indice_e_cria_historico() {
    let Some(binary) = binary() else { return };
    let temp = tempfile::tempdir().unwrap();
    let (registry, cli) = services(temp.path(), binary);
    let external = temp.path().join("external");
    fs::create_dir(&external).unwrap();
    let mut manifest = warden_packwiz::PackManifest::new("Externo", "1.20.1");
    manifest.version = "2.0.0".into();
    fs::write(external.join("pack.toml"), manifest.to_toml_string()).unwrap();
    fs::write(
        external.join("index.toml"),
        warden_packwiz::PackIndex::default().to_toml_string(),
    )
    .unwrap();
    cli.refresh(&external, false, RunContext::new(&CancellationToken::new()))
        .await
        .unwrap();
    let before_pack = fs::read(external.join("pack.toml")).unwrap();
    let before_index = fs::read(external.join("index.toml")).unwrap();
    let imported = import(&external, false, &registry).await.unwrap();
    assert_eq!(fs::read(external.join("pack.toml")).unwrap(), before_pack);
    assert_eq!(fs::read(external.join("index.toml")).unwrap(), before_index);
    assert!(external.join(".git").is_dir());
    assert!(external.join(".warden/project.toml").is_file());
    assert!(
        warden_versioning::PackRepo::open(&external)
            .unwrap()
            .head_id()
            .unwrap()
            .is_some()
    );
    assert_eq!(imported.required_ignore_added.len(), 4);
    assert_eq!(
        fs::read_dir(&external)
            .unwrap()
            .filter_map(Result::ok)
            .count(),
        5
    );
}

#[test]
fn quilt_e_recusado_sem_tocar_na_pasta() {
    let temp = tempfile::tempdir().unwrap();
    let mut pack = warden_packwiz::PackManifest::new("Quilt", "1.20.1");
    pack.versions
        .as_mut()
        .unwrap()
        .insert("quilt".into(), "1.0.0".into());
    fs::write(temp.path().join("pack.toml"), pack.to_toml_string()).unwrap();
    fs::write(
        temp.path().join("index.toml"),
        warden_packwiz::PackIndex::default().to_toml_string(),
    )
    .unwrap();
    let before = fs::read_dir(temp.path()).unwrap().count();
    assert_eq!(
        preview(temp.path()).unwrap_err().code,
        Code::UnsupportedLoader
    );
    assert_eq!(fs::read_dir(temp.path()).unwrap().count(), before);
    assert!(!temp.path().join(".warden").exists());
}

#[tokio::test]
async fn higiene_apaga_itens_indexados_e_refresh_nao_os_reintroduz() {
    let Some(binary) = binary() else { return };
    let temp = tempfile::tempdir().unwrap();
    let (registry, cli) = services(temp.path(), binary);
    let made = create(
        &request(temp.path(), "1.20.1", None, None),
        temp.path(),
        &registry,
        &cli,
        &CancellationToken::new(),
    )
    .await
    .unwrap();
    fs::write(
        made.path.join(".packwizignore"),
        "/.warden/\n/CHANGELOG.md\n/README.md\n*.warden-tmp\n",
    )
    .unwrap();
    fs::create_dir(made.path.join("config")).unwrap();
    for path in [
        "config/x.toml.bak",
        "packwiz-installer-bootstrap.jar",
        ".packwiz.toml",
    ] {
        fs::write(made.path.join(path), b"lixo").unwrap();
    }
    cli.refresh(
        &made.path,
        false,
        RunContext::new(&CancellationToken::new()),
    )
    .await
    .unwrap();
    let found = hygiene::scan(&made.path).unwrap();
    for path in [
        "config/x.toml.bak",
        "packwiz-installer-bootstrap.jar",
        ".packwiz.toml",
    ] {
        assert!(
            found.iter().any(|item| item.path == path && item.in_index),
            "{path}"
        );
    }
    let selected = [
        "config/x.toml.bak",
        "packwiz-installer-bootstrap.jar",
        ".packwiz.toml",
    ]
    .map(str::to_owned);
    let deleted = hygiene::fix(
        &made.path,
        &selected,
        "Autor",
        &cli,
        &CancellationToken::new(),
    )
    .await
    .unwrap();
    assert_eq!(deleted.len(), 3);
    cli.refresh(
        &made.path,
        false,
        RunContext::new(&CancellationToken::new()),
    )
    .await
    .unwrap();
    let index = warden_packwiz::read_pack(&made.path)
        .unwrap()
        .index
        .unwrap()
        .value;
    for path in selected {
        assert!(!made.path.join(&path).exists());
        assert!(!index.files.iter().any(|item| item.file == path));
    }
    assert!(
        !warden_versioning::PackRepo::open(&made.path)
            .unwrap()
            .safety_points()
            .unwrap()
            .is_empty()
    );
}

#[test]
fn cinquenta_packs_listam_rapido_e_relocate_preserva_registro() {
    let temp = tempfile::tempdir().unwrap();
    let registry = Registry::open(temp.path().join("packs.json")).unwrap();
    let mut first = None;
    for n in 0..50 {
        let path = temp.path().join(format!("pack{n}"));
        fs::create_dir(&path).unwrap();
        fs::write(
            path.join("pack.toml"),
            warden_packwiz::PackManifest::new(&format!("Pack {n}"), "1.20.1").to_toml_string(),
        )
        .unwrap();
        fs::write(
            path.join("index.toml"),
            warden_packwiz::PackIndex::default().to_toml_string(),
        )
        .unwrap();
        let id = warden_core::PackId::new();
        if first.is_none() {
            first = Some((id, path.clone()));
        }
        registry
            .insert(PackRecord {
                id,
                name: format!("Pack {n}"),
                path,
                last_test: None,
                extra: std::collections::BTreeMap::default(),
            })
            .unwrap();
    }
    let start = std::time::Instant::now();
    let list = registry.list();
    assert_eq!(list.len(), 50);
    assert!(list.iter().all(|row| row.modified_at_ms.is_some()));
    assert!(
        start.elapsed() < std::time::Duration::from_secs(1),
        "50 packs em {:?}",
        start.elapsed()
    );
    let (id, old) = first.unwrap();
    let new = temp.path().join("movido");
    fs::rename(&old, &new).unwrap();
    assert!(matches!(
        registry
            .list()
            .into_iter()
            .find(|row| row.id == id)
            .unwrap()
            .status,
        PackStatus::FolderMissing
    ));
    registry.relocate(id, new.clone()).unwrap();
    assert!(matches!(
        registry
            .list()
            .into_iter()
            .find(|row| row.id == id)
            .unwrap()
            .status,
        PackStatus::Ready
    ));
    let before = fs::read(new.join("pack.toml")).unwrap();
    registry.forget(id).unwrap();
    assert_eq!(fs::read(new.join("pack.toml")).unwrap(), before);
}
