//! Aceite de "Navegar por modpacks" (P1-17; SPEC CA-T08-14, CA-T08-15 e CA-T08-16).
//!
//! - **Modrinth:** respostas reais do "Create+" (`t1tOiUHZ`, versão `6.0.0-alpha-f`, NeoForge
//!   1.21.1), gravadas da API em 13/10/2026 em `testdata/`: as duas versões mais novas
//!   (`/project/t1tOiUHZ/version`), os 155 projetos em lote (`/projects?ids=`, sem os campos
//!   grandes que o Warden não lê: `body`, `versions`, `gallery`, `donation_urls`) e o
//!   `modrinth.index.json` de dentro do `.mrpack` de 3,5 MB.
//! - **Leitura por partes:** um `.mrpack` de ~3,5 MB com 768 entradas, montado aqui com o índice
//!   real por último (como no arquivo real), servido por um servidor que conta pedidos e bytes,
//!   com e sem `Range`.
//! - **CurseForge:** respostas montadas aqui (os termos da CurseForge proíbem guardar dados da
//!   API), com dois servidores que imitam o `edge.forgecdn.net` (404 a pedidos com `Range`) e o
//!   `mediafilez.forgecdn.net` (aceita `Range`).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::io::Write as _;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use serde_json::{Value, json};
use warden_core::CancellationToken;
use warden_curseforge::CurseforgeClient;
use warden_http::{HttpClient, HttpConfig, RangeOptions};
use warden_modrinth::ModrinthClient;
use warden_packwiz::Loader;
use warden_project::dedup::{PackItem, PackItems};
use warden_project::search::{PackTarget, SourceId};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, Request, Respond, ResponseTemplate};

use super::archive::read_entry;
use super::{
    ModpackContents, ModpackFailure, ModpackModState, ModpackSources, PackView, modpack_contents,
};
use crate::DiscoveryErrorCode;

const CREATE_PLUS: &str = "t1tOiUHZ";
const VERSIONS: &str = include_str!("testdata/create-plus-versions.json");
const PROJECTS: &str = include_str!("testdata/create-plus-projects.json");
const INDEX: &str = include_str!("testdata/create-plus-modrinth.index.json");

fn http() -> HttpClient {
    let mut config = HttpConfig::for_version("1");
    config.max_retries = 0;
    HttpClient::new(config).unwrap()
}

fn neoforge_1_21_1() -> PackTarget {
    PackTarget {
        minecraft: "1.21.1".into(),
        game_versions: vec!["1.21.1".into()],
        loaders: vec![Loader::NeoForge],
    }
}

fn item(path: &str, name: &str, source: SourceId, project_id: &str) -> PackItem {
    PackItem {
        path: path.into(),
        name: name.into(),
        source: Some(source),
        project_id: Some(project_id.into()),
        version_id: Some("1".into()),
        sha1: None,
    }
}

/// O pack de teste: o Create pelo Modrinth e o Farmer's Delight pela CurseForge (achado pelo
/// nome, deduplicação entre fontes).
fn pack_items() -> PackItems {
    PackItems::new(vec![
        item(
            "mods/create.pw.toml",
            "Create",
            SourceId::Modrinth,
            "LNytGWDc",
        ),
        item(
            "mods/farmers-delight.pw.toml",
            "Farmer's Delight",
            SourceId::Curseforge,
            "398521",
        ),
    ])
}

/// `/projects?ids=[...]`: os projetos pedidos, do lote gravado.
struct ProjectsById(Vec<Value>);

impl Respond for ProjectsById {
    fn respond(&self, request: &Request) -> ResponseTemplate {
        let ids: Vec<String> = request
            .url
            .query_pairs()
            .find(|(key, _)| key == "ids")
            .and_then(|(_, value)| serde_json::from_str(&value).ok())
            .unwrap_or_default();
        let found: Vec<&Value> = self
            .0
            .iter()
            .filter(|project| ids.iter().any(|id| project["id"] == id.as_str()))
            .collect();
        ResponseTemplate::new(200).set_body_json(found)
    }
}

/// Arquivo servido com ou sem `Range`, contando pedidos parciais e bytes enviados.
#[derive(Clone)]
struct FileServer {
    data: Arc<Vec<u8>>,
    ranges: bool,
    requests: Arc<AtomicUsize>,
    ranged: Arc<AtomicUsize>,
    sent: Arc<AtomicUsize>,
}

impl FileServer {
    fn new(data: Vec<u8>, ranges: bool) -> Self {
        Self {
            data: Arc::new(data),
            ranges,
            requests: Arc::default(),
            ranged: Arc::default(),
            sent: Arc::default(),
        }
    }
}

impl Respond for FileServer {
    fn respond(&self, request: &Request) -> ResponseTemplate {
        let len = self.data.len();
        if request.method.as_str() == "HEAD" {
            return ResponseTemplate::new(200);
        }
        self.requests.fetch_add(1, Ordering::Relaxed);
        let range = request
            .headers
            .get("range")
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.strip_prefix("bytes="))
            .filter(|_| self.ranges)
            .and_then(|spec| {
                let (start, end) = spec.split_once('-')?;
                if start.is_empty() {
                    let suffix: usize = end.parse().ok()?;
                    return Some((len.saturating_sub(suffix), len - 1));
                }
                let start: usize = start.parse().ok()?;
                let end: usize = end.parse().ok()?;
                Some((start, end.min(len - 1)))
            });
        if let Some((start, end)) = range {
            self.ranged.fetch_add(1, Ordering::Relaxed);
            self.sent.fetch_add(end + 1 - start, Ordering::Relaxed);
            ResponseTemplate::new(206)
                .insert_header("content-range", format!("bytes {start}-{end}/{len}"))
                .set_body_bytes(self.data[start..=end].to_vec())
        } else {
            self.sent.fetch_add(len, Ordering::Relaxed);
            ResponseTemplate::new(200).set_body_bytes(self.data.as_slice().to_vec())
        }
    }
}

/// Imita o `edge.forgecdn.net`: 404 a qualquer pedido com `Range`; sem `Range`, redireciona.
struct Edge {
    target: String,
    ranged: Arc<AtomicUsize>,
}

impl Respond for Edge {
    fn respond(&self, request: &Request) -> ResponseTemplate {
        if request.headers.contains_key("range") {
            self.ranged.fetch_add(1, Ordering::Relaxed);
            return ResponseTemplate::new(404);
        }
        ResponseTemplate::new(302).insert_header("location", self.target.as_str())
    }
}

/// Um `.mrpack` de ~3,5 MB com 768 entradas: 767 de `overrides/` (arquivos guardados sem
/// compressão, dois deles jars soltos em `overrides/mods/`) e, por último, o
/// `modrinth.index.json` real comprimido, como no arquivo do Create+.
fn mrpack(index: &str) -> Vec<u8> {
    let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let stored =
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    let filler: Vec<u8> = (0..4_400_u32)
        .map(|i| u8::try_from(i.wrapping_mul(2_654_435_761) >> 24).unwrap_or(0))
        .collect();
    for i in 0..765 {
        zip.start_file(format!("overrides/config/arquivo-{i:03}.toml"), stored)
            .unwrap();
        zip.write_all(&filler).unwrap();
    }
    for name in [
        "overrides/mods/ajustes-do-autor.jar",
        "overrides/mods/outro.jar",
    ] {
        zip.start_file(name, stored).unwrap();
        zip.write_all(&filler).unwrap();
    }
    let deflated = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    zip.start_file("modrinth.index.json", deflated).unwrap();
    zip.write_all(index.as_bytes()).unwrap();
    zip.finish().unwrap().into_inner()
}

struct Modrinth {
    server: MockServer,
    client: ModrinthClient,
}

impl Modrinth {
    async fn start(versions: Value) -> Self {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path(format!("/v2/project/{CREATE_PLUS}/version")))
            .respond_with(ResponseTemplate::new(200).set_body_json(versions))
            .mount(&server)
            .await;
        let projects: Vec<Value> = serde_json::from_str(PROJECTS).unwrap();
        Mock::given(method("GET"))
            .and(path("/v2/projects"))
            .respond_with(ProjectsById(projects))
            .mount(&server)
            .await;
        let client =
            ModrinthClient::with_base_url(http(), &format!("{}/v2", server.uri()), None).unwrap();
        Self { server, client }
    }

    async fn api_requests(&self) -> usize {
        self.server
            .received_requests()
            .await
            .unwrap_or_default()
            .iter()
            .filter(|request| request.url.path().starts_with("/v2/"))
            .count()
    }

    fn sources(&self) -> ModpackSources {
        ModpackSources {
            modrinth: self.client.clone(),
            curseforge: None,
            http: http(),
        }
    }
}

async fn open(
    sources: &ModpackSources,
    items: &PackItems,
    source: SourceId,
    project_id: &str,
) -> Result<ModpackContents, ModpackFailure> {
    let target = neoforge_1_21_1();
    let pack = PackView {
        target: &target,
        items,
    };
    modpack_contents(
        sources,
        &pack,
        source,
        project_id,
        &CancellationToken::new(),
    )
    .await
}

fn count(contents: &ModpackContents, state: ModpackModState) -> usize {
    contents.mods.iter().filter(|m| m.state == state).count()
}

fn find<'a>(contents: &'a ModpackContents, title: &str) -> &'a super::ModpackMod {
    contents
        .mods
        .iter()
        .find(|m| m.title == title)
        .unwrap_or_else(|| panic!("{title} não está na lista"))
}

/// CA-T08-14: o Create+ aberto com no máximo 4 requisições, com os quatro estados.
#[tokio::test]
async fn create_plus_do_modrinth_lista_os_mods_com_no_maximo_4_requisicoes() {
    let api = Modrinth::start(serde_json::from_str(VERSIONS).unwrap()).await;
    let contents = open(
        &api.sources(),
        &pack_items(),
        SourceId::Modrinth,
        CREATE_PLUS,
    )
    .await
    .unwrap();

    assert!(
        api.api_requests().await <= 4,
        "{} requisições",
        api.api_requests().await
    );
    assert_eq!(contents.title, "Create+");
    assert_eq!(contents.version_number, "6.0.0-alpha-f");
    assert!(
        contents.same_target,
        "modpack NeoForge 1.21.1 num pack NeoForge 1.21.1"
    );
    assert_eq!(contents.license.name.as_deref(), Some("LGPL-3.0-only"));
    assert!(contents.license.free);
    assert!(!contents.whole_download);
    // 168 dependências `embedded`: 150 mods, 4 resource packs (fora da lista de mods) e 14
    // arquivos fora das lojas.
    assert_eq!(contents.mods.len(), 164);
    assert_eq!(count(&contents, ModpackModState::InPack), 2);
    assert_eq!(count(&contents, ModpackModState::Compatible), 144);
    assert_eq!(count(&contents, ModpackModState::NoVersion), 4);
    assert_eq!(count(&contents, ModpackModState::External), 14);
    assert!(
        contents
            .mods
            .iter()
            .all(|m| !m.title.starts_with("Fresh Animations"))
    );

    // Já no pack pelo mesmo projeto e pela deduplicação entre fontes (nome).
    let create = find(&contents, "Create");
    assert_eq!(create.state, ModpackModState::InPack);
    assert_eq!(create.pack_path.as_deref(), Some("mods/create.pw.toml"));
    let farmers = find(&contents, "Farmer's Delight");
    assert_eq!(farmers.state, ModpackModState::InPack);
    assert_eq!(
        farmers.pack_path.as_deref(),
        Some("mods/farmers-delight.pw.toml")
    );
    // Compatível, com a versão exata do modpack para "a mesma do modpack".
    let sodium = find(&contents, "Sodium");
    assert_eq!(sodium.state, ModpackModState::Compatible);
    assert_eq!(sodium.key, "modrinth:AANobbMI");
    assert!(
        sodium
            .version_id
            .as_deref()
            .is_some_and(|id| !id.is_empty())
    );
    // Só para Fabric: sem versão para um pack NeoForge.
    assert_eq!(
        find(&contents, "Smart Block Placement").state,
        ModpackModState::NoVersion
    );
    // Jar fora das lojas, só com o nome do arquivo.
    let external = contents
        .mods
        .iter()
        .find(|m| m.state == ModpackModState::External)
        .unwrap();
    assert!(external.source.is_none() && external.project_id.is_none());
    assert!(
        std::path::Path::new(external.file_name.as_deref().unwrap())
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("jar"))
    );
    // Ordem: compatíveis, no pack, sem versão, fora das lojas.
    assert_eq!(contents.mods[0].state, ModpackModState::Compatible);
    assert_eq!(
        contents.mods.last().unwrap().state,
        ModpackModState::External
    );
}

/// Um modpack de outro loader ou versão: "a mesma do modpack" não é o padrão.
#[tokio::test]
async fn modpack_de_outro_alvo_nao_sugere_a_mesma_versao() {
    let mut versions: Vec<Value> = serde_json::from_str(VERSIONS).unwrap();
    for version in &mut versions {
        version["loaders"] = json!(["fabric"]);
    }
    let api = Modrinth::start(Value::Array(versions)).await;
    let contents = open(
        &api.sources(),
        &pack_items(),
        SourceId::Modrinth,
        CREATE_PLUS,
    )
    .await
    .unwrap();
    assert!(!contents.same_target);
}

/// Versões com dependências `embedded` vazias e o `.mrpack` no servidor de arquivos.
async fn without_embedded(file_url: &str) -> Modrinth {
    let mut versions: Vec<Value> = serde_json::from_str(VERSIONS).unwrap();
    for version in &mut versions {
        version["dependencies"] = json!([]);
        version["files"][0]["url"] = json!(file_url);
    }
    Modrinth::start(Value::Array(versions)).await
}

/// CA-T08-15: sem a lista na API, o `modrinth.index.json` é lido por partes do `.mrpack` de
/// 3,5 MB com no máximo 6 pedidos `Range` e sem baixar o arquivo inteiro.
#[tokio::test]
async fn sem_lista_na_api_le_o_indice_do_mrpack_por_partes() {
    let data = mrpack(INDEX);
    assert!(
        data.len() > 3_400_000 && data.len() < 3_700_000,
        "{} bytes",
        data.len()
    );
    let files = MockServer::start().await;
    let file = FileServer::new(data.clone(), true);
    Mock::given(path("/data/create-plus.mrpack"))
        .respond_with(file.clone())
        .mount(&files)
        .await;
    let api = without_embedded(&format!("{}/data/create-plus.mrpack", files.uri())).await;

    let contents = open(
        &api.sources(),
        &pack_items(),
        SourceId::Modrinth,
        CREATE_PLUS,
    )
    .await
    .unwrap();

    let ranged = file.ranged.load(Ordering::Relaxed);
    let sent = file.sent.load(Ordering::Relaxed);
    assert!(ranged <= 6, "{ranged} pedidos Range");
    assert_eq!(
        file.requests.load(Ordering::Relaxed),
        ranged,
        "todo pedido foi parcial"
    );
    assert!(sent < data.len() / 10, "{sent} de {} bytes", data.len());
    assert!(!contents.whole_download);
    // Os 155 arquivos do índice vêm do CDN do Modrinth: 150 em `mods/` (os de `resourcepacks/`
    // ficam fora) e os dois jars soltos em `overrides/mods/` ficam fora das lojas.
    assert_eq!(count(&contents, ModpackModState::External), 2);
    assert_eq!(
        count(&contents, ModpackModState::Compatible)
            + count(&contents, ModpackModState::InPack)
            + count(&contents, ModpackModState::NoVersion),
        150
    );
    assert_eq!(find(&contents, "Create").state, ModpackModState::InPack);
}

/// CA-T08-15: servidor sem `Range` → o arquivo inteiro (até 200 MB), com aviso.
#[tokio::test]
async fn servidor_sem_range_baixa_o_mrpack_inteiro_com_aviso() {
    let files = MockServer::start().await;
    let file = FileServer::new(mrpack(INDEX), false);
    Mock::given(path("/data/create-plus.mrpack"))
        .respond_with(file.clone())
        .mount(&files)
        .await;
    let api = without_embedded(&format!("{}/data/create-plus.mrpack", files.uri())).await;
    let contents = open(
        &api.sources(),
        &pack_items(),
        SourceId::Modrinth,
        CREATE_PLUS,
    )
    .await
    .unwrap();
    assert!(contents.whole_download);
    assert_eq!(file.requests.load(Ordering::Relaxed), 1);
    assert_eq!(count(&contents, ModpackModState::External), 2);
}

/// CA-T08-15: sem `Range` e acima do limite, recusa (`MODPACK_TOO_LARGE`) sem ler o corpo todo.
#[tokio::test]
async fn sem_range_e_acima_do_limite_recusa() {
    let files = MockServer::start().await;
    let file = FileServer::new(mrpack(INDEX), false);
    Mock::given(path("/grande.mrpack"))
        .respond_with(file)
        .mount(&files)
        .await;
    let options = RangeOptions {
        max_whole: 1024 * 1024,
        ..RangeOptions::default()
    };
    let error = read_entry(
        &http(),
        SourceId::Modrinth,
        &format!("{}/grande.mrpack", files.uri()),
        "modrinth.index.json",
        options,
        &CancellationToken::new(),
    )
    .await
    .unwrap_err();
    match error {
        ModpackFailure::Modpack(error) => {
            assert_eq!(error.code, DiscoveryErrorCode::ModpackTooLarge);
            assert_eq!(error.params.get("size").map(String::as_str), Some("1"));
        }
        other @ ModpackFailure::Source(_) => panic!("{other:?}"),
    }
}

/// Um zip sem a entrada pedida: `MODPACK_UNREADABLE`.
#[tokio::test]
async fn zip_sem_o_indice_e_ilegivel() {
    let files = MockServer::start().await;
    Mock::given(path("/x.zip"))
        .respond_with(FileServer::new(curseforge_zip(None), true))
        .mount(&files)
        .await;
    let error = read_entry(
        &http(),
        SourceId::Modrinth,
        &format!("{}/x.zip", files.uri()),
        "modrinth.index.json",
        RangeOptions::default(),
        &CancellationToken::new(),
    )
    .await
    .unwrap_err();
    assert!(
        matches!(&error, ModpackFailure::Modpack(e) if e.code == DiscoveryErrorCode::ModpackUnreadable),
        "{error:?}"
    );
}

/// Um zip com bytes antes do primeiro arquivo (um autoextraível, por exemplo) não abre com o
/// deslocamento 0 que o modpack comum usa: a leitura cai na detecção da zip 9 e ainda acha a
/// entrada.
#[tokio::test]
async fn zip_com_bytes_antes_ainda_e_lido() {
    let manifest = json!({ "manifestType": "minecraftModpack" });
    let mut data: Vec<u8> = (0..4_096).map(junk_byte).collect();
    data.extend(curseforge_zip(Some(&manifest)));
    let files = MockServer::start().await;
    Mock::given(path("/x.zip"))
        .respond_with(FileServer::new(data, true))
        .mount(&files)
        .await;
    let entry = read_entry(
        &http(),
        SourceId::Curseforge,
        &format!("{}/x.zip", files.uri()),
        "manifest.json",
        RangeOptions::default(),
        &CancellationToken::new(),
    )
    .await
    .unwrap();
    let read: Value = serde_json::from_slice(&entry.bytes).unwrap();
    assert_eq!(read, manifest);
    assert!(entry.names.iter().any(|name| name == "overrides/mods/feito-a-mao.jar"));
}

// ---------- Arquivos que não são modpack (revisão da P1-17) ----------

/// Byte `i` do "lixo": nunca forma `PK`.
fn junk_byte(i: u64) -> u8 {
    match u8::try_from(i % 251).unwrap_or(0) {
        b'P' => b'Q',
        byte => byte,
    }
}

/// Lixo de qualquer tamanho declarado (até "1 GB"), servido com `Range`; os bytes são gerados
/// a cada pedido. `end` substitui os últimos bytes (um fim de diretório central falso).
#[derive(Clone)]
struct Junk {
    len: u64,
    end: Arc<Vec<u8>>,
    requests: Arc<AtomicUsize>,
    sent: Arc<AtomicUsize>,
}

impl Junk {
    fn new(len: u64, end: Vec<u8>) -> Self {
        Self {
            len,
            end: Arc::new(end),
            requests: Arc::default(),
            sent: Arc::default(),
        }
    }

    fn byte(&self, i: u64) -> u8 {
        let from = self.len - self.end.len() as u64;
        if i >= from {
            self.end[usize::try_from(i - from).unwrap()]
        } else {
            junk_byte(i)
        }
    }
}

impl Respond for Junk {
    fn respond(&self, request: &Request) -> ResponseTemplate {
        if request.method.as_str() == "HEAD" {
            return ResponseTemplate::new(200);
        }
        self.requests.fetch_add(1, Ordering::Relaxed);
        let Some((start, end)) = request
            .headers
            .get("range")
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.strip_prefix("bytes="))
            .and_then(|spec| spec.split_once('-'))
        else {
            return ResponseTemplate::new(500);
        };
        let (start, end) = if start.is_empty() {
            let suffix: u64 = end.parse().unwrap();
            (self.len.saturating_sub(suffix), self.len - 1)
        } else {
            let end: u64 = end.parse().unwrap();
            (start.parse().unwrap(), end.min(self.len - 1))
        };
        let body: Vec<u8> = (start..=end).map(|i| self.byte(i)).collect();
        self.sent.fetch_add(body.len(), Ordering::Relaxed);
        ResponseTemplate::new(206)
            .insert_header("content-range", format!("bytes {start}-{end}/{}", self.len))
            .set_body_bytes(body)
    }
}

async fn read_junk(junk: &Junk) -> ModpackFailure {
    let files = MockServer::start().await;
    Mock::given(path("/lixo.mrpack"))
        .respond_with(junk.clone())
        .mount(&files)
        .await;
    read_entry(
        &http(),
        SourceId::Modrinth,
        &format!("{}/lixo.mrpack", files.uri()),
        "modrinth.index.json",
        RangeOptions::default(),
        &CancellationToken::new(),
    )
    .await
    .unwrap_err()
}

fn assert_unreadable(error: &ModpackFailure) {
    assert!(
        matches!(error, ModpackFailure::Modpack(e) if e.code == DiscoveryErrorCode::ModpackUnreadable),
        "{error:?}"
    );
}

/// Revisão: 300 KB de lixo eram 232 pedidos; 3 MB, 2.877; "1 GB", mais de 31 mil em 60 s.
/// Sem o fim do diretório central no fim do arquivo, é recusado com o primeiro pedido.
#[tokio::test]
async fn lixo_que_nao_e_zip_e_recusado_no_primeiro_pedido() {
    for len in [300_000_u64, 3_000_000, 1024 * 1024 * 1024] {
        let started = std::time::Instant::now();
        let junk = Junk::new(len, Vec::new());
        let error = read_junk(&junk).await;
        assert_unreadable(&error);
        assert_eq!(junk.requests.load(Ordering::Relaxed), 1, "{len} bytes");
        assert!(junk.sent.load(Ordering::Relaxed) <= 128 * 1024);
        assert!(started.elapsed() < std::time::Duration::from_secs(5));
    }
}

/// Um fim de diretório central falso no fim de "1 GB" de lixo (o `zip` procura o diretório
/// central e outro fim pelo arquivo inteiro): para no teto de pedidos e de bytes, rápido.
#[tokio::test]
async fn zip_falso_de_1_gb_para_no_teto_de_pedidos() {
    let mut end = Vec::new();
    end.extend_from_slice(b"PK\x05\x06");
    end.extend_from_slice(&[0, 0, 0, 0]); // disco, disco do diretório
    end.extend_from_slice(&1_u16.to_le_bytes()); // entradas neste disco
    end.extend_from_slice(&1_u16.to_le_bytes()); // entradas
    end.extend_from_slice(&46_u32.to_le_bytes()); // tamanho do diretório
    end.extend_from_slice(&0_u32.to_le_bytes()); // começo do diretório
    end.extend_from_slice(&0_u16.to_le_bytes()); // comentário
    let started = std::time::Instant::now();
    let junk = Junk::new(1024 * 1024 * 1024, end);
    let error = read_junk(&junk).await;
    assert_unreadable(&error);
    let requests = junk.requests.load(Ordering::Relaxed) as u64;
    assert!(
        requests <= warden_http::range_reader::MAX_REQUESTS,
        "{requests} pedidos"
    );
    assert!(
        junk.sent.load(Ordering::Relaxed) as u64
            <= warden_http::range_reader::MAX_READ_BYTES + 128 * 1024
    );
    assert!(
        started.elapsed() < std::time::Duration::from_secs(20),
        "{:?}",
        started.elapsed()
    );
}

/// Cancelar (fechar o painel, trocar de modpack) interrompe a leitura do arquivo do modpack
/// na hora, e o servidor vê a conexão fechada.
#[tokio::test]
async fn cancelar_interrompe_a_leitura_do_modpack() {
    use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};

    // Servidor que responde o cabeçalho e para: a leitura só termina se for cancelada.
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let (closed_tx, closed_rx) = tokio::sync::oneshot::channel::<()>();
    tokio::spawn(async move {
        let mut closed_tx = Some(closed_tx);
        while let Ok((mut socket, _)) = listener.accept().await {
            let mut buffer = [0_u8; 4096];
            loop {
                let mut request = Vec::new();
                while !request.windows(4).any(|w| w == b"\r\n\r\n") {
                    match socket.read(&mut buffer).await {
                        Ok(0) | Err(_) => break,
                        Ok(n) => request.extend_from_slice(&buffer[..n]),
                    }
                }
                if request.is_empty() {
                    break;
                }
                let head = "HTTP/1.1 200 OK\r\ncontent-length: 1000000\r\n\r\n";
                if socket.write_all(head.as_bytes()).await.is_err() {
                    break;
                }
                if request.starts_with(b"HEAD") {
                    continue;
                }
                let _ = socket.write_all(&[7_u8; 1024]).await;
                while matches!(socket.read(&mut buffer).await, Ok(n) if n > 0) {}
                if let Some(tx) = closed_tx.take() {
                    let _ = tx.send(());
                }
                break;
            }
        }
    });
    let api = without_embedded(&format!("http://{address}/parado.mrpack")).await;
    let sources = api.sources();
    let items = pack_items();
    let target = neoforge_1_21_1();
    let cancel = CancellationToken::new();
    let trigger = cancel.clone();
    tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        trigger.cancel();
    });
    let started = std::time::Instant::now();
    let pack = PackView {
        target: &target,
        items: &items,
    };
    let result = modpack_contents(&sources, &pack, SourceId::Modrinth, CREATE_PLUS, &cancel).await;
    assert!(result.is_err());
    assert!(
        started.elapsed() < std::time::Duration::from_secs(5),
        "{:?}",
        started.elapsed()
    );
    tokio::time::timeout(std::time::Duration::from_secs(5), closed_rx)
        .await
        .expect("a conexão fechou")
        .expect("aviso do servidor");
}

// ---------- CurseForge ----------

const CF_PACK: u64 = 900_100;
const CF_PACK_FILE: u64 = 6_000_001;
const CF_MARKER: &str = "Manifesto-do-Teste-P117";

/// O zip de um modpack da CurseForge: `manifest.json` com três mods (um já no pack pelo
/// Modrinth, um compatível, um só para Fabric) e um jar solto em `overrides/mods/`.
fn curseforge_zip(manifest: Option<&Value>) -> Vec<u8> {
    let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default();
    for i in 0..40 {
        zip.start_file(format!("overrides/config/c{i}.toml"), options)
            .unwrap();
        zip.write_all(&[b'x'; 2_000]).unwrap();
    }
    zip.start_file("overrides/mods/feito-a-mao.jar", options)
        .unwrap();
    zip.write_all(b"PK").unwrap();
    if let Some(manifest) = manifest {
        zip.start_file("manifest.json", options).unwrap();
        zip.write_all(manifest.to_string().as_bytes()).unwrap();
    }
    zip.finish().unwrap().into_inner()
}

fn cf_manifest() -> Value {
    json!({
        "minecraft": {"version": "1.21.1", "modLoaders": [{"id": "neoforge-21.1.77", "primary": true}]},
        "manifestType": "minecraftModpack", "manifestVersion": 1,
        "name": CF_MARKER, "version": "2.3.0", "author": "harborlight",
        "files": [
            {"projectID": 1001, "fileID": 7001, "required": true},
            {"projectID": 1002, "fileID": 7002, "required": true},
            {"projectID": 1003, "fileID": 7003, "required": true}
        ],
        "overrides": "overrides"
    })
}

fn cf_mod(id: u64, name: &str, slug: &str, loader: u32) -> Value {
    json!({
        "id": id, "gameId": 432, "name": name, "slug": slug, "summary": "resumo", "status": 4,
        "downloadCount": 10, "classId": 6, "authors": [{"id": 1, "name": "autor"}],
        "latestFilesIndexes": [{"gameVersion": "1.21.1", "fileId": id + 6000,
                                "filename": "a.jar", "releaseType": 1, "modLoader": loader}],
        "mainFileId": id + 6000
    })
}

fn cf_file(id: u64, mod_id: u64) -> Value {
    json!({
        "id": id, "gameId": 432, "modId": mod_id, "isAvailable": true,
        "displayName": format!("arquivo {id}"), "fileName": format!("mod-{id}.jar"),
        "releaseType": 1, "fileStatus": 4,
        "hashes": [{"value": format!("{id:040}"), "algo": 1}],
        "fileLength": 10, "downloadCount": 1, "gameVersions": ["1.21.1", "NeoForge"],
        "fileFingerprint": 1
    })
}

struct Curseforge {
    api: MockServer,
    edge_ranged: Arc<AtomicUsize>,
    file: FileServer,
    client: CurseforgeClient,
    _servers: Vec<MockServer>,
}

impl Curseforge {
    async fn start(allow_distribution: bool, download_url: bool) -> Self {
        let api = MockServer::start().await;
        let media = MockServer::start().await;
        let file = FileServer::new(curseforge_zip(Some(&cf_manifest())), true);
        Mock::given(path("/files/6000/001/pack.zip"))
            .respond_with(file.clone())
            .mount(&media)
            .await;
        let edge = MockServer::start().await;
        let edge_ranged = Arc::new(AtomicUsize::new(0));
        Mock::given(path("/files/6000/001/pack.zip"))
            .respond_with(Edge {
                target: format!("{}/files/6000/001/pack.zip", media.uri()),
                ranged: Arc::clone(&edge_ranged),
            })
            .mount(&edge)
            .await;
        let mut modpack = json!({
            "id": CF_PACK, "gameId": 432, "name": "Cozy Create", "slug": "cozy-create",
            "summary": "Create com fazendas.", "status": 4, "downloadCount": 1_200_000,
            "classId": 4471, "authors": [{"id": 9, "name": "harborlight"}],
            "links": {"websiteUrl": "https://www.curseforge.com/minecraft/modpacks/cozy-create"},
            "latestFilesIndexes": [
                {"gameVersion": "1.21.1", "fileId": CF_PACK_FILE, "filename": "pack.zip",
                 "releaseType": 1, "modLoader": 6},
                {"gameVersion": "1.20.1", "fileId": 5_000_000, "filename": "old.zip",
                 "releaseType": 1, "modLoader": 1}
            ],
            "mainFileId": CF_PACK_FILE
        });
        if !allow_distribution {
            modpack["allowModDistribution"] = json!(false);
        }
        Mock::given(method("GET"))
            .and(path(format!("/v1/mods/{CF_PACK}")))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"data": modpack})))
            .mount(&api)
            .await;
        let url = download_url.then(|| format!("{}/files/6000/001/pack.zip", edge.uri()));
        Mock::given(method("GET"))
            .and(path(format!(
                "/v1/mods/{CF_PACK}/files/{CF_PACK_FILE}/download-url"
            )))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"data": url})))
            .mount(&api)
            .await;
        let mut pack_file = cf_file(CF_PACK_FILE, CF_PACK);
        pack_file["downloadUrl"] = Value::Null;
        Mock::given(method("GET"))
            .and(path(format!("/v1/mods/{CF_PACK}/files/{CF_PACK_FILE}")))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"data": pack_file})))
            .mount(&api)
            .await;
        Mock::given(method("POST"))
            .and(path("/v1/mods"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"data": [
                cf_mod(1001, "Create", "create", 6),
                cf_mod(1002, "Storage Drawers", "storage-drawers", 6),
                cf_mod(1003, "Só Fabric", "so-fabric", 4),
            ]})))
            .mount(&api)
            .await;
        Mock::given(method("POST"))
            .and(path("/v1/mods/files"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"data": [
                cf_file(7001, 1001), cf_file(7002, 1002), cf_file(7003, 1003),
            ]})))
            .mount(&api)
            .await;
        let client = CurseforgeClient::with_base_url(
            http(),
            &format!("{}/v1", api.uri()),
            Some("chave-de-teste".to_owned().into()),
        )
        .unwrap();
        Self {
            api,
            edge_ranged,
            file,
            client,
            _servers: vec![media, edge],
        }
    }

    fn sources(&self) -> ModpackSources {
        ModpackSources {
            modrinth: ModrinthClient::with_base_url(http(), "https://127.0.0.1:9/v2", None)
                .unwrap(),
            curseforge: Some(self.client.clone()),
            http: http(),
        }
    }
}

/// CA-T08-15/16: o `manifest.json` lido por partes no servidor final (o "edge" nunca recebe
/// `Range`), com os estados dos mods; nada vai para o disco (a função não recebe pasta nenhuma
/// e o zip fica em memória; a inspeção da pasta de dados depois da sessão é do E2E).
#[tokio::test]
async fn modpack_da_curseforge_le_o_manifesto_por_partes() {
    let cf = Curseforge::start(true, true).await;
    let contents = open(
        &cf.sources(),
        &pack_items(),
        SourceId::Curseforge,
        &CF_PACK.to_string(),
    )
    .await
    .unwrap();

    assert_eq!(
        cf.edge_ranged.load(Ordering::Relaxed),
        0,
        "nenhum Range no edge"
    );
    let ranged = cf.file.ranged.load(Ordering::Relaxed);
    assert!((1..=6).contains(&ranged), "{ranged} pedidos Range");
    assert_eq!(cf.file.requests.load(Ordering::Relaxed), ranged);
    assert_eq!(contents.title, "Cozy Create");
    assert_eq!(contents.version_number, "2.3.0");
    assert!(contents.same_target);
    assert_eq!(contents.loaders, ["neoforge"]);
    assert_eq!(contents.license.name, None);
    assert!(!contents.license.free);
    assert_eq!(
        contents.page_url.as_deref(),
        Some("https://www.curseforge.com/minecraft/modpacks/cozy-create")
    );
    // O Create já está no pack pelo Modrinth (deduplicação pelo nome).
    let create = find(&contents, "Create");
    assert_eq!(create.state, ModpackModState::InPack);
    let drawers = find(&contents, "Storage Drawers");
    assert_eq!(drawers.state, ModpackModState::Compatible);
    assert_eq!(drawers.key, "curseforge:1002");
    assert_eq!(drawers.version_id.as_deref(), Some("7002"));
    assert_eq!(drawers.file_name.as_deref(), Some("mod-7002.jar"));
    assert_eq!(
        find(&contents, "Só Fabric").state,
        ModpackModState::NoVersion
    );
    assert_eq!(
        find(&contents, "feito-a-mao").state,
        ModpackModState::External
    );
    assert_eq!(contents.mods.len(), 4);
    // A API foi consultada só pelo necessário: projeto, endereço de download e os dois lotes.
    let api_requests = cf.api.received_requests().await.unwrap_or_default().len();
    assert_eq!(api_requests, 4);
}

/// "Este modpack não pode ser aberto por apps de terceiros": `allowModDistribution` falso.
#[tokio::test]
async fn modpack_bloqueado_a_terceiros() {
    let cf = Curseforge::start(false, true).await;
    let error = open(
        &cf.sources(),
        &pack_items(),
        SourceId::Curseforge,
        &CF_PACK.to_string(),
    )
    .await
    .unwrap_err();
    let ModpackFailure::Modpack(error) = error else {
        panic!("{error:?}")
    };
    assert_eq!(error.code, DiscoveryErrorCode::ModpackBlocked);
    assert_eq!(
        error.params.get("page").map(String::as_str),
        Some("https://www.curseforge.com/minecraft/modpacks/cozy-create")
    );
    assert_eq!(
        cf.file.requests.load(Ordering::Relaxed),
        0,
        "nada foi baixado"
    );
}

/// O mesmo quando só o arquivo vem sem `downloadUrl`.
#[tokio::test]
async fn arquivo_do_modpack_sem_download_url_tambem_e_bloqueado() {
    let cf = Curseforge::start(true, false).await;
    let error = open(
        &cf.sources(),
        &pack_items(),
        SourceId::Curseforge,
        &CF_PACK.to_string(),
    )
    .await
    .unwrap_err();
    assert!(
        matches!(&error, ModpackFailure::Modpack(e) if e.code == DiscoveryErrorCode::ModpackBlocked),
        "{error:?}"
    );
}

/// Sem a CurseForge ligada (sem chave), nenhuma requisição: fonte indisponível.
#[tokio::test]
async fn sem_curseforge_ligada_e_fonte_indisponivel() {
    let api = Modrinth::start(json!([])).await;
    let error = open(&api.sources(), &pack_items(), SourceId::Curseforge, "1")
        .await
        .unwrap_err();
    let ModpackFailure::Source(error) = error else {
        panic!("{error:?}")
    };
    assert_eq!(
        error.code,
        warden_project::ProjectErrorCode::SearchSourceUnavailable
    );
    assert_eq!(
        error.params.get("source").map(String::as_str),
        Some("CurseForge")
    );
}

#[test]
fn licencas_livres_e_jars_soltos() {
    assert!(super::is_free_license("LGPL-3.0-only"));
    assert!(super::is_free_license("MIT"));
    assert!(super::is_free_license("GPL-3.0-or-later"));
    assert!(!super::is_free_license("LicenseRef-All-Rights-Reserved"));
    assert!(!super::is_free_license(""));
    let names = [
        "overrides/mods/a.jar".to_owned(),
        "overrides/config/a.jar".to_owned(),
        "client-overrides/mods/b.JAR".to_owned(),
        "overrides/mods/c.txt".to_owned(),
    ];
    assert_eq!(
        super::override_jars(&names),
        ["client-overrides/mods/b.JAR", "overrides/mods/a.jar"]
    );
}
