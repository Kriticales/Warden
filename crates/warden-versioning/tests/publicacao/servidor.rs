//! Servidor local que imita o GitHub para a publicação (ROADMAP V-03, Verificação):
//!
//! - `/git/<dono>/<repo>.git/…`: o servidor git "smart HTTP" (`info/refs` e
//!   `git-receive-pack`) sobre um repositório nu numa pasta temporária, com a troca protegida
//!   (o valor antigo de cada referência precisa ser o atual), o envio `atomic` e as mesmas
//!   respostas de recusa do `git-receive-pack` real;
//! - `/api/…`: `GET /user`, `POST /user/repos`, `GET /repos/{dono}/{repo}` e as Releases;
//! - `/raw/<dono>/<repo>/main/<caminho>`: o `raw.githubusercontent.com`, lido da `main` do
//!   repositório nu.
//!
//! Tudo só em 127.0.0.1. Guarda os cabeçalhos `Authorization` recebidos para os testes de
//! credencial (CA-T18-02).

#![allow(
    dead_code,
    unreachable_pub,
    clippy::unwrap_used,
    clippy::panic,
    clippy::expect_used
)]

use std::collections::BTreeMap;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use base64::Engine as _;
use secrecy::SecretString;
use warden_http::{HttpClient, HttpConfig};
use warden_versioning::github::{GithubClient, GithubEndpoints, RepoRef};
use wiremock::matchers::{method, path_regex};
use wiremock::{Mock, MockServer, Request, Respond, ResponseTemplate};

pub const LOGIN: &str = "dono-teste";
pub const TOKEN: &str = "ghp_tokenDeTesteQueNuncaPodeVazar0123456789";

#[derive(Debug, Clone)]
pub struct Release {
    pub id: u64,
    pub tag: String,
    pub name: String,
    pub body: String,
}

#[derive(Default)]
pub struct State {
    /// `dono/repo` → (pasta do repositório nu, privado).
    pub repos: BTreeMap<String, (PathBuf, bool)>,
    pub releases: Vec<Release>,
    pub authorizations: Vec<String>,
    /// Próximas criações de Release que falham com 500.
    pub fail_releases: u32,
    /// Responde 403 no `git-receive-pack` (token sem permissão de envio).
    pub deny_push: bool,
    /// Responde 403 na criação de repositório.
    pub deny_create: bool,
    /// Alguém muda a `main` pelo site logo antes do próximo `git-receive-pack` chegar.
    pub race_main: bool,
    /// O próximo `git-receive-pack` é aplicado, mas a resposta se perde (502).
    pub lose_push_response: bool,
}

pub struct FakeGithub {
    pub server: MockServer,
    pub state: Arc<Mutex<State>>,
    pub dir: tempfile::TempDir,
}

fn json(status: u16, value: serde_json::Value) -> ResponseTemplate {
    ResponseTemplate::new(status).set_body_json(value)
}

fn pkt(out: &mut Vec<u8>, line: &str) {
    out.extend_from_slice(format!("{:04x}", line.len() + 4).as_bytes());
    out.extend_from_slice(line.as_bytes());
}

#[derive(Clone)]
struct Handler {
    state: Arc<Mutex<State>>,
    base: String,
    dir: PathBuf,
}

impl Handler {
    fn authorized(&self, request: &Request, expected: &str) -> bool {
        let value = request
            .headers
            .get("authorization")
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default()
            .to_owned();
        self.state
            .lock()
            .unwrap()
            .authorizations
            .push(value.clone());
        value == expected
    }

    fn bearer() -> String {
        format!("Bearer {TOKEN}")
    }

    fn basic() -> String {
        format!(
            "Basic {}",
            base64::engine::general_purpose::STANDARD.encode(format!("x-access-token:{TOKEN}"))
        )
    }

    fn repo_json(&self, full: &str, private: bool) -> serde_json::Value {
        let (owner, name) = full.split_once('/').unwrap();
        serde_json::json!({
            "name": name,
            "owner": { "login": owner },
            "private": private,
            "html_url": format!("{}/git/{full}", self.base),
        })
    }

    fn bare(&self, full: &str) -> Option<git2::Repository> {
        let path = self.state.lock().unwrap().repos.get(full)?.0.clone();
        Some(git2::Repository::open_bare(path).unwrap())
    }

    fn api(&self, request: &Request, route: &str) -> ResponseTemplate {
        if !self.authorized(request, &Self::bearer()) {
            return json(401, serde_json::json!({ "message": "Bad credentials" }));
        }
        let post = request.method.as_str() == "POST";
        let parts: Vec<&str> = route.split('/').collect();
        match (post, parts.as_slice()) {
            (false, ["user"]) => json(200, serde_json::json!({ "login": LOGIN })),
            (true, ["user", "repos"]) => {
                let body: serde_json::Value = serde_json::from_slice(&request.body).unwrap();
                let name = body["name"].as_str().unwrap();
                let private = body["private"].as_bool().unwrap();
                let full = format!("{LOGIN}/{name}");
                let mut state = self.state.lock().unwrap();
                if state.deny_create {
                    return json(
                        403,
                        serde_json::json!({ "message": "Resource not accessible by personal access token" }),
                    );
                }
                if state.repos.contains_key(&full) {
                    return json(
                        422,
                        serde_json::json!({ "message": "name already exists on this account" }),
                    );
                }
                let path = self.dir.join(full.replace('/', "__"));
                git2::Repository::init_bare(&path).unwrap();
                state.repos.insert(full.clone(), (path, private));
                drop(state);
                json(201, self.repo_json(&full, private))
            }
            (false, ["repos", owner, name]) => {
                let full = format!("{owner}/{name}");
                match self.state.lock().unwrap().repos.get(&full) {
                    Some((_, private)) => json(200, self.repo_json(&full, *private)),
                    None => json(404, serde_json::json!({ "message": "Not Found" })),
                }
            }
            (false, ["repos", _, _, "releases", "tags", tag]) => {
                let state = self.state.lock().unwrap();
                match state.releases.iter().find(|r| r.tag == *tag) {
                    Some(release) => json(
                        200,
                        serde_json::json!({ "id": release.id, "html_url": format!("{}/rel/{}", self.base, release.id) }),
                    ),
                    None => json(404, serde_json::json!({ "message": "Not Found" })),
                }
            }
            (true, ["repos", owner, name, "releases"]) => {
                let mut state = self.state.lock().unwrap();
                if state.fail_releases > 0 {
                    state.fail_releases -= 1;
                    return json(500, serde_json::json!({ "message": "Server Error" }));
                }
                let body: serde_json::Value = serde_json::from_slice(&request.body).unwrap();
                let tag = body["tag_name"].as_str().unwrap().to_owned();
                let full = format!("{owner}/{name}");
                drop(state);
                let has_tag = self
                    .bare(&full)
                    .is_some_and(|repo| repo.find_reference(&format!("refs/tags/{tag}")).is_ok());
                if !has_tag {
                    return json(422, serde_json::json!({ "message": "tag inexistente" }));
                }
                let mut state = self.state.lock().unwrap();
                let id = 1000 + state.releases.len() as u64;
                state.releases.push(Release {
                    id,
                    tag,
                    name: body["name"].as_str().unwrap().to_owned(),
                    body: body["body"].as_str().unwrap().to_owned(),
                });
                json(
                    201,
                    serde_json::json!({ "id": id, "html_url": format!("{}/rel/{id}", self.base) }),
                )
            }
            _ => json(404, serde_json::json!({ "message": "Not Found" })),
        }
    }

    fn git(&self, request: &Request, route: &str) -> ResponseTemplate {
        let Some((full, action)) = route.split_once(".git/") else {
            return ResponseTemplate::new(404);
        };
        if !self.authorized(request, &Self::basic()) {
            return ResponseTemplate::new(401);
        }
        let Some(repo) = self.bare(full) else {
            return ResponseTemplate::new(404);
        };
        if action.starts_with("info/refs") {
            let mut body = Vec::new();
            pkt(&mut body, "# service=git-receive-pack\n");
            body.extend_from_slice(b"0000");
            let mut first = true;
            let caps = "\0report-status delete-refs atomic ofs-delta\n";
            for reference in repo.references().unwrap() {
                let reference = reference.unwrap();
                let name = reference.name().unwrap().to_owned();
                let target = reference.target().unwrap();
                pkt(
                    &mut body,
                    &format!("{target} {name}{}", if first { caps } else { "\n" }),
                );
                first = false;
            }
            if first {
                pkt(
                    &mut body,
                    &format!("{} capabilities^{{}}{caps}", "0".repeat(40)),
                );
            }
            body.extend_from_slice(b"0000");
            return ResponseTemplate::new(200)
                .insert_header(
                    "content-type",
                    "application/x-git-receive-pack-advertisement",
                )
                .set_body_bytes(body);
        }
        if action == "git-receive-pack" {
            if self.state.lock().unwrap().deny_push {
                return ResponseTemplate::new(403);
            }
            let data = &request.body;
            let mut offset = 0;
            let mut commands = Vec::new();
            let mut atomic = false;
            loop {
                let len = usize::from_str_radix(
                    std::str::from_utf8(&data[offset..offset + 4]).unwrap(),
                    16,
                )
                .unwrap();
                if len == 0 {
                    offset += 4;
                    break;
                }
                let line = String::from_utf8_lossy(&data[offset + 4..offset + len]).to_string();
                let mut split = line.split('\0');
                let command = split.next().unwrap().trim_end().to_owned();
                if let Some(caps) = split.next() {
                    atomic |= caps.split_whitespace().any(|cap| cap == "atomic");
                }
                let line = command;
                let parts: Vec<String> = line.split(' ').map(ToOwned::to_owned).collect();
                commands.push((parts[0].clone(), parts[1].clone(), parts[2].clone()));
                offset += len;
            }
            let pack = &data[offset..];
            if !pack.is_empty() {
                let odb = repo.odb().unwrap();
                let mut writer = odb.packwriter().unwrap();
                writer.write_all(pack).unwrap();
                writer.commit().unwrap();
            }
            if std::mem::take(&mut self.state.lock().unwrap().race_main) {
                let main = repo
                    .find_reference("refs/heads/main")
                    .unwrap()
                    .peel_to_commit()
                    .unwrap();
                let sig = git2::Signature::now("Outra pessoa", "x@y").unwrap();
                let tree = main.tree().unwrap();
                repo.commit(
                    Some("refs/heads/main"),
                    &sig,
                    &sig,
                    "Editado no site",
                    &tree,
                    &[&main],
                )
                .unwrap();
            }
            let current = |name: &str| {
                repo.find_reference(name)
                    .ok()
                    .and_then(|r| r.target())
                    .map_or_else(|| "0".repeat(40), |oid| oid.to_string())
            };
            let mut body = Vec::new();
            pkt(&mut body, "unpack ok\n");
            // Como o `git-receive-pack` real: com `atomic`, uma referência recusada recusa todas.
            if atomic && commands.iter().any(|(old, _, name)| current(name) != *old) {
                for (_, _, name) in &commands {
                    pkt(&mut body, &format!("ng {name} atomic transaction failed\n"));
                }
                body.extend_from_slice(b"0000");
                return ResponseTemplate::new(200)
                    .insert_header("content-type", "application/x-git-receive-pack-result")
                    .set_body_bytes(body);
            }
            for (old, new, name) in commands {
                if current(&name) != old {
                    // A mesma resposta do `git-receive-pack` real (o motivo detalhado vai para
                    // o canal de erro: "cannot lock ref …: is at … but expected …").
                    pkt(&mut body, &format!("ng {name} failed to update ref\n"));
                    continue;
                }
                let new = git2::Oid::from_str(&new).unwrap();
                assert!(
                    repo.find_object(new, None).is_ok(),
                    "objeto {new} não chegou"
                );
                repo.reference(&name, new, true, "receive-pack").unwrap();
                pkt(&mut body, &format!("ok {name}\n"));
            }
            body.extend_from_slice(b"0000");
            if std::mem::take(&mut self.state.lock().unwrap().lose_push_response) {
                return ResponseTemplate::new(502);
            }
            return ResponseTemplate::new(200)
                .insert_header("content-type", "application/x-git-receive-pack-result")
                .set_body_bytes(body);
        }
        ResponseTemplate::new(404)
    }

    fn raw(&self, route: &str) -> ResponseTemplate {
        let mut parts = route.splitn(4, '/');
        let (Some(owner), Some(name), Some(branch), Some(path)) =
            (parts.next(), parts.next(), parts.next(), parts.next())
        else {
            return ResponseTemplate::new(404);
        };
        let Some(repo) = self.bare(&format!("{owner}/{name}")) else {
            return ResponseTemplate::new(404);
        };
        let Ok(reference) = repo.find_reference(&format!("refs/heads/{branch}")) else {
            return ResponseTemplate::new(404);
        };
        let tree = reference.peel_to_tree().unwrap();
        let path = percent_decode(path);
        match tree.get_path(Path::new(&path)) {
            Ok(entry) => ResponseTemplate::new(200)
                .set_body_bytes(repo.find_blob(entry.id()).unwrap().content().to_vec()),
            Err(_) => ResponseTemplate::new(404),
        }
    }
}

fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && i + 2 < bytes.len()
            && let Ok(value) = u8::from_str_radix(&text[i + 1..i + 3], 16)
        {
            out.push(value);
            i += 3;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8(out).unwrap()
}

impl Respond for Handler {
    fn respond(&self, request: &Request) -> ResponseTemplate {
        let path = request.url.path().to_owned();
        let query = request
            .url
            .query()
            .map(|q| format!("?{q}"))
            .unwrap_or_default();
        if let Some(route) = path.strip_prefix("/api/") {
            self.api(request, route)
        } else if let Some(route) = path.strip_prefix("/git/") {
            self.git(request, &format!("{route}{query}"))
        } else if let Some(route) = path.strip_prefix("/raw/") {
            self.raw(route)
        } else {
            ResponseTemplate::new(404)
        }
    }
}

impl FakeGithub {
    pub async fn start() -> Self {
        let server = MockServer::start().await;
        let dir = tempfile::tempdir().unwrap();
        let state = Arc::new(Mutex::new(State::default()));
        let handler = Handler {
            state: Arc::clone(&state),
            base: server.uri(),
            dir: dir.path().to_path_buf(),
        };
        Mock::given(method("GET"))
            .and(path_regex("^/"))
            .respond_with(handler.clone())
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path_regex("^/"))
            .respond_with(handler)
            .mount(&server)
            .await;
        Self { server, state, dir }
    }

    pub fn endpoints(&self) -> GithubEndpoints {
        let base = self.server.uri();
        GithubEndpoints::new(
            &format!("{base}/api/"),
            &format!("{base}/git/"),
            &format!("{base}/raw/"),
        )
        .unwrap()
    }

    pub fn client_with(&self, token: &str) -> GithubClient {
        GithubClient::new(
            HttpClient::new(HttpConfig::default()).unwrap(),
            self.endpoints(),
            SecretString::from(token.to_owned()),
        )
    }

    pub fn client(&self) -> GithubClient {
        self.client_with(TOKEN)
    }

    /// Cria um repositório nu vazio, como se já existisse no GitHub.
    pub fn create(&self, name: &str) -> RepoRef {
        let full = format!("{LOGIN}/{name}");
        let path = self.dir.path().join(full.replace('/', "__"));
        git2::Repository::init_bare(&path).unwrap();
        self.state.lock().unwrap().repos.insert(full, (path, false));
        RepoRef {
            owner: LOGIN.into(),
            name: name.into(),
        }
    }

    pub fn bare(&self, repo: &RepoRef) -> git2::Repository {
        let path = self.state.lock().unwrap().repos[&repo.full_name()]
            .0
            .clone();
        git2::Repository::open_bare(path).unwrap()
    }

    /// Referências do remoto: nome → id.
    pub fn refs(&self, repo: &RepoRef) -> BTreeMap<String, String> {
        let bare = self.bare(repo);
        let mut refs = BTreeMap::new();
        for reference in bare.references().unwrap() {
            let reference = reference.unwrap();
            refs.insert(
                reference.name().unwrap().to_owned(),
                reference.target().unwrap().to_string(),
            );
        }
        refs
    }

    /// Arquivos da `main` remota: caminho → bytes.
    pub fn main_files(&self, repo: &RepoRef) -> BTreeMap<String, Vec<u8>> {
        let bare = self.bare(repo);
        let tree = bare
            .find_reference("refs/heads/main")
            .unwrap()
            .peel_to_tree()
            .unwrap();
        let mut files = BTreeMap::new();
        tree.walk(git2::TreeWalkMode::PreOrder, |dir, entry| {
            if entry.kind() == Some(git2::ObjectType::Blob) {
                let blob = bare.find_blob(entry.id()).unwrap();
                files.insert(
                    format!("{dir}{}", entry.name().unwrap()),
                    blob.content().to_vec(),
                );
            }
            git2::TreeWalkResult::Ok
        })
        .unwrap();
        files
    }

    /// Mensagens dos commits da `main` remota, da mais nova para a mais antiga.
    pub fn main_log(&self, repo: &RepoRef) -> Vec<String> {
        let bare = self.bare(repo);
        let mut walk = bare.revwalk().unwrap();
        walk.push_ref("refs/heads/main").unwrap();
        walk.map(|id| {
            bare.find_commit(id.unwrap())
                .unwrap()
                .summary()
                .ok()
                .flatten()
                .unwrap_or_default()
                .to_owned()
        })
        .collect()
    }

    pub fn releases(&self) -> Vec<Release> {
        self.state.lock().unwrap().releases.clone()
    }
}
