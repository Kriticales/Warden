//! Spike R6: YARA-X sobre jars. Aplica as regras de `regras.yar` a cada jar
//! (arquivo inteiro) e a cada entrada descompactada, descendo em jars embutidos.
//! Uso: r6yarax <regras.yar> <jars…>   |   r6yarax <regras.yar> --amostras

#[path = "../../probe/src/classgen.rs"]
mod classgen;
#[path = "../../probe/src/samples.rs"]
#[allow(dead_code)]
mod samples;

use std::io::Read;
use std::time::Instant;

fn entries(bytes: &[u8], path: &str, depth: u32, out: &mut Vec<(String, Vec<u8>)>) {
    if depth > 3 {
        return;
    }
    let Ok(mut z) = zip::ZipArchive::new(std::io::Cursor::new(bytes)) else { return };
    for i in 0..z.len() {
        let Ok(mut f) = z.by_index(i) else { continue };
        let name = format!("{path}!/{}", f.name());
        let mut d = vec![];
        if f.read_to_end(&mut d).is_err() {
            continue;
        }
        if name.ends_with(".jar") {
            entries(&d, &name, depth + 1, out);
        }
        out.push((name, d));
    }
}

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let src = std::fs::read_to_string(&a[0]).unwrap();
    let t = Instant::now();
    let rules = yara_x::compile(src.as_str()).unwrap();
    println!("regras compiladas em {:?}", t.elapsed());
    let mut scanner = yara_x::Scanner::new(&rules);
    let inputs: Vec<(String, Vec<u8>)> = if a.get(1).map(String::as_str) == Some("--amostras") {
        samples::all().into_iter().map(|(n, b)| (n.to_string(), b)).collect()
    } else {
        a[1..].iter().map(|p| (p.clone(), std::fs::read(p).unwrap())).collect()
    };
    let t = Instant::now();
    let (mut total, mut n_entries) = (0usize, 0usize);
    for (name, bytes) in &inputs {
        total += bytes.len();
        let mut hits = vec![];
        let r = scanner.scan(bytes).unwrap();
        for m in r.matching_rules() {
            hits.push(format!("{} (jar inteiro)", m.identifier()));
        }
        let mut es = vec![];
        entries(bytes, name, 0, &mut es);
        n_entries += es.len();
        for (en, d) in &es {
            let r = scanner.scan(d).unwrap();
            for m in r.matching_rules() {
                hits.push(format!("{} em {en}", m.identifier()));
            }
        }
        println!("{name}: {}", if hits.is_empty() { "nada".into() } else { hits.join("; ") });
    }
    println!("total: {} arquivos, {} entradas, {} MB, {:?}", inputs.len(), n_entries, total / 1_048_576, t.elapsed());
}
