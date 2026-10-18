//! Propostas sem efeitos colaterais e contrato de aplicação pelos serviços normais do app.
//!
//! A D-04 guarda os itens enviados à IA em `Conversation.sent`. O adaptador Tauri os registra em
//! [`EvidenceLedger`] e implementa [`ProposalServices`] usando os mesmos fluxos da interface;
//! nenhum acesso ao pack ou chamada ao Gemini mora nesta crate.

use std::collections::BTreeMap;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Trecho que o modelo atribuiu a um resultado de ferramenta desta conversa.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Citation {
    /// Identificador citável devolvido pela ferramenta.
    pub id: String,
    /// Trecho literal do resultado enviado à IA.
    #[serde(rename = "citacao")]
    pub quote: String,
}

/// Origem do resultado. Mensagens de jogadores são citáveis, mas não provam sozinhas uma causa.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EvidenceOrigin {
    /// Informação produzida pelo Warden ou por uma API.
    Direct,
    /// Texto que um terceiro podia ter escrito no log, numa issue ou num changelog.
    ThirdParty,
    /// Linha de chat escrita por um jogador; sozinha não sustenta uma proposta.
    PlayerMessage,
}

/// Registro dos bytes de texto efetivamente devolvidos pelas ferramentas nesta conversa.
#[derive(Debug, Default, Clone)]
pub struct EvidenceLedger(BTreeMap<String, Vec<(String, EvidenceOrigin)>>);

impl EvidenceLedger {
    /// Registra o texto enviado à IA sob o mesmo id mostrado no resultado da ferramenta.
    pub fn insert(&mut self, id: String, text: String, origin: EvidenceOrigin) {
        self.0.entry(id).or_default().push((text, origin));
    }

    /// Confere id e citação literal, sem normalização que pudesse validar texto inventado.
    pub fn validate(&self, citation: &Citation) -> Result<EvidenceOrigin, ProposalError> {
        let Some(items) = self.0.get(&citation.id) else {
            return Err(ProposalError::InvalidEvidence);
        };
        items
            .iter()
            .find(|(text, _)| !citation.quote.trim().is_empty() && text.contains(&citation.quote))
            .map(|(_, origin)| *origin)
            .ok_or(ProposalError::InvalidEvidence)
    }
}

/// A ação sugerida pela IA. A serialização usa nomes estáveis para o registro da D-04.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ProposalKind {
    /// Adicionar um projeto pelo planejador normal, incluindo dependências.
    AddMod {
        /// Catálogo de origem.
        source: String,
        /// Id do projeto.
        project_id: String,
    },
    /// Remover o item do pack.
    RemoveMod {
        /// Id do item no pack.
        mod_id: String,
    },
    /// Atualizar o item para uma versão específica.
    UpdateMod {
        /// Id do item no pack.
        mod_id: String,
        /// Versão escolhida.
        version_id: String,
    },
    /// Mudar a versão do Minecraft e/ou loader.
    ChangeVersion {
        /// Versão do Minecraft.
        minecraft: String,
        /// Loader e versão escolhidos.
        loader: String,
    },
    /// Ajustar a presença do item nos lados cliente/servidor.
    SetSide {
        /// Id do item no pack.
        mod_id: String,
        /// Lado desejado.
        side: String,
    },
    /// Alterar uma chave de config por `config_write`, após mostrar a diferença.
    EditConfig {
        /// Caminho relativo da configuração.
        path: String,
        /// Chave a alterar.
        key: String,
        /// Novo valor.
        value: serde_json::Value,
    },
    /// Ajustar somente as opções do próximo teste.
    TestSettings {
        /// Memória em MiB, se alterada.
        memory_mib: Option<u32>,
        /// Java escolhido, se alterado.
        java: Option<String>,
        /// Argumentos da JVM.
        jvm_args: Vec<String>,
    },
    /// Executar um teste com as configurações atuais.
    RunTest,
    /// Abrir um plano de busca do culpado.
    StartBisect,
}

/// Proposta ainda não conferida; jamais deve aparecer como aplicável.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProposalDraft {
    /// Identificador da proposta na conversa.
    pub id: String,
    /// Ação solicitada.
    pub kind: ProposalKind,
    /// Citações obrigatórias.
    pub evidence: Vec<Citation>,
}

/// Proposta cuja evidência foi validada contra esta conversa.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ValidatedProposal {
    /// Identificador da proposta na conversa.
    pub id: String,
    /// Ação que poderá ser preparada para confirmação.
    pub kind: ProposalKind,
    /// Evidências verificadas com suas origens.
    pub evidence: Vec<ValidatedCitation>,
}

/// Citação conferida, inclusive sua procedência.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidatedCitation {
    /// Identificador devolvido pela ferramenta.
    pub id: String,
    /// Trecho literal.
    #[serde(rename = "citacao")]
    pub quote: String,
    /// Procedência do texto.
    pub origin: EvidenceOrigin,
}

/// Erro devolvido à IA quando a proposta não pode ser exibida ou aplicada.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum ProposalError {
    /// A proposta não cita texto válido da conversa.
    #[error("a proposta precisa citar evidência válida desta conversa")]
    InvalidEvidence,
    /// Um identificador ou valor obrigatório está vazio.
    #[error("a proposta contém campos obrigatórios vazios")]
    InvalidAction,
    /// O pack ou a config mudou depois da prévia.
    #[error("o pack mudou desde a prévia; prepare a proposta novamente")]
    StalePreview,
    /// O usuário não aprovou exatamente a prévia mostrada.
    #[error("a prévia aprovada não corresponde à proposta")]
    ApprovalMismatch,
    /// O serviço normal recusou o plano.
    #[error("o serviço do pack recusou a proposta: {0}")]
    Service(String),
}

impl ProposalDraft {
    /// Confere cada evidência. Uma citação inválida invalida a proposta toda.
    pub fn validate(self, ledger: &EvidenceLedger) -> Result<ValidatedProposal, ProposalError> {
        if self.evidence.is_empty() {
            return Err(ProposalError::InvalidEvidence);
        }
        if self.id.trim().is_empty() || !self.kind.has_valid_fields() {
            return Err(ProposalError::InvalidAction);
        }
        let evidence = self
            .evidence
            .into_iter()
            .map(|citation| {
                let origin = ledger.validate(&citation)?;
                Ok(ValidatedCitation {
                    id: citation.id,
                    quote: citation.quote,
                    origin,
                })
            })
            .collect::<Result<Vec<_>, ProposalError>>()?;
        if evidence
            .iter()
            .all(|item| item.origin == EvidenceOrigin::PlayerMessage)
        {
            return Err(ProposalError::InvalidEvidence);
        }
        Ok(ValidatedProposal {
            id: self.id,
            kind: self.kind,
            evidence,
        })
    }
}

impl ProposalKind {
    fn has_valid_fields(&self) -> bool {
        let filled = |value: &str| !value.trim().is_empty();
        match self {
            Self::AddMod { source, project_id } => {
                matches!(source.as_str(), "modrinth" | "curseforge") && filled(project_id)
            }
            Self::RemoveMod { mod_id } => filled(mod_id),
            Self::UpdateMod { mod_id, version_id } => filled(mod_id) && filled(version_id),
            Self::ChangeVersion { minecraft, loader } => filled(minecraft) && filled(loader),
            Self::SetSide { mod_id, side } => {
                filled(mod_id) && matches!(side.as_str(), "client" | "server" | "both")
            }
            Self::EditConfig { path, key, .. } => {
                filled(key)
                    && !key.contains(['\n', '\r'])
                    && !path.contains(['\\', ':', '\0'])
                    && !path.starts_with('/')
                    && path
                        .split('/')
                        .all(|part| !part.is_empty() && part != "." && part != "..")
            }
            Self::TestSettings { .. } | Self::RunTest | Self::StartBisect => true,
        }
    }

    /// Se a ação exige um ponto de segurança antes da gravação.
    #[must_use]
    pub fn changes_pack(&self) -> bool {
        matches!(
            self,
            Self::AddMod { .. }
                | Self::RemoveMod { .. }
                | Self::UpdateMod { .. }
                | Self::ChangeVersion { .. }
                | Self::SetSide { .. }
                | Self::EditConfig { .. }
        )
    }
}

/// Prévia produzida pelo mesmo serviço de adição da interface, com dependências resolvidas.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdditionPreview {
    /// Referência opaca para o plano de adição do serviço normal.
    pub plan_id: String,
    /// Mods que o serviço instalará, incluindo dependências.
    pub items: Vec<String>,
}

/// Prévia produzida pelo serviço normal de configurações.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfigPreview {
    /// Hash do conteúdo lido, conferido pelo `config_write`.
    pub content_hash: String,
    /// Diferença exata que a interface deve mostrar antes de Aplicar.
    pub diff: String,
    /// Texto completo com somente a chave pedida alterada; entregue ao `config_write` normal.
    pub new_text: String,
}

/// Dados exibidos no cartão. `pack_hash` é conferido de novo ao aplicar.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PreparedPlan {
    /// A proposta validada.
    pub proposal: ValidatedProposal,
    /// Hash da árvore do pack quando a prévia foi criada.
    pub pack_hash: String,
    /// Prévia do planejador de dependências, quando pertinente.
    pub addition: Option<AdditionPreview>,
    /// Diferença da configuração, quando pertinente.
    pub config: Option<ConfigPreview>,
}

/// Confirmação explícita do cartão mostrado ao usuário.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserApproval {
    /// Identificador do cartão aprovado.
    pub proposal_id: String,
    /// Hash da árvore mostrado na prévia.
    pub pack_hash: String,
    /// Diferença exata que o usuário viu, quando há edição de config.
    pub config_diff: Option<String>,
    /// Lista exata de itens mostrada para adição, incluindo dependências.
    pub addition_items: Option<Vec<String>>,
}

/// Adaptador da `warden-app` aos serviços que já atendem os fluxos normais da interface.
/// Os métodos de gravação devem manter a própria verificação de concorrência e reversão.
#[async_trait]
pub trait ProposalServices: Send + Sync {
    /// Hash atual da árvore do pack.
    async fn pack_hash(&self) -> Result<String, ProposalError>;
    /// Mesmo planejador de adição usado pela interface, incluindo dependências.
    async fn plan_add_mod(
        &self,
        source: &str,
        project_id: &str,
    ) -> Result<AdditionPreview, ProposalError>;
    /// Mesma prévia de diferença usada antes de `config_write`.
    async fn preview_config(
        &self,
        path: &str,
        key: &str,
        value: &serde_json::Value,
    ) -> Result<ConfigPreview, ProposalError>;
    /// Cria ponto de segurança antes de qualquer alteração do pack.
    async fn safety_point(&self) -> Result<(), ProposalError>;
    /// Aplica o plano de adição já resolvido pelo serviço normal.
    async fn apply_add_mod(&self, plan_id: &str) -> Result<(), ProposalError>;
    /// Remove um mod pelo serviço normal, com suas regras de dependências.
    async fn remove_mod(&self, mod_id: &str) -> Result<(), ProposalError>;
    /// Atualiza um mod pelo serviço normal.
    async fn update_mod(&self, mod_id: &str, version_id: &str) -> Result<(), ProposalError>;
    /// Muda a versão do pack pelo serviço normal.
    async fn change_version(&self, minecraft: &str, loader: &str) -> Result<(), ProposalError>;
    /// Muda o lado pelo serviço normal.
    async fn set_side(&self, mod_id: &str, side: &str) -> Result<(), ProposalError>;
    /// Executa `config_write` com o hash do conteúdo lido para a prévia.
    async fn config_write(
        &self,
        path: &str,
        text: &str,
        content_hash: &str,
    ) -> Result<(), ProposalError>;
    /// Usa o fluxo normal de ajustes do teste.
    async fn test_settings(
        &self,
        memory_mib: Option<u32>,
        java: Option<&str>,
        jvm_args: &[String],
    ) -> Result<(), ProposalError>;
    /// Usa o comando normal de executar teste.
    async fn run_test(&self) -> Result<(), ProposalError>;
    /// Usa o fluxo normal de busca do culpado quando a D-12 chegar ao main.
    async fn start_bisect(&self) -> Result<(), ProposalError>;
}

impl ValidatedProposal {
    /// Consulta os serviços de prévia sem escrever no pack.
    pub async fn prepare<S: ProposalServices>(
        self,
        services: &S,
    ) -> Result<PreparedPlan, ProposalError> {
        let pack_hash = services.pack_hash().await?;
        let addition = match &self.kind {
            ProposalKind::AddMod { source, project_id } => {
                Some(services.plan_add_mod(source, project_id).await?)
            }
            _ => None,
        };
        let config = match &self.kind {
            ProposalKind::EditConfig { path, key, value } => {
                let preview = services.preview_config(path, key, value).await?;
                if preview.diff.trim().is_empty()
                    || preview.content_hash.is_empty()
                    || preview.new_text.is_empty()
                {
                    return Err(ProposalError::InvalidAction);
                }
                Some(preview)
            }
            _ => None,
        };
        Ok(PreparedPlan {
            proposal: self,
            pack_hash,
            addition,
            config,
        })
    }
}

impl PreparedPlan {
    /// Aplica só após o clique em Aplicar, com a mesma prévia e a árvore ainda intacta.
    pub async fn apply<S: ProposalServices>(
        self,
        approval: &UserApproval,
        services: &S,
    ) -> Result<(), ProposalError> {
        if approval.proposal_id != self.proposal.id
            || approval.pack_hash != self.pack_hash
            || approval.config_diff.as_deref()
                != self.config.as_ref().map(|preview| preview.diff.as_str())
            || approval.addition_items.as_ref()
                != self.addition.as_ref().map(|preview| &preview.items)
        {
            return Err(ProposalError::ApprovalMismatch);
        }
        if services.pack_hash().await? != self.pack_hash {
            return Err(ProposalError::StalePreview);
        }
        if self.proposal.kind.changes_pack() {
            services.safety_point().await?;
        }
        match self.proposal.kind {
            ProposalKind::AddMod { .. } => {
                let plan = self.addition.ok_or(ProposalError::InvalidAction)?;
                services.apply_add_mod(&plan.plan_id).await
            }
            ProposalKind::RemoveMod { mod_id } => services.remove_mod(&mod_id).await,
            ProposalKind::UpdateMod { mod_id, version_id } => {
                services.update_mod(&mod_id, &version_id).await
            }
            ProposalKind::ChangeVersion { minecraft, loader } => {
                services.change_version(&minecraft, &loader).await
            }
            ProposalKind::SetSide { mod_id, side } => services.set_side(&mod_id, &side).await,
            ProposalKind::EditConfig { path, .. } => {
                let preview = self.config.ok_or(ProposalError::InvalidAction)?;
                services
                    .config_write(&path, &preview.new_text, &preview.content_hash)
                    .await
            }
            ProposalKind::TestSettings {
                memory_mib,
                java,
                jvm_args,
            } => {
                services
                    .test_settings(memory_mib, java.as_deref(), &jvm_args)
                    .await
            }
            ProposalKind::RunTest => services.run_test().await,
            ProposalKind::StartBisect => services.start_bisect().await,
        }
    }
}

#[cfg(test)]
mod tests;
