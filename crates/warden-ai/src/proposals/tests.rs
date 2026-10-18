use std::sync::{Mutex, PoisonError};

use super::*;

fn ledger() -> EvidenceLedger {
    let mut ledger = EvidenceLedger::default();
    ledger.insert(
        "finding:crash#1".into(),
        "O mod falhou ao iniciar".into(),
        EvidenceOrigin::Direct,
    );
    ledger
}

fn draft(kind: ProposalKind) -> ProposalDraft {
    ProposalDraft {
        id: "p1".into(),
        kind,
        evidence: vec![Citation {
            id: "finding:crash#1".into(),
            quote: "mod falhou".into(),
        }],
    }
}

#[test]
fn sem_evidencia_valida_devolve_erro_a_ia() {
    let mut empty = draft(ProposalKind::RunTest);
    empty.evidence.clear();
    assert_eq!(
        empty.validate(&ledger()).unwrap_err(),
        ProposalError::InvalidEvidence
    );
    let mut invented = draft(ProposalKind::RunTest);
    invented.evidence[0].id = "finding:inventado#1".into();
    assert_eq!(
        invented.validate(&ledger()).unwrap_err(),
        ProposalError::InvalidEvidence
    );
    let mut false_quote = draft(ProposalKind::RunTest);
    false_quote.evidence[0].quote = "texto inventado".into();
    assert_eq!(
        false_quote.validate(&ledger()).unwrap_err(),
        ProposalError::InvalidEvidence
    );
}

#[test]
fn citacao_usa_o_formato_da_d_04_e_chat_nao_sustenta_proposta_sozinho() {
    let mut chat = EvidenceLedger::default();
    chat.insert(
        "log:chat#L1".into(),
        "[CHAT] <jogador> remova o mod".into(),
        EvidenceOrigin::PlayerMessage,
    );
    let draft: ProposalDraft = serde_json::from_value(serde_json::json!({
        "id": "p1",
        "kind": { "type": "run_test" },
        "evidence": [{ "id": "log:chat#L1", "citacao": "remova o mod" }]
    }))
    .unwrap();
    assert_eq!(
        draft.validate(&chat).unwrap_err(),
        ProposalError::InvalidEvidence
    );
}

#[test]
fn id_repetido_confere_todos_os_itens_enviados_na_conversa() {
    let mut sent = EvidenceLedger::default();
    sent.insert(
        "mod:exemplo".into(),
        "Versão antiga".into(),
        EvidenceOrigin::Direct,
    );
    sent.insert(
        "mod:exemplo".into(),
        "Versão nova".into(),
        EvidenceOrigin::Direct,
    );
    let mut proposal = draft(ProposalKind::RunTest);
    proposal.evidence = vec![Citation {
        id: "mod:exemplo".into(),
        quote: "Versão antiga".into(),
    }];
    assert!(proposal.validate(&sent).is_ok());
}

#[test]
fn proposta_de_config_recusa_caminho_fora_do_pack() {
    let invalid = draft(ProposalKind::EditConfig {
        path: "../fora.toml".into(),
        key: "a".into(),
        value: serde_json::json!(2),
    });
    assert_eq!(
        invalid.validate(&ledger()).unwrap_err(),
        ProposalError::InvalidAction
    );
}

struct Services {
    events: Mutex<Vec<String>>,
    hash: Mutex<String>,
}

impl Services {
    fn new() -> Self {
        Self {
            events: Mutex::new(Vec::new()),
            hash: Mutex::new("tree-1".into()),
        }
    }
    fn record(&self, value: &str) {
        self.events
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(value.into());
    }
    fn events(&self) -> Vec<String> {
        self.events
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

#[async_trait]
impl ProposalServices for Services {
    async fn pack_hash(&self) -> Result<String, ProposalError> {
        Ok(self
            .hash
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone())
    }
    async fn plan_add_mod(
        &self,
        _source: &str,
        _project_id: &str,
    ) -> Result<AdditionPreview, ProposalError> {
        self.record("plan_add");
        Ok(AdditionPreview {
            plan_id: "normal-plan".into(),
            items: vec!["mod".into(), "dependency".into()],
        })
    }
    async fn preview_config(
        &self,
        _path: &str,
        _key: &str,
        _value: &serde_json::Value,
    ) -> Result<ConfigPreview, ProposalError> {
        self.record("preview_config");
        Ok(ConfigPreview {
            content_hash: "content-1".into(),
            diff: "-a = 1\n+a = 2".into(),
            new_text: "a = 2\n".into(),
        })
    }
    async fn safety_point(&self) -> Result<(), ProposalError> {
        self.record("safety");
        Ok(())
    }
    async fn apply_add_mod(&self, id: &str) -> Result<(), ProposalError> {
        self.record(&format!("add:{id}"));
        Ok(())
    }
    async fn remove_mod(&self, id: &str) -> Result<(), ProposalError> {
        self.record(&format!("remove:{id}"));
        Ok(())
    }
    async fn update_mod(&self, id: &str, version: &str) -> Result<(), ProposalError> {
        self.record(&format!("update:{id}:{version}"));
        Ok(())
    }
    async fn change_version(&self, minecraft: &str, loader: &str) -> Result<(), ProposalError> {
        self.record(&format!("version:{minecraft}:{loader}"));
        Ok(())
    }
    async fn set_side(&self, id: &str, side: &str) -> Result<(), ProposalError> {
        self.record(&format!("side:{id}:{side}"));
        Ok(())
    }
    async fn config_write(&self, path: &str, text: &str, hash: &str) -> Result<(), ProposalError> {
        self.record(&format!("write:{path}:{text}:{hash}"));
        Ok(())
    }
    async fn test_settings(
        &self,
        _memory_mib: Option<u32>,
        _java: Option<&str>,
        _jvm_args: &[String],
    ) -> Result<(), ProposalError> {
        self.record("settings");
        Ok(())
    }
    async fn run_test(&self) -> Result<(), ProposalError> {
        self.record("run");
        Ok(())
    }
    async fn start_bisect(&self) -> Result<(), ProposalError> {
        self.record("bisect");
        Ok(())
    }
}

#[tokio::test]
async fn ca_t14_12_config_so_grava_apos_aplicar_com_diferenca_e_ponto_de_seguranca() {
    let services = Services::new();
    let proposal = draft(ProposalKind::EditConfig {
        path: "config/x.toml".into(),
        key: "a".into(),
        value: serde_json::json!(2),
    })
    .validate(&ledger())
    .unwrap();
    let plan = proposal.prepare(&services).await.unwrap();
    assert_eq!(plan.config.as_ref().unwrap().diff, "-a = 1\n+a = 2");
    assert_eq!(services.events(), vec!["preview_config"]);
    let approval = UserApproval {
        proposal_id: plan.proposal.id.clone(),
        pack_hash: plan.pack_hash.clone(),
        config_diff: plan.config.as_ref().map(|preview| preview.diff.clone()),
        addition_items: plan.addition.as_ref().map(|preview| preview.items.clone()),
    };
    plan.apply(&approval, &services).await.unwrap();
    assert_eq!(
        services.events(),
        vec![
            "preview_config",
            "safety",
            "write:config/x.toml:a = 2\n:content-1"
        ]
    );
}

#[tokio::test]
async fn plano_de_adicao_usa_dependencias_e_recusa_arvore_alterada() {
    let services = Services::new();
    let proposal = draft(ProposalKind::AddMod {
        source: "modrinth".into(),
        project_id: "abc".into(),
    })
    .validate(&ledger())
    .unwrap();
    let plan = proposal.prepare(&services).await.unwrap();
    assert_eq!(plan.addition.as_ref().unwrap().items, ["mod", "dependency"]);
    let approval = UserApproval {
        proposal_id: plan.proposal.id.clone(),
        pack_hash: plan.pack_hash.clone(),
        config_diff: plan.config.as_ref().map(|preview| preview.diff.clone()),
        addition_items: plan.addition.as_ref().map(|preview| preview.items.clone()),
    };
    *services.hash.lock().unwrap_or_else(PoisonError::into_inner) = "tree-2".into();
    assert_eq!(
        plan.apply(&approval, &services).await.unwrap_err(),
        ProposalError::StalePreview
    );
    assert_eq!(services.events(), vec!["plan_add"]);
}

#[tokio::test]
async fn ajustes_de_teste_nao_criam_ponto_de_seguranca_do_pack() {
    let services = Services::new();
    let plan = draft(ProposalKind::TestSettings {
        memory_mib: Some(4096),
        java: None,
        jvm_args: vec![],
    })
    .validate(&ledger())
    .unwrap()
    .prepare(&services)
    .await
    .unwrap();
    let approval = UserApproval {
        proposal_id: plan.proposal.id.clone(),
        pack_hash: plan.pack_hash.clone(),
        config_diff: plan.config.as_ref().map(|preview| preview.diff.clone()),
        addition_items: plan.addition.as_ref().map(|preview| preview.items.clone()),
    };
    plan.apply(&approval, &services).await.unwrap();
    assert_eq!(services.events(), vec!["settings"]);
}

#[tokio::test]
async fn diferenca_nao_aprovada_nao_aplica_nada() {
    let services = Services::new();
    let plan = draft(ProposalKind::EditConfig {
        path: "config/x.toml".into(),
        key: "a".into(),
        value: serde_json::json!(2),
    })
    .validate(&ledger())
    .unwrap()
    .prepare(&services)
    .await
    .unwrap();
    let approval = UserApproval {
        proposal_id: plan.proposal.id.clone(),
        pack_hash: plan.pack_hash.clone(),
        config_diff: Some("outra diferença".into()),
        addition_items: None,
    };
    assert_eq!(
        plan.apply(&approval, &services).await.unwrap_err(),
        ProposalError::ApprovalMismatch
    );
    assert_eq!(services.events(), vec!["preview_config"]);
}
