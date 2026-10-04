//! Testes do laço contra o servidor simulado (sem rede externa).

use std::time::Duration;

use serde_json::{Value, json};

use crate::laco::{Config, Conversa, ModoFinal};
use crate::mock::{self, CHAVE_FALSA, Roteiro, Servidor};

fn cfg(url: &str) -> Config {
    Config {
        base_url: url.into(),
        modelo: "simulado".into(),
        chave: CHAVE_FALSA.into(),
        modo_final: ModoFinal::Junto,
        max_rodadas: 8,
        thinking_level: None,
        escalonar_ferramentas: true,
        timeout: Duration::from_secs(10),
        modo_chamada: "VALIDATED".into(),
        instrucao_extra: None,
        ferramentas_extras: false,
    }
}

fn final_simples() -> Value {
    mock::final_json(&json!({
        "resumo": "O Rails 1.6.4 quebra com o Create 6.", "semConclusao": false, "propostas": [],
        "achados": [{ "afirmacao": "a", "mods": ["railways"], "confianca": "alta",
            "evidencias": [{ "id": "crash:s3", "citacao": "railways$tickBogeys" }] }]
    }))
}

#[test]
fn paralelas_malformada_e_final() {
    let roteiro: Roteiro = Box::new(|i, _| match i {
        0 => mock::chamadas(&[("get_findings", json!({})), ("get_crash_report", json!({ "session_id": "s3" }))]),
        1 => mock::malformada(),
        2 => mock::chamadas(&[("get_mod_details", json!({ "mod_id": "railways" }))]),
        _ => final_simples(),
    });
    let srv = Servidor::iniciar(roteiro);
    let mut c = Conversa::nova(cfg(&srv.url));
    let r = c.perguntar("Por que travou?").expect("laço");
    assert!(srv.recusas().is_empty(), "{:?}", srv.recusas());
    assert_eq!(r.rodadas, 2);
    assert!(r.conferencia.achados[0].verificada);
    assert_eq!(c.eventos.iter().filter(|e| e.finish_reason == "MALFORMED_FUNCTION_CALL").count(), 1);
    // A primeira rodada só libera as ferramentas iniciais; depois do crash report, todas.
    let reqs = srv.requisicoes();
    assert_eq!(reqs[0].pointer("/toolConfig/functionCallingConfig/allowedFunctionNames").unwrap().as_array().unwrap().len(), 11);
    assert_eq!(reqs[2].pointer("/toolConfig/functionCallingConfig/allowedFunctionNames").unwrap().as_array().unwrap().len(), 17);
    assert_eq!(reqs[0].pointer("/toolConfig/functionCallingConfig/mode").unwrap(), "VALIDATED");
}

#[test]
fn servidor_recusa_assinatura_removida() {
    let emitida = mock::chamadas(&[("get_findings", json!({})), ("list_sessions", json!({}))]);
    let mut content = emitida["candidates"][0]["content"].clone();
    let emitidos = vec![content.clone()];
    let user = json!({ "role": "user", "parts": [{ "text": "oi" }] });
    let respostas = json!({ "role": "user", "parts": content["parts"].as_array().unwrap().iter().map(|p| json!({ "functionResponse": { "id": p["functionCall"]["id"], "name": p["functionCall"]["name"], "response": {} } })).collect::<Vec<_>>() });
    assert!(mock::validar(&json!({ "contents": [user, content, respostas] }), &emitidos).is_ok());
    content["parts"][0].as_object_mut().unwrap().remove("thoughtSignature");
    let e = mock::validar(&json!({ "contents": [user, content, respostas] }), &emitidos).unwrap_err();
    assert!(e.contains("missing a thought_signature"), "{e}");
}

#[test]
fn servidor_recusa_intercalado() {
    let emitida = mock::chamadas(&[("get_findings", json!({})), ("list_sessions", json!({}))]);
    let content = emitida["candidates"][0]["content"].clone();
    let ps = content["parts"].as_array().unwrap();
    let user = json!({ "role": "user", "parts": [{ "text": "oi" }] });
    let fr = |p: &Value| json!({ "role": "user", "parts": [{ "functionResponse": { "id": p["functionCall"]["id"], "name": p["functionCall"]["name"], "response": {} } }] });
    let m1 = json!({ "role": "model", "parts": [ps[0].clone()] });
    let m2 = json!({ "role": "model", "parts": [ps[1].clone()] });
    let r = mock::validar(&json!({ "contents": [user, m1, fr(&ps[0]), m2, fr(&ps[1])] }), &[content.clone()]);
    assert!(r.is_err());
}

#[test]
fn limite_de_rodadas_forca_none() {
    let roteiro: Roteiro = Box::new(|_, req| {
        if req.pointer("/toolConfig/functionCallingConfig/mode").and_then(Value::as_str) == Some("NONE") {
            final_simples()
        } else {
            mock::chamadas(&[("list_mods", json!({}))])
        }
    });
    let srv = Servidor::iniciar(roteiro);
    let mut c = Conversa::nova(cfg(&srv.url));
    let r = c.perguntar("lento").expect("laço");
    assert_eq!(r.rodadas, 8);
    assert_eq!(c.eventos.len(), 9);
    assert_eq!(c.eventos[8].modo, "NONE");
    assert!(srv.recusas().is_empty());
}

#[test]
fn segundo_turno_mantem_historico_intacto() {
    let roteiro: Roteiro = Box::new(|i, _| match i {
        0 => mock::chamadas(&[("get_crash_report", json!({ "session_id": "s3" }))]),
        1 => final_simples(),
        2 => mock::chamadas(&[("diff_versions", json!({ "from": "v1.3", "to": "v1.4" }))]),
        _ => final_simples(),
    });
    let srv = Servidor::iniciar(roteiro);
    let mut c = Conversa::nova(cfg(&srv.url));
    c.perguntar("1").unwrap();
    c.perguntar("2").unwrap();
    assert!(srv.recusas().is_empty(), "{:?}", srv.recusas());
    assert_eq!(c.contents.len(), 8);
}

#[test]
fn modo_separado_faz_rodada_extra() {
    let roteiro: Roteiro = Box::new(|i, req| {
        let tem_formato = req.pointer("/generationConfig/responseFormat").is_some();
        match i {
            0 => {
                assert!(!tem_formato);
                mock::chamadas(&[("get_crash_report", json!({ "session_id": "s3" }))])
            }
            1 => json!({ "candidates": [{ "content": { "role": "model", "parts": [{ "text": "É o Rails." }] }, "finishReason": "STOP" }] }),
            _ => {
                assert!(tem_formato);
                final_simples()
            }
        }
    });
    let srv = Servidor::iniciar(roteiro);
    let mut c = cfg(&srv.url);
    c.modo_final = ModoFinal::Separado;
    let mut conv = Conversa::nova(c);
    conv.perguntar("x").unwrap();
    assert_eq!(conv.eventos.len(), 3);
    assert_eq!(conv.eventos[2].modo, "NONE");
}

#[test]
fn chave_errada_da_403_sem_vazar() {
    let srv = Servidor::iniciar(Box::new(|_, _| final_simples()));
    let mut c = cfg(&srv.url);
    c.chave = "outra".into();
    let mut conv = Conversa::nova(c);
    let e = conv.perguntar("x").unwrap_err().to_string();
    assert!(e.contains("403"));
    assert!(!e.contains("outra"));
}

#[test]
fn modo_ferramenta_encerra_com_responder() {
    let roteiro: Roteiro = Box::new(|i, req| {
        let nomes = req.pointer("/tools/0/functionDeclarations").unwrap().as_array().unwrap().len();
        assert_eq!(nomes, 18);
        match i {
            0 => mock::chamadas(&[("get_crash_report", json!({ "session_id": "s3" }))]),
            1 => mock::chamadas(&[("responder", json!({ "resumo": "O Rails 1.6.4 quebra com o Create 6.", "semConclusao": false, "propostas": [],
                "achados": [{ "afirmacao": "a", "mods": ["railways"], "confianca": "alta",
                    "evidencias": [{ "id": "crash:s3", "citacao": "railways$tickBogeys" }] }] }))]),
            2 => mock::chamadas(&[("diff_versions", json!({ "from": "v1.3", "to": "v1.4" }))]),
            _ => mock::chamadas(&[("responder", json!({ "resumo": "Sem conclusão nesta pergunta.", "semConclusao": true, "propostas": [], "achados": [] }))]),
        }
    });
    let srv = Servidor::iniciar(roteiro);
    let mut c = cfg(&srv.url);
    c.modo_final = ModoFinal::Ferramenta;
    let mut conv = Conversa::nova(c);
    let r = conv.perguntar("x").unwrap();
    assert!(r.conferencia.achados[0].verificada);
    conv.perguntar("y").unwrap();
    assert!(srv.recusas().is_empty(), "{:?}", srv.recusas());
    assert_eq!(conv.eventos.len(), 4);
}

fn erros_reais() -> Value {
    serde_json::from_str(include_str!("../fixtures/erros-http-reais.json")).unwrap()
}

#[test]
fn erro_503_real_e_recuperado() {
    let e = erros_reais()["http_503_alta_demanda"].clone();
    let roteiro: Roteiro = Box::new(move |i, _| match i {
        0 => e.clone(),
        _ => final_simples(),
    });
    let srv = Servidor::iniciar(roteiro);
    let mut c = Conversa::nova(cfg(&srv.url));
    c.registro.itens.push(crate::tools::ItemEnviado { id: "crash:s3".into(), texto: "railways$tickBogeys".into() });
    let r = c.perguntar("x").unwrap();
    assert!(r.conferencia.achados[0].verificada);
    assert_eq!(c.eventos[0].http, 503);
}

#[test]
fn cota_por_minuto_le_espera() {
    let e = erros_reais()["http_429_cota_por_minuto_plano_gratuito"].to_string();
    assert!(crate::laco::cota_esgotada(&e).is_none());
    let d = erros_reais()["http_429_cota_diaria_plano_gratuito"].to_string();
    assert!(crate::laco::cota_esgotada(&d).is_some());
}

#[test]
fn cota_diaria_real_para_na_hora() {
    let e = erros_reais()["http_429_cota_diaria_plano_gratuito"].clone();
    let srv = Servidor::iniciar(Box::new(move |_, _| e.clone()));
    let mut c = Conversa::nova(cfg(&srv.url));
    let inicio = std::time::Instant::now();
    match c.perguntar("x") {
        Err(crate::laco::ErroLaco::Cota { por_dia, plano_gratuito, espera_s }) => {
            assert!(por_dia && plano_gratuito && espera_s > 3600);
        }
        outro => panic!("{outro:?}"),
    }
    assert_eq!(c.eventos.len(), 1);
    assert!(inicio.elapsed() < Duration::from_secs(2));
}

#[test]
fn gravacoes_reais_passam_no_simulado() {
    for nome in ["travamento-gemini-3.8-flash.json", "travamento-gemini-3.8-flash-ferramenta.json"] {
        let f: Value = serde_json::from_str(&std::fs::read_to_string(format!("{}/fixtures/{nome}", env!("CARGO_MANIFEST_DIR"))).unwrap()).unwrap();
        let respostas = f["respostas"].as_array().unwrap().clone();
        let srv = Servidor::iniciar(mock::roteiro_de_gravacao(respostas));
        let mut c = cfg(&srv.url);
        if nome.contains("ferramenta") {
            c.modo_final = ModoFinal::Ferramenta;
        }
        let mut conv = Conversa::nova(c);
        let r = conv.perguntar(f["perguntas"][0].as_str().unwrap()).unwrap();
        assert!(srv.recusas().is_empty(), "{nome}: {:?}", srv.recusas());
        assert_eq!(r.conferencia.evidencias_validas, r.conferencia.evidencias_total, "{nome}");
        assert!(r.conferencia.evidencias_total >= 6);
    }
}

#[test]
fn resposta_vazia_e_json_degenerado_sao_repetidos() {
    let roteiro: Roteiro = Box::new(|i, _| match i {
        0 => mock::chamadas(&[("get_crash_report", json!({ "session_id": "s3" }))]),
        1 => json!({ "candidates": [{ "content": { "role": "model", "parts": [{ "text": "" }] }, "finishReason": "STOP" }] }),
        2 => mock::final_json(&json!({ "resumo": "...", "achados": [], "propostas": [], "semConclusao": false })),
        _ => final_simples(),
    });
    let srv = Servidor::iniciar(roteiro);
    let mut c = Conversa::nova(cfg(&srv.url));
    let r = c.perguntar("x").unwrap();
    assert!(r.conferencia.achados[0].verificada);
    let erros: Vec<_> = c.eventos.iter().filter_map(|e| e.erro.clone()).collect();
    assert_eq!(erros, vec!["resposta vazia".to_string(), "JSON vazio".to_string()]);
}
