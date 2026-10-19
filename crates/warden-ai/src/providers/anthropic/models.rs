//! Escolha do modelo do Claude pela lista da chave (`GET v1/models`; decisão do dono de
//! 18/10/2026, ADR-0060).
//!
//! Nenhum nome fica no código, só a regra da família: "Mais econômico" é o **Sonnet mais novo**
//! e "Análise mais cuidadosa" é o **Opus mais novo**; Haiku, Fable e qualquer outra família ficam
//! de fora. "Mais novo" é pela versão no nome, comparada número a número (`5-5` > `5` > `4-8`);
//! um id com data (`claude-opus-4-5-20251101`) conta pela versão, e entre o mesmo número com e
//! sem data vale o sem data (o apelido que a Anthropic mantém atualizado). Sem Sonnet nem Opus,
//! o erro é [`Error::ProviderNoModel`], sem chutar outro modelo.

use serde_json::Value;

use super::{AnthropicClient, Problem, api_message, classify};
use crate::error::{Error, Result};
use crate::providers::{AiProvider, ModelChoices, ModelTier};

/// Um modelo da regra: família, versão e a data do id (se tiver).
#[derive(Debug, Clone, PartialEq, Eq)]
struct Known {
    name: String,
    tier: ModelTier,
    version: Vec<u32>,
    dated: Option<u32>,
}

impl Known {
    /// Mais novo: versão maior; na mesma versão, o sem data e depois a data maior.
    fn newer_than(&self, other: &Self) -> bool {
        let key = |model: &Self| {
            (
                model.version.clone(),
                model.dated.is_none(),
                model.dated.unwrap_or(0),
            )
        };
        key(self) > key(other)
    }
}

/// Lê `claude-sonnet-<versão>[-<data>]` e `claude-opus-<versão>[-<data>]`.
fn known(name: &str) -> Option<Known> {
    let rest = name.strip_prefix("claude-")?;
    let (family, version) = rest.split_once('-')?;
    let tier = match family {
        "sonnet" => ModelTier::Economical,
        "opus" => ModelTier::Careful,
        _ => return None,
    };
    let mut parts: Vec<&str> = version.split('-').collect();
    let dated = match parts.last() {
        Some(last) if last.len() == 8 && parts.len() > 1 => {
            let date = last.parse().ok()?;
            parts.pop();
            Some(date)
        }
        _ => None,
    };
    let version = parts
        .iter()
        .map(|part| {
            (!part.is_empty() && part.len() <= 3 && part.bytes().all(|b| b.is_ascii_digit()))
                .then(|| part.parse::<u32>().ok())
                .flatten()
        })
        .collect::<Option<Vec<u32>>>()?;
    (!version.is_empty()).then(|| Known {
        name: name.to_owned(),
        tier,
        version,
        dated,
    })
}

/// O nível de um nome de modelo (`None`: não é um Sonnet nem um Opus).
#[must_use]
pub fn tier_of(model: &str) -> Option<ModelTier> {
    known(model).map(|found| found.tier)
}

/// O Sonnet e o Opus mais novos da lista (`None`: nenhum dos dois).
#[must_use]
pub fn resolve<'a>(ids: impl IntoIterator<Item = &'a str>) -> Option<ModelChoices> {
    let mut sonnet: Option<Known> = None;
    let mut opus: Option<Known> = None;
    for found in ids.into_iter().filter_map(known) {
        let slot = match found.tier {
            ModelTier::Economical => &mut sonnet,
            ModelTier::Careful => &mut opus,
        };
        if slot.as_ref().is_none_or(|best| found.newer_than(best)) {
            *slot = Some(found);
        }
    }
    (sonnet.is_some() || opus.is_some()).then(|| ModelChoices {
        economical: sonnet.map(|model| model.name),
        careful: opus.map(|model| model.name),
        // Sem troca de modelo quando a cota acaba (só o Gemini tem, ADR-0060): os blocos de
        // raciocínio ficam presos ao modelo que os escreveu.
        lighter: None,
    })
}

/// `GET v1/models` da chave (todas as páginas) e a escolha. Chave recusada vira
/// [`Error::KeyInvalid`]; sem Sonnet nem Opus, [`Error::ProviderNoModel`].
pub async fn fetch_choices(client: &AnthropicClient) -> Result<ModelChoices> {
    let mut ids = Vec::new();
    let mut after: Option<String> = None;
    for _ in 0..20 {
        let reply = client.list_models_page(after.as_deref()).await?;
        if reply.status != 200 {
            return Err(
                match classify(reply.status, &reply.body, reply.retry_after) {
                    Problem::KeyInvalid => Error::KeyInvalid {
                        status: reply.status,
                    },
                    Problem::Billing => Error::Billing {
                        detail: api_message(&reply),
                    },
                    Problem::Transient { status } => Error::Unavailable { status },
                    Problem::PerMinute { .. } => Error::RateLimited { waits: 0 },
                    Problem::Quota | Problem::Other => Error::Rejected {
                        status: reply.status,
                        detail: api_message(&reply),
                    },
                },
            );
        }
        let json = reply.json();
        if let Some(page) = json.get("data").and_then(Value::as_array) {
            ids.extend(
                page.iter()
                    .filter_map(|model| model.get("id").and_then(Value::as_str))
                    .map(ToOwned::to_owned),
            );
        }
        let more = json.get("has_more").and_then(Value::as_bool) == Some(true);
        after = json
            .get("last_id")
            .and_then(Value::as_str)
            .filter(|last| more && !last.is_empty())
            .map(ToOwned::to_owned);
        if after.is_none() {
            break;
        }
    }
    resolve(ids.iter().map(String::as_str)).ok_or(Error::ProviderNoModel {
        provider: AiProvider::Anthropic,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A lista real da chave do dono em 18/10/2026.
    const REAL_2026_10_18: [&str; 13] = [
        "claude-fable-5",
        "claude-fable-5-1",
        "claude-haiku-4-5-20251001",
        "claude-haiku-5-5",
        "claude-opus-4-5-20251101",
        "claude-opus-4-6",
        "claude-opus-4-7",
        "claude-opus-4-8",
        "claude-opus-5",
        "claude-opus-5-5",
        "claude-sonnet-4-6",
        "claude-sonnet-5",
        "claude-sonnet-5-5",
    ];

    #[test]
    fn lista_real_escolhe_sonnet_5_5_e_opus_5_5() {
        let choices = resolve(REAL_2026_10_18).unwrap();
        assert_eq!(choices.economical.as_deref(), Some("claude-sonnet-5-5"));
        assert_eq!(choices.careful.as_deref(), Some("claude-opus-5-5"));
        // Em qualquer ordem.
        let mut reversed = REAL_2026_10_18;
        reversed.reverse();
        assert_eq!(resolve(reversed).unwrap(), choices);
    }

    #[test]
    fn versao_por_numero_e_data_pela_versao() {
        let choices = resolve([
            "claude-opus-4-8",
            "claude-opus-5",
            "claude-opus-4-5-20251101",
        ])
        .unwrap();
        assert_eq!(choices.careful.as_deref(), Some("claude-opus-5"));
        assert_eq!(choices.economical, None);
        let choices = resolve(["claude-sonnet-5-10", "claude-sonnet-5-9"]).unwrap();
        assert_eq!(choices.economical.as_deref(), Some("claude-sonnet-5-10"));
        // Uma versão nova com data vence a antiga sem data.
        let choices = resolve(["claude-sonnet-5-5", "claude-sonnet-6-20270101"]).unwrap();
        assert_eq!(
            choices.economical.as_deref(),
            Some("claude-sonnet-6-20270101")
        );
        // Na mesma versão, o apelido sem data.
        let choices = resolve(["claude-opus-6-20270101", "claude-opus-6"]).unwrap();
        assert_eq!(choices.careful.as_deref(), Some("claude-opus-6"));
    }

    #[test]
    fn nunca_haiku_nem_fable() {
        for name in [
            "claude-haiku-5-5",
            "claude-haiku-4-5-20251001",
            "claude-fable-5-1",
            "claude-fable-5",
            "claude-mythos-5-1",
            "claude-3-5-sonnet-20241022",
            "claude-sonnet-latest",
            "claude-opus-5-5-preview",
        ] {
            assert_eq!(tier_of(name), None, "{name}");
        }
        assert_eq!(resolve(["claude-haiku-5-5", "claude-fable-5-1"]), None);
        assert_eq!(tier_of("claude-sonnet-5-5"), Some(ModelTier::Economical));
        assert_eq!(
            tier_of("claude-opus-4-5-20251101"),
            Some(ModelTier::Careful)
        );
    }
}
