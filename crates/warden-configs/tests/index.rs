//! Busca em todas as configs (C-05) com o corpus de configs reais (`tests/corpus/`).
//!
//! - CA-T12-06: 500 configs reais (o corpus repetido em lotes, cada arquivo com o nome
//!   original), índice lido em segundo plano; "spawn" acha "Spawn" e chaves como `spawnRate`
//!   sem diferenciar maiúsculas, agrupa por arquivo, responde em menos de 200 ms com o índice
//!   pronto, e a linha de cada resultado é a da chave no arquivo.
//! - CA-T12-07: editar, criar e apagar um arquivo por fora atualiza a busca em até 2 s.
//!
//! Os tempos medidos saem no `stderr` (`cargo nextest run -p warden-configs --no-capture`).

// Os tempos medidos vão para o relatório; nos utilitários de teste, falhar com pânico é o
// jeito de reprovar.
#![allow(clippy::print_stderr, clippy::unwrap_used, clippy::panic)]

mod common;

use std::fs;
use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use common::{corpus_root, folder};
use warden_configs::ConfigFormat;
use warden_configs::index::{
    ConfigSearchResults, HitField, IndexChange, LiveIndex, LiveIndexOptions, SearchQuery,
    SourceFile,
};
use warden_core::CancellationToken;

/// Quantos arquivos o pack de teste tem (CA-T12-06).
const PACK_FILES: usize = 500;

/// Quanto a busca pode demorar com o índice pronto (CA-T12-06). O critério vale para o app de
/// verdade (compilado com otimização: ~10 ms neste corpus); sem otimização (`cargo test`
/// comum, CI) o mesmo código leva de 60 a 100 ms, e o limite triplica para a medida não
/// depender da máquina. `cargo nextest run --release -p warden-configs` confere os 200 ms.
const SEARCH_LIMIT: Duration = if cfg!(debug_assertions) {
    Duration::from_millis(600)
} else {
    Duration::from_millis(200)
};

/// Quanto a busca pode demorar para ver uma edição feita por fora (CA-T12-07).
const UPDATE_LIMIT: Duration = Duration::from_secs(2);

/// Os arquivos do corpus, lidos uma vez.
fn corpus() -> Vec<(ConfigFormat, String, Vec<u8>)> {
    let mut files = Vec::new();
    for format in ConfigFormat::ALL {
        let dir = corpus_root().join(folder(format));
        let mut names: Vec<PathBuf> = fs::read_dir(&dir)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect();
        names.sort();
        for path in names {
            let name = path.file_name().unwrap().to_string_lossy().into_owned();
            files.push((format, name, fs::read(&path).unwrap()));
        }
    }
    files
}

/// Um pack com `count` configs reais: o corpus repetido em `config/lote<n>/`. Os `options.txt`
/// ficam numa pasta própria, com o nome que o formato exige.
fn write_pack(root: &Path, count: usize) -> Vec<String> {
    let corpus = corpus();
    let mut written = Vec::new();
    for (position, (format, name, bytes)) in corpus.iter().cycle().take(count).enumerate() {
        let batch = position / corpus.len();
        let relative = if *format == ConfigFormat::OptionsTxt {
            let stem = name.trim_end_matches(".txt");
            format!("config/lote{batch}/{stem}/options.txt")
        } else {
            format!("config/lote{batch}/{name}")
        };
        let path = root.join(&relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, bytes).unwrap();
        written.push(relative);
    }
    written
}

/// A lista da origem, como o `list_files` da `warden-project` faz (sem as regras do pack).
fn list(root: &Path) -> std::io::Result<Option<Vec<SourceFile>>> {
    if !root.is_dir() {
        return Ok(None);
    }
    let mut files = Vec::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(dir) = pending.pop() {
        for entry in fs::read_dir(&dir)? {
            let entry = entry?;
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

struct Live {
    index: LiveIndex,
    changes: Arc<Mutex<Vec<IndexChange>>>,
}

fn start(root: &Path) -> Live {
    let changes = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&changes);
    let index = LiveIndex::start(LiveIndexOptions {
        root: root.to_path_buf(),
        lister: Arc::new(list),
        relevant: Arc::new(|parts: &[&str]| !parts.iter().any(|part| part.starts_with('.'))),
        on_change: Arc::new(move |change| sink.lock().unwrap().push(change)),
        debounce: Duration::from_millis(150),
    });
    Live { index, changes }
}

fn search(live: &Live, text: &str) -> ConfigSearchResults {
    live.index
        .search(&SearchQuery::new(text), &CancellationToken::new())
        .unwrap()
}

/// Repete a busca até `check` aceitar o resultado ou `limit` passar. Devolve o tempo.
fn wait_for(
    live: &Live,
    text: &str,
    limit: Duration,
    check: impl Fn(&ConfigSearchResults) -> bool,
) -> Duration {
    let started = Instant::now();
    loop {
        if check(&search(live, text)) {
            return started.elapsed();
        }
        assert!(
            started.elapsed() < limit,
            "a busca por {text:?} não mudou em {limit:?}"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn keys(results: &ConfigSearchResults) -> Vec<String> {
    results
        .groups
        .iter()
        .flat_map(|group| &group.hits)
        .filter_map(|hit| {
            hit.key
                .as_ref()
                .map(warden_configs::index::MarkedText::plain)
        })
        .collect()
}

#[test]
fn ca_t12_06_busca_em_500_configs_reais() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let written = write_pack(root, PACK_FILES);
    // A chave do critério: `spawnRate` (o corpus tem `spawnLogic`, `spawnHasFuzz`…).
    fs::write(
        root.join("config/mobs-common.toml"),
        "[mobs]\n\t#Chance de um mob aparecer\n\tspawnRate = 5\n",
    )
    .unwrap();

    let started = Instant::now();
    let live = start(root);
    assert!(live.index.wait_ready(Duration::from_secs(120)));
    let indexing = started.elapsed();
    let status = live.index.status();
    assert!(status.available && status.ready);
    assert_eq!(status.total_files as usize, written.len() + 1);
    assert_eq!(status.indexed_files, status.total_files);
    eprintln!(
        "C-05: índice de {} configs reais ({} entradas) lido em {indexing:?}",
        status.total_files, status.entries
    );
    assert!(status.entries > 10_000, "{} entradas", status.entries);
    let first = live.changes.lock().unwrap().clone();
    assert!(matches!(first.as_slice(), [change] if change.first));

    // Medida com o índice pronto: a pior de cinco buscas.
    let mut slowest = Duration::ZERO;
    let mut results = None;
    for _ in 0..5 {
        let at = Instant::now();
        results = Some(search(&live, "spawn"));
        slowest = slowest.max(at.elapsed());
    }
    let results = results.unwrap();
    eprintln!(
        "C-05: \"spawn\" em {} arquivos ({} resultados), pior de 5 buscas: {slowest:?}",
        results.total_files, results.total_hits
    );
    assert!(slowest < SEARCH_LIMIT, "busca levou {slowest:?}");

    // Agrupada por arquivo: cada arquivo aparece uma vez, com os resultados na ordem das linhas.
    // (o corpus repetido tem mais de 500 resultados: os últimos arquivos ficam cortados).
    let mut paths: Vec<&str> = results.groups.iter().map(|g| g.path.as_str()).collect();
    assert!(results.truncated && results.total_hits > 500);
    assert!(paths.len() <= results.total_files as usize);
    paths.sort_unstable();
    paths.dedup();
    assert_eq!(paths.len(), results.groups.len());
    for group in &results.groups {
        assert!(
            group
                .hits
                .windows(2)
                .all(|pair| pair[0].line <= pair[1].line)
        );
        assert!(group.total as usize >= group.hits.len());
    }

    // Sem diferenciar maiúsculas: "Spawn" e `spawnRate` na mesma busca.
    let found = keys(&results);
    assert!(found.iter().any(|key| key == "mobs.spawnRate"), "{found:?}");
    assert!(
        found.iter().any(|key| key.contains("Spawn")),
        "nenhuma chave com Spawn: {found:?}"
    );
    assert!(found.iter().any(|key| key.contains("spawnHasFuzz")));
    // O arquivo com `spawn` na chave vem antes de um que só tem no comentário.
    let first_group = &results.groups[0];
    assert_eq!(first_group.hits[0].field, HitField::Key);

    // A linha de cada resultado é a da chave: clicar abre o arquivo nela.
    for group in &results.groups {
        let text = String::from_utf8_lossy(&fs::read(root.join(&group.path)).unwrap()).into_owned();
        let lines: Vec<&str> = text.lines().collect();
        for hit in &group.hits {
            // Sem os escapes do formato (`minecraft\:bloco` no `.properties`).
            let source = lines[hit.line as usize - 1]
                .replace('\\', "")
                .to_lowercase();
            let wanted = match (&hit.key, &hit.text) {
                (Some(key), _) => key_name(&key.plain()),
                (None, Some(text)) => text.plain().trim_end_matches('…').to_lowercase(),
                (None, None) => panic!("resultado sem chave nem linha em {}", group.path),
            };
            assert!(
                source.contains(&wanted),
                "{}:{}: {source:?} não tem {wanted:?}",
                group.path,
                hit.line
            );
        }
    }

    // "Só deste mod" deixa só os arquivos daquele mod.
    let mods = results.mods.clone();
    assert!(!mods.is_empty());
    let chosen = &mods[0].id;
    let only = live
        .index
        .search(
            &SearchQuery::new("spawn").with_mod(Some(chosen.clone())),
            &CancellationToken::new(),
        )
        .unwrap();
    assert!(!only.groups.is_empty());
    assert!(
        only.groups
            .iter()
            .all(|g| g.mod_id.as_deref() == Some(chosen.as_str()))
    );
    assert_eq!(only.total_hits, mods[0].hits);
}

/// O nome da chave como ele aparece no arquivo: o último passo do caminho, sem aspas nem índice
/// de lista, em minúsculas.
fn key_name(path: &str) -> String {
    let last = if let Some(inner) = path.strip_suffix('"') {
        // `secao."nome com ponto"`
        inner.rsplit_once('"').map_or(inner, |(_, name)| name)
    } else {
        path.rsplit('.').next().unwrap_or(path)
    };
    let last = last.split('[').next().unwrap_or(last);
    last.replace("\\\"", "\"").to_lowercase()
}

#[test]
fn ca_t12_07_edicao_por_fora_atualiza_a_busca_em_ate_2_s() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    write_pack(root, 40);
    fs::create_dir_all(root.join("config")).unwrap();
    let file = root.join("config/vale-common.toml");
    fs::write(&file, "#Ovelhas\nsheepCount = 3\n").unwrap();
    let live = start(root);
    assert!(live.index.wait_ready(Duration::from_secs(60)));
    assert!(search(&live, "creeperzinho").groups.is_empty());

    // Editado por fora: a chave nova aparece e a antiga some.
    fs::write(&file, "#Ovelhas\ncreeperzinhoCount = 3\n").unwrap();
    let took = wait_for(&live, "creeperzinho", UPDATE_LIMIT, |r| r.total_hits == 1);
    eprintln!("C-05: edição por fora apareceu na busca em {took:?}");
    assert!(search(&live, "sheepCount").groups.is_empty());

    // Arquivo novo e arquivo apagado.
    fs::write(
        root.join("config/novo.json"),
        "{\"creeperzinhoNovo\": true}\n",
    )
    .unwrap();
    wait_for(&live, "creeperzinho", UPDATE_LIMIT, |r| r.total_files == 2);
    fs::remove_file(&file).unwrap();
    wait_for(&live, "creeperzinho", UPDATE_LIMIT, |r| {
        r.total_files == 1 && r.groups[0].path == "config/novo.json"
    });

    // Cada mudança avisou quem chamou, com os arquivos que mudaram.
    let changes = live.changes.lock().unwrap().clone();
    assert!(changes[0].first);
    assert!(
        changes[1..]
            .iter()
            .all(|change| !change.first && change.changed_files > 0)
    );
    assert!(changes.len() >= 4, "{changes:?}");
}

#[test]
fn mesma_gravacao_rapida_com_mesmo_tamanho_e_vista_pelo_aviso() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("a.toml");
    fs::write(&file, "aaaa = 1\n").unwrap();
    let live = start(dir.path());
    assert!(live.index.wait_ready(Duration::from_secs(10)));
    // Mesmo tamanho; a data pode ficar igual: o aviso do observador obriga a reler.
    fs::write(&file, "bbbb = 1\n").unwrap();
    wait_for(&live, "bbbb", UPDATE_LIMIT, |r| r.total_hits == 1);
}

#[test]
fn origem_que_ainda_nao_existe_e_aparece_depois() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("minecraft");
    let live = start(&root);
    assert!(live.index.wait_ready(Duration::from_secs(10)));
    assert!(!live.index.status().available);
    assert!(search(&live, "x").groups.is_empty());

    fs::create_dir_all(root.join("config")).unwrap();
    fs::write(root.join("config/a.toml"), "primeiroTeste = 1\n").unwrap();
    // Sem observador (a pasta não existia): a atualização vem pelo pedido de quem busca.
    live.index.refresh(Duration::ZERO);
    wait_for(&live, "primeiroTeste", UPDATE_LIMIT, |r| r.total_hits == 1);
    assert!(live.index.status().available);

    // Agora observada: a próxima edição chega sozinha.
    fs::write(root.join("config/a.toml"), "segundoTeste = 1\n").unwrap();
    wait_for(&live, "segundoTeste", UPDATE_LIMIT, |r| r.total_hits == 1);

    // A pasta some (instância apagada): índice vazio.
    fs::remove_dir_all(&root).unwrap();
    live.index.refresh(Duration::ZERO);
    let started = Instant::now();
    while live.index.status().available {
        assert!(started.elapsed() < UPDATE_LIMIT);
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(search(&live, "segundoTeste").groups.is_empty());
}

#[test]
fn refresh_respeita_o_intervalo_e_soltar_para_de_observar() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("a.toml");
    fs::write(&file, "a = 1\n").unwrap();
    let calls = Arc::new(AtomicU32::new(0));
    let counter = Arc::clone(&calls);
    let live = LiveIndex::start(LiveIndexOptions {
        root: dir.path().to_path_buf(),
        lister: Arc::new(move |root: &Path| {
            counter.fetch_add(1, Ordering::SeqCst);
            list(root)
        }),
        relevant: Arc::new(|_: &[&str]| true),
        on_change: Arc::new(|_| {}),
        debounce: Duration::from_millis(50),
    });
    assert!(live.wait_ready(Duration::from_secs(10)));
    let after_first = calls.load(Ordering::SeqCst);
    assert_eq!(after_first, 1);

    // A última comparação é recente: o pedido não faz nada.
    live.refresh(Duration::from_secs(60));
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(calls.load(Ordering::SeqCst), after_first);
    live.refresh(Duration::ZERO);
    let started = Instant::now();
    while calls.load(Ordering::SeqCst) == after_first {
        assert!(started.elapsed() < UPDATE_LIMIT);
        std::thread::sleep(Duration::from_millis(10));
    }

    // Solto, o índice para de observar: uma edição não lista mais a pasta.
    drop(live);
    std::thread::sleep(Duration::from_millis(200));
    let before = calls.load(Ordering::SeqCst);
    fs::write(&file, "a = 2\n").unwrap();
    std::thread::sleep(Duration::from_millis(500));
    assert_eq!(calls.load(Ordering::SeqCst), before);
}

/// Medição para o relatório da C-05: um pack grande (5.000 configs reais, ~35 MB). Fica fora
/// da rodada comum pelo tempo de escrever os arquivos; rode com
/// `cargo nextest run --release -p warden-configs --run-ignored only --no-capture`.
#[test]
#[ignore = "medição"]
fn medicao_pack_grande_de_5000_configs() {
    let dir = tempfile::tempdir().unwrap();
    write_pack(dir.path(), 5_000);
    let started = Instant::now();
    let live = start(dir.path());
    assert!(live.index.wait_ready(Duration::from_secs(600)));
    let indexing = started.elapsed();
    let status = live.index.status();
    let at = Instant::now();
    let results = search(&live, "spawn");
    eprintln!(
        "C-05: pack grande: {} configs ({} entradas) lidas em {indexing:?}; \"spawn\" ({} resultados) em {:?}",
        status.total_files,
        status.entries,
        results.total_hits,
        at.elapsed()
    );
    assert_eq!(status.total_files, 5_000);
}
