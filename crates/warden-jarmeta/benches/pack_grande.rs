//! Índices de pacotes do pack grande (A-04; SPEC §8 "Raio-x de mixins e índices de pacotes
//! (300 mods)": < 3 s sem cache; < 300 ms com cache por hash dos jars).
//!
//! Os 300 jars sintéticos do gerador (descritor, config de mixin, idioma e classes em vários
//! pacotes) de dois tipos: os mínimos (poucos KB) e os calibrados pelos packs reais (~730
//! entradas e ~2,6 MB em média, ~780 MB ao todo). Com `WARDEN_A04_PACK_REAL`, também os jars
//! de um pack real baixado só para a medida (`comum/pack_real.rs`).
//!
//! `cargo bench -p warden-jarmeta` (o resumo com mediana e pior caso sai no `stderr`).

#![allow(missing_docs, clippy::expect_used, clippy::unwrap_used, clippy::panic)]

#[path = "../../warden-packwiz/benches/comum/medida.rs"]
mod medida;
#[path = "../../warden-packwiz/benches/comum/pack_grande.rs"]
mod pack_grande;
#[path = "../../warden-packwiz/benches/comum/pack_real.rs"]
mod pack_real;

use std::fs;
use std::path::Path;
use std::time::Duration;

use criterion::{BatchSize, Criterion, criterion_group, criterion_main};
use warden_jarmeta::index::{JarIndexCache, PackJar, build_pack_index};
use warden_jarmeta::{Limits, Loader};

fn jars_do_pack(dir: &Path, tamanho_real: bool) -> Vec<PackJar> {
    let pack = pack_grande::gerar(
        &dir.join("pack"),
        &pack_grande::Opcoes {
            jars: Some(dir.join("instancia").join("mods")),
            tamanho_real,
            configs: 0,
            ..pack_grande::Opcoes::default()
        },
    );
    pack.mods
        .iter()
        .map(|m| PackJar::new(m.metafile.clone(), m.jar.clone().unwrap()))
        .collect()
}

fn medir(c: &mut Criterion, tamanho_real: bool) {
    let rotulo = if tamanho_real {
        "300 jars calibrados"
    } else {
        "300 jars mínimos"
    };
    let dir = tempfile::tempdir().unwrap();
    let jars = jars_do_pack(dir.path(), tamanho_real);
    medir_jars(c, rotulo, &jars, Loader::Fabric, dir.path());
    let mixin = format!("{}.mixins.json", pack_grande::mod_id(7));
    let cache = JarIndexCache::in_cache_dir(&dir.path().join("cache"));
    let build = build_pack_index(
        &jars,
        Some(Loader::Fabric),
        Some(&cache),
        &Limits::default(),
    );
    assert_eq!(
        build.index.mixin_owner(&mixin).unwrap().owner.item,
        format!("mods/{}.pw.toml", pack_grande::mod_id(7))
    );
}

fn medir_jars(c: &mut Criterion, rotulo: &str, jars: &[PackJar], loader: Loader, dir: &Path) {
    let limits = Limits::default();
    let cache_dir = dir.join("cache");
    let sem_cache = || {
        let _ = fs::remove_dir_all(&cache_dir);
        JarIndexCache::in_cache_dir(&cache_dir)
    };

    let primeira = build_pack_index(jars, Some(loader), Some(&sem_cache()), &limits);
    assert!(primeira.failures.is_empty(), "{:?}", primeira.failures);
    assert_eq!(primeira.stats.jars_opened, jars.len());
    let cache = JarIndexCache::in_cache_dir(&cache_dir);
    let segunda = build_pack_index(jars, Some(loader), Some(&cache), &limits);
    assert_eq!(segunda.stats.jars_opened, 0);
    assert_eq!(segunda.index, primeira.index);

    medida::resumir(
        &format!("índice de pacotes, {rotulo}, sem cache"),
        Duration::from_secs(3),
        sem_cache,
        |cache| build_pack_index(jars, Some(loader), Some(&cache), &limits),
    );
    medida::resumir(
        &format!("índice de pacotes, {rotulo}, com cache"),
        Duration::from_millis(300),
        || (),
        |()| build_pack_index(jars, Some(loader), Some(&cache), &limits),
    );
    let mut grupo = c.benchmark_group(format!("indice-de-pacotes ({rotulo})"));
    grupo.sample_size(10);
    grupo.bench_function("sem cache", |b| {
        b.iter_batched(
            sem_cache,
            |cache| build_pack_index(jars, Some(loader), Some(&cache), &limits),
            BatchSize::PerIteration,
        );
    });
    // O cache da medida anterior foi apagado: monta de novo antes de medir com ele.
    let _ = build_pack_index(jars, Some(loader), Some(&cache), &limits);
    grupo.bench_function("com cache", |b| {
        b.iter(|| build_pack_index(jars, Some(loader), Some(&cache), &limits));
    });
    grupo.finish();
}

fn indice(c: &mut Criterion) {
    medir(c, false);
    medir(c, true);
    if let Some(real) = pack_real::PackReal::do_ambiente() {
        let dir = tempfile::tempdir().unwrap();
        let pack = real.gerar(&dir.path().join("pack"));
        let jars: Vec<PackJar> = pack
            .mods
            .iter()
            .map(|m| PackJar::new(m.metafile.clone(), m.jar.clone().unwrap()))
            .collect();
        let loader = match real.loader.0.as_str() {
            "quilt" => Loader::Quilt,
            "forge" => Loader::Forge,
            "neoforge" => Loader::NeoForge,
            _ => Loader::Fabric,
        };
        medir_jars(c, &real.rotulo(), &jars, loader, dir.path());
    }
}

criterion_group!(benches, indice);
criterion_main!(benches);
