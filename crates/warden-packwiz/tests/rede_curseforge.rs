//! CA-3 da P1-01: o murmur2 do Warden confere com as impressões digitais da CurseForge para
//! jars reais. Teste de rede (`cargo xtask test-network`): usa `CURSEFORGE_API_KEY` do
//! ambiente e o `curl` do sistema, com a chave passada pela entrada padrão (nunca em argumento
//! nem impressa).

// Nos auxiliares dos testes, `unwrap`, `expect` e `panic!` são a forma idiomática de falhar
// (o `clippy.toml` só os libera dentro das funções `#[test]`).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::fs::File;
use std::io::Write as _;
use std::path::Path;
use std::process::{Command, Stdio};

use warden_packwiz::hash::{HashFormat, curseforge_fingerprint_reader, hash_reader};

/// (projeto, arquivo): JEI 1.20.1 e 1.12.2, Jade, `JourneyMap` 1.7.10, AE2 (8 MB) e outro JEI.
const FILES: [(u32, u32); 6] = [
    (238_222, 9_009_995),
    (324_717, 8_591_528),
    (238_222, 8_963_414),
    (32_274, 8_923_608),
    (223_794, 7_148_487),
    (238_222, 9_058_038),
];

fn curl(args: &[&str], key: Option<&str>) -> Vec<u8> {
    let mut command = Command::new(if cfg!(windows) { "curl.exe" } else { "curl" });
    command
        .args([
            "-sSfL",
            "--max-time",
            "300",
            "-A",
            "Kriticales/Warden (testes)",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if key.is_some() {
        command.args(["-H", "@-"]);
    }
    command.args(args);
    let mut child = command.spawn().expect("curl indisponível");
    let mut stdin = child.stdin.take().unwrap();
    if let Some(key) = key {
        stdin
            .write_all(format!("x-api-key: {key}\n").as_bytes())
            .unwrap();
    }
    drop(stdin);
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "curl falhou: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    output.stdout
}

fn api(key: &str, path: &str) -> serde_json::Value {
    let url = format!("https://api.curseforge.com{path}");
    serde_json::from_slice(&curl(&[&url], Some(key))).unwrap()
}

#[test]
#[ignore = "rede"]
fn rede_murmur2_confere_com_a_curseforge() {
    let key = std::env::var("CURSEFORGE_API_KEY").expect("CURSEFORGE_API_KEY ausente");
    let temp = tempfile::tempdir().unwrap();
    let mut fingerprints = Vec::new();
    for (project, file) in FILES {
        let data = &api(&key, &format!("/v1/mods/{project}/files/{file}"))["data"];
        let expected = data["fileFingerprint"].as_u64().unwrap();
        let url = data["downloadUrl"]
            .as_str()
            .expect("arquivo sem link de download");
        let jar = temp.path().join(format!("{file}.jar"));
        let jar_text = jar.to_string_lossy().into_owned();
        curl(&["-o", &jar_text, url], None);

        let fingerprint = curseforge_fingerprint_reader(&mut File::open(&jar).unwrap()).unwrap();
        assert_eq!(u64::from(fingerprint), expected, "{project}/{file}");
        let sha1 = data["hashes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|hash| hash["algo"].as_i64() == Some(1))
            .and_then(|hash| hash["value"].as_str())
            .unwrap();
        assert_eq!(
            hash_reader(HashFormat::Sha1, &mut File::open(&jar).unwrap()).unwrap(),
            sha1
        );
        assert_eq!(
            hash_reader(HashFormat::Murmur2, &mut File::open(&jar).unwrap()).unwrap(),
            expected.to_string()
        );
        fingerprints.push((file, fingerprint, jar));
    }

    // A CurseForge reconhece cada arquivo pela impressão digital que o Warden calculou.
    let body = serde_json::json!({
        "fingerprints": fingerprints.iter().map(|(_, fp, _)| fp).collect::<Vec<_>>()
    })
    .to_string();
    let body_path = temp.path().join("corpo.json");
    std::fs::write(&body_path, &body).unwrap();
    let body_arg = format!("@{}", body_path.display());
    let response: serde_json::Value = serde_json::from_slice(&curl(
        &[
            "-X",
            "POST",
            "-H",
            "Content-Type: application/json",
            "--data-binary",
            &body_arg,
            "https://api.curseforge.com/v1/fingerprints/432",
        ],
        Some(&key),
    ))
    .unwrap();
    let matched: Vec<u64> = response["data"]["exactMatches"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|item| item["file"]["id"].as_u64())
        .collect();
    for (file, _, jar) in &fingerprints {
        assert!(
            matched.contains(&u64::from(*file)),
            "{file} não reconhecido"
        );
        assert!(Path::new(jar).is_file());
    }
}
