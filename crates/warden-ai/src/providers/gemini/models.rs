//! Escolha do modelo pelo `models.list` da conta (ADR-0055; ARCHITECTURE §9.5 "Modelo").
//!
//! Nenhum nome de modelo fica no código. As duas opções ("Mais econômico" e "Análise mais
//! cuidadosa") usam o modelo mais forte que o plano gratuito da API permite, o **Flash estável
//! mais novo** que a chave alcança (o Pro não tem plano gratuito: tabela de preços do Google,
//! consultada em 19/10/2026); o **Flash-Lite estável mais novo** é o mais leve, que o laço usa
//! sozinho numa pergunta quando a cota diária do Flash acaba (decisão do dono; ADR-0060,
//! complemento de 19/10/2026). Sem nenhum Flash estável, as duas opções usam o Flash-Lite.
//! "Estável" é o nome sem sufixo: `gemini-<versão>-flash-lite` ou `gemini-<versão>-flash`;
//! `-preview`, `-exp`, `-latest`, números de revisão e os apelidos (`gemini-flash-latest`)
//! ficam de fora. Só entram modelos com `generateContent`.

use serde_json::Value;

use super::{GeminiClient, Problem, api_message, classify};
use crate::error::{Error, Result};
use crate::providers::{ModelChoices, ModelTier};

/// Um modelo estável reconhecido: versão numérica e família.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Stable {
    name: String,
    version: Vec<u32>,
    lite: bool,
}

/// Lê `gemini-<versão>-flash[-lite]`; qualquer outra forma não é "estável" para o Warden.
fn stable(name: &str) -> Option<Stable> {
    let name = name.strip_prefix("models/").unwrap_or(name);
    let rest = name.strip_prefix("gemini-")?;
    let (version, family) = rest.split_once('-')?;
    let lite = match family {
        "flash-lite" => true,
        "flash" => false,
        _ => return None,
    };
    let numbers: Vec<u32> = version
        .split('.')
        .map(|part| part.parse().ok())
        .collect::<Option<_>>()?;
    (!numbers.is_empty()).then(|| Stable {
        name: name.to_owned(),
        version: numbers,
        lite,
    })
}

/// O nível de um nome de modelo (`None`: não é um Flash nem um Flash-Lite estável).
#[must_use]
pub fn tier_of(model: &str) -> Option<ModelTier> {
    stable(model).map(|found| {
        if found.lite {
            ModelTier::Economical
        } else {
            ModelTier::Careful
        }
    })
}

/// Escolhe, entre os modelos da conta, o Flash estável mais novo para as duas opções e o
/// Flash-Lite estável mais novo como o mais leve (ou o Flash-Lite nas duas, sem Flash).
#[must_use]
pub fn resolve(models: &[Value]) -> ModelChoices {
    let mut lite: Option<Stable> = None;
    let mut flash: Option<Stable> = None;
    for model in models {
        let generates = model
            .get("supportedGenerationMethods")
            .and_then(Value::as_array)
            .is_some_and(|methods| {
                methods
                    .iter()
                    .any(|method| method.as_str() == Some("generateContent"))
            });
        let Some(found) = model
            .get("name")
            .and_then(Value::as_str)
            .and_then(stable)
            .filter(|_| generates)
        else {
            continue;
        };
        let slot = if found.lite { &mut lite } else { &mut flash };
        if slot
            .as_ref()
            .is_none_or(|best| found.version > best.version)
        {
            *slot = Some(found);
        }
    }
    let lite = lite.map(|model| model.name);
    match flash.map(|model| model.name) {
        Some(flash) => ModelChoices {
            economical: Some(flash.clone()),
            careful: Some(flash),
            lighter: lite,
        },
        None => ModelChoices {
            economical: lite.clone(),
            careful: lite,
            lighter: None,
        },
    }
}

/// `models.list` da chave (todas as páginas) e a escolha. Chave recusada vira
/// [`Error::KeyInvalid`].
pub async fn fetch_choices(client: &GeminiClient) -> Result<ModelChoices> {
    let mut models = Vec::new();
    let mut token: Option<String> = None;
    for _ in 0..20 {
        let reply = client.list_models_page(token.as_deref()).await?;
        if reply.status != 200 {
            return Err(match classify(reply.status, &reply.body) {
                Problem::KeyInvalid => Error::KeyInvalid {
                    status: reply.status,
                },
                Problem::Billing => Error::Billing {
                    detail: api_message(&reply),
                },
                Problem::Transient { status } => Error::Unavailable { status },
                Problem::PerMinute { .. } => Error::RateLimited { waits: 0 },
                _ => Error::Rejected {
                    status: reply.status,
                    detail: api_message(&reply),
                },
            });
        }
        let json = reply.json();
        if let Some(page) = json.get("models").and_then(Value::as_array) {
            models.extend(page.iter().cloned());
        }
        token = json
            .get("nextPageToken")
            .and_then(Value::as_str)
            .filter(|next| !next.is_empty())
            .map(ToOwned::to_owned);
        if token.is_none() {
            break;
        }
    }
    Ok(resolve(&models))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn model(name: &str) -> Value {
        json!({ "name": format!("models/{name}"), "supportedGenerationMethods": ["generateContent", "countTokens"] })
    }

    #[test]
    fn mais_novo_estavel_de_cada_familia() {
        // A lista do `models.list` da chave no S-R5-4 (§2), em ordem qualquer.
        let list = [
            "gemini-3.1-pro-preview",
            "gemini-3.1-flash-lite",
            "gemini-3.5-flash-lite",
            "gemini-3.1-flash-lite-preview",
            "gemini-flash-lite-latest",
            "gemini-flash-latest",
            "gemini-3.5-flash",
            "gemini-3.8-flash",
            "gemini-3.6-flash",
            "gemini-3-flash-preview",
            "gemini-2.5-flash-lite-001",
            "gemini-3.10-flash-lite-exp",
        ]
        .map(model);
        let choices = resolve(&list);
        // As duas opções usam o mais forte do plano gratuito; o Flash-Lite é o mais leve.
        assert_eq!(choices.economical.as_deref(), Some("gemini-3.8-flash"));
        assert_eq!(choices.careful.as_deref(), Some("gemini-3.8-flash"));
        assert_eq!(choices.lighter.as_deref(), Some("gemini-3.5-flash-lite"));
    }

    #[test]
    fn sem_flash_as_duas_opcoes_usam_o_flash_lite_sem_troca() {
        let choices = resolve(&[model("gemini-3.5-flash-lite"), model("gemini-3.1-pro")]);
        assert_eq!(choices.economical.as_deref(), Some("gemini-3.5-flash-lite"));
        assert_eq!(choices.careful.as_deref(), Some("gemini-3.5-flash-lite"));
        assert_eq!(choices.lighter, None);
        let only_flash = resolve(&[model("gemini-3.8-flash")]);
        assert_eq!(only_flash.economical.as_deref(), Some("gemini-3.8-flash"));
        assert_eq!(only_flash.lighter, None);
    }

    #[test]
    fn versao_comparada_por_numero() {
        let choices = resolve(&[model("gemini-3.9-flash"), model("gemini-3.10-flash")]);
        assert_eq!(choices.careful.as_deref(), Some("gemini-3.10-flash"));
        assert_eq!(choices.lighter, None);
    }

    #[test]
    fn sem_generate_content_nao_entra() {
        let embed = json!({ "name": "models/gemini-9.0-flash", "supportedGenerationMethods": ["embedContent"] });
        assert_eq!(resolve(&[embed]), ModelChoices::default());
    }

    #[test]
    fn nivel_pelo_nome() {
        assert_eq!(
            tier_of("gemini-3.5-flash-lite"),
            Some(ModelTier::Economical)
        );
        assert_eq!(tier_of("gemini-3.8-flash"), Some(ModelTier::Careful));
        assert_eq!(tier_of("gemini-pro-latest"), None);
    }
}
