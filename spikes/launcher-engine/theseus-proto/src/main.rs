//! Spike S1 do Warden — protótipo com o app-lib (theseus) do Modrinth App.
//!
//! Uso: `theseus-proto <caso> [--timeout <s>]`
//!
//! O theseus não tem conta offline nem expõe o módulo `launcher`: o lançamento só
//! acontece por `instance::run`, que exige uma credencial Microsoft ativa no banco
//! SQLite interno. Para testar mesmo assim, este protótipo grava diretamente nesse
//! banco uma "credencial" falsa com token `0` e validade longa (para o theseus não
//! tentar renová-la). Isso é um contorno do spike, não algo aceitável em produção.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use theseus::data::{InstanceLink, ModLoader};
use theseus::install::{self, InstallJobStatus};
use theseus::instance::{self, QuickPlayType};

struct Case {
    id: &'static str,
    mc: &'static str,
    loader: ModLoader,
    loader_version: Option<&'static str>,
    ready: &'static [&'static str],
}

const CASES: &[Case] = &[
    Case {
        id: "fabric-1.20.1",
        mc: "1.20.1",
        loader: ModLoader::Fabric,
        loader_version: Some("stable"),
        ready: &["Loading Minecraft 1.20.1 with Fabric Loader", "Created: 1024x512x4 minecraft:textures/atlas/blocks.png-atlas", "Sound engine started"],
    },
    Case {
        id: "forge-1.12.2-2860",
        mc: "1.12.2",
        loader: ModLoader::Forge,
        // O theseus usa o id do meta do Modrinth (sem o prefixo "1.12.2-"). Um id que
        // não existe cai silenciosamente para a versão mais recente (visto no spike).
        loader_version: Some("14.23.5.2860"),
        ready: &["Forge Mod Loader has successfully loaded", "textures-atlas", "Sound engine started"],
    },
    Case {
        id: "neoforge-1.21.1",
        mc: "1.21.1",
        loader: ModLoader::NeoForge,
        loader_version: Some("21.1.252"),
        ready: &["Launching target 'forgeclient'", "Created: 1024x512x4 minecraft:textures/atlas/blocks.png-atlas", "Sound engine started"],
    },
    Case {
        id: "forge-1.7.10",
        mc: "1.7.10",
        loader: ModLoader::Forge,
        loader_version: Some("10.13.4.1614"),
        ready: &["Forge Mod Loader has successfully loaded", "textures/blocks-atlas", "Sound engine started"],
    },
];

const FATAL: &[&str] = &["---- Minecraft Crash Report ----", "Exception in thread \"main\"", "Could not find or load main class"];

type AnyResult<T> = Result<T, Box<dyn std::error::Error>>;

fn data_root() -> PathBuf {
    std::env::var_os("WARDEN_SPIKE_THESEUS_DATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(std::env::var_os("HOME").expect("HOME")).join(".local/share/warden-spike/theseus"))
}

/// Grava uma credencial falsa (perfil offline) direto no banco do theseus.
async fn inject_offline_credential(db: &Path, player: &str) -> AnyResult<()> {
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(SqliteConnectOptions::new().filename(db))
        .await?;
    // UUID offline como o jogo calcula: MD5 de "OfflinePlayer:<nome>", versão 3.
    let uuid = offline_uuid(player);
    let expires = (chrono::Utc::now() + chrono::Duration::days(3650)).timestamp();
    sqlx::query("UPDATE minecraft_users SET active = FALSE").execute(&pool).await?;
    sqlx::query(
        "INSERT INTO minecraft_users (uuid, active, username, access_token, refresh_token, expires)
         VALUES (?1, TRUE, ?2, '0', '', ?3)
         ON CONFLICT (uuid) DO UPDATE SET active = TRUE, username = ?2, access_token = '0', expires = ?3",
    )
    .bind(uuid.as_hyphenated().to_string())
    .bind(player)
    .bind(expires)
    .execute(&pool)
    .await?;
    pool.close().await;
    Ok(())
}

fn offline_uuid(player: &str) -> uuid::Uuid {
    // `Uuid::new_v3` prefixa um namespace; o Java não usa namespace, então
    // calculamos o MD5 cru e marcamos versão 3 / variante RFC 4122 pelo Builder.
    let digest = md5_digest(format!("OfflinePlayer:{player}").as_bytes());
    uuid::Builder::from_md5_bytes(digest).into_uuid()
}

/// MD5 (RFC 1321) compacto, só para o spike não depender de outra crate.
fn md5_digest(input: &[u8]) -> [u8; 16] {
    const S: [u32; 64] = [
        7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 5, 9, 14, 20, 5, 9, 14, 20, 5, 9, 14, 20, 5, 9, 14, 20,
        4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 6, 10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21,
    ];
    let k: Vec<u32> = (0..64).map(|i| ((i as f64 + 1.0).sin().abs() * 4294967296.0) as u32).collect();
    let mut msg = input.to_vec();
    let bit_len = (input.len() as u64).wrapping_mul(8);
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&bit_len.to_le_bytes());
    let (mut a0, mut b0, mut c0, mut d0) = (0x67452301u32, 0xefcdab89u32, 0x98badcfeu32, 0x10325476u32);
    for chunk in msg.chunks(64) {
        let m: Vec<u32> = chunk.chunks(4).map(|w| u32::from_le_bytes([w[0], w[1], w[2], w[3]])).collect();
        let (mut a, mut b, mut c, mut d) = (a0, b0, c0, d0);
        for i in 0..64 {
            let (f, g) = match i / 16 {
                0 => ((b & c) | (!b & d), i),
                1 => ((d & b) | (!d & c), (5 * i + 1) % 16),
                2 => (b ^ c ^ d, (3 * i + 5) % 16),
                _ => (c ^ (b | !d), (7 * i) % 16),
            };
            let f = f.wrapping_add(a).wrapping_add(k[i]).wrapping_add(m[g]);
            a = d;
            d = c;
            c = b;
            b = b.wrapping_add(f.rotate_left(S[i]));
        }
        a0 = a0.wrapping_add(a);
        b0 = b0.wrapping_add(b);
        c0 = c0.wrapping_add(c);
        d0 = d0.wrapping_add(d);
    }
    let mut out = [0u8; 16];
    for (i, v) in [a0, b0, c0, d0].iter().enumerate() {
        out[i * 4..i * 4 + 4].copy_from_slice(&v.to_le_bytes());
    }
    out
}

#[tokio::main]
async fn main() -> AnyResult<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let case_id = args.first().ok_or("uso: theseus-proto <caso>")?;
    let case = CASES.iter().find(|c| c.id == case_id).ok_or("caso desconhecido")?;
    let timeout = Duration::from_secs(
        args.iter().position(|a| a == "--timeout").and_then(|i| args.get(i + 1)).and_then(|s| s.parse().ok()).unwrap_or(900),
    );

    let root = data_root();
    std::fs::create_dir_all(&root)?;
    // A única forma de escolher a pasta de dados do theseus é por variável de ambiente
    // (lida dentro de State::init).
    unsafe { std::env::set_var("THESEUS_CONFIG_DIR", &root) };

    let t0 = Instant::now();
    theseus::EventState::init().await?;
    theseus::State::init("warden-spike".to_string()).await?;
    println!("[{:>7.1}s] estado inicializado em {}", t0.elapsed().as_secs_f64(), root.display());

    inject_offline_credential(&root.join("app.db"), "WardenTest").await?;
    println!("[{:>7.1}s] credencial offline injetada", t0.elapsed().as_secs_f64());

    // Reaproveita a instância se já existir (mesmo nome).
    let existing = instance::list().await?.into_iter().find(|i| i.instance.name == case.id);
    let instance_id = match existing {
        Some(inst) => {
            println!("[{:>7.1}s] instância existente {}", t0.elapsed().as_secs_f64(), inst.instance.id);
            inst.instance.id
        }
        None => {
            let snap = install::create_instance(
                case.id.to_string(),
                case.mc.to_string(),
                case.loader,
                case.loader_version.map(str::to_string),
                None,
                None,
                InstanceLink::Unmanaged,
            )
            .await?;
            let job_id: uuid::Uuid = snap.job_id.parse()?;
            println!("[{:>7.1}s] job de instalação {job_id}", t0.elapsed().as_secs_f64());
            let mut last = String::new();
            let snap = loop {
                let s = install::get_job(job_id).await?;
                let line = format!(
                    "status={:?} fase={:?} progresso={:?}",
                    s.status,
                    s.phase,
                    s.progress.as_ref().map(|p| (p.current, p.total))
                );
                if line != last {
                    println!("[{:>7.1}s] {line}", t0.elapsed().as_secs_f64());
                    last = line;
                }
                if !matches!(s.status, InstallJobStatus::Queued | InstallJobStatus::Running) {
                    break s;
                }
                tokio::time::sleep(Duration::from_millis(500)).await;
            };
            if !matches!(snap.status, InstallJobStatus::Succeeded) {
                println!("!! instalação terminou com {:?}: {:?}", snap.status, snap.error);
                return Err("instalação falhou".into());
            }
            snap.instance_id.ok_or("job sem instance_id")?
        }
    };
    println!("== instalação concluída em {:.1}s", t0.elapsed().as_secs_f64());

    let path = instance::get_full_path(&instance_id).await?;
    let log = path.join("logs").join("launcher_log.txt");
    let proc_meta = instance::run(&instance_id, QuickPlayType::None).await?;
    let started = Instant::now();
    println!("== jogo iniciado (processo {}), log capturado pelo theseus em {}", proc_meta.uuid, log.display());

    let mut seen = vec![false; case.ready.len()];
    let mut offset = 0usize;
    let ok = loop {
        if started.elapsed() > timeout {
            println!("!! tempo esgotado");
            break false;
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
        let Ok(bytes) = std::fs::read(&log) else { continue };
        if bytes.len() <= offset {
            continue;
        }
        let text = String::from_utf8_lossy(&bytes[offset..]).to_string();
        offset = bytes.len();
        let mut fatal = None;
        for line in text.lines() {
            for (i, m) in case.ready.iter().enumerate() {
                if !seen[i] && line.contains(m) {
                    seen[i] = true;
                    println!("[{:>7.1}s] marcador: {m}", started.elapsed().as_secs_f64());
                }
            }
            if fatal.is_none() && FATAL.iter().any(|f| line.contains(f)) && !line.contains("SplashProgress") {
                fatal = Some(line.to_string());
            }
        }
        if let Some(f) = fatal {
            println!("!! erro fatal: {f}");
            break false;
        }
        if seen.iter().all(|s| *s) {
            println!("== CARREGOU em {:.1}s após o run()", started.elapsed().as_secs_f64());
            tokio::time::sleep(Duration::from_secs(10)).await;
            break true;
        }
    };

    let t_kill = Instant::now();
    theseus::process::kill(proc_meta.uuid).await?;
    let waited = tokio::time::timeout(Duration::from_secs(30), theseus::process::wait_for(proc_meta.uuid)).await;
    println!("== kill() + wait_for(): {:?} em {:.1}s", waited.map(|r| r.map_err(|e| e.to_string())), t_kill.elapsed().as_secs_f64());
    if ok { Ok(()) } else { Err("não carregou".into()) }
}
