//! Busca em todas as configs com o pack grande (A-04; SPEC §8 e CA-T12-06): índice das 500
//! configs pronto em < 2 s e cada busca em < 200 ms.
//!
//! As configs são o corpus de configs reais da crate (`tests/corpus/`) repetido em lotes, no
//! pack gerado de 300 mods. O índice lê a pasta do pack com a mesma lista de arquivos do teste
//! da C-05 (a da `warden-project` acrescenta as regras do pack, sem custo que importe aqui).
//!
//! `cargo bench -p warden-configs` (o resumo com mediana e pior caso sai no `stderr`).

#![allow(missing_docs, clippy::expect_used, clippy::unwrap_used, clippy::panic)]

#[path = "../../warden-packwiz/benches/comum/medida.rs"]
mod medida;
#[path = "../../warden-packwiz/benches/comum/pack_grande.rs"]
mod pack_grande;

use std::fs;
use std::io::Read as _;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use criterion::{Criterion, criterion_group, criterion_main};
use warden_configs::index::{LiveIndex, LiveIndexOptions, SearchQuery, SourceFile};
use warden_core::CancellationToken;

/// Buscas medidas: palavras comuns nas configs, duas palavras e uma letra só (a pior: casa
/// com quase tudo).
const BUSCAS: [&str; 5] = ["spawn", "enable", "render distance", "max", "a"];

/// A lista da origem: todo arquivo fora de pastas ocultas, com tamanho, data e se é binário.
fn listar(root: &Path) -> std::io::Result<Option<Vec<SourceFile>>> {
    if !root.is_dir() {
        return Ok(None);
    }
    let mut files = Vec::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(dir) = pending.pop() {
        for entry in fs::read_dir(&dir)? {
            let entry = entry?;
            if entry.file_name().to_string_lossy().starts_with('.') {
                continue;
            }
            let kind = entry.file_type()?;
            if kind.is_dir() {
                pending.push(entry.path());
            } else if kind.is_file() {
                let metadata = entry.metadata()?;
                let mut head = [0u8; 8192];
                let read = fs::File::open(entry.path())?.read(&mut head)?;
                let relative = entry.path().strip_prefix(root).unwrap().to_path_buf();
                #[allow(clippy::cast_precision_loss)]
                files.push(SourceFile {
                    path: relative.to_string_lossy().replace('\\', "/"),
                    size: metadata.len(),
                    modified_ms: metadata
                        .modified()
                        .ok()
                        .and_then(|at| at.duration_since(std::time::UNIX_EPOCH).ok())
                        .map(|elapsed| elapsed.as_millis() as f64),
                    binary: head[..read].contains(&0),
                });
            }
        }
    }
    Ok(Some(files))
}

fn iniciar(root: &Path) -> LiveIndex {
    LiveIndex::start(LiveIndexOptions {
        root: root.to_path_buf(),
        lister: Arc::new(listar),
        relevant: Arc::new(|parts: &[&str]| !parts.iter().any(|part| part.starts_with('.'))),
        on_change: Arc::new(|_| {}),
        debounce: Duration::from_millis(150),
    })
}

fn indice(c: &mut Criterion) {
    let dir = tempfile::tempdir().unwrap();
    let pack = pack_grande::gerar(dir.path(), &pack_grande::Opcoes::default());

    medida::resumir(
        "busca em todas as configs: índice de 500 configs pronto",
        Duration::from_secs(2),
        || (),
        |()| {
            let index = iniciar(&pack.raiz);
            assert!(index.wait_ready(Duration::from_secs(60)));
            index
        },
    );
    let mut grupo = c.benchmark_group("configs");
    grupo.sample_size(10);
    grupo.bench_function("índice de 500 configs pronto", |b| {
        b.iter(|| {
            let index = iniciar(&pack.raiz);
            assert!(index.wait_ready(Duration::from_secs(60)));
            index
        });
    });

    let index = iniciar(&pack.raiz);
    assert!(index.wait_ready(Duration::from_secs(60)));
    let status = index.status();
    // As 500 configs e os 300 metafiles, `pack.toml` e `index.toml`.
    assert!(status.total_files as usize >= pack_grande::CONFIGS);
    for busca in BUSCAS {
        let query = SearchQuery::new(busca);
        let resultado = index.search(&query, &CancellationToken::new()).unwrap();
        assert!(resultado.total_hits > 0, "{busca}: nada");
        medida::resumir(
            &format!("busca em todas as configs: \"{busca}\""),
            Duration::from_millis(200),
            || (),
            |()| index.search(&query, &CancellationToken::new()).unwrap(),
        );
        grupo.bench_function(format!("busca \"{busca}\""), |b| {
            b.iter(|| index.search(&query, &CancellationToken::new()).unwrap());
        });
    }
    grupo.finish();
}

criterion_group!(benches, indice);
criterion_main!(benches);
