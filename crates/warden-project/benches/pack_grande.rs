//! Metas da SPEC §8 que passam pela `warden-project` (A-04), com o pack grande gerado (300 mods
//! e 500 configs):
//!
//! - **Meus packs com 50 packs** (CA-T02-01: a lista em < 1 s): o `Registry::list` que o comando
//!   `packs_list` chama, com 50 packs versionados no git como o Warden os cria: 45 pequenos e 5
//!   grandes, metade com uma versão salva e alterações por salvar, metade só com o ponto inicial
//!   (pack importado ou criado e nunca salvo).
//! - **Abrir um pack de 300 mods (com cache)** (< 1 s até a lista visível): o inventário que a
//!   lista de Mods mostra, com o cache do Modrinth já preenchido (o endereço da API não
//!   responde: nada vem da rede).
//! - **Atualizar índice** (300 mods + 500 configs, < 5 s): `packwiz refresh` pelo sidecar, com
//!   uma config alterada antes de cada vez. Sem o sidecar compilado (`cargo xtask setup`), essa
//!   medida é pulada com aviso.
//!
//! `cargo bench -p warden-project` (o resumo com mediana e pior caso sai no `stderr`).

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

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use criterion::{Criterion, criterion_group, criterion_main};
use warden_core::{CancellationToken, PackId};
use warden_http::{HttpClient, HttpConfig};
use warden_modrinth::{MetadataCache, ModrinthClient, Project, Version};
use warden_packwiz_cli::{Packwiz, RunContext};
use warden_project::registry::{PackRecord, Registry};
use warden_versioning::{Identity, InitialPoint, Moment, PackRepo, SaveVersion};

/// Quantos packs a lista de Meus packs tem (CA-T02-01).
const PACKS: usize = 50;
/// Quantos deles são grandes (300 mods e 500 configs).
const GRANDES: usize = 5;

/// Um pack pequeno: 10 mods e 20 configs.
fn pequeno() -> pack_grande::Opcoes {
    pack_grande::Opcoes {
        mods: 10,
        configs: 20,
        ..pack_grande::Opcoes::default()
    }
}

/// Versiona o pack como o Warden: `.gitignore`, ponto inicial e, se `salvar`, uma config nova
/// salva como versão 1.0.0 seguida de alterações ainda não salvas (uma config e um mod novo).
fn versionar(raiz: &Path, salvar: bool) {
    fs::write(raiz.join(".gitignore"), ".warden/cache/\n").unwrap();
    fs::write(raiz.join(".gitattributes"), "* -text\n").unwrap();
    let identity = Identity::for_pack("Warden", None);
    let repo = PackRepo::init(raiz, InitialPoint::Created, &identity, Moment::now_utc()).unwrap();
    if salvar {
        // Uma versão precisa de alteração depois do ponto inicial, como no app.
        fs::write(raiz.join("config/salva.toml"), "valor = 1\n").unwrap();
        repo.save_version(&SaveVersion {
            version: "1.0.0",
            tag_message: "Primeira versão",
            identity: &identity,
            when: Moment::now_utc(),
            mark_final: false,
        })
        .unwrap();
        fs::write(raiz.join("config/alterada.toml"), "valor = 2\n").unwrap();
        fs::write(
            raiz.join("mods/novo.pw.toml"),
            "name = \"Novo\"\nfilename = \"novo.jar\"\nside = \"both\"\n\n[download]\n\
             url = \"https://example.com/novo.jar\"\nhash-format = \"sha1\"\nhash = \"00\"\n",
        )
        .unwrap();
    }
}

fn registro_com_50_packs(dir: &Path) -> Registry {
    let registry = Registry::open(dir.join("packs.json")).unwrap();
    for n in 0..PACKS {
        let raiz = dir.join(format!("pack-{n:02}"));
        let opcoes = if n < GRANDES {
            pack_grande::Opcoes::default()
        } else {
            pequeno()
        };
        pack_grande::gerar(&raiz, &opcoes);
        versionar(&raiz, n % 2 == 0);
        registry
            .insert(PackRecord {
                id: PackId::new(),
                name: format!("Pack {n}"),
                path: raiz,
                last_test: None,
                extra: std::collections::BTreeMap::default(),
            })
            .unwrap();
    }
    registry
}

fn meus_packs(c: &mut Criterion) {
    let dir = tempfile::tempdir().unwrap();
    let registry = registro_com_50_packs(dir.path());
    let rows = registry.list();
    assert_eq!(rows.len(), PACKS);
    assert!(rows.iter().all(|row| row.minecraft.is_some()));
    assert!(rows.iter().any(|row| row.unsaved_files > 0));

    medida::resumir(
        "Meus packs: lista com 50 packs (5 grandes, git)",
        Duration::from_secs(1),
        || (),
        |()| registry.list(),
    );
    let mut grupo = c.benchmark_group("meus-packs");
    grupo.sample_size(10);
    grupo.bench_function("lista com 50 packs (5 grandes, git)", |b| {
        b.iter(|| registry.list());
    });
    grupo.finish();
}

/// Cliente do Modrinth com o cache preenchido para os mods do pack e um endereço que não
/// responde (porta fechada): tudo o que a lista mostra vem do cache.
fn modrinth_com_cache(
    runtime: &tokio::runtime::Runtime,
    pack: &pack_grande::PackGerado,
) -> ModrinthClient {
    let cache = MetadataCache::open(&pack.raiz.join("..").join("metadata.sqlite")).unwrap();
    let projetos: Vec<(Project, String)> = pack
        .mods
        .iter()
        .map(|m| {
            let json = serde_json::json!({
                "id": m.projeto, "slug": m.id, "project_type": "mod",
                "title": format!("Mod {}", m.id),
                "description": "Um mod de bancada para medir a lista.",
                "status": "approved", "updated": "2026-10-01T00:00:00Z",
                "game_versions": [pack_grande::MINECRAFT], "loaders": ["fabric"],
                "client_side": "required", "server_side": "required",
                "icon_url": format!("https://cdn.modrinth.com/data/{}/icon.png", m.projeto),
            })
            .to_string();
            (serde_json::from_str(&json).unwrap(), json)
        })
        .collect();
    let versoes: Vec<(Version, String)> = pack
        .mods
        .iter()
        .enumerate()
        .map(|(n, m)| {
            let json = serde_json::json!({
                "id": format!("Wv{n:06}"), "project_id": m.projeto,
                "version_number": format!("1.0.{n}"), "version_type": "release",
                "game_versions": [pack_grande::MINECRAFT], "loaders": ["fabric"],
            })
            .to_string();
            (serde_json::from_str(&json).unwrap(), json)
        })
        .collect();
    runtime.block_on(async {
        cache.put_projects(projetos).await.unwrap();
        cache.put_versions(versoes, true).await.unwrap();
    });
    let http = HttpClient::new(HttpConfig::for_version("0.0.0")).unwrap();
    ModrinthClient::with_base_url(http, "http://127.0.0.1:9/v2/", Some(cache)).unwrap()
}

fn abrir_pack(c: &mut Criterion) {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let pack = pack_grande::gerar(&dir.path().join("pack"), &pack_grande::Opcoes::default());
    let client = modrinth_com_cache(&runtime, &pack);
    let inventario = runtime
        .block_on(warden_project::inventory::inventory(
            &pack.raiz,
            Some(&client),
        ))
        .unwrap();
    assert_eq!(inventario.items.len(), pack_grande::MODS);
    assert!(inventario.items.iter().all(|item| item.icon_url.is_some()));
    assert_eq!(client.request_count(), 0, "a lista foi à rede");

    medida::resumir(
        "abrir um pack de 300 mods (com cache): inventário",
        Duration::from_secs(1),
        || (),
        |()| {
            runtime
                .block_on(warden_project::inventory::inventory(
                    &pack.raiz,
                    Some(&client),
                ))
                .unwrap()
        },
    );
    let mut grupo = c.benchmark_group("abrir-pack");
    grupo.sample_size(20);
    grupo.bench_function("inventário de 300 mods (com cache)", |b| {
        b.iter(|| {
            runtime
                .block_on(warden_project::inventory::inventory(
                    &pack.raiz,
                    Some(&client),
                ))
                .unwrap()
        });
    });
    grupo.finish();
}

/// O sidecar do packwiz, como os testes da crate o procuram.
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

fn atualizar_indice(c: &mut Criterion) {
    let Some(binary) = sidecar() else {
        eprintln!(
            "A-04 | atualizar índice | pulado: sidecar do packwiz ausente (cargo xtask setup)"
        );
        return;
    };
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let pack = pack_grande::gerar(&dir.path().join("pack"), &pack_grande::Opcoes::default());
    let packwiz = Packwiz::new(
        binary,
        dir.path().join("packwiz-cache"),
        dir.path().join("packwiz-config.toml"),
    );
    let cancel = CancellationToken::new();
    let config = pack.raiz.join(&pack.configs[0]);
    let original = fs::read(&config).unwrap();
    let mut vez = 0u32;
    let mut alterar = || {
        vez += 1;
        let mut bytes = original.clone();
        bytes.extend_from_slice(format!("\n# A-04 {vez}\n").as_bytes());
        fs::write(&config, bytes).unwrap();
    };
    let refresh = || {
        runtime
            .block_on(packwiz.refresh(&pack.raiz, false, RunContext::new(&cancel)))
            .unwrap()
    };
    alterar();
    let relatorio = refresh();
    assert_eq!(
        relatorio.index.files.len(),
        pack_grande::MODS + pack_grande::CONFIGS
    );

    medida::resumir(
        "atualizar índice: packwiz refresh (300 mods + 500 configs)",
        Duration::from_secs(5),
        &mut alterar,
        |()| refresh(),
    );
    let mut grupo = c.benchmark_group("atualizar-indice");
    grupo.sample_size(10);
    grupo.bench_function("packwiz refresh (300 mods + 500 configs)", |b| {
        b.iter_batched(
            &mut alterar,
            |()| refresh(),
            criterion::BatchSize::PerIteration,
        );
    });
    grupo.finish();
}

criterion_group!(benches, meus_packs, abrir_pack, atualizar_indice);
criterion_main!(benches);
