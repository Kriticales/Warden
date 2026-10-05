//! Salvar versão, validar o número e marcar versão final (SPEC T16 e T17; CA-T16-03 a CA-T16-05
//! no nível da crate).

#![allow(linker_messages)]
// libgit2 no MSVC exporta símbolos; veja o relatório da V-01
// Auxiliares fora de `#[test]`: falhar com pânico reprova o teste que chamou.
#![allow(clippy::unwrap_used, clippy::panic)]

mod common;

use common::{TestPack, at, git_status, identity, raw};
use warden_core::DomainError as _;
use warden_versioning::{
    Error, FINAL_REF_PREFIX, PUBLISH_TAG_PREFIX, PackRepo, SaveVersion, SavedVersion,
};

fn save(repo: &PackRepo, version: &str, minutes: i64) -> Result<SavedVersion, Error> {
    repo.save_version(&SaveVersion {
        version,
        tag_message: &format!("## {version} — 2026-10-01\n\n- mudança\n"),
        identity: &identity(),
        when: at(minutes),
        mark_final: false,
    })
}

#[test]
fn ca_t16_03_salvar_grava_commit_e_tag_anotada_e_deixa_tudo_limpo() {
    let (pack, repo) = TestPack::versioned();
    pack.write_pack_toml("1.3.0", "1.21.1", &[("fabric", "0.16.9")]);
    pack.write(
        "CHANGELOG.md",
        "# Changelog\n\n## 1.3.0 — 2026-10-01\n".as_bytes(),
    );
    pack.write("config/ação.toml", "x = 1\n".as_bytes());

    let saved = repo
        .save_version(&SaveVersion {
            version: "1.3.0",
            tag_message: "## 1.3.0 — 2026-10-01\n\n### Configs alteradas\n- config/ação.toml\n",
            identity: &identity(),
            when: at(10),
            mark_final: false,
        })
        .unwrap();

    assert_eq!(saved.version, "1.3.0");
    assert_eq!(saved.date, "2026-10-01T12:10:00-03:00");
    assert!(!saved.is_final && !saved.is_published && saved.reachable);
    assert!(repo.unsaved_changes().unwrap().is_empty());
    assert!(git_status(&pack).is_empty(), "{:?}", git_status(&pack));

    let git = raw(&pack);
    let head = git.head().unwrap().peel_to_commit().unwrap();
    assert_eq!(head.id().to_string(), saved.commit);
    assert_eq!(head.message().unwrap(), "Versão 1.3.0");
    assert_eq!(head.parent_count(), 1);
    let tag = git
        .find_reference("refs/tags/v1.3.0")
        .unwrap()
        .peel_to_tag()
        .unwrap();
    assert_eq!(tag.target_id(), head.id());
    assert_eq!(
        tag.message().unwrap().unwrap(),
        "## 1.3.0 — 2026-10-01\n\n### Configs alteradas\n- config/ação.toml\n"
    );
    assert_eq!(tag.tagger().unwrap().name().unwrap(), "Autor de Teste");
    // A árvore salva tem o pack.toml com a versão nova.
    let blob = head
        .tree()
        .unwrap()
        .get_path(std::path::Path::new("pack.toml"))
        .unwrap()
        .to_object(&git)
        .unwrap()
        .peel_to_blob()
        .unwrap();
    assert!(String::from_utf8_lossy(blob.content()).contains("version = \"1.3.0\""));

    let versions = repo.versions().unwrap();
    assert_eq!(versions, vec![SavedVersion { ..saved }]);
    assert_eq!(repo.last_version().unwrap().unwrap().to_string(), "1.3.0");
}

#[test]
fn ca_t16_04_versao_menor_que_a_ultima_e_recusada_com_explicacao() {
    let (pack, repo) = TestPack::versioned();
    pack.write("config/a.toml", b"a = 1\n");
    save(&repo, "1.2.0", 1).unwrap();
    pack.write("config/a.toml", b"a = 2\n");

    let error = save(&repo, "1.0.0", 2).unwrap_err();
    assert!(matches!(error, Error::VersionNotGreater { .. }), "{error}");
    let params = error.params();
    assert_eq!(params["version"], "1.0.0");
    assert_eq!(params["last"], "1.2.0");

    let same = save(&repo, "1.2.0", 2).unwrap_err();
    assert!(matches!(same, Error::VersionExists { .. }), "{same}");
    // Pré-versão de 1.2.0 é menor que 1.2.0.
    assert!(matches!(
        save(&repo, "1.2.0-beta.1", 2).unwrap_err(),
        Error::VersionNotGreater { .. }
    ));
    // Nada foi gravado nas tentativas recusadas.
    assert_eq!(repo.unsaved_changes().unwrap().len(), 1);
    assert_eq!(repo.versions().unwrap().len(), 1);
    save(&repo, "1.2.1", 3).unwrap();
}

#[test]
fn numero_invalido_e_recusado() {
    let (pack, repo) = TestPack::versioned();
    pack.write("config/a.toml", b"a = 1\n");
    for text in ["", "1.0", "v1.0.0", "1.0.0.0", "01.0.0", "um"] {
        let error = save(&repo, text, 1).unwrap_err();
        assert!(
            matches!(error, Error::InvalidVersion { .. }),
            "{text:?}: {error}"
        );
        assert_eq!(error.params()["version"], text);
    }
    assert!(repo.validate_new_version("2.0.0-rc.1").is_ok());
}

#[test]
fn nada_mudou_desde_a_ultima_versao() {
    let (pack, repo) = TestPack::versioned();
    let error = save(&repo, "0.1.0", 1).unwrap_err();
    assert!(
        matches!(error, Error::NothingChanged { last: None }),
        "{error}"
    );
    pack.write("config/a.toml", b"a = 1\n");
    save(&repo, "0.1.0", 1).unwrap();
    let error = save(&repo, "0.2.0", 2).unwrap_err();
    assert!(
        matches!(&error, Error::NothingChanged { last: Some(last) } if last == "0.1.0"),
        "{error}"
    );
    assert_eq!(error.params()["last"], "0.1.0");
}

#[test]
fn salvar_inclui_arquivos_apagados_e_novos() {
    let (pack, repo) = TestPack::versioned();
    pack.write("config/velho.toml", b"v = 1\n");
    save(&repo, "0.1.0", 1).unwrap();
    pack.remove("config/velho.toml");
    pack.write("config/novo/opções.json", "{}\n".as_bytes());
    save(&repo, "0.2.0", 2).unwrap();
    let git = raw(&pack);
    let tree = git.head().unwrap().peel_to_tree().unwrap();
    assert!(
        tree.get_path(std::path::Path::new("config/velho.toml"))
            .is_err()
    );
    assert!(
        tree.get_path(std::path::Path::new("config/novo/opções.json"))
            .is_ok()
    );
    assert!(git_status(&pack).is_empty());
}

#[test]
fn ca_t16_05_versao_final_marca_desmarca_sem_tocar_nos_arquivos() {
    let (pack, repo) = TestPack::versioned();
    pack.write("config/a.toml", b"a = 1\n");
    let saved = repo
        .save_version(&SaveVersion {
            version: "1.0.0",
            tag_message: "final",
            identity: &identity(),
            when: at(1),
            mark_final: true,
        })
        .unwrap();
    assert!(saved.is_final);
    let git = raw(&pack);
    let final_ref = git
        .find_reference(&format!("{FINAL_REF_PREFIX}v1.0.0"))
        .unwrap();
    assert_eq!(final_ref.target().unwrap().to_string(), saved.commit);
    let before = pack.hashes();
    let head_before = repo.head_id().unwrap();

    repo.set_final("1.0.0", false).unwrap();
    assert!(!repo.versions().unwrap()[0].is_final);
    // Desmarcar de novo não é erro.
    repo.set_final("1.0.0", false).unwrap();
    repo.set_final("1.0.0", true).unwrap();
    assert!(repo.versions().unwrap()[0].is_final);

    assert_eq!(pack.hashes(), before);
    assert_eq!(repo.head_id().unwrap(), head_before);
    assert!(repo.unsaved_changes().unwrap().is_empty());
    // Nenhum remoto foi configurado: nada vai para o GitHub ao salvar ou marcar.
    assert_eq!(git.remotes().unwrap().len(), 0);
}

#[test]
fn versao_publicada_nao_pode_ser_desmarcada() {
    let (pack, repo) = TestPack::versioned();
    pack.write("config/a.toml", b"a = 1\n");
    let saved = save(&repo, "1.0.0", 1).unwrap();
    repo.set_final("1.0.0", true).unwrap();
    let git = raw(&pack);
    git.reference(
        &format!("{PUBLISH_TAG_PREFIX}v1.0.0"),
        git2::Oid::from_str(&saved.commit).unwrap(),
        false,
        "teste: publicada",
    )
    .unwrap();
    assert!(repo.versions().unwrap()[0].is_published);
    let error = repo.set_final("1.0.0", false).unwrap_err();
    assert!(matches!(error, Error::VersionPublished { .. }), "{error}");
    assert!(repo.versions().unwrap()[0].is_final);
}

#[test]
fn marcar_versao_inexistente() {
    let (_pack, repo) = TestPack::versioned();
    assert!(matches!(
        repo.set_final("9.9.9", true).unwrap_err(),
        Error::VersionNotFound { .. }
    ));
    assert!(matches!(
        repo.set_final("x", true).unwrap_err(),
        Error::InvalidVersion { .. }
    ));
}

#[test]
fn falha_ao_gravar_a_tag_devolve_a_branch() {
    let (pack, repo) = TestPack::versioned();
    let initial = repo.head_id().unwrap();
    pack.write("config/a.toml", b"a = 1\n");
    // Uma referência `v1.0.0/x` impede a referência `v1.0.0` (nome em conflito): a gravação
    // da tag falha depois do commit.
    let git = raw(&pack);
    let head = git.head().unwrap().target().unwrap();
    git.reference("refs/tags/v1.0.0/x", head, false, "teste: bloqueio")
        .unwrap();
    let error = save(&repo, "1.0.0", 1).unwrap_err();
    assert!(
        matches!(error, Error::Git { .. } | Error::VersionExists { .. }),
        "{error}"
    );
    assert_eq!(repo.head_id().unwrap(), initial);
    assert_eq!(repo.unsaved_changes().unwrap().len(), 1);
}

#[test]
fn versoes_em_ordem_da_maior_para_a_menor() {
    let (pack, repo) = TestPack::versioned();
    for (i, version) in ["0.1.0", "0.2.0", "0.10.0", "1.0.0-beta.1", "1.0.0"]
        .into_iter()
        .enumerate()
    {
        pack.write("config/a.toml", format!("a = {i}\n").as_bytes());
        save(&repo, version, i64::try_from(i).unwrap() + 1).unwrap();
    }
    let versions: Vec<String> = repo
        .versions()
        .unwrap()
        .into_iter()
        .map(|saved| saved.version)
        .collect();
    assert_eq!(
        versions,
        ["1.0.0", "1.0.0-beta.1", "0.10.0", "0.2.0", "0.1.0"]
    );
}

#[test]
fn arvore_de_uma_versao_salva_para_a_exportacao() {
    use warden_versioning::Snapshot;
    let (pack, repo) = TestPack::versioned();
    pack.write("config/ação.toml", b"a = 1\r\n");
    save(&repo, "1.0.0", 1).unwrap();
    pack.write("config/ação.toml", b"a = 2\n");
    pack.write("config/novo.toml", b"novo\n");
    pack.write("logs/latest.log", b"ignorado");

    let v1 = Snapshot::Version("1.0.0".into());
    let files = repo.files_at(&v1).unwrap();
    assert_eq!(
        files,
        [
            ".gitattributes",
            ".gitignore",
            "config/ação.toml",
            "index.toml",
            "pack.toml"
        ]
    );
    assert_eq!(
        repo.read_file_at(&v1, "config/ação.toml").unwrap().unwrap(),
        b"a = 1\r\n"
    );
    assert_eq!(repo.read_file_at(&v1, "config/novo.toml").unwrap(), None);
    assert_eq!(repo.read_file_at(&v1, "config").unwrap(), None);
    assert!(
        repo.read_file_at(&Snapshot::WorkingTree, "pack.toml")
            .is_err()
    );
    assert_eq!(
        repo.read_file_at(&Snapshot::Empty, "pack.toml").unwrap(),
        None
    );
    assert!(repo.files_at(&Snapshot::Empty).unwrap().is_empty());
    let now = repo.files_at(&Snapshot::WorkingTree).unwrap();
    assert!(now.contains(&"config/novo.toml".to_owned()));
    assert!(!now.contains(&"logs/latest.log".to_owned()));
    assert!(matches!(
        repo.files_at(&Snapshot::Version("3.0.0".into()))
            .unwrap_err(),
        Error::VersionNotFound { .. }
    ));
}
