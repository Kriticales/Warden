//! Aceite do editor de configs (C-02): CA-T12-01, CA-T12-03 e CA-T12-05, com o sidecar real do
//! packwiz quando disponível (o pack gravado precisa ficar com o índice válido).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::fs;
use std::path::{Path, PathBuf};

use warden_catalog::Loader;
use warden_core::CancellationToken;
use warden_packwiz::PackIndex;
use warden_packwiz_cli::{Packwiz, RunContext};
use warden_project::ProjectErrorCode as Code;
use warden_project::configs::{
    ConfigOrigin, ReadOnlyReason, hash_bytes, list_files, read_file, save_to_instance, save_to_pack,
};
use warden_project::create::{CreatePack, create};
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

fn write(root: &Path, path: &str, bytes: &[u8]) {
    let full = root.join(path);
    fs::create_dir_all(full.parent().unwrap()).unwrap();
    fs::write(full, bytes).unwrap();
}

/// Pack criado pelo Warden, com uma config TOML do Forge e um `options.txt`.
async fn real_pack(root: &Path, binary: PathBuf) -> (PathBuf, Packwiz) {
    fs::create_dir_all(root.join("dados")).unwrap();
    let registry = Registry::open(root.join("dados/packs.json")).unwrap();
    let cli = Packwiz::new(
        binary,
        root.join("dados/cache"),
        root.join("dados/config.toml"),
    );
    let made = create(
        &CreatePack {
            name: "Pack de Configs".into(),
            author: "Autor".into(),
            description: String::new(),
            destination: Some(root.join("pack")),
            minecraft: "1.20.1".into(),
            loader: Some(Loader::Forge),
            loader_version: Some("47.3.0".into()),
        },
        root,
        &registry,
        &cli,
        &CancellationToken::new(),
    )
    .await
    .unwrap();
    write(
        &made.path,
        "config/create-common.toml",
        "#Configurações gerais\r\n[kinetics]\r\n\t#Range: > 64\r\n\tmaxRotationSpeed = 256\r\n\tstress = 1.0\r\n"
            .as_bytes(),
    );
    write(&made.path, "options.txt", b"renderDistance:12\nfov:0.5\n");
    cli.refresh(
        &made.path,
        false,
        RunContext::new(&CancellationToken::new()),
    )
    .await
    .unwrap();
    (made.path, cli)
}

fn index_of(path: &Path) -> PackIndex {
    warden_packwiz::read_pack(path)
        .unwrap()
        .index
        .unwrap()
        .value
}

fn leftovers(root: &Path) -> Vec<String> {
    let mut found = Vec::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(dir) = pending.pop() {
        for entry in fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            if path.file_name().is_some_and(|name| name == ".git") {
                continue;
            }
            if path.is_dir() {
                pending.push(path);
            } else {
                let extension = path
                    .extension()
                    .map(|ext| ext.to_string_lossy().to_lowercase());
                if matches!(extension.as_deref(), Some("bak" | "warden-tmp" | "tmp")) {
                    found.push(path.display().to_string());
                }
            }
        }
    }
    found
}

/// CA-T12-01 no nível do serviço: abrir e salvar sem mudanças, com o corpus real de configs
/// da C-01, não altera um byte.
#[test]
fn ca_t12_01_abrir_e_salvar_sem_mudancas_devolve_os_mesmos_bytes_do_corpus() {
    let corpus = Path::new(env!("CARGO_MANIFEST_DIR")).join("../warden-configs/tests/corpus");
    let temp = tempfile::tempdir().unwrap();
    let mut checked = 0;
    let mut read_only = 0;
    let mut pending = vec![corpus];
    while let Some(dir) = pending.pop() {
        for entry in fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                pending.push(path);
                continue;
            }
            let name = path.file_name().unwrap().to_string_lossy().into_owned();
            if name == "FIXTURES.md" || name == ".gitattributes" {
                continue;
            }
            let original = fs::read(&path).unwrap();
            let relative = format!("config/{name}");
            write(temp.path(), &relative, &original);
            let content = read_file(temp.path(), ConfigOrigin::Instance, &relative).unwrap();
            assert_eq!(content.hash, hash_bytes(&original), "{name}");
            if let Some(text) = content.text.as_deref()
                && content.read_only.is_none()
            {
                let saved = save_to_instance(temp.path(), &relative, text, &content.hash).unwrap();
                assert!(!saved.changed, "{name} mudou sem edição");
                checked += 1;
            } else {
                assert!(matches!(
                    content.read_only,
                    Some(
                        ReadOnlyReason::NotUtf8 | ReadOnlyReason::TooLarge | ReadOnlyReason::Binary
                    )
                ));
                read_only += 1;
            }
            assert_eq!(
                fs::read(temp.path().join(&relative)).unwrap(),
                original,
                "{name}"
            );
        }
    }
    assert!(
        checked >= 100,
        "só {checked} arquivos do corpus foram conferidos ({read_only} só leitura)"
    );
}

#[tokio::test]
async fn gravar_no_pack_muda_so_o_arquivo_e_atualiza_o_indice() {
    let Some(binary) = binary() else { return };
    let temp = tempfile::tempdir().unwrap();
    let (pack, cli) = real_pack(temp.path(), binary).await;
    let cancel = CancellationToken::new();

    let list = list_files(&pack, ConfigOrigin::Pack).unwrap();
    let paths: Vec<_> = list.files.iter().map(|file| file.path.as_str()).collect();
    assert_eq!(paths, ["config/create-common.toml", "options.txt"]);

    let read = read_file(&pack, ConfigOrigin::Pack, "config/create-common.toml").unwrap();
    let text = read.text.clone().unwrap();
    assert!(text.contains("\r\n"));
    let edited = text.replace("maxRotationSpeed = 256", "maxRotationSpeed = 512");
    let saved = save_to_pack(
        &pack,
        "config/create-common.toml",
        &edited,
        &read.hash,
        &cli,
        &cancel,
    )
    .await
    .unwrap();
    assert!(saved.changed);

    let after = fs::read(pack.join("config/create-common.toml")).unwrap();
    assert_eq!(hash_bytes(&after), saved.hash);
    let before_lines: Vec<_> = text.split("\r\n").collect();
    let after_text = String::from_utf8(after).unwrap();
    let after_lines: Vec<_> = after_text.split("\r\n").collect();
    assert_eq!(before_lines.len(), after_lines.len());
    let differing: Vec<_> = before_lines
        .iter()
        .zip(&after_lines)
        .filter(|(a, b)| a != b)
        .collect();
    assert_eq!(
        differing,
        [(&"\tmaxRotationSpeed = 256", &"\tmaxRotationSpeed = 512")]
    );

    // O índice ganhou o arquivo com o hash novo e o packwiz não acha mais nada a atualizar.
    let index = index_of(&pack);
    assert!(
        index
            .files
            .iter()
            .any(|entry| entry.file == "config/create-common.toml")
    );
    let index_bytes = fs::read(pack.join("index.toml")).unwrap();
    cli.refresh(&pack, false, RunContext::new(&cancel))
        .await
        .unwrap();
    assert_eq!(fs::read(pack.join("index.toml")).unwrap(), index_bytes);
    assert_eq!(leftovers(&pack), Vec::<String>::new());
}

#[tokio::test]
async fn ca_t12_03_alterado_por_fora_nao_grava_nada() {
    let Some(binary) = binary() else { return };
    let temp = tempfile::tempdir().unwrap();
    let (pack, cli) = real_pack(temp.path(), binary).await;
    let cancel = CancellationToken::new();
    let read = read_file(&pack, ConfigOrigin::Pack, "options.txt").unwrap();

    // Outro programa edita o arquivo depois que o Warden o abriu.
    write(&pack, "options.txt", b"renderDistance:32\n");
    let index_before = fs::read(pack.join("index.toml")).unwrap();

    let error = save_to_pack(
        &pack,
        "options.txt",
        "renderDistance:8\n",
        &read.hash,
        &cli,
        &cancel,
    )
    .await
    .unwrap_err();
    assert_eq!(error.code, Code::FileChangedOnDisk);
    assert_eq!(
        fs::read(pack.join("options.txt")).unwrap(),
        b"renderDistance:32\n"
    );
    assert_eq!(fs::read(pack.join("index.toml")).unwrap(), index_before);

    // "Sobrescrever": a pessoa relê o arquivo, escolhe e grava com o hash novo.
    let fresh = read_file(&pack, ConfigOrigin::Pack, "options.txt").unwrap();
    let saved = save_to_pack(
        &pack,
        "options.txt",
        "renderDistance:8\n",
        &fresh.hash,
        &cli,
        &cancel,
    )
    .await
    .unwrap();
    assert!(saved.changed);
    assert_eq!(
        fs::read(pack.join("options.txt")).unwrap(),
        b"renderDistance:8\n"
    );
}

#[tokio::test]
async fn ca_t12_05_falha_ou_encerramento_na_gravacao_nao_deixa_bak_nem_temporario() {
    let Some(binary) = binary() else { return };
    let temp = tempfile::tempdir().unwrap();
    let (pack, cli) = real_pack(temp.path(), binary).await;
    let cancel = CancellationToken::new();
    let original = fs::read(pack.join("options.txt")).unwrap();
    let index_before = fs::read(pack.join("index.toml")).unwrap();
    let read = read_file(&pack, ConfigOrigin::Pack, "options.txt").unwrap();

    // Falha antes de gravar e entre gravar o temporário e renomear: o original continua e nada
    // sobra.
    for point in ["atomic_write.before_write", "atomic_write.before_rename"] {
        let _armed = warden_core::fault::arm(point);
        let error = save_to_pack(
            &pack,
            "options.txt",
            "renderDistance:8\n",
            &read.hash,
            &cli,
            &cancel,
        )
        .await
        .unwrap_err();
        assert_eq!(error.code, Code::Internal, "{point}");
        assert_eq!(
            fs::read(pack.join("options.txt")).unwrap(),
            original,
            "{point}"
        );
        assert_eq!(
            fs::read(pack.join("index.toml")).unwrap(),
            index_before,
            "{point}"
        );
        assert_eq!(leftovers(&pack), Vec::<String>::new(), "{point}");
    }

    // O app encerrado à força deixa o temporário para trás; a próxima gravação o apaga.
    write(&pack, "options.txt.9999-0.warden-tmp", b"renderDis");
    save_to_pack(
        &pack,
        "options.txt",
        "renderDistance:8\n",
        &read.hash,
        &cli,
        &cancel,
    )
    .await
    .unwrap();
    assert_eq!(leftovers(&pack), Vec::<String>::new());
    assert_eq!(
        fs::read(pack.join("options.txt")).unwrap(),
        b"renderDistance:8\n"
    );
}
