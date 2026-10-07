//! Teste contra a CDN real da CurseForge (`cargo xtask test-network`; QUALITY §4.1): baixa por
//! HTTPS, sem `Range`, 3 arquivos pela URL montada pelo Warden e confere o `sha1` da API
//! (ADR-0050, ADR-0051). A chave vem de `CURSEFORGE_API_KEY`; nada é gravado fora de uma pasta
//! temporária.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use sha1::{Digest as _, Sha1};
use warden_core::NoProgress;
use warden_curseforge::{CurseforgeClient, SecretString};
use warden_export::curseforge_file_url;
use warden_http::{DownloadRequest, HttpClient, HttpConfig};

/// JEI, Fabric API e Cloth Config: mods conhecidos que permitem distribuição por terceiros.
const PROJECTS: [u64; 3] = [238_222, 306_612, 348_521];

#[tokio::test]
#[ignore = "rede"]
async fn rede_e04_cdn_responde_pela_url_montada_e_o_sha1_confere() {
    let key = std::env::var("CURSEFORGE_API_KEY")
        .expect("CURSEFORGE_API_KEY ausente: rode pelo `cargo xtask test-network`");
    let http = HttpClient::new(HttpConfig::default()).unwrap();
    let client = CurseforgeClient::new(http.clone(), Some(SecretString::from(key))).unwrap();
    let temp = tempfile::tempdir().unwrap();
    for project in PROJECTS {
        let project = client.project(project, None).await.unwrap();
        let file = project
            .latest_files
            .iter()
            .find(|f| f.download_url().is_some() && f.sha1().is_some() && f.file_length < 5_000_000)
            .unwrap_or_else(|| panic!("{} sem arquivo baixável", project.name));
        let url = curseforge_file_url(u32::try_from(file.id).unwrap(), &file.file_name).unwrap();
        assert_eq!(url.host_str(), Some("edge.forgecdn.net"));
        let destination = temp.path().join(&file.file_name);
        let mut request = DownloadRequest::new(url.clone(), &destination);
        request.headers = client.download_headers(&url).unwrap();
        let downloaded = http.download(&request, &NoProgress, None).await.unwrap();
        let bytes = std::fs::read(&destination).unwrap();
        assert_eq!(downloaded.size, file.file_length);
        assert_eq!(
            hex::encode(Sha1::digest(&bytes)),
            file.sha1().unwrap().to_lowercase()
        );
    }
}
