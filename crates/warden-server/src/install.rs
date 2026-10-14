//! Instalação do servidor do loader certo (ARCHITECTURE §7.6; ADR-0032; R5B §4.2).
//!
//! Por loader e faixa, sempre com a versão exata do `pack.toml`:
//! - **vanilla:** o `server.jar` do `downloads.server` do JSON da versão (conferido pelo
//!   `sha1`);
//! - **Fabric:** o mesmo `server.jar` mais o lançador de servidor do Fabric meta
//!   (`/v2/versions/loader/<mc>/<loader>/<instalador>/server/jar`), com
//!   `fabric-server-launcher.properties` apontando o `server.jar` (assim ele não baixa o
//!   vanilla na primeira abertura);
//! - **Forge e NeoForge:** o instalador oficial (coordenada Maven do catálogo, conferido pelo
//!   `.sha1` do Maven) com `--installServer` na pasta do servidor. Forge 1.17+ e NeoForge
//!   deixam `libraries/…/unix_args.txt`/`win_args.txt` (o Warden monta a linha com o
//!   `@arquivo`, sem o `run.bat`); Forge até a 1.16.5 deixa um jar executável (`forge-*.jar`,
//!   `-universal` no 1.7.10), aberto com o Java 8 que a política escolhe para essas versões.
//!
//! Os downloads ficam em cache (`<cache>/server/`, por versão), e a pasta do servidor guarda o
//! que foi instalado em [`MARKER_FILE`]: abrir de novo a mesma versão não baixa nem roda o
//! instalador (idempotente). O motor do launcher não instala servidor (R5B §4.2).

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::io::{AsyncRead, AsyncReadExt as _};
use warden_catalog::Catalog;
use warden_core::{CancellationToken, ProgressSink, atomic_write};
use warden_http::{DownloadRequest, HttpClient, Url};
use warden_launcher::{GameSpec, LoaderSpec};
use warden_packwiz::HashFormat;

use crate::error::{Error, Result, ServerSource};

/// O que foi instalado na pasta do servidor.
pub const MARKER_FILE: &str = ".warden-server.json";

/// O `server.jar` vanilla na pasta.
pub const VANILLA_JAR: &str = "server.jar";

/// O lançador de servidor do Fabric na pasta.
pub const FABRIC_JAR: &str = "fabric-server-launch.jar";

/// Quanto o instalador do Forge/NeoForge pode demorar (baixa as bibliotecas e roda os
/// processadores).
pub const INSTALLER_TIMEOUT: Duration = Duration::from_mins(20);

/// Quanto da saída do instalador vai para "Detalhes técnicos".
const INSTALLER_OUTPUT_TAIL: usize = 4096;

/// Maior jar aceito nos downloads do servidor.
const MAX_JAR_BYTES: u64 = 512 * 1024 * 1024;

/// `CREATE_NO_WINDOW`: o instalador não abre janela de console.
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Como o servidor abre.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ServerLaunch {
    /// `java -jar <jar> nogui` (vanilla, Fabric, Forge até a 1.16.5).
    Jar {
        /// O jar, relativo à pasta do servidor.
        jar: String,
    },
    /// `java @<arquivo> nogui` (Forge 1.17+ e NeoForge).
    ArgsFile {
        /// O arquivo de argumentos, relativo à pasta do servidor, com `/`.
        file: String,
    },
    /// Uma classe num diretório (o servidor simulado dos testes e do E2E).
    Class {
        /// O diretório com a classe.
        class_dir: PathBuf,
        /// A classe principal.
        main_class: String,
        /// Argumentos depois da classe.
        args: Vec<String>,
    },
}

/// Um servidor pronto para abrir.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledServer {
    /// A pasta do servidor (o diretório de trabalho do processo).
    pub dir: PathBuf,
    /// O Minecraft e o loader.
    pub spec: GameSpec,
    /// O `releaseTime` do JSON da versão vanilla (faixa do log4j).
    pub release_time: String,
    /// Como abrir.
    pub launch: ServerLaunch,
}

/// O que o [`MARKER_FILE`] guarda.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Marker {
    /// `forge 1.20.1-47.3.0` (o `Display` do `GameSpec`).
    spec: String,
    release_time: String,
    launch: ServerLaunch,
}

impl InstalledServer {
    /// O servidor já instalado nesta pasta para este jogo, se houver (sem rede).
    #[must_use]
    pub fn existing(dir: &Path, spec: &GameSpec) -> Option<Self> {
        let marker: Marker =
            serde_json::from_slice(&std::fs::read(dir.join(MARKER_FILE)).ok()?).ok()?;
        if marker.spec != spec.to_string() {
            return None;
        }
        let present = match &marker.launch {
            ServerLaunch::Jar { jar } => dir.join(jar).is_file(),
            ServerLaunch::ArgsFile { file } => dir.join(file).is_file(),
            ServerLaunch::Class { class_dir, .. } => class_dir.is_dir(),
        };
        present.then(|| Self {
            dir: dir.to_path_buf(),
            spec: spec.clone(),
            release_time: marker.release_time,
            launch: marker.launch,
        })
    }

    /// Grava o [`MARKER_FILE`] e a configuração do log4j.
    fn record(&self) -> Result<()> {
        let marker = Marker {
            spec: self.spec.to_string(),
            release_time: self.release_time.clone(),
            launch: self.launch.clone(),
        };
        let json = serde_json::to_vec_pretty(&marker)
            .map_err(|error| Error::Internal(format!("gravar o {MARKER_FILE}: {error}")))?;
        atomic_write(&self.dir.join(MARKER_FILE), &json)?;
        crate::log4j::write_config(&self.dir)
    }
}

/// Quem instala servidores: rede, catálogo (versões e coordenadas Maven) e o cache.
#[derive(Debug, Clone)]
pub struct Installer {
    http: HttpClient,
    catalog: Arc<Catalog>,
    cache_dir: PathBuf,
}

impl Installer {
    /// Instalador com o cache em `cache_dir` (no app, `<cache>/server`).
    #[must_use]
    pub fn new(http: HttpClient, catalog: Arc<Catalog>, cache_dir: PathBuf) -> Self {
        Self {
            http,
            catalog,
            cache_dir,
        }
    }

    /// Instala (idempotente) o servidor de `spec` em `dir`. `java` é o programa `java` que roda
    /// o instalador do Forge/NeoForge (o mesmo que vai abrir o servidor).
    ///
    /// Erros: [`Error::LoaderUnsupported`] (Quilt), [`Error::LoaderVersionNotFound`],
    /// [`Error::NoServerDownload`], [`Error::Download`], [`Error::InstallerFailed`],
    /// [`Error::InstallerOutputMissing`], [`Error::Cancelled`] e os de disco.
    pub async fn install(
        &self,
        spec: &GameSpec,
        dir: &Path,
        java: &Path,
        progress: &dyn ProgressSink,
        cancel: &CancellationToken,
    ) -> Result<InstalledServer> {
        if let Some(installed) = InstalledServer::existing(dir, spec) {
            crate::log4j::write_config(dir)?;
            return Ok(installed);
        }
        tokio::fs::create_dir_all(dir)
            .await
            .map_err(|error| Error::io("criar a pasta do servidor", dir, error))?;
        let minecraft = spec.minecraft.as_str();
        let version = self
            .catalog
            .version_json(minecraft, Some(cancel))
            .await
            .map_err(Error::catalog(ServerSource::Mojang))?;
        let json = version
            .json()
            .map_err(|error| Error::Internal(format!("JSON da versão {minecraft}: {error}")))?;
        let release_time = json
            .get("releaseTime")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_owned();
        let launch = match &spec.loader {
            LoaderSpec::Vanilla => {
                self.vanilla_jar(minecraft, &json, dir, progress, cancel)
                    .await?;
                ServerLaunch::Jar {
                    jar: VANILLA_JAR.to_owned(),
                }
            }
            LoaderSpec::Fabric { version } => {
                self.vanilla_jar(minecraft, &json, dir, progress, cancel)
                    .await?;
                self.fabric_launcher(minecraft, version, dir, progress, cancel)
                    .await?
            }
            LoaderSpec::Forge { version } => {
                self.forge_like(
                    ForgeKind::Forge,
                    minecraft,
                    version,
                    dir,
                    java,
                    progress,
                    cancel,
                )
                .await?
            }
            LoaderSpec::NeoForge { version } => {
                self.forge_like(
                    ForgeKind::NeoForge,
                    minecraft,
                    version,
                    dir,
                    java,
                    progress,
                    cancel,
                )
                .await?
            }
            LoaderSpec::Quilt { .. } => {
                return Err(Error::LoaderUnsupported {
                    loader: spec.loader.label().to_owned(),
                });
            }
        };
        let installed = InstalledServer {
            dir: dir.to_path_buf(),
            spec: spec.clone(),
            release_time,
            launch,
        };
        installed.record()?;
        tracing::info!(server = %spec, "servidor local instalado");
        Ok(installed)
    }

    /// Baixa para o cache (se faltar) e copia para `destination`.
    async fn cached(
        &self,
        request: DownloadRequest,
        source: ServerSource,
        destination: &Path,
        progress: &dyn ProgressSink,
        cancel: &CancellationToken,
    ) -> Result<()> {
        let cached = request.destination.clone();
        if !cached.is_file() {
            self.http
                .download(&request, progress, Some(cancel))
                .await
                .map_err(Error::http(source))?;
        }
        if let Some(parent) = destination.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|error| Error::io("criar a pasta do servidor", parent, error))?;
        }
        tokio::fs::copy(&cached, destination)
            .await
            .map_err(|error| Error::io("copiar para a pasta do servidor", destination, error))?;
        Ok(())
    }

    /// O `server.jar` vanilla.
    async fn vanilla_jar(
        &self,
        minecraft: &str,
        json: &serde_json::Value,
        dir: &Path,
        progress: &dyn ProgressSink,
        cancel: &CancellationToken,
    ) -> Result<()> {
        let server = json
            .pointer("/downloads/server")
            .ok_or_else(|| Error::NoServerDownload {
                minecraft: minecraft.to_owned(),
            })?;
        let url = server
            .get("url")
            .and_then(serde_json::Value::as_str)
            .and_then(|url| Url::parse(url).ok())
            .ok_or_else(|| Error::NoServerDownload {
                minecraft: minecraft.to_owned(),
            })?;
        let sha1 = server.get("sha1").and_then(serde_json::Value::as_str);
        let cached = self
            .cache_dir
            .join("vanilla")
            .join(safe_name(minecraft))
            .join(VANILLA_JAR);
        let mut request = DownloadRequest::new(url, cached).max_size(MAX_JAR_BYTES);
        if let Some(sha1) = sha1 {
            request = request.expect_hash(HashFormat::Sha1, sha1);
        }
        if let Some(size) = server.get("size").and_then(serde_json::Value::as_u64) {
            request = request.expect_size(size);
        }
        self.cached(
            request,
            ServerSource::Mojang,
            &dir.join(VANILLA_JAR),
            progress,
            cancel,
        )
        .await
    }

    /// O lançador de servidor do Fabric.
    async fn fabric_launcher(
        &self,
        minecraft: &str,
        loader: &str,
        dir: &Path,
        progress: &dyn ProgressSink,
        cancel: &CancellationToken,
    ) -> Result<ServerLaunch> {
        let base = &self.catalog.endpoints().fabric;
        let installers: Vec<FabricInstaller> = self
            .http
            .get(join_url(base, "v2/versions/installer")?)
            .cancel(cancel)
            .json()
            .await
            .map_err(Error::http(ServerSource::Fabric))?;
        let installer = installers
            .iter()
            .find(|item| item.stable)
            .or_else(|| installers.first())
            .ok_or_else(|| Error::Download {
                source_name: ServerSource::Fabric,
                message: "o Fabric meta não listou instaladores".into(),
                core: None,
            })?;
        let path = format!(
            "v2/versions/loader/{}/{}/{}/server/jar",
            url_segment(minecraft),
            url_segment(loader),
            url_segment(&installer.version)
        );
        let cached = self.cache_dir.join("fabric").join(format!(
            "fabric-server-{}-{}-{}.jar",
            safe_name(minecraft),
            safe_name(loader),
            safe_name(&installer.version)
        ));
        let request = DownloadRequest::new(join_url(base, &path)?, cached).max_size(MAX_JAR_BYTES);
        self.cached(
            request,
            ServerSource::Fabric,
            &dir.join(FABRIC_JAR),
            progress,
            cancel,
        )
        .await?;
        let properties = format!("serverJar={VANILLA_JAR}\n");
        atomic_write(
            &dir.join("fabric-server-launcher.properties"),
            properties.as_bytes(),
        )?;
        Ok(ServerLaunch::Jar {
            jar: FABRIC_JAR.to_owned(),
        })
    }

    /// Forge ou NeoForge pelo instalador oficial.
    #[allow(clippy::too_many_arguments)] // os dois loaders compartilham o mesmo caminho
    async fn forge_like(
        &self,
        kind: ForgeKind,
        minecraft: &str,
        version: &str,
        dir: &Path,
        java: &Path,
        progress: &dyn ProgressSink,
        cancel: &CancellationToken,
    ) -> Result<ServerLaunch> {
        let versions = self
            .catalog
            .loader_versions(
                kind.catalog_loader(),
                minecraft,
                warden_catalog::Refresh::IfStale,
                Some(cancel),
            )
            .await
            .map_err(Error::catalog(kind.source()))?;
        let not_found = || Error::LoaderVersionNotFound {
            loader: kind.label().to_owned(),
            version: version.to_owned(),
            minecraft: minecraft.to_owned(),
        };
        let coordinate = versions.get(version).ok_or_else(not_found)?.maven.clone();
        let artifact = MavenArtifact::parse(&coordinate).ok_or_else(not_found)?;
        let base = kind.maven_base(self.catalog.endpoints())?;
        let url = join_url(&base, &artifact.installer_path())?;
        let sha1 = self.maven_sha1(&url, kind.source(), cancel).await?;
        let name = artifact.installer_name();
        let mut request = DownloadRequest::new(url, self.cache_dir.join("installers").join(&name))
            .max_size(MAX_JAR_BYTES);
        if let Some(sha1) = sha1 {
            request = request.expect_hash(HashFormat::Sha1, sha1);
        }
        let installer = dir.join(&name);
        self.cached(request, kind.source(), &installer, progress, cancel)
            .await?;
        let result = run_installer(java, &name, dir, cancel).await;
        let _ = tokio::fs::remove_file(&installer).await;
        let output = result?;
        if output.code != Some(0) {
            return Err(Error::InstallerFailed {
                loader: kind.label().to_owned(),
                code: output.code,
                output: output.tail,
            });
        }
        // O instalador do Forge às vezes termina com 0 sem ter instalado (sem rede, certificado
        // recusado): o que vale é o servidor estar lá.
        find_launch(dir, &artifact.version).ok_or_else(|| Error::InstallerOutputMissing {
            loader: kind.label().to_owned(),
            dir: dir.to_path_buf(),
            output: output.tail,
        })
    }

    /// O `sha1` publicado ao lado do arquivo no Maven (`None` se o Maven não tem).
    async fn maven_sha1(
        &self,
        url: &Url,
        source: ServerSource,
        cancel: &CancellationToken,
    ) -> Result<Option<String>> {
        let sha1_url = Url::parse(&format!("{url}.sha1"))
            .map_err(|error| Error::Internal(format!("endereço do sha1: {error}")))?;
        let response = self
            .http
            .get(sha1_url)
            .cancel(cancel)
            .send()
            .await
            .map_err(Error::http(source))?;
        if response.status() == warden_http::StatusCode::NOT_FOUND {
            return Ok(None);
        }
        let body = response
            .error_for_status()
            .await
            .map_err(Error::http(source))?
            .bytes_limited(256)
            .await
            .map_err(Error::http(source))?;
        let text = String::from_utf8_lossy(&body);
        Ok(text
            .split_whitespace()
            .next()
            .filter(|hash| hash.len() == 40 && hash.bytes().all(|byte| byte.is_ascii_hexdigit()))
            .map(str::to_ascii_lowercase))
    }
}

/// Uma linha de `/v2/versions/installer` do Fabric meta.
#[derive(Debug, Deserialize)]
struct FabricInstaller {
    version: String,
    #[serde(default)]
    stable: bool,
}

/// Forge ou NeoForge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ForgeKind {
    Forge,
    NeoForge,
}

impl ForgeKind {
    const fn label(self) -> &'static str {
        match self {
            Self::Forge => "Forge",
            Self::NeoForge => "NeoForge",
        }
    }

    const fn source(self) -> ServerSource {
        match self {
            Self::Forge => ServerSource::Forge,
            Self::NeoForge => ServerSource::NeoForge,
        }
    }

    const fn catalog_loader(self) -> warden_catalog::Loader {
        match self {
            Self::Forge => warden_catalog::Loader::Forge,
            Self::NeoForge => warden_catalog::Loader::NeoForge,
        }
    }

    /// A raiz do Maven com os artefatos.
    fn maven_base(self, endpoints: &warden_catalog::Endpoints) -> Result<Url> {
        match self {
            Self::Forge => Ok(endpoints.forge_maven.clone()),
            Self::NeoForge => join_url(&endpoints.neoforge, "releases/"),
        }
    }
}

/// `grupo:artefato:versão`.
#[derive(Debug, Clone, PartialEq, Eq)]
struct MavenArtifact {
    group: String,
    artifact: String,
    version: String,
}

impl MavenArtifact {
    fn parse(coordinate: &str) -> Option<Self> {
        let mut parts = coordinate.split(':');
        let group = parts.next()?;
        let artifact = parts.next()?;
        let version = parts.next()?;
        let valid = |text: &str| {
            !text.is_empty()
                && text.bytes().all(|byte| {
                    byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_' | b'+')
                })
                && !text.contains("..")
        };
        (parts.next().is_none() && valid(group) && valid(artifact) && valid(version)).then(|| {
            Self {
                group: group.to_owned(),
                artifact: artifact.to_owned(),
                version: version.to_owned(),
            }
        })
    }

    fn installer_name(&self) -> String {
        format!("{}-{}-installer.jar", self.artifact, self.version)
    }

    fn installer_path(&self) -> String {
        format!(
            "{}/{}/{}/{}",
            self.group.replace('.', "/"),
            self.artifact,
            self.version,
            self.installer_name()
        )
    }
}

/// Junta um caminho relativo a uma raiz (que termina em `/`).
fn join_url(base: &Url, path: &str) -> Result<Url> {
    base.join(path)
        .map_err(|error| Error::Internal(format!("endereço {path}: {error}")))
}

/// Um segmento de caminho seguro na URL (as versões só têm estes caracteres).
fn url_segment(text: &str) -> String {
    text.chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_' | '+'))
        .collect()
}

/// Um nome de arquivo seguro para o cache.
fn safe_name(text: &str) -> String {
    let name: String = text
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_' | '+') {
                c
            } else {
                '_'
            }
        })
        .collect();
    name.trim_matches('.').to_owned()
}

/// O que o instalador deixou.
struct InstallerOutput {
    code: Option<i32>,
    tail: String,
}

/// Roda `java -jar <instalador> --installServer` na pasta do servidor, com a saída guardada
/// em `installer-output.log` (o fim dela vai para o erro).
async fn run_installer(
    java: &Path,
    installer: &str,
    dir: &Path,
    cancel: &CancellationToken,
) -> Result<InstallerOutput> {
    let mut command = tokio::process::Command::new(java);
    command
        .arg("-jar")
        .arg(installer)
        .arg("--installServer")
        .current_dir(dir)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    #[cfg(windows)]
    command.creation_flags(CREATE_NO_WINDOW);
    let mut child = command.spawn().map_err(|source| {
        Error::Process(warden_launcher::Error::LaunchFailed {
            program: java.to_path_buf(),
            source,
        })
    })?;
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let collect = async {
        let (out, err) = tokio::join!(read_all(stdout), read_all(stderr));
        let mut text = out;
        text.push_str(&err);
        text
    };
    let waited = async {
        let (status, text) = tokio::join!(child.wait(), collect);
        (status, text)
    };
    let (status, text) = tokio::select! {
        result = tokio::time::timeout(INSTALLER_TIMEOUT, waited) => match result {
            Ok(done) => done,
            Err(_) => {
                return Ok(InstallerOutput {
                    code: None,
                    tail: format!("o instalador passou de {} min", INSTALLER_TIMEOUT.as_secs() / 60),
                });
            }
        },
        () = cancel.cancelled() => return Err(Error::Cancelled),
    };
    let _ = tokio::fs::write(dir.join("installer-output.log"), text.as_bytes()).await;
    let status = status.map_err(|error| Error::io("esperar o instalador", dir, error))?;
    Ok(InstallerOutput {
        code: status.code(),
        tail: tail_of(&text, INSTALLER_OUTPUT_TAIL),
    })
}

async fn read_all(pipe: Option<impl AsyncRead + Unpin>) -> String {
    let mut bytes = Vec::new();
    if let Some(mut pipe) = pipe {
        let _ = pipe.read_to_end(&mut bytes).await;
    }
    String::from_utf8_lossy(&bytes).into_owned()
}

/// O fim do texto, até `max` bytes, começando numa linha inteira.
fn tail_of(text: &str, max: usize) -> String {
    if text.len() <= max {
        return text.to_owned();
    }
    let mut start = text.len() - max;
    while !text.is_char_boundary(start) {
        start += 1;
    }
    let tail = &text[start..];
    tail.find('\n')
        .map_or(tail, |newline| &tail[newline + 1..])
        .to_owned()
}

/// Como abrir o que o instalador deixou: o arquivo de argumentos do sistema (Forge 1.17+ e
/// NeoForge) ou o jar executável (Forge até a 1.16.5).
fn find_launch(dir: &Path, version: &str) -> Option<ServerLaunch> {
    let wanted = if cfg!(windows) {
        "win_args.txt"
    } else {
        "unix_args.txt"
    };
    let mut args_files = Vec::new();
    collect_named(&dir.join("libraries"), wanted, 12, &mut args_files);
    args_files.sort();
    let pick = args_files
        .iter()
        .find(|path| path.to_string_lossy().contains(version))
        .or_else(|| args_files.first());
    if let Some(file) = pick {
        let relative = file.strip_prefix(dir).ok()?;
        let text = relative
            .components()
            .map(|part| part.as_os_str().to_string_lossy())
            .collect::<Vec<_>>()
            .join("/");
        return Some(ServerLaunch::ArgsFile { file: text });
    }
    let mut jars: Vec<String> = std::fs::read_dir(dir)
        .ok()?
        .flatten()
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_file()))
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| {
            (name.starts_with("forge-") || name.starts_with("neoforge-"))
                && Path::new(name)
                    .extension()
                    .is_some_and(|extension| extension.eq_ignore_ascii_case("jar"))
                && !name.contains("installer")
        })
        .collect();
    jars.sort();
    let jar = jars
        .iter()
        .find(|name| name.contains(version))
        .or_else(|| jars.first())?
        .clone();
    Some(ServerLaunch::Jar { jar })
}

/// Arquivos com o nome dado, até `depth` pastas abaixo.
fn collect_named(dir: &Path, name: &str, depth: usize, found: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        match entry.file_type() {
            Ok(kind) if kind.is_dir() && depth > 0 => collect_named(&path, name, depth - 1, found),
            Ok(kind) if kind.is_file() && entry.file_name() == name => found.push(path),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coordenada_maven_vira_o_caminho_do_instalador() {
        let forge =
            MavenArtifact::parse("net.minecraftforge:forge:1.7.10-10.13.4.1614-1.7.10").unwrap();
        assert_eq!(
            forge.installer_path(),
            "net/minecraftforge/forge/1.7.10-10.13.4.1614-1.7.10/forge-1.7.10-10.13.4.1614-1.7.10-installer.jar"
        );
        let neo = MavenArtifact::parse("net.neoforged:neoforge:21.1.252").unwrap();
        assert_eq!(neo.installer_name(), "neoforge-21.1.252-installer.jar");
        assert!(MavenArtifact::parse("a:b").is_none());
        assert!(MavenArtifact::parse("a:b:../c").is_none());
        assert!(MavenArtifact::parse("a:b:c:d").is_none());
    }

    #[test]
    fn acha_o_arquivo_de_argumentos_ou_o_jar() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(find_launch(dir.path(), "47.3.0"), None);
        std::fs::write(dir.path().join("forge-1.16.5-36.2.34-installer.jar"), "x").unwrap();
        std::fs::write(dir.path().join("forge-1.16.5-36.2.34.jar"), "x").unwrap();
        assert_eq!(
            find_launch(dir.path(), "36.2.34"),
            Some(ServerLaunch::Jar {
                jar: "forge-1.16.5-36.2.34.jar".into()
            })
        );
        let name = if cfg!(windows) {
            "win_args.txt"
        } else {
            "unix_args.txt"
        };
        let args = dir
            .path()
            .join("libraries/net/minecraftforge/forge/1.20.1-47.3.0");
        std::fs::create_dir_all(&args).unwrap();
        std::fs::write(args.join(name), "-cp x").unwrap();
        assert_eq!(
            find_launch(dir.path(), "47.3.0"),
            Some(ServerLaunch::ArgsFile {
                file: format!("libraries/net/minecraftforge/forge/1.20.1-47.3.0/{name}")
            })
        );
    }

    #[test]
    fn nomes_seguros_e_fim_da_saida() {
        assert_eq!(safe_name("1.20.1"), "1.20.1");
        assert_eq!(safe_name("../x y"), "_x_y");
        assert_eq!(url_segment("0.19.5/../"), "0.19.5..");
        let text = "linha 1\nlinha 2\nlinha 3\n";
        assert_eq!(tail_of(text, 100), text);
        assert_eq!(tail_of(text, 10), "linha 3\n");
    }
}
