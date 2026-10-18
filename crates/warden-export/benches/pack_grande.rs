//! Exportação do pack grande (A-04; E-01): a prévia que a seção Exportar mostra ao abrir e a
//! exportação nativa (zip e pasta) do estado atual, com o `packwiz refresh` em staging.
//!
//! A SPEC §8 não dá meta própria para a exportação; a referência é a de "Atualizar índice"
//! (< 5 s), que é o passo mais pesado dela, e a prévia segue a de abrir uma seção (< 1 s).
//! Sem o sidecar do packwiz compilado (`cargo xtask setup`), só a prévia é medida. Com
//! `WARDEN_A04_PACK_REAL`, a prévia e o `.zip` também são medidos com um pack real baixado só
//! para a medida (`comum/pack_real.rs`: os metafiles e as configs dele).
//!
//! `cargo bench -p warden-export` (o resumo com mediana e pior caso sai no `stderr`).

#![allow(
    missing_docs,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stderr
)]

#[path = "../../warden-packwiz/benches/comum/medida.rs"]
mod medida;
#[path = "../../warden-packwiz/benches/comum/pack_grande.rs"]
mod pack_grande;
#[path = "../../warden-packwiz/benches/comum/pack_real.rs"]
mod pack_real;

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use criterion::{BatchSize, Criterion, criterion_group, criterion_main};
use warden_core::CancellationToken;
use warden_export::{ExportFormat, ExportSource, export, preview};
use warden_packwiz_cli::Packwiz;
use warden_versioning::{Identity, InitialPoint, Moment, PackRepo};

/// O sidecar do packwiz, como os testes das crates o procuram.
fn sidecar() -> Option<PathBuf> {
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
    path.is_file().then_some(path)
}

fn exportacao(c: &mut Criterion) {
    let dir = tempfile::tempdir().unwrap();
    let pack = pack_grande::gerar(&dir.path().join("pack"), &pack_grande::Opcoes::default());
    medir(c, &pack, "300 mods + 500 configs", dir.path(), true);
    if let Some(real) = pack_real::PackReal::do_ambiente() {
        let dir = tempfile::tempdir().unwrap();
        let pack = real.gerar(&dir.path().join("pack"));
        let rotulo = format!("{}, {} configs", real.rotulo(), pack.configs.len());
        medir(c, &pack, &rotulo, dir.path(), false);
    }
}

/// Mede a prévia e a exportação de `pack` (zip e, com `pasta`, também a pasta).
fn medir(
    c: &mut Criterion,
    pack: &pack_grande::PackGerado,
    rotulo_pack: &str,
    dir: &Path,
    pasta: bool,
) {
    let minimo = pack.mods.len() + pack.configs.len();
    fs::write(pack.raiz.join(".gitattributes"), "* -text\n").unwrap();
    PackRepo::init(
        &pack.raiz,
        InitialPoint::Created,
        &Identity::for_pack("Warden", None),
        Moment::now_utc(),
    )
    .unwrap();

    let previa = preview(&pack.raiz, &ExportSource::Current).unwrap();
    assert!(previa.files.len() >= minimo);
    medida::resumir(
        &format!("exportar: prévia do pack ({rotulo_pack})"),
        Duration::from_secs(1),
        || (),
        |()| preview(&pack.raiz, &ExportSource::Current).unwrap(),
    );
    let mut grupo = c.benchmark_group(format!("exportar ({rotulo_pack})"));
    grupo.sample_size(10);
    grupo.bench_function("prévia", |b| {
        b.iter(|| preview(&pack.raiz, &ExportSource::Current).unwrap());
    });

    let Some(binary) = sidecar() else {
        eprintln!(
            "A-04 | exportar | zip e pasta pulados: sidecar do packwiz ausente (cargo xtask setup)"
        );
        grupo.finish();
        return;
    };
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let packwiz = Packwiz::new(
        binary,
        dir.join("packwiz-cache"),
        dir.join("packwiz-config.toml"),
    );
    let staging = dir.join("staging");
    fs::create_dir_all(&staging).unwrap();
    let saidas = dir.join("saidas");
    let cancel = CancellationToken::new();
    let mut vez = 0u32;
    let mut destino = |extensao: &str| {
        vez += 1;
        let _ = fs::remove_dir_all(&saidas);
        fs::create_dir_all(&saidas).unwrap();
        saidas.join(format!("pack-{vez}{extensao}"))
    };
    let formatos = [
        (ExportFormat::Zip, ".zip", "zip"),
        (ExportFormat::Folder, "", "pasta"),
    ];
    for (formato, extensao, rotulo) in formatos.into_iter().take(if pasta { 2 } else { 1 }) {
        let exportar = |destino: PathBuf| {
            runtime
                .block_on(export(
                    &pack.raiz,
                    &ExportSource::Current,
                    formato,
                    &destino,
                    &staging,
                    &packwiz,
                    &cancel,
                ))
                .unwrap()
        };
        let resultado = exportar(destino(extensao));
        assert!(resultado.files.len() >= minimo);
        medida::resumir(
            &format!("exportar: {rotulo} ({rotulo_pack}, refresh em staging)"),
            Duration::from_secs(5),
            || destino(extensao),
            exportar,
        );
        grupo.bench_function(rotulo, |b| {
            b.iter_batched(|| destino(extensao), exportar, BatchSize::PerIteration);
        });
    }
    grupo.finish();
}

criterion_group!(benches, exportacao);
criterion_main!(benches);
