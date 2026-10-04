//! Spike R6 do Warden: segurança dos mods.
//!
//! Comandos:
//!   gen <pasta>            grava as amostras seguras e confere, 5 s depois, se o antivírus mexeu nelas
//!   hashes                 SHA-1 e SHA-256 das amostras (sem gravar nada)
//!   amsi-mem               examina as amostras pela AMSI direto da memória (arquivo inteiro e cada entrada)
//!   amsi-files <arqs…>     o mesmo, lendo arquivos do disco
//!   amsi-bench <arqs…>     mede o tempo da AMSI em jars reais (arquivo inteiro e por entrada)
//!   sigscan-mem            busca de sinais do protótipo nas amostras em memória
//!   sigscan <arqs…>        busca de sinais em jars do disco (falso positivo e tempo)

mod classgen;
#[cfg(windows)]
mod amsi;
mod samples;
mod sigscan;

use sha1::Digest;
use std::io::Read;
use std::time::Instant;

fn sha256(b: &[u8]) -> String {
    hex::encode(sha2::Sha256::digest(b))
}
fn sha1(b: &[u8]) -> String {
    hex::encode(sha1::Sha1::digest(b))
}

/// Entradas de um zip, descendo nos jars embutidos (profundidade até 3).
fn entries(bytes: &[u8], path: &str, depth: u32, out: &mut Vec<(String, Vec<u8>)>) {
    if depth > 3 {
        return;
    }
    let Ok(mut z) = zip::ZipArchive::new(std::io::Cursor::new(bytes)) else { return };
    for i in 0..z.len() {
        let Ok(mut f) = z.by_index(i) else { continue };
        let name = format!("{path}!/{}", f.name());
        let mut data = vec![];
        if f.read_to_end(&mut data).is_err() {
            continue;
        }
        if name.ends_with(".jar") {
            entries(&data, &name, depth + 1, out);
        }
        out.push((name, data));
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let cmd = args.first().map(String::as_str).unwrap_or("");
    let rest = &args[1.min(args.len())..];
    match cmd {
        "gen" => gen(&rest[0]),
        "hashes" => {
            for (n, b) in samples::all() {
                println!("{n}\tsha1={}\tsha256={}", sha1(&b), sha256(&b));
            }
        }
        #[cfg(windows)]
        "amsi-mem" => {
            let s = amsi::Amsi::new("Warden-R6");
            for (n, b) in samples::all() {
                scan_all(&s, n, &b);
            }
        }
        #[cfg(windows)]
        "amsi-nomes" => {
            // O resultado depende do nome informado à AMSI? E se repete?
            let s = amsi::Amsi::new("Warden-R6");
            for (n, b) in samples::all() {
                if !n.starts_with("stage0") && !n.starts_with("eicar") {
                    continue;
                }
                for nome in [n, "arquivo.bin", ""] {
                    for rep in 1..=2 {
                        let (r, _) = s.scan(&b, nome);
                        println!("{n}\tnome=\"{nome}\"\trepetição {rep}: {r}");
                    }
                }
            }
        }
        #[cfg(windows)]
        "amsi-files" => {
            let s = amsi::Amsi::new("Warden-R6");
            for p in rest {
                match std::fs::read(p) {
                    Ok(b) => scan_all(&s, p, &b),
                    Err(e) => println!("{p}: não deu para ler: {e}"),
                }
            }
        }
        #[cfg(windows)]
        "amsi-bench" => {
            let s = amsi::Amsi::new("Warden-R6");
            for p in rest {
                let b = std::fs::read(p).unwrap();
                let t = Instant::now();
                let r = s.scan(&b, p);
                let whole = t.elapsed();
                let mut es = vec![];
                entries(&b, p, 0, &mut es);
                let t = Instant::now();
                let mut det = 0;
                for (n, d) in &es {
                    if s.scan(d, n).1 {
                        det += 1;
                    }
                }
                println!(
                    "{p}\t{} KB\tinteiro: {:?} ({:?})\t{} entradas: {:?}, detectadas {det}",
                    b.len() / 1024,
                    whole,
                    r,
                    es.len(),
                    t.elapsed()
                );
            }
        }
        "sigscan-mem" => {
            for (n, b) in samples::all() {
                report_sig(n, &b);
            }
        }
        "sigscan" => {
            let t = Instant::now();
            let mut total = 0usize;
            for p in rest {
                let b = std::fs::read(p).unwrap();
                total += b.len();
                report_sig(p, &b);
            }
            println!("total: {} arquivos, {} MB, {:?}", rest.len(), total / 1_048_576, t.elapsed());
        }
        _ => eprintln!("uso: veja o comentário no topo de main.rs"),
    }
}

fn report_sig(name: &str, b: &[u8]) {
    let mut f = vec![];
    let t = Instant::now();
    sigscan::scan_jar(b, name, 0, &mut f);
    let el = t.elapsed();
    if f.is_empty() {
        println!("{name}: nenhum achado ({el:?})");
    }
    for x in f {
        println!("{name}: [{}] {} em {} -> {}", x.level, x.rule, x.path, x.detail);
    }
}

#[cfg(windows)]
fn scan_all(s: &amsi::Amsi, name: &str, b: &[u8]) {
    let (r, det, el) = {
        let t = Instant::now();
        let (r, d) = s.scan(b, name);
        (r, d, t.elapsed())
    };
    println!("{name} (arquivo inteiro, {} bytes): resultado={r} detectado={det} ({el:?})", b.len());
    let mut es = vec![];
    entries(b, name, 0, &mut es);
    for (n, d) in es {
        let (r, det) = s.scan(&d, &n);
        println!("    {n}: resultado={r} detectado={det}");
    }
}

fn gen(dir: &str) {
    std::fs::create_dir_all(dir).unwrap();
    let all = samples::all();
    for (n, b) in &all {
        let p = std::path::Path::new(dir).join(n);
        match std::fs::write(&p, b) {
            Ok(()) => println!("gravado {n} ({} bytes, sha256 {})", b.len(), sha256(b)),
            Err(e) => println!("FALHOU ao gravar {n}: {e}"),
        }
    }
    std::thread::sleep(std::time::Duration::from_secs(5));
    println!("--- 5 s depois");
    for (n, b) in &all {
        let p = std::path::Path::new(dir).join(n);
        match std::fs::read(&p) {
            Ok(x) if x == *b => println!("{n}: intacto"),
            Ok(x) => println!("{n}: ALTERADO ({} bytes agora)", x.len()),
            Err(e) => println!("{n}: SUMIU ou bloqueado: {e}"),
        }
    }
}
