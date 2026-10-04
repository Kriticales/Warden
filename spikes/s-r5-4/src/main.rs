//! Spike S-R5-4: laço de ferramentas do Gemini.
//!
//! Uso:
//!   sr54 simulado                         cenários contra o servidor simulado (sem custo)
//!   sr54 real <modelo> <cenário> [opções]  conversa real (custa); grava fixture e métricas
//!   sr54 assinaturas <modelo>             como a API real reage a assinaturas alteradas
//!   sr54 reproduzir <fixture.json>        roda o laço contra a gravação, no simulado
//!   sr54 contar <modelo>                  countTokens do pedido inicial
//!
//! Opções de `real`: --separado (responseFormat só no fim), --thinking <low|medium|high|minimal>,
//! --sem-escalonar (todas as ferramentas liberadas desde a primeira rodada).

mod env;
mod evidence;
mod laco;
mod mock;
mod pack;
mod tools;
#[cfg(test)]
mod testes_laco;

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use anyhow::{Context, bail};
use serde_json::{Value, json};

use laco::{Config, Conversa, ModoFinal};

const ENV_PRINCIPAL: &str = r"C:\Users\solel\orca\projects\Warden\.env";
const BASE_REAL: &str = "https://generativelanguage.googleapis.com";

const P_TRAVAMENTO: &str = "O pack travou no último teste (sessão s3). Por que travou e como resolvo?";
const P_SEGUIMENTO: &str = "E se, em vez de atualizar o Steam 'n' Rails, eu voltar o Create para a versão que eu tinha antes? Funciona?";
const P_LENTIDAO: &str = "Na sessão s3 apareceu 'Can't keep up!' no log antes de travar. O que está deixando o servidor lento?";

fn perguntas(cenario: &str) -> anyhow::Result<Vec<&'static str>> {
    Ok(match cenario {
        "travamento" => vec![P_TRAVAMENTO],
        "seguimento" => vec![P_TRAVAMENTO, P_SEGUIMENTO],
        "lentidao" => vec![P_LENTIDAO],
        _ => bail!("cenário desconhecido: {cenario} (travamento, seguimento, lentidao)"),
    })
}

fn chave_real() -> anyhow::Result<String> {
    let caminho = std::env::var("WARDEN_ENV").map(PathBuf::from).unwrap_or_else(|_| PathBuf::from(ENV_PRINCIPAL));
    env::ler_variavel(&caminho, "GEMINI_API_KEY")?.context("GEMINI_API_KEY ausente no .env")
}

fn pasta_spike() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn cfg(base: &str, modelo: &str, chave: String) -> Config {
    Config {
        base_url: base.into(),
        modelo: modelo.into(),
        chave,
        modo_final: ModoFinal::Junto,
        max_rodadas: 8,
        thinking_level: None,
        escalonar_ferramentas: true,
        timeout: Duration::from_secs(90),
    }
}

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("simulado") => simulado(),
        Some("real") if args.len() >= 3 => real(&args[1], &args[2], &args[3..]),
        Some("assinaturas") if args.len() >= 2 => assinaturas(&args[1]),
        Some("reproduzir") if args.len() >= 2 => reproduzir(Path::new(&args[1])),
        Some("contar") if args.len() >= 2 => contar(&args[1]),
        _ => {
            eprintln!("uso: sr54 simulado | real <modelo> <cenário> [--separado] [--thinking X] [--sem-escalonar] | assinaturas <modelo> | reproduzir <fixture> | contar <modelo>");
            std::process::exit(2);
        }
    }
}

// ---------------------------------------------------------------- simulado

fn simulado() -> anyhow::Result<()> {
    use mock::*;
    // Roteiro: chamadas paralelas, uma MALFORMED_FUNCTION_CALL, mais chamadas e o JSON final
    // com uma evidência boa, uma citação inventada e um mod inventado.
    let roteiro: Roteiro = Box::new(|i, _req| match i {
        0 => chamadas(&[("get_findings", json!({})), ("get_crash_report", json!({ "session_id": "s3" }))]),
        1 => malformada(),
        2 => chamadas(&[("get_mod_details", json!({ "mod_id": "railways" })), ("diff_versions", json!({ "from": "v1.3", "to": "v1.4" }))]),
        3 => chamadas(&[("get_mod_changelog", json!({ "mod_id": "railways" }))]),
        _ => final_json(&json!({
            "resumo": "O Steam 'n' Rails 1.6.4 não é compatível com o Create 6.",
            "semConclusao": false,
            "achados": [
                { "afirmacao": "O travamento vem do mixin do Steam 'n' Rails.", "mods": ["railways"], "confianca": "alta",
                  "evidencias": [{ "id": "crash:s3", "citacao": "MixinCarriageContraptionEntity.railways$tickBogeys" }] },
                { "afirmacao": "Falta memória.", "mods": [], "confianca": "baixa",
                  "evidencias": [{ "id": "crash:s3", "citacao": "OutOfMemoryError" }] },
                { "afirmacao": "Instale o TurboFix Ultimate.", "mods": ["turbofix"], "confianca": "alta",
                  "evidencias": [{ "id": "log:s3/latest.log#L788", "citacao": "TurboFix" }] }
            ],
            "propostas": [
                { "tipo": "atualizar_mod", "modId": "railways", "detalhe": "Atualizar para 1.6.7+forge-mc1.20.1",
                  "evidencias": [{ "id": "changelog:railways@1.6.7+forge-mc1.20.1", "citacao": "Ported to Create 6.0." }] },
                { "tipo": "remover_mod", "modId": "create", "detalhe": "sem prova", "evidencias": [] }
            ]
        })),
    });
    let srv = Servidor::iniciar(roteiro);
    let mut conv = Conversa::nova(cfg(&srv.url, "simulado", CHAVE_FALSA.into()));
    let r = conv.perguntar(P_TRAVAMENTO)?;
    println!("== cenário feliz (simulado) ==");
    imprimir_eventos(&conv);
    println!("recusas do servidor: {:?}", srv.recusas());
    println!("{}", serde_json::to_string_pretty(&r.conferencia)?);

    // Limite de rodadas: o roteiro nunca para de chamar ferramentas.
    let roteiro: Roteiro = Box::new(|_i, req| {
        if req.pointer("/toolConfig/functionCallingConfig/mode").and_then(Value::as_str) == Some("NONE") {
            final_json(&json!({ "resumo": "Não encontrei a causa.", "semConclusao": true, "achados": [], "propostas": [] }))
        } else {
            chamadas(&[("list_mods", json!({}))])
        }
    });
    let srv = Servidor::iniciar(roteiro);
    let mut conv = Conversa::nova(cfg(&srv.url, "simulado", CHAVE_FALSA.into()));
    let r = conv.perguntar(P_LENTIDAO)?;
    println!("\n== limite de 8 rodadas (simulado) ==");
    println!("rodadas: {}, requisições: {}, última: modo {}", r.rodadas, conv.eventos.len(), conv.eventos.last().unwrap().modo);
    Ok(())
}

fn imprimir_eventos(conv: &Conversa) {
    for e in &conv.eventos {
        println!(
            "P{} r{} t{} {} fmt={} perm={} http={} {}ms fin={} chamadas={:?} assin={:?} outras_assin={} uso={:?}{}",
            e.pergunta,
            e.rodada,
            e.tentativa,
            e.modo,
            e.com_response_format,
            e.permitidas,
            e.http,
            e.ms,
            e.finish_reason,
            e.chamadas,
            e.assinaturas,
            e.outras_partes_com_assinatura,
            (e.uso.prompt, e.uso.cache, e.uso.saida, e.uso.pensamento),
            e.erro.as_ref().map(|x| format!(" ERRO={x}")).unwrap_or_default()
        );
    }
}

// ---------------------------------------------------------------- real

fn real(modelo: &str, cenario: &str, opcoes: &[String]) -> anyhow::Result<()> {
    let chave = chave_real()?;
    let mut c = cfg(BASE_REAL, modelo, chave.clone());
    let mut i = 0;
    while i < opcoes.len() {
        match opcoes[i].as_str() {
            "--separado" => c.modo_final = ModoFinal::Separado,
            "--ferramenta" => c.modo_final = ModoFinal::Ferramenta,
            "--sem-escalonar" => c.escalonar_ferramentas = false,
            "--thinking" => {
                i += 1;
                c.thinking_level = opcoes.get(i).cloned();
            }
            o => bail!("opção desconhecida: {o}"),
        }
        i += 1;
    }
    let rotulo = format!(
        "{cenario}-{modelo}{}{}",
        match c.modo_final {
            ModoFinal::Junto => "",
            ModoFinal::Separado => "-separado",
            ModoFinal::Ferramenta => "-ferramenta",
        },
        c.thinking_level.as_ref().map(|t| format!("-thinking-{t}")).unwrap_or_default()
    );
    let mut conv = Conversa::nova(c);
    let inicio = Instant::now();
    let mut resultados = vec![];
    let mut erro = None;
    for p in perguntas(cenario)? {
        match conv.perguntar(p) {
            Ok(r) => resultados.push(r),
            Err(e) => {
                erro = Some(e.to_string());
                break;
            }
        }
    }
    let total_ms = inicio.elapsed().as_millis();
    imprimir_eventos(&conv);
    let uso = conv.uso_total();
    let custo = laco::custo(modelo, &uso);
    println!("total: {total_ms} ms, uso {uso:?}, custo US$ {custo:?}");
    if let Some(e) = &erro {
        println!("ERRO: {e}");
    }
    for r in &resultados {
        println!("{}", serde_json::to_string_pretty(&r.conferencia)?);
    }

    // Execução completa (fora do git) e fixture pequena (no git).
    let execucao = json!({
        "modelo": modelo, "cenario": cenario, "total_ms": total_ms, "uso": uso, "custo_usd": custo,
        "erro": erro, "eventos": conv.eventos,
        "resultados": resultados,
        "contents": conv.contents,
    });
    let pasta_exec = pasta_spike().join("execucoes");
    std::fs::create_dir_all(&pasta_exec)?;
    gravar_sem_chave(&pasta_exec.join(format!("{rotulo}.json")), &execucao, &chave)?;

    let fixture = json!({
        "origem": "Gravação real da API do Gemini (generateContent) feita pelo spike S-R5-4 em 2026-10-04, com o pack fictício Vale Sombrio. Só os corpos das respostas, na ordem; sem cabeçalhos nem chave.",
        "modelo": modelo,
        "cenario": cenario,
        "perguntas": perguntas(cenario)?,
        "respostas": conv.eventos.iter().filter_map(|e| e.resposta.clone()).collect::<Vec<_>>(),
        "erros": conv.eventos.iter().filter_map(|e| e.erro.clone().map(|x| json!({ "rodada": e.rodada, "http": e.http, "erro": x }))).collect::<Vec<_>>(),
        "uso_total": uso,
        "conferencias": resultados.iter().map(|r| &r.conferencia).collect::<Vec<_>>(),
    });
    gravar_sem_chave(&pasta_spike().join("fixtures").join(format!("{rotulo}.json")), &fixture, &chave)?;
    Ok(())
}

fn gravar_sem_chave(caminho: &Path, v: &Value, chave: &str) -> anyhow::Result<()> {
    let texto = serde_json::to_string_pretty(v)? + "\n";
    if texto.contains(chave) {
        bail!("a chave apareceria em {}; nada gravado", caminho.display());
    }
    std::fs::write(caminho, texto)?;
    println!("gravado: {}", caminho.display());
    Ok(())
}

// ---------------------------------------------------------------- reproduzir

fn reproduzir(caminho: &Path) -> anyhow::Result<()> {
    let f: Value = serde_json::from_str(&std::fs::read_to_string(caminho)?)?;
    let respostas = f["respostas"].as_array().cloned().unwrap_or_default();
    let srv = mock::Servidor::iniciar(mock::roteiro_de_gravacao(respostas));
    let mut conv = Conversa::nova(cfg(&srv.url, f["modelo"].as_str().unwrap_or("?"), mock::CHAVE_FALSA.into()));
    for p in f["perguntas"].as_array().cloned().unwrap_or_default() {
        let r = conv.perguntar(p.as_str().unwrap_or(""))?;
        println!("{}", serde_json::to_string(&r.conferencia)?);
    }
    imprimir_eventos(&conv);
    println!("recusas do servidor simulado: {:?}", srv.recusas());
    Ok(())
}

// ---------------------------------------------------------------- contar

fn contar(modelo: &str) -> anyhow::Result<()> {
    let mut conv = Conversa::nova(cfg(BASE_REAL, modelo, chave_real()?));
    conv.contents.push(json!({ "role": "user", "parts": [{ "text": P_TRAVAMENTO }] }));
    println!("countTokens (sistema + 17 ferramentas + pergunta): {}", conv.contar_tokens()?);
    Ok(())
}

// ---------------------------------------------------------------- assinaturas

/// Uma chamada real que deve produzir chamadas paralelas; depois várias versões do histórico
/// (intacto, sem assinatura, assinatura alterada, assinatura na parte errada, respostas
/// intercaladas, assinatura falsa documentada) para ver o que a API real aceita.
fn assinaturas(modelo: &str) -> anyhow::Result<()> {
    let chave = chave_real()?;
    let mut c = cfg(BASE_REAL, modelo, chave.clone());
    c.escalonar_ferramentas = false;
    c.thinking_level = Some("low".into());
    let conv = Conversa::nova(c.clone());
    let http = reqwest::blocking::Client::builder().timeout(Duration::from_secs(90)).build()?;
    let url = format!("{BASE_REAL}/v1beta/models/{modelo}:generateContent");
    let post = |corpo: &Value| -> anyhow::Result<(u16, Value)> {
        let r = http.post(&url).header("x-goog-api-key", &chave).json(corpo).send()?;
        let s = r.status().as_u16();
        Ok((s, serde_json::from_str(&r.text()?).unwrap_or(Value::Null)))
    };
    let pergunta = json!({ "role": "user", "parts": [{ "text": "Preciso de duas coisas ao mesmo tempo: os achados do diagnóstico e o crash report da sessão s3. Chame get_findings e get_crash_report juntas, na mesma resposta." }] });
    let mut conv = conv;
    conv.contents.push(pergunta.clone());
    let mut base = None;
    let mut registros = vec![];
    for tentativa in 0..2 {
        let corpo = json!({
            "systemInstruction": { "parts": [{ "text": laco::SYSTEM_PROMPT }] },
            "contents": [pergunta],
            "tools": tools::declaracoes(),
            "toolConfig": { "functionCallingConfig": { "mode": "VALIDATED" } },
            "generationConfig": { "temperature": 1.0, "thinkingConfig": { "thinkingLevel": "low" } }
        });
        let (s, r) = post(&corpo)?;
        let partes = r.pointer("/candidates/0/content/parts").and_then(Value::as_array).cloned().unwrap_or_default();
        let n = partes.iter().filter(|p| p.get("functionCall").is_some()).count();
        println!("tentativa {tentativa}: http {s}, {n} chamadas, assinaturas por parte: {:?}", partes.iter().map(|p| (p.get("functionCall").is_some(), p.get("thoughtSignature").map(|x| x.as_str().unwrap_or("").len()))).collect::<Vec<_>>());
        registros.push(json!({ "etapa": format!("chamada inicial {tentativa}"), "http": s, "resposta": r.clone() }));
        if n >= 2 {
            base = Some(r);
            break;
        }
    }
    let Some(base) = base else {
        gravar_sem_chave(&pasta_spike().join("execucoes").join(format!("assinaturas-{modelo}.json")), &json!(registros), &chave)?;
        bail!("o modelo não fez chamadas paralelas em 2 tentativas");
    };
    let modelo_content = base.pointer("/candidates/0/content").cloned().unwrap();
    let mut reg = tools::Registro::default();
    let respostas: Vec<Value> = modelo_content["parts"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|p| p.get("functionCall"))
        .map(|fc| {
            let nome = fc["name"].as_str().unwrap();
            let mut fr = json!({ "name": nome, "response": tools::executar(nome, &fc["args"], &mut reg) });
            if let Some(id) = fc.get("id") {
                fr["id"] = id.clone();
            }
            json!({ "functionResponse": fr })
        })
        .collect();

    let variantes: Vec<(&str, Vec<Value>)> = {
        let resp_user = json!({ "role": "user", "parts": respostas.clone() });
        let mut sem = modelo_content.clone();
        for p in sem["parts"].as_array_mut().unwrap() {
            p.as_object_mut().unwrap().remove("thoughtSignature");
        }
        let mut alterada = modelo_content.clone();
        for p in alterada["parts"].as_array_mut().unwrap() {
            if let Some(s) = p.get("thoughtSignature").and_then(Value::as_str) {
                let mut b = s.as_bytes().to_vec();
                let meio = b.len() / 2;
                b[meio] = if b[meio] == b'A' { b'B' } else { b'A' };
                p["thoughtSignature"] = json!(String::from_utf8(b).unwrap());
            }
        }
        let mut trocada = modelo_content.clone();
        {
            let ps = trocada["parts"].as_array_mut().unwrap();
            if let Some(s) = ps[0].as_object_mut().unwrap().remove("thoughtSignature") {
                ps[1]["thoughtSignature"] = s;
            }
        }
        let mut falsa = sem.clone();
        falsa["parts"][0]["thoughtSignature"] = json!("skip_thought_signature_validator");
        // Intercalado: FC1, FR1, FC2, FR2.
        let ps = modelo_content["parts"].as_array().unwrap();
        let intercalado = vec![
            pergunta.clone(),
            json!({ "role": "model", "parts": [ps[0].clone()] }),
            json!({ "role": "user", "parts": [respostas[0].clone()] }),
            json!({ "role": "model", "parts": [ps[1].clone()] }),
            json!({ "role": "user", "parts": [respostas[1].clone()] }),
        ];
        vec![
            ("sem assinatura", vec![pergunta.clone(), sem, resp_user.clone()]),
            ("assinatura alterada (1 caractere)", vec![pergunta.clone(), alterada, resp_user.clone()]),
            ("assinatura movida para a 2a chamada", vec![pergunta.clone(), trocada, resp_user.clone()]),
            ("respostas intercaladas", intercalado),
            ("assinatura falsa documentada", vec![pergunta.clone(), falsa, resp_user.clone()]),
            ("intacto", vec![pergunta.clone(), modelo_content.clone(), resp_user]),
        ]
    };
    for (nome, contents) in variantes {
        let corpo = json!({
            "systemInstruction": { "parts": [{ "text": laco::SYSTEM_PROMPT }] },
            "contents": contents,
            "tools": tools::declaracoes(),
            "toolConfig": { "functionCallingConfig": { "mode": "NONE" } },
            "generationConfig": { "temperature": 1.0, "thinkingConfig": { "thinkingLevel": "low" }, "maxOutputTokens": 300 }
        });
        let (s, r) = post(&corpo)?;
        let msg = r.pointer("/error/message").and_then(Value::as_str).map(String::from);
        let uso = r.get("usageMetadata").cloned();
        println!("{nome}: http {s} {}", msg.clone().unwrap_or_else(|| format!("ok, uso {}", uso.clone().unwrap_or(Value::Null))));
        registros.push(json!({ "etapa": nome, "http": s, "erro": msg, "uso": uso }));
    }
    gravar_sem_chave(&pasta_spike().join("fixtures").join(format!("assinaturas-{modelo}.json")), &json!({
        "origem": "Gravação real (spike S-R5-4, 2026-10-04): reação da API do Gemini a históricos com assinaturas alteradas. Sem cabeçalhos nem chave.",
        "modelo": modelo,
        "resposta_com_chamadas_paralelas": base,
        "variantes": registros,
    }), &chave)?;
    Ok(())
}
