//! Testes das ferramentas e dos comandos da IA sobre um pack de verdade numa pasta temporária,
//! com um Gemini simulado (`wiremock`): o texto inicial redigido e byte a byte o enviado
//! (CA-T14-03 e CA-T14-04), cancelar sem rede (CA-T14-05), o log do computador com a conversa
//! guardada fora da pasta do pack e de volta depois de reabrir o app (CA-T14-06), o aviso de pack
//! mudado e apagar (CA-T14-15) e o testador da chave.

// Os roteiros do simulado são `match` pelo número da requisição, como nos da `warden-ai`.
#![allow(clippy::single_match_else)]

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{Value, json};
use sha2::{Digest as _, Sha256};
use warden_ai::conversation::{ConversationOrigin, Symptom};
use warden_ai::evidence::ClaimStatus;
use warden_ai::tools::{AiPermissions, ToolSource as _};
use warden_core::PackId;
use warden_project::registry::PackRecord;
use warden_secrets::{SecretKind, SecretString};
use wiremock::{Mock, MockServer, Request, Respond, ResponseTemplate};

use super::PackTools;
use crate::commands::ai::{
    AiConversationState, AiPreviewRequest, AiSource, GeminiKeyTester, delete_impl, get_impl,
    list_impl, preview_impl, remember_pick, run_impl, send_impl, start_impl,
};
use crate::commands::secrets::SecretTester as _;
use crate::state::AppState;
use crate::state::tests::test_state;

const KEY: &str = "AIza-chave-de-teste-do-simulado-0123456789";

fn sha256(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn metafile(name: &str, version: &str) -> String {
    format!(
        "name = \"{name}\"\nfilename = \"{name}-{version}.jar\"\nside = \"both\"\n\n[download]\nurl = \"https://example.com/{name}-{version}.jar\"\nhash-format = \"sha1\"\nhash = \"{}\"\n",
        sha256(name.as_bytes()).get(..40).unwrap_or_default()
    )
}

/// Um pack Fabric com três mods e uma config.
fn write_pack(root: &Path, versions: [&str; 3]) {
    fs::create_dir_all(root.join("mods")).unwrap();
    fs::create_dir_all(root.join("config")).unwrap();
    let mut index = warden_packwiz::PackIndex::default();
    for (name, version) in ["create", "railways", "flywheel"].into_iter().zip(versions) {
        let text = metafile(name, version);
        let file = format!("mods/{name}.pw.toml");
        fs::write(root.join(&file), &text).unwrap();
        index.files.push(warden_packwiz::IndexEntry::new(
            &file,
            &sha256(text.as_bytes()),
        ));
    }
    let config = "[trains]\nmaxAssemblyLength = 128\n";
    fs::write(root.join("config/create-common.toml"), config).unwrap();
    index.files.push(warden_packwiz::IndexEntry::new(
        "config/create-common.toml",
        &sha256(config.as_bytes()),
    ));
    fs::write(root.join("index.toml"), index.to_toml_string()).unwrap();
    let mut manifest = warden_packwiz::PackManifest::new("Vale D04", "1.20.1");
    manifest
        .versions
        .get_or_insert_with(BTreeMap::new)
        .insert("fabric".into(), "0.16.10".into());
    fs::write(root.join("pack.toml"), manifest.to_toml_string()).unwrap();
}

/// Resumo de todos os arquivos da pasta (caminho e conteúdo).
fn folder_hash(root: &Path) -> String {
    fn walk(dir: &Path, out: &mut Vec<(PathBuf, Vec<u8>)>) {
        let mut entries: Vec<_> = fs::read_dir(dir).unwrap().flatten().collect();
        entries.sort_by_key(fs::DirEntry::path);
        for entry in entries {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, out);
            } else {
                out.push((path.clone(), fs::read(&path).unwrap()));
            }
        }
    }
    let mut files = Vec::new();
    walk(root, &mut files);
    let mut hasher = Sha256::new();
    for (path, bytes) in files {
        hasher.update(path.to_string_lossy().as_bytes());
        hasher.update(&bytes);
    }
    hex::encode(hasher.finalize())
}

fn register(state: &AppState, root: &Path) -> PackId {
    let record = PackRecord {
        id: PackId::new(),
        name: "Vale D04".into(),
        path: root.to_path_buf(),
        last_test: None,
        extra: BTreeMap::new(),
    };
    let id = record.id;
    state.packs.insert(record).unwrap();
    id
}

/// O que o roteiro devolve a cada `generateContent`.
type Script = Box<dyn Fn(usize, &Value) -> Value + Send + Sync>;

/// Gemini simulado: `models.list`, `countTokens` e `generateContent` por roteiro, anotando o
/// corpo de cada requisição como chegou.
struct Gemini {
    seen: Arc<Mutex<Vec<(String, String)>>>,
    script: Script,
    count: Mutex<usize>,
    /// Quanto o `generateContent` demora para responder.
    delay: Arc<Mutex<Duration>>,
}

impl Respond for Gemini {
    fn respond(&self, request: &Request) -> ResponseTemplate {
        let url = request.url.path().to_owned();
        let body = String::from_utf8_lossy(&request.body).into_owned();
        self.seen.lock().unwrap().push((url.clone(), body.clone()));
        if request
            .headers
            .get("x-goog-api-key")
            .and_then(|v| v.to_str().ok())
            != Some(KEY)
        {
            return ResponseTemplate::new(400).set_body_json(json!({"error": {"code": 400,
                "status": "INVALID_ARGUMENT", "details": [{"reason": "API_KEY_INVALID"}]}}));
        }
        if request.method.as_str() == "GET" {
            return ResponseTemplate::new(200).set_body_json(json!({ "models": [
                { "name": "models/gemini-9.1-flash-lite", "supportedGenerationMethods": ["generateContent"] },
                { "name": "models/gemini-9.0-flash-lite", "supportedGenerationMethods": ["generateContent"] },
                { "name": "models/gemini-9.2-flash-preview", "supportedGenerationMethods": ["generateContent"] }
            ]}));
        }
        if url.ends_with(":countTokens") {
            return ResponseTemplate::new(200).set_body_json(json!({ "totalTokens": 1552 }));
        }
        let parsed: Value = serde_json::from_str(&body).unwrap();
        let mut count = self.count.lock().unwrap();
        let reply = (self.script)(*count, &parsed);
        *count += 1;
        ResponseTemplate::new(200)
            .set_body_json(reply)
            .set_delay(*self.delay.lock().unwrap())
    }
}

fn calls(list: &[(&str, Value)]) -> Value {
    let parts: Vec<Value> = list
        .iter()
        .enumerate()
        .map(|(i, (name, args))| {
            let mut part = json!({ "functionCall": { "name": name, "args": args, "id": format!("call_{name}_{i}") } });
            if i == 0 {
                part["thoughtSignature"] = json!("c2ln");
            }
            part
        })
        .collect();
    json!({ "candidates": [{ "content": { "role": "model", "parts": parts }, "finishReason": "STOP" }],
            "usageMetadata": { "promptTokenCount": 2000, "candidatesTokenCount": 40 } })
}

/// A última `functionResponse` de um item da requisição: o primeiro `texto` devolvido.
fn first_text(request: &Value) -> (String, String) {
    let contents = request["contents"].as_array().unwrap();
    let item = &contents.last().unwrap()["parts"][0]["functionResponse"]["response"]["itens"][0];
    (
        item["id"].as_str().unwrap().to_owned(),
        item["texto"].as_str().unwrap().to_owned(),
    )
}

struct Setup {
    dir: tempfile::TempDir,
    data: PathBuf,
    root: PathBuf,
    state: AppState,
    pack: PackId,
    server: MockServer,
    seen: Arc<Mutex<Vec<(String, String)>>>,
    delay: Arc<Mutex<Duration>>,
}

async fn setup(script: Script) -> Setup {
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path().join("dados");
    let state = test_state(&data);
    let root = dir.path().join("pack");
    write_pack(&root, ["6.0.4", "1.6.4", "1.0.2"]);
    let pack = register(&state, &root);
    state
        .secrets
        .set(SecretKind::Gemini, &SecretString::from(KEY))
        .unwrap();
    let server = MockServer::start().await;
    let seen = Arc::new(Mutex::new(Vec::new()));
    let delay = Arc::new(Mutex::new(Duration::ZERO));
    Mock::given(wiremock::matchers::any())
        .respond_with(Gemini {
            seen: Arc::clone(&seen),
            script,
            count: Mutex::new(0),
            delay: Arc::clone(&delay),
        })
        .mount(&server)
        .await;
    *state.ai.base_url.lock().unwrap() = Some(server.uri());
    Setup {
        dir,
        data,
        root,
        state,
        pack,
        server,
        seen,
        delay,
    }
}

fn noop() -> crate::commands::ai::Notify {
    Arc::new(|_, _| {})
}

fn generated(seen: &Arc<Mutex<Vec<(String, String)>>>) -> Vec<String> {
    seen.lock()
        .unwrap()
        .iter()
        .filter(|(url, _)| url.ends_with(":generateContent"))
        .map(|(_, body)| body.clone())
        .collect()
}

#[tokio::test]
async fn ca_t14_05_consentimento_sem_nada_do_pack_e_cancelar_sem_rede() {
    let setup = setup(Box::new(|_, _| unreachable!("nada pode ir à IA"))).await;
    let preview = preview_impl(
        &setup.state,
        setup.pack,
        AiPreviewRequest {
            symptom: Symptom::CrashOnStart,
            source: AiSource::PackOnly,
            note: Some("trava quando entro no Nether".into()),
        },
    )
    .await
    .unwrap();
    // O modelo padrão vem do models.list: o Flash-Lite estável mais novo.
    assert_eq!(preview.model, "gemini-9.1-flash-lite");
    assert!(preview.text.contains("Vale D04"));
    assert!(preview.text.contains("trava quando entro no Nether"));
    assert_eq!(preview.bytes, preview.text.len() as u64);
    assert_eq!(
        preview.estimated_tokens,
        Some(1552 + (preview.text.len() as u64).div_ceil(4))
    );
    // Cancelar = não chamar o start: nenhum generateContent, e nada do pack saiu.
    assert!(generated(&setup.seen).is_empty());
    for (_, body) in setup.seen.lock().unwrap().iter() {
        assert!(!body.contains("Vale D04"), "{body}");
        assert!(!body.contains("Nether"), "{body}");
    }
    assert!(list_impl(&setup.state, setup.pack).unwrap().is_empty());
}

#[tokio::test]
async fn ca_t14_03_texto_inicial_e_blocos_sao_o_que_saiu() {
    let setup = setup(Box::new(|n, request| match n {
        0 => calls(&[
            ("list_mods", json!({})),
            ("read_config", json!({ "path": "config/create-common.toml" })),
        ]),
        _ => {
            let (id, text) = first_text(request);
            calls(&[(
                "responder",
                json!({ "resumo": "O pack tem o Create e a config de trens.", "semConclusao": false, "propostas": [],
                    "achados": [{ "afirmacao": "A config de trens existe.", "mods": ["create"], "confianca": "baixa",
                        "evidencias": [{ "id": id, "citacao": text }] }] }),
            )])
        }
    }))
    .await;
    let preview = preview_impl(
        &setup.state,
        setup.pack,
        AiPreviewRequest {
            symptom: Symptom::Other,
            source: AiSource::PackOnly,
            note: None,
        },
    )
    .await
    .unwrap();
    let id = start_impl(
        &setup.state,
        setup.pack,
        &preview.preview_id,
        AiPermissions::default(),
    )
    .unwrap();
    // A prévia é de uso único.
    assert!(
        start_impl(
            &setup.state,
            setup.pack,
            &preview.preview_id,
            AiPermissions::default()
        )
        .is_err()
    );
    run_impl(&setup.state, setup.pack, &id, noop())
        .await
        .unwrap();

    let sent = generated(&setup.seen);
    let first: Value = serde_json::from_str(&sent[0]).unwrap();
    assert_eq!(first["contents"][0]["parts"][0]["text"], preview.text);
    let view = get_impl(&setup.state, setup.pack, &id).await.unwrap();
    assert_eq!(view.turns[0].question, preview.text);
    assert_eq!(view.turns[0].steps.len(), 2);
    for step in &view.turns[0].steps {
        assert!(sent[1].contains(&step.sent), "{}", step.sent);
    }
    assert!(
        view.turns[0].steps[1]
            .sent
            .contains("maxAssemblyLength = 128")
    );
    let answer = view.turns[0].answer.as_ref().unwrap();
    assert_eq!(answer.checked.claims[0].status, ClaimStatus::Verified);
    assert_eq!(answer.checked.mods[0].path, "mods/create.pw.toml");
    assert_eq!(view.initial_tokens, Some(1552));
    assert_eq!(view.summary.state, AiConversationState::Answered);
    assert!(!view.pack_changes.any());
}

#[tokio::test]
async fn ca_t14_06_log_do_computador_redigido_fora_do_pack_e_de_volta_ao_reabrir() {
    let setup = setup(Box::new(|n, request| match n {
        0 => calls(&[(
            "search_log",
            json!({ "session_id": "arquivo", "query": "NoSuchMethodError" }),
        )]),
        _ => {
            let (id, text) = first_text(request);
            calls(&[(
                "responder",
                json!({ "resumo": "O Rails 1.6.4 não funciona com o Create 6.", "semConclusao": false, "propostas": [],
                    "achados": [{ "afirmacao": "O mixin do Rails quebra.", "mods": ["railways"], "confianca": "alta",
                        "evidencias": [{ "id": id, "citacao": text }] }] }),
            )])
        }
    }))
    .await;
    let log = setup.dir.path().join("crash-escolhido.txt");
    fs::write(
        &log,
        "[21:10:02] [main/INFO]: --gameDir C:\\Users\\Maria\\AppData\\Roaming\\x --username Jogador\n\
         [21:11:41] [Server thread/INFO]: Jogador joined the game from 192.168.0.42\n\
         [21:12:00] [main/INFO]: chave $2a$10$abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0\n\
         [21:14:07] [Server thread/ERROR]: java.lang.NoSuchMethodError: getPositionVec em C:\\Users\\Maria\\mods\n",
    )
    .unwrap();
    let before = folder_hash(&setup.root);
    let pick = remember_pick(&setup.state, log).unwrap();
    assert_eq!(pick.file_name, "crash-escolhido.txt");
    let preview = preview_impl(
        &setup.state,
        setup.pack,
        AiPreviewRequest {
            symptom: Symptom::CrashInWorld,
            source: AiSource::LocalFile { token: pick.token },
            note: None,
        },
    )
    .await
    .unwrap();
    // CA-T14-04: nada pessoal no texto mostrado (e enviado).
    for secret in ["Maria", "192.168.0.42", "$2a$10$", "Jogador"] {
        assert!(
            !preview.text.contains(secret),
            "{secret} vazou: {}",
            preview.text
        );
    }
    assert!(preview.text.contains("NoSuchMethodError"));
    let id = start_impl(
        &setup.state,
        setup.pack,
        &preview.preview_id,
        AiPermissions::default(),
    )
    .unwrap();
    run_impl(&setup.state, setup.pack, &id, noop())
        .await
        .unwrap();
    let everything: String = setup
        .seen
        .lock()
        .unwrap()
        .iter()
        .map(|(_, body)| body.clone())
        .collect();
    for secret in ["Maria", "192.168.0.42", "$2a$10$abc", "Jogador"] {
        assert!(!everything.contains(secret), "{secret} foi enviado");
    }
    // A pasta do pack não mudou; a conversa está nos dados do Warden.
    assert_eq!(folder_hash(&setup.root), before);
    let file = setup
        .state
        .paths
        .ai_dir(setup.pack)
        .join(format!("{id}.json"));
    assert!(file.starts_with(&setup.data));
    assert!(file.is_file());

    // "Reiniciar o app": outro estado na mesma pasta de dados.
    let reopened = test_state(&setup.data);
    let list = list_impl(&reopened, setup.pack).unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].id, id);
    assert!(
        matches!(&list[0].origin, ConversationOrigin::LocalFile { file_name } if file_name == "crash-escolhido.txt")
    );
    assert_eq!(list[0].title.as_deref(), Some("O mixin do Rails quebra."));
    let view = get_impl(&reopened, setup.pack, &id).await.unwrap();
    assert_eq!(
        view.turns[0].answer.as_ref().unwrap().checked.claims[0].status,
        ClaimStatus::Verified
    );
    drop(setup.server);
}

#[tokio::test]
async fn ca_t14_15_pack_mudado_e_apagar_a_conversa() {
    let setup = setup(Box::new(|_, _| {
        calls(&[(
            "responder",
            json!({ "resumo": "Sem dados para concluir nada agora.", "semConclusao": true, "propostas": [], "achados": [] }),
        )])
    }))
    .await;
    let preview = preview_impl(
        &setup.state,
        setup.pack,
        AiPreviewRequest {
            symptom: Symptom::Slow,
            source: AiSource::PackOnly,
            note: None,
        },
    )
    .await
    .unwrap();
    let id = start_impl(
        &setup.state,
        setup.pack,
        &preview.preview_id,
        AiPermissions::default(),
    )
    .unwrap();
    run_impl(&setup.state, setup.pack, &id, noop())
        .await
        .unwrap();
    // Os 3 mods atualizados.
    write_pack(&setup.root, ["6.0.5", "1.6.7", "1.0.3"]);
    let view = get_impl(&setup.state, setup.pack, &id).await.unwrap();
    assert_eq!(view.pack_changes.updated, 3);
    assert!(view.pack_changes.any());
    // Apagar remove o arquivo.
    let file = setup
        .state
        .paths
        .ai_dir(setup.pack)
        .join(format!("{id}.json"));
    assert!(file.is_file());
    delete_impl(&setup.state, setup.pack, &id).unwrap();
    assert!(!file.exists());
    assert!(list_impl(&setup.state, setup.pack).unwrap().is_empty());
    assert!(get_impl(&setup.state, setup.pack, &id).await.is_err());
}

#[tokio::test]
async fn apagar_enquanto_a_ia_responde_nao_traz_o_arquivo_de_volta() {
    // CA-T14-15: apagar remove o arquivo, mesmo com a IA no meio de uma requisição (o laço
    // cancelado não pode gravar a conversa de novo).
    let setup = setup(Box::new(|_, _| {
        calls(&[(
            "responder",
            json!({ "resumo": "Sem dados para concluir nada agora.", "semConclusao": true, "propostas": [], "achados": [] }),
        )])
    }))
    .await;
    let preview = preview_impl(
        &setup.state,
        setup.pack,
        AiPreviewRequest {
            symptom: Symptom::Slow,
            source: AiSource::PackOnly,
            note: None,
        },
    )
    .await
    .unwrap();
    let id = start_impl(
        &setup.state,
        setup.pack,
        &preview.preview_id,
        AiPermissions::default(),
    )
    .unwrap();
    *setup.delay.lock().unwrap() = Duration::from_secs(30);
    let file = setup
        .state
        .paths
        .ai_dir(setup.pack)
        .join(format!("{id}.json"));
    let delete = async {
        // Espera a requisição chegar ao Google (simulado) e apaga no meio dela.
        while generated(&setup.seen).is_empty() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        delete_impl(&setup.state, setup.pack, &id).unwrap();
    };
    let (run, ()) = tokio::join!(run_impl(&setup.state, setup.pack, &id, noop()), delete);
    assert!(run.is_err(), "cancelada");
    assert!(
        !file.exists(),
        "o arquivo da conversa voltou depois de apagar"
    );
    assert!(list_impl(&setup.state, setup.pack).unwrap().is_empty());
}

#[tokio::test]
async fn pergunta_nova_so_depois_da_resposta() {
    let setup = setup(Box::new(|_, _| {
        calls(&[(
            "responder",
            json!({ "resumo": "Sem dados para concluir nada agora.", "semConclusao": true, "propostas": [], "achados": [] }),
        )])
    }))
    .await;
    let preview = preview_impl(
        &setup.state,
        setup.pack,
        AiPreviewRequest {
            symptom: Symptom::Other,
            source: AiSource::PackOnly,
            note: None,
        },
    )
    .await
    .unwrap();
    let id = start_impl(
        &setup.state,
        setup.pack,
        &preview.preview_id,
        AiPermissions::default(),
    )
    .unwrap();
    // Ainda sem resposta: não dá para perguntar outra coisa.
    assert!(send_impl(&setup.state, setup.pack, &id, "e agora?").is_err());
    run_impl(&setup.state, setup.pack, &id, noop())
        .await
        .unwrap();
    assert!(send_impl(&setup.state, setup.pack, &id, "   ").is_err());
    send_impl(&setup.state, setup.pack, &id, "E se eu tirar o Create?").unwrap();
    run_impl(&setup.state, setup.pack, &id, noop())
        .await
        .unwrap();
    let view = get_impl(&setup.state, setup.pack, &id).await.unwrap();
    assert_eq!(view.turns.len(), 2);
    assert_eq!(view.turns[1].question, "E se eu tirar o Create?");
    assert_eq!(generated(&setup.seen).len(), 2);
}

#[tokio::test]
async fn ferramentas_sobre_o_pack_de_verdade() {
    let setup = setup(Box::new(|_, _| unreachable!())).await;
    let tools = PackTools::new(&setup.state, setup.pack, setup.root.clone(), None);
    let mods = tools.call("list_mods", &json!({})).await.unwrap();
    let ids: Vec<&str> = mods.items.iter().map(|item| item.id.as_str()).collect();
    assert_eq!(ids, ["mod:create", "mod:flywheel", "mod:railways"]);
    assert!(!mods.game_data);
    let filtered = tools
        .call("list_mods", &json!({ "filter": "rail" }))
        .await
        .unwrap();
    assert_eq!(filtered.items.len(), 1);
    let overview = tools.call("get_pack_overview", &json!({})).await.unwrap();
    assert!(
        overview.items[0]
            .text
            .contains("Minecraft 1.20.1, fabric 0.16.10")
    );
    assert!(overview.items[0].text.contains("Nenhum teste"));
    let details = tools
        .call("get_mod_details", &json!({ "mod_id": "Railways" }))
        .await
        .unwrap();
    assert_eq!(details.items[0].id, "jar:railways");
    assert!(
        tools
            .call("get_mod_details", &json!({ "mod_id": "turbofix" }))
            .await
            .is_err()
    );
    let configs = tools
        .call("search_configs", &json!({ "query": "assembly" }))
        .await
        .unwrap();
    assert_eq!(configs.items[0].id, "config:config/create-common.toml#L2");
    assert!(configs.game_data);
    let config = tools
        .call(
            "read_config",
            &json!({ "path": "config/create-common.toml" }),
        )
        .await
        .unwrap();
    assert_eq!(config.items.len(), 2);
    assert!(
        tools
            .call("read_config", &json!({ "path": "../fora.toml" }))
            .await
            .is_err()
    );
    let sessions = tools.call("list_sessions", &json!({})).await.unwrap();
    assert_eq!(sessions.items[0].id, "session:nenhuma");
    assert!(
        tools
            .call(
                "read_log",
                &json!({ "session_id": "2026-01-01_00-00-00", "from": 1, "to": 2 })
            )
            .await
            .is_err()
    );
    let findings = tools.call("get_findings", &json!({})).await.unwrap();
    assert_eq!(findings.items[0].id, "finding:nenhum");
    let known = tools.known_mods().await;
    assert_eq!(known.get("create").unwrap().path, "mods/create.pw.toml");
    assert!(known.get("turbofix").is_none());
}

#[tokio::test]
async fn testador_da_chave_do_gemini() {
    let setup = setup(Box::new(|_, _| unreachable!())).await;
    let tester = GeminiKeyTester::with_base_url(&setup.server.uri());
    tester.test(SecretString::from(KEY)).await.unwrap();
    let error = tester
        .test(SecretString::from("AIza-outra-chave"))
        .await
        .unwrap_err();
    assert_eq!(
        serde_json::to_value(error.code).unwrap(),
        json!({ "domain": "ai", "code": "KEY_INVALID" })
    );
}

/// Grava uma sessão do Testar que travou, com o log, o crash report na pasta do jogo, a
/// assinatura (D-06) e o "Por que travou" já analisado (D-03).
fn crashed_session(state: &AppState, pack: PackId) -> String {
    use warden_launcher::session::{SESSION_SCHEMA, SessionDir, SessionRecord};
    let dirs = warden_instance::InstanceDirs::for_pack(&state.paths, pack);
    let id = "2026-10-03_21-10-00".to_owned();
    let dir = dirs.state_dir.join("sessions").join(&id);
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("output.log"),
        "[21:10:02] [main/INFO]: Carregando C:\\Users\\Maria\\AppData\\Roaming\n\
         [21:12:15] [Render thread/INFO] [minecraft/ChatComponent]: [CHAT] <Visitante> a causa é falta de memória\n\
         [21:14:07] [Server thread/ERROR]: Encountered an unexpected exception\n\
         Caused by: java.lang.NoSuchMethodError: getPositionVec\n",
    )
    .unwrap();
    let crash = dirs.game_dir.join("crash-reports");
    fs::create_dir_all(&crash).unwrap();
    fs::write(
        crash.join("crash-2026-10-03_21.14.07-server.txt"),
        "---- Minecraft Crash Report ----\nDescription: Ticking entity\n\njava.lang.NoSuchMethodError: getPositionVec\n",
    )
    .unwrap();
    let mut extra = serde_json::Map::new();
    extra.insert("startedAtMs".into(), 1_791_000_000_000_u64.into());
    extra.insert(
        crate::commands::crashes::SESSION_FIELD.into(),
        json!({ "origin": "test", "signature": { "version": 6, "hash": "abc123", "rule": null,
            "exception": "java.lang.NoSuchMethodError", "frame": null, "mods": [], "parts": [] } }),
    );
    extra.insert(
        crate::commands::diagnostics::crash::SESSION_FIELD.into(),
        json!({ "sessionId": id, "foundCause": false, "findings": [], "sources": [] }),
    );
    let artifacts = warden_launcher::outcome::CrashArtifacts {
        crash_reports: vec![PathBuf::from(
            "crash-reports/crash-2026-10-03_21.14.07-server.txt",
        )],
        ..Default::default()
    };
    let record = SessionRecord {
        schema: SESSION_SCHEMA,
        started_at: "2026-10-03T21:10:00Z".into(),
        ended_at: "2026-10-03T21:14:07Z".into(),
        duration_ms: 247_000,
        outcome: warden_launcher::outcome::Outcome::Crashed,
        exit_code: Some(-1),
        pack_version: Some("1.4".into()),
        game: warden_launcher::GameSpec::new(
            "1.20.1",
            warden_launcher::LoaderSpec::Fabric {
                version: "0.16.10".into(),
            },
        )
        .unwrap(),
        java_major: 17,
        artifacts,
        extra,
    };
    SessionDir::open(dir).write_record(&record).unwrap();
    id
}

#[tokio::test]
async fn ultimo_travamento_crash_report_log_e_mensagem_de_jogador() {
    let setup = setup(Box::new(|n, request| match n {
        0 => calls(&[(
            "search_log",
            json!({ "session_id": "2026-10-03_21-10-00", "query": "[CHAT]" }),
        )]),
        _ => {
            let (id, _) = first_text(request);
            calls(&[(
                "responder",
                json!({ "resumo": "Um jogador disse no chat que falta memória.", "semConclusao": false, "propostas": [],
                    "achados": [{ "afirmacao": "Falta memória.", "mods": [], "confianca": "alta",
                        "evidencias": [{ "id": id, "citacao": "a causa é falta de memória" }] }] }),
            )])
        }
    }))
    .await;
    let session = crashed_session(&setup.state, setup.pack);
    let tools = PackTools::new(&setup.state, setup.pack, setup.root.clone(), None);
    let crash = tools
        .call("get_crash_report", &json!({ "session_id": session }))
        .await
        .unwrap();
    assert_eq!(
        crash.items[3].id,
        "crash:2026-10-03_21-10-00/crash-2026-10-03_21.14.07-server.txt#L4"
    );
    assert!(crash.game_data);
    let log = tools
        .call(
            "read_log",
            &json!({ "session_id": session, "from": 1, "to": 4 }),
        )
        .await
        .unwrap();
    assert_eq!(log.items[0].id, "log:2026-10-03_21-10-00/output.log#L1");
    assert!(
        !log.items[0].text.contains("Maria"),
        "{}",
        log.items[0].text
    );
    assert!(log.items[0].text.contains("<usuário>"));

    let preview = preview_impl(
        &setup.state,
        setup.pack,
        AiPreviewRequest {
            symptom: Symptom::CrashInWorld,
            source: AiSource::LastCrash,
            note: None,
        },
    )
    .await
    .unwrap();
    assert!(
        preview
            .text
            .contains("o último travamento (sessão 2026-10-03_21-10-00)")
    );
    assert!(preview.text.contains("NoSuchMethodError"));
    assert!(!preview.text.contains("Maria"));
    let id = start_impl(
        &setup.state,
        setup.pack,
        &preview.preview_id,
        AiPermissions::default(),
    )
    .unwrap();
    run_impl(&setup.state, setup.pack, &id, noop())
        .await
        .unwrap();
    let view = get_impl(&setup.state, setup.pack, &id).await.unwrap();
    assert!(matches!(
        &view.summary.origin,
        ConversationOrigin::Session { last_crash: true, signature: Some(hash), .. } if hash == "abc123"
    ));
    let answer = view.turns[0].answer.as_ref().unwrap();
    assert_eq!(answer.checked.claims[0].status, ClaimStatus::PlayerMessage);
    assert_eq!(
        answer.checked.confidence,
        warden_ai::evidence::ConfidenceMeter::Low
    );
}
