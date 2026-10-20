//! Testes dos comandos do diagnóstico (D-03): montagem da entrada com o pack de verdade (o
//! packwiz real grava o índice), o relatório com os itens ligados, "Ignorar neste pack", a
//! marca de relatório velho, a redação do que vai para a tela e as correções "Adicionar
//! dependência" (o projeto da fonte) e "Mudar lado" (o mesmo "Alterar lado" da lista de Mods),
//! com um Modrinth simulado.

use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};

use warden_core::PackId;
use warden_diagnostics::{
    Evidence, Finding, ItemRef, RedactionProfile, Redactor, RuleCode, Severity, SuggestedFix,
};
use warden_packwiz_cli::Packwiz;
use warden_project::registry::PackRecord;
use warden_project::transaction::PackTransaction;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::report::{
    CheckPass, ItemNames, ReportItem, finding_id, parse_ignored, redact_finding, with_ignored,
};
use super::*;
use crate::commands::inventory::tests::{packwiz_binary, test_packwiz};
use crate::state::tests::test_state;

const MODRINTH_FIXTURES: &str = "../../../crates/warden-modrinth/tests/fixtures/http";

fn fixture(name: &str) -> serde_json::Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(MODRINTH_FIXTURES)
        .join(name);
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

/// Um jar Fabric com o `fabric.mod.json` dado.
fn fabric_jar(json: &serde_json::Value) -> Vec<u8> {
    let mut bytes = std::io::Cursor::new(Vec::new());
    let mut zip = zip::ZipWriter::new(&mut bytes);
    zip.start_file("fabric.mod.json", zip::write::SimpleFileOptions::default())
        .unwrap();
    zip.write_all(json.to_string().as_bytes()).unwrap();
    zip.finish().unwrap();
    bytes.into_inner()
}

/// O jar de "Meu Mod", que exige o Sodium (e o Fabric Loader, que vem com o loader).
fn meu_mod_jar() -> Vec<u8> {
    fabric_jar(&serde_json::json!({
        "schemaVersion": 1,
        "id": "meumod",
        "name": "Meu Mod",
        "version": "1.0.0",
        "environment": "*",
        "depends": { "fabricloader": ">=0.15", "sodium": "*" }
    }))
}

/// O metafile do Sodium (versão gravada `SMxNOGZ6`) com o lado dado.
fn sodium_metafile(side: &str) -> String {
    format!(
        "name = \"Sodium\"\nfilename = \"sodium-fabric-0.8.13+mc1.21.1.jar\"\nside = \"{side}\"\n\n\
         [download]\nurl = \"https://cdn.modrinth.com/data/AANobbMI/versions/SMxNOGZ6/sodium-fabric-0.8.13%2Bmc1.21.1.jar\"\n\
         hash-format = \"sha512\"\nhash = \"8614a374593698069a80ac74de8cd9c28f3928d16819a0e0d6320b538e823f7e54e8be06d5ea5a9c268beb0e02073209d1f831a02dede851588b972367680853\"\n\n\
         [update]\n[update.modrinth]\nmod-id = \"AANobbMI\"\nversion = \"SMxNOGZ6\"\n"
    )
}

/// Um pack Fabric 1.21.1 com os arquivos dados; o packwiz real grava o índice.
async fn fabric_pack(
    state: &AppState,
    dir: &Path,
    cli: &Packwiz,
    files: &[(&str, &[u8])],
) -> (PackId, PathBuf) {
    let root = dir.join("pack");
    fs::create_dir_all(&root).unwrap();
    let mut manifest = warden_packwiz::PackManifest::new("Vale D03", "1.21.1");
    manifest
        .versions
        .get_or_insert_with(BTreeMap::new)
        .insert("fabric".into(), "0.16.10".into());
    fs::write(root.join("pack.toml"), manifest.to_toml_string()).unwrap();
    fs::write(
        root.join("index.toml"),
        warden_packwiz::PackIndex::default().to_toml_string(),
    )
    .unwrap();
    for (path, bytes) in files {
        let file = root.join(path);
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(file, bytes).unwrap();
    }
    PackTransaction::new(root.clone())
        .commit(cli, &warden_core::CancellationToken::new())
        .await
        .unwrap();
    let record = PackRecord {
        id: PackId::new(),
        name: "Vale D03".into(),
        path: root.clone(),
        last_test: None,
        extra: BTreeMap::new(),
    };
    let id = record.id;
    state.packs.insert(record).unwrap();
    (id, root)
}

/// Estado de teste com o packwiz real, ou `None` sem o sidecar.
fn setup() -> Option<(tempfile::TempDir, AppState, Packwiz)> {
    let binary = packwiz_binary()?;
    let dir = tempfile::tempdir().unwrap();
    let state = test_state(dir.path());
    let cli = test_packwiz(&state, binary);
    Some((dir, state, cli))
}

/// Modrinth simulado com as respostas gravadas do Sodium.
async fn modrinth(state: &mut AppState) -> MockServer {
    let server = MockServer::start().await;
    let version = fixture("2026-10-04-version-SMxNOGZ6.json");
    let project = fixture("2026-10-04-project-sodium.json");
    Mock::given(method("GET"))
        .and(path("/versions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([version])))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/projects"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!([project.clone()])),
        )
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/project/sodium"))
        .respond_with(ResponseTemplate::new(200).set_body_json(project))
        .mount(&server)
        .await;
    let http = warden_http::HttpClient::new(warden_http::HttpConfig::for_version("0.0.0")).unwrap();
    state.modrinth = warden_modrinth::ModrinthClient::with_base_url(
        http,
        &format!("{}/", server.uri()),
        Some(warden_modrinth::MetadataCache::in_memory(None).unwrap()),
    )
    .unwrap();
    server
}

const MEZZ_SHA1: &str = "5555555555555555555555555555555555555555";

/// O metafile do Modrinth de um projeto e versão simulados.
fn modrinth_metafile(name: &str, project: &str, version: &str) -> String {
    format!(
        "name = \"{name}\"\nfilename = \"{project}.jar\"\nside = \"both\"\n\n\
         [download]\nurl = \"https://cdn.modrinth.com/data/{project}/versions/{version}/{project}.jar\"\n\
         hash-format = \"sha512\"\nhash = \"{}\"\n\n\
         [update]\n[update.modrinth]\nmod-id = \"{project}\"\nversion = \"{version}\"\n",
        "ab".repeat(64)
    )
}

/// O metafile da CurseForge de um projeto e arquivo simulados.
fn curseforge_metafile(name: &str, project: u64, file: u64, sha1: &str) -> String {
    format!(
        "name = \"{name}\"\nfilename = \"{project}.jar\"\nside = \"both\"\n\n\
         [download]\nhash-format = \"sha1\"\nhash = \"{sha1}\"\nmode = \"metadata:curseforge\"\n\n\
         [update]\n[update.curseforge]\nfile-id = {file}\nproject-id = {project}\n"
    )
}

/// O Modrinth simulado do caso do `MezzConfig`: o JEI pede o `mrMezz001`, achado pelo SHA-1.
async fn mezz_modrinth(state: &mut AppState) -> MockServer {
    let modrinth = MockServer::start().await;
    let file = |sha1: &str| {
        serde_json::json!([{
            "hashes": {"sha1": sha1, "sha512": "ab".repeat(64)},
            "url": "https://cdn.modrinth.com/x.jar", "filename": "x.jar",
            "primary": true, "size": 1
        }])
    };
    let version = |id: &str, project: &str, sha1: &str, deps: serde_json::Value| {
        serde_json::json!({
            "id": id, "project_id": project, "name": id, "version_number": "1.0.0",
            "game_versions": ["1.21.1"], "loaders": ["fabric"], "version_type": "release",
            "status": "listed", "featured": false, "downloads": 1,
            "date_published": "2026-09-01T00:00:00Z", "files": file(sha1), "dependencies": deps
        })
    };
    let jei = version(
        "vJei00001",
        "mrJei0001",
        "4444444444444444444444444444444444444444",
        serde_json::json!([{"project_id": "mrMezz001", "dependency_type": "required"}]),
    );
    let mezz = version("vMezz0001", "mrMezz001", MEZZ_SHA1, serde_json::json!([]));
    Mock::given(method("GET"))
        .and(path("/versions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([jei])))
        .mount(&modrinth)
        .await;
    Mock::given(method("GET"))
        .and(path("/projects"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([{
            "id": "mrJei0001", "slug": "jei", "project_type": "mod",
            "title": "Just Enough Items", "status": "approved",
            "client_side": "required", "server_side": "required"
        }])))
        .mount(&modrinth)
        .await;
    Mock::given(method("POST"))
        .and(path("/version_files"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!({ MEZZ_SHA1: mezz })),
        )
        .mount(&modrinth)
        .await;
    let http = warden_http::HttpClient::new(warden_http::HttpConfig::for_version("0.0.0")).unwrap();
    state.modrinth = warden_modrinth::ModrinthClient::with_base_url(
        http,
        &format!("{}/", modrinth.uri()),
        Some(warden_modrinth::MetadataCache::in_memory(None).unwrap()),
    )
    .unwrap();
    modrinth
}

/// A CurseForge simulada (com a chave de teste) do caso do `MezzConfig`.
async fn mezz_curseforge(state: &mut AppState) -> MockServer {
    let curseforge = MockServer::start().await;
    let cf_file = |id: u64, mod_id: u64, sha1: &str| {
        serde_json::json!({
            "id": id, "gameId": 432, "modId": mod_id, "isAvailable": true,
            "displayName": "x", "fileName": "x.jar", "releaseType": 1,
            "hashes": [{"value": sha1, "algo": 1}],
            "downloadUrl": "https://edge.forgecdn.net/files/x.jar",
            "gameVersions": ["1.21.1", "Fabric"], "dependencies": []
        })
    };
    let cf_project = |id: u64, slug: &str, name: &str| {
        serde_json::json!({
            "id": id, "gameId": 432, "name": name, "slug": slug, "classId": 6,
            "links": {"websiteUrl": format!("https://www.curseforge.com/minecraft/mc-mods/{slug}")}
        })
    };
    Mock::given(method("POST"))
        .and(path("/v1/mods/files"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"data": [
            cf_file(9_200_020, 920_002, MEZZ_SHA1),
            cf_file(9_200_010, 920_001, "3333333333333333333333333333333333333333"),
        ]})))
        .mount(&curseforge)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/mods"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"data": [
            cf_project(920_002, "mezzconfig", "MezzConfig"),
            cf_project(920_001, "jei", "Just Enough Items (JEI)"),
        ]})))
        .mount(&curseforge)
        .await;
    state.tests = std::sync::Arc::new(
        crate::test_session::TestSessions::new(&state.paths, &state.catalog)
            .with_curseforge_base(format!("{}/v1/", curseforge.uri())),
    );
    state
        .secrets
        .set(
            warden_secrets::SecretKind::Curseforge,
            &secrecy::SecretString::from("chave-de-teste".to_owned()),
        )
        .unwrap();
    curseforge
}

/// P4 de ponta a ponta, na passagem rápida: o JEI do Modrinth pede o `mrMezz001`, que está no
/// pack pela CurseForge (o Modrinth acha o arquivo pelo SHA-1): nenhuma dependência faltando.
/// E o JEI que também veio pela CurseForge é acusado como duplicado pelo slug, que a CurseForge
/// agora informa.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn dependencia_pela_outra_fonte_e_duplicata_pelo_slug_da_curseforge() {
    let Some((dir, mut state, cli)) = setup() else {
        return;
    };
    let _modrinth = mezz_modrinth(&mut state).await;
    let _curseforge = mezz_curseforge(&mut state).await;

    let jei = modrinth_metafile("Just Enough Items", "mrJei0001", "vJei00001");
    let mezz = curseforge_metafile("MezzConfig", 920_002, 9_200_020, MEZZ_SHA1);
    let jei_cf = curseforge_metafile(
        "Just Enough Items (JEI)",
        920_001,
        9_200_010,
        "3333333333333333333333333333333333333333",
    );
    let (pack, _root) = fabric_pack(
        &state,
        dir.path(),
        &cli,
        &[
            ("mods/jei.pw.toml", jei.as_bytes()),
            ("mods/mezzconfig.pw.toml", mezz.as_bytes()),
            ("mods/jei-cf.pw.toml", jei_cf.as_bytes()),
        ],
    )
    .await;
    let report = check(&state, pack, CheckPass::Quick, None).await.unwrap();
    let found = rules(&report);
    assert!(!found.contains(&"E_MISSING_DEP"), "{report:#?}");
    assert!(found.contains(&"E_DUPLICATE_FILE"), "{report:#?}");
}

fn rules(report: &DiagnosticsReport) -> Vec<&str> {
    report
        .findings
        .iter()
        .map(|entry| entry.finding.rule().as_str())
        .collect()
}

/// A passagem rápida não lê jars; a completa (Verificar agora e Verificação final) lê o jar
/// guardado no pack e acha a dependência que falta, com o item ligado ao pack, a correção
/// "Adicionar dependência" e a evidência do `fabric.mod.json`. O relatório fica gravado,
/// vale até o pack mudar, e "Ignorar neste pack" grava o achado em `.warden/project.toml`
/// sem bloquear mais o teste.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn completa_acha_dependencia_que_falta_e_ignorar_grava_no_pack() {
    let Some((dir, state, cli)) = setup() else {
        return;
    };
    let jar = meu_mod_jar();
    let (pack, root) = fabric_pack(&state, dir.path(), &cli, &[("mods/meu-mod.jar", &jar)]).await;

    let quick = check(&state, pack, CheckPass::Quick, None).await.unwrap();
    assert!(quick.checked_mods);
    assert_eq!(quick.item_count, 1);
    assert!(!rules(&quick).contains(&"E_MISSING_DEP"), "{quick:#?}");

    let report = check(&state, pack, CheckPass::Complete, None).await.unwrap();
    let missing = report
        .findings
        .iter()
        .find(|entry| entry.finding.rule().as_str() == "E_MISSING_DEP")
        .unwrap_or_else(|| panic!("sem E_MISSING_DEP: {report:#?}"));
    assert_eq!(missing.finding.severity(), Severity::Error);
    assert_eq!(
        missing.items,
        vec![ReportItem {
            path: Some("mods/meu-mod.jar".into()),
            name: "meu-mod.jar".into(),
        }]
    );
    assert!(missing.finding.fixes().iter().any(|fix| matches!(
        fix,
        SuggestedFix::AddDependency { mod_id, .. } if mod_id == "sodium"
    )));
    assert!(matches!(
        missing.finding.primary_evidence(),
        Evidence::JarMetadata { file_in_jar, .. } if file_in_jar == "fabric.mod.json"
    ));
    assert_eq!(report.blocking_errors(), 1);

    // Gravado e ainda valendo.
    let stored = last_report(&state, pack).unwrap().unwrap();
    assert!(!stored.stale);
    assert_eq!(stored.findings.len(), report.findings.len());

    // Ignorar: grava no pack, não bloqueia mais.
    write_ignored(&root, &missing.id, true).unwrap();
    let project = fs::read_to_string(root.join(".warden/project.toml")).unwrap();
    assert!(project.contains("[diagnostics]"), "{project}");
    assert!(project.contains(&missing.id), "{project}");
    let ignored = last_report(&state, pack).unwrap().unwrap();
    assert_eq!(ignored.blocking_errors(), 0);
    assert!(
        ignored
            .findings
            .iter()
            .any(|entry| entry.id == missing.id && entry.ignored)
    );
    write_ignored(&root, &missing.id, false).unwrap();
    assert_eq!(
        last_report(&state, pack).unwrap().unwrap().blocking_errors(),
        1
    );

    // O pack mudou: o relatório ficou velho.
    fs::write(root.join("mods/outro.jar"), meu_mod_jar()).unwrap();
    PackTransaction::new(root.clone())
        .commit(&cli, &warden_core::CancellationToken::new())
        .await
        .unwrap();
    assert!(last_report(&state, pack).unwrap().unwrap().stale);
}

/// Mod do pack sem jar ainda (acabou de entrar pela correção "Adicionar dependência" e não foi
/// baixado): "Verificar agora" não o dá como ausente; vale a passagem rápida, e a completa
/// fica para a "Verificação final" do Testar.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn sem_o_jar_de_um_mod_a_completa_vira_rapida() {
    let Some((dir, mut state, cli)) = setup() else {
        return;
    };
    let _server = modrinth(&mut state).await;
    let jar = meu_mod_jar();
    let meta = sodium_metafile("client");
    let (pack, _root) = fabric_pack(
        &state,
        dir.path(),
        &cli,
        &[
            ("mods/meu-mod.jar", &jar),
            ("mods/sodium.pw.toml", meta.as_bytes()),
        ],
    )
    .await;
    let report = check(&state, pack, CheckPass::Complete, None).await.unwrap();
    assert_eq!(report.pass, CheckPass::Quick);
    assert!(!rules(&report).contains(&"E_MISSING_DEP"), "{report:#?}");
}

/// "Mudar lado" de ponta a ponta: o Modrinth diz que o Sodium só funciona no cliente e o pack o
/// manda para os dois lados (`W_SIDE` com a correção "Só cliente"); o "Alterar lado" da lista
/// de Mods grava `side = "client"` e a verificação seguinte não acha mais o problema.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mudar_lado_de_ponta_a_ponta() {
    let Some((dir, mut state, cli)) = setup() else {
        return;
    };
    let _server = modrinth(&mut state).await;
    let meta = sodium_metafile("both");
    let (pack, root) = fabric_pack(
        &state,
        dir.path(),
        &cli,
        &[("mods/sodium.pw.toml", meta.as_bytes())],
    )
    .await;

    let report = check(&state, pack, CheckPass::Quick, None).await.unwrap();
    let side = report
        .findings
        .iter()
        .find(|entry| entry.finding.rule().as_str() == "W_SIDE")
        .unwrap_or_else(|| panic!("sem W_SIDE: {report:#?}"));
    assert_eq!(side.items[0].name, "Sodium");
    assert_eq!(side.items[0].path.as_deref(), Some("mods/sodium.pw.toml"));
    let fix = side
        .finding
        .fixes()
        .iter()
        .find_map(|fix| match fix {
            SuggestedFix::SetSide { side, .. } => Some(*side),
            _ => None,
        })
        .unwrap();
    assert_eq!(fix, warden_diagnostics::FindingSide::Client);
    assert!(matches!(
        side.finding.primary_evidence(),
        Evidence::Api { field, .. } if field == "server_side"
    ));

    // O mesmo comando do "Alterar lado" da lista de Mods.
    crate::commands::inventory::set_side_impl(
        &state,
        &cli,
        pack,
        &["mods/sodium.pw.toml".to_owned()],
        warden_project::side::SideChoice::Client,
    )
    .await
    .unwrap();
    assert!(
        fs::read_to_string(root.join("mods/sodium.pw.toml"))
            .unwrap()
            .contains("side = \"client\"")
    );
    let again = check(&state, pack, CheckPass::Quick, None).await.unwrap();
    assert!(!rules(&again).contains(&"W_SIDE"), "{again:#?}");
}

/// "Adicionar dependência": o id do mod no jar (`sodium`) vira o projeto do Modrinth, que a
/// interface leva ao diálogo de dependências; um id que a fonte não conhece não vira nada.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn adicionar_dependencia_acha_o_projeto_pelo_id_do_mod() {
    let dir = tempfile::tempdir().unwrap();
    let mut state = test_state(dir.path());
    let _server = modrinth(&mut state).await;
    let found = find_dependency(&state, "sodium", None).await.unwrap();
    assert_eq!(
        found,
        DependencyCandidate {
            source: SourceId::Modrinth,
            project_id: "AANobbMI".into(),
            name: "Sodium".into(),
        }
    );
    assert!(find_dependency(&state, "nao_existe", None).await.is_none());
    assert!(find_dependency(&state, "  ", None).await.is_none());
}

/// Pack sem loader (vanilla): nada de mods para conferir, sem erro.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn pack_vanilla_nao_tem_o_que_conferir() {
    let Some((dir, state, cli)) = setup() else {
        return;
    };
    let root = dir.path().join("pack");
    fs::create_dir_all(&root).unwrap();
    fs::write(
        root.join("pack.toml"),
        warden_packwiz::PackManifest::new("Vanilla", "1.20.1").to_toml_string(),
    )
    .unwrap();
    fs::write(
        root.join("index.toml"),
        warden_packwiz::PackIndex::default().to_toml_string(),
    )
    .unwrap();
    PackTransaction::new(root.clone())
        .commit(&cli, &warden_core::CancellationToken::new())
        .await
        .unwrap();
    let record = PackRecord {
        id: PackId::new(),
        name: "Vanilla".into(),
        path: root,
        last_test: None,
        extra: BTreeMap::new(),
    };
    let id = record.id;
    state.packs.insert(record).unwrap();
    let report = check(&state, id, CheckPass::Complete, None).await.unwrap();
    assert!(!report.checked_mods);
    assert!(report.findings.is_empty());
}

#[test]
fn ignorar_preserva_o_resto_do_project_toml() {
    let before = "# Ferramentas do jogador\n[player-tools]\nspark = \"mods/spark.pw.toml\"\n";
    let after = with_ignored(before, "W_SIDE|mods/xaero.pw.toml", true).unwrap();
    assert!(after.starts_with(before), "{after}");
    assert_eq!(
        parse_ignored(&after).unwrap().into_iter().collect::<Vec<_>>(),
        ["W_SIDE|mods/xaero.pw.toml"]
    );
    // Repetir não duplica; desfazer tira só o achado.
    let twice = with_ignored(&after, "W_SIDE|mods/xaero.pw.toml", true).unwrap();
    assert_eq!(twice, after);
    let undone = with_ignored(&after, "W_SIDE|mods/xaero.pw.toml", false).unwrap();
    assert!(parse_ignored(&undone).unwrap().is_empty());
    assert!(undone.contains("spark = \"mods/spark.pw.toml\""));
    assert!(with_ignored("isto não é [toml", "x", true).is_err());
    assert!(parse_ignored("").unwrap().is_empty());
}

fn finding(rule: &str, items: &[ItemRef]) -> Finding {
    let mut finding = Finding::new(
        RuleCode::parse(rule).unwrap(),
        Severity::Warning,
        "diagnostics.pretest.x",
        Evidence::PackFile {
            path: "pack.toml".into(),
            excerpt: "x".into(),
        },
    );
    for item in items {
        finding = finding.with_item(item.clone());
    }
    finding
}

#[test]
fn id_do_achado_e_estavel_e_nao_depende_da_ordem() {
    let a = ItemRef::Metafile {
        path: "mods/a.pw.toml".into(),
    };
    let b = ItemRef::ModId { id: "balm".into() };
    let resolved = |path: Option<&str>, name: &str| ReportItem {
        path: path.map(ToOwned::to_owned),
        name: name.into(),
    };
    let one = finding_id(
        &finding("E_KNOWN_CONFLICT", &[a.clone(), b.clone()]),
        &[resolved(Some("mods/a.pw.toml"), "A"), resolved(None, "balm")],
    );
    let other = finding_id(
        &finding("E_KNOWN_CONFLICT", &[b, a]),
        &[resolved(None, "balm"), resolved(Some("mods/a.pw.toml"), "A")],
    );
    assert_eq!(one, "E_KNOWN_CONFLICT|mod:balm|mods/a.pw.toml");
    assert_eq!(one, other);
}

#[test]
fn itens_do_log_ligam_ao_pack_pelo_id_e_pelo_arquivo() {
    let items = vec![warden_project::inventory::InventoryItem {
        key: "modrinth:x".into(),
        path: "mods/waystones.pw.toml".into(),
        kind: warden_project::inventory::ItemKind::Mod,
        state: warden_project::inventory::ItemState::Ok,
        name: "Waystones".into(),
        file_name: Some("waystones-14.1.6.jar".into()),
        version: Some("14.1.6".into()),
        source: warden_project::inventory::ItemSource::Modrinth,
        side: warden_project::inventory::ItemSide::Both,
        side_editable: true,
        pinned: false,
        optional: false,
        project_id: None,
        source_version_id: None,
        summary: None,
        icon_url: None,
        error: None,
    }];
    let mod_paths = HashMap::from([("waystones".to_owned(), "mods/waystones.pw.toml".to_owned())]);
    let mod_names = HashMap::from([("balm".to_owned(), "Balm".to_owned())]);
    let names = ItemNames::new(&items, &mod_paths, &mod_names);
    let waystones = ReportItem {
        path: Some("mods/waystones.pw.toml".into()),
        name: "Waystones".into(),
    };
    assert_eq!(
        names.resolve(&ItemRef::ModId {
            id: "waystones".into()
        }),
        waystones
    );
    assert_eq!(
        names.resolve(&ItemRef::JarFile {
            name: "WAYSTONES-14.1.6.jar".into()
        }),
        waystones
    );
    assert_eq!(
        names.resolve(&ItemRef::ModId { id: "balm".into() }),
        ReportItem {
            path: None,
            name: "Balm".into()
        }
    );
    assert_eq!(
        names.resolve(&ItemRef::Metafile {
            path: "mods/sumiu.pw.toml".into()
        }),
        ReportItem {
            path: None,
            name: "sumiu".into()
        }
    );
}

/// O que vai para a tela (e para o "Copiar relatório") não tem o nome do usuário do computador,
/// o do jogador, IPs nem chaves (CA-T14-04 aplicado à tela).
#[test]
fn trechos_e_parametros_vao_redigidos_para_a_tela() {
    let profile = RedactionProfile::from_env_vars(|_| None).with_player("Kriticales");
    let redactor = Redactor::new(profile);
    let log = "[12:00:00] Kriticales entrou de 192.168.0.12 com C:\\Users\\Maria\\AppData\\Roaming e $2a$10$abcdefghijklmnopqrstuv";
    let original = Finding::new(
        RuleCode::parse("E_CRASH_REPORT").unwrap(),
        Severity::Error,
        "diagnostico.achados.crashReport",
        Evidence::log("logs/latest.log", 12, log),
    )
    .with_param("description", "C:\\Users\\Maria\\mundo")
    .with_fix(SuggestedFix::ChangeMemory);
    let redacted = redact_finding(&original, &redactor);
    let Evidence::Log { excerpt, line, .. } = redacted.primary_evidence() else {
        panic!("esperava log");
    };
    assert_eq!(*line, 12);
    for secret in ["Maria", "Kriticales", "192.168.0.12", "$2a$10$"] {
        assert!(!excerpt.contains(secret), "{secret} em {excerpt}");
        assert!(
            !redacted.param("description").unwrap().contains(secret),
            "{secret}"
        );
    }
    assert_eq!(redacted.fixes(), original.fixes());
    assert_eq!(redacted.rule(), original.rule());
}
