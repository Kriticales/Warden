//! As propostas no app, sobre um pack de verdade numa pasta temporária, com o Gemini simulado e
//! o packwiz de `binaries/` (CA-T14-12): a proposta não muda nada até **Aplicar**; Aplicar
//! mostra a diferença, grava só aquela linha e cria o ponto de segurança; **Descartar** não grava
//! nada (hash da pasta antes e depois); o pack mudado depois da diferença não é gravado.

use std::fs;

use serde_json::{Value, json};
use warden_ai::proposals::{ProposalKind, ProposalState};
use warden_ai::tools::AiPermissions;
use warden_project::configs::LineEnding;
use warden_versioning::{Identity, InitialPoint, Moment, PackRepo};

use super::*;
use crate::ai_tools::tests::{Setup, calls, folder_hash, noop, setup};
use crate::commands::ai::{
    AiPreviewRequest, AiSource, get_impl, preview_impl, run_impl, start_impl,
};
use crate::commands::inventory::tests::{packwiz_binary, test_packwiz};

const CONFIG: &str = "config/create-common.toml";

fn propose(value: &str) -> (&'static str, Value) {
    (
        "propose_edit_config",
        json!({ "path": CONFIG, "key": ["trains", "maxAssemblyLength"], "value": value,
                "motivo": "Trens longos travam o servidor.",
                "evidencias": [{ "id": "config:config/create-common.toml#L2", "citacao": "maxAssemblyLength = 128" }] }),
    )
}

/// A IA lê a config, propõe dois valores para a mesma chave e responde.
fn script() -> crate::ai_tools::tests::Script {
    Box::new(|n, _| match n {
        0 => calls(&[("read_config", json!({ "path": CONFIG }))]),
        1 => calls(&[propose("256"), propose("512")]),
        _ => calls(&[(
            "responder",
            json!({ "resumo": "Aumentar o limite de montagem dos trens deve resolver.", "semConclusao": false,
                    "achados": [], "propostas": [] }),
        )]),
    })
}

/// Conversa rodada até a resposta, com o pack versionado (para o ponto de segurança).
async fn conversation(setup: &Setup) -> String {
    PackRepo::init(
        &setup.root,
        InitialPoint::Created,
        &Identity::for_pack("Kriticales", None),
        Moment::new(1_790_787_600, -180),
    )
    .unwrap();
    let preview = preview_impl(
        &setup.state,
        setup.pack,
        AiPreviewRequest {
            symptom: warden_ai::conversation::AiSymptom::CrashInWorld,
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
    id
}

#[tokio::test]
async fn ca_t14_12_so_aplicar_grava_so_aquela_linha_com_ponto_de_seguranca_e_descartar_nada() {
    let Some(binary) = packwiz_binary() else {
        return;
    };
    let setup = setup(script()).await;
    let cli = test_packwiz(&setup.state, binary);
    let config_path = setup.root.join(CONFIG);
    let original = fs::read_to_string(&config_path).unwrap();
    let id = conversation(&setup).await;
    let untouched = folder_hash(&setup.root);

    // A IA propôs: dois cartões pendentes, nada mudou no pack.
    let view = get_impl(&setup.state, setup.pack, &id).await.unwrap();
    assert_eq!(view.proposals.len(), 2, "{:?}", view.turns[0].steps);
    assert!(
        view.proposals
            .iter()
            .all(|p| p.state == ProposalState::Pending)
    );
    assert_eq!(view.proposals[0].summary.from.as_deref(), Some("128"));
    assert_eq!(view.proposals[0].summary.to.as_deref(), Some("256"));
    assert_eq!(view.summary.pending_proposals, 2);
    assert_eq!(folder_hash(&setup.root), untouched);

    // Aplicar mostra a diferença exata; mostrar não grava.
    let review = review(&setup.state, setup.pack, &id, "p1").await.unwrap();
    assert!(review.changes_pack);
    let ProposalPreview::EditConfig {
        before, after, key, ..
    } = &review.preview
    else {
        panic!("{:?}", review.preview);
    };
    assert_eq!(key, "trains.maxAssemblyLength");
    assert_eq!(before, &original);
    assert_eq!(after, &original.replace("= 128", "= 256"));
    assert_eq!(folder_hash(&setup.root), untouched);

    // Descartar a outra: nada é gravado na pasta do pack.
    let discarded = discard(&setup.state, setup.pack, &id, "p2").unwrap();
    assert!(matches!(discarded.state, ProposalState::Discarded { .. }));
    assert_eq!(folder_hash(&setup.root), untouched);

    // Aplicar com o token do que foi mostrado: só aquela linha muda, com ponto de segurança.
    let repo = PackRepo::open(&setup.root).unwrap();
    assert!(repo.safety_points().unwrap().is_empty());
    let applied = apply_using(
        &setup.state,
        setup.pack,
        &id,
        "p1",
        &review.token,
        Some(&cli),
    )
    .await
    .unwrap();
    let written = fs::read_to_string(&config_path).unwrap();
    let changed: Vec<(&str, &str)> = original
        .lines()
        .zip(written.lines())
        .filter(|(a, b)| a != b)
        .collect();
    assert_eq!(
        changed,
        [("maxAssemblyLength = 128", "maxAssemblyLength = 256")]
    );
    assert_eq!(original.lines().count(), written.lines().count());
    let points = repo.safety_points().unwrap();
    assert_eq!(points.len(), 1);
    assert!(
        points[0].reason.contains("proposta da IA"),
        "{}",
        points[0].reason
    );
    assert_eq!(
        applied.safety_point.as_deref(),
        Some(points[0].name.as_str())
    );
    assert!(matches!(
        applied.proposal.state,
        ProposalState::Applied { .. }
    ));

    // Decidida não muda mais; a conversa guarda as duas decisões.
    let again = apply_using(
        &setup.state,
        setup.pack,
        &id,
        "p1",
        &review.token,
        Some(&cli),
    )
    .await;
    assert_eq!(
        again.unwrap_err().code,
        AppError::from_domain(&warden_ai::Error::ProposalDecided { id: "p1".into() }).code
    );
    let view = get_impl(&setup.state, setup.pack, &id).await.unwrap();
    assert!(matches!(
        view.proposals[0].state,
        ProposalState::Applied { .. }
    ));
    assert!(matches!(
        view.proposals[1].state,
        ProposalState::Discarded { .. }
    ));
    assert_eq!(view.summary.pending_proposals, 0);
}

#[tokio::test]
async fn pack_mudado_depois_da_diferenca_nao_e_gravado() {
    let Some(binary) = packwiz_binary() else {
        return;
    };
    let setup = setup(script()).await;
    let cli = test_packwiz(&setup.state, binary);
    let id = conversation(&setup).await;
    let review = review(&setup.state, setup.pack, &id, "p1").await.unwrap();
    // Alguém mudou a config depois de a diferença aparecer.
    let config_path = setup.root.join(CONFIG);
    let edited = fs::read_to_string(&config_path)
        .unwrap()
        .replace("= 128", "= 64");
    fs::write(&config_path, &edited).unwrap();
    let before = folder_hash(&setup.root);
    let stale = apply_using(
        &setup.state,
        setup.pack,
        &id,
        "p1",
        &review.token,
        Some(&cli),
    )
    .await;
    assert_eq!(
        stale.unwrap_err().code,
        AppError::from_domain(&warden_ai::Error::ProposalStale { id: "p1".into() }).code
    );
    assert_eq!(
        folder_hash(&setup.root),
        before,
        "nada gravado, nem ponto de segurança"
    );
    let view = get_impl(&setup.state, setup.pack, &id).await.unwrap();
    assert_eq!(view.proposals[0].state, ProposalState::Pending);
}

#[tokio::test]
async fn proposta_com_chave_inexistente_volta_para_a_ia() {
    let setup = setup(Box::new(|n, _| match n {
        0 => calls(&[("read_config", json!({ "path": CONFIG }))]),
        1 => calls(&[(
            "propose_edit_config",
            json!({ "path": CONFIG, "key": ["trains", "naoExiste"], "value": "1", "motivo": "x",
                    "evidencias": [{ "id": "config:config/create-common.toml#L2", "citacao": "maxAssemblyLength" }] }),
        )]),
        _ => calls(&[(
            "responder",
            json!({ "resumo": "Não achei a causa com o que vi.", "semConclusao": true, "achados": [], "propostas": [] }),
        )]),
    }))
    .await;
    let id = conversation(&setup).await;
    let view = get_impl(&setup.state, setup.pack, &id).await.unwrap();
    assert!(view.proposals.is_empty());
    let step = view.turns[0]
        .steps
        .iter()
        .find(|step| step.name == "propose_edit_config")
        .unwrap();
    assert!(!step.ok);
    assert!(
        step.sent.contains("trains.naoExiste não existe"),
        "{}",
        step.sent
    );
}

fn content(text: &str) -> warden_project::configs::ConfigContent {
    warden_project::configs::ConfigContent {
        path: "config/x.toml".into(),
        text: Some(text.into()),
        hash: "h".into(),
        size: 0,
        line_ending: LineEnding::Crlf,
        read_only: None,
        modified_at_ms: None,
    }
}

#[test]
fn edicao_de_config_no_windows_mantem_o_crlf_e_o_resto_do_arquivo() {
    let text =
        "# Limite\r\n[general]\r\n\t#Range: 1 ~ 512\r\n\tmaxChunks = 900\r\n\tname = \"x\"\r\n";
    let prepared =
        edit_config(content(text), &["general".into(), "maxChunks".into()], "2").unwrap();
    let ProposalPreview::EditConfig { after, .. } = &prepared.preview else {
        panic!();
    };
    assert_eq!(after, &text.replace("maxChunks = 900", "maxChunks = 2"));
    assert_eq!(prepared.summary.from.as_deref(), Some("900"));
    let Action::EditConfig { hash, .. } = &prepared.action else {
        panic!();
    };
    assert_eq!(
        hash, "h",
        "a gravação confere o arquivo lido para a diferença"
    );

    for (key, value) in [
        (vec!["general", "maxChunks"], "muitos"),
        (vec!["general", "nada"], "1"),
        (vec!["general", "maxChunks"], "900"),
    ] {
        let key: Vec<String> = key.into_iter().map(ToOwned::to_owned).collect();
        assert!(
            edit_config(content(text), &key, value).is_err(),
            "{key:?} = {value}"
        );
    }
    let flag = "[a]\nligado = true\n";
    let prepared = edit_config(content(flag), &["a".into(), "ligado".into()], "false").unwrap();
    assert!(
        matches!(&prepared.preview, ProposalPreview::EditConfig { after, .. } if after == "[a]\nligado = false\n")
    );
    assert!(edit_config(content(flag), &["a".into(), "ligado".into()], "talvez").is_err());
}

#[test]
fn areas_relidas_pela_interface() {
    let config = ProposalKind::EditConfig {
        path: CONFIG.into(),
        key: vec![],
        value: String::new(),
    };
    assert_eq!(areas_of(&config), [PackArea::Configs, PackArea::History]);
    assert_eq!(
        areas_of(&ProposalKind::RemoveMod {
            path: "mods/a.pw.toml".into()
        }),
        [PackArea::Inventory, PackArea::History]
    );
    assert!(areas_of(&ProposalKind::RunTest).is_empty());
}
