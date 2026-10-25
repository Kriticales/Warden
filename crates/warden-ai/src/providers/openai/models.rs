//! Escolha do modelo da OpenAI pela lista da chave (`GET v1/models`; decisão do dono de
//! 18/10/2026, ADR-0060).
//!
//! Nenhum nome fica no código, só a regra da família: "Procurar rápido" usa o `gpt-N-luna` e
//! "Procurar a fundo" usa o `gpt-N-sol`, com **N a maior geração inteira em que os dois
//! existem** na lista da chave. Fora: versões quebradas (`gpt-6.1-sol`, `gpt-5.6-luna`), o
//! `astra` (equivale ao mais caro), `pro`, `codex`, `chat-latest`, `mini`, `nano` e qualquer
//! sufixo (`gpt-6-luna-2026-09-01`): só a forma exata `gpt-<inteiro>-luna|sol` entra. Sem o par,
//! o erro é [`Error::ProviderNoModel`], sem chutar outro modelo.

use std::collections::BTreeMap;

use serde_json::Value;

use super::{OpenAiClient, Problem, api_message, classify};
use crate::error::{Error, Result};
use crate::providers::{AiProvider, ModelChoices, SearchMode};

/// Lê `gpt-<N>-luna` e `gpt-<N>-sol` (N inteiro, sem ponto nem sufixo).
fn family(name: &str) -> Option<(u32, SearchMode)> {
    let rest = name.strip_prefix("gpt-")?;
    let (generation, kind) = rest.split_once('-')?;
    if generation.is_empty() || !generation.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    let mode = match kind {
        "luna" => SearchMode::Quick,
        "sol" => SearchMode::Deep,
        _ => return None,
    };
    Some((generation.parse().ok()?, mode))
}

/// O modo de um nome de modelo (`None`: não é um `gpt-N-luna` nem um `gpt-N-sol`).
#[must_use]
pub fn mode_of(model: &str) -> Option<SearchMode> {
    family(model).map(|(_, mode)| mode)
}

/// O par da maior geração inteira com `luna` e `sol` (`None`: nenhuma geração tem os dois).
#[must_use]
pub fn resolve<'a>(ids: impl IntoIterator<Item = &'a str>) -> Option<ModelChoices> {
    let mut generations: BTreeMap<u32, (bool, bool)> = BTreeMap::new();
    for id in ids {
        if let Some((generation, mode)) = family(id) {
            let slot = generations.entry(generation).or_default();
            match mode {
                SearchMode::Quick => slot.0 = true,
                SearchMode::Deep => slot.1 = true,
            }
        }
    }
    let (generation, _) = generations
        .iter()
        .rev()
        .find(|(_, (luna, sol))| *luna && *sol)?;
    Some(ModelChoices {
        economical: Some(format!("gpt-{generation}-luna")),
        careful: Some(format!("gpt-{generation}-sol")),
        // Sem troca de modelo quando a cota acaba (só o Gemini tem, ADR-0060).
        lighter: None,
    })
}

/// `GET v1/models` da chave e a escolha. Chave recusada vira [`Error::KeyInvalid`]; sem o par
/// da regra, [`Error::ProviderNoModel`].
pub async fn fetch_choices(client: &OpenAiClient) -> Result<ModelChoices> {
    let reply = client.list_models().await?;
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
    let ids = json
        .get("data")
        .and_then(Value::as_array)
        .map_or(&[][..], Vec::as_slice)
        .iter()
        .filter_map(|model| model.get("id").and_then(Value::as_str));
    resolve(ids).ok_or(Error::ProviderNoModel {
        provider: AiProvider::Openai,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A lista real da chave do dono em 18/10/2026 (entre outros modelos).
    const REAL_2026_10_18: [&str; 10] = [
        "gpt-5.5",
        "gpt-5.6-luna",
        "gpt-5.6-sol",
        "gpt-5.6-terra",
        "gpt-6-astra",
        "gpt-6-luna",
        "gpt-6-sol",
        "gpt-6.1-sol",
        "o3",
        "o4-mini",
    ];

    #[test]
    fn lista_real_escolhe_gpt_6_luna_e_gpt_6_sol() {
        let choices = resolve(REAL_2026_10_18).unwrap();
        assert_eq!(choices.economical.as_deref(), Some("gpt-6-luna"));
        assert_eq!(choices.careful.as_deref(), Some("gpt-6-sol"));
    }

    #[test]
    fn geracao_nova_entra_sozinha_so_com_o_par() {
        // Só o sol da 7 ainda: fica a 6.
        let mut list = REAL_2026_10_18.to_vec();
        list.push("gpt-7-sol");
        assert_eq!(
            resolve(list.clone()).unwrap().careful.as_deref(),
            Some("gpt-6-sol")
        );
        // Com o par da 7, a 7.
        list.push("gpt-7-luna");
        let choices = resolve(list).unwrap();
        assert_eq!(choices.economical.as_deref(), Some("gpt-7-luna"));
        assert_eq!(choices.careful.as_deref(), Some("gpt-7-sol"));
        // Comparação por número: 10 > 9.
        let choices = resolve(["gpt-9-luna", "gpt-9-sol", "gpt-10-luna", "gpt-10-sol"]).unwrap();
        assert_eq!(choices.careful.as_deref(), Some("gpt-10-sol"));
    }

    #[test]
    fn nunca_quebradas_astra_pro_codex_mini_nano_nem_sufixo() {
        for name in [
            "gpt-6.1-sol",
            "gpt-5.6-luna",
            "gpt-6-astra",
            "gpt-6-pro",
            "gpt-6-codex",
            "gpt-6-chat-latest",
            "gpt-6-mini",
            "gpt-6-nano",
            "gpt-6-luna-mini",
            "gpt-6-sol-pro",
            "gpt-6-luna-2026-09-01",
            "gpt--luna",
            "gpt-x-sol",
            "o3",
        ] {
            assert_eq!(mode_of(name), None, "{name}");
        }
        assert_eq!(mode_of("gpt-6-luna"), Some(SearchMode::Quick));
        assert_eq!(mode_of("gpt-6-sol"), Some(SearchMode::Deep));
    }

    #[test]
    fn sem_o_par_nao_chuta() {
        assert_eq!(
            resolve(["gpt-6.1-sol", "gpt-6-astra", "gpt-5.6-luna", "o3"]),
            None
        );
        assert_eq!(resolve(["gpt-6-luna"]), None);
        assert_eq!(resolve(std::iter::empty()), None);
    }
}
