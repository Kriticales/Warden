//! Pontos de segurança: commits fora da branch com a pasta do pack inteira, referenciados por
//! `refs/warden/safety/<data-hora>-<motivo>` (ARCHITECTURE §11).
//!
//! Criar um ponto de segurança não mexe na branch, nas tags, no índice nem na árvore de
//! trabalho. O commit tem o HEAD como pai (quando existe), para o histórico mostrar de onde o
//! estado partiu, e a mensagem "Ponto de segurança: <motivo>", que a lista devolve.
//!
//! Lista canônica de quando são criados ([`SafetyReason`]): restaurar versão; limpeza de
//! higiene; atualizar todos; remover dois ou mais itens; substituir um item por outro de outra
//! fonte; trazer mudanças do teste que sobrescrevem arquivos do pack. Recuperar um ponto de
//! segurança também cria um, com o estado de antes da recuperação.

use git2::Oid;
use serde::{Deserialize, Serialize};

use crate::error::{Error, GitContext as _, Result};
use crate::moment::Moment;
use crate::repo::{Identity, PackRepo};

/// Prefixo das referências dos pontos de segurança.
pub const SAFETY_REF_PREFIX: &str = "refs/warden/safety/";

/// Começo da mensagem dos commits de ponto de segurança.
const MESSAGE_PREFIX: &str = "Ponto de segurança: ";

/// Por que um ponto de segurança foi criado (lista canônica da ARCHITECTURE §11).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SafetyReason {
    /// Antes de voltar para uma versão salva.
    RestoreVersion {
        /// Versão de destino.
        version: String,
    },
    /// Antes de recuperar outro ponto de segurança.
    RecoverSafetyPoint {
        /// Data e hora do ponto recuperado, como a lista mostra.
        created_at: String,
    },
    /// Antes da limpeza de higiene.
    HygieneCleanup,
    /// Antes de "Atualizar todos".
    UpdateAll,
    /// Antes de remover dois ou mais itens.
    RemoveItems {
        /// Quantos itens.
        count: usize,
    },
    /// Antes de substituir um item por outro de outra fonte.
    ReplaceItem {
        /// Nome do item substituído.
        name: String,
    },
    /// Antes de trazer mudanças do teste que sobrescrevem arquivos do pack.
    BringTestChanges,
}

impl SafetyReason {
    /// Trecho curto e sem acento do nome da referência.
    fn slug(&self) -> String {
        match self {
            Self::RestoreVersion { version } => format!("voltar-{}", ref_safe(version)),
            Self::RecoverSafetyPoint { .. } => "recuperar".to_owned(),
            Self::HygieneCleanup => "limpeza".to_owned(),
            Self::UpdateAll => "atualizar-todos".to_owned(),
            Self::RemoveItems { count } => format!("remover-{count}"),
            Self::ReplaceItem { .. } => "substituir".to_owned(),
            Self::BringTestChanges => "trazer-do-teste".to_owned(),
        }
    }

    /// Motivo como a lista mostra ("antes de voltar para 1.2.0").
    #[must_use]
    pub fn describe(&self) -> String {
        match self {
            Self::RestoreVersion { version } => format!("antes de voltar para {version}"),
            Self::RecoverSafetyPoint { created_at } => {
                format!("antes de recuperar o ponto de segurança de {created_at}")
            }
            Self::HygieneCleanup => "antes da limpeza do pack".to_owned(),
            Self::UpdateAll => "antes de atualizar todos".to_owned(),
            Self::RemoveItems { count } => format!("antes de remover {count} itens"),
            Self::ReplaceItem { name } => format!("antes de substituir {name}"),
            Self::BringTestChanges => "antes de trazer as mudanças do teste".to_owned(),
        }
    }
}

/// Um ponto de segurança gravado.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SafetyPoint {
    /// Nome (a referência sem o prefixo `refs/warden/safety/`).
    pub name: String,
    /// Motivo ("antes de voltar para 1.2.0").
    pub reason: String,
    /// Data e hora da criação, RFC 3339.
    pub created_at: String,
    /// Commit com a pasta do pack, em hexadecimal.
    pub commit: String,
}

impl PackRepo {
    /// Grava um ponto de segurança com a pasta do pack como está agora.
    pub fn create_safety_point(
        &self,
        reason: &SafetyReason,
        identity: &Identity,
        when: Moment,
    ) -> Result<SafetyPoint> {
        let tree_id = self.snapshot_tree()?;
        self.create_safety_point_from_tree(tree_id, reason, identity, when)
    }

    pub(crate) fn create_safety_point_from_tree(
        &self,
        tree_id: Oid,
        reason: &SafetyReason,
        identity: &Identity,
        when: Moment,
    ) -> Result<SafetyPoint> {
        let tree = self
            .repo
            .find_tree(tree_id)
            .ctx("ler a árvore do ponto de segurança")?;
        let signature = identity.signature(when)?;
        let parent = self.head_commit()?;
        let parents: Vec<&git2::Commit<'_>> = parent.iter().collect();
        let description = reason.describe();
        let commit = self
            .repo
            .commit(
                None,
                &signature,
                &signature,
                &format!("{MESSAGE_PREFIX}{description}"),
                &tree,
                &parents,
            )
            .ctx("gravar o ponto de segurança")?;
        let base = format!("{}-{}", when.compact_utc(), reason.slug());
        let mut name = base.clone();
        let mut attempt = 1;
        loop {
            match self.repo.reference(
                &format!("{SAFETY_REF_PREFIX}{name}"),
                commit,
                false,
                "warden: ponto de segurança",
            ) {
                Ok(_) => break,
                Err(error) if error.code() == git2::ErrorCode::Exists && attempt < 1000 => {
                    attempt += 1;
                    name = format!("{base}-{attempt}");
                }
                Err(error) => return Err(Error::git("gravar o ponto de segurança", error)),
            }
        }
        Ok(SafetyPoint {
            name,
            reason: description,
            created_at: when.rfc3339(),
            commit: commit.to_string(),
        })
    }

    /// Pontos de segurança, do mais novo para o mais antigo.
    pub fn safety_points(&self) -> Result<Vec<SafetyPoint>> {
        let mut points = Vec::new();
        let references = self
            .repo
            .references_glob(&format!("{SAFETY_REF_PREFIX}*"))
            .ctx("listar os pontos de segurança")?;
        for reference in references {
            let reference = reference.ctx("listar os pontos de segurança")?;
            let Some(name) = std::str::from_utf8(reference.name_bytes())
                .ok()
                .and_then(|name| name.strip_prefix(SAFETY_REF_PREFIX))
                .map(ToOwned::to_owned)
            else {
                continue;
            };
            let commit = reference
                .peel_to_commit()
                .ctx("ler um ponto de segurança")?;
            let message = String::from_utf8_lossy(commit.message_bytes());
            let reason = message
                .strip_prefix(MESSAGE_PREFIX)
                .unwrap_or(&message)
                .trim_end()
                .to_owned();
            let time = commit.time();
            points.push((
                time.seconds(),
                SafetyPoint {
                    name,
                    reason,
                    created_at: Moment::from_git(time).rfc3339(),
                    commit: commit.id().to_string(),
                },
            ));
        }
        points.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| b.1.name.cmp(&a.1.name)));
        Ok(points.into_iter().map(|(_, point)| point).collect())
    }

    /// Ponto de segurança pelo nome.
    ///
    /// Erros: [`Error::SafetyPointNotFound`].
    pub fn safety_point(&self, name: &str) -> Result<SafetyPoint> {
        self.safety_points()?
            .into_iter()
            .find(|point| point.name == name)
            .ok_or_else(|| Error::SafetyPointNotFound {
                name: name.to_owned(),
            })
    }
}

/// Só letras, números, `.` e `-` (nomes de referência válidos em qualquer plataforma).
fn ref_safe(text: &str) -> String {
    let cleaned: String = text
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '.' || c == '-' {
                c
            } else {
                '-'
            }
        })
        .collect();
    let cleaned = cleaned.trim_matches(['-', '.']).replace("..", ".");
    if cleaned.is_empty() {
        "x".to_owned()
    } else {
        cleaned
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nomes_e_motivos() {
        let cases = [
            (
                SafetyReason::RestoreVersion {
                    version: "1.2.0-beta.1+b".into(),
                },
                "voltar-1.2.0-beta.1-b",
                "antes de voltar para 1.2.0-beta.1+b",
            ),
            (
                SafetyReason::RecoverSafetyPoint {
                    created_at: "2026-10-01T10:00:00-03:00".into(),
                },
                "recuperar",
                "antes de recuperar o ponto de segurança de 2026-10-01T10:00:00-03:00",
            ),
            (
                SafetyReason::HygieneCleanup,
                "limpeza",
                "antes da limpeza do pack",
            ),
            (
                SafetyReason::UpdateAll,
                "atualizar-todos",
                "antes de atualizar todos",
            ),
            (
                SafetyReason::RemoveItems { count: 3 },
                "remover-3",
                "antes de remover 3 itens",
            ),
            (
                SafetyReason::ReplaceItem {
                    name: "Sodium".into(),
                },
                "substituir",
                "antes de substituir Sodium",
            ),
            (
                SafetyReason::BringTestChanges,
                "trazer-do-teste",
                "antes de trazer as mudanças do teste",
            ),
        ];
        for (reason, slug, text) in cases {
            assert_eq!(reason.slug(), slug);
            assert_eq!(reason.describe(), text);
            assert!(git2::Reference::is_valid_name(&format!(
                "{SAFETY_REF_PREFIX}20261001T100000Z-{slug}"
            )));
        }
    }

    #[test]
    fn trecho_seguro_para_referencia() {
        assert_eq!(ref_safe("..a..b.."), "a.b");
        assert_eq!(ref_safe("çã"), "x");
        assert_eq!(ref_safe("1.0 final"), "1.0-final");
    }
}
