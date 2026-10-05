//! Downloads reais (`cargo xtask test-network`; QUALITY §4.1): retomada com `Range` nos CDNs
//! do Modrinth e da CurseForge. No CDN da CurseForge, `edge.forgecdn.net` responde 404 a um
//! `GET` com `Range` (R7 §9): a retomada só funciona porque o cliente resolve o
//! redirecionamento antes, sem `Range`. Sem chave: o CDN não a exige hoje.

// Nos auxiliares dos testes, `unwrap`, `expect` e `panic!` são a forma idiomática de falhar
// (o `clippy.toml` só os libera dentro das funções `#[test]`).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::fs;

use warden_core::NoProgress;
use warden_http::{DownloadRequest, HttpClient, HttpConfig, parse_url, part_path};
use warden_packwiz::HashFormat;

/// Baixa inteiro, depois recomeça de um `.part` com a metade e confere que a retomada usou
/// `Range` e chegou aos mesmos hashes.
async fn resume_check(url: &str, expected_host: &str) {
    let dir = tempfile::tempdir().unwrap();
    let client = HttpClient::new(HttpConfig::default()).unwrap();
    let url = parse_url(url).unwrap();

    let whole = dir.path().join("inteiro.jar");
    let full = client
        .download(
            &DownloadRequest::new(url.clone(), &whole),
            &NoProgress,
            None,
        )
        .await
        .unwrap();
    assert_eq!(full.final_url.host_str(), Some(expected_host));
    let bytes = fs::read(&whole).unwrap();
    assert!(bytes.len() > 100_000);

    let resumed_path = dir.path().join("retomado.jar");
    let half = bytes.len() / 2;
    fs::write(part_path(&resumed_path), &bytes[..half]).unwrap();
    let request = DownloadRequest::new(url, &resumed_path)
        .expect_hash(HashFormat::Sha512, &full.hashes.sha512)
        .expect_hash(HashFormat::Murmur2, full.hashes.murmur2.to_string())
        .expect_size(full.size);
    let resumed = client.download(&request, &NoProgress, None).await.unwrap();
    assert_eq!(resumed.resumed_from, half as u64);
    assert_eq!(resumed.hashes, full.hashes);
    assert_eq!(fs::read(&resumed_path).unwrap(), bytes);
}

#[tokio::test]
#[ignore = "rede"]
async fn rede_retomada_no_cdn_do_modrinth() {
    resume_check(
        "https://cdn.modrinth.com/data/AANobbMI/versions/SMxNOGZ6/sodium-fabric-0.8.13%2Bmc1.21.1.jar",
        "cdn.modrinth.com",
    )
    .await;
}

#[tokio::test]
#[ignore = "rede"]
async fn rede_retomada_no_cdn_da_curseforge_sem_range_no_edge() {
    resume_check(
        "https://edge.forgecdn.net/files/5101/366/jei-1.20.1-forge-15.3.0.4.jar",
        "mediafilez.forgecdn.net",
    )
    .await;
}
