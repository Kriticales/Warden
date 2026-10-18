//! Cache do índice de pacotes que reconhece o jar já visto sem ler o arquivo inteiro (A-04:
//! com os jars de um pack real, calcular o hash de cada jar a cada passagem passava da meta de
//! < 300 ms com cache).

// Auxiliares de teste fora de funções #[test]: falhar com mensagem é o comportamento certo.
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

mod common;

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use common::{JarBuilder, fabric_mod_json};
use warden_jarmeta::index::{
    FILES_DIR_NAME, JarIndexCache, PackJar, build_pack_index, file_identity, sha256_hex,
};
use warden_jarmeta::{Limits, Loader};

/// Jar do mod `id` com a classe `Main` no pacote `package`.
fn jar(id: &str, package: &str) -> Vec<u8> {
    JarBuilder::new()
        .file("fabric.mod.json", fabric_mod_json(id, "1.0", &[]))
        .class(&format!("{package}/Main.class"), 61)
        .build()
}

fn write_jar(dir: &Path, id: &str, package: &str) -> PathBuf {
    let path = dir.join(format!("{id}.jar"));
    fs::write(&path, jar(id, package)).unwrap();
    path
}

fn set_modified(path: &Path, when: SystemTime) {
    fs::File::options()
        .write(true)
        .open(path)
        .unwrap()
        .set_modified(when)
        .unwrap();
}

#[test]
fn segunda_passagem_nao_le_os_jars() {
    let dir = tempfile::tempdir().unwrap();
    let jars: Vec<PackJar> = (0..20)
        .map(|n| {
            let id = format!("mod{n:02}");
            PackJar::new(
                format!("mods/{id}.pw.toml"),
                write_jar(dir.path(), &id, &id),
            )
        })
        .collect();
    let cache = JarIndexCache::in_cache_dir(&dir.path().join("cache"));
    let limits = Limits::default();

    let first = build_pack_index(&jars, Some(Loader::Fabric), Some(&cache), &limits);
    assert!(first.failures.is_empty(), "{:?}", first.failures);
    assert_eq!(first.stats.jars_read, 20);
    assert_eq!(first.stats.jars_opened, 20);
    assert_eq!(first.stats.cache_write_failures, 0);
    // Os itens saem na ordem pedida, mesmo com os leitores pegando os jars de uma fila.
    let items: Vec<&str> = first
        .index
        .items()
        .iter()
        .map(|(i, _)| i.as_str())
        .collect();
    let asked: Vec<&str> = jars.iter().map(|j| j.item.as_str()).collect();
    assert_eq!(items, asked);

    let second = build_pack_index(&jars, Some(Loader::Fabric), Some(&cache), &limits);
    assert_eq!(
        second.stats.jars_read, 0,
        "a segunda passagem leu jars inteiros"
    );
    assert_eq!(second.stats.jars_opened, 0);
    assert_eq!(second.stats.cache_hits, 20);
    assert_eq!(second.index, first.index);
}

#[test]
fn jar_mudado_e_lido_de_novo() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_jar(dir.path(), "a", "velho");
    let jars = [PackJar::new("mods/a.pw.toml", &path)];
    let cache = JarIndexCache::in_cache_dir(&dir.path().join("cache"));
    let limits = Limits::default();
    let first = build_pack_index(&jars, None, Some(&cache), &limits);
    assert!(first.index.class_owner("velho.Main").is_some());

    // Conteúdo novo com o mesmo tamanho e a mesma data: o diretório central (nome e CRC-32 das
    // entradas) muda a identidade, e o índice sai do jar novo.
    let when = fs::metadata(&path).unwrap().modified().unwrap();
    let before = fs::read(&path).unwrap();
    fs::write(&path, jar("a", "novos")).unwrap();
    set_modified(&path, when);
    assert_eq!(fs::read(&path).unwrap().len(), before.len());
    let changed = build_pack_index(&jars, None, Some(&cache), &limits);
    assert_eq!(changed.stats.jars_read, 1);
    assert_eq!(changed.stats.jars_opened, 1);
    assert!(changed.index.class_owner("novos.Main").is_some());
    assert!(changed.index.class_owner("velho.Main").is_none());

    // Os mesmos bytes com outra data (cópia nova do mesmo arquivo): lê para conferir o hash,
    // usa o índice guardado e guarda a identidade nova.
    set_modified(&path, when + Duration::from_secs(60));
    let touched = build_pack_index(&jars, None, Some(&cache), &limits);
    assert_eq!(touched.stats.jars_read, 1);
    assert_eq!(touched.stats.jars_opened, 0);
    assert_eq!(touched.stats.cache_hits, 1);
    assert_eq!(touched.index, changed.index);
    let again = build_pack_index(&jars, None, Some(&cache), &limits);
    assert_eq!(again.stats.jars_read, 0);
    assert_eq!(again.index, changed.index);
}

#[test]
fn identidade_guardada_que_nao_serve_e_ignorada() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_jar(dir.path(), "a", "a");
    let jars = [PackJar::new("mods/a.pw.toml", &path)];
    let cache = JarIndexCache::in_cache_dir(&dir.path().join("cache"));
    let limits = Limits::default();
    let first = build_pack_index(&jars, None, Some(&cache), &limits);
    let identity = file_identity(&path).unwrap();
    let sha = sha256_hex(&fs::read(&path).unwrap());
    assert_eq!(cache.known_sha256(&identity), Some(sha.clone()));

    // Texto que não é um hash, ou hash de um índice que não existe: lê o jar e regrava.
    let stored = cache.dir().join(FILES_DIR_NAME).join(&identity);
    for bad in ["../../x".to_owned(), "b".repeat(64)] {
        fs::write(&stored, &bad).unwrap();
        let build = build_pack_index(&jars, None, Some(&cache), &limits);
        assert_eq!(build.stats.jars_read, 1, "{bad}");
        assert_eq!(build.index, first.index);
        assert_eq!(cache.known_sha256(&identity), Some(sha.clone()));
    }
    // Identidade e hash inválidos nunca viram caminho.
    assert_eq!(cache.known_sha256("../../a"), None);
    assert!(cache.remember_sha256("../../a", &sha).is_err());
    assert!(cache.remember_sha256(&identity, "../x").is_err());
}

#[test]
fn identidade_do_arquivo() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_jar(dir.path(), "a", "a");
    let identity = file_identity(&path).unwrap();
    assert_eq!(identity.len(), 64);
    assert_eq!(file_identity(&path), Some(identity.clone()));

    // Outra data, outra identidade.
    let when = fs::metadata(&path).unwrap().modified().unwrap();
    set_modified(&path, when + Duration::from_secs(1));
    assert_ne!(file_identity(&path), Some(identity));

    // Zip com comentário no fim e com dados antes (o diretório é achado pelo registro de fim).
    let mut with_comment = jar("b", "b");
    let len = with_comment.len();
    with_comment[len - 2..].copy_from_slice(&5u16.to_le_bytes());
    with_comment.extend_from_slice(b"ola!!");
    let commented = dir.path().join("b.jar");
    fs::write(&commented, &with_comment).unwrap();
    assert!(file_identity(&commented).is_some());
    let mut prefixed = b"#!/bin/sh\n".to_vec();
    prefixed.extend_from_slice(&jar("c", "c"));
    let prefixed_path = dir.path().join("c.jar");
    fs::write(&prefixed_path, &prefixed).unwrap();
    assert!(file_identity(&prefixed_path).is_some());

    // O que não termina num zip, arquivo curto e arquivo ausente: sem identidade (o jar é lido).
    for (name, bytes) in [
        (
            "texto.jar",
            b"isto nao e um zip, mas tem mais de 22 bytes".as_slice(),
        ),
        ("curto.jar", b"PK".as_slice()),
    ] {
        let p = dir.path().join(name);
        fs::write(&p, bytes).unwrap();
        assert_eq!(file_identity(&p), None, "{name}");
    }
    assert_eq!(file_identity(&dir.path().join("sumiu.jar")), None);
    let mut truncated = jar("d", "d");
    truncated.truncate(truncated.len() - 1);
    let truncated_path = dir.path().join("d.jar");
    fs::write(&truncated_path, &truncated).unwrap();
    assert_eq!(file_identity(&truncated_path), None);

    // zip64 com os campos de 32 bits preenchidos: os registros zip64 ficam entre o diretório e o
    // registro de fim, então o diretório não termina onde o registro de fim começa (sem
    // identidade, o jar é lido).
    let plain = jar("e", "e");
    let eocd = plain.len() - 22;
    let mut zip64 = plain[..eocd].to_vec();
    let mut record = b"PK\x06\x06".to_vec();
    record.extend_from_slice(&44u64.to_le_bytes());
    record.resize(56, 0);
    zip64.extend_from_slice(&record);
    let mut locator = b"PK\x06\x07".to_vec();
    locator.resize(20, 0);
    zip64.extend_from_slice(&locator);
    zip64.extend_from_slice(&plain[eocd..]);
    let zip64_path = dir.path().join("e.jar");
    fs::write(&zip64_path, &zip64).unwrap();
    assert_eq!(file_identity(&zip64_path), None);
}

#[test]
fn sem_cache_le_o_jar_toda_vez() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_jar(dir.path(), "a", "a");
    let jars = [PackJar::new("mods/a.pw.toml", &path)];
    let limits = Limits::default();
    let first = build_pack_index(&jars, None, None, &limits);
    let second = build_pack_index(&jars, None, None, &limits);
    assert_eq!((first.stats.jars_read, second.stats.jars_read), (1, 1));
    assert_eq!((first.stats.cache_hits, second.stats.cache_hits), (0, 0));
    assert_eq!(second.index, first.index);
    assert_eq!(
        second.index.items()[0].1.sha256,
        sha256_hex(&fs::read(&path).unwrap())
    );
}
