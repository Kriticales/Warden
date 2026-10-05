//! "Voltar para esta versão" e pontos de segurança (SPEC T17; CA-T17-01 a CA-T17-03).
//!
//! A falha no meio é provada de dois jeitos: com os pontos de falha da `warden-core`
//! (feature `fault-injection`), em cada passo da preparação e da aplicação, e no Windows com
//! um arquivo aberto de verdade por outro "programa" sem compartilhamento.

#![allow(linker_messages)]
// libgit2 no MSVC exporta símbolos; veja o relatório da V-01
// Auxiliares fora de `#[test]`: falhar com pânico reprova o teste que chamou.
#![allow(clippy::unwrap_used, clippy::panic)]

mod common;

use std::collections::BTreeMap;
use std::fs;

use common::{TestPack, at, git_status, identity, raw};
use warden_core::{DomainError as _, fault};
use warden_packwiz::Side;
use warden_versioning::{
    Error, PackRepo, RestoreTarget, SAFETY_REF_PREFIX, SafetyReason, SaveVersion,
};

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

fn restore(
    repo: &PackRepo,
    version: &str,
    minutes: i64,
) -> Result<warden_versioning::RestoreReport, Error> {
    repo.restore(
        &RestoreTarget::Version(version.to_owned()),
        &identity(),
        at(minutes),
    )
}

/// Versão 1.0.0 com mods, configs com acento e espaço, CRLF e um arquivo binário; depois um
/// estado bem diferente (itens trocados, arquivos novos em pastas novas, apagados).
fn pack_with_two_states() -> (TestPack, PackRepo, BTreeMap<String, String>) {
    let (pack, repo) = TestPack::versioned();
    pack.add_modrinth("sodium", "Sodium", "AANobbMI", "v1", Side::Client);
    pack.add_curseforge("jei", "Just Enough Items", 238_222, 1000, Side::Both);
    pack.write(
        "config/ação.toml",
        "opção = \"sim\"\r\nlinha = 2\r\n".as_bytes(),
    );
    pack.write("config/meu mod/opções da tela.json", b"{\"a\": 1}\n");
    pack.write("mods/local mod.jar", &[0u8, 159, 146, 150, 255, 0, 13, 10]);
    pack.write("kubejs/server_scripts/receitas.js", b"// receitas\n");
    save(&repo, "1.0.0", 1);
    let version_1 = pack.hashes();

    pack.add_modrinth("sodium", "Sodium", "AANobbMI", "v2", Side::Client);
    pack.remove("mods/jei.pw.toml");
    pack.add_modrinth("lithium", "Lithium", "gvQqBUqZ", "l1", Side::Both);
    pack.write("config/ação.toml", "opção = \"não\"\r\n".as_bytes());
    pack.remove("config/meu mod/opções da tela.json");
    pack.write("config/novo/fundo/arquivo novo.txt", b"novo\n");
    pack.write("kubejs/server_scripts/receitas.js", b"// receitas 2\n");
    save(&repo, "1.1.0", 2);
    // E alterações não salvas por cima.
    pack.write("config/rascunho.toml", b"rascunho = true\n");
    pack.write("mods/local mod.jar", b"outro jar");
    (pack, repo, version_1)
}

/// Arquivos ignorados que a restauração nunca toca.
fn write_ignored(pack: &TestPack) {
    pack.write("logs/latest.log", b"log do jogo");
    pack.write("saves/Mundo/level.dat", b"mundo");
    pack.write("options.txt.bak", b"ignorado por *.bak");
}

fn without_ignored(mut hashes: BTreeMap<String, String>) -> BTreeMap<String, String> {
    hashes.retain(|path, _| {
        !path.starts_with("logs/") && !path.starts_with("saves/") && path != "options.txt.bak"
    });
    hashes
}

#[test]
fn ca_t17_01_voltar_restaura_a_arvore_exata_e_remove_o_que_veio_depois() {
    let (pack, repo, version_1) = pack_with_two_states();
    write_ignored(&pack);
    let ignored_before: BTreeMap<String, String> = pack
        .hashes()
        .into_iter()
        .filter(|(path, _)| {
            without_ignored(BTreeMap::from([(path.clone(), String::new())])).is_empty()
        })
        .collect();
    let head_before = repo.head_id().unwrap();
    let branch_before = repo.branch_name().unwrap();

    let report = restore(&repo, "1.0.0", 3).unwrap();

    let after = pack.hashes();
    assert_eq!(
        without_ignored(after.clone()),
        version_1,
        "árvore diferente da versão 1.0.0"
    );
    for (path, hash) in &ignored_before {
        assert_eq!(
            after.get(path),
            Some(hash),
            "arquivo ignorado mudou: {path}"
        );
    }
    // Bytes exatos, inclusive CRLF e binário.
    assert_eq!(
        pack.read("config/ação.toml"),
        "opção = \"sim\"\r\nlinha = 2\r\n".as_bytes()
    );
    assert_eq!(
        pack.read("mods/local mod.jar"),
        [0u8, 159, 146, 150, 255, 0, 13, 10]
    );
    // Pastas que ficaram vazias somem; as que a versão tinha voltam.
    assert!(!pack.exists("config/novo"));
    assert!(pack.exists("config/meu mod/opções da tela.json"));
    assert!(
        report
            .deleted
            .contains(&"config/novo/fundo/arquivo novo.txt".to_owned())
    );
    assert!(report.deleted.contains(&"config/rascunho.toml".to_owned()));
    assert!(report.deleted.contains(&"mods/lithium.pw.toml".to_owned()));
    assert!(report.written.contains(&"mods/jei.pw.toml".to_owned()));
    assert!(report.leftover_temp_files.is_empty());
    // Nenhum temporário sobrou.
    assert!(
        after.keys().all(|path| !path.ends_with(".warden-tmp")),
        "{after:?}"
    );
    // Histórico intacto: branch e HEAD não se movem.
    assert_eq!(repo.head_id().unwrap(), head_before);
    assert_eq!(repo.branch_name().unwrap(), branch_before);
    assert_eq!(repo.versions().unwrap().len(), 2);
    // E agora "Salvar versão" cria uma versão nova a partir dali.
    assert!(!repo.unsaved_changes().unwrap().is_empty());
    save(&repo, "1.2.0", 4);
    assert!(git_status(&pack).is_empty());
    let git = raw(&pack);
    let tree_v1 = git
        .find_reference("refs/tags/v1.0.0")
        .unwrap()
        .peel_to_tree()
        .unwrap()
        .id();
    assert_eq!(git.head().unwrap().peel_to_tree().unwrap().id(), tree_v1);
}

#[test]
fn ca_t17_03_estado_anterior_volta_pelo_ponto_de_seguranca() {
    let (pack, repo, version_1) = pack_with_two_states();
    let before = pack.hashes();

    let report = restore(&repo, "1.0.0", 3).unwrap();
    let point = &report.safety_point;
    assert_eq!(point.reason, "antes de voltar para 1.0.0");
    assert_eq!(point.created_at, "2026-10-01T12:03:00-03:00");
    assert_eq!(point.name, "20261001T150300Z-voltar-1.0.0");
    let git = raw(&pack);
    let reference = git
        .find_reference(&format!("{SAFETY_REF_PREFIX}{}", point.name))
        .unwrap();
    assert_eq!(reference.target().unwrap().to_string(), point.commit);
    // O ponto de segurança guarda até as alterações não salvas.
    let commit = reference.peel_to_commit().unwrap();
    assert_eq!(
        commit.message().unwrap(),
        "Ponto de segurança: antes de voltar para 1.0.0"
    );
    assert_eq!(
        commit.parent_id(0).unwrap().to_string(),
        repo.head_id().unwrap().unwrap()
    );
    assert!(
        commit
            .tree()
            .unwrap()
            .get_path(std::path::Path::new("config/rascunho.toml"))
            .is_ok()
    );
    assert_eq!(repo.safety_points().unwrap(), vec![point.clone()]);

    let recovered = repo
        .restore(
            &RestoreTarget::SafetyPoint(point.name.clone()),
            &identity(),
            at(4),
        )
        .unwrap();
    assert_eq!(pack.hashes(), before);
    // Recuperar também guardou o estado de antes (a versão 1.0.0) num ponto novo.
    assert!(
        recovered
            .safety_point
            .reason
            .starts_with("antes de recuperar o ponto de segurança de 2026-10-01T12:03:00")
    );
    let points = repo.safety_points().unwrap();
    assert_eq!(points.len(), 2);
    assert_eq!(points[0], recovered.safety_point);
    let back = repo
        .restore(
            &RestoreTarget::SafetyPoint(points[0].name.clone()),
            &identity(),
            at(5),
        )
        .unwrap();
    assert!(!back.written.is_empty());
    assert_eq!(pack.hashes(), version_1);
}

/// Estado completo da pasta e do histórico, para provar que uma falha não mudou nada.
fn full_state(
    pack: &TestPack,
    repo: &PackRepo,
) -> (
    BTreeMap<String, String>,
    Vec<String>,
    Option<String>,
    Vec<String>,
) {
    let tags: Vec<String> = repo
        .versions()
        .unwrap()
        .into_iter()
        .map(|v| v.commit)
        .collect();
    (pack.hashes(), pack.dirs(), repo.head_id().unwrap(), tags)
}

#[test]
fn ca_t17_02_falha_em_qualquer_passo_deixa_o_pack_como_estava() {
    let (pack, repo, _version_1) = pack_with_two_states();
    write_ignored(&pack);
    let before = full_state(&pack, &repo);

    // Conta os passos de uma restauração que dá certo, num pack igual.
    let (probe_pack, probe_repo, _) = pack_with_two_states();
    let apply_probe = fault::arm_after("versioning.restore.apply", usize::MAX);
    let stage_probe = fault::arm_after("versioning.restore.stage", usize::MAX);
    restore(&probe_repo, "1.0.0", 3).unwrap();
    let apply_steps = fault::hits("versioning.restore.apply");
    let stage_steps = fault::hits("versioning.restore.stage");
    drop((apply_probe, stage_probe, probe_pack));
    assert!(
        apply_steps >= 8,
        "poucos passos para o teste valer: {apply_steps}"
    );
    assert!(stage_steps >= 4, "{stage_steps}");

    for step in 0..stage_steps {
        let _guard = fault::arm_after("versioning.restore.stage", step);
        let error = restore(&repo, "1.0.0", 3).unwrap_err();
        assert!(
            matches!(error, Error::Core(_)),
            "preparação {step}: {error}"
        );
        assert_eq!(full_state(&pack, &repo), before, "preparação {step}");
    }
    for step in 0..apply_steps {
        let _guard = fault::arm_after("versioning.restore.apply", step);
        let error = restore(&repo, "1.0.0", 3).unwrap_err();
        assert!(matches!(error, Error::Core(_)), "aplicação {step}: {error}");
        assert_eq!(full_state(&pack, &repo), before, "aplicação {step}");
    }
    // Cada tentativa deixou o seu ponto de segurança (o estado de antes, intacto).
    let points = repo.safety_points().unwrap();
    assert_eq!(points.len(), stage_steps + apply_steps);
    // E depois de tudo, a restauração normal funciona.
    restore(&repo, "1.0.0", 4).unwrap();
}

#[test]
fn desfazer_que_tambem_falha_aponta_o_ponto_de_seguranca() {
    let (pack, repo, _) = pack_with_two_states();
    let before = pack.hashes();
    // Falha no 5º passo da aplicação e no primeiro passo do desfazer: a reserva não volta
    // por renomeação, então o arquivo é regravado a partir do ponto de segurança.
    let _apply = fault::arm_after("versioning.restore.apply", 4);
    let _rollback = fault::arm_after("versioning.restore.rollback", 0);
    let error = restore(&repo, "1.0.0", 3).unwrap_err();
    let hashes = pack.hashes();
    let relevant: BTreeMap<_, _> = hashes
        .into_iter()
        .filter(|(path, _)| !path.ends_with(".warden-tmp"))
        .collect();
    match &error {
        // O desfazer recuperou tudo pelo ponto de segurança.
        Error::Core(_) => assert_eq!(relevant, before),
        Error::RollbackFailed {
            safety_point,
            paths,
            ..
        } => {
            assert!(!paths.is_empty());
            assert!(repo.safety_point(safety_point).is_ok());
            assert_eq!(error.params()["safetyPoint"], *safety_point);
        }
        other => panic!("erro inesperado: {other}"),
    }
}

#[test]
fn desfazer_sem_saida_devolve_rollback_failed() {
    let (pack, repo, _) = pack_with_two_states();
    // Um arquivo novo é posto no lugar (passo de gravação) e a falha vem depois; o desfazer
    // não consegue apagar o arquivo novo (falha injetada em todos os passos do desfazer).
    let _apply = fault::arm_after("versioning.restore.apply", 7);
    let _rollback = fault::arm_after("versioning.restore.rollback", 0);
    let error = restore(&repo, "1.0.0", 3).unwrap_err();
    // O primeiro passo desfeito é o último aplicado; com a falha injetada, ele não volta.
    let Error::RollbackFailed {
        safety_point,
        paths,
        source,
    } = &error
    else {
        panic!("esperava RollbackFailed: {error}");
    };
    assert_eq!(paths.len(), 1, "{paths:?}");
    assert!(matches!(**source, Error::Core(_)));
    assert!(repo.safety_point(safety_point).is_ok());
    assert!(!error.retryable());
    // Recuperar pelo ponto de segurança devolve o estado de antes.
    let point = repo.safety_point(safety_point).unwrap();
    repo.restore(&RestoreTarget::SafetyPoint(point.name), &identity(), at(9))
        .unwrap();
    assert!(pack.exists("config/rascunho.toml"));
    assert_eq!(pack.read("mods/local mod.jar"), b"outro jar");
}

#[cfg(windows)]
#[test]
fn arquivo_aberto_por_outro_programa_nao_deixa_nada_pela_metade() {
    use std::os::windows::fs::OpenOptionsExt as _;

    let (pack, repo, _) = pack_with_two_states();
    let before = full_state(&pack, &repo);
    // O "jogo" abre um config que a restauração precisa trocar, sem compartilhar nada
    // (nem leitura, nem remoção): renomear e apagar falham com violação de compartilhamento.
    let held = fs::OpenOptions::new()
        .read(true)
        .share_mode(0)
        .open(pack.path("kubejs/server_scripts/receitas.js"))
        .unwrap();
    let error = restore(&repo, "1.0.0", 3).unwrap_err();
    drop(held);

    assert!(
        matches!(&error, Error::FileInUse { path, .. } if path == "kubejs/server_scripts/receitas.js"),
        "{error}"
    );
    assert_eq!(error.params()["path"], "kubejs/server_scripts/receitas.js");
    assert!(error.retryable());
    assert_eq!(full_state(&pack, &repo), before);
    // Fechado o "jogo", a mesma restauração funciona.
    restore(&repo, "1.0.0", 4).unwrap();
}

#[cfg(windows)]
#[test]
fn arquivo_somente_leitura_e_substituido_e_a_reserva_apagada() {
    let (pack, repo, version_1) = pack_with_two_states();
    let path = pack.path("config/ação.toml");
    let mut permissions = fs::metadata(&path).unwrap().permissions();
    permissions.set_readonly(true);
    fs::set_permissions(&path, permissions).unwrap();
    let report = restore(&repo, "1.0.0", 3).unwrap();
    assert!(
        report.leftover_temp_files.is_empty(),
        "{:?}",
        report.leftover_temp_files
    );
    assert_eq!(pack.hashes(), version_1);
}

#[test]
fn voltar_para_versao_inexistente_nao_mexe_em_nada() {
    let (pack, repo, _) = pack_with_two_states();
    let before = full_state(&pack, &repo);
    assert!(matches!(
        restore(&repo, "7.0.0", 3).unwrap_err(),
        Error::VersionNotFound { .. }
    ));
    assert!(matches!(
        repo.restore(
            &RestoreTarget::SafetyPoint("nao-existe".into()),
            &identity(),
            at(3)
        )
        .unwrap_err(),
        Error::SafetyPointNotFound { .. }
    ));
    assert_eq!(full_state(&pack, &repo), before);
    assert!(repo.safety_points().unwrap().is_empty());
}

#[test]
fn voltar_com_historico_em_estado_especial_e_recusado() {
    let (pack, repo, _) = pack_with_two_states();
    let head = repo.head_id().unwrap().unwrap();
    fs::write(pack.path(".git/MERGE_HEAD"), format!("{head}\n")).unwrap();
    assert!(matches!(
        restore(&repo, "1.0.0", 3).unwrap_err(),
        Error::RepoBusy { .. }
    ));
    assert!(repo.safety_points().unwrap().is_empty());
}

#[test]
fn voltar_para_a_versao_atual_sem_mudancas() {
    let (pack, repo) = TestPack::versioned();
    pack.write("config/a.toml", b"a = 1\n");
    save(&repo, "1.0.0", 1);
    let before = pack.hashes();
    let report = restore(&repo, "1.0.0", 2).unwrap();
    assert!(report.written.is_empty() && report.deleted.is_empty());
    assert_eq!(pack.hashes(), before);
}

#[test]
fn arquivo_no_lugar_de_uma_pasta_e_pasta_no_lugar_de_um_arquivo() {
    let (pack, repo) = TestPack::versioned();
    pack.write("config/x", b"arquivo x\n");
    save(&repo, "1.0.0", 1);
    let version_1 = pack.hashes();
    pack.remove("config/x");
    pack.write("config/x/dentro.toml", b"agora x e pasta\n");
    save(&repo, "1.1.0", 2);
    let version_2 = pack.hashes();

    restore(&repo, "1.0.0", 3).unwrap();
    assert_eq!(pack.hashes(), version_1);
    restore(&repo, "1.1.0", 4).unwrap();
    assert_eq!(pack.hashes(), version_2);
}

#[test]
fn line_endings_seguem_o_gitattributes_mesmo_com_autocrlf_no_repositorio() {
    let (pack, repo) = TestPack::versioned();
    // Mesmo que alguém ligue o autocrlf neste repositório, o `* -text` do pack manda.
    raw(&pack)
        .config()
        .unwrap()
        .set_bool("core.autocrlf", true)
        .unwrap();
    pack.write("config/crlf.cfg", b"a=1\r\nb=2\r\n");
    pack.write("config/lf.cfg", b"a=1\nb=2\n");
    pack.write("config/misto.cfg", b"a=1\r\nb=2\n");
    save(&repo, "1.0.0", 1);
    let version_1 = pack.hashes();
    let git = raw(&pack);
    let tree = git.head().unwrap().peel_to_tree().unwrap();
    let blob = |path: &str| {
        tree.get_path(std::path::Path::new(path))
            .unwrap()
            .to_object(&git)
            .unwrap()
            .peel_to_blob()
            .unwrap()
            .content()
            .to_vec()
    };
    assert_eq!(blob("config/crlf.cfg"), b"a=1\r\nb=2\r\n");
    assert_eq!(blob("config/misto.cfg"), b"a=1\r\nb=2\n");
    pack.write("config/crlf.cfg", b"mudou\n");
    pack.remove("config/lf.cfg");
    restore(&repo, "1.0.0", 2).unwrap();
    assert_eq!(pack.hashes(), version_1);
}

#[test]
fn ponto_de_seguranca_avulso_nao_mexe_em_nada() {
    let (pack, repo, _) = pack_with_two_states();
    let before = full_state(&pack, &repo);
    let index_before = fs::read(pack.path(".git/index")).unwrap();
    let first = repo
        .create_safety_point(&SafetyReason::RemoveItems { count: 3 }, &identity(), at(7))
        .unwrap();
    // Mesmo segundo e mesmo motivo: nome com sufixo, sem sobrescrever.
    let second = repo
        .create_safety_point(&SafetyReason::RemoveItems { count: 3 }, &identity(), at(7))
        .unwrap();
    assert_eq!(first.name, "20261001T150700Z-remover-3");
    assert_eq!(second.name, "20261001T150700Z-remover-3-2");
    assert_eq!(first.reason, "antes de remover 3 itens");
    assert_eq!(full_state(&pack, &repo), before);
    assert_eq!(fs::read(pack.path(".git/index")).unwrap(), index_before);
    assert!(git_status(&pack).contains(&"config/rascunho.toml".to_owned()));
    assert_eq!(repo.safety_point(&first.name).unwrap(), first);
    assert_eq!(repo.safety_points().unwrap().len(), 2);
}
