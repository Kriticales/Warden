//! CA-T19-04 e CA-T19-05 com o packwiz real: `.mrpack` e zip da CurseForge de um pack com mod
//! do Modrinth, da CurseForge, de link direto e local.
//!
//! O Modrinth e a CurseForge são servidores simulados (wiremock). O packwiz baixa os jars de
//! um servidor local e, para os links de `cdn.modrinth.com`, usa o cache de downloads que a
//! própria exportação preenche antes (o mesmo cache que o Warden usa): nenhum teste sai para a
//! internet.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#![allow(linker_messages)]

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use secrecy::SecretString;
use serde_json::{Value, json};
use sha1::Sha1;
use sha2::{Digest as _, Sha256, Sha512};
use warden_core::{CancellationToken, DomainCode, DomainError as _};
use warden_curseforge::CurseforgeClient;
use warden_export::{
    ExportErrorCode, ExportSource, FormatChoices, FormatRequest, ItemOrigin, ItemOutcome,
    LauncherFormat, Lookups, MrpackRules, analyze_format, export_format, read_mrpack,
    validate_curseforge, validate_mrpack, CurseforgeRules,
};
use warden_http::{HttpClient, HttpConfig, ManualTimer};
use warden_modrinth::ModrinthClient;
use warden_packwiz::{
    CurseForgeFile, HashFormat, Loader, Metafile, ModrinthFile, PackManifest, Side,
};
use warden_packwiz_cli::{Packwiz, RunContext};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const CDN_JAR: &[u8] = b"jar do mod hospedado no cdn do modrinth";
const SWAP_JAR: &[u8] = b"jar do mod da curseforge que existe no modrinth";
const LINK_JAR: &[u8] = b"jar de um link direto de outro site";
const LOCAL_JAR: &[u8] = b"jar local do usuario";

fn hex(bytes: &[u8]) -> String {
    bytes.iter().fold(String::new(), |mut text, byte| {
        let _ = write!(text, "{byte:02x}");
        text
    })
}

fn sha1(bytes: &[u8]) -> String {
    hex(&Sha1::digest(bytes))
}

fn sha256(bytes: &[u8]) -> String {
    hex(&Sha256::digest(bytes))
}

fn sha512(bytes: &[u8]) -> String {
    hex(&Sha512::digest(bytes))
}

fn sidecar() -> Option<PathBuf> {
    let name = if cfg!(windows) {
        "packwiz-x86_64-pc-windows-msvc.exe"
    } else {
        "packwiz-x86_64-unknown-linux-gnu"
    };
    let path = std::env::var_os("WARDEN_PACKWIZ_BIN").map_or_else(
        || {
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../apps/desktop/src-tauri/binaries")
                .join(name)
        },
        PathBuf::from,
    );
    if path.is_file() {
        Some(path)
    } else {
        assert_ne!(
            std::env::var("WARDEN_REQUIRE_EXTERNALS").ok().as_deref(),
            Some("1"),
            "WARDEN_REQUIRE_EXTERNALS=1, mas o packwiz não está em {}",
            path.display()
        );
        eprintln!("AVISO: packwiz não encontrado; teste de formatos pulado.");
        None
    }
}

fn write(root: &Path, relative: &str, bytes: impl AsRef<[u8]>) {
    let path = root.join(relative);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, bytes).unwrap();
}

fn json_ok(value: Value) -> ResponseTemplate {
    ResponseTemplate::new(200)
        .insert_header("content-type", "application/json")
        .set_body_json(value)
}

struct Fixture {
    _temp: tempfile::TempDir,
    root: PathBuf,
    staging: PathBuf,
    packwiz: Packwiz,
    modrinth: ModrinthClient,
    curseforge: CurseforgeClient,
    _servers: Vec<MockServer>,
}

fn http() -> HttpClient {
    HttpClient::with_timer(HttpConfig::for_version("1"), Arc::new(ManualTimer::new())).unwrap()
}

/// O pack de teste e os servidores simulados. `curseforge_file` é a resposta de
/// `POST /v1/mods/files` para o mod 100/200; `modrinth_match` diz se o Modrinth conhece o jar.
async fn fixture(
    binary: &Path,
    version: &str,
    modrinth_match: bool,
    cf_file_url: Option<&str>,
) -> Fixture {
    let temp = tempfile::tempdir().unwrap();
    let packwiz = Packwiz::new(
        binary.to_path_buf(),
        temp.path().join("dados/cache/packwiz"),
        temp.path().join("dados/packwiz/packwiz.toml"),
    );
    let cancel = CancellationToken::new();

    // Servidor local dos jars baixados pelo packwiz.
    let files = MockServer::start().await;
    for (name, bytes) in [
        ("cdn.jar", CDN_JAR),
        ("swap.jar", SWAP_JAR),
        ("link.jar", LINK_JAR),
    ] {
        Mock::given(method("GET"))
            .and(path(format!("/files/{name}")))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(bytes.to_vec()))
            .mount(&files)
            .await;
    }

    // Preenche o cache de downloads do packwiz com os jars que o pack aponta para o CDN do
    // Modrinth (e para a troca), sem sair da máquina.
    let seed = temp.path().join("semente");
    write(&seed, "pack.toml", seed_manifest().to_toml_string());
    write(&seed, "index.toml", "hash-format = \"sha256\"\n");
    for (name, bytes) in [("cdn.jar", CDN_JAR), ("swap.jar", SWAP_JAR)] {
        let mut meta = Metafile::url(
            name,
            &format!("{}/files/{name}", files.uri()),
            &sha256(bytes),
        )
        .unwrap();
        meta.side = Side::Both;
        write(
            &seed,
            &format!("mods/{name}.pw.toml"),
            meta.to_toml_string(),
        );
    }
    packwiz
        .refresh(&seed, false, RunContext::new(&cancel))
        .await
        .unwrap();
    packwiz
        .modrinth_export(
            &seed,
            &temp.path().join("semente.mrpack"),
            None,
            RunContext::new(&cancel),
        )
        .await
        .unwrap();

    // O pack de teste.
    let root = temp.path().join("pack");
    let mut manifest = seed_manifest();
    manifest.version = version.to_owned();
    write(&root, "pack.toml", manifest.to_toml_string());
    write(&root, "index.toml", "hash-format = \"sha256\"\n");
    let mut cdn = Metafile::modrinth(
        &ModrinthFile {
            title: "Mod do CDN".into(),
            project_id: "CDNPRJ".into(),
            version_id: "CDNVER".into(),
            filename: "cdn.jar".into(),
            url: "https://cdn.modrinth.com/data/CDNPRJ/versions/CDNVER/cdn.jar".into(),
            hash_format: HashFormat::Sha512,
            hash: sha512(CDN_JAR),
        },
        Side::Client,
    );
    cdn.option = None;
    write(&root, "mods/cdn.pw.toml", cdn.to_toml_string());
    let swap = Metafile::curseforge(
        &CurseForgeFile {
            name: "Mod da CurseForge".into(),
            project_id: 100,
            file_id: 200,
            filename: "swap.jar".into(),
            hash_format: HashFormat::Sha1,
            hash: sha1(SWAP_JAR),
        },
        Side::Both,
    );
    write(&root, "mods/swap.pw.toml", swap.to_toml_string());
    let mut link = Metafile::url(
        "Mod de link",
        &format!("{}/files/link.jar", files.uri()),
        &sha256(LINK_JAR),
    )
    .unwrap();
    link.side = Side::Both;
    write(&root, "mods/link.pw.toml", link.to_toml_string());
    write(&root, "mods/local.jar", LOCAL_JAR);
    write(&root, "config/a.txt", "ajuste");
    packwiz
        .refresh(&root, false, RunContext::new(&cancel))
        .await
        .unwrap();

    // Modrinth simulado: conhece o jar da troca pelo hash (ou nenhum).
    let modrinth_server = MockServer::start().await;
    let versions = if modrinth_match {
        json!({ sha1(SWAP_JAR): {
            "id": "SWAPVER", "project_id": "SWAPPRJ", "version_number": "1.0.0",
            "version_type": "release",
            "files": [{
                "hashes": {"sha1": sha1(SWAP_JAR), "sha512": sha512(SWAP_JAR)},
                "url": "https://cdn.modrinth.com/data/SWAPPRJ/versions/SWAPVER/swap.jar",
                "filename": "swap.jar", "primary": true, "size": SWAP_JAR.len()
            }]
        }})
    } else {
        json!({})
    };
    Mock::given(method("POST"))
        .and(path("/v2/version_files"))
        .respond_with(json_ok(versions))
        .mount(&modrinth_server)
        .await;
    let modrinth =
        ModrinthClient::with_base_url(http(), &format!("{}/v2", modrinth_server.uri()), None)
            .unwrap();

    // CurseForge simulada.
    let cf_server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/mods/files"))
        .respond_with(json_ok(json!({"data": [cf_file(cf_file_url)]})))
        .mount(&cf_server)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/mods"))
        .respond_with(json_ok(json!({"data": [cf_project()]})))
        .mount(&cf_server)
        .await;
    let curseforge = CurseforgeClient::with_base_url(
        http(),
        &format!("{}/v1", cf_server.uri()),
        Some(SecretString::from("chave-de-teste")),
    )
    .unwrap();

    Fixture {
        staging: temp.path().join("staging"),
        _temp: temp,
        root,
        packwiz,
        modrinth,
        curseforge,
        _servers: vec![files, modrinth_server, cf_server],
    }
}

fn seed_manifest() -> PackManifest {
    let mut manifest = PackManifest::new("Pack de teste", "1.21.1");
    manifest.set_loader_version(Loader::Fabric, "0.16.0");
    manifest
}

fn cf_file(download_url: Option<&str>) -> Value {
    json!({
        "id": 200, "gameId": 432, "modId": 100, "isAvailable": true,
        "displayName": "swap.jar", "fileName": "swap.jar", "releaseType": 1, "fileStatus": 4,
        "hashes": [{"value": sha1(SWAP_JAR), "algo": 1}],
        "fileLength": SWAP_JAR.len(), "downloadUrl": download_url,
        "gameVersions": ["1.21.1", "Fabric"]
    })
}

fn cf_project() -> Value {
    json!({
        "id": 100, "gameId": 432, "name": "Mod da CurseForge", "slug": "mod-cf",
        "links": {"websiteUrl": "https://www.curseforge.com/minecraft/mc-mods/mod-cf"},
        "allowModDistribution": false, "mainFileId": 200
    })
}

impl Fixture {
    fn lookups(&self) -> Lookups<'_> {
        Lookups {
            modrinth: &self.modrinth,
            curseforge: Some(&self.curseforge),
        }
    }

    async fn analyze(&self, format: LauncherFormat) -> warden_export::Result<warden_export::FormatAnalysis> {
        analyze_format(
            &self.root,
            &ExportSource::Current,
            format,
            &self.lookups(),
            &CancellationToken::new(),
        )
        .await
    }

    async fn export(
        &self,
        format: LauncherFormat,
        choices: &FormatChoices,
        destination: &Path,
    ) -> warden_export::Result<warden_export::FormatExportResult> {
        export_format(
            &FormatRequest {
                root: &self.root,
                source: &ExportSource::Current,
                format,
                choices,
                destination,
                staging_parent: &self.staging,
                packwiz: &self.packwiz,
                key: None,
                lookups: self.lookups(),
            },
            &CancellationToken::new(),
        )
        .await
    }

    fn pack_snapshot(&self) -> BTreeMap<String, Vec<u8>> {
        let mut map = BTreeMap::new();
        let mut stack = vec![self.root.clone()];
        while let Some(dir) = stack.pop() {
            for entry in fs::read_dir(dir).unwrap() {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    stack.push(path);
                } else {
                    let name = path
                        .strip_prefix(&self.root)
                        .unwrap()
                        .to_string_lossy()
                        .replace('\\', "/");
                    map.insert(name, fs::read(path).unwrap());
                }
            }
        }
        map
    }
}

fn code(error: &warden_export::Error) -> ExportErrorCode {
    match error.code() {
        DomainCode::Domain(code) => code,
        DomainCode::Core(core) => panic!("código do núcleo: {core:?}"),
    }
}

fn confirmed(swap: &[&str]) -> FormatChoices {
    FormatChoices {
        swap: swap.iter().map(|path| (*path).to_owned()).collect(),
        confirm_embed: true,
        version: None,
    }
}

#[tokio::test]
async fn mrpack_com_modrinth_curseforge_link_e_local() {
    let Some(binary) = sidecar() else { return };
    let f = fixture(&binary, "1.0.0", true, Some("https://edge.forgecdn.net/files/0/200/swap.jar")).await;

    let analysis = f.analyze(LauncherFormat::Mrpack).await.unwrap();
    assert_eq!(analysis.version.as_deref(), Some("1.0.0"));
    assert_eq!(analysis.references, 1, "{analysis:#?}");
    let kinds: Vec<_> = analysis
        .items
        .iter()
        .map(|item| (item.path.as_str(), item.origin, &item.outcome))
        .collect();
    assert_eq!(kinds.len(), 3, "{kinds:#?}");
    assert!(matches!(
        kinds[0],
        ("mods/link.pw.toml", ItemOrigin::Link, ItemOutcome::Embed)
    ));
    assert!(matches!(
        kinds[1],
        ("mods/local.jar", ItemOrigin::Local, ItemOutcome::Embed)
    ));
    assert!(matches!(
        kinds[2],
        ("mods/swap.pw.toml", ItemOrigin::Curseforge, ItemOutcome::Swap { required: false, .. })
    ));
    assert_eq!(analysis.embedded, 2);
    assert!(!analysis.has_blockers());
    // O formato guarda o lado por `env`: nada de "lado" nas perdas.
    assert!(analysis.losses.iter().all(|loss| {
        !matches!(loss.kind, warden_export::LossKind::SideNotKept)
    }));

    let before = f.pack_snapshot();
    let out = f._temp.path().join("saida.mrpack");

    // Sem trocar e sem confirmar: o jar da CurseForge, o de link e o local iriam dentro.
    let error = f.export(LauncherFormat::Mrpack, &FormatChoices::default(), &out).await.unwrap_err();
    assert_eq!(code(&error), ExportErrorCode::EmbedNeedsConfirmation);
    assert!(!out.exists());
    // Trocando, ainda faltam o link e o jar local.
    let error = f
        .export(LauncherFormat::Mrpack, &FormatChoices { swap: vec!["mods/swap.pw.toml".into()], ..FormatChoices::default() }, &out)
        .await
        .unwrap_err();
    assert_eq!(code(&error), ExportErrorCode::EmbedNeedsConfirmation);
    // Troca pedida para um arquivo que não tem equivalente.
    let error = f.export(LauncherFormat::Mrpack, &confirmed(&["mods/cdn.pw.toml"]), &out).await.unwrap_err();
    assert_eq!(code(&error), ExportErrorCode::SwapNotAvailable);

    let result = f.export(LauncherFormat::Mrpack, &confirmed(&["mods/swap.pw.toml"]), &out).await.unwrap();
    assert_eq!(result.swapped, 1);
    assert_eq!(result.validation.references, 2);
    assert_eq!(result.validation.overrides, 3); // config + jar de link + jar local
    assert!(out.is_file());

    let archive = read_mrpack(&out).unwrap();
    assert_eq!(archive.index.version_id, "1.0.0");
    assert_eq!(archive.index.dependencies["minecraft"], "1.21.1");
    assert_eq!(archive.index.dependencies["fabric-loader"], "0.16.0");
    let by_path: BTreeMap<_, _> = archive.index.files.iter().map(|file| (file.path.as_str(), file)).collect();
    let cdn = by_path["mods/cdn.jar"];
    assert_eq!(cdn.downloads, ["https://cdn.modrinth.com/data/CDNPRJ/versions/CDNVER/cdn.jar"]);
    assert_eq!(cdn.hashes["sha1"], sha1(CDN_JAR));
    assert_eq!(cdn.hashes["sha512"], sha512(CDN_JAR));
    assert_eq!(cdn.file_size as usize, CDN_JAR.len());
    let env = cdn.env.as_ref().unwrap();
    assert_eq!((env.client.as_str(), env.server.as_str()), ("required", "unsupported"));
    // A troca: o jar da CurseForge vai pelo endereço do Modrinth, não dentro do arquivo.
    let swapped = by_path["mods/swap.jar"];
    assert_eq!(swapped.downloads, ["https://cdn.modrinth.com/data/SWAPPRJ/versions/SWAPVER/swap.jar"]);
    assert_eq!(swapped.hashes["sha1"], sha1(SWAP_JAR));
    let mut zip = zip::ZipArchive::new(fs::File::open(&out).unwrap()).unwrap();
    assert!(zip.by_name("overrides/mods/swap.jar").is_err());
    let mut link = Vec::new();
    std::io::Read::read_to_end(&mut zip.by_name("overrides/mods/link.jar").unwrap(), &mut link).unwrap();
    assert_eq!(link, LINK_JAR);
    assert!(zip.by_name("overrides/mods/local.jar").is_ok());
    assert!(zip.by_name("overrides/config/a.txt").is_ok());
    assert_eq!(
        result.validation.embedded_jars,
        ["overrides/mods/link.jar", "overrides/mods/local.jar"]
    );

    // O pack no Warden não mudou e o destino não é sobrescrito.
    assert_eq!(f.pack_snapshot(), before);
    let error = f.export(LauncherFormat::Mrpack, &confirmed(&["mods/swap.pw.toml"]), &out).await.unwrap_err();
    assert_eq!(code(&error), ExportErrorCode::DestinationNotEmpty);

    // O validador reprova o mesmo arquivo se a pessoa não tivesse confirmado o jar local.
    let error = validate_mrpack(
        &out,
        &MrpackRules {
            strict: true,
            allowed_overrides: None,
            allowed_jars: Some(["mods/link.jar".to_owned()].into()),
        },
    )
    .unwrap_err();
    assert_eq!(code(&error), ExportErrorCode::InvalidOutput);
}

#[tokio::test]
async fn zip_da_curseforge_com_mods_de_fora_so_com_confirmacao() {
    let Some(binary) = sidecar() else { return };
    let f = fixture(&binary, "1.0.0", true, Some("https://edge.forgecdn.net/files/0/200/swap.jar")).await;

    let analysis = f.analyze(LauncherFormat::Curseforge).await.unwrap();
    assert_eq!(analysis.references, 1);
    // O mod do CDN do Modrinth, o de link e o jar local iriam dentro do zip.
    let paths: Vec<_> = analysis.items.iter().map(|item| item.path.as_str()).collect();
    assert_eq!(paths, ["mods/cdn.pw.toml", "mods/link.pw.toml", "mods/local.jar"]);
    assert!(analysis.items.iter().all(|item| item.outcome == ItemOutcome::Embed));
    assert!(analysis.losses.iter().any(|loss| {
        matches!(loss.kind, warden_export::LossKind::SideNotKept) && loss.count == 1
    }));

    let out = f._temp.path().join("saida.zip");
    let error = f.export(LauncherFormat::Curseforge, &FormatChoices::default(), &out).await.unwrap_err();
    assert_eq!(code(&error), ExportErrorCode::EmbedNeedsConfirmation);
    assert!(!out.exists());

    let result = f.export(LauncherFormat::Curseforge, &confirmed(&[]), &out).await.unwrap();
    assert_eq!(result.validation.references, 1);
    assert_eq!(
        result.validation.embedded_jars,
        ["overrides/mods/cdn.jar", "overrides/mods/link.jar", "overrides/mods/local.jar"]
    );
    let mut zip = zip::ZipArchive::new(fs::File::open(&out).unwrap()).unwrap();
    let manifest: Value = serde_json::from_reader(zip.by_name("manifest.json").unwrap()).unwrap();
    assert_eq!(manifest["files"][0]["projectID"], 100);
    assert_eq!(manifest["files"][0]["fileID"], 200);
    assert_eq!(manifest["version"], "1.0.0");
    assert!(zip.by_name("overrides/config/a.txt").is_ok());
    // O jar da CurseForge vai por referência, nunca dentro.
    assert!(zip.by_name("overrides/mods/swap.jar").is_err());

    // CA-T19-05: nenhum jar de terceiros sem confirmação explícita.
    let error = validate_curseforge(
        &out,
        &CurseforgeRules {
            allowed_overrides: None,
            allowed_jars: Some(std::collections::BTreeSet::new()),
        },
    )
    .unwrap_err();
    assert_eq!(code(&error), ExportErrorCode::InvalidOutput);
}

#[tokio::test]
async fn mod_bloqueado_para_terceiros_sem_equivalente_impede_o_mrpack() {
    let Some(binary) = sidecar() else { return };
    // Sem equivalente no Modrinth e sem `downloadUrl`: o autor bloqueou apps de terceiros.
    let f = fixture(&binary, "1.0.0", false, None).await;
    let analysis = f.analyze(LauncherFormat::Mrpack).await.unwrap();
    assert!(analysis.has_blockers());
    let blocked = analysis.items.iter().find(|item| item.path == "mods/swap.pw.toml").unwrap();
    assert_eq!(
        blocked.outcome,
        ItemOutcome::Blocked {
            page_url: Some("https://www.curseforge.com/minecraft/mc-mods/mod-cf/files/200".into())
        }
    );
    let out = f._temp.path().join("saida.mrpack");
    let error = f.export(LauncherFormat::Mrpack, &confirmed(&[]), &out).await.unwrap_err();
    assert_eq!(code(&error), ExportErrorCode::BlockedByAuthor);
    assert_eq!(error.params()["files"], "Mod da CurseForge");
    assert!(!out.exists());
    // No zip da CurseForge o mesmo mod vai por referência: sem bloqueio.
    let zip = f.analyze(LauncherFormat::Curseforge).await.unwrap();
    assert!(!zip.has_blockers());
}

#[tokio::test]
async fn troca_e_obrigatoria_quando_a_curseforge_bloqueia_e_o_modrinth_tem() {
    let Some(binary) = sidecar() else { return };
    let f = fixture(&binary, "1.0.0", true, None).await;
    let analysis = f.analyze(LauncherFormat::Mrpack).await.unwrap();
    let swap = analysis.items.iter().find(|item| item.path == "mods/swap.pw.toml").unwrap();
    assert!(matches!(swap.outcome, ItemOutcome::Swap { required: true, .. }));
    // Sem aceitar a troca, o mod não pode ir embutido.
    let out = f._temp.path().join("saida.mrpack");
    let error = f.export(LauncherFormat::Mrpack, &confirmed(&[]), &out).await.unwrap_err();
    assert_eq!(code(&error), ExportErrorCode::BlockedByAuthor);
}

#[tokio::test]
async fn sem_chave_da_curseforge_so_a_troca_pelo_modrinth_e_possivel() {
    let Some(binary) = sidecar() else { return };
    let f = fixture(&binary, "1.0.0", true, Some("https://edge.forgecdn.net/files/0/200/swap.jar")).await;
    let lookups = Lookups { modrinth: &f.modrinth, curseforge: None };
    let analysis = analyze_format(&f.root, &ExportSource::Current, LauncherFormat::Mrpack, &lookups, &CancellationToken::new())
        .await
        .unwrap();
    assert!(analysis.items.iter().any(|item| matches!(item.outcome, ItemOutcome::Swap { required: false, .. })));
    // Sem a troca e sem a chave, não há como saber se a CurseForge libera o arquivo.
    let out = f._temp.path().join("saida.mrpack");
    let error = export_format(
        &FormatRequest {
            root: &f.root,
            source: &ExportSource::Current,
            format: LauncherFormat::Mrpack,
            choices: &confirmed(&[]),
            destination: &out,
            staging_parent: &f.staging,
            packwiz: &f.packwiz,
            key: None,
            lookups,
        },
        &CancellationToken::new(),
    )
    .await
    .unwrap_err();
    assert_eq!(code(&error), ExportErrorCode::CurseforgeKeyMissing);
}

#[tokio::test]
async fn version_e_exigida_e_informada_so_no_arquivo_gerado() {
    let Some(binary) = sidecar() else { return };
    let f = fixture(&binary, "", true, Some("https://edge.forgecdn.net/files/0/200/swap.jar")).await;
    let analysis = f.analyze(LauncherFormat::Mrpack).await.unwrap();
    assert_eq!(analysis.version, None);
    let out = f._temp.path().join("saida.mrpack");
    let mut choices = confirmed(&["mods/swap.pw.toml"]);
    let error = f.export(LauncherFormat::Mrpack, &choices, &out).await.unwrap_err();
    assert_eq!(code(&error), ExportErrorCode::VersionRequired);
    choices.version = Some("  2.5.0  ".into());
    let before = f.pack_snapshot();
    f.export(LauncherFormat::Mrpack, &choices, &out).await.unwrap();
    assert_eq!(read_mrpack(&out).unwrap().index.version_id, "2.5.0");
    assert_eq!(f.pack_snapshot(), before, "o pack.toml do Warden não pode mudar");
}
