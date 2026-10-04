//! Protótipo da busca de sinais proposta para a `warden-security` (W-01):
//! lê cada `.class` com `cafebabe`, desce em jars embutidos, reconstrói os
//! textos de `new String(new byte[]{…})` e aplica dois tipos de regra:
//! sequência de chamadas (como o SIG1 do nekodetector/jNeedle) e constantes.

use cafebabe::attributes::AttributeData;
use cafebabe::bytecode::Opcode;
use cafebabe::constant_pool::{LiteralConstant, Loadable};
use std::io::Read;

#[derive(Debug)]
pub struct Finding {
    pub rule: &'static str,
    pub level: &'static str,
    pub path: String,
    pub detail: String,
}

/// SIG1 do fractureiser (nekodetector, MIT; jNeedle, MIT): ordem das chamadas
/// do estágio 0, sem as instruções intermediárias.
const SIG1: &[&str] = &[
    "java/lang/String.<init>([B)V",
    "java/lang/String.<init>([B)V",
    "java/lang/Class.forName(Ljava/lang/String;)Ljava/lang/Class;",
    "java/lang/Class.getConstructor([Ljava/lang/Class;)Ljava/lang/reflect/Constructor;",
    "java/lang/String.<init>([B)V",
    "java/lang/String.<init>([B)V",
    "java/lang/String.<init>([B)V",
    "java/net/URL.<init>(Ljava/lang/String;Ljava/lang/String;ILjava/lang/String;)V",
    "java/lang/reflect/Constructor.newInstance([Ljava/lang/Object;)Ljava/lang/Object;",
    "java/lang/Class.forName(Ljava/lang/String;ZLjava/lang/ClassLoader;)Ljava/lang/Class;",
    "java/lang/String.<init>([B)V",
    "java/lang/Class.getMethod(Ljava/lang/String;[Ljava/lang/Class;)Ljava/lang/reflect/Method;",
    "java/lang/reflect/Method.invoke(Ljava/lang/Object;[Ljava/lang/Object;)Ljava/lang/Object;",
];

/// Constantes de casos conhecidos (IOCs públicos).
const IOC_STRINGS: &[(&str, &str)] = &[
    ("85.217.144.130", "fractureiser: servidor do estágio 0 (2023)"),
    ("107.189.3.101", "fractureiser: segundo servidor (2023)"),
    ("connect.skyrage.de", "Skyrage"),
    ("files.skyrage.de", "Skyrage"),
];

pub fn scan_jar(bytes: &[u8], path: &str, depth: u32, out: &mut Vec<Finding>) {
    if depth > 3 {
        return;
    }
    let Ok(mut z) = zip::ZipArchive::new(std::io::Cursor::new(bytes)) else { return };
    for i in 0..z.len() {
        let Ok(mut f) = z.by_index(i) else { continue };
        let name = f.name().to_string();
        let mut data = Vec::new();
        if f.read_to_end(&mut data).is_err() {
            continue;
        }
        let here = format!("{path}!/{name}");
        if name.ends_with(".class") {
            scan_class(&data, &here, out);
        } else if name.ends_with(".jar") {
            scan_jar(&data, &here, depth + 1, out);
        }
    }
}

fn scan_class(bytes: &[u8], path: &str, out: &mut Vec<Finding>) {
    let Ok(class) = cafebabe::parse_class_with_options(bytes, cafebabe::ParseOptions::default().parse_bytecode(true)) else {
        return;
    };
    for m in &class.methods {
        for a in &m.attributes {
            let AttributeData::Code(code) = &a.data else { continue };
            let Some(bc) = &code.bytecode else { continue };
            let mut calls: Vec<String> = vec![];
            let mut consts: Vec<String> = vec![];
            // Reconstrução de new String(new byte[]{…}).
            let mut cur: Option<Vec<u8>> = None;
            let mut stack_ints: Vec<i32> = vec![];
            for (_, op) in &bc.opcodes {
                let int = match op {
                    Opcode::IconstM1 => Some(-1),
                    Opcode::Iconst0 => Some(0),
                    Opcode::Iconst1 => Some(1),
                    Opcode::Iconst2 => Some(2),
                    Opcode::Iconst3 => Some(3),
                    Opcode::Iconst4 => Some(4),
                    Opcode::Iconst5 => Some(5),
                    Opcode::Bipush(v) => Some(*v as i32),
                    Opcode::Sipush(v) => Some(*v as i32),
                    _ => None,
                };
                if let Some(v) = int {
                    stack_ints.push(v);
                    continue;
                }
                match op {
                    Opcode::Newarray(_) => {
                        cur = Some(vec![]);
                        stack_ints.clear();
                    }
                    Opcode::Bastore => {
                        if let (Some(buf), Some(v)) = (cur.as_mut(), stack_ints.last()) {
                            buf.push(*v as u8);
                        }
                        stack_ints.clear();
                    }
                    Opcode::Dup => {}
                    Opcode::Ldc(Loadable::LiteralConstant(LiteralConstant::String(s)))
                    | Opcode::LdcW(Loadable::LiteralConstant(LiteralConstant::String(s))) => {
                        consts.push(s.to_string());
                    }
                    Opcode::Invokespecial(r) | Opcode::Invokestatic(r) | Opcode::Invokevirtual(r) => {
                        let sig = format!("{}.{}{}", r.class_name, r.name_and_type.name, r.name_and_type.descriptor);
                        if sig == "java/lang/String.<init>([B)V" {
                            if let Some(buf) = cur.take() {
                                consts.push(String::from_utf8_lossy(&buf).into_owned());
                            }
                        }
                        calls.push(sig);
                    }
                    _ => {}
                }
            }
            let where_ = format!("{path} :: {}", m.name);
            if calls.windows(SIG1.len()).any(|w| w.iter().zip(SIG1).all(|(a, b)| a == b)) {
                out.push(Finding {
                    rule: "fractureiser-estagio0-sig1",
                    level: "sinal",
                    path: where_.clone(),
                    detail: "sequência de chamadas do estágio 0 (URLClassLoader por reflexão)".into(),
                });
            }
            for c in &consts {
                for (ioc, case) in IOC_STRINGS {
                    if c.contains(ioc) {
                        out.push(Finding { rule: "ioc-constante", level: "sinal", path: where_.clone(), detail: format!("{case}: \"{c}\"") });
                    }
                }
                if c.contains("discord.com/api/webhooks") || c.contains("discordapp.com/api/webhooks") {
                    out.push(Finding { rule: "webhook-discord", level: "atencao", path: where_.clone(), detail: c.clone() });
                }
                if is_ipv4(c) {
                    out.push(Finding { rule: "ip-fixo", level: "atencao", path: where_.clone(), detail: c.clone() });
                }
                if c == "java.net.URLClassLoader" {
                    out.push(Finding { rule: "texto-escondido-classloader", level: "atencao", path: where_.clone(), detail: c.clone() });
                }
            }
            if calls.iter().any(|c| c.starts_with("java/lang/Runtime.exec(")) {
                out.push(Finding { rule: "runtime-exec", level: "atencao", path: where_.clone(), detail: "Runtime.exec".into() });
            }
        }
    }
}

fn is_ipv4(s: &str) -> bool {
    let parts: Vec<&str> = s.split('.').collect();
    parts.len() == 4 && parts.iter().all(|p| !p.is_empty() && p.len() <= 3 && p.parse::<u8>().is_ok())
}
