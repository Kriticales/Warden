//! Apoio dos testes de materialização: packs de teste montados em pasta temporária, servidor
//! simulado com os arquivos dos mods, o packwiz-installer real e comparação de árvores.
//!
//! Os "jars" são bytes sintéticos servidos pelo `wiremock` em `127.0.0.1`: o packwiz-installer e
//! o Warden baixam do mesmo lugar, sem internet e sem versionar jars de terceiros. As pastas
//! temporárias ficam dentro de `WARDEN_DATA_ROOT` quando ela está definida (ADR-0053).

// Nos auxiliares dos testes, `unwrap`, `expect` e `panic!` são a forma idiomática de falhar
// (o `clippy.toml` só os libera dentro das funções `#[test]`).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::missing_panics_doc,
    clippy::cast_possible_truncation,
    clippy::needless_pass_by_value,
    dead_code,
    unreachable_pub
)]

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;

use serde_json::{Value, json};
use warden_core::{CancellationToken, NoProgress};
use warden_curseforge::{CurseforgeClient, SecretString};
use warden_http::{HttpClient, HttpConfig};
use warden_instance::{
    DownloadCache, InstanceDirs, MaterializeOptions, Outcome, Report, Sources, materialize,
};
use warden_packwiz::HashFormat;
use warden_packwiz::hash::hash_bytes;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// Chave falsa, no formato das chaves da CurseForge.
pub const KEY: &str = "$2a$10$chaveFalsaDoWardenParaTestes0123456789abcdefghijkl";

/// Pasta temporária (dentro de `WARDEN_DATA_ROOT`, se definida).
pub fn temp_dir() -> tempfile::TempDir {
    match std::env::var_os("WARDEN_DATA_ROOT").filter(|root| !root.is_empty()) {
        Some(root) => {
            fs::create_dir_all(&root).unwrap();
            tempfile::tempdir_in(root).unwrap()
        }
        None => tempfile::tempdir().unwrap(),
    }
}

/// Grava `contents` em `root/relative`, criando as pastas.
pub fn write(root: &Path, relative: &str, contents: &[u8]) {
    let path = root.join(relative);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, contents).unwrap();
}

/// Bytes de um "jar" de teste (determinísticos, diferentes por nome).
pub fn jar(name: &str) -> Vec<u8> {
    let mut data = format!("PK-falso:{name}:").into_bytes();
    data.extend((0..2048_u32).map(|i| (i.wrapping_mul(31) ^ name.len() as u32) as u8));
    data
}

/// Lado de um metafile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TestSide {
    Both,
    Client,
    Server,
}

impl TestSide {
    fn as_str(self) -> &'static str {
        match self {
            Self::Both => "both",
            Self::Client => "client",
            Self::Server => "server",
        }
    }
}

/// Um mod por link no pack de teste.
#[derive(Debug, Clone)]
pub struct TestMod {
    /// Caminho do metafile no índice (`mods/a.pw.toml`).
    pub metafile: String,
    pub name: String,
    pub filename: String,
    pub side: TestSide,
    /// Conteúdo servido.
    pub data: Vec<u8>,
    /// `Some(default)` para opcional.
    pub optional: Option<bool>,
    pub hash_format: HashFormat,
    /// `preserve` no índice.
    pub preserve: bool,
    /// Modo CurseForge (`metadata:curseforge`) com IDs; senão, link.
    pub curseforge: Option<(u32, u32)>,
    /// IDs do Modrinth em `[update.modrinth]`.
    pub modrinth: Option<(String, String)>,
    /// Endereço real (testes de rede); senão, o do servidor simulado.
    pub url: Option<String>,
    /// Hash dado (testes de rede, quando os bytes não são conhecidos); senão, o de `data`.
    pub known_hash: Option<String>,
}

impl TestMod {
    /// Mod por link em `mods/<slug>.pw.toml`, lado `both`, hash sha1.
    pub fn new(slug: &str) -> Self {
        Self {
            metafile: format!("mods/{slug}.pw.toml"),
            name: slug.to_uppercase(),
            filename: format!("{slug}-1.0.jar"),
            side: TestSide::Both,
            data: jar(slug),
            optional: None,
            hash_format: HashFormat::Sha1,
            preserve: false,
            curseforge: None,
            modrinth: None,
            url: None,
            known_hash: None,
        }
    }

    /// Mod real: endereço (vazio para a CurseForge), nome do arquivo e hash conhecidos.
    pub fn real(
        slug: &str,
        filename: &str,
        format: HashFormat,
        hash: &str,
        url: Option<&str>,
    ) -> Self {
        let mut test_mod = Self::new(slug);
        filename.clone_into(&mut test_mod.filename);
        test_mod.hash_format = format;
        test_mod.known_hash = Some(hash.to_owned());
        test_mod.url = url.map(str::to_owned);
        test_mod
    }

    pub fn side(mut self, side: TestSide) -> Self {
        self.side = side;
        self
    }

    pub fn optional(mut self, default: bool) -> Self {
        self.optional = Some(default);
        self
    }

    pub fn at(mut self, metafile: &str, filename: &str) -> Self {
        metafile.clone_into(&mut self.metafile);
        filename.clone_into(&mut self.filename);
        self
    }

    pub fn version(mut self, version: &str) -> Self {
        let slug = self
            .metafile
            .rsplit('/')
            .next()
            .unwrap()
            .trim_end_matches(".pw.toml")
            .to_owned();
        self.filename = format!("{slug}-{version}.jar");
        self.data = jar(&format!("{slug}-{version}"));
        self
    }

    pub fn hash(mut self, format: HashFormat) -> Self {
        self.hash_format = format;
        self
    }

    pub fn curseforge(mut self, project: u32, file: u32) -> Self {
        self.curseforge = Some((project, file));
        self
    }

    pub fn modrinth(mut self, project: &str, version: &str) -> Self {
        self.modrinth = Some((project.to_owned(), version.to_owned()));
        self
    }

    /// Caminho servido pelo servidor simulado.
    pub fn served_path(&self) -> String {
        format!("/files/{}", self.filename.replace(' ', "%20"))
    }

    fn metafile_text(&self, base_url: &str) -> String {
        let mut text = format!(
            "name = \"{}\"\nfilename = \"{}\"\nside = \"{}\"\n\n[download]\n",
            self.name,
            self.filename,
            self.side.as_str()
        );
        let hash = self
            .known_hash
            .clone()
            .unwrap_or_else(|| hash_bytes(self.hash_format, &self.data));
        if self.curseforge.is_some() {
            let _ = writeln!(text, "hash-format = \"{}\"", self.hash_format);
            let _ = writeln!(text, "hash = \"{hash}\"");
            text.push_str("mode = \"metadata:curseforge\"\n");
        } else {
            match &self.url {
                Some(url) => {
                    let _ = writeln!(text, "url = \"{url}\"");
                }
                None => {
                    let _ = writeln!(text, "url = \"{base_url}{}\"", self.served_path());
                }
            }
            let _ = writeln!(text, "hash-format = \"{}\"", self.hash_format);
            let _ = writeln!(text, "hash = \"{hash}\"");
        }
        if let Some(default) = self.optional {
            let _ = write!(
                text,
                "\n[option]\noptional = true\ndescription = \"Opcional de teste\"\n"
            );
            if default {
                text.push_str("default = true\n");
            }
        }
        if let Some((project, file)) = self.curseforge {
            let _ = write!(
                text,
                "\n[update]\n[update.curseforge]\nfile-id = {file}\nproject-id = {project}\n"
            );
        } else if let Some((project, version)) = &self.modrinth {
            let _ = write!(
                text,
                "\n[update]\n[update.modrinth]\nmod-id = \"{project}\"\nversion = \"{version}\"\n"
            );
        }
        text
    }
}

/// Um pack de teste (o conteúdo; [`TestPack::write_to`] grava índice e `pack.toml`).
#[derive(Debug, Clone, Default)]
pub struct TestPack {
    pub mods: Vec<TestMod>,
    /// Arquivos do pack (`config/x.toml` → conteúdo), com `preserve`.
    pub files: BTreeMap<String, (Vec<u8>, bool)>,
    /// Pasta do índice (vazia: raiz).
    pub index_dir: String,
}

impl TestPack {
    pub fn with_mod(mut self, test_mod: TestMod) -> Self {
        self.mods.push(test_mod);
        self
    }

    pub fn with_file(mut self, relative: &str, contents: &str) -> Self {
        self.files
            .insert(relative.to_owned(), (contents.as_bytes().to_vec(), false));
        self
    }

    pub fn with_preserved(mut self, relative: &str, contents: &str) -> Self {
        self.files
            .insert(relative.to_owned(), (contents.as_bytes().to_vec(), true));
        self
    }

    pub fn without_mod(mut self, metafile: &str) -> Self {
        self.mods.retain(|m| m.metafile != metafile);
        self
    }

    pub fn replace_mod(mut self, test_mod: TestMod) -> Self {
        self.mods.retain(|m| m.metafile != test_mod.metafile);
        self.mods.push(test_mod);
        self
    }

    /// Grava o pack em `root` (apagando o que havia), com o índice e o `pack.toml` com os
    /// hashes certos, como o `packwiz refresh` faria.
    pub fn write_to(&self, root: &Path, base_url: &str) {
        if root.exists() {
            fs::remove_dir_all(root).unwrap();
        }
        fs::create_dir_all(root).unwrap();
        let prefix = if self.index_dir.is_empty() {
            String::new()
        } else {
            format!("{}/", self.index_dir)
        };
        let mut entries: Vec<(String, String, bool, bool)> = Vec::new();
        for test_mod in &self.mods {
            let text = test_mod.metafile_text(base_url);
            write(
                root,
                &format!("{prefix}{}", test_mod.metafile),
                text.as_bytes(),
            );
            entries.push((
                test_mod.metafile.clone(),
                hash_bytes(HashFormat::Sha256, text.as_bytes()),
                true,
                test_mod.preserve,
            ));
        }
        for (relative, (contents, preserve)) in &self.files {
            write(root, &format!("{prefix}{relative}"), contents);
            entries.push((
                relative.clone(),
                hash_bytes(HashFormat::Sha256, contents),
                false,
                *preserve,
            ));
        }
        entries.sort();
        let mut index = String::from("hash-format = \"sha256\"\n");
        for (file, hash, metafile, preserve) in &entries {
            let _ = write!(index, "\n[[files]]\nfile = \"{file}\"\nhash = \"{hash}\"\n");
            if *metafile {
                index.push_str("metafile = true\n");
            }
            if *preserve {
                index.push_str("preserve = true\n");
            }
        }
        let index_file = format!("{prefix}index.toml");
        write(root, &index_file, index.as_bytes());
        let pack = format!(
            "name = \"Pack de teste\"\nauthor = \"Warden\"\nversion = \"1.0.0\"\npack-format = \"packwiz:1.1.0\"\n\n\
             [index]\nfile = \"{index_file}\"\nhash-format = \"sha256\"\nhash = \"{}\"\n\n\
             [versions]\nfabric = \"0.16.5\"\nminecraft = \"1.20.1\"\n",
            hash_bytes(HashFormat::Sha256, index.as_bytes())
        );
        write(root, "pack.toml", pack.as_bytes());
    }

    /// Registra no servidor os arquivos dos mods por link.
    pub async fn serve(&self, server: &MockServer) {
        for test_mod in &self.mods {
            Mock::given(method("GET"))
                .and(path(test_mod.served_path()))
                .respond_with(
                    ResponseTemplate::new(200)
                        .insert_header("content-type", "application/java-archive")
                        .set_body_bytes(test_mod.data.clone()),
                )
                .mount(server)
                .await;
        }
    }
}

/// Fontes da materialização com cache em `cache_dir`.
pub fn sources(cache_dir: &Path, curseforge: Option<CurseforgeClient>) -> Sources {
    let http = HttpClient::new(HttpConfig::for_version("teste")).unwrap();
    Sources {
        http,
        curseforge,
        cache: Arc::new(DownloadCache::open(cache_dir).unwrap()),
    }
}

/// Cliente da CurseForge apontado para o servidor simulado.
pub fn curseforge(server: &MockServer, key: Option<&str>) -> CurseforgeClient {
    let http = HttpClient::new(HttpConfig::for_version("teste")).unwrap();
    CurseforgeClient::with_base_url(
        http,
        &format!("{}/v1", server.uri()),
        key.map(|key| SecretString::from(key.to_owned())),
    )
    .unwrap()
}

/// Pastas de instância em `root`.
pub fn dirs(root: &Path) -> InstanceDirs {
    InstanceDirs {
        game_dir: root.join("minecraft"),
        state_dir: root.join("state"),
    }
}

/// Materializa e exige [`Outcome::Done`].
pub async fn materialize_ok(
    sources: &Sources,
    pack: &Path,
    dirs: &InstanceDirs,
    options: &MaterializeOptions,
) -> Report {
    match materialize(
        sources,
        pack,
        dirs,
        options,
        &NoProgress,
        &CancellationToken::new(),
    )
    .await
    {
        Ok(Outcome::Done(report)) => report,
        other => panic!("materialização não terminou: {other:?}"),
    }
}

/// Árvore de uma pasta: caminho relativo (com `/`) → SHA-256; pastas com `/` no fim e valor
/// vazio. Ignora `packwiz.json` (manifesto do packwiz-installer).
pub fn tree(root: &Path) -> BTreeMap<String, String> {
    fn walk(root: &Path, dir: &Path, out: &mut BTreeMap<String, String>) {
        for entry in fs::read_dir(dir).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            let relative = path
                .strip_prefix(root)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/");
            if path.is_dir() {
                out.insert(format!("{relative}/"), String::new());
                walk(root, &path, out);
            } else if relative != "packwiz.json" {
                out.insert(
                    relative,
                    hash_bytes(HashFormat::Sha256, &fs::read(&path).unwrap()),
                );
            }
        }
    }
    let mut out = BTreeMap::new();
    if root.exists() {
        walk(root, root, &mut out);
    }
    out
}

/// O que o teste de conformidade precisa do `cargo xtask installer`.
#[derive(Debug, Clone)]
pub struct Externals {
    pub java: PathBuf,
    pub bootstrap: PathBuf,
    pub installer: PathBuf,
}

/// Pasta do cache do `cargo xtask installer` (mesma regra de `xtask/src/installer.rs`).
fn installer_cache_dir() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os("WARDEN_INSTALLER_CACHE").filter(|v| !v.is_empty()) {
        return Some(PathBuf::from(dir));
    }
    if cfg!(windows) {
        let local = std::env::var_os("LOCALAPPDATA")?;
        return Some(
            PathBuf::from(local)
                .join("Warden-dev")
                .join("cache")
                .join("installer"),
        );
    }
    if let Some(cache) = std::env::var_os("XDG_CACHE_HOME").filter(|v| !v.is_empty()) {
        return Some(PathBuf::from(cache).join("Warden-dev").join("installer"));
    }
    let home = std::env::var_os("HOME")?;
    Some(
        PathBuf::from(home)
            .join(".cache")
            .join("Warden-dev")
            .join("installer"),
    )
}

/// Java, bootstrap e packwiz-installer do `cargo xtask installer`. Sem eles: aviso e `None`;
/// com `WARDEN_REQUIRE_EXTERNALS=1` (CI), falha (QUALITY §4.1).
pub fn externals() -> Option<Externals> {
    let file = installer_cache_dir().map(|dir| dir.join("externals.json"));
    let parsed = file
        .as_ref()
        .and_then(|file| fs::read(file).ok())
        .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
        .and_then(|json| {
            let path = |key: &str| json[key].as_str().map(PathBuf::from);
            Some(Externals {
                java: path("java")?,
                bootstrap: path("bootstrap")?,
                installer: path("installer")?,
            })
        })
        .filter(|ext| ext.java.is_file() && ext.bootstrap.is_file() && ext.installer.is_file());
    if parsed.is_none() {
        assert!(
            std::env::var_os("WARDEN_REQUIRE_EXTERNALS").is_none_or(|value| value != "1"),
            "WARDEN_REQUIRE_EXTERNALS=1, mas o packwiz-installer e o Java não estão em {file:?} \
             (rode `cargo xtask installer`)"
        );
        eprintln!(
            "AVISO: packwiz-installer e Java não encontrados ({file:?}); rode `cargo xtask \
             installer`. Teste de conformidade com o instalador real pulado."
        );
    }
    parsed
}

/// Roda o packwiz-installer real (pelo bootstrap, sem atualizar, sem janela, lado cliente) do
/// `pack.toml` local para `out`.
pub fn run_installer(externals: &Externals, pack_toml: &Path, out: &Path, cwd: &Path) {
    fs::create_dir_all(out).unwrap();
    fs::create_dir_all(cwd).unwrap();
    let output = Command::new(&externals.java)
        .arg("-jar")
        .arg(&externals.bootstrap)
        .arg("--bootstrap-no-update")
        .arg("--bootstrap-main-jar")
        .arg(&externals.installer)
        .args(["-g", "-s", "client", "--pack-folder"])
        .arg(out)
        .arg(pack_toml)
        .current_dir(cwd)
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success() && stdout.contains("Finished successfully!"),
        "packwiz-installer falhou ({}):\n{stdout}\n{}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
}

/// Um arquivo no formato da API da CurseForge (campos que o Warden lê).
pub fn cf_file(mod_id: u64, file_id: u64, name: &str, data: &[u8], url: Option<&str>) -> Value {
    json!({
        "id": file_id,
        "gameId": 432,
        "modId": mod_id,
        "isAvailable": true,
        "displayName": name,
        "fileName": name,
        "releaseType": 1,
        "fileStatus": 4,
        "hashes": [{"value": hash_bytes(HashFormat::Sha1, data), "algo": 1}],
        "fileDate": "2026-09-19T11:47:57.347Z",
        "fileLength": data.len(),
        "downloadCount": 7,
        "downloadUrl": url,
        "gameVersions": ["1.20.1", "Fabric"],
        "dependencies": [],
        "fileFingerprint": warden_packwiz::hash::curseforge_fingerprint(data),
    })
}

/// Um projeto no formato da API da CurseForge.
pub fn cf_project(id: u64, slug: &str, allow_distribution: bool) -> Value {
    json!({
        "id": id,
        "gameId": 432,
        "name": slug,
        "slug": slug,
        "links": {"websiteUrl": format!("https://www.curseforge.com/minecraft/mc-mods/{slug}")},
        "summary": "",
        "status": 4,
        "downloadCount": 1,
        "classId": 6,
        "authors": [],
        "allowModDistribution": allow_distribution,
        "isAvailable": true
    })
}

/// `{"data": …}`.
pub fn cf_data(value: Value) -> ResponseTemplate {
    ResponseTemplate::new(200)
        .insert_header("content-type", "application/json; charset=utf-8")
        .set_body_json(json!({ "data": value }))
}
