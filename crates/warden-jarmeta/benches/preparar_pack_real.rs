//! Prepara um pack **real** para as medições de desempenho (A-04): baixa um `.mrpack` público
//! do Modrinth e os jars dos mods dele para uma pasta fora do repositório.
//!
//! ```text
//! WARDEN_A04_MRPACK=<URL ou arquivo .mrpack> WARDEN_A04_PACK_REAL=<pasta fora do repositório> \
//!   cargo bench -p warden-jarmeta --bench preparar_pack_real
//! ```
//!
//! Depois, com a mesma `WARDEN_A04_PACK_REAL`, `cargo bench -p warden-jarmeta`,
//! `-p warden-diagnostics` e `-p warden-export` medem também o pack real
//! (`warden-packwiz/benches/comum/pack_real.rs`). Jars já baixados e com o SHA-1 certo não são
//! baixados de novo. Nada disso entra no git: apague a pasta depois de medir (um pack de 400
//! mods passa de 1 GB). Sem as variáveis, só explica o uso (o `cargo bench -p warden-jarmeta`
//! roda este alvo também).

#![allow(
    missing_docs,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stderr
)]

use std::fmt::Write as _;
use std::fs;
use std::io::{Cursor, Read as _};
use std::path::{Component, Path, PathBuf};

use sha1::{Digest as _, Sha1};

const USER_AGENT: &str = "Kriticales/Warden (medidas de desempenho A-04)";
/// Maior arquivo aceito (o `.mrpack` com os `overrides/` ou um jar).
const LIMITE: u64 = 1024 * 1024 * 1024;

fn main() {
    // `cargo bench` passa `--bench`; nenhum outro argumento é usado.
    let (Some(origem), Some(pasta)) = (
        std::env::var_os("WARDEN_A04_MRPACK"),
        std::env::var_os("WARDEN_A04_PACK_REAL"),
    ) else {
        eprintln!(
            "A-04 | preparar pack real | pulado: defina WARDEN_A04_MRPACK (URL ou arquivo \
             .mrpack) e WARDEN_A04_PACK_REAL (pasta fora do repositório); veja docs/DESEMPENHO.md"
        );
        return;
    };
    let origem = origem.to_string_lossy().into_owned();
    let pasta = PathBuf::from(pasta);
    let mrpack = if origem.starts_with("https://") {
        baixar(&origem)
    } else {
        fs::read(&origem).unwrap_or_else(|e| panic!("{origem}: {e}"))
    };
    let mut zip = zip::ZipArchive::new(Cursor::new(mrpack)).expect(".mrpack não é um zip");
    let mut texto = String::new();
    zip.by_name("modrinth.index.json")
        .expect("o .mrpack não tem modrinth.index.json")
        .read_to_string(&mut texto)
        .expect("ler modrinth.index.json");
    let indice: serde_json::Value = serde_json::from_str(&texto).expect("modrinth.index.json");
    let mods = pasta.join("instancia").join("mods");
    let config = pasta.join("instancia").join("config");
    fs::create_dir_all(&mods).expect("pasta dos mods");
    let _ = fs::remove_dir_all(&config);
    fs::create_dir_all(&config).expect("pasta das configs");

    // As configs dos overrides (as de `client-overrides/` e `server-overrides/` ficam de fora).
    let mut configs = 0;
    for i in 0..zip.len() {
        let mut entrada = zip.by_index(i).expect("entrada do .mrpack");
        let Some(relativo) = entrada.enclosed_name() else {
            continue;
        };
        let Ok(dentro) = relativo.strip_prefix("overrides/config") else {
            continue;
        };
        if entrada.is_dir() || !seguro(dentro) {
            continue;
        }
        let destino = config.join(dentro);
        fs::create_dir_all(destino.parent().expect("pai")).expect("pasta da config");
        let mut bytes = Vec::new();
        entrada.read_to_end(&mut bytes).expect("ler config");
        fs::write(&destino, bytes).expect("gravar config");
        configs += 1;
    }

    let arquivos: Vec<(String, String, String)> = indice["files"]
        .as_array()
        .expect("files")
        .iter()
        .filter_map(|f| {
            let nome = f["path"].as_str()?.strip_prefix("mods/")?;
            (Path::new(nome)
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("jar"))
                && seguro(Path::new(nome))
                && !nome.contains('/'))
            .then(|| {
                (
                    nome.to_owned(),
                    f["hashes"]["sha1"].as_str().unwrap_or_default().to_owned(),
                    f["downloads"][0].as_str().unwrap_or_default().to_owned(),
                )
            })
        })
        .collect();
    // Oito downloads de cada vez.
    let parte = arquivos.len().div_ceil(8).max(1);
    let baixados: usize = std::thread::scope(|scope| {
        let tarefas: Vec<_> = arquivos
            .chunks(parte)
            .map(|bloco| {
                let mods = &mods;
                scope.spawn(move || {
                    let mut baixados = 0;
                    for (nome, sha1, url) in bloco {
                        let destino = mods.join(nome);
                        if fs::read(&destino).is_ok_and(|bytes| sha1_hex(&bytes) == *sha1) {
                            continue;
                        }
                        let bytes = baixar(url);
                        assert_eq!(sha1_hex(&bytes), *sha1, "{nome}: SHA-1 diferente do índice");
                        fs::write(&destino, bytes).expect("gravar jar");
                        baixados += 1;
                    }
                    baixados
                })
            })
            .collect();
        tarefas.into_iter().map(|t| t.join().unwrap()).sum()
    });
    fs::write(pasta.join("modrinth.index.json"), texto).expect("gravar modrinth.index.json");
    eprintln!(
        "A-04 | preparar pack real | {} {}: {} mods ({baixados} baixados agora), {configs} configs",
        indice["name"].as_str().unwrap_or("?"),
        indice["versionId"].as_str().unwrap_or("?"),
        arquivos.len()
    );
    imprimir_faixas(&mods, &arquivos);
}

/// Imprime os jars do pack em 20 faixas pelo tamanho, com as médias de cada uma: o formato das
/// `FAIXAS` do gerador (`warden-packwiz/benches/comum/pack_grande.rs`), para recalibrá-lo.
fn imprimir_faixas(mods: &Path, arquivos: &[(String, String, String)]) {
    // (bytes, entradas, classes, pacotes, embutidos)
    let mut jars: Vec<[usize; 5]> = arquivos
        .iter()
        .map(|(nome, _, _)| {
            let bytes = fs::read(mods.join(nome)).expect("ler jar");
            let tamanho = bytes.len();
            let zip = zip::ZipArchive::new(Cursor::new(bytes)).expect("jar não é um zip");
            let nomes: Vec<String> = zip
                .file_names()
                .map(|nome| nome.expect("nome de entrada do jar").into_owned())
                .collect();
            let classes: Vec<&String> = nomes.iter().filter(|n| extensao(n, "class")).collect();
            let pacotes: std::collections::BTreeSet<&str> = classes
                .iter()
                .filter_map(|n| n.rsplit_once('/').map(|(pacote, _)| pacote))
                .collect();
            let embutidos = nomes.iter().filter(|n| extensao(n, "jar")).count();
            [
                tamanho,
                nomes.len(),
                classes.len(),
                pacotes.len(),
                embutidos,
            ]
        })
        .collect();
    jars.sort_unstable();
    let n = jars.len();
    if n == 0 {
        return;
    }
    let media = |parte: &[[usize; 5]], campo: usize| {
        let soma: usize = parte.iter().map(|j| j[campo]).sum();
        (soma + parte.len() / 2) / parte.len().max(1)
    };
    for k in 0..20 {
        let parte = &jars[k * n / 20..((k + 1) * n / 20).max(k * n / 20 + 1).min(n)];
        eprintln!(
            "A-04 | faixa {k:2} | faixa({}, {}, {}, {}, {}),",
            media(parte, 0) / 1024,
            media(parte, 1),
            media(parte, 2),
            media(parte, 3),
            media(parte, 4)
        );
    }
}

/// O nome termina com `.<ext>` (sem diferenciar maiúsculas).
fn extensao(nome: &str, ext: &str) -> bool {
    Path::new(nome)
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case(ext))
}

/// Caminho relativo sem `..` nem raiz.
fn seguro(path: &Path) -> bool {
    path.components().all(|c| matches!(c, Component::Normal(_)))
}

fn baixar(url: &str) -> Vec<u8> {
    let mut ultimo = String::new();
    for _ in 0..3 {
        let resposta = ureq::get(url).header("User-Agent", USER_AGENT).call();
        match resposta.and_then(|mut r| r.body_mut().with_config().limit(LIMITE).read_to_vec()) {
            Ok(bytes) => return bytes,
            Err(e) => ultimo = e.to_string(),
        }
    }
    panic!("{url}: {ultimo}");
}

fn sha1_hex(bytes: &[u8]) -> String {
    Sha1::digest(bytes)
        .iter()
        .fold(String::with_capacity(40), |mut out, byte| {
            let _ = write!(out, "{byte:02x}");
            out
        })
}
