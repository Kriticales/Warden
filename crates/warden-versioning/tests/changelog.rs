//! Changelog estruturado, sugestão de versão e `CHANGELOG.md` (SPEC T16; CA-T16-01 e
//! CA-T16-02 no nível da crate).
//!
//! Os metafiles são escritos pelo codificador da `warden-packwiz`, provado byte a byte contra
//! o packwiz real (P1-01). Os nomes legíveis vêm de um resolver de teste que imita o cache do
//! Modrinth e a API da CurseForge.

#![allow(linker_messages)]
// libgit2 no MSVC exporta símbolos; veja o relatório da V-01
// Auxiliares fora de `#[test]`: falhar com pânico reprova o teste que chamou.
#![allow(clippy::unwrap_used, clippy::panic)]

mod common;

use std::cell::RefCell;
use std::collections::HashMap;

use common::{TestPack, at, identity};
use warden_packwiz::Side;
use warden_versioning::changelog::{WORLD_REMOVAL_WARNING, prepend_entry, render_entry};
use warden_versioning::{
    Bump, ChangeFacts, ChangeSet, FileNames, ItemCategory, ItemChangeKind, ItemSource, PackRepo,
    SaveVersion, Snapshot, SuggestReason, VersionNameResolver, VersionRef, suggest_version,
};

/// Resolver de teste: nomes conhecidos e registro das consultas (em lote).
#[derive(Default)]
struct Names {
    known: HashMap<VersionRef, String>,
    calls: RefCell<Vec<usize>>,
}

impl Names {
    fn modrinth(mut self, project: &str, version: &str, name: &str) -> Self {
        self.known.insert(
            VersionRef::Modrinth {
                project_id: project.into(),
                version_id: version.into(),
            },
            name.into(),
        );
        self
    }

    fn curseforge(mut self, project: u32, file: u32, name: &str) -> Self {
        self.known.insert(
            VersionRef::CurseForge {
                project_id: project,
                file_id: file,
            },
            name.into(),
        );
        self
    }
}

impl VersionNameResolver for Names {
    fn version_names(&self, versions: &[VersionRef]) -> Vec<Option<String>> {
        self.calls.borrow_mut().push(versions.len());
        versions
            .iter()
            .map(|version| self.known.get(version).cloned())
            .collect()
    }
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

/// Pack com a versão 1.0.0: Sodium (só cliente), Lithium (Modrinth), JEI (CurseForge, cliente e
/// servidor), um resource pack e um config.
fn base_pack() -> (TestPack, PackRepo) {
    let (pack, repo) = TestPack::versioned();
    pack.add_modrinth("sodium", "Sodium", "AANobbMI", "s1", Side::Client);
    pack.add_modrinth("lithium", "Lithium", "gvQqBUqZ", "l1", Side::Both);
    pack.add_curseforge("jei", "Just Enough Items", 238_222, 4_000, Side::Both);
    pack.add_modrinth_in(
        "resourcepacks",
        "faithful",
        "Faithful 32x",
        "faith",
        "f1",
        Side::Client,
    );
    pack.write("config/sodium-options.json", b"{\"a\": 1}\n");
    save(&repo, "1.0.0", 1);
    (pack, repo)
}

fn names() -> Names {
    Names::default()
        .modrinth("AANobbMI", "s1", "0.5.11")
        .modrinth("AANobbMI", "s2", "0.6.0")
        .modrinth("gvQqBUqZ", "l1", "0.14.1")
        .modrinth("gvQqBUqZ", "l2", "0.14.3")
        .modrinth("P7dR8mSH", "fa1", "0.116.0")
        .modrinth("mOgUt4GM", "mm1", "11.0.1")
        .curseforge(238_222, 4_000, "19.21.0.247")
}

#[test]
fn ca_t16_01_dois_adicionados_e_um_atualizado_sugere_menor_com_versoes_legiveis() {
    let (pack, repo) = base_pack();
    pack.add_modrinth("fabric-api", "Fabric API", "P7dR8mSH", "fa1", Side::Both);
    pack.add_modrinth("modmenu", "Mod Menu", "mOgUt4GM", "mm1", Side::Client);
    pack.add_modrinth("lithium", "Lithium", "gvQqBUqZ", "l2", Side::Both);
    pack.write_pack_toml("1.1.0", "1.21.1", &[("fabric", "0.16.9")]);
    let resolver = names();

    let changes = repo.changes_since_last_version(&resolver).unwrap();

    let added: Vec<(&str, &str)> = changes
        .items_of(ItemChangeKind::Added)
        .map(|item| {
            (
                item.name.as_str(),
                item.new.as_ref().unwrap().label.as_str(),
            )
        })
        .collect();
    assert_eq!(added, [("Fabric API", "0.116.0"), ("Mod Menu", "11.0.1")]);
    let updated: Vec<(&str, &str, &str)> = changes
        .items_of(ItemChangeKind::Updated)
        .map(|item| {
            (
                item.name.as_str(),
                item.old.as_ref().unwrap().label.as_str(),
                item.new.as_ref().unwrap().label.as_str(),
            )
        })
        .collect();
    assert_eq!(updated, [("Lithium", "0.14.1", "0.14.3")]);
    assert_eq!(changes.items.len(), 3, "{:#?}", changes.items);
    assert!(changes.configs.is_empty(), "{:?}", changes.configs);
    assert_eq!(changes.minecraft, None);
    // Uma consulta em lote ao resolver.
    assert_eq!(*resolver.calls.borrow(), [4]);

    let facts = ChangeFacts::from_changes(&changes, |_| false);
    let highest = repo.highest_version().unwrap();
    let suggestion = suggest_version(highest.as_ref(), "1.1.0", &facts).unwrap();
    assert_eq!(suggestion.version, "1.1.0");
    assert_eq!(suggestion.bump, Some(Bump::Minor));
    assert_eq!(
        suggestion.reasons,
        [
            SuggestReason::AddedItems { count: 2 },
            SuggestReason::UpdatedItems { count: 1 }
        ]
    );

    let entry = render_entry("1.1.0", "2026-10-01", "", &changes);
    assert_eq!(
        entry,
        "## 1.1.0 — 2026-10-01\n\n\
         ### Mods adicionados\n\
         - Fabric API 0.116.0 (Modrinth)\n\
         - Mod Menu 11.0.1 (Modrinth)\n\n\
         ### Mods atualizados\n\
         - Lithium 0.14.1 → 0.14.3\n"
    );
}

#[test]
fn ca_t16_02_remover_mod_cliente_e_servidor_sugere_maior_com_atencao() {
    let (pack, repo) = base_pack();
    pack.remove("mods/jei.pw.toml");
    let changes = repo.changes_since_last_version(&names()).unwrap();
    let removed: Vec<_> = changes.items_of(ItemChangeKind::Removed).collect();
    assert_eq!(removed.len(), 1);
    assert_eq!(removed[0].name, "Just Enough Items");
    assert_eq!(removed[0].source, ItemSource::CurseForge);
    assert_eq!(removed[0].side, "both");
    assert_eq!(changes.removed_world_mods().count(), 1);

    let facts = ChangeFacts::from_changes(&changes, |_| false);
    let suggestion = suggest_version(Some(&semver("1.0.0")), "1.0.0", &facts).unwrap();
    assert_eq!(suggestion.version, "2.0.0");
    assert_eq!(suggestion.bump, Some(Bump::Major));
    assert_eq!(
        suggestion.reasons,
        [SuggestReason::RemovedWorldMods { count: 1 }]
    );

    let entry = render_entry("2.0.0", "2026-10-01", "Tiramos o JEI.", &changes);
    assert_eq!(
        entry,
        format!(
            "## 2.0.0 — 2026-10-01\n\nTiramos o JEI.\n\n### Atenção\n- {WORLD_REMOVAL_WARNING}\n\n\
             ### Mods removidos\n- Just Enough Items 19.21.0.247 (CurseForge)\n"
        )
    );
}

#[test]
fn remover_mod_so_cliente_nao_pede_atencao_nem_maior() {
    let (pack, repo) = base_pack();
    pack.remove("mods/sodium.pw.toml");
    let changes = repo.changes_since_last_version(&names()).unwrap();
    assert_eq!(changes.removed_world_mods().count(), 0);
    let facts = ChangeFacts::from_changes(&changes, |_| false);
    let suggestion = suggest_version(Some(&semver("1.0.0")), "", &facts).unwrap();
    assert_eq!(suggestion.bump, Some(Bump::Patch));
    assert!(!render_entry("1.0.1", "d", "", &changes).contains("Atenção"));
}

#[test]
fn mod_de_geracao_de_mundo_e_minecraft_sugerem_maior() {
    let (pack, repo) = base_pack();
    pack.add_modrinth("terralith", "Terralith", "8oi3bsk5", "t1", Side::Both);
    let changes = repo.changes_since_last_version(&FileNames).unwrap();
    let facts = ChangeFacts::from_changes(&changes, |item| {
        item.project_id.as_deref() == Some("8oi3bsk5")
    });
    assert_eq!(facts.worldgen_changed, 1);
    let suggestion = suggest_version(Some(&semver("1.0.0")), "", &facts).unwrap();
    assert_eq!(suggestion.bump, Some(Bump::Major));

    pack.write_pack_toml("1.0.0", "1.21.4", &[("neoforge", "21.4.1")]);
    let changes = repo.changes_since_last_version(&FileNames).unwrap();
    let minecraft = changes.minecraft.as_ref().unwrap();
    assert_eq!(minecraft.old.as_deref(), Some("1.21.1"));
    assert_eq!(minecraft.new.as_deref(), Some("1.21.4"));
    let loaders: Vec<_> = changes.loaders.iter().map(|l| l.loader.as_str()).collect();
    assert_eq!(loaders, ["fabric", "neoforge"]);
    let entry = render_entry("2.0.0", "d", "", &changes);
    assert!(entry.contains(
        "### Minecraft e loader\n- Minecraft 1.21.1 → 1.21.4\n- Fabric 0.16.9 removido\n- NeoForge 21.4.1 adicionado\n"
    ), "{entry}");
}

#[test]
fn resource_packs_shaders_locais_configs_e_ajustes() {
    let (pack, repo) = base_pack();
    // Resource pack atualizado (sem nome conhecido: nome do arquivo), shader local novo.
    pack.add_modrinth_in(
        "resourcepacks",
        "faithful",
        "Faithful 32x",
        "faith",
        "f2",
        Side::Client,
    );
    pack.write("shaderpacks/Complementary Reimagined r5.zip", b"zip");
    // Jar local novo e config com acento.
    pack.write("mods/meu-mod-1.0.jar", b"jar");
    pack.write("config/ação.toml", "x = 1\n".as_bytes());
    pack.write("config/sodium-options.json", b"{\"a\": 2}\n");
    // Ajuste sem trocar de versão: Sodium passa a ser "cliente e servidor".
    pack.add_modrinth("sodium", "Sodium", "AANobbMI", "s1", Side::Both);
    // Arquivos de controle não aparecem como configs.
    pack.write("CHANGELOG.md", b"# Changelog\n");
    pack.write("index.toml", b"hash-format = \"sha256\"\n# mudou\n");
    pack.write(".warden/notas.toml", b"x = 1\n");

    let changes = repo.changes_since_last_version(&names()).unwrap();
    assert_eq!(
        changes.configs,
        ["config/ação.toml", "config/sodium-options.json"]
    );
    let summary: Vec<(ItemCategory, ItemChangeKind, &str, ItemSource)> = changes
        .items
        .iter()
        .map(|item| (item.category, item.kind, item.name.as_str(), item.source))
        .collect();
    assert_eq!(
        summary,
        [
            (
                ItemCategory::Mod,
                ItemChangeKind::Added,
                "meu-mod-1.0.jar",
                ItemSource::Local
            ),
            (
                ItemCategory::Mod,
                ItemChangeKind::Adjusted,
                "Sodium",
                ItemSource::Modrinth
            ),
            (
                ItemCategory::ResourcePack,
                ItemChangeKind::Updated,
                "Faithful 32x",
                ItemSource::Modrinth
            ),
            (
                ItemCategory::Shader,
                ItemChangeKind::Added,
                "Complementary Reimagined r5.zip",
                ItemSource::Local
            ),
        ]
    );
    insta::assert_snapshot!(
        "changelog_completo",
        render_entry(
            "1.1.0",
            "2026-10-01",
            "Notas *livres* do usuário.",
            &changes
        )
    );
}

#[test]
fn metafile_renomeado_com_versao_nova_e_atualizacao_e_sem_versao_nova_nao_aparece() {
    let (pack, repo) = base_pack();
    pack.remove("mods/lithium.pw.toml");
    pack.add_modrinth("lithium-renomeado", "Lithium", "gvQqBUqZ", "l2", Side::Both);
    // Mesmo conteúdo em outro caminho: não é mudança de item.
    let sodium = pack.read("mods/sodium.pw.toml");
    pack.remove("mods/sodium.pw.toml");
    pack.write("mods/sodium-novo-nome.pw.toml", &sodium);
    // Mesmo projeto e mesma versão com outro nome de arquivo: ajuste do item.
    pack.remove("mods/jei.pw.toml");
    pack.add_curseforge(
        "jei-renomeado",
        "Just Enough Items",
        238_222,
        4_000,
        Side::Both,
    );
    let changes = repo.changes_since_last_version(&names()).unwrap();
    let kinds: Vec<(&str, ItemChangeKind)> = changes
        .items
        .iter()
        .map(|item| (item.name.as_str(), item.kind))
        .collect();
    assert_eq!(
        kinds,
        [
            ("Lithium", ItemChangeKind::Updated),
            ("Just Enough Items", ItemChangeKind::Adjusted)
        ]
    );
    assert_eq!(changes.items[0].path, "mods/lithium-renomeado.pw.toml");
}

#[test]
fn primeira_versao_lista_tudo_como_adicionado_e_sugere_o_pack_toml() {
    let (pack, repo) = TestPack::versioned();
    pack.add_modrinth("sodium", "Sodium", "AANobbMI", "s1", Side::Client);
    let changes = repo.changes_since_last_version(&names()).unwrap();
    assert_eq!(changes.items.len(), 1);
    assert_eq!(changes.items[0].kind, ItemChangeKind::Added);
    // Sem `pack.toml` do lado antigo, Minecraft e loader não "mudam".
    assert_eq!(changes.minecraft, None);
    assert!(changes.loaders.is_empty());
    let facts = ChangeFacts::from_changes(&changes, |_| false);
    let suggestion = suggest_version(None, "0.1.0", &facts).unwrap();
    assert_eq!(suggestion.version, "0.1.0");
}

#[test]
fn diferencas_entre_versoes_e_pontos_de_seguranca() {
    let (pack, repo) = base_pack();
    pack.add_modrinth("lithium", "Lithium", "gvQqBUqZ", "l2", Side::Both);
    save(&repo, "1.1.0", 2);
    let between = repo
        .changes(
            &Snapshot::Version("1.0.0".into()),
            &Snapshot::Version("1.1.0".into()),
            &names(),
        )
        .unwrap();
    assert_eq!(between.items.len(), 1);
    assert_eq!(between.items[0].kind, ItemChangeKind::Updated);
    assert_eq!(between.files.len(), 1);
    // Ao contrário, a mesma atualização vista de trás para frente.
    let back = repo
        .changes(
            &Snapshot::Version("1.1.0".into()),
            &Snapshot::Version("1.0.0".into()),
            &names(),
        )
        .unwrap();
    assert_eq!(back.items[0].new.as_ref().unwrap().label, "0.14.1");
    // Versão contra o estado atual ("Ver diferenças para o estado atual").
    pack.write("config/novo.toml", b"x = 1\n");
    let current = repo
        .changes(
            &Snapshot::Version("1.0.0".into()),
            &Snapshot::WorkingTree,
            &names(),
        )
        .unwrap();
    assert_eq!(current.configs, ["config/novo.toml"]);
    // Ponto de segurança como lado.
    let point = repo
        .create_safety_point(
            &warden_versioning::SafetyReason::UpdateAll,
            &identity(),
            at(3),
        )
        .unwrap();
    let from_point = repo
        .changes(
            &Snapshot::SafetyPoint(point.name),
            &Snapshot::WorkingTree,
            &names(),
        )
        .unwrap();
    assert!(from_point.is_empty(), "{:?}", from_point.files);
    assert!(
        repo.changes(
            &Snapshot::Version("9.0.0".into()),
            &Snapshot::WorkingTree,
            &names()
        )
        .is_err()
    );
    assert!(
        repo.changes(&Snapshot::Empty, &Snapshot::Empty, &names())
            .unwrap()
            .is_empty()
    );
}

#[test]
fn metafile_quebrado_nao_derruba_o_changelog() {
    let (pack, repo) = base_pack();
    pack.write("mods/quebrado.pw.toml", b"name = [\n");
    let changes = repo.changes_since_last_version(&names()).unwrap();
    assert_eq!(changes.items.len(), 1);
    assert_eq!(changes.items[0].source, ItemSource::Unknown);
    assert_eq!(changes.items[0].name, "quebrado");
    let entry = render_entry("1.0.1", "d", "", &changes);
    assert!(
        entry.contains("- quebrado (origem desconhecida)"),
        "{entry}"
    );
}

#[test]
fn changelog_md_acumula_versoes_no_topo() {
    let mut file = None::<String>;
    for (version, line) in [
        ("1.0.0", "primeira"),
        ("1.1.0", "segunda"),
        ("2.0.0", "terceira"),
    ] {
        let entry = format!("## {version} — 2026-10-01\n\n{line}\n");
        file = Some(prepend_entry(file.as_deref(), &entry));
    }
    insta::assert_snapshot!("changelog_acumulado", file.unwrap());
    assert_eq!(
        render_entry("1.0.0", "2026-10-01", "", &ChangeSet::default()),
        "## 1.0.0 — 2026-10-01\n"
    );
}

fn semver(text: &str) -> semver::Version {
    semver::Version::parse(text).unwrap()
}
