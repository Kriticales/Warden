//! CA-T19-06, conformidade: o pacote pelo link descompactado numa pasta vazia, com o pack
//! publicado num servidor local que imita o GitHub (`raw.githubusercontent.com`): o próprio
//! script do pacote (`start.sh` no Linux e no macOS, `start.bat` no Windows) roda o
//! packwiz-installer-bootstrap real, instala só os mods do lado servidor, baixa o "Fabric" (o
//! servidor simulado da `warden-server`, servido como `fabric-server-launch.jar`), pergunta a
//! EULA e o servidor chega a "Done" depois do "s". Rodar de novo não pergunta a EULA nem baixa o
//! loader outra vez.

use std::fs;
use std::io::{BufRead as _, BufReader, Write as _};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use sha2::Sha256;
use warden_core::CancellationToken;
use warden_export::{
    ExportFormat, ExportSource, JavaRange, LoaderDownload, ServerPackOptions, ServerPackRequest,
    ServerPackVariant, export, export_server_pack,
};
use warden_instance::player_install::LocalPackServer;
use warden_packwiz::{Metafile, PackManifest, Side};
use warden_packwiz_cli::{Packwiz, RunContext};
use zip::write::SimpleFileOptions;

use super::{externals, hex_of, read_zip, sidecar, write};

const SERVIDOR: &[u8] = b"MOD-DOS-DOIS-LADOS-DO-LINK";
const SO_SERVIDOR: &[u8] = b"MOD-SO-DO-SERVIDOR-DO-LINK";
const CLIENTE: &[u8] = b"MOD-SO-DO-CLIENTE-DO-LINK";

/// Quanto o script tem para chegar a "Done" (o bootstrap e o installer reais abrem o Java).
const DONE_TIMEOUT: Duration = Duration::from_mins(4);

/// O servidor simulado da `warden-server` num jar executável.
fn fake_server_jar() -> Vec<u8> {
    let classes =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../warden-server/tests/fixtures/java");
    let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let options = SimpleFileOptions::default();
    zip.start_file("META-INF/MANIFEST.MF", options).unwrap();
    zip.write_all(b"Manifest-Version: 1.0\r\nMain-Class: WardenFakeServer\r\n\r\n")
        .unwrap();
    for class in ["WardenFakeServer.class", "WardenFakeServer$1.class"] {
        zip.start_file(class, options).unwrap();
        zip.write_all(&fs::read(classes.join(class)).unwrap())
            .unwrap();
    }
    zip.finish().unwrap().into_inner()
}

fn free_port() -> u16 {
    std::net::TcpListener::bind(("127.0.0.1", 0))
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

fn unzip(archive: &Path, into: &Path) {
    for (name, bytes) in read_zip(archive) {
        write(into, &name, &bytes);
    }
}

/// O script do pacote, com o Java do teste no `JAVA_HOME` e sem proxy (tudo é `127.0.0.1`).
fn start_script(dir: &Path, java: &Path) -> Child {
    let java_home = java.parent().and_then(Path::parent).unwrap();
    let mut command = if cfg!(windows) {
        let mut command = Command::new("cmd");
        command.args(["/d", "/c", "start.bat"]);
        command
    } else {
        let mut command = Command::new("sh");
        command.arg("start.sh");
        command
    };
    command
        .current_dir(dir)
        .env("JAVA_HOME", java_home)
        .env_remove("JAVA_TOOL_OPTIONS")
        .env_remove("_JAVA_OPTIONS");
    for proxy in [
        "http_proxy",
        "HTTP_PROXY",
        "https_proxy",
        "HTTPS_PROXY",
        "all_proxy",
        "ALL_PROXY",
    ] {
        command.env_remove(proxy);
    }
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap()
}

/// Roda o script: responde "s" à EULA, espera "Done", manda `stop` e espera o fim. Devolve a
/// saída inteira.
fn run_until_done(dir: &Path, java: &Path) -> String {
    let mut child = start_script(dir, java);
    let mut stdin = child.stdin.take().unwrap();
    stdin.write_all(b"s\n").unwrap();
    stdin.flush().unwrap();
    let (sender, receiver) = mpsc::channel::<String>();
    let readers: Vec<_> = [
        Box::new(child.stdout.take().unwrap()) as Box<dyn std::io::Read + Send>,
        Box::new(child.stderr.take().unwrap()),
    ]
    .into_iter()
    .map(|stream| {
        let sender = sender.clone();
        std::thread::spawn(move || {
            for line in BufReader::new(stream).split(b'\n') {
                let Ok(line) = line else { break };
                let text = String::from_utf8_lossy(&line).trim_end().to_owned();
                if sender.send(text).is_err() {
                    break;
                }
            }
        })
    })
    .collect();
    drop(sender);
    let mut output = String::new();
    let deadline = Instant::now() + DONE_TIMEOUT;
    let mut done = false;
    while Instant::now() < deadline {
        match receiver.recv_timeout(Duration::from_millis(200)) {
            Ok(line) => {
                output.push_str(&line);
                output.push('\n');
                if line.contains("]: Done (") {
                    done = true;
                    break;
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                if child.try_wait().unwrap().is_some() {
                    break;
                }
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
    if !done {
        let _ = child.kill();
        let _ = child.wait();
        for line in receiver.try_iter() {
            output.push_str(&line);
            output.push('\n');
        }
        panic!("o servidor não chegou a Done:\n{output}");
    }
    stdin.write_all(b"stop\n").unwrap();
    stdin.flush().unwrap();
    drop(stdin);
    let stop_deadline = Instant::now() + Duration::from_mins(1);
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if Instant::now() > stop_deadline {
            let _ = child.kill();
            panic!("o servidor não fechou com stop:\n{output}");
        }
        std::thread::sleep(Duration::from_millis(100));
    };
    for reader in readers {
        let _ = reader.join();
    }
    for line in receiver.try_iter() {
        output.push_str(&line);
        output.push('\n');
    }
    assert!(
        status.success(),
        "o script terminou com {status}:\n{output}"
    );
    output
}

#[tokio::test(flavor = "multi_thread")]
async fn ca_t19_06_conformidade_o_script_instala_os_mods_do_servidor_e_chega_a_done() {
    let Some(binary) = sidecar() else { return };
    let Some((java, jars)) = externals() else {
        return;
    };
    let temp_dir = tempfile::tempdir().unwrap();
    let temp = temp_dir.path();
    let github = temp.join("github");
    fs::create_dir_all(&github).unwrap();
    let served = LocalPackServer::start(github.clone()).await.unwrap();
    let file_url = |name: &str| served.link(&format!("arquivos/{name}")).unwrap();
    for (name, bytes) in [
        ("servidor.jar", SERVIDOR),
        ("so-servidor.jar", SO_SERVIDOR),
        ("cliente.jar", CLIENTE),
    ] {
        write(&github, &format!("arquivos/{name}"), bytes);
    }
    write(
        &github,
        "arquivos/fabric-server-launch.jar",
        &fake_server_jar(),
    );

    // O pack: um mod dos dois lados, um só do servidor e um só do cliente, por link.
    let root = temp.join("pack");
    let mut manifest = PackManifest::new("Vale Sereno", "1.20.1");
    "1.0.0".clone_into(&mut manifest.version);
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
    for (meta_path, name, file, bytes, side) in [
        (
            "mods/servidor.pw.toml",
            "Servidor",
            "servidor.jar",
            SERVIDOR,
            Side::Both,
        ),
        (
            "mods/so-servidor.pw.toml",
            "Só servidor",
            "so-servidor.jar",
            SO_SERVIDOR,
            Side::Server,
        ),
        (
            "mods/cliente.pw.toml",
            "Minimapa",
            "cliente.jar",
            CLIENTE,
            Side::Client,
        ),
    ] {
        let mut meta =
            Metafile::url(name, file_url(file).as_str(), &hex_of::<Sha256>(bytes)).unwrap();
        meta.side = side;
        write(&root, meta_path, meta.to_toml_string().as_bytes());
    }
    write(&root, "config/a.txt", b"ajuste do pack");
    write(&root, "kubejs/client_scripts/c.js", b"// cliente");
    let port = free_port();
    let properties = format!("server-ip=127.0.0.1\nserver-port={port}\nmotd=Vale Sereno\n");
    write(
        &root,
        "server-overrides/server.properties",
        properties.as_bytes(),
    );
    let cli = Packwiz::new(binary, temp.join("cache"), temp.join("config.toml"));
    let cancel = CancellationToken::new();
    cli.refresh(&root, false, RunContext::new(&cancel))
        .await
        .unwrap();

    // "Publicar": a exportação nativa vira a ponta da `main` do repositório simulado.
    let published = github.join("dono/vale-sereno/main");
    fs::create_dir_all(published.parent().unwrap()).unwrap();
    export(
        &root,
        &ExportSource::Current,
        ExportFormat::Folder,
        &published,
        &temp.join("staging"),
        &cli,
        &cancel,
    )
    .await
    .unwrap();
    let link = served.link("dono/vale-sereno/main/pack.toml").unwrap();

    let destination = temp.join("vale-sereno-servidor.zip");
    let loader = LoaderDownload::Fabric {
        url: file_url("fabric-server-launch.jar"),
    };
    let options = ServerPackOptions {
        variant: ServerPackVariant::Link,
        memory_mb: 1024,
        confirm_third_party: false,
    };
    export_server_pack(
        &ServerPackRequest {
            root: &root,
            source: &ExportSource::Current,
            destination: &destination,
            staging_parent: &temp.join("staging"),
            packwiz: &cli,
            options: &options,
            loader: &loader,
            java: JavaRange { min: 8, max: None },
            link: Some(&link),
            installer: Some(&jars),
            sources: None,
            manual: None,
        },
        &warden_core::NoProgress,
        &cancel,
    )
    .await
    .unwrap();

    // Numa pasta vazia, como quem recebe o zip.
    let dir: PathBuf = temp.join("servidor");
    fs::create_dir_all(&dir).unwrap();
    unzip(&destination, &dir);
    let java_for_script = java.clone();
    let script_dir = dir.clone();
    let output = tokio::task::spawn_blocking(move || run_until_done(&script_dir, &java_for_script))
        .await
        .unwrap();
    assert!(output.contains("aceita a EULA do Minecraft?"), "{output}");
    assert!(output.contains("fabric-server-launch.jar"), "{output}");

    assert_eq!(fs::read(dir.join("mods/servidor.jar")).unwrap(), SERVIDOR);
    assert_eq!(
        fs::read(dir.join("mods/so-servidor.jar")).unwrap(),
        SO_SERVIDOR
    );
    assert!(
        !dir.join("mods/cliente.jar").exists(),
        "o mod só de cliente não vai para o servidor"
    );
    assert_eq!(
        fs::read(dir.join("config/a.txt")).unwrap(),
        b"ajuste do pack"
    );
    assert_eq!(
        fs::read_to_string(dir.join("server.properties")).unwrap(),
        properties
    );
    let eula = fs::read_to_string(dir.join("eula.txt")).unwrap();
    assert!(eula.lines().any(|line| line == "eula=true"), "{eula}");
    assert!(dir.join("packwiz.json").is_file());
    assert_eq!(
        fs::read_to_string(dir.join(".warden-loader"))
            .unwrap()
            .trim_end(),
        "fabric 1.20.1 0.16.5"
    );

    // De novo: sem a pergunta da EULA e sem baixar o loader.
    let java_for_script = java.clone();
    let script_dir = dir.clone();
    let again = tokio::task::spawn_blocking(move || run_until_done(&script_dir, &java_for_script))
        .await
        .unwrap();
    assert!(!again.contains("aceita a EULA do Minecraft?"), "{again}");
    assert!(
        !again.contains("Baixando fabric-server-launch.jar"),
        "{again}"
    );
}
