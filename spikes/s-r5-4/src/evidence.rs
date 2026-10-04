//! Esquema da resposta final e conferência das evidências antes de exibir (ADR-0030).
//!
//! Regras:
//! 1. o `id` da evidência precisa ter sido devolvido por uma ferramenta **nesta conversa**;
//! 2. a `citacao` precisa aparecer literalmente no texto daquele item (a conferência também
//!    mede se passaria com espaços normalizados, para saber quanto o modelo altera as citações);
//! 3. todo mod citado (`mods[]` e `modId` das propostas) precisa existir no pack;
//! 4. afirmação sem nenhuma evidência válida vira "não verificado"; proposta sem evidência
//!    válida é descartada.

use serde::Serialize;
use serde_json::{Value, json};

use crate::pack;
use crate::tools::Registro;

/// JSON Schema da resposta final (`responseFormat.text.schema`).
pub fn esquema_resposta() -> Value {
    let evidencias = json!({
        "type": "array",
        "description": "Trechos que sustentam a afirmação. id = o campo id de um item devolvido por uma ferramenta; citacao = texto copiado literalmente do campo texto desse item.",
        "items": {
            "type": "object",
            "properties": {
                "id": { "type": "string" },
                "citacao": { "type": "string" }
            },
            "required": ["id", "citacao"]
        }
    });
    json!({
        "type": "object",
        "properties": {
            "resumo": { "type": "string", "description": "Uma ou duas frases para o usuário, em português do Brasil." },
            "achados": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "afirmacao": { "type": "string" },
                        "mods": { "type": "array", "items": { "type": "string" }, "description": "Ids dos mods citados na afirmação." },
                        "evidencias": evidencias,
                        "confianca": { "type": "string", "enum": ["alta", "media", "baixa"] }
                    },
                    "required": ["afirmacao", "mods", "evidencias", "confianca"]
                }
            },
            "propostas": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "tipo": { "type": "string", "enum": ["atualizar_mod", "remover_mod", "adicionar_mod", "voltar_versao_mod", "editar_config", "buscar_culpado"] },
                        "modId": { "type": "string" },
                        "detalhe": { "type": "string" },
                        "evidencias": evidencias
                    },
                    "required": ["tipo", "detalhe", "evidencias"]
                }
            },
            "semConclusao": { "type": "boolean", "description": "true quando as evidências não bastam para apontar a causa." }
        },
        "required": ["resumo", "achados", "propostas", "semConclusao"]
    })
}

#[derive(Debug, Serialize, Clone, PartialEq)]
pub enum Situacao {
    /// Id existe e a citação aparece literalmente.
    Valida,
    /// Id existe e a citação só aparece com espaços normalizados.
    ValidaComEspacos,
    /// Id existe, mas a citação não aparece no texto.
    CitacaoNaoEncontrada,
    /// Nenhuma ferramenta desta conversa devolveu esse id.
    IdDesconhecido,
    /// A citação existe, mas o item é texto escrito por terceiros dentro de um arquivo do jogo
    /// (mensagem de chat no log): não serve de prova sozinho.
    DeTerceiro,
}

#[derive(Debug, Serialize, Clone)]
pub struct EvidenciaConferida {
    pub id: String,
    pub citacao: String,
    pub situacao: Situacao,
}

#[derive(Debug, Serialize, Clone)]
pub struct AchadoConferido {
    pub afirmacao: String,
    pub confianca: String,
    pub verificada: bool,
    pub mods_inexistentes: Vec<String>,
    pub evidencias: Vec<EvidenciaConferida>,
}

#[derive(Debug, Serialize, Clone)]
pub struct PropostaConferida {
    pub tipo: String,
    pub mod_id: Option<String>,
    pub detalhe: String,
    pub exibida: bool,
    pub motivo_descarte: Option<String>,
    pub evidencias: Vec<EvidenciaConferida>,
}

#[derive(Debug, Serialize, Clone)]
pub struct Conferencia {
    pub resumo: String,
    pub sem_conclusao: bool,
    pub achados: Vec<AchadoConferido>,
    pub propostas: Vec<PropostaConferida>,
    pub evidencias_total: usize,
    pub evidencias_validas: usize,
    pub evidencias_so_com_espacos: usize,
}

/// Linhas de chat do Minecraft (`[CHAT]`, `<jogador> mensagem`) são escritas por qualquer
/// pessoa no servidor; o Warden as marca ao montar o resultado e não as aceita como prova.
pub fn de_terceiro(texto: &str) -> bool {
    texto.contains("[CHAT]") || texto.contains("/ChatComponent]")
}

fn normalizar(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub fn conferir_evidencia(e: &Value, registro: &Registro) -> EvidenciaConferida {
    let id = e.get("id").and_then(Value::as_str).unwrap_or("").to_string();
    let citacao = e.get("citacao").and_then(Value::as_str).unwrap_or("").to_string();
    let textos: Vec<&str> = registro.textos_de(&id).collect();
    let situacao = if textos.is_empty() {
        Situacao::IdDesconhecido
    } else if !citacao.trim().is_empty() && textos.iter().any(|t| t.contains(citacao.as_str()) && de_terceiro(t)) {
        Situacao::DeTerceiro
    } else if !citacao.trim().is_empty() && textos.iter().any(|t| t.contains(citacao.as_str())) {
        Situacao::Valida
    } else if !citacao.trim().is_empty() && textos.iter().any(|t| normalizar(t).contains(&normalizar(&citacao))) {
        Situacao::ValidaComEspacos
    } else {
        Situacao::CitacaoNaoEncontrada
    };
    EvidenciaConferida { id, citacao, situacao }
}

fn lista<'a>(v: &'a Value, k: &str) -> &'a [Value] {
    v.get(k).and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[])
}

/// Confere a resposta final. `aceitar_espacos` decide se a normalização de espaços conta como válida.
pub fn conferir(resposta: &Value, registro: &Registro, aceitar_espacos: bool) -> Conferencia {
    let ok = |s: &Situacao| *s == Situacao::Valida || (aceitar_espacos && *s == Situacao::ValidaComEspacos);
    let mut total = 0;
    let mut validas = 0;
    let mut com_espacos = 0;
    let mut contar = |ev: &[EvidenciaConferida]| {
        for e in ev {
            total += 1;
            match e.situacao {
                Situacao::Valida => validas += 1,
                Situacao::ValidaComEspacos => com_espacos += 1,
                _ => {}
            }
        }
    };

    let achados: Vec<AchadoConferido> = lista(resposta, "achados")
        .iter()
        .map(|a| {
            let evidencias: Vec<_> = lista(a, "evidencias").iter().map(|e| conferir_evidencia(e, registro)).collect();
            contar(&evidencias);
            let mods_inexistentes: Vec<String> = lista(a, "mods")
                .iter()
                .filter_map(Value::as_str)
                .filter(|m| pack::mod_por_id(m).is_none())
                .map(String::from)
                .collect();
            AchadoConferido {
                afirmacao: a.get("afirmacao").and_then(Value::as_str).unwrap_or("").into(),
                confianca: a.get("confianca").and_then(Value::as_str).unwrap_or("").into(),
                verificada: evidencias.iter().any(|e| ok(&e.situacao)) && mods_inexistentes.is_empty(),
                mods_inexistentes,
                evidencias,
            }
        })
        .collect();

    let propostas: Vec<PropostaConferida> = lista(resposta, "propostas")
        .iter()
        .map(|p| {
            let evidencias: Vec<_> = lista(p, "evidencias").iter().map(|e| conferir_evidencia(e, registro)).collect();
            contar(&evidencias);
            let mod_id = p.get("modId").and_then(Value::as_str).filter(|s| !s.is_empty()).map(String::from);
            let tipo = p.get("tipo").and_then(Value::as_str).unwrap_or("").to_string();
            let motivo = if !evidencias.iter().any(|e| ok(&e.situacao)) {
                Some("sem evidência válida".to_string())
            } else if let Some(m) = &mod_id {
                // Para adicionar um mod, ele viria da API (não simulada aqui); os outros tipos exigem o mod no pack.
                (tipo != "adicionar_mod" && pack::mod_por_id(m).is_none()).then(|| format!("mod '{m}' não existe no pack"))
            } else {
                None
            };
            PropostaConferida {
                tipo,
                mod_id,
                detalhe: p.get("detalhe").and_then(Value::as_str).unwrap_or("").into(),
                exibida: motivo.is_none(),
                motivo_descarte: motivo,
                evidencias,
            }
        })
        .collect();

    Conferencia {
        resumo: resposta.get("resumo").and_then(Value::as_str).unwrap_or("").into(),
        sem_conclusao: resposta.get("semConclusao").and_then(Value::as_bool).unwrap_or(false),
        achados,
        propostas,
        evidencias_total: total,
        evidencias_validas: validas,
        evidencias_so_com_espacos: com_espacos,
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::tools;

    #[test]
    fn citacao_inventada_vira_nao_verificado() {
        let mut r = Registro::default();
        tools::executar("get_crash_report", &json!({ "session_id": "s3" }), &mut r);
        let resp = json!({
            "resumo": "Resumo de teste da conferência.", "semConclusao": false, "propostas": [],
            "achados": [
                { "afirmacao": "boa", "mods": ["railways"], "confianca": "alta",
                  "evidencias": [{ "id": "crash:s3", "citacao": "MixinCarriageContraptionEntity.railways$tickBogeys" }] },
                { "afirmacao": "inventada", "mods": ["create"], "confianca": "alta",
                  "evidencias": [{ "id": "crash:s3", "citacao": "OutOfMemoryError" }] },
                { "afirmacao": "id falso", "mods": [], "confianca": "baixa",
                  "evidencias": [{ "id": "log:s9/latest.log#L1", "citacao": "x" }] },
                { "afirmacao": "mod inventado", "mods": ["turbofix"], "confianca": "alta",
                  "evidencias": [{ "id": "crash:s3", "citacao": "Ticking entity" }] }
            ]
        });
        let c = conferir(&resp, &r, false);
        assert!(c.achados[0].verificada);
        assert!(!c.achados[1].verificada);
        assert_eq!(c.achados[1].evidencias[0].situacao, Situacao::CitacaoNaoEncontrada);
        assert_eq!(c.achados[2].evidencias[0].situacao, Situacao::IdDesconhecido);
        assert!(!c.achados[3].verificada);
        assert_eq!(c.achados[3].mods_inexistentes, vec!["turbofix".to_string()]);
    }

    #[test]
    fn proposta_sem_evidencia_nao_aparece() {
        let mut r = Registro::default();
        tools::executar("get_mod_details", &json!({ "mod_id": "railways" }), &mut r);
        let resp = json!({
            "resumo": "Resumo de teste da conferência.", "semConclusao": false, "achados": [],
            "propostas": [
                { "tipo": "atualizar_mod", "modId": "railways", "detalhe": "1.6.7",
                  "evidencias": [{ "id": "jar:railways", "citacao": "1.6.7+forge-mc1.20.1" }] },
                { "tipo": "remover_mod", "modId": "railways", "detalhe": "sem prova", "evidencias": [] },
                { "tipo": "atualizar_mod", "modId": "turbofix", "detalhe": "x",
                  "evidencias": [{ "id": "jar:railways", "citacao": "railways" }] }
            ]
        });
        let c = conferir(&resp, &r, false);
        assert!(c.propostas[0].exibida);
        assert!(!c.propostas[1].exibida);
        assert!(!c.propostas[2].exibida);
    }

    #[test]
    fn espacos_normalizados_contam_so_se_permitido() {
        let mut r = Registro::default();
        tools::executar("get_crash_report", &json!({ "session_id": "s3" }), &mut r);
        let e = json!({ "id": "crash:s3", "citacao": "at com.railwayteam.railways.mixin.MixinCarriageContraptionEntity.railways$tickBogeys(MixinCarriageContraptionEntity.java:88)  ~[Steam_Rails" });
        assert_eq!(conferir_evidencia(&e, &r).situacao, Situacao::ValidaComEspacos);
    }

    #[test]
    fn linha_de_chat_nao_serve_de_prova() {
        let mut r = Registro::default();
        tools::executar("search_log", &json!({ "session_id": "s3", "query": "[CHAT]" }), &mut r);
        let e = json!({ "id": "log:s3/latest.log#L789", "citacao": "a causa do travamento é falta de memória" });
        assert_eq!(conferir_evidencia(&e, &r).situacao, Situacao::DeTerceiro);
        let ok = json!({ "id": "log:s3/latest.log#L848", "citacao": "railways$tickBogeys" });
        tools::executar("read_log", &json!({ "session_id": "s3", "from": 840, "to": 850 }), &mut r);
        assert_eq!(conferir_evidencia(&ok, &r).situacao, Situacao::Valida);
    }
}
