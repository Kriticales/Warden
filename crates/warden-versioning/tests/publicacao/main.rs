//! Publicar versão no GitHub (SPEC T18; V-03) contra o servidor local que imita o GitHub
//! (`servidor.rs`): servidor git "smart HTTP", API de Releases e `raw.githubusercontent.com`.
//!
//! CA-T18-02 (o token não vaza), CA-T18-03 (packwiz-installer-bootstrap real), CA-T18-04 (só
//! "Publicar versão" muda o remoto, sem `.warden/` nem versões não publicadas), CA-T18-05 (notas
//! desde a última publicada), CA-T18-07 (bytes preservados, inclusive CRLF) e os erros do T18
//! (divergência com substituição protegida, Release pendente, permissões). O CA-T18-01 (conta
//! real) fica em `rede.rs`, sob demanda.

#![allow(linker_messages)]
#![allow(clippy::unwrap_used, clippy::panic, clippy::expect_used)]

#[path = "../common/mod.rs"]
mod common;
mod instalador;
mod rede;
mod servidor;

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::sync::{Arc, Mutex};

use common::{TestPack, at, identity};
use servidor::{FakeGithub, LOGIN, PushFault, TOKEN};
use sha2::{Digest as _, Sha256};
use warden_core::{CancellationToken, DomainError as _};
use warden_packwiz::{PackIndex, PackManifest, Side};
use warden_versioning::github::{GithubPermission, RepoRef};
use warden_versioning::publish::{
    GITATTRIBUTES, PublishRequest, PublishTree, ReplaceRemote, publish, remote_status,
    retry_release,
};
use warden_versioning::{Error, FileNames, PackRepo, SaveVersion, Snapshot, changelog};

/// Arquivos de controle, que não entram no índice.
const CONTROL: [&str; 5] = [
    "pack.toml",
    "index.toml",
    ".gitignore",
    ".gitattributes",
    "CHANGELOG.md",
];

/// Reescreve o `index.toml` com o SHA-256 de cada arquivo do pack (fora os de controle e
/// `.warden/`) e o `pack.toml` com o hash do índice, como o `packwiz refresh`.
fn refresh(pack: &TestPack, version: &str) {
    let mut index = "hash-format = \"sha256\"\n".to_owned();
    for (path, hash) in pack.hashes() {
        if CONTROL.contains(&path.as_str()) || path.starts_with(".warden/") {
            continue;
        }
        write!(index, "\n[[files]]\nfile = \"{path}\"\nhash = \"{hash}\"\n").unwrap();
        if path.ends_with(".pw.toml") {
            index.push_str("metafile = true\n");
        }
    }
    pack.write("index.toml", index.as_bytes());
    let mut manifest = PackManifest::new("Vale Sereno", "1.21.1");
    version.clone_into(&mut manifest.version);
    "Autor de Teste".clone_into(&mut manifest.author);
    manifest
        .versions
        .get_or_insert_with(BTreeMap::new)
        .insert("fabric".into(), "0.16.9".into());
    manifest.index.hash = hex::encode(Sha256::digest(index.as_bytes()));
    pack.write("pack.toml", manifest.to_toml_string().as_bytes());
}

fn save(repo: &PackRepo, pack: &TestPack, version: &str, minutes: i64, is_final: bool) {
    refresh(pack, version);
    repo.save_version(&SaveVersion {
        version,
        tag_message: &format!("## {version}\n"),
        identity: &identity(),
        when: at(minutes),
        mark_final: is_final,
    })
    .unwrap();
}

/// A árvore da versão como a exportação (E-01) entrega: `pack.toml`, `index.toml` e os
/// arquivos do índice, lidos da versão salva.
fn tree_of(repo: &PackRepo, version: &str) -> PublishTree {
    let snapshot = Snapshot::Version(version.into());
    let read = |path: &str| repo.read_file_at(&snapshot, path).unwrap().unwrap();
    let index_bytes = read("index.toml");
    let index = PackIndex::parse(std::str::from_utf8(&index_bytes).unwrap())
        .unwrap()
        .value;
    let mut tree = PublishTree::new();
    tree.insert("pack.toml".into(), read("pack.toml"));
    for entry in index.normalized_entries() {
        tree.insert(entry.file.clone(), read(&entry.file));
    }
    tree.insert("index.toml".into(), index_bytes);
    tree
}

struct Ctx {
    github: FakeGithub,
    repo_ref: RepoRef,
    pack: TestPack,
}

impl Ctx {
    async fn new() -> Self {
        let github = FakeGithub::start().await;
        let repo_ref = github.create("vale-sereno");
        let (pack, _) = TestPack::versioned();
        Self {
            github,
            repo_ref,
            pack,
        }
    }

    fn repo(&self) -> PackRepo {
        PackRepo::open(&self.pack.root).unwrap()
    }

    async fn publish(
        &self,
        version: &str,
        notes: &str,
    ) -> Result<warden_versioning::publish::Published, Error> {
        self.publish_replacing(version, notes, None).await
    }

    async fn publish_replacing(
        &self,
        version: &str,
        notes: &str,
        replace: Option<&ReplaceRemote>,
    ) -> Result<warden_versioning::publish::Published, Error> {
        let tree = tree_of(&self.repo(), version);
        let id = identity();
        let request = PublishRequest {
            version,
            tree: &tree,
            notes,
            pack_name: "Vale Sereno",
            identity: &id,
            when: at(500),
        };
        publish(
            &self.github.client(),
            &self.pack.root,
            &self.repo_ref,
            &request,
            replace,
            &CancellationToken::new(),
        )
        .await
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

#[tokio::test]
async fn ca_t18_04_so_publicar_muda_o_remoto_e_a_main_so_tem_o_conteudo_publicado() {
    let ctx = Ctx::new().await;
    let pack = &ctx.pack;
    pack.add_modrinth("sodium", "Sodium", "AANobbMI", "v1", Side::Client);
    pack.write("config/sodium.json", b"{\"a\": 1}\n");
    pack.write(".warden/project.toml", b"[notas]\nx = \"privado\"\n");
    save(&ctx.repo(), pack, "1.0.0", 10, true);
    // Salvar e marcar como final não mexem no GitHub.
    assert!(ctx.github.refs(&ctx.repo_ref).is_empty());

    let published = ctx.publish("1.0.0", "Primeira versão.").await.unwrap();
    assert_eq!(
        published.pack_link,
        format!(
            "{}/raw/{LOGIN}/vale-sereno/main/pack.toml",
            ctx.github.server.uri()
        )
    );
    let files = ctx.github.main_files(&ctx.repo_ref);
    let mut expected: Vec<String> = tree_of(&ctx.repo(), "1.0.0").into_keys().collect();
    expected.extend(["CHANGELOG.md".into(), ".gitattributes".into()]);
    expected.sort();
    assert_eq!(files.keys().cloned().collect::<Vec<_>>(), expected);
    assert!(!files.keys().any(|path| path.starts_with(".warden")));
    assert_eq!(
        ctx.github.main_log(&ctx.repo_ref),
        vec!["Vale Sereno 1.0.0"]
    );
    let after_first = ctx.github.refs(&ctx.repo_ref);
    assert_eq!(
        after_first.keys().collect::<Vec<_>>(),
        vec!["refs/heads/main", "refs/tags/v1.0.0"]
    );

    // Trabalho local, uma versão salva e final não publicada: o remoto não muda.
    pack.write("config/sodium.json", b"{\"a\": 2}\n");
    save(&ctx.repo(), pack, "1.1.0", 20, true);
    pack.write("config/extra.toml", b"x = 1\n");
    save(&ctx.repo(), pack, "1.2.0", 30, false);
    ctx.repo().set_final("1.2.0", true).unwrap();
    assert_eq!(ctx.github.refs(&ctx.repo_ref), after_first);

    ctx.publish("1.2.0", "").await.unwrap();
    let refs = ctx.github.refs(&ctx.repo_ref);
    assert_eq!(
        refs.keys().collect::<Vec<_>>(),
        vec!["refs/heads/main", "refs/tags/v1.0.0", "refs/tags/v1.2.0"],
        "a 1.1.0 não publicada nunca vai"
    );
    assert_eq!(
        ctx.github.main_log(&ctx.repo_ref),
        vec!["Vale Sereno 1.2.0", "Vale Sereno 1.0.0"],
        "nenhum commit de trabalho na main remota"
    );
    let files = ctx.github.main_files(&ctx.repo_ref);
    assert_eq!(files["config/extra.toml"], b"x = 1\n");
    let changelog = String::from_utf8(files["CHANGELOG.md"].clone()).unwrap();
    let newer = changelog.find("## 1.2.0").unwrap();
    let older = changelog.find("## 1.0.0").unwrap();
    assert!(newer < older, "a mais nova no topo:\n{changelog}");
    assert!(changelog.contains("Primeira versão."));

    // O histórico local mostra as publicadas e a ponta da linha bate com a main remota.
    let repo = ctx.repo();
    let versions = repo.versions().unwrap();
    let published: Vec<_> = versions
        .iter()
        .filter(|v| v.is_published)
        .map(|v| v.version.as_str())
        .collect();
    assert_eq!(published, vec!["1.2.0", "1.0.0"]);
    assert_eq!(
        repo.publish_line_tip().unwrap().unwrap().to_string(),
        refs["refs/heads/main"]
    );
    assert!(
        repo.published_versions()
            .unwrap()
            .iter()
            .all(|v| !v.release_pending)
    );
    let releases = ctx.github.releases();
    assert_eq!(releases.len(), 2);
    assert_eq!(releases[0].name, "Vale Sereno 1.0.0");
    assert_eq!(releases[0].body, "Primeira versão.");
}

#[tokio::test]
async fn ca_t18_07_bytes_preservados_inclusive_crlf() {
    let ctx = Ctx::new().await;
    let pack = &ctx.pack;
    pack.write("config/windows.cfg", b"# config\r\nB:ligado=true\r\n");
    pack.write("config/unix.toml", b"a = 1\n");
    pack.write("kubejs/data/acao.js", "// ação\r\n".as_bytes());
    save(&ctx.repo(), pack, "1.0.0", 10, true);
    ctx.publish("1.0.0", "").await.unwrap();

    let files = ctx.github.main_files(&ctx.repo_ref);
    assert_eq!(files[".gitattributes"], GITATTRIBUTES.as_bytes());
    assert_eq!(GITATTRIBUTES, "* -text\n");
    let index = PackIndex::parse(std::str::from_utf8(&files["index.toml"]).unwrap())
        .unwrap()
        .value;
    let entries = index.normalized_entries();
    assert_eq!(entries.len(), 3);
    for entry in entries {
        assert_eq!(
            sha256_hex(&files[&entry.file]),
            entry.hash,
            "{} mudou no caminho",
            entry.file
        );
    }
    assert_eq!(
        files["config/windows.cfg"],
        b"# config\r\nB:ligado=true\r\n"
    );
    let manifest = PackManifest::parse(std::str::from_utf8(&files["pack.toml"]).unwrap())
        .unwrap()
        .value;
    assert_eq!(manifest.index.hash, sha256_hex(&files["index.toml"]));
}

#[tokio::test]
async fn ca_t18_05_notas_comparam_com_a_ultima_publicada() {
    let ctx = Ctx::new().await;
    let pack = &ctx.pack;
    pack.add_modrinth("create", "Create", "LNytGWDc", "c1", Side::Both);
    pack.add_modrinth("jei", "Just Enough Items", "u6dRKJwZ", "j1", Side::Both);
    save(&ctx.repo(), pack, "1.0.0", 10, true);
    ctx.publish("1.0.0", "").await.unwrap();

    pack.add_modrinth("sodium", "Sodium", "AANobbMI", "s1", Side::Client);
    save(&ctx.repo(), pack, "1.1.0", 20, true); // salva, nunca publicada
    pack.remove("mods/create.pw.toml");
    pack.add_modrinth("jei", "Just Enough Items", "u6dRKJwZ", "j2", Side::Both);
    save(&ctx.repo(), pack, "1.2.0", 30, true);

    let repo = ctx.repo();
    assert_eq!(
        repo.publication_base().unwrap(),
        Snapshot::Version("1.0.0".into())
    );
    let changes = repo.publication_changes("1.2.0", &FileNames).unwrap();
    let notes = changelog::render_body("", &changes);
    assert!(notes.contains("### Atenção"), "{notes}");
    assert!(notes.contains("### Mods adicionados\n- Sodium"), "{notes}");
    assert!(notes.contains("### Mods removidos\n- Create"), "{notes}");
    assert!(
        notes.contains("### Mods atualizados\n- Just Enough Items"),
        "{notes}"
    );
    // Desde a última salva (1.1.0), o Sodium não apareceria: a base é a última publicada.
    let since_saved = repo
        .changes(
            &Snapshot::Version("1.1.0".into()),
            &Snapshot::Version("1.2.0".into()),
            &FileNames,
        )
        .unwrap();
    assert!(!changelog::render_body("", &since_saved).contains("Sodium"));

    let published = ctx.publish("1.2.0", &notes).await.unwrap();
    assert_eq!(published.notes, notes.trim_end());
    assert_eq!(ctx.github.releases()[1].body, notes.trim_end());
}

#[tokio::test]
async fn ca_t18_02_o_token_nunca_aparece_em_config_registros_erros_ou_debug() {
    let ctx = Ctx::new().await;
    ctx.pack.write("config/a.toml", b"a = 1\n");
    save(&ctx.repo(), &ctx.pack, "1.0.0", 10, true);

    let logs = Arc::new(Mutex::new(Vec::<u8>::new()));
    let writer = {
        let logs = Arc::clone(&logs);
        move || Capture(Arc::clone(&logs))
    };
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::TRACE)
        .with_writer(writer)
        .finish();
    let guard = tracing::subscriber::set_default(subscriber);
    let published = ctx.publish("1.0.0", "").await.unwrap();

    // Token errado: a mensagem e o detalhe técnico não trazem nenhum dos dois.
    let wrong = "ghp_tokenErradoQueTambemNaoPodeVazar99999";
    let error = ctx.github.client_with(wrong).user(None).await.unwrap_err();
    assert!(
        matches!(error, Error::GithubTokenRejected { .. }),
        "{error:?}"
    );
    drop(guard);

    let mut texts = vec![
        String::from_utf8(logs.lock().unwrap().clone()).unwrap(),
        error.to_string(),
        error.detail().unwrap_or_default(),
        format!("{error:?}"),
        format!("{:?}", error.params()),
        format!("{published:?}"),
        format!("{:?}", ctx.github.client()),
        std::fs::read_to_string(ctx.pack.root.join(".git/config")).unwrap(),
    ];
    for entry in walk(&ctx.pack.root.join(".git")) {
        texts.push(String::from_utf8_lossy(&std::fs::read(entry).unwrap()).into_owned());
    }
    assert!(
        texts[0].contains("versão enviada ao GitHub"),
        "o registro foi capturado"
    );
    for text in &texts {
        for secret in [TOKEN, wrong, "x-access-token:"] {
            assert!(!text.contains(secret), "vazou {secret} em: {text:.300}");
        }
    }
    assert!(
        !texts[7].contains("[remote"),
        "nenhum remoto gravado no .git/config"
    );
    // O servidor recebeu a credencial em memória, no cabeçalho.
    let authorizations = ctx.github.state.lock().unwrap().authorizations.clone();
    assert!(authorizations.iter().any(|a| a.starts_with("Basic ")));
    assert!(
        authorizations
            .iter()
            .any(|a| a == &format!("Bearer {TOKEN}"))
    );
}

struct Capture(Arc<Mutex<Vec<u8>>>);

impl std::io::Write for Capture {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

fn walk(dir: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            out.extend(walk(&path));
        } else {
            out.push(path);
        }
    }
    out
}

#[tokio::test]
async fn divergencia_bloqueia_e_a_substituicao_so_vale_para_o_commit_visto() {
    let ctx = Ctx::new().await;
    ctx.pack.write("config/a.toml", b"a = 1\n");
    save(&ctx.repo(), &ctx.pack, "1.0.0", 10, true);
    ctx.publish("1.0.0", "").await.unwrap();

    // Alguém mudou a main pelo site do GitHub.
    let bare = ctx.github.bare(&ctx.repo_ref);
    let main = bare
        .find_reference("refs/heads/main")
        .unwrap()
        .peel_to_commit()
        .unwrap();
    let sig = git2::Signature::now("Outra pessoa", "x@y").unwrap();
    let other = bare
        .commit(
            None,
            &sig,
            &sig,
            "Editado no site",
            &main.tree().unwrap(),
            &[&main],
        )
        .unwrap();
    bare.reference("refs/heads/main", other, true, "site")
        .unwrap();

    let client = ctx.github.client();
    let cancel = CancellationToken::new();
    let status = remote_status(&client, &ctx.pack.root, &ctx.repo_ref, &cancel)
        .await
        .unwrap();
    assert!(!status.in_sync);
    assert_eq!(
        status.remote_main.as_deref(),
        Some(other.to_string().as_str())
    );

    ctx.pack.write("config/a.toml", b"a = 2\n");
    save(&ctx.repo(), &ctx.pack, "1.1.0", 20, true);
    let error = ctx.publish("1.1.0", "").await.unwrap_err();
    assert!(matches!(error, Error::RemoteDiverged { .. }), "{error:?}");
    assert_eq!(error.params()["repo"], format!("{LOGIN}/vale-sereno"));
    assert_eq!(ctx.github.main_log(&ctx.repo_ref)[0], "Editado no site");
    assert!(!ctx.repo().versions().unwrap()[0].is_published);

    // Confirmação para um commit que não é mais o da main: recusa de novo.
    let stale = ReplaceRemote {
        expected_main: Some(main.id().to_string()),
    };
    assert!(matches!(
        ctx.publish_replacing("1.1.0", "", Some(&stale)).await,
        Err(Error::RemoteDiverged { .. })
    ));
    // Para o commit visto: substitui.
    let seen = ReplaceRemote {
        expected_main: status.remote_main.clone(),
    };
    ctx.publish_replacing("1.1.0", "", Some(&seen))
        .await
        .unwrap();
    assert_eq!(
        ctx.github.main_log(&ctx.repo_ref),
        vec!["Vale Sereno 1.1.0", "Vale Sereno 1.0.0"]
    );
    assert!(
        remote_status(&client, &ctx.pack.root, &ctx.repo_ref, &cancel)
            .await
            .unwrap()
            .in_sync
    );
}

#[tokio::test]
async fn main_que_muda_durante_o_envio_vira_divergencia_e_nada_e_registrado() {
    let ctx = Ctx::new().await;
    ctx.pack.write("config/a.toml", b"a = 1\n");
    save(&ctx.repo(), &ctx.pack, "1.0.0", 10, true);
    ctx.publish("1.0.0", "").await.unwrap();
    ctx.pack.write("config/a.toml", b"a = 2\n");
    save(&ctx.repo(), &ctx.pack, "1.1.0", 20, true);

    // A main muda entre a leitura das referências e o envio: o servidor recusa a troca.
    ctx.github.state.lock().unwrap().next_push = Some(PushFault::RaceMain);
    let error = ctx.publish("1.1.0", "").await.unwrap_err();
    assert!(matches!(error, Error::RemoteDiverged { .. }), "{error:?}");
    assert_eq!(ctx.github.main_log(&ctx.repo_ref)[0], "Editado no site");
    assert!(
        !ctx.github
            .refs(&ctx.repo_ref)
            .contains_key("refs/tags/v1.1.0"),
        "a tag da versão também não entra"
    );
    assert!(!ctx.repo().versions().unwrap()[0].is_published);
    assert_eq!(ctx.github.releases().len(), 1);
}

#[tokio::test]
async fn envio_que_chegou_mas_perdeu_a_resposta_e_registrado_ao_repetir_sem_duplicar() {
    let ctx = Ctx::new().await;
    ctx.pack.write("config/a.toml", b"a = 1\n");
    save(&ctx.repo(), &ctx.pack, "1.0.0", 10, true);
    ctx.publish("1.0.0", "Primeira.").await.unwrap();
    ctx.pack.write("config/a.toml", b"a = 2\n");
    save(&ctx.repo(), &ctx.pack, "1.1.0", 20, true);

    ctx.github.state.lock().unwrap().next_push = Some(PushFault::LoseResponse);
    let error = ctx.publish("1.1.0", "Segunda.").await.unwrap_err();
    assert!(matches!(error, Error::GithubFailed { .. }), "{error:?}");
    let refs = ctx.github.refs(&ctx.repo_ref);
    assert!(
        refs.contains_key("refs/tags/v1.1.0"),
        "o servidor aplicou o envio"
    );
    assert!(
        !ctx.repo().versions().unwrap()[0].is_published,
        "aqui não registrou"
    );

    // Repetir não acusa divergência nem envia outra vez: registra e cria a Release.
    let published = ctx.publish("1.1.0", "Segunda.").await.unwrap();
    assert_eq!(published.notes, "Segunda.");
    assert_eq!(ctx.github.refs(&ctx.repo_ref), refs, "nada enviado de novo");
    assert_eq!(
        ctx.github.main_log(&ctx.repo_ref),
        vec!["Vale Sereno 1.1.0", "Vale Sereno 1.0.0"]
    );
    let releases = ctx.github.releases();
    assert_eq!(releases.len(), 2);
    assert_eq!(releases[1].name, "Vale Sereno 1.1.0");
    assert_eq!(releases[1].body, "Segunda.");
    let repo = ctx.repo();
    assert!(repo.versions().unwrap()[0].is_published);
    assert_eq!(
        repo.publish_line_tip().unwrap().unwrap().to_string(),
        refs["refs/heads/main"]
    );
    assert!(
        repo.published_versions()
            .unwrap()
            .iter()
            .all(|v| !v.release_pending)
    );
}

#[tokio::test]
async fn repositorio_existente_com_conteudo_e_divergencia_na_primeira_publicacao() {
    let ctx = Ctx::new().await;
    let bare = ctx.github.bare(&ctx.repo_ref);
    let sig = git2::Signature::now("x", "x@y").unwrap();
    let tree = bare.treebuilder(None).unwrap().write().unwrap();
    let tree = bare.find_tree(tree).unwrap();
    let readme = bare
        .commit(Some("refs/heads/main"), &sig, &sig, "README", &tree, &[])
        .unwrap();
    ctx.pack.write("config/a.toml", b"a = 1\n");
    save(&ctx.repo(), &ctx.pack, "1.0.0", 10, true);
    assert!(matches!(
        ctx.publish("1.0.0", "").await,
        Err(Error::RemoteDiverged { .. })
    ));
    let seen = ReplaceRemote {
        expected_main: Some(readme.to_string()),
    };
    ctx.publish_replacing("1.0.0", "", Some(&seen))
        .await
        .unwrap();
    assert_eq!(
        ctx.github.main_log(&ctx.repo_ref),
        vec!["Vale Sereno 1.0.0"]
    );
}

#[tokio::test]
async fn release_que_falha_fica_pendente_e_tentar_de_novo_cria_so_ela() {
    let ctx = Ctx::new().await;
    ctx.pack.write("config/a.toml", b"a = 1\n");
    save(&ctx.repo(), &ctx.pack, "1.0.0", 10, true);
    ctx.github.state.lock().unwrap().fail_releases = 1;

    let error = ctx.publish("1.0.0", "Notas.").await.unwrap_err();
    assert!(matches!(error, Error::ReleasePending { .. }), "{error:?}");
    assert_eq!(error.params()["version"], "1.0.0");
    assert!(error.retryable());
    let repo = ctx.repo();
    assert!(repo.versions().unwrap()[0].is_published);
    assert!(repo.published_versions().unwrap()[0].release_pending);
    let refs = ctx.github.refs(&ctx.repo_ref);
    assert!(refs.contains_key("refs/tags/v1.0.0"), "o envio aconteceu");
    assert!(ctx.github.releases().is_empty());

    let done = retry_release(&ctx.github.client(), &ctx.pack.root, &ctx.repo_ref, "1.0.0")
        .await
        .unwrap();
    assert_eq!(done.release.id, 1000);
    assert_eq!(done.notes, "Notas.");
    assert_eq!(ctx.github.releases()[0].name, "Vale Sereno 1.0.0");
    assert!(!ctx.repo().published_versions().unwrap()[0].release_pending);
    assert_eq!(ctx.github.refs(&ctx.repo_ref), refs, "nada enviado de novo");
    // Repetir não cria outra.
    retry_release(&ctx.github.client(), &ctx.pack.root, &ctx.repo_ref, "1.0.0")
        .await
        .unwrap();
    assert_eq!(ctx.github.releases().len(), 1);
}

#[tokio::test]
async fn so_versao_final_nova_e_nao_publicada_pode_ser_publicada() {
    let ctx = Ctx::new().await;
    ctx.pack.write("config/a.toml", b"a = 1\n");
    save(&ctx.repo(), &ctx.pack, "1.0.0", 10, false);
    assert!(matches!(
        ctx.publish("1.0.0", "").await,
        Err(Error::NotFinal { .. })
    ));
    assert!(
        ctx.github.state.lock().unwrap().authorizations.is_empty(),
        "nada pedido à rede"
    );
    ctx.repo().set_final("1.0.0", true).unwrap();
    ctx.pack.write("config/a.toml", b"a = 2\n");
    save(&ctx.repo(), &ctx.pack, "1.1.0", 20, true);
    ctx.publish("1.1.0", "").await.unwrap();
    assert!(matches!(
        ctx.publish("1.1.0", "").await,
        Err(Error::AlreadyPublished { .. })
    ));
    let error = ctx.publish("1.0.0", "").await.unwrap_err();
    assert!(matches!(error, Error::PublishNotNewer { .. }), "{error:?}");
    assert_eq!(error.params()["last"], "1.1.0");
    // Versão publicada não deixa de ser final.
    assert!(matches!(
        ctx.repo().set_final("1.1.0", false),
        Err(Error::VersionPublished { .. })
    ));
}

#[tokio::test]
async fn permissoes_e_token_recusado_no_envio() {
    let ctx = Ctx::new().await;
    ctx.pack.write("config/a.toml", b"a = 1\n");
    save(&ctx.repo(), &ctx.pack, "1.0.0", 10, true);
    ctx.github.state.lock().unwrap().deny_push = true;
    let error = ctx.publish("1.0.0", "").await.unwrap_err();
    assert!(
        matches!(
            error,
            Error::GithubPermissionMissing {
                permission: GithubPermission::Push,
                ..
            }
        ),
        "{error:?}"
    );
    assert_eq!(error.params()["permission"], "Contents");
    assert!(
        !ctx.repo().versions().unwrap()[0].is_published,
        "nada registrado"
    );
    assert!(ctx.repo().publish_line_tip().unwrap().is_none());

    let tree = tree_of(&ctx.repo(), "1.0.0");
    let id = identity();
    let error = publish(
        &ctx.github
            .client_with("ghp_outroTokenQualquer000000000000000"),
        &ctx.pack.root,
        &ctx.repo_ref,
        &PublishRequest {
            version: "1.0.0",
            tree: &tree,
            notes: "",
            pack_name: "Vale Sereno",
            identity: &id,
            when: at(1),
        },
        None,
        &CancellationToken::new(),
    )
    .await
    .unwrap_err();
    assert!(
        matches!(error, Error::GithubTokenRejected { .. }),
        "{error:?}"
    );

    let missing = RepoRef::parse(&format!("{LOGIN}/nao-existe")).unwrap();
    let error = remote_status(
        &ctx.github.client(),
        &ctx.pack.root,
        &missing,
        &CancellationToken::new(),
    )
    .await
    .unwrap_err();
    assert!(
        matches!(error, Error::GithubRepoNotFound { .. }),
        "{error:?}"
    );
}

#[tokio::test]
async fn api_criar_vincular_e_erros() {
    let github = FakeGithub::start().await;
    let client = github.client();
    assert_eq!(client.user(None).await.unwrap(), LOGIN);
    let created = client
        .create_repo(LOGIN, "vale-sereno", false, "Modpack")
        .await
        .unwrap();
    assert_eq!(created.repo.full_name(), format!("{LOGIN}/vale-sereno"));
    assert!(!created.private);
    let private = client.create_repo(LOGIN, "backup", true, "").await.unwrap();
    assert!(private.private);
    assert!(matches!(
        client.create_repo(LOGIN, "vale-sereno", false, "").await,
        Err(Error::GithubRepoNameTaken { .. })
    ));
    assert!(matches!(
        client.create_repo(LOGIN, "nome inválido", false, "").await,
        Err(Error::GithubInvalidRepoName { .. })
    ));
    let linked = client
        .repo(&RepoRef::parse(&format!("{LOGIN}/backup")).unwrap())
        .await
        .unwrap();
    assert!(linked.private);
    assert!(matches!(
        client.repo(&RepoRef::parse("alguem/nada").unwrap()).await,
        Err(Error::GithubRepoNotFound { .. })
    ));
    github.state.lock().unwrap().deny_create = true;
    let error = client
        .create_repo(LOGIN, "outro", false, "")
        .await
        .unwrap_err();
    assert!(
        matches!(
            error,
            Error::GithubPermissionMissing {
                permission: GithubPermission::CreateRepo,
                ..
            }
        ),
        "{error:?}"
    );
}

#[tokio::test]
async fn sem_conexao_vira_erro_de_github_offline() {
    // Porta fechada em 127.0.0.1: recusa a conexão na hora.
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    let base = format!("http://127.0.0.1:{port}");
    let config = warden_http::HttpConfig {
        max_retries: 0,
        ..warden_http::HttpConfig::default()
    };
    let client = warden_versioning::github::GithubClient::new(
        warden_http::HttpClient::new(config).unwrap(),
        warden_versioning::github::GithubEndpoints::new(&base, &base, &base).unwrap(),
        secrecy::SecretString::from(TOKEN.to_owned()),
    );
    let error = client.user(None).await.unwrap_err();
    assert!(matches!(error, Error::GithubOffline { .. }), "{error:?}");
    assert!(error.retryable());
}
