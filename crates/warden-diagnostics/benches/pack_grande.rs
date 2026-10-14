//! Diagnóstico pré-teste com o pack grande (A-04; SPEC §8 "Diagnóstico pré-teste (300 mods,
//! cache quente)": < 3 s).
//!
//! - **Passagem rápida** (D-01): as regras com os dados do pack e das APIs (relações,
//!   versões do Minecraft e loaders de cada versão), sem jars.
//! - **Passagem completa** (D-01 + D-05): ler os metadados dos 300 jars (em paralelo, como o
//!   app), rodar as regras com eles e montar o índice de pacotes com o cache quente.
//!
//! Os jars são os sintéticos do gerador, com o tamanho dos de verdade (~1 MB em média). Um
//! terço dos mods depende do anterior, então as regras de dependência têm o que conferir.
//!
//! `cargo bench -p warden-diagnostics` (o resumo com mediana e pior caso sai no `stderr`).

#![allow(missing_docs, clippy::expect_used, clippy::unwrap_used, clippy::panic)]

#[path = "../../warden-packwiz/benches/comum/medida.rs"]
mod medida;
#[path = "../../warden-packwiz/benches/comum/pack_grande.rs"]
mod pack_grande;

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

use criterion::{Criterion, criterion_group, criterion_main};
use warden_diagnostics::Source;
use warden_diagnostics::pretest::{
    ApiMetadata, ApiRelation, ApiRelationKind, Input, PackContext, PackItem, Pass, ReleaseChannel,
    analyze, build_index,
};
use warden_jarmeta::{JarMetadata, Limits, Loader};
use warden_packwiz::Side;

fn contexto() -> PackContext {
    PackContext {
        minecraft: pack_grande::MINECRAFT.into(),
        minecraft_release_target: None,
        loader: Loader::Fabric,
        loader_version: pack_grande::FABRIC.into(),
        language_loader_versions: HashMap::new(),
        java_major: 17,
        minimum_java_major: Some(17),
        resource_pack_format: None,
        acceptable_game_versions: Vec::new(),
    }
}

/// Os metadados que o Modrinth daria para o mod `n` (a versão do pack gerado).
fn api(n: usize, m: &pack_grande::ModGerado) -> ApiMetadata {
    let relations = if n % 3 == 1 {
        vec![ApiRelation {
            project_id: format!("Wb{:06}", n - 1),
            kind: ApiRelationKind::Required,
            field: "dependencies[0]".into(),
        }]
    } else {
        Vec::new()
    };
    ApiMetadata {
        source: Source::Modrinth,
        url: format!("https://api.modrinth.com/v2/version/Wv{n:06}"),
        project_id: m.projeto.clone(),
        project_slug: Some(m.id.replace('_', "-")),
        game_versions: vec![pack_grande::MINECRAFT.into()],
        loaders: vec![Loader::Fabric],
        release: ReleaseChannel::Release,
        relations,
        archived: false,
        download_blocked: false,
        client_side: Some(true),
        server_side: Some(true),
    }
}

fn itens(
    pack: &pack_grande::PackGerado,
    jars: Option<&HashMap<String, JarMetadata>>,
) -> Vec<PackItem> {
    pack.mods
        .iter()
        .enumerate()
        .map(|(n, m)| PackItem {
            path: m.metafile.clone(),
            filename: m.arquivo.clone(),
            side: Side::Both,
            file_hash: None,
            api: Some(api(n, m)),
            jar: jars.and_then(|jars| jars.get(&m.metafile).cloned()),
            accepted_minecraft_versions: Vec::new(),
            resource_pack_format: None,
        })
        .collect()
}

/// Lê os metadados dos jars em paralelo, como o app (`graph::read_jars`).
fn ler_jars(caminhos: &[(String, PathBuf)]) -> HashMap<String, JarMetadata> {
    let threads = std::thread::available_parallelism()
        .map_or(2, std::num::NonZero::get)
        .min(8);
    let chunk = caminhos.len().div_ceil(threads).max(1);
    let limits = Limits::default();
    std::thread::scope(|scope| {
        let handles: Vec<_> = caminhos
            .chunks(chunk)
            .map(|parte| {
                let limits = &limits;
                scope.spawn(move || {
                    parte
                        .iter()
                        .map(|(item, path)| {
                            (
                                item.clone(),
                                warden_jarmeta::read_jar_file(path, limits).unwrap(),
                            )
                        })
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        handles
            .into_iter()
            .flat_map(|handle| handle.join().unwrap())
            .collect()
    })
}

fn pre_teste(c: &mut Criterion) {
    let dir = tempfile::tempdir().unwrap();
    let instancia = dir.path().join("instancia");
    let pack = pack_grande::gerar(
        &dir.path().join("pack"),
        &pack_grande::Opcoes {
            jars: Some(instancia.join("mods")),
            tamanho_real: true,
            configs: 0,
            ..pack_grande::Opcoes::default()
        },
    );
    let caminhos: Vec<(String, PathBuf)> = pack
        .mods
        .iter()
        .map(|m| (m.metafile.clone(), m.jar.clone().unwrap()))
        .collect();
    let cache = dir.path().join("cache");

    let rapida = Input {
        pack: contexto(),
        items: itens(&pack, None),
        pass: Pass::Quick,
    };
    assert!(
        analyze(&rapida).unwrap().is_empty(),
        "{:?}",
        analyze(&rapida)
    );
    medida::resumir(
        "pré-teste: passagem rápida (300 mods)",
        Duration::from_secs(3),
        || (),
        |()| analyze(&rapida).unwrap(),
    );

    let completa = |()| {
        let jars = ler_jars(&caminhos);
        let input = Input {
            pack: contexto(),
            items: itens(&pack, Some(&jars)),
            pass: Pass::Complete,
        };
        let achados = analyze(&input).unwrap();
        let indice = build_index(&input, &instancia, &cache);
        (achados, indice)
    };
    let (achados, indice) = completa(());
    assert!(achados.is_empty(), "{achados:?}");
    assert!(indice.skipped.is_empty());
    assert_eq!(indice.stats.jars, pack_grande::MODS);
    medida::resumir(
        "pré-teste: passagem completa com jars e índice de pacotes (cache quente)",
        Duration::from_secs(3),
        || (),
        completa,
    );

    let mut grupo = c.benchmark_group("pre-teste");
    grupo.sample_size(10);
    grupo.bench_function("passagem rápida (300 mods)", |b| {
        b.iter(|| analyze(&rapida).unwrap());
    });
    grupo.bench_function("passagem completa (300 jars, cache quente)", |b| {
        b.iter(|| completa(()));
    });
    grupo.finish();
}

criterion_group!(benches, pre_teste);
criterion_main!(benches);
