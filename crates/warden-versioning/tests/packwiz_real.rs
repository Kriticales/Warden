//! Changelog e restauração com packs gerados pelo packwiz real (fixtures da P1-01 em
//! `crates/warden-packwiz/tests/fixtures/packwiz-output/`, só leitura; origem em
//! `FIXTURES.md` daquela pasta).
//!
//! O pack começa como o `fabric-1.21.1` e vira o `neoforge-1.21.1`: troca de loader, Sodium
//! do mesmo projeto em outra versão, mods, resource pack, shader e configs removidos, mod
//! novo.

#![allow(linker_messages)]
// libgit2 no MSVC exporta símbolos; veja o relatório da V-01
// Auxiliares fora de `#[test]`: falhar com pânico reprova o teste que chamou.
#![allow(clippy::unwrap_used, clippy::panic)]

mod common;

use std::fs;
use std::path::{Path, PathBuf};

use common::{TestPack, at, identity};
use warden_versioning::changelog::render_entry;
use warden_versioning::{
    Bump, ChangeFacts, FileNames, ItemCategory, ItemChangeKind, PackRepo, RestoreTarget,
    SaveVersion, suggest_version,
};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../warden-packwiz/tests/fixtures/packwiz-output")
        .join(name)
}

fn copy_dir(from: &Path, to: &Path) {
    for entry in fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            fs::create_dir_all(&target).unwrap();
            copy_dir(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), target).unwrap();
        }
    }
}

/// Troca o conteúdo do pack (fora `.git` e os arquivos de controle do Warden) pelo da fixture.
fn replace_with(pack: &TestPack, name: &str) {
    for entry in fs::read_dir(&pack.root).unwrap() {
        let entry = entry.unwrap();
        let file_name = entry.file_name();
        let keep = [".git", ".gitignore", ".gitattributes"];
        if keep.iter().any(|kept| file_name == *kept) {
            continue;
        }
        if entry.file_type().unwrap().is_dir() {
            fs::remove_dir_all(entry.path()).unwrap();
        } else {
            fs::remove_file(entry.path()).unwrap();
        }
    }
    copy_dir(&fixture(name), &pack.root);
}

fn save(repo: &PackRepo, version: &str, minutes: i64) {
    repo.save_version(&SaveVersion {
        version,
        tag_message: version,
        identity: &identity(),
        when: at(minutes),
        mark_final: false,
    })
    .unwrap();
}

#[test]
fn fabric_para_neoforge_com_metafiles_do_packwiz_real() {
    assert!(
        fixture("fabric-1.21.1").join("pack.toml").is_file(),
        "fixtures da warden-packwiz não encontradas"
    );
    let (pack, repo) = TestPack::versioned();
    replace_with(&pack, "fabric-1.21.1");
    save(&repo, "1.0.0", 1);
    let fabric = pack.hashes();
    replace_with(&pack, "neoforge-1.21.1");

    let changes = repo.changes_since_last_version(&FileNames).unwrap();

    assert_eq!(changes.minecraft, None);
    let loaders: Vec<_> = changes
        .loaders
        .iter()
        .map(|change| {
            (
                change.loader.as_str(),
                change.change.old.as_deref(),
                change.change.new.as_deref(),
            )
        })
        .collect();
    assert_eq!(
        loaders,
        [
            ("fabric", Some("0.16.14"), None),
            ("neoforge", None, Some("21.1.209"))
        ]
    );
    let sodium = changes
        .items
        .iter()
        .find(|item| item.name == "Sodium")
        .unwrap();
    assert_eq!(sodium.kind, ItemChangeKind::Updated);
    assert_eq!(
        sodium.old.as_ref().unwrap().label,
        "sodium-fabric-0.8.13+mc1.21.1.jar"
    );
    assert_eq!(
        sodium.new.as_ref().unwrap().label,
        "sodium-neoforge-0.8.13+mc1.21.1.jar"
    );
    assert!(changes.items.iter().any(|item| {
        item.category == ItemCategory::ResourcePack && item.kind == ItemChangeKind::Removed
    }));
    assert!(changes.items.iter().any(|item| {
        item.category == ItemCategory::Shader && item.kind == ItemChangeKind::Removed
    }));

    let facts = ChangeFacts::from_changes(&changes, |_| false);
    assert!(facts.loader_changed);
    let suggestion = suggest_version(Some(&semver::Version::new(1, 0, 0)), "", &facts).unwrap();
    assert_eq!(suggestion.bump, Some(Bump::Major));
    insta::assert_snapshot!(
        "fabric_para_neoforge",
        render_entry(&suggestion.version, "2026-10-01", "", &changes)
    );

    // Salvar a versão NeoForge e voltar para a Fabric devolve os bytes exatos do packwiz.
    save(&repo, "2.0.0", 2);
    let neoforge = pack.hashes();
    repo.restore(&RestoreTarget::Version("1.0.0".into()), &identity(), at(3))
        .unwrap();
    assert_eq!(pack.hashes(), fabric);
    repo.restore(&RestoreTarget::Version("2.0.0".into()), &identity(), at(4))
        .unwrap();
    assert_eq!(pack.hashes(), neoforge);
}

#[test]
fn todos_os_packs_das_fixtures_entram_e_saem_do_historico_iguais() {
    let names = [
        "fabric-1.21.1",
        "forge-1.12.2",
        "forge-1.20.1",
        "forge-1.7.10",
        "indice-sha512",
        "neoforge-1.20.1",
        "neoforge-1.21.1",
        "opcoes",
        "quilt-1.20.1",
        "sem-hashes",
    ];
    let (pack, repo) = TestPack::versioned();
    let mut saved = Vec::new();
    for (i, name) in names.iter().enumerate() {
        replace_with(&pack, name);
        let version = format!("{}.0.0", i + 1);
        save(&repo, &version, i64::try_from(i).unwrap() + 1);
        assert!(repo.unsaved_changes().unwrap().is_empty(), "{name}");
        // Cada pack lido de volta como changelog contra o vazio não falha.
        repo.changes(
            &warden_versioning::Snapshot::Empty,
            &warden_versioning::Snapshot::Version(version.clone()),
            &FileNames,
        )
        .unwrap();
        saved.push((version, pack.hashes()));
    }
    for (minutes, (version, hashes)) in (100..).zip(saved.iter().rev()) {
        repo.restore(
            &RestoreTarget::Version(version.clone()),
            &identity(),
            at(minutes),
        )
        .unwrap();
        assert_eq!(&pack.hashes(), hashes, "{version}");
    }
}
