//! Leitura do pack grande (A-04; SPEC §8 "Abrir um pack de 300 mods"): `pack.toml`,
//! `index.toml` e os 300 metafiles, a base de toda tela do pack.
//!
//! `cargo bench -p warden-packwiz` (o resumo com mediana e pior caso sai no `stderr`).

#![allow(missing_docs, clippy::expect_used, clippy::unwrap_used, clippy::panic)]

#[path = "comum/medida.rs"]
mod medida;
#[path = "comum/pack_grande.rs"]
mod pack_grande;

use std::time::Duration;

use criterion::{Criterion, criterion_group, criterion_main};
use warden_packwiz::read_pack;

fn leitura(c: &mut Criterion) {
    let dir = tempfile::tempdir().unwrap();
    let pack = pack_grande::gerar(dir.path(), &pack_grande::Opcoes::default());
    let lido = read_pack(&pack.raiz).unwrap();
    assert_eq!(lido.metafiles.len(), pack_grande::MODS);
    assert!(lido.metafiles.iter().all(|m| m.result.is_ok()));
    assert_eq!(
        lido.index.as_ref().unwrap().value.files.len(),
        pack_grande::MODS + pack_grande::CONFIGS
    );

    medida::resumir(
        "ler pack.toml, index.toml e 300 metafiles",
        Duration::from_secs(1),
        || (),
        |()| read_pack(&pack.raiz).unwrap(),
    );
    let mut grupo = c.benchmark_group("packwiz");
    grupo.sample_size(20);
    grupo.bench_function("ler o pack (300 mods, 500 configs)", |b| {
        b.iter(|| read_pack(&pack.raiz).unwrap());
    });
    grupo.finish();
}

criterion_group!(benches, leitura);
criterion_main!(benches);
