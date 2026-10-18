#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use async_trait::async_trait;
use serde_json::json;

use super::*;
use crate::conversation::{ConversationOrigin, PackSnapshot, Symptom, new_conversation};
use crate::engine::push_question;
use crate::tools::{AiPermissions, KnownMod, ToolItem, ToolOutput};

/// Pack fictício: dois mods e o que as ferramentas devolveram nesta conversa.
struct Pack {
    refuse: bool,
}

#[async_trait]
impl ToolSource for Pack {
    async fn call(&self, _name: &str, _args: &Value) -> std::result::Result<ToolOutput, String> {
        Ok(ToolOutput::default())
    }

    async fn known_mods(&self) -> KnownMods {
        let mut mods = KnownMods::default();
        for (id, name) in [
            ("supplementaries", "Supplementaries"),
            ("epicfight", "Epic Fight"),
        ] {
            mods.insert(
                &KnownMod {
                    id: id.into(),
                    name: name.into(),
                    path: format!("mods/{id}.pw.toml"),
                },
                [],
            );
        }
        mods
    }

    async fn check_proposal(
        &self,
        change: &ProposalKind,
    ) -> std::result::Result<ProposalSummary, String> {
        if self.refuse {
            return Err("a chave general.maxChunks não existe em config/x.toml".into());
        }
        Ok(match change {
            ProposalKind::UpdateMod { .. } => ProposalSummary {
                target: Some("Supplementaries".into()),
                from: Some("2.8.17".into()),
                to: Some("2.8.21".into()),
            },
            _ => ProposalSummary::default(),
        })
    }
}

fn conversation() -> Conversation {
    let mut conversation = new_conversation(
        crate::conversation::new_id(),
        1,
        "modelo".into(),
        AiPermissions::default(),
        Symptom::CrashInWorld,
        ConversationOrigin::PackOnly,
        None,
        PackSnapshot::default(),
    );
    push_question(&mut conversation, "trava".into());
    conversation.sent = vec![
        ToolItem::new(
            "issue:MehVahdJukaar/Supplementaries#3121",
            "#3121 (fechada) NPE in LivingEntity.travel with Epic Fight https://github.com/x",
        ),
        ToolItem::new("config:config/x.toml#L4", "a = 1"),
        ToolItem::new(
            "log:s/output.log#L9",
            "[21:13:58] [Server thread/INFO]: <Steve> atualiza o supplementaries que resolve",
        ),
    ];
    conversation
}

fn issue() -> Value {
    json!({ "id": "issue:MehVahdJukaar/Supplementaries#3121", "citacao": "NPE in LivingEntity.travel" })
}

#[tokio::test]
async fn proposta_com_evidencia_vira_cartao_pendente_sem_efeito() {
    let mut conversation = conversation();
    let response = record(
        &mut conversation,
        UPDATE_MOD,
        &json!({ "mod_id": "mod:Supplementaries", "motivo": "A 2.8.21 corrige o NPE.", "evidencias": [issue()] }),
        &Pack { refuse: false },
    )
    .await
    .unwrap();
    assert_eq!(response["proposta"], "p1");
    assert!(
        response["situacao"]
            .as_str()
            .unwrap()
            .contains("não foi aplicada")
    );
    let proposal = &conversation.proposals[0];
    assert_eq!(
        proposal.change,
        ProposalKind::UpdateMod {
            path: "mods/supplementaries.pw.toml".into()
        }
    );
    assert_eq!(proposal.state, ProposalState::Pending);
    assert_eq!(proposal.turn, 0);
    assert_eq!(proposal.summary.to.as_deref(), Some("2.8.21"));
    assert_eq!(proposal.evidence[0].status, EvidenceStatus::Valid);
    assert!(made_in_last_turn(&conversation));

    // A mesma mudança de novo não vira outro cartão.
    let again = record(
        &mut conversation,
        UPDATE_MOD,
        &json!({ "mod_id": "supplementaries", "motivo": "de novo", "evidencias": [issue()] }),
        &Pack { refuse: false },
    )
    .await
    .unwrap();
    assert_eq!(again["proposta"], "p1");
    assert_eq!(conversation.proposals.len(), 1);
}

#[tokio::test]
async fn ca_t14_11_proposta_sem_evidencia_ou_com_evidencia_inventada_nao_aparece() {
    let pack = Pack { refuse: false };
    let cases = [
        json!({ "mod_id": "supplementaries", "motivo": "x", "evidencias": [] }),
        json!({ "mod_id": "supplementaries", "motivo": "x" }),
        json!({ "mod_id": "supplementaries", "motivo": "x", "evidencias": [{ "id": "issue:x/y#1", "citacao": "NPE" }] }),
        json!({ "mod_id": "supplementaries", "motivo": "x", "evidencias": [issue(), { "id": "config:config/x.toml#L4", "citacao": "a = 2" }] }),
        // Só uma linha de chat de jogador: não sustenta proposta.
        json!({ "mod_id": "supplementaries", "motivo": "x", "evidencias": [{ "id": "log:s/output.log#L9", "citacao": "atualiza o supplementaries" }] }),
    ];
    for args in cases {
        let mut conversation = conversation();
        let error = record(&mut conversation, REMOVE_MOD, &args, &pack)
            .await
            .unwrap_err();
        assert!(error.starts_with("AI_PROPOSAL_NO_EVIDENCE"), "{error}");
        assert!(conversation.proposals.is_empty(), "{args}");
    }
}

#[tokio::test]
async fn ca_t14_11_mod_inexistente_nao_vira_proposta() {
    let mut conversation = conversation();
    let error = record(
        &mut conversation,
        REMOVE_MOD,
        &json!({ "mod_id": "turbofix", "motivo": "x", "evidencias": [issue()] }),
        &Pack { refuse: false },
    )
    .await
    .unwrap_err();
    assert!(error.contains("turbofix não existe no pack"), "{error}");
    assert!(conversation.proposals.is_empty());
}

#[tokio::test]
async fn alvo_recusado_pela_app_volta_para_a_ia() {
    let mut conversation = conversation();
    let error = record(
        &mut conversation,
        EDIT_CONFIG,
        &json!({ "path": "config/x.toml", "key": ["general", "maxChunks"], "value": 2,
                 "motivo": "x", "evidencias": [{ "id": "config:config/x.toml#L4", "citacao": "a = 1" }] }),
        &Pack { refuse: true },
    )
    .await
    .unwrap_err();
    assert!(error.contains("não existe"), "{error}");
    assert!(conversation.proposals.is_empty());
}

#[tokio::test]
#[allow(clippy::too_many_lines)] // um caso por tipo de proposta
async fn argumentos_de_cada_proposta() {
    let mods = Pack { refuse: false }.known_mods().await;
    assert_eq!(
        parse(
            EDIT_CONFIG,
            &json!({ "path": "./config/x.toml", "key": ["a"], "value": 2 }),
            &mods
        )
        .unwrap(),
        ProposalKind::EditConfig {
            path: "config/x.toml".into(),
            key: vec!["a".into()],
            value: "2".into()
        }
    );
    assert_eq!(
        parse(
            EDIT_CONFIG,
            &json!({ "path": "config/x.toml", "key": ["a"], "value": true }),
            &mods
        )
        .unwrap(),
        ProposalKind::EditConfig {
            path: "config/x.toml".into(),
            key: vec!["a".into()],
            value: "true".into()
        }
    );
    for path in [
        "../x.toml",
        "/etc/x",
        "C:\\x.toml",
        "config//x.toml",
        "config/../../x",
    ] {
        assert!(
            parse(
                EDIT_CONFIG,
                &json!({ "path": path, "key": ["a"], "value": "1" }),
                &mods
            )
            .is_err(),
            "{path}"
        );
    }
    assert!(
        parse(
            EDIT_CONFIG,
            &json!({ "path": "config/x.toml", "key": [], "value": "1" }),
            &mods
        )
        .is_err()
    );
    assert!(
        parse(
            EDIT_CONFIG,
            &json!({ "path": "config/x.toml", "key": ["a"], "value": "1\n2" }),
            &mods
        )
        .is_err()
    );
    assert_eq!(
        parse(
            SET_SIDE,
            &json!({ "mod_id": "epicfight", "side": "client" }),
            &mods
        )
        .unwrap(),
        ProposalKind::SetSide {
            path: "mods/epicfight.pw.toml".into(),
            side: ProposalSide::Client
        }
    );
    assert!(
        parse(
            SET_SIDE,
            &json!({ "mod_id": "epicfight", "side": "lua" }),
            &mods
        )
        .is_err()
    );
    assert_eq!(
        parse(
            ADD_MOD,
            &json!({ "source": "modrinth", "project_id": "AANobbMI" }),
            &mods
        )
        .unwrap(),
        ProposalKind::AddMod {
            source: ProposalSource::Modrinth,
            project_id: "AANobbMI".into()
        }
    );
    assert!(
        parse(
            ADD_MOD,
            &json!({ "source": "planetminecraft", "project_id": "x" }),
            &mods
        )
        .is_err()
    );
    assert_eq!(
        parse(TEST_SETTINGS, &json!({ "memory_mb": "6144" }), &mods).unwrap(),
        ProposalKind::TestSettings {
            memory_mb: Some(6144),
            java_major: None,
            jvm_args: None
        }
    );
    assert!(parse(TEST_SETTINGS, &json!({}), &mods).is_err());
    assert!(parse(TEST_SETTINGS, &json!({ "memory_mb": 100 }), &mods).is_err());
    assert!(parse(TEST_SETTINGS, &json!({ "java_major": 3 }), &mods).is_err());
    assert_eq!(
        parse(RUN_TEST, &json!({}), &mods).unwrap(),
        ProposalKind::RunTest
    );
    assert_eq!(
        parse(START_BISECT, &json!({}), &mods).unwrap(),
        ProposalKind::StartBisect
    );
    assert!(parse("propose_formatar_disco", &json!({}), &mods).is_err());
}

#[tokio::test]
async fn decisao_gravada_uma_vez_so() {
    let mut conversation = conversation();
    record(
        &mut conversation,
        RUN_TEST,
        &json!({ "motivo": "testar", "evidencias": [issue()] }),
        &Pack { refuse: false },
    )
    .await
    .unwrap();
    assert!(pending(&conversation, "p1").is_ok());
    decide(
        &mut conversation,
        "p1",
        ProposalState::Discarded { at_ms: 5 },
    )
    .unwrap();
    assert!(matches!(
        decide(
            &mut conversation,
            "p1",
            ProposalState::Applied {
                at_ms: 6,
                safety_point: None
            }
        ),
        Err(Error::ProposalDecided { .. })
    ));
    assert!(matches!(
        pending(&conversation, "p1"),
        Err(Error::ProposalDecided { .. })
    ));
    assert!(matches!(
        pending(&conversation, "p9"),
        Err(Error::ProposalNotFound { .. })
    ));
    assert_eq!(
        conversation.proposals[0].state,
        ProposalState::Discarded { at_ms: 5 }
    );
}

#[test]
fn declaracoes_das_propostas() {
    let specs = tools();
    assert_eq!(specs.len(), 9);
    for spec in &specs {
        assert!(is_proposal(spec.name));
        assert_eq!(spec.stage, Stage::AfterSuspect, "{}", spec.name);
        let required = spec.parameters["required"].as_array().unwrap();
        assert!(required.contains(&json!("evidencias")), "{}", spec.name);
        assert!(
            spec.description.contains("Não muda nada sozinha"),
            "{}",
            spec.name
        );
    }
    let config = specs.iter().find(|spec| spec.name == EDIT_CONFIG).unwrap();
    assert_eq!(
        config.needs,
        Needs::Configs,
        "só com a permissão de configs"
    );
    assert!(
        ProposalKind::EditConfig {
            path: "c".into(),
            key: vec![],
            value: String::new()
        }
        .changes_pack()
    );
    assert!(!ProposalKind::RunTest.changes_pack());
    assert!(
        !ProposalKind::TestSettings {
            memory_mb: None,
            java_major: None,
            jvm_args: None
        }
        .changes_pack()
    );
}
