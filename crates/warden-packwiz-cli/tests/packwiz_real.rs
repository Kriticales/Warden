//! Integração com o packwiz real (QUALITY §4.1): `WARDEN_PACKWIZ_BIN` ou o sidecar da F0-03.
//! Sem binário, os testes avisam e passam; com `WARDEN_REQUIRE_EXTERNALS=1` (CI), a falta é
//! falha. Os testes `rede_*` rodam com `cargo xtask test-network` (chave da CurseForge em
//! `CURSEFORGE_API_KEY`).

// Nos auxiliares dos testes, `unwrap`, `expect` e `panic!` são a forma idiomática de falhar.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use common::{CapturedLogs, new_pack, packwiz, real_binary, write};
use secrecy::SecretString;
use warden_core::{CancellationToken, DomainCode, DomainError};
use warden_packwiz::hash::hash_bytes;
use warden_packwiz::{
    HashFormat, IndexEntry, Metafile, ModrinthFile, PackIndex, PackManifest, Side, Value,
};
use warden_packwiz_cli::{
    CurseForgeTarget, Difference, Error, ExportSide, PackwizCliErrorCode, RunContext, Staging,
    check_conformance,
};

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../warden-packwiz/tests/fixtures")
}

fn copy_dir(from: &Path, to: &Path) {
    for file in warden_packwiz_cli::list_files(from).unwrap() {
        let target = to.join(&file.relative);
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        fs::copy(&file.path, target).unwrap();
    }
}

/// Metafile do Modrinth com dados sintéticos (o refresh não baixa nada).
fn modrinth_metafile(n: usize) -> String {
    let hash = hash_bytes(HashFormat::Sha512, format!("mod {n}").as_bytes());
    Metafile::modrinth(
        &ModrinthFile {
            title: format!("Mod Sintético {n:03}"),
            project_id: format!("proj{n:04}"),
            version_id: format!("ver{n:05}"),
            filename: format!("mod-sintetico-{n:03}-1.0.0.jar"),
            url: format!("https://cdn.modrinth.com/data/proj{n:04}/versions/ver{n:05}/mod-sintetico-{n:03}-1.0.0.jar"),
            hash_format: HashFormat::Sha512,
            hash,
        },
        if n.is_multiple_of(3) { Side::Client } else { Side::Both },
    )
    .to_toml_string()
}

fn index_bytes(pack: &Path) -> Vec<u8> {
    fs::read(pack.join("index.toml")).unwrap()
}

#[tokio::test]
async fn ca_1_refresh_de_300_metafiles_confere() {
    let Some(bin) = real_binary() else { return };
    let temp = tempfile::tempdir().unwrap();
    let pack = temp.path().join("pack");
    new_pack(&pack);
    let mut expected = PackIndex::default();
    for n in 0..300 {
        let path = format!("mods/mod-sintetico-{n:03}.pw.toml");
        let text = modrinth_metafile(n);
        write(&pack, &path, &text);
        expected.files.push(IndexEntry::new(
            &path,
            &hash_bytes(HashFormat::Sha256, text.as_bytes()),
        ));
    }
    for (path, text) in [
        ("config/a.toml", "a = 1\n"),
        ("options.txt", "lang:pt_br\n"),
    ] {
        write(&pack, path, text);
        expected.files.push(IndexEntry::new(
            path,
            &hash_bytes(HashFormat::Sha256, text.as_bytes()),
        ));
    }
    // Dados de execução, que o `.packwizignore` padrão exclui.
    write(&pack, "logs/latest.log", "x\n");
    write(&pack, "saves/mundo/level.dat", "x\n");

    let packwiz = packwiz(bin, temp.path());
    let cancel = CancellationToken::new();
    let report = packwiz
        .refresh(&pack, false, RunContext::new(&cancel))
        .await
        .unwrap();
    assert_eq!(report.output.exit_code, Some(0));
    assert_eq!(
        report.index.normalized_entries(),
        expected.normalized_entries()
    );
    assert_eq!(report.index.metafiles().len(), 300);
    // O índice gravado é byte a byte o que o Warden escreveria.
    let text = String::from_utf8(index_bytes(&pack)).unwrap();
    assert_eq!(text, expected.to_toml_string());
    let manifest = PackManifest::parse(&fs::read_to_string(pack.join("pack.toml")).unwrap())
        .unwrap()
        .value;
    assert_eq!(
        manifest.index.hash,
        hash_bytes(HashFormat::Sha256, text.as_bytes())
    );
    // Linhas limpas, sem a barra de progresso.
    assert!(report.output.lines.contains(&"Index refreshed!".to_owned()));
    assert!(
        !report
            .output
            .lines
            .iter()
            .any(|line| line.contains('\u{1b}') || line.contains(" % ["))
    );

    // `list` lê os mesmos 300 metafiles, na ordem do packwiz.
    let names = packwiz.list(&pack, RunContext::new(&cancel)).await.unwrap();
    let mut sorted: Vec<String> = (0..300).map(|n| format!("Mod Sintético {n:03}")).collect();
    sorted.sort_by_key(|name| name.to_lowercase());
    assert_eq!(names, sorted);
}

#[tokio::test]
async fn ignore_ancorado_respeitado_com_pack_file_relativo() {
    // O defeito do packwiz com `--pack-file` absoluto (padrões ancorados ignorados) não
    // acontece pela execução da crate: o índice bate com o do caso relativo da P1-01.
    let Some(bin) = real_binary() else { return };
    let temp = tempfile::tempdir().unwrap();
    let pack = temp.path().join("pack");
    new_pack(&pack);
    let dir = fixtures().join("ignore-100");
    fs::copy(dir.join("packwizignore.txt"), pack.join(".packwizignore")).unwrap();
    for path in fs::read_to_string(dir.join("caminhos.txt"))
        .unwrap()
        .lines()
    {
        write(&pack, path, "x\n");
    }
    let cancel = CancellationToken::new();
    let report = packwiz(bin, temp.path())
        .refresh(&pack, false, RunContext::new(&cancel))
        .await
        .unwrap();
    let indexed: BTreeSet<String> = report
        .index
        .files
        .into_iter()
        .map(|entry| entry.file)
        .collect();
    let expected: BTreeSet<String> =
        fs::read_to_string(dir.join("indexados-pack-file-relativo.txt"))
            .unwrap()
            .lines()
            .map(str::to_owned)
            .collect();
    let absolute: BTreeSet<String> =
        fs::read_to_string(dir.join("indexados-pack-file-absoluto.txt"))
            .unwrap()
            .lines()
            .map(str::to_owned)
            .collect();
    assert_ne!(
        expected, absolute,
        "a fixture mostra o defeito do caminho absoluto"
    );
    assert_eq!(indexed, expected);
}

#[tokio::test]
async fn refresh_build_gera_hashes_com_no_internal_hashes() {
    let Some(bin) = real_binary() else { return };
    let temp = tempfile::tempdir().unwrap();
    let pack = temp.path().join("pack");
    new_pack(&pack);
    let mut manifest = PackManifest::parse(&fs::read_to_string(pack.join("pack.toml")).unwrap())
        .unwrap()
        .value;
    manifest.set_option("no-internal-hashes", Value::Boolean(true));
    fs::write(pack.join("pack.toml"), manifest.to_toml_string()).unwrap();
    write(&pack, "config/a.toml", "a = 1\n");
    let packwiz = packwiz(bin, temp.path());
    let cancel = CancellationToken::new();
    let report = packwiz
        .refresh(&pack, false, RunContext::new(&cancel))
        .await
        .unwrap();
    assert_eq!(report.index.entry("config/a.toml").unwrap().hash, "");
    let report = packwiz
        .refresh(&pack, true, RunContext::new(&cancel))
        .await
        .unwrap();
    assert_eq!(
        report.index.entry("config/a.toml").unwrap().hash,
        hash_bytes(HashFormat::Sha256, b"a = 1\n")
    );
}

/// Pack conforme: escrito pelo Warden e passado pelo refresh.
async fn conforming_pack(bin: PathBuf, temp: &Path) -> PathBuf {
    let pack = temp.join("pack");
    new_pack(&pack);
    for n in 0..5 {
        write(&pack, &format!("mods/m{n}.pw.toml"), &modrinth_metafile(n));
    }
    write(&pack, "config/a.toml", "a = 1\n");
    write(&pack, "config/b.toml", "b = 1\n");
    write(&pack, ".git/HEAD", "ref: refs/heads/main\n");
    let cancel = CancellationToken::new();
    packwiz(bin, temp)
        .refresh(&pack, false, RunContext::new(&cancel))
        .await
        .unwrap();
    pack
}

#[tokio::test]
async fn ca_2_indice_desatualizado_e_reprovado_com_as_diferencas() {
    let Some(bin) = real_binary() else { return };
    let temp = tempfile::tempdir().unwrap();
    let pack = conforming_pack(bin.clone(), temp.path()).await;
    let packwiz = packwiz(bin, temp.path());
    let cancel = CancellationToken::new();
    let staging = temp.path().join("staging");

    let report = check_conformance(&packwiz, &pack, &staging, RunContext::new(&cancel))
        .await
        .unwrap();
    assert!(report.is_conforming(), "{report:?}");
    assert!(!staging.exists(), "a cópia é apagada no fim");

    // O índice fica desatualizado: um arquivo mudou, outro é novo, outro sumiu.
    write(&pack, "config/a.toml", "a = 2\n");
    write(&pack, "config/novo.toml", "c = 1\n");
    fs::remove_file(pack.join("mods/m4.pw.toml")).unwrap();
    let before: Vec<(String, Vec<u8>)> = warden_packwiz_cli::list_files(&pack)
        .unwrap()
        .into_iter()
        .map(|file| (file.relative, fs::read(file.path).unwrap()))
        .collect();

    let report = check_conformance(&packwiz, &pack, &staging, RunContext::new(&cancel))
        .await
        .unwrap();
    assert!(!report.is_conforming());
    assert_eq!(
        report.differences,
        vec![
            Difference::FileChanged {
                path: "index.toml".into()
            },
            Difference::FileChanged {
                path: "pack.toml".into()
            },
            Difference::IndexEntryAdded {
                file: "config/novo.toml".into()
            },
            Difference::IndexEntryRemoved {
                file: "mods/m4.pw.toml".into()
            },
            Difference::IndexEntryChanged {
                file: "config/a.toml".into()
            },
        ]
    );
    // O pack original não foi tocado.
    let after: Vec<(String, Vec<u8>)> = warden_packwiz_cli::list_files(&pack)
        .unwrap()
        .into_iter()
        .map(|file| (file.relative, fs::read(file.path).unwrap()))
        .collect();
    assert_eq!(before, after);
    assert!(!staging.exists());
}

#[tokio::test]
async fn conformidade_de_pack_ilegivel_e_erro() {
    let Some(bin) = real_binary() else { return };
    let temp = tempfile::tempdir().unwrap();
    let pack = temp.path().join("pack");
    write(
        &pack,
        "pack.toml",
        "name = \"x\"\npack-format = \"packwiz:9.9.9\"\n",
    );
    let cancel = CancellationToken::new();
    let error = check_conformance(
        &packwiz(bin, temp.path()),
        &pack,
        &temp.path().join("staging"),
        RunContext::new(&cancel),
    )
    .await
    .unwrap_err();
    let Error::CommandFailed { tail, .. } = &error else {
        panic!("{error}")
    };
    assert!(
        tail.iter()
            .any(|line| line.contains("incompatible with this version of packwiz")),
        "{tail:?}"
    );
}

#[tokio::test]
async fn ca_3_cancelar_refresh_lento_mata_o_packwiz() {
    let Some(bin) = real_binary() else { return };
    let temp = tempfile::tempdir().unwrap();
    let pack = temp.path().join("pack");
    new_pack(&pack);
    // Pack sintético grande: 2 GB num arquivo (criado sem escrever os bytes); o refresh
    // levaria vários segundos só para calcular o hash.
    let big = pack.join("config/grande.bin");
    fs::create_dir_all(big.parent().unwrap()).unwrap();
    fs::File::create(&big)
        .unwrap()
        .set_len(2 * 1024 * 1024 * 1024)
        .unwrap();
    let index_before = index_bytes(&pack);
    let cancel = CancellationToken::new();
    let cancelled_at = Mutex::new(None);
    let lines = Mutex::new(Vec::new());
    let on_line = |line: &str| {
        lines.lock().unwrap().push(line.to_owned());
        if line == "Loading modpack..." {
            *cancelled_at.lock().unwrap() = Some(Instant::now());
            cancel.cancel();
        }
    };
    let error = packwiz(bin, temp.path())
        .refresh(&pack, false, RunContext::new(&cancel).with_lines(&on_line))
        .await
        .unwrap_err();
    let took = cancelled_at.lock().unwrap().unwrap().elapsed();
    assert!(matches!(error, Error::Cancelled { .. }), "{error}");
    assert!(
        took < Duration::from_secs(2),
        "levou {took:?} para matar o packwiz"
    );
    // Evidência para o relatório (`--no-capture` mostra).
    #[allow(clippy::print_stderr)]
    {
        eprintln!("cancelamento até o processo morrer: {took:?}");
    }
    assert!(
        !lines
            .lock()
            .unwrap()
            .contains(&"Index refreshed!".to_owned())
    );
    assert_eq!(index_bytes(&pack), index_before, "o refresh não terminou");
    // No Windows, o packwiz segura o arquivo aberto enquanto calcula o hash: apagar agora
    // prova que o processo já morreu.
    fs::remove_file(&big).unwrap();
}

#[tokio::test]
async fn ca_4_saida_ao_vivo_sem_ansi_nem_barra() {
    let Some(bin) = real_binary() else { return };
    let temp = tempfile::tempdir().unwrap();
    let pack = temp.path().join("pack");
    new_pack(&pack);
    for n in 0..400 {
        write(&pack, &format!("config/f{n}.txt"), &format!("{n}\n"));
    }
    let cancel = CancellationToken::new();
    let report = packwiz(bin, temp.path())
        .refresh(&pack, false, RunContext::new(&cancel))
        .await
        .unwrap();
    let lines = &report.output.lines;
    assert_eq!(lines.len(), 3, "{lines:?}");
    assert!(lines[0].starts_with("Using config file:"));
    assert_eq!(lines[1..], ["Loading modpack...", "Index refreshed!"]);
}

#[tokio::test]
async fn ca_5_curseforge_sem_chave_falha_antes_da_rede() {
    let Some(bin) = real_binary() else { return };
    let temp = tempfile::tempdir().unwrap();
    let pack = temp.path().join("pack");
    copy_dir(&fixtures().join("packwiz-output/forge-1.20.1"), &pack);
    let staging = Staging::copy_from(&pack, &temp.path().join("staging")).unwrap();
    let cancel = CancellationToken::new();
    let error = packwiz(bin, temp.path())
        .curseforge_add(
            staging.path(),
            &CurseForgeTarget::Ids {
                addon_id: 238_222,
                file_id: 9_009_995,
            },
            None,
            RunContext::new(&cancel),
        )
        .await
        .unwrap_err();
    assert_eq!(
        error.code(),
        DomainCode::Domain(PackwizCliErrorCode::CurseForgeKeyMissing),
        "{error}"
    );
}

#[tokio::test]
async fn exportacoes_de_pack_sem_mods_externos() {
    let Some(bin) = real_binary() else { return };
    let temp = tempfile::tempdir().unwrap();
    let pack = conforming_pack(bin.clone(), temp.path()).await;
    // Sem metafiles: nada a baixar, a exportação funciona sem rede.
    for n in 0..5 {
        fs::remove_file(pack.join(format!("mods/m{n}.pw.toml"))).unwrap();
    }
    let staging = Staging::copy_from(&pack, &temp.path().join("staging")).unwrap();
    let packwiz = packwiz(bin, temp.path());
    let cancel = CancellationToken::new();
    let mrpack = temp.path().join("saida/Teste.mrpack");
    fs::create_dir_all(mrpack.parent().unwrap()).unwrap();
    let report = packwiz
        .modrinth_export(staging.path(), &mrpack, None, RunContext::new(&cancel))
        .await
        .unwrap();
    assert_eq!(report.path, mrpack);
    let zip = temp.path().join("saida/Teste.zip");
    let report = packwiz
        .curseforge_export(
            staging.path(),
            ExportSide::Both,
            &zip,
            None,
            RunContext::new(&cancel),
        )
        .await
        .unwrap();
    assert!(report.path.is_file());
    // A exportação reescreveu o índice só na cópia.
    assert!(
        PackIndex::parse(&fs::read_to_string(pack.join("index.toml")).unwrap())
            .unwrap()
            .value
            .entry("mods/m0.pw.toml")
            .is_some()
    );
}

fn curseforge_key() -> SecretString {
    SecretString::from(std::env::var("CURSEFORGE_API_KEY").expect("CURSEFORGE_API_KEY ausente"))
}

#[tokio::test]
#[ignore = "rede"]
async fn rede_curseforge_add_gera_o_mesmo_metafile_do_packwiz() {
    let Some(bin) = real_binary() else { return };
    let temp = tempfile::tempdir().unwrap();
    let fixture = fixtures().join("packwiz-output/forge-1.20.1");
    let pack = temp.path().join("pack");
    copy_dir(&fixture, &pack);
    fs::remove_file(pack.join("mods/jei.pw.toml")).unwrap();
    let packwiz = packwiz(bin, temp.path());
    let cancel = CancellationToken::new();
    packwiz
        .refresh(&pack, false, RunContext::new(&cancel))
        .await
        .unwrap();
    let key = curseforge_key();
    let targets = [
        CurseForgeTarget::Ids {
            addon_id: 238_222,
            file_id: 9_009_995,
        },
        CurseForgeTarget::Url(
            "https://www.curseforge.com/minecraft/mc-mods/jei/files/9009995".into(),
        ),
    ];
    for target in targets {
        let staging = Staging::copy_from(&pack, &temp.path().join("staging")).unwrap();
        let report = packwiz
            .curseforge_add(
                staging.path(),
                &target,
                Some(&key),
                RunContext::new(&cancel),
            )
            .await
            .unwrap();
        assert_eq!(report.metafiles, ["mods/jei.pw.toml"], "{target:?}");
        assert_eq!(
            fs::read(staging.path().join("mods/jei.pw.toml")).unwrap(),
            fs::read(fixture.join("mods/jei.pw.toml")).unwrap()
        );
        // O pack de verdade não foi tocado.
        assert!(!pack.join("mods/jei.pw.toml").exists());
    }
}

#[tokio::test]
#[ignore = "rede"]
async fn rede_ca_5_chave_invalida_vai_so_pelo_ambiente() {
    let Some(bin) = real_binary() else { return };
    let temp = tempfile::tempdir().unwrap();
    let pack = temp.path().join("pack");
    copy_dir(&fixtures().join("packwiz-output/forge-1.20.1"), &pack);
    let staging = Staging::copy_from(&pack, &temp.path().join("staging")).unwrap();
    let fake = "$2a$10$chaveInvalidaDosTestesDaP102abcdefghijklmnopqrstuv";
    let logs = CapturedLogs::default();
    let _guard = logs.install();
    let cancel = CancellationToken::new();
    let error = packwiz(bin, temp.path())
        .curseforge_add(
            staging.path(),
            &CurseForgeTarget::Ids {
                addon_id: 238_222,
                file_id: 9_009_995,
            },
            Some(&SecretString::from(fake.to_owned())),
            RunContext::new(&cancel),
        )
        .await
        .unwrap_err();
    // A chave chegou (não é "ausente") e foi recusada pela CurseForge.
    assert_eq!(
        error.code(),
        DomainCode::Domain(PackwizCliErrorCode::CommandFailed),
        "{error}"
    );
    let detail = error.detail().unwrap();
    assert!(!detail.contains(fake) && !logs.text().contains(fake));
    assert!(logs.text().contains("curseforge add --addon-id 238222"));
}
