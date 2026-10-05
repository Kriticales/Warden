//! Repositório do pack: criar com ponto inicial, abrir, alterações não salvas, estados
//! especiais e packs importados que já têm histórico (ARCHITECTURE §11).

#![allow(linker_messages)]
// libgit2 no MSVC exporta símbolos; veja o relatório da V-01
// Auxiliares fora de `#[test]`: falhar com pânico reprova o teste que chamou.
#![allow(clippy::unwrap_used, clippy::panic)]

mod common;

use std::fs;

use common::{TestPack, at, git_status, identity, raw};
use warden_core::DomainError as _;
use warden_versioning::{
    CREATED_MESSAGE, Error, FileChange, FileChangeKind, IMPORTED_MESSAGE, InitialPoint,
    MAIN_BRANCH, PackRepo, SaveVersion, VersioningErrorCode,
};

fn change(path: &str, kind: FileChangeKind) -> FileChange {
    FileChange {
        path: path.to_owned(),
        kind,
    }
}

#[test]
fn criar_grava_ponto_inicial_na_main_com_tudo_menos_o_ignorado() {
    let pack = TestPack::new();
    pack.add_modrinth(
        "sodium",
        "Sodium",
        "AANobbMI",
        "v1",
        warden_packwiz::Side::Client,
    );
    pack.write("config/ação.toml", "chave = \"valor\"\n".as_bytes());
    pack.write("logs/latest.log", b"ignorado");
    pack.write("mods/.connector/cache.bin", b"ignorado");

    let repo = PackRepo::init(&pack.root, InitialPoint::Created, &identity(), at(0)).unwrap();

    assert_eq!(repo.branch_name().unwrap().as_deref(), Some(MAIN_BRANCH));
    assert!(repo.unsaved_changes().unwrap().is_empty());
    assert!(git_status(&pack).is_empty(), "{:?}", git_status(&pack));
    let git = raw(&pack);
    let head = git.head().unwrap().peel_to_commit().unwrap();
    assert_eq!(head.message().unwrap(), CREATED_MESSAGE);
    assert_eq!(head.author().name().unwrap(), "Autor de Teste");
    assert_eq!(head.author().email().unwrap(), "warden@localhost");
    assert_eq!(head.time().seconds(), common::T0);
    assert_eq!(head.time().offset_minutes(), -180);
    let tree = head.tree().unwrap();
    for present in [
        ".gitignore",
        ".gitattributes",
        "pack.toml",
        "index.toml",
        "mods/sodium.pw.toml",
        "config/ação.toml",
    ] {
        assert!(
            tree.get_path(std::path::Path::new(present)).is_ok(),
            "{present} faltando no ponto inicial"
        );
    }
    assert!(
        tree.get_path(std::path::Path::new("logs/latest.log"))
            .is_err()
    );
    assert!(
        tree.get_path(std::path::Path::new("mods/.connector/cache.bin"))
            .is_err()
    );
    let config = git
        .config()
        .unwrap()
        .open_level(git2::ConfigLevel::Local)
        .unwrap();
    assert!(!config.get_bool("core.autocrlf").unwrap());
    assert!(config.get_bool("core.longpaths").unwrap());
}

#[test]
fn importado_sem_historico_recebe_pack_importado() {
    let pack = TestPack::new();
    PackRepo::init(&pack.root, InitialPoint::Imported, &identity(), at(0)).unwrap();
    let git = raw(&pack);
    let head = git.head().unwrap().peel_to_commit().unwrap();
    assert_eq!(head.message().unwrap(), IMPORTED_MESSAGE);
}

#[test]
fn criar_duas_vezes_e_recusado() {
    let (pack, _repo) = TestPack::versioned();
    let error = PackRepo::init(&pack.root, InitialPoint::Created, &identity(), at(1)).unwrap_err();
    assert!(matches!(error, Error::AlreadyVersioned { .. }), "{error}");
    assert_eq!(
        error.code(),
        warden_core::DomainCode::Domain(VersioningErrorCode::AlreadyVersioned)
    );
}

#[test]
fn abrir_pasta_sem_historico() {
    let pack = TestPack::new();
    let error = PackRepo::open(&pack.root).unwrap_err();
    assert!(matches!(error, Error::NotVersioned { .. }), "{error}");
    assert_eq!(error.params()["path"], pack.root.display().to_string());
}

#[test]
fn pack_dentro_de_outro_repositorio_nao_usa_o_historico_de_fora() {
    let outer = tempfile::tempdir().unwrap();
    git2::Repository::init(outer.path()).unwrap();
    let inner = outer.path().join("packs").join("meu pack");
    fs::create_dir_all(&inner).unwrap();
    fs::write(inner.join("pack.toml"), b"name = \"x\"\n").unwrap();
    assert!(matches!(
        PackRepo::open(&inner).unwrap_err(),
        Error::NotVersioned { .. }
    ));
    // E criar um repositório ali dentro funciona, independente do de fora.
    let repo = PackRepo::init(&inner, InitialPoint::Imported, &identity(), at(0)).unwrap();
    assert!(repo.unsaved_changes().unwrap().is_empty());
}

#[test]
fn alteracoes_nao_salvas_com_acentos_espacos_e_ignorados() {
    let (pack, repo) = TestPack::versioned();
    pack.write("config/ação.toml", b"a = 1\n");
    pack.write("config/meu mod/opções da tela.json", b"{}\n");
    pack.write("index.toml", b"hash-format = \"sha256\"\n\n[[files]]\n");
    pack.remove(".gitattributes");
    pack.write("logs/latest.log", b"ignorado");
    pack.write("crash-reports/crash.txt", b"ignorado");
    pack.write("mods/sodium.jar.warden-tmp", b"temporario");

    let changes = repo.unsaved_changes().unwrap();
    assert_eq!(
        changes,
        vec![
            change(".gitattributes", FileChangeKind::Deleted),
            change("config/ação.toml", FileChangeKind::Added),
            change("config/meu mod/opções da tela.json", FileChangeKind::Added),
            change("index.toml", FileChangeKind::Modified),
        ]
    );
}

#[test]
fn alteracoes_nao_salvas_ignoram_o_indice_deixado_diferente_por_fora() {
    let (pack, repo) = TestPack::versioned();
    pack.write("config/a.toml", b"a = 1\n");
    // Alguém roda `git add` por fora: o arquivo continua "não salvo".
    let git = raw(&pack);
    let mut index = git.index().unwrap();
    index
        .add_path(std::path::Path::new("config/a.toml"))
        .unwrap();
    index.write().unwrap();
    assert_eq!(
        repo.unsaved_changes().unwrap(),
        vec![change("config/a.toml", FileChangeKind::Added)]
    );
    // E se o arquivo some do disco depois do `git add`, não há nada para salvar.
    pack.remove("config/a.toml");
    assert!(repo.unsaved_changes().unwrap().is_empty());
}

#[test]
fn arquivo_ja_no_historico_conta_mesmo_se_ignorado_depois() {
    let (pack, repo) = TestPack::versioned();
    pack.write("notas.log", b"versionado de proposito");
    let git = raw(&pack);
    let mut index = git.index().unwrap();
    index.add_path(std::path::Path::new("notas.log")).unwrap();
    index.write().unwrap();
    repo.save_version(&SaveVersion {
        version: "0.1.0",
        tag_message: "x",
        identity: &identity(),
        when: at(1),
        mark_final: false,
    })
    .unwrap();
    pack.write("notas.log", b"mudou");
    assert_eq!(
        repo.unsaved_changes().unwrap(),
        vec![change("notas.log", FileChangeKind::Modified)]
    );
}

#[test]
fn merge_em_andamento_bloqueia_escrita() {
    let (pack, repo) = TestPack::versioned();
    let head = raw(&pack).head().unwrap().target().unwrap();
    fs::write(pack.path(".git/MERGE_HEAD"), format!("{head}\n")).unwrap();
    let error = repo.check_writable().unwrap_err();
    assert!(
        matches!(&error, Error::RepoBusy { state } if state == "merge"),
        "{error}"
    );
    assert_eq!(error.params()["state"], "merge");
    pack.write("config/a.toml", b"a = 1\n");
    let saved = repo.save_version(&SaveVersion {
        version: "0.1.0",
        tag_message: "x",
        identity: &identity(),
        when: at(1),
        mark_final: false,
    });
    assert!(matches!(saved, Err(Error::RepoBusy { .. })));
    // Leituras continuam funcionando.
    assert_eq!(repo.unsaved_changes().unwrap().len(), 1);
}

#[test]
fn rebase_em_andamento_bloqueia_escrita() {
    let (pack, repo) = TestPack::versioned();
    fs::create_dir_all(pack.path(".git/rebase-merge")).unwrap();
    fs::write(pack.path(".git/rebase-merge/interactive"), b"").unwrap();
    assert!(matches!(
        repo.check_writable().unwrap_err(),
        Error::RepoBusy { state } if state == "rebase"
    ));
}

#[test]
fn head_destacado_bloqueia_escrita() {
    let (pack, repo) = TestPack::versioned();
    let git = raw(&pack);
    let head = git.head().unwrap().target().unwrap();
    git.set_head_detached(head).unwrap();
    assert!(matches!(
        repo.check_writable().unwrap_err(),
        Error::DetachedHead
    ));
    assert_eq!(repo.branch_name().unwrap(), None);
}

#[test]
fn conflitos_bloqueiam_escrita() {
    let (pack, repo) = TestPack::versioned();
    let git = raw(&pack);
    let mut index = git.index().unwrap();
    let blob = git.blob(b"conflito\n").unwrap();
    let entry = |stage: u16| git2::IndexEntry {
        ctime: git2::IndexTime::new(0, 0),
        mtime: git2::IndexTime::new(0, 0),
        dev: 0,
        ino: 0,
        mode: 0o100_644,
        uid: 0,
        gid: 0,
        file_size: 9,
        id: blob,
        flags: stage << 12,
        flags_extended: 0,
        path: b"config/x.toml".to_vec(),
    };
    index.add(&entry(1)).unwrap();
    index.add(&entry(2)).unwrap();
    index.add(&entry(3)).unwrap();
    index.write().unwrap();
    let error = repo.check_writable().unwrap_err();
    assert!(
        matches!(&error, Error::Conflicts { paths } if paths == &["config/x.toml"]),
        "{error}"
    );
}

#[test]
fn trava_de_outro_processo_vira_repo_travado() {
    let (pack, repo) = TestPack::versioned();
    fs::write(pack.path(".git/index.lock"), b"").unwrap();
    pack.write("config/a.toml", b"a = 1\n");
    let error = repo
        .save_version(&SaveVersion {
            version: "0.1.0",
            tag_message: "x",
            identity: &identity(),
            when: at(1),
            mark_final: false,
        })
        .unwrap_err();
    assert!(matches!(error, Error::RepoLocked { .. }), "{error}");
    assert!(error.retryable());
    // Nada foi gravado: o HEAD continua no ponto inicial.
    let git = raw(&pack);
    let head = git.head().unwrap().peel_to_commit().unwrap();
    assert_eq!(head.message().unwrap(), CREATED_MESSAGE);
}

/// Pack importado com histórico próprio: branch `trunk`, commits feitos fora do Warden e uma
/// tag de versão antiga.
#[test]
#[allow(clippy::too_many_lines)] // monta um histórico inteiro feito fora do Warden
fn importado_com_historico_mantem_branch_e_usa_a_ultima_tag_alcancavel() {
    let pack = TestPack::new();
    let git = git2::Repository::init_opts(
        &pack.root,
        git2::RepositoryInitOptions::new().initial_head("trunk"),
    )
    .unwrap();
    let signature = git2::Signature::new(
        "Outra Pessoa",
        "outra@exemplo.com",
        &git2::Time::new(common::T0 - 86_400, 0),
    )
    .unwrap();
    let commit_all = |message: &str, parents: &[&git2::Commit<'_>]| {
        let mut index = git.index().unwrap();
        index
            .add_all(["*"], git2::IndexAddOption::DEFAULT, None)
            .unwrap();
        index.write().unwrap();
        let tree = git.find_tree(index.write_tree().unwrap()).unwrap();
        git.commit(
            Some("HEAD"),
            &signature,
            &signature,
            message,
            &tree,
            parents,
        )
        .unwrap()
    };
    let first = commit_all("primeiro commit de fora", &[]);
    let first = git.find_commit(first).unwrap();
    git.tag(
        "v1.0.0",
        first.as_object(),
        &signature,
        "versão de fora",
        false,
    )
    .unwrap();
    pack.write("config/a.toml", b"a = 1\n");
    let second = commit_all("segundo commit de fora", &[&first]);
    // Tag leve maior, mas numa branch que não é a atual.
    let other = git
        .commit(
            None,
            &signature,
            &signature,
            "outra branch",
            &first.tree().unwrap(),
            &[&first],
        )
        .unwrap();
    git.tag_lightweight("v5.0.0", &git.find_object(other, None).unwrap(), false)
        .unwrap();
    git.tag_lightweight("nao-e-versao", first.as_object(), false)
        .unwrap();

    let repo = PackRepo::open(&pack.root).unwrap();
    assert_eq!(repo.branch_name().unwrap().as_deref(), Some("trunk"));
    assert_eq!(repo.last_version().unwrap().unwrap().to_string(), "1.0.0");
    assert_eq!(
        repo.highest_version().unwrap().unwrap().to_string(),
        "5.0.0"
    );
    let versions = repo.versions().unwrap();
    assert_eq!(versions.len(), 2);
    assert_eq!(versions[0].version, "5.0.0");
    assert!(!versions[0].reachable);
    assert_eq!(versions[0].message, "");
    assert_eq!(versions[1].version, "1.0.0");
    assert!(versions[1].reachable);
    assert_eq!(versions[1].message, "versão de fora");
    assert!(repo.unsaved_changes().unwrap().is_empty());

    // Salvar exige ser maior que todas (5.0.0) e grava na `trunk`, sem renomear.
    pack.write("config/a.toml", b"a = 2\n");
    let saved_low = repo.save_version(&SaveVersion {
        version: "2.0.0",
        tag_message: "x",
        identity: &identity(),
        when: at(1),
        mark_final: false,
    });
    assert!(matches!(saved_low, Err(Error::VersionNotGreater { ref last, .. }) if last == "5.0.0"));
    let saved = repo
        .save_version(&SaveVersion {
            version: "5.1.0",
            tag_message: "nova",
            identity: &identity(),
            when: at(1),
            mark_final: false,
        })
        .unwrap();
    let trunk = git
        .find_reference("refs/heads/trunk")
        .unwrap()
        .target()
        .unwrap();
    assert_eq!(trunk.to_string(), saved.commit);
    assert_eq!(
        git.find_commit(trunk).unwrap().parent_id(0).unwrap(),
        second
    );
    assert!(git.find_reference("refs/heads/main").is_err());
    assert_eq!(repo.last_version().unwrap().unwrap().to_string(), "5.1.0");
}

#[test]
fn repositorio_sem_commits_ainda() {
    let pack = TestPack::new();
    git2::Repository::init(&pack.root).unwrap();
    let repo = PackRepo::open(&pack.root).unwrap();
    assert_eq!(repo.head_id().unwrap(), None);
    assert_eq!(repo.last_version().unwrap(), None);
    let changes = repo.unsaved_changes().unwrap();
    assert!(changes.iter().all(|c| c.kind == FileChangeKind::Added));
    assert!(changes.iter().any(|c| c.path == "pack.toml"));
    let saved = repo
        .save_version(&SaveVersion {
            version: "0.1.0",
            tag_message: "primeira",
            identity: &identity(),
            when: at(1),
            mark_final: false,
        })
        .unwrap();
    assert_eq!(repo.head_id().unwrap(), Some(saved.commit));
    assert!(repo.unsaved_changes().unwrap().is_empty());
}

#[test]
fn arquivo_tirado_do_indice_por_fora_continua_salvo() {
    let (pack, repo) = TestPack::versioned();
    // `git rm --cached pack.toml` por fora: o arquivo continua no disco, igual ao do HEAD.
    let git = raw(&pack);
    let mut index = git.index().unwrap();
    index
        .remove_path(std::path::Path::new("pack.toml"))
        .unwrap();
    index.write().unwrap();
    assert!(repo.unsaved_changes().unwrap().is_empty());
    pack.write("pack.toml", b"name = \"mudou\"\n");
    assert_eq!(
        repo.unsaved_changes().unwrap(),
        vec![change("pack.toml", FileChangeKind::Modified)]
    );
}
