//! hsperf-probe: leitor de prova do `hsperfdata` (spike S-R5-2 do Warden).
//!
//! Uso:
//!   hsperf-probe read <pid> [--temp DIR] [--via map|file]
//!   hsperf-probe counters <pid> [--temp DIR] | counters --file ARQ
//!   hsperf-probe dump <pid> <saida> [--temp DIR]
//!   hsperf-probe compare <pid> --jstat JSTAT [--temp DIR] [--count N] [--interval-ms MS] [--label L]
//!   hsperf-probe parse <arquivo>
//!   hsperf-probe anonymize <entrada> <saida> <texto>=<marcador> | @<contador>=<texto> [...]

mod perfdata;
#[cfg(windows)]
mod win;

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Instant;

use perfdata::{GcSummary, Snapshot, Value};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("erro: {e}");
            ExitCode::FAILURE
        }
    }
}

type Res<T> = Result<T, Box<dyn std::error::Error>>;

fn opt<'a>(args: &'a [String], name: &str) -> Option<&'a str> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1))
        .map(String::as_str)
}

fn temp_dir(args: &[String]) -> PathBuf {
    opt(args, "--temp")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
}

fn run(args: &[String]) -> Res<()> {
    let cmd = args.first().map(String::as_str).unwrap_or("");
    match cmd {
        "parse" => {
            let snap = perfdata::parse(&std::fs::read(args.get(1).ok_or("falta o arquivo")?)?)?;
            println!("{}", summary_json(&snap, None));
            Ok(())
        }
        "counters" => {
            let buf = if let Some(f) = opt(args, "--file") {
                std::fs::read(f)?
            } else {
                read_live(args, pid_arg(args)?, "map")?.0
            };
            let snap = perfdata::parse(&buf)?;
            println!(
                "# versão {}.{}, acessível={}, {} contadores, {} bytes usados de {}, mod_time_stamp={}",
                snap.major,
                snap.minor,
                snap.accessible,
                snap.counters.len(),
                snap.used,
                buf.len(),
                snap.mod_time_stamp
            );
            for (k, c) in &snap.counters {
                let v = match &c.value {
                    Value::Long(v) => v.to_string(),
                    Value::Text(s) => format!("{s:?}"),
                    Value::Other { data_type, len } => {
                        format!("<tipo {} x{len}>", *data_type as char)
                    }
                };
                println!("{k} = {v}  (u{} v{})", c.units, c.variability);
            }
            Ok(())
        }
        "read" => {
            let via = opt(args, "--via").unwrap_or("map");
            let t = Instant::now();
            let (buf, src) = read_live(args, pid_arg(args)?, via)?;
            let took = t.elapsed();
            let snap = perfdata::parse(&buf)?;
            println!("{}", summary_json(&snap, Some(&src)));
            eprintln!(
                "leitura em {:.3} ms ({} bytes)",
                took.as_secs_f64() * 1e3,
                buf.len()
            );
            Ok(())
        }
        "dump" => {
            let (buf, src) = read_live(args, pid_arg(args)?, "map")?;
            let out = args.get(2).ok_or("falta a saída")?;
            // Grava só a parte usada (o resto da vista é zero).
            let snap = perfdata::parse(&buf)?;
            let used = usize::try_from(snap.used)
                .unwrap_or(buf.len())
                .min(buf.len());
            std::fs::write(out, &buf[..used])?;
            eprintln!("gravados {used} bytes de {src}");
            Ok(())
        }
        "compare" => compare(args),
        "anonymize" => {
            let input = args.get(1).ok_or("falta a entrada")?;
            let output = args.get(2).ok_or("falta a saída")?;
            let mut buf = std::fs::read(input)?;
            for pair in &args[3..] {
                // "@contador=texto": troca o texto inteiro do contador (completa com NUL).
                if let Some(rest) = pair.strip_prefix('@') {
                    let (name, repl) = rest.split_once('=').ok_or("use @contador=texto")?;
                    let snap = perfdata::parse(&buf)?;
                    let old = snap
                        .text(name)
                        .ok_or_else(|| format!("{name} não é texto"))?
                        .to_owned();
                    if !old.is_ascii() || repl.len() > old.len() {
                        return Err(format!("{name}: texto não ASCII ou substituto maior").into());
                    }
                    let mut new = repl.as_bytes().to_vec();
                    new.resize(old.len(), 0);
                    let n = replace_all(&mut buf, old.as_bytes(), &new)?;
                    eprintln!("{name}: texto trocado ({n} ocorrência)");
                    continue;
                }
                let (from, to) = pair.split_once('=').ok_or("use texto=marcador")?;
                let n = replace_all(&mut buf, from.as_bytes(), to.as_bytes())?;
                eprintln!("{from:?}: {n} ocorrência(s) trocadas");
            }
            perfdata::parse(&buf)?; // continua válido
            std::fs::write(output, buf)?;
            Ok(())
        }
        _ => Err("comando desconhecido; veja o cabeçalho de src/main.rs".into()),
    }
}

fn pid_arg(args: &[String]) -> Res<u32> {
    Ok(args.get(1).ok_or("falta o pid")?.parse()?)
}

/// Troca `from` por `to` (mesmo tamanho, para não mudar deslocamentos), sem
/// diferenciar maiúsculas de minúsculas no ASCII.
fn replace_all(buf: &mut [u8], from: &[u8], to: &[u8]) -> Res<usize> {
    if from.len() != to.len() || from.is_empty() {
        return Err("o marcador precisa ter o mesmo tamanho do texto".into());
    }
    let mut n = 0;
    let mut i = 0;
    while i + from.len() <= buf.len() {
        if buf[i..i + from.len()].eq_ignore_ascii_case(from) {
            buf[i..i + from.len()].copy_from_slice(to);
            n += 1;
            i += from.len();
        } else {
            i += 1;
        }
    }
    Ok(n)
}

#[cfg(windows)]
fn read_live(args: &[String], pid: u32, via: &str) -> Res<(Vec<u8>, String)> {
    let temp = temp_dir(args);
    let loc = win::locate(&temp, pid)
        .ok_or_else(|| format!("nenhum {}\\hsperfdata_*\\{pid}", temp.display()))?;
    match via {
        "file" => Ok((
            win::read_file(&loc.file)?,
            format!("arquivo {}", loc.file.display()),
        )),
        _ => {
            let m = win::Mapping::open(&loc.mapping_name)?;
            Ok((
                m.snapshot(),
                format!(
                    "mapeamento {} (sufixo {:?}, {} bytes)",
                    loc.mapping_name,
                    loc.user_suffix,
                    m.len()
                ),
            ))
        }
    }
}

#[cfg(not(windows))]
fn read_live(args: &[String], pid: u32, _via: &str) -> Res<(Vec<u8>, String)> {
    // Linux: /tmp/hsperfdata_<usuário>/<pid>; mesmo princípio de busca.
    let temp = opt(args, "--temp")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/tmp"));
    for e in std::fs::read_dir(&temp)?.flatten() {
        if e.file_name().to_string_lossy().starts_with("hsperfdata_") {
            let f = e.path().join(pid.to_string());
            if f.is_file() {
                return Ok((std::fs::read(&f)?, f.display().to_string()));
            }
        }
    }
    Err(format!("nenhum {}/hsperfdata_*/{pid}", temp.display()).into())
}

fn f(v: Option<f64>) -> String {
    v.map_or("null".into(), |x| format!("{x:.1}"))
}

fn summary_json(snap: &Snapshot, src: Option<&str>) -> String {
    let g = snap.gc_summary();
    format!(
        "{{\"src\":{:?},\"vm\":{:?},\"version\":{:?},\"heap_used_kb\":{:.1},\"heap_capacity_kb\":{:.1},\
         \"S0U\":{},\"S1U\":{},\"EU\":{},\"OU\":{},\"EC\":{},\"OC\":{},\"MU\":{},\"gc_count\":{},\"gc_time_s\":{:.3},\
         \"collectors\":[{}]}}",
        src.unwrap_or("-"),
        snap.text("java.property.java.vm.name").unwrap_or("?"),
        snap.text("java.property.java.version").unwrap_or("?"),
        g.heap_used_kb,
        g.heap_capacity_kb,
        f(g.s0u),
        f(g.s1u),
        f(g.eu),
        f(g.ou),
        f(g.ec),
        f(g.oc),
        f(g.mu),
        g.gc_count,
        g.gc_time_s,
        g.collectors.iter().map(|(n, t)| format!("[{n},{t:.3}]")).collect::<Vec<_>>().join(","),
    )
}

/// Saída do `jstat -gc <pid>`: cabeçalho e valores separados por espaços.
fn run_jstat(jstat: &Path, pid: u32) -> Res<Vec<(String, f64)>> {
    let out = std::process::Command::new(jstat)
        .arg("-gc")
        .arg(pid.to_string())
        .output()?;
    let text = String::from_utf8_lossy(&out.stdout);
    let mut lines = text.lines().filter(|l| !l.trim().is_empty());
    let head: Vec<&str> = lines
        .next()
        .ok_or("jstat sem saída")?
        .split_whitespace()
        .collect();
    let vals: Vec<&str> = lines
        .next()
        .ok_or_else(|| {
            format!(
                "jstat sem valores: {text} {}",
                String::from_utf8_lossy(&out.stderr)
            )
        })?
        .split_whitespace()
        .collect();
    Ok(head
        .iter()
        .zip(vals)
        .map(|(h, v)| {
            (
                h.to_string(),
                v.replace(',', ".").parse::<f64>().unwrap_or(f64::NAN),
            )
        })
        .collect())
}

fn jget(j: &[(String, f64)], k: &str) -> f64 {
    j.iter().find(|(h, _)| h == k).map_or(f64::NAN, |(_, v)| *v)
}

/// Número para JSON (`null` quando o jstat mostrou "-").
fn jnum(v: f64) -> String {
    if v.is_nan() {
        "null".into()
    } else {
        v.to_string()
    }
}

/// Lê (A), roda o jstat, lê de novo (B). Imprime uma linha JSON por rodada.
fn compare(args: &[String]) -> Res<()> {
    let pid = pid_arg(args)?;
    let jstat = PathBuf::from(opt(args, "--jstat").ok_or("falta --jstat")?);
    let count: u32 = opt(args, "--count").unwrap_or("1").parse()?;
    let interval: u64 = opt(args, "--interval-ms").unwrap_or("1000").parse()?;
    let label = opt(args, "--label").unwrap_or("");
    for i in 0..count {
        let t0 = Instant::now();
        let a = perfdata::parse(&read_live(args, pid, "map")?.0)?.gc_summary();
        let t1 = Instant::now();
        let j = run_jstat(&jstat, pid)?;
        let t2 = Instant::now();
        let b = perfdata::parse(&read_live(args, pid, "map")?.0)?.gc_summary();
        // O jstat mostra "-" nas colunas que o coletor não tem (ZGC, Shenandoah): conta 0.
        let sum = |ks: &[&str]| {
            ks.iter()
                .map(|k| jget(&j, k))
                .filter(|v| !v.is_nan())
                .sum::<f64>()
        };
        let j_used = sum(&["S0U", "S1U", "EU", "OU"]);
        let j_gc = sum(&["YGC", "FGC", "CGC"]);
        let near = |x: &GcSummary| x.heap_used_kb;
        // Diferença contra a leitura mais próxima (A ou B) do valor do jstat.
        let best = if (near(&a) - j_used).abs() <= (near(&b) - j_used).abs() {
            near(&a)
        } else {
            near(&b)
        };
        let diff_pct = if j_used > 0.0 {
            (best - j_used) / j_used * 100.0
        } else {
            f64::NAN
        };
        println!(
            "{{\"label\":{label:?},\"i\":{i},\"reader_a_kb\":{:.1},\"jstat_kb\":{:.1},\"reader_b_kb\":{:.1},\
             \"diff_pct\":{:.3},\"reader_gc_a\":{},\"jstat_gc\":{},\"reader_gc_b\":{},\"reader_gct_a\":{:.3},\
             \"jstat_gct\":{:.3},\"jstat_ms\":{},\"reader_ms\":{:.3},\
             \"reader\":{{\"S0U\":{},\"S1U\":{},\"EU\":{},\"OU\":{},\"MU\":{}}},\
             \"jstat\":{{\"S0U\":{},\"S1U\":{},\"EU\":{},\"OU\":{},\"MU\":{}}}}}",
            a.heap_used_kb,
            j_used,
            b.heap_used_kb,
            diff_pct,
            a.gc_count,
            j_gc,
            b.gc_count,
            a.gc_time_s,
            jget(&j, "GCT"),
            (t2 - t1).as_millis(),
            (t1 - t0).as_secs_f64() * 1e3,
            f(a.s0u),
            f(a.s1u),
            f(a.eu),
            f(a.ou),
            f(a.mu),
            jnum(jget(&j, "S0U")),
            jnum(jget(&j, "S1U")),
            jnum(jget(&j, "EU")),
            jnum(jget(&j, "OU")),
            jnum(jget(&j, "MU")),
        );
        if i + 1 < count {
            std::thread::sleep(std::time::Duration::from_millis(interval));
        }
    }
    Ok(())
}
