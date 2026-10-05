//! Extração dos pacotes do Temurin (`.zip` e `.tar.gz`), em qualquer sistema, com os pacotes
//! maliciosos que a extração precisa recusar (*zip slip*, caminho absoluto, link para fora).

// Nos auxiliares dos testes, `unwrap`, `expect` e `panic!` são a forma idiomática de falhar
// (o `clippy.toml` só os libera dentro das funções `#[test]`).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod comum;

use std::io::Write as _;

use comum::{jre_tar_gz, jre_zip};
use warden_java::Error;
use warden_java::archive::{ArchiveKind, extract};

fn write(dir: &std::path::Path, name: &str, bytes: &[u8]) -> std::path::PathBuf {
    let path = dir.join(name);
    std::fs::write(&path, bytes).unwrap();
    path
}

#[test]
fn zip_do_temurin_sem_a_pasta_do_topo() {
    let dir = tempfile::tempdir().unwrap();
    let archive = write(
        dir.path(),
        "jre.zip",
        &jre_zip("jdk-21.0.12.1+1-jre", "21.0.12.1+1"),
    );
    let target = dir.path().join("saida");
    std::fs::create_dir(&target).unwrap();
    extract(&archive, ArchiveKind::Zip, &target).unwrap();
    assert!(target.join("bin").join("java.exe").is_file());
    assert!(target.join("release").is_file());
    assert_eq!(
        std::fs::read(target.join("lib").join("modules"))
            .unwrap()
            .len(),
        200_000
    );
    assert!(!target.join("jdk-21.0.12.1+1-jre").exists());
}

#[test]
fn tar_gz_do_temurin_sem_a_pasta_do_topo() {
    let dir = tempfile::tempdir().unwrap();
    let archive = write(
        dir.path(),
        "jre.tar.gz",
        &jre_tar_gz("jdk8u504-b01-jre", "1.8.0_504-b01"),
    );
    let target = dir.path().join("saida");
    std::fs::create_dir(&target).unwrap();
    extract(&archive, ArchiveKind::TarGz, &target).unwrap();
    assert!(target.join("bin").join("java").is_file());
    assert!(
        target
            .join("legal")
            .join("java.base")
            .join("LICENSE")
            .is_file()
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        let mode = std::fs::metadata(target.join("bin").join("java"))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o111, 0o111, "java sem permissão de execução");
    }
}

fn zip_with(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut buffer = std::io::Cursor::new(Vec::new());
    {
        let mut zip = zip::ZipWriter::new(&mut buffer);
        let options = zip::write::SimpleFileOptions::default();
        for (name, bytes) in entries {
            zip.start_file(*name, options).unwrap();
            zip.write_all(bytes).unwrap();
        }
        zip.finish().unwrap();
    }
    buffer.into_inner()
}

fn expect_archive_error(result: warden_java::Result<()>, needle: &str) {
    match result {
        Err(Error::Archive { message, .. }) => assert!(message.contains(needle), "{message}"),
        other => panic!("esperado erro de pacote com {needle:?}, veio {other:?}"),
    }
}

/// Um caso: rótulo, entradas do zip e o trecho esperado na mensagem.
type Case<'a> = (&'a str, &'a [(&'a str, &'a [u8])], &'a str);

#[test]
fn zip_malicioso_e_recusado_sem_gravar_fora() {
    let cases: [Case<'_>; 4] = [
        (
            "..",
            &[("jre/bin/java.exe", b"x"), ("jre/../../fora.txt", b"x")],
            "sai da pasta",
        ),
        ("absoluto", &[("/fora.txt", b"x")], "absoluto"),
        (
            "barra invertida",
            &[("jre\\..\\..\\fora.txt", b"x")],
            "sai da pasta",
        ),
        (
            "nome reservado",
            &[("jre/bin/CON.txt", b"x"), ("jre/bin/java.exe", b"x")],
            "fora da pasta",
        ),
    ];
    for (label, entries, needle) in cases {
        let dir = tempfile::tempdir().unwrap();
        let archive = write(dir.path(), "mau.zip", &zip_with(entries));
        let target = dir.path().join("a").join("saida");
        std::fs::create_dir_all(&target).unwrap();
        expect_archive_error(extract(&archive, ArchiveKind::Zip, &target), needle);
        assert!(!dir.path().join("fora.txt").exists(), "{label}");
        assert!(!dir.path().join("a").join("fora.txt").exists(), "{label}");
    }
}

#[test]
fn zip_corrompido_e_recusado() {
    let dir = tempfile::tempdir().unwrap();
    let archive = write(
        dir.path(),
        "corrompido.zip",
        b"PK\x03\x04 isto nao e um zip",
    );
    let target = dir.path().join("saida");
    std::fs::create_dir(&target).unwrap();
    expect_archive_error(extract(&archive, ArchiveKind::Zip, &target), "zip ilegível");
    let truncated = jre_zip("jre", "21.0.12.1+1");
    let archive = write(dir.path(), "cortado.zip", &truncated[..truncated.len() / 2]);
    assert!(extract(&archive, ArchiveKind::Zip, &target).is_err());
}

fn tar_gz_with(
    build: impl FnOnce(&mut tar::Builder<flate2::write::GzEncoder<Vec<u8>>>),
) -> Vec<u8> {
    let encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
    let mut tar = tar::Builder::new(encoder);
    build(&mut tar);
    tar.into_inner().unwrap().finish().unwrap()
}

#[test]
fn tar_com_link_para_fora_e_recusado() {
    let bytes = tar_gz_with(|tar| {
        let mut header = tar::Header::new_gnu();
        header.set_entry_type(tar::EntryType::Symlink);
        header.set_size(0);
        header.set_mode(0o777);
        tar.append_link(&mut header, "jre/lib/fora", "../../../etc")
            .unwrap();
    });
    let dir = tempfile::tempdir().unwrap();
    let archive = write(dir.path(), "link.tar.gz", &bytes);
    let target = dir.path().join("saida");
    std::fs::create_dir(&target).unwrap();
    expect_archive_error(
        extract(&archive, ArchiveKind::TarGz, &target),
        "sai da pasta",
    );
}

#[test]
fn tar_com_link_para_dentro_e_aceito() {
    let bytes = tar_gz_with(|tar| {
        let mut file = tar::Header::new_gnu();
        file.set_size(3);
        file.set_mode(0o644);
        file.set_cksum();
        tar.append_data(&mut file, "jre/lib/real", &b"abc"[..])
            .unwrap();
        let mut link = tar::Header::new_gnu();
        link.set_entry_type(tar::EntryType::Symlink);
        link.set_size(0);
        link.set_mode(0o777);
        tar.append_link(&mut link, "jre/bin/atalho", "../lib/real")
            .unwrap();
    });
    let dir = tempfile::tempdir().unwrap();
    let archive = write(dir.path(), "link.tar.gz", &bytes);
    let target = dir.path().join("saida");
    std::fs::create_dir(&target).unwrap();
    extract(&archive, ArchiveKind::TarGz, &target).unwrap();
    assert!(target.join("lib").join("real").is_file());
    #[cfg(unix)]
    assert_eq!(
        std::fs::read(target.join("bin").join("atalho")).unwrap(),
        b"abc"
    );
}

#[test]
fn tar_com_link_fisico_vira_copia() {
    let bytes = tar_gz_with(|tar| {
        let mut file = tar::Header::new_gnu();
        file.set_size(3);
        file.set_mode(0o644);
        file.set_cksum();
        tar.append_data(&mut file, "jre/lib/real", &b"abc"[..])
            .unwrap();
        let mut link = tar::Header::new_gnu();
        link.set_entry_type(tar::EntryType::Link);
        link.set_size(0);
        link.set_mode(0o644);
        tar.append_link(&mut link, "jre/lib/copia", "jre/lib/real")
            .unwrap();
    });
    let dir = tempfile::tempdir().unwrap();
    let archive = write(dir.path(), "hard.tar.gz", &bytes);
    let target = dir.path().join("saida");
    std::fs::create_dir(&target).unwrap();
    extract(&archive, ArchiveKind::TarGz, &target).unwrap();
    assert_eq!(
        std::fs::read(target.join("lib").join("copia")).unwrap(),
        b"abc"
    );
}

mod propriedades {
    use proptest::prelude::*;

    use super::*;

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(64))]
        /// Bytes arbitrários (ou um zip real com bytes trocados) nunca entram em pânico nem
        /// gravam fora do destino.
        #[test]
        fn pacotes_arbitrarios_nao_quebram(
            noise in prop::collection::vec(any::<u8>(), 0..512),
            flips in prop::collection::vec((any::<prop::sample::Index>(), any::<u8>()), 0..8),
        ) {
            let dir = tempfile::tempdir().unwrap();
            let target = dir.path().join("saida");
            std::fs::create_dir(&target).unwrap();
            let archive = write(dir.path(), "ruido.zip", &noise);
            let _ = extract(&archive, ArchiveKind::Zip, &target);
            let _ = extract(&archive, ArchiveKind::TarGz, &target);
            let mut zip = jre_zip("jre", "21.0.12.1+1");
            for (index, byte) in flips {
                let at = index.index(zip.len());
                zip[at] = byte;
            }
            let archive = write(dir.path(), "trocado.zip", &zip);
            let _ = extract(&archive, ArchiveKind::Zip, &target);
            let outside: Vec<_> = std::fs::read_dir(dir.path()).unwrap()
                .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
                .collect();
            for name in outside {
                prop_assert!(["saida", "ruido.zip", "trocado.zip"].contains(&name.as_str()), "{}", name);
            }
        }
    }
}
