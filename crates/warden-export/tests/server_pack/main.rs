//! CA-T19-06 e CA-T19-07: o pacote para servidor com o packwiz real, servidores simulados da
//! CurseForge e, na conformidade, o packwiz-installer-bootstrap real rodado pelo `start.sh`
//! (Linux) ou pelo `start.bat` (Windows) a partir de um servidor local que imita o GitHub.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::too_many_lines
)]
#![allow(linker_messages)]

mod conformidade;

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use secrecy::SecretString;
use serde_json::{Value, json};
use sha1::Sha1;
use sha2::Sha256;
use warden_core::{CancellationToken, NoProgress};
use warden_curseforge::CurseforgeClient;
use warden_export::{
    ExportErrorCode, ExportSource, JavaRange, LoaderDownload, ManualDownloads, ServerPackOptions,
    ServerPackRequest, ServerPackVariant, WaitFuture, analyze_server_pack, export_server_pack,
};
use warden_http::{ExpectedHash, HttpClient, HttpConfig, Url};
use warden_instance::{BlockedFile, DownloadCache, FileOrigin, InstallerJars, Sources};
use warden_packwiz::{CurseForgeFile, HashFormat, Metafile, PackManifest, Side};
use warden_packwiz_cli::{Packwiz, RunContext};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

pub(crate) const KEY: &str = "$2a$10$chaveFalsaDoWardenParaTestes0123456789abcdefghijkl";
const SERVIDOR: &[u8] = b"MOD-DOS-DOIS-LADOS";
const SO_SERVIDOR: &[u8] = b"MOD-SO-DO-SERVIDOR";
const CLIENTE: &[u8] = b"MOD-SO-DO-CLIENTE";
const OPCIONAL: &[u8] = b"MOD-OPCIONAL";
const BLOQUEADO: &[u8] = b"MOD-BLOQUEADO-DA-CURSEFORGE";
const RESOURCE_PACK: &[u8] = b"RESOURCE-PACK";

pub(crate) fn hex_of<D: sha2::digest::Digest>(bytes: &[u8]) -> String {
    hex::encode(D::digest(bytes))
}

pub(crate) fn sidecar() -> Option<PathBuf> {
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
            Some("1")
        );
        None
    }
}

/// O Java e os jars fixados do `cargo xtask installer` (os mesmos do app).
pub(crate) fn externals() -> Option<(PathBuf, InstallerJars)> {
    let found = std::env::var_os("WARDEN_INSTALLER_CACHE")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("LOCALAPPDATA")
                .map(|local| PathBuf::from(local).join("Warden-dev/cache/installer"))
        })
        .and_then(|dir| fs::read(dir.join("externals.json")).ok())
        .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
        .and_then(|value| {
            let get = |key: &str| value[key].as_str().map(PathBuf::from);
            Some((
                get("java")?,
                InstallerJars {
                    bootstrap: get("bootstrap")?,
                    installer: get("installer")?,
                },
            ))
        })
        .filter(|(java, jars)| {
            java.is_file() && jars.bootstrap.is_file() && jars.installer.is_file()
        });
    if found.is_none() {
        assert_ne!(
            std::env::var("WARDEN_REQUIRE_EXTERNALS").ok().as_deref(),
            Some("1"),
            "WARDEN_REQUIRE_EXTERNALS=1, mas faltam o Java e os jars do installer"
        );
    }
    found
}

pub(crate) fn write(root: &Path, relative: &str, bytes: &[u8]) {
    let target = root.join(relative);
    fs::create_dir_all(target.parent().unwrap()).unwrap();
    fs::write(target, bytes).unwrap();
}

/// Um pack Fabric 1.20.1 com mods dos dois lados, só de servidor, só de cliente, opcional, um
/// da CurseForge bloqueado para terceiros, um resource pack, um jar local e os arquivos que
/// ficam de fora (`options.txt`, `kubejs/client_scripts/`).
struct World {
    _temp: tempfile::TempDir,
    root: PathBuf,
    temp: PathBuf,
    cli: Packwiz,
    _cdn: MockServer,
    sources: Sources,
}

fn url_mod(name: &str, url: &str, bytes: &[u8], side: Side, optional: bool) -> Metafile {
    let mut meta = Metafile::url(name, url, &hex_of::<Sha256>(bytes)).unwrap();
    meta.side = side;
    if optional {
        meta.option = Some(warden_packwiz::ModOption {
            optional: true,
            default: false,
            description: String::new(),
        });
    }
    meta
}

async fn world() -> Option<World> {
    let binary = sidecar()?;
    let temp_dir = tempfile::tempdir().unwrap();
    let temp = temp_dir.path().to_path_buf();
    let root = temp.join("pack");
    let mut manifest = PackManifest::new("Vale Sereno", "1.20.1");
    "1.4.2".clone_into(&mut manifest.version);
    manifest
        .versions
        .get_or_insert_default()
        .insert("fabric".to_owned(), "0.16.5".to_owned());
    write(&root, "pack.toml", manifest.to_toml_string().as_bytes());
    write(
        &root,
        "index.toml",
        b"hash-format = \"sha256\"\nfiles = []\n",
    );
    let metafiles = [
        (
            "mods/servidor.pw.toml",
            url_mod(
                "Servidor",
                "https://example.invalid/servidor.jar",
                SERVIDOR,
                Side::Both,
                false,
            ),
        ),
        (
            "mods/so-servidor.pw.toml",
            url_mod(
                "Só servidor",
                "https://example.invalid/so-servidor.jar",
                SO_SERVIDOR,
                Side::Server,
                false,
            ),
        ),
        (
            "mods/cliente.pw.toml",
            url_mod(
                "Minimapa",
                "https://example.invalid/cliente.jar",
                CLIENTE,
                Side::Client,
                false,
            ),
        ),
        (
            "mods/opcional.pw.toml",
            url_mod(
                "Opcional",
                "https://example.invalid/opcional.jar",
                OPCIONAL,
                Side::Both,
                true,
            ),
        ),
        (
            "resourcepacks/rp.pw.toml",
            url_mod(
                "Texturas",
                "https://example.invalid/rp.zip",
                RESOURCE_PACK,
                Side::Both,
                false,
            ),
        ),
        (
            "mods/bloqueado.pw.toml",
            Metafile::curseforge(
                &CurseForgeFile {
                    name: "Bloqueado".into(),
                    project_id: 448_233,
                    file_id: 5_000_001,
                    filename: "bloqueado.jar".into(),
                    hash_format: HashFormat::Sha1,
                    hash: hex_of::<Sha1>(BLOQUEADO),
                },
                Side::Both,
            ),
        ),
    ];
    for (file, meta) in metafiles {
        write(&root, file, meta.to_toml_string().as_bytes());
    }
    write(&root, "mods/local.jar", b"JAR-LOCAL");
    write(&root, "config/a.txt", b"do pack");
    write(&root, "kubejs/server_scripts/s.js", b"// servidor");
    write(&root, "kubejs/client_scripts/c.js", b"// cliente");
    write(&root, "options.txt", b"fov:90");
    write(&root, "server-overrides/config/a.txt", b"do servidor");
    write(&root, "server-overrides/eula.txt", b"eula=true\n");

    let cli = Packwiz::new(binary, temp.join("cache"), temp.join("config.toml"));
    cli.refresh(&root, false, RunContext::new(&CancellationToken::new()))
        .await
        .unwrap();

    let cdn = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/mods/files"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"data": [
            {"id": 5_000_001, "modId": 448_233, "fileName": "bloqueado.jar", "downloadUrl": null,
             "fileLength": BLOQUEADO.len(), "hashes": []},
        ]})))
        .mount(&cdn)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/mods"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"data": [
            {"id": 448_233, "links": {"websiteUrl": "https://www.curseforge.com/minecraft/mc-mods/bloqueado"},
             "allowModDistribution": false}
        ]})))
        .mount(&cdn)
        .await;
    let http = HttpClient::new(HttpConfig::for_version("1")).unwrap();
    let curseforge = CurseforgeClient::with_base_url(
        http.clone(),
        &format!("{}/v1", cdn.uri()),
        Some(SecretString::from(KEY.to_owned())),
    )
    .unwrap();
    let cache = Arc::new(DownloadCache::open(&temp.join("downloads")).unwrap());
    // O que os testes do pack já baixaram (os endereços `example.invalid` nunca são usados).
    for (name, bytes) in [
        ("servidor.jar", SERVIDOR),
        ("so-servidor.jar", SO_SERVIDOR),
        ("opcional.jar", OPCIONAL),
        ("rp.zip", RESOURCE_PACK),
    ] {
        let file = temp.join(name);
        fs::write(&file, bytes).unwrap();
        cache
            .import_file(
                &file,
                &ExpectedHash::new(HashFormat::Sha256, hex_of::<Sha256>(bytes)),
                &FileOrigin::url("https://example.invalid/"),
            )
            .unwrap()
            .unwrap();
    }
    Some(World {
        _temp: temp_dir,
        root,
        temp,
        cli,
        _cdn: cdn,
        sources: Sources {
            http,
            curseforge: Some(curseforge),
            cache,
        },
    })
}

/// O download manual da T20 simulado: a pessoa "baixa" cada arquivo pelo site e o Warden o
/// confere e guarda no cache.
struct Manual {
    cache: Arc<DownloadCache>,
    folder: PathBuf,
    calls: AtomicUsize,
    names: std::sync::Mutex<Vec<String>>,
}

impl ManualDownloads for Manual {
    fn wait(&self, blocked: Vec<BlockedFile>) -> WaitFuture<'_> {
        Box::pin(async move {
            self.calls.fetch_add(1, Ordering::Relaxed);
            for file in blocked {
                self.names.lock().unwrap().push(file.name.clone());
                let downloaded = self.folder.join(&file.file_name);
                fs::create_dir_all(&self.folder).unwrap();
                fs::write(&downloaded, BLOQUEADO).unwrap();
                warden_instance::accept_manual_file(&self.cache, &file, &downloaded).unwrap();
            }
            Ok(())
        })
    }
}

fn fabric_download() -> LoaderDownload {
    LoaderDownload::Fabric {
        url: Url::parse(
            "https://meta.fabricmc.net/v2/versions/loader/1.20.1/0.16.5/1.0.1/server/jar",
        )
        .unwrap(),
    }
}

fn options(variant: ServerPackVariant, confirm: bool) -> ServerPackOptions {
    ServerPackOptions {
        variant,
        memory_mb: 4096,
        confirm_third_party: confirm,
    }
}

fn embedded_request<'a>(
    w: &'a World,
    destination: &'a Path,
    options: &'a ServerPackOptions,
    loader: &'a LoaderDownload,
    staging: &'a Path,
    manual: Option<&'a dyn ManualDownloads>,
) -> ServerPackRequest<'a> {
    ServerPackRequest {
        root: &w.root,
        source: &ExportSource::Current,
        destination,
        staging_parent: staging,
        packwiz: &w.cli,
        options,
        loader,
        java: JavaRange { min: 17, max: None },
        link: None,
        installer: None,
        sources: Some(&w.sources),
        manual,
    }
}

pub(crate) fn code(error: &warden_export::Error) -> ExportErrorCode {
    use warden_core::{DomainCode, DomainError as _};
    match error.code() {
        DomainCode::Domain(code) => code,
        other @ DomainCode::Core(_) => panic!("código inesperado: {other:?}"),
    }
}

pub(crate) fn read_zip(path: &Path) -> BTreeMap<String, Vec<u8>> {
    let mut zip = zip::ZipArchive::new(fs::File::open(path).unwrap()).unwrap();
    (0..zip.len())
        .map(|i| {
            let mut file = zip.by_index(i).unwrap();
            let mut bytes = Vec::new();
            file.read_to_end(&mut bytes).unwrap();
            (file.name().unwrap().into_owned(), bytes)
        })
        .collect()
}

fn names(files: &BTreeMap<String, Vec<u8>>) -> BTreeSet<&str> {
    files.keys().map(String::as_str).collect()
}

#[tokio::test]
async fn analise_mostra_o_que_fica_de_fora_e_o_download_manual() {
    let Some(w) = world().await else { return };
    let analysis = analyze_server_pack(
        &w.root,
        &ExportSource::Current,
        Some(&w.sources),
        &CancellationToken::new(),
    )
    .await
    .unwrap();
    assert_eq!(analysis.minecraft, "1.20.1");
    assert_eq!(analysis.loader, "Fabric 0.16.5");
    assert!(analysis.loader_supported);
    assert_eq!(analysis.client_only, ["Minimapa", "Texturas"]);
    assert_eq!(analysis.client_only_count, 2);
    // options.txt e kubejs/client_scripts/c.js.
    assert_eq!(analysis.client_files, 2);
    assert!(analysis.kubejs_client_scripts);
    assert_eq!(analysis.server_overrides, 1);
    assert_eq!(analysis.manual_downloads, ["Bloqueado"]);
    assert!(!analysis.curseforge_key_missing);
    // servidor, só servidor, opcional e o bloqueado; mais o jar local.
    assert_eq!(analysis.embedded.server_items, 4);
    assert_eq!(analysis.embedded.third_party, 5);
    assert_eq!(analysis.link.third_party, 0);
    let folders = |contents: &warden_export::ServerPackContents| {
        contents
            .folders
            .iter()
            .map(|f| (f.path.clone(), f.files))
            .collect::<Vec<_>>()
    };
    assert_eq!(
        folders(&analysis.link),
        [
            ("config".to_owned(), 1),
            ("kubejs".to_owned(), 1),
            ("server-overrides".to_owned(), 1),
        ]
    );
    assert_eq!(
        folders(&analysis.embedded),
        [("config".to_owned(), 1), ("kubejs".to_owned(), 1)]
    );

    // Sem a chave, não dá para conferir a CurseForge.
    let no_key = Sources {
        curseforge: None,
        ..w.sources.clone()
    };
    let analysis = analyze_server_pack(
        &w.root,
        &ExportSource::Current,
        Some(&no_key),
        &CancellationToken::new(),
    )
    .await
    .unwrap();
    assert!(analysis.curseforge_key_missing);
    assert!(analysis.manual_downloads.is_empty());
}

#[tokio::test]
async fn ca_t19_07_com_mods_dentro_jars_conferidos_download_manual_e_licenca() {
    let Some(w) = world().await else { return };
    let manual = Manual {
        cache: Arc::clone(&w.sources.cache),
        folder: w.temp.join("Downloads"),
        calls: AtomicUsize::new(0),
        names: std::sync::Mutex::new(Vec::new()),
    };
    let loader = fabric_download();
    let staging = w.temp.join("staging");
    let cancel = CancellationToken::new();

    // Sem a confirmação de licença, nada é gerado (nem baixado).
    let destination = w.temp.join("sem-licenca.zip");
    let error = export_server_pack(
        &embedded_request(
            &w,
            &destination,
            &options(ServerPackVariant::Embedded, false),
            &loader,
            &staging,
            Some(&manual),
        ),
        &NoProgress,
        &cancel,
    )
    .await
    .unwrap_err();
    assert_eq!(code(&error), ExportErrorCode::ServerPackNeedsConfirmation);
    assert!(!destination.exists());
    assert_eq!(manual.calls.load(Ordering::Relaxed), 0);

    // Sem quem faça o download manual, o mod bloqueado impede gerar.
    let destination = w.temp.join("sem-manual.zip");
    let error = export_server_pack(
        &embedded_request(
            &w,
            &destination,
            &options(ServerPackVariant::Embedded, true),
            &loader,
            &staging,
            None,
        ),
        &NoProgress,
        &cancel,
    )
    .await
    .unwrap_err();
    assert_eq!(code(&error), ExportErrorCode::ServerPackManualDownloads);
    assert!(!destination.exists());

    // Com o download manual: abre antes de gerar, e o jar entra conferido.
    let destination = w.temp.join("vale-sereno-servidor.zip");
    let result = export_server_pack(
        &embedded_request(
            &w,
            &destination,
            &options(ServerPackVariant::Embedded, true),
            &loader,
            &staging,
            Some(&manual),
        ),
        &NoProgress,
        &cancel,
    )
    .await
    .unwrap();
    assert_eq!(manual.calls.load(Ordering::Relaxed), 1);
    assert_eq!(*manual.names.lock().unwrap(), ["Bloqueado"]);
    // Os 4 de referência e o jar local.
    assert_eq!(result.mods, 5);
    assert_eq!(result.variant, ServerPackVariant::Embedded);
    let files = read_zip(&destination);
    assert_eq!(
        names(&files),
        BTreeSet::from([
            "config/a.txt",
            "kubejs/server_scripts/s.js",
            "mods/bloqueado.jar",
            "mods/local.jar",
            "mods/opcional.jar",
            "mods/servidor.jar",
            "mods/so-servidor.jar",
            "start.bat",
            "start.sh",
            "user_jvm_args.txt",
        ])
    );
    // Cada jar com o hash do índice.
    for (path, bytes) in [
        ("mods/servidor.jar", SERVIDOR),
        ("mods/so-servidor.jar", SO_SERVIDOR),
        ("mods/opcional.jar", OPCIONAL),
        ("mods/bloqueado.jar", BLOQUEADO),
    ] {
        assert_eq!(files[path], bytes, "{path}");
    }
    // O server-overrides/ já aplicado; o eula.txt dele nunca vai.
    assert_eq!(files["config/a.txt"], b"do servidor");
    assert_eq!(result.files.len(), files.len());
    assert_eq!(
        result.sha256,
        hex_of::<Sha256>(&fs::read(&destination).unwrap())
    );
    let jvm = String::from_utf8(files["user_jvm_args.txt"].clone()).unwrap();
    assert!(jvm.contains("\n-Xms4G\n-Xmx4G\n"), "{jvm}");

    // O destino existente nunca é sobrescrito.
    let error = export_server_pack(
        &embedded_request(
            &w,
            &destination,
            &options(ServerPackVariant::Embedded, true),
            &loader,
            &staging,
            Some(&manual),
        ),
        &NoProgress,
        &cancel,
    )
    .await
    .unwrap_err();
    assert_eq!(code(&error), ExportErrorCode::DestinationNotEmpty);
}

#[tokio::test]
async fn ca_t19_06_pelo_link_leva_scripts_jars_fixados_e_configs_sem_cliente() {
    let Some(w) = world().await else { return };
    let link =
        Url::parse("https://raw.githubusercontent.com/dono/vale-sereno/main/pack.toml").unwrap();
    let loader = fabric_download();
    let opts = options(ServerPackVariant::Link, false);
    let staging = w.temp.join("staging");
    let cancel = CancellationToken::new();

    // Sem publicação, não há link.
    let destination = w.temp.join("sem-link.zip");
    let mut request = ServerPackRequest {
        root: &w.root,
        source: &ExportSource::Current,
        destination: &destination,
        staging_parent: &staging,
        packwiz: &w.cli,
        options: &opts,
        loader: &loader,
        java: JavaRange { min: 17, max: None },
        link: None,
        installer: None,
        sources: None,
        manual: None,
    };
    let error = export_server_pack(&request, &NoProgress, &cancel)
        .await
        .unwrap_err();
    assert_eq!(code(&error), ExportErrorCode::ServerPackNotPublished);

    // Um jar que não é o fixado é recusado.
    let fake = InstallerJars {
        bootstrap: w.temp.join("bootstrap-falso.jar"),
        installer: w.temp.join("installer-falso.jar"),
    };
    fs::write(&fake.bootstrap, b"falso").unwrap();
    fs::write(&fake.installer, b"falso").unwrap();
    request.link = Some(&link);
    request.installer = Some(&fake);
    let error = export_server_pack(&request, &NoProgress, &cancel)
        .await
        .unwrap_err();
    assert_eq!(code(&error), ExportErrorCode::ServerPackInvalidOutput);
    assert!(!destination.exists());

    let Some((_, jars)) = externals() else { return };
    request.installer = Some(&jars);
    let destination = w.temp.join("vale-sereno-servidor.zip");
    request.destination = &destination;
    let result = export_server_pack(&request, &NoProgress, &cancel)
        .await
        .unwrap();
    assert_eq!(result.mods, 0);
    let files = read_zip(&destination);
    assert_eq!(
        names(&files),
        BTreeSet::from([
            "LICENCAS-DE-TERCEIROS.txt",
            "config/a.txt",
            "kubejs/server_scripts/s.js",
            "packwiz-installer-bootstrap.jar",
            "packwiz-installer.jar",
            "server-overrides/config/a.txt",
            "start.bat",
            "start.sh",
            "user_jvm_args.txt",
        ])
    );
    assert_eq!(
        hex_of::<Sha256>(&files["packwiz-installer-bootstrap.jar"]),
        warden_instance::BOOTSTRAP.sha256
    );
    assert_eq!(
        hex_of::<Sha256>(&files["packwiz-installer.jar"]),
        warden_instance::INSTALLER.sha256
    );
    let sh = String::from_utf8(files["start.sh"].clone()).unwrap();
    assert!(sh.contains(link.as_str()));
    assert!(!sh.contains('\r'));
    let bat = &files["start.bat"];
    assert!(
        bat.windows(2)
            .filter(|w| w[1] == b'\n')
            .all(|w| w[0] == b'\r'),
        "start.bat sem CRLF"
    );
    // O start.sh sai executável do zip.
    let mut zip = zip::ZipArchive::new(fs::File::open(&destination).unwrap()).unwrap();
    assert_eq!(
        zip.by_name("start.sh").unwrap().unix_mode().unwrap() & 0o777,
        0o755
    );
    assert_eq!(
        result.sha256,
        hex_of::<Sha256>(&fs::read(&destination).unwrap())
    );
}
