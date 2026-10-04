//! Spike S-R5-3 do Warden — marcadores e mecanismos da busca do culpado.
//!
//! Uso:
//!   bisect-proto run <mc> <loader> [opções]     abre o cliente e mede os marcadores
//!   bisect-proto server <pasta> [opções]        abre só o servidor dedicado da pasta
//!
//! `<loader>`: `vanilla`, `fabric[:<versão do loader>]`, `forge:<versão maven>`
//! (ex.: `1.12.2-14.23.5.2860`) ou `neoforge:<versão>` (ex.: `21.1.252`).
//!
//! Opções de `run`:
//!   --instance <nome>       pasta da instância em <dados>/instances (padrão: <mc>-<loader>)
//!   --goal menu|world|exit  o que esperar (padrão: menu)
//!   --world <nome>          acrescenta `--quickPlaySingleplayer <nome>` (1.20+)
//!   --qp-path <arquivo>     acrescenta `--quickPlayPath <arquivo>` (relativo à instância)
//!   --qp-multi <host:porta> acrescenta `--quickPlayMultiplayer host:porta` (1.20+)
//!   --legacy-server <host:porta>  acrescenta `--server host --port porta` (antes de 1.20)
//!   --prop k=v              propriedade da JVM (`-Dk=v`), pode repetir
//!   --jvm-arg <arg>         argumento extra da JVM (ex.: `-Xmx256M`), pode repetir
//!   --java <caminho>        Java fixo em vez do runtime da Mojang
//!   --option k:v            grava a linha no `options.txt` da instância, pode repetir
//!   --server-dir <pasta>    sobe antes o servidor dedicado dessa pasta (ver `server`)
//!   --timeout <s>  --hold <s>  --tag <rótulo>  --print-command
//!
//! Opções de `server`: --java <caminho> --prop k=v --timeout <s> --hold <s> --tag <rótulo>
//! A pasta precisa ter `warden-args.txt` (um argumento por linha, depois do `java`),
//! gerado por `scripts/prep_server.py`.
//!
//! Tudo fica em `$WARDEN_SPIKE_DATA` (padrão `C:\wt\s-r5-3\data`), fora do repositório.

mod engine;
mod markers;
mod supervisor;

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::time::{Duration, Instant};

use engine::{CancelFlag, EngineDirs, EngineEvent, InstallRequest, JavaSpec, LauncherEngine, LoaderSpec, PortableMcEngine};
use supervisor::{LogLine, RunningProcess};

const PLAYER: &str = "WardenTest";

fn data_root() -> PathBuf {
    std::env::var_os("WARDEN_SPIKE_DATA").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(r"C:\wt\s-r5-3\data"))
}

struct Args(Vec<String>);

impl Args {
    fn opt(&self, name: &str) -> Option<String> {
        self.0.iter().position(|a| a == name).and_then(|i| self.0.get(i + 1)).cloned()
    }
    fn all(&self, name: &str) -> Vec<String> {
        self.0.windows(2).filter(|w| w[0] == name).map(|w| w[1].clone()).collect()
    }
    fn flag(&self, name: &str) -> bool {
        self.0.iter().any(|a| a == name)
    }
    fn secs(&self, name: &str, default: u64) -> Duration {
        Duration::from_secs(self.opt(name).and_then(|s| s.parse().ok()).unwrap_or(default))
    }
}

fn parse_loader(s: &str) -> Option<(LoaderSpec, &'static str)> {
    let (kind, ver) = s.split_once(':').map(|(a, b)| (a, Some(b.to_string()))).unwrap_or((s, None));
    Some(match kind {
        "vanilla" => (LoaderSpec::Vanilla, "vanilla"),
        "fabric" => (LoaderSpec::Fabric { loader_version: ver }, "fabric"),
        "quilt" => (LoaderSpec::Quilt { loader_version: ver }, "quilt"),
        "forge" => (LoaderSpec::Forge { version: ver? }, "forge"),
        "neoforge" => (LoaderSpec::NeoForge { version: ver? }, "neoforge"),
        _ => return None,
    })
}

fn main() -> ExitCode {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    match argv.first().map(String::as_str) {
        Some("run") if argv.len() >= 3 => run_client(&argv[1], &argv[2], Args(argv[3..].to_vec())),
        Some("server") if argv.len() >= 2 => run_server_only(Path::new(&argv[1]), Args(argv[2..].to_vec())),
        _ => {
            eprintln!("uso: bisect-proto run <mc> <loader> [opções] | bisect-proto server <pasta> [opções]");
            ExitCode::from(2)
        }
    }
}

/// Registro dos marcadores vistos num processo (primeira ocorrência de cada um).
#[derive(Default)]
struct Seen {
    first: BTreeMap<&'static str, (Duration, String)>,
}

impl Seen {
    fn feed(&mut self, who: &str, line: &LogLine) -> Vec<&'static str> {
        let mut new = Vec::new();
        for mk in markers::CATALOG {
            if !self.first.contains_key(mk.id) && markers::matches(mk, &line.text) {
                let excerpt: String = clean(&line.text).chars().take(220).collect();
                println!("[{:>7.1}s {who}] marcador {}: {}", line.at.as_secs_f64(), mk.id, excerpt);
                self.first.insert(mk.id, (line.at, excerpt));
                new.push(mk.id);
            }
        }
        new
    }
    fn has(&self, id: &str) -> bool {
        self.first.contains_key(id)
    }
    fn at(&self, id: &str) -> Option<Duration> {
        self.first.get(id).map(|(d, _)| *d)
    }
    fn json(&self) -> String {
        let mut s = String::from("{");
        for (i, (k, (d, _))) in self.first.iter().enumerate() {
            if i > 0 {
                s.push(',');
            }
            let _ = write!(s, "\"{k}\":{:.1}", d.as_secs_f64());
        }
        s.push('}');
        s
    }
}

/// Extrai o texto de uma linha XML do log4j (`<log4j:Message><![CDATA[...]]>`), se for o caso.
fn clean(line: &str) -> String {
    if let Some(i) = line.find("<![CDATA[") {
        let rest = &line[i + 9..];
        return rest.split("]]>").next().unwrap_or(rest).to_string();
    }
    line.to_string()
}

fn stamp() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs()
}

fn write_options(instance: &Path, opts: &[String]) {
    if opts.is_empty() {
        return;
    }
    let path = instance.join("options.txt");
    let mut lines: Vec<String> = std::fs::read_to_string(&path).map(|s| s.lines().map(str::to_string).collect()).unwrap_or_default();
    for o in opts {
        let key = o.split(':').next().unwrap_or(o);
        lines.retain(|l| l.split(':').next() != Some(key));
        lines.push(o.clone());
    }
    std::fs::write(&path, lines.join("\n") + "\n").expect("gravar options.txt");
}

/// Servidor dedicado de uma pasta preparada por `prep_server.py`.
struct ServerSpec {
    dir: PathBuf,
    java: PathBuf,
    props: Vec<String>,
}

impl ServerSpec {
    fn command(&self) -> Command {
        let args = std::fs::read_to_string(self.dir.join("warden-args.txt")).expect("warden-args.txt na pasta do servidor");
        let mut cmd = Command::new(&self.java);
        cmd.current_dir(&self.dir);
        for p in &self.props {
            cmd.arg(format!("-D{p}"));
        }
        cmd.args(args.lines().map(str::trim).filter(|l| !l.is_empty()));
        cmd
    }
}

/// `java.exe` ao lado do `javaw.exe` (servidor precisa de console para ler `stop`).
fn console_java(java: &Path) -> PathBuf {
    let candidate = java.with_file_name(if cfg!(windows) { "java.exe" } else { "java" });
    if candidate.exists() { candidate } else { java.to_path_buf() }
}

fn start_server(spec: &ServerSpec, tag: &str, timeout: Duration) -> Result<(RunningProcess, Seen), String> {
    let capture = data_root().join("runs").join(format!("{tag}-server-{}.log", stamp()));
    let mut srv = RunningProcess::spawn(spec.command(), &capture, true).map_err(|e| format!("falha ao iniciar servidor: {e}"))?;
    println!("== servidor iniciado (pid {}), saída em {}", srv.pid(), capture.display());
    let mut seen = Seen::default();
    loop {
        if srv.elapsed() > timeout {
            return Err(format!("servidor: tempo esgotado ({}s)", timeout.as_secs()));
        }
        match srv.next_line(Duration::from_millis(500)) {
            Ok(line) => {
                seen.feed("srv", &line);
                if seen.has("server-done") {
                    return Ok((srv, seen));
                }
                // A pergunta do StartupQuery no servidor espera "/fml confirm" no console.
                if line.text.contains("/fml confirm") {
                    println!("[{:>7.1}s srv] pergunta do FML no console: {}", line.at.as_secs_f64(), line.text);
                }
            }
            Err(_) => {
                if let Some(st) = srv.try_exit_status() {
                    return Err(format!("servidor saiu antes de ficar pronto: {st}"));
                }
            }
        }
    }
}

fn run_server_only(dir: &Path, a: Args) -> ExitCode {
    std::fs::create_dir_all(data_root().join("runs")).ok();
    let Some(java) = a.opt("--java").map(PathBuf::from) else {
        eprintln!("--java é obrigatório");
        return ExitCode::from(2);
    };
    let spec = ServerSpec { dir: dir.to_path_buf(), java: console_java(&java), props: a.all("--prop") };
    let tag = a.opt("--tag").unwrap_or_else(|| "server".into());
    let hold = a.secs("--hold", 0);
    match start_server(&spec, &tag, a.secs("--timeout", 600)) {
        Ok((mut srv, mut seen)) => {
            println!("== SERVIDOR PRONTO em {:.1}s", seen.at("server-done").unwrap().as_secs_f64());
            let until = Instant::now() + hold;
            while Instant::now() < until {
                if let Ok(l) = srv.next_line(Duration::from_millis(200)) {
                    seen.feed("srv", &l);
                }
            }
            let report = srv.stop(Some("stop"), Duration::from_secs(60), |l| {
                seen.feed("srv", l);
            });
            println!("== servidor encerrado: {:?} forçado={} em {:.1}s", report.status.map(|s| s.to_string()), report.forced, report.took.as_secs_f64());
            println!("== marcadores-servidor {}", seen.json());
            ExitCode::SUCCESS
        }
        Err(e) => {
            println!("!! {e}");
            ExitCode::from(1)
        }
    }
}

fn run_client(mc: &str, loader_arg: &str, a: Args) -> ExitCode {
    let Some((loader, loader_kind)) = parse_loader(loader_arg) else {
        eprintln!("loader inválido: {loader_arg}");
        return ExitCode::from(2);
    };
    let root = data_root();
    let instance_name = a.opt("--instance").unwrap_or_else(|| format!("{mc}-{}", loader_arg.replace(':', "-")));
    let tag = a.opt("--tag").unwrap_or_else(|| instance_name.clone());
    let goal = a.opt("--goal").unwrap_or_else(|| "menu".into());
    let timeout = a.secs("--timeout", 900);
    let hold = a.secs("--hold", 3);
    let req = InstallRequest {
        mc_version: mc.to_string(),
        loader,
        instance_dir: root.join("instances").join(&instance_name),
        java: a.opt("--java").map(|p| JavaSpec::Path(PathBuf::from(p))).unwrap_or(JavaSpec::Mojang),
        offline_player: PLAYER.into(),
    };
    std::fs::create_dir_all(&req.instance_dir).expect("criar instância");
    std::fs::create_dir_all(root.join("runs")).expect("criar runs");
    println!("== {mc} {loader_arg} (faixa {:?}), instância {}", markers::band(mc), req.instance_dir.display());

    let t0 = Instant::now();
    let mut sink = |ev: EngineEvent| {
        if let EngineEvent::Warning(w) = ev {
            println!("[{:>7.1}s] aviso do motor: {w}", t0.elapsed().as_secs_f64());
        }
    };
    let mut prepared = match PortableMcEngine.install(&EngineDirs::under(&root.join("shared")), &req, &mut sink, &CancelFlag::default()) {
        Ok(p) => p,
        Err(e) => {
            println!("!! instalação falhou: {e}");
            return ExitCode::from(1);
        }
    };
    println!("== instalação/verificação em {:.1}s; java {}", t0.elapsed().as_secs_f64(), prepared.java.display());

    // Ajustes do Warden sobre o plano de lançamento (LaunchOptions na arquitetura).
    for p in a.all("--prop") {
        prepared.jvm_args.push(format!("-D{p}"));
    }
    for j in a.all("--jvm-arg") {
        prepared.jvm_args.push(j);
    }
    if let Some(w) = a.opt("--world") {
        prepared.game_args.extend(["--quickPlaySingleplayer".into(), w]);
    }
    let qp_path = a.opt("--qp-path");
    if let Some(p) = &qp_path {
        let _ = std::fs::remove_file(req.instance_dir.join(p));
        prepared.game_args.extend(["--quickPlayPath".into(), p.clone()]);
    }
    if let Some(m) = a.opt("--qp-multi") {
        prepared.game_args.extend(["--quickPlayMultiplayer".into(), m]);
    }
    if let Some(hp) = a.opt("--legacy-server") {
        let (h, p) = hp.split_once(':').expect("host:porta");
        prepared.game_args.extend(["--server".into(), h.into(), "--port".into(), p.into()]);
    }
    write_options(&req.instance_dir, &a.all("--option"));
    if a.flag("--print-command") {
        println!("== jvm_args (fim): {:?}", &prepared.jvm_args[prepared.jvm_args.len().saturating_sub(4)..]);
        let ga: Vec<String> = prepared.game_args.iter().scan(false, |hide, x| {
            let out = if *hide { "<omitido>".to_string() } else { x.clone() };
            *hide = matches!(x.as_str(), "--uuid" | "--accessToken" | "--session");
            Some(out)
        }).collect();
        println!("== game_args: {}", ga.join(" "));
    }

    // Servidor local, se pedido: sobe antes e espera o "Done".
    let mut server = None;
    if let Some(dir) = a.opt("--server-dir") {
        let spec = ServerSpec {
            dir: PathBuf::from(dir),
            java: console_java(&a.opt("--server-java").map(PathBuf::from).unwrap_or_else(|| prepared.java.clone())),
            props: a.all("--server-prop"),
        };
        match start_server(&spec, &tag, Duration::from_secs(600)) {
            Ok((srv, seen)) => {
                println!("== servidor pronto em {:.1}s", seen.at("server-done").unwrap().as_secs_f64());
                server = Some((srv, seen));
            }
            Err(e) => {
                println!("!! {e}");
                return ExitCode::from(1);
            }
        }
    }

    let capture = root.join("runs").join(format!("{tag}-{}.log", stamp()));
    let mut game = match RunningProcess::spawn(prepared.command(), &capture, false) {
        Ok(g) => g,
        Err(e) => {
            println!("!! falha ao iniciar o jogo: {e}");
            return ExitCode::from(1);
        }
    };
    println!("== jogo iniciado (pid {}), saída em {}", game.pid(), capture.display());

    let ready = markers::ready_set(mc, loader_kind);
    let mut seen = Seen::default();
    let mut qp_file_at: Option<Duration> = None;
    let mut fatal: Option<String> = None;
    let mut ready_at: Option<Duration> = None;
    let mut world_at: Option<(Duration, &'static str)> = None;
    // Maior silêncio no log (entre duas linhas seguidas) e momento da última linha.
    let mut last_line = Duration::ZERO;
    let mut max_gap = (Duration::ZERO, Duration::ZERO);
    let outcome: String = loop {
        if game.elapsed() > timeout {
            break "tempo-esgotado".into();
        }
        let mut got = false;
        while let Some(line) = game.try_next_line() {
            got = true;
            if line.at - last_line > max_gap.0 {
                max_gap = (line.at - last_line, last_line);
            }
            last_line = line.at;
            seen.feed("cli", &line);
            if fatal.is_none() {
                if let Some(f) = markers::is_fatal(&line.text) {
                    println!("[{:>7.1}s cli] FATAL {f}", line.at.as_secs_f64());
                    fatal = Some(f.to_string());
                }
            }
        }
        if let Some((srv, sseen)) = server.as_mut() {
            while let Some(line) = srv.try_next_line() {
                got = true;
                // Tempo do servidor convertido para o relógio do cliente.
                let at = (srv.started() + line.at).saturating_duration_since(game.started());
                sseen.feed("srv", &LogLine { at, ..line });
            }
        }
        if ready_at.is_none() && ready.iter().all(|r| seen.has(r)) {
            let at = ready.iter().filter_map(|r| seen.at(r)).max().unwrap();
            println!("== CLIENTE PRONTO (menu) em {:.1}s", at.as_secs_f64());
            ready_at = Some(at);
        }
        if let Some(p) = &qp_path {
            if qp_file_at.is_none() && req.instance_dir.join(p).exists() {
                qp_file_at = Some(game.elapsed());
                println!("[{:>7.1}s] arquivo do quickPlayPath criado: {}", game.elapsed().as_secs_f64(),
                    std::fs::read_to_string(req.instance_dir.join(p)).unwrap_or_default().trim());
            }
        }
        if world_at.is_none() {
            let srv_seen = server.as_ref().map(|(_, s)| s);
            for id in markers::WORLD_ANY {
                let at = seen.at(id).or_else(|| srv_seen.and_then(|s| s.at(id)));
                if let Some(at) = at {
                    println!("== ENTROU NO MUNDO ({id}) em {:.1}s", at.as_secs_f64());
                    world_at = Some((at, id));
                    break;
                }
            }
        }
        match goal.as_str() {
            "menu" if ready_at.is_some() => break "pronto".into(),
            "world" if world_at.is_some() => break "no-mundo".into(),
            _ => {}
        }
        if fatal.is_some() && goal != "exit" {
            break "fatal".into();
        }
        if let Some(st) = game.try_exit_status() {
            // Drena o resto da saída antes de concluir.
            std::thread::sleep(Duration::from_millis(500));
            while let Some(line) = game.try_next_line() {
                seen.feed("cli", &line);
            }
            break format!("saiu ({st})");
        }
        if !got {
            std::thread::sleep(Duration::from_millis(50));
        }
    };
    println!("== resultado: {outcome} em {:.1}s", game.elapsed().as_secs_f64());

    // Estabilidade: segue lendo por `hold` segundos.
    let until = Instant::now() + hold;
    let mut unstable = None;
    while Instant::now() < until && !outcome.starts_with("saiu") {
        if let Ok(line) = game.next_line(Duration::from_millis(200)) {
            seen.feed("cli", &line);
            if let Some(f) = markers::is_fatal(&line.text) {
                unstable = Some(f);
            }
        }
        if let Some((srv, sseen)) = server.as_mut() {
            while let Some(line) = srv.try_next_line() {
                let at = (srv.started() + line.at).saturating_duration_since(game.started());
                sseen.feed("srv", &LogLine { at, ..line });
            }
        }
        if let Some(p) = &qp_path {
            if qp_file_at.is_none() && req.instance_dir.join(p).exists() {
                qp_file_at = Some(game.elapsed());
                println!("[{:>7.1}s] arquivo do quickPlayPath criado: {}", game.elapsed().as_secs_f64(),
                    std::fs::read_to_string(req.instance_dir.join(p)).unwrap_or_default().trim());
            }
        }
        if game.try_exit_status().is_some() {
            unstable = Some("processo saiu sozinho");
            break;
        }
    }
    if let Some(u) = unstable {
        println!("!! instável depois do resultado: {u}");
    }
    #[cfg(windows)]
    let procs = game.processes_in_job();
    #[cfg(not(windows))]
    let procs = 0;
    println!("== processos no Job Object antes de encerrar: {procs}");
    let report = game.stop(None, Duration::ZERO, |l| {
        seen.feed("cli", l);
    });
    println!("== jogo encerrado: {:?} em {:.1}s", report.status.map(|s| s.to_string()), report.took.as_secs_f64());
    let mut srv_json = String::from("null");
    if let Some((srv, mut sseen)) = server.take() {
        let report = srv.stop(Some("stop"), Duration::from_secs(60), |l| {
            sseen.feed("srv", l);
        });
        println!("== servidor encerrado: {:?} forçado={} em {:.1}s", report.status.map(|s| s.to_string()), report.forced, report.took.as_secs_f64());
        srv_json = sseen.json();
    }

    let line = format!(
        "{{\"tag\":\"{tag}\",\"mc\":\"{mc}\",\"loader\":\"{loader_arg}\",\"goal\":\"{goal}\",\"outcome\":\"{outcome}\",\"ready\":{},\"world\":{},\"qp_file\":{},\"fatal\":{},\"procs_in_job\":{procs},\"last_line\":{:.1},\"max_gap\":{:.1},\"max_gap_after\":{:.1},\"client\":{},\"server\":{srv_json},\"capture\":\"{}\"}}",
        opt_secs(ready_at),
        opt_secs(world_at.map(|w| w.0)),
        opt_secs(qp_file_at),
        fatal.map(|f| format!("\"{}\"", f.replace('"', "'"))).unwrap_or("null".into()),
        last_line.as_secs_f64(),
        max_gap.0.as_secs_f64(),
        max_gap.1.as_secs_f64(),
        seen.json(),
        capture.file_name().unwrap().to_string_lossy()
    );
    println!("== resumo {line}");
    use std::io::Write as _;
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(root.join("results.jsonl")) {
        let _ = writeln!(f, "{line}");
    }
    let ok = match goal.as_str() {
        "menu" => outcome == "pronto",
        "world" => outcome == "no-mundo",
        _ => true,
    };
    if ok && unstable.is_none() { ExitCode::SUCCESS } else { ExitCode::from(1) }
}

fn opt_secs(d: Option<Duration>) -> String {
    d.map(|d| format!("{:.1}", d.as_secs_f64())).unwrap_or_else(|| "null".into())
}
