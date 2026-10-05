//! Conformidade com o packwiz-installer real (ROADMAP L-03, critério 1; ADR-0011;
//! ARCHITECTURE §8.2).
//!
//! Para cada pack de teste, em vários passos (versão 1, versão 2 com mudanças, de novo sem
//! mudanças), o packwiz-installer 0.5.14 (pelo bootstrap, `-g -s client`) e o Warden instalam o
//! mesmo pack em pastas separadas, e as árvores precisam ser idênticas, arquivo a arquivo e
//! pasta a pasta (ignorando o `packwiz.json` do instalador; o estado do Warden fica fora do
//! `gameDir`). Entre os passos, o mesmo "jogo" mexe nas duas pastas (mundo criado, arquivo
//! preservado editado ou apagado).
//!
//! O instalador, sem janela, liga **todos** os opcionais; por isso aqui o Warden recebe as
//! escolhas "todos ligados". O padrão do Warden (o `default` de cada item) é testado em
//! `conformance_semantica.rs`.
//!
//! Sem murmur2 aqui: o `Murmur2Lib.java` do packwiz-installer 0.5.14 estende o sinal dos bytes
//! de 0x80 em diante no fim do arquivo (`(int) data[i]`), então o valor dele difere do da
//! CurseForge (e do packwiz) quando o tamanho sem espaços não é múltiplo de 4 e o fim tem esses
//! bytes; o instalador recusa o arquivo ("Hash invalid!"). O Warden segue a CurseForge (o
//! murmur2 da `warden-packwiz`, provado contra vetores do packwiz).
//!
//! Requer `cargo xtask installer` (Java e jars fixados). Sem eles, os testes avisam e passam;
//! com `WARDEN_REQUIRE_EXTERNALS=1`, falham.

// Nos auxiliares dos testes, `unwrap`, `expect` e `panic!` são a forma idiomática de falhar
// (o `clippy.toml` só os libera dentro das funções `#[test]`).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod conformance_support;

use std::fs;
use std::path::Path;

use conformance_support::{
    TestMod, TestPack, TestSide, dirs, externals, materialize_ok, run_installer, sources, temp_dir,
    tree, write,
};
use warden_instance::{MaterializeOptions, OptionalChoices, OptionalSelection};
use warden_packwiz::HashFormat;
use wiremock::MockServer;

/// O que o "jogo" faz na instância antes de um passo.
type Touch = fn(&Path);

/// Um passo: o pack e o que muda na instância antes de instalar.
struct Step {
    pack: TestPack,
    touch: Option<Touch>,
}

fn step(pack: &TestPack) -> Step {
    Step {
        pack: pack.clone(),
        touch: None,
    }
}

fn step_after(pack: &TestPack, touch: Touch) -> Step {
    Step {
        pack: pack.clone(),
        touch: Some(touch),
    }
}

/// Instala cada passo com o packwiz-installer e com o Warden e compara as árvores.
async fn conformance(name: &str, steps: Vec<Step>) {
    let Some(externals) = externals() else {
        return;
    };
    let dir = temp_dir();
    let root = dir.path();
    let server = MockServer::start().await;
    for step in &steps {
        step.pack.serve(&server).await;
    }
    let pack_root = root.join("pack");
    let installer_out = root.join("instalador").join("minecraft");
    let installer_cwd = root.join("instalador").join("cwd");
    let warden = dirs(&root.join("warden"));
    let sources = sources(&root.join("cache"), None);
    for (number, step) in steps.iter().enumerate() {
        step.pack.write_to(&pack_root, &server.uri());
        if let Some(touch) = step.touch {
            fs::create_dir_all(&installer_out).unwrap();
            fs::create_dir_all(&warden.game_dir).unwrap();
            touch(&installer_out);
            touch(&warden.game_dir);
        }
        let (ext, pack_toml, out, cwd) = (
            externals.clone(),
            pack_root.join("pack.toml"),
            installer_out.clone(),
            installer_cwd.clone(),
        );
        tokio::task::spawn_blocking(move || run_installer(&ext, &pack_toml, &out, &cwd))
            .await
            .unwrap();
        let options = MaterializeOptions {
            optionals: OptionalSelection::Explicit(OptionalChoices::all_enabled(
                step.pack.mods.iter().map(|m| m.metafile.as_str()),
            )),
            ..MaterializeOptions::default()
        };
        materialize_ok(&sources, &pack_root, &warden, &options).await;
        let expected = tree(&installer_out);
        let actual = tree(&warden.game_dir);
        assert!(
            expected.len() > 1,
            "{name}, passo {}: o instalador não instalou nada",
            number + 1
        );
        assert_eq!(
            actual,
            expected,
            "{name}, passo {}: árvores diferentes (esquerda: Warden; direita: packwiz-installer)",
            number + 1
        );
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn conformidade_opcionais() {
    let v1 = TestPack::default()
        .with_mod(TestMod::new("base"))
        .with_mod(TestMod::new("opcional-desligado").optional(false))
        .with_mod(
            TestMod::new("opcional-ligado")
                .optional(true)
                .hash(HashFormat::Sha512),
        )
        .with_file("config/base.toml", "valor = 1\n");
    let v2 = v1
        .clone()
        .without_mod("mods/opcional-ligado.pw.toml")
        .with_mod(
            TestMod::new("opcional-novo")
                .optional(false)
                .hash(HashFormat::Md5),
        );
    conformance("opcionais", vec![step(&v1), step(&v2), step(&v2)]).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn conformidade_preserve() {
    let v1 = TestPack::default()
        .with_mod(TestMod::new("base"))
        .with_preserved("config/preservado.cfg", "padrao = 1\n")
        .with_file("config/normal.cfg", "normal = 1\n")
        .with_preserved("options.txt", "renderDistance:8\n");
    let v2 = v1
        .clone()
        .with_preserved("config/preservado.cfg", "padrao = 2\n")
        .with_file("config/normal.cfg", "normal = 2\n")
        .with_preserved("config/preservado-novo.cfg", "novo = 1\n");
    conformance(
        "preserve",
        vec![
            step(&v1),
            // O jogador mudou o arquivo preservado: a versão 2 não pode tocar nele.
            step_after(&v2, |game| {
                write(game, "config/preservado.cfg", b"padrao = 99 # meu\n");
                write(game, "options.txt", b"renderDistance:16\n");
            }),
            // Apagado pelo jogador: volta do pack.
            step_after(&v2, |game| {
                fs::remove_file(game.join("config/preservado.cfg")).unwrap();
            }),
        ],
    )
    .await;
}

#[tokio::test(flavor = "multi_thread")]
async fn conformidade_lado_servidor() {
    let v1 = TestPack::default()
        .with_mod(TestMod::new("comum"))
        .with_mod(TestMod::new("so-cliente").side(TestSide::Client))
        .with_mod(TestMod::new("so-servidor").side(TestSide::Server))
        .with_mod(
            TestMod::new("servidor-config")
                .side(TestSide::Server)
                .at("config/servidor.pw.toml", "servidor.toml"),
        );
    let v2 = v1
        .clone()
        .replace_mod(TestMod::new("so-servidor").side(TestSide::Both))
        .replace_mod(TestMod::new("so-cliente").side(TestSide::Server));
    conformance("lado-servidor", vec![step(&v1), step(&v2), step(&v2)]).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn conformidade_configs() {
    let v1 = TestPack::default()
        .with_mod(TestMod::new("create").hash(HashFormat::Sha256))
        .with_file("config/create-client.toml", "[client]\nmostrar = true\n")
        .with_file("config/pasta/funda/x.json", "{\"a\": 1}\n")
        .with_file("defaultconfigs/create-server.toml", "[server]\nx = 1\n")
        .with_file(
            "kubejs/server_scripts/receitas.js",
            "ServerEvents.recipes(e => {})\n",
        )
        .with_file("config/acentuação é ok.txt", "ação\n")
        .with_file("options.txt", "lang:pt_br\n");
    let v2 = {
        let mut pack = v1
            .clone()
            .with_file("config/create-client.toml", "[client]\nmostrar = false\n")
            .with_file("config/novo.toml", "novo = true\n");
        pack.files.remove("config/pasta/funda/x.json");
        pack
    };
    conformance("configs", vec![step(&v1), step(&v2), step(&v2)]).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn conformidade_resource_packs_e_atualizacao() {
    let v1 = TestPack::default()
        .with_mod(
            TestMod::new("sodium")
                .modrinth("AANobbMI", "v1")
                .hash(HashFormat::Sha512),
        )
        .with_mod(TestMod::new("removido"))
        .with_mod(
            TestMod::new("faithful")
                .at("resourcepacks/faithful.pw.toml", "Faithful 32x.zip")
                .hash(HashFormat::Sha256),
        )
        .with_mod(
            TestMod::new("complementary")
                .side(TestSide::Client)
                .at("shaderpacks/complementary.pw.toml", "Complementary.zip"),
        )
        .with_file("resourcepacks/embutido.zip", "zip de teste");
    let v2 = v1.clone().without_mod("mods/removido.pw.toml").replace_mod(
        TestMod::new("sodium")
            .version("2.0")
            .modrinth("AANobbMI", "v2")
            .hash(HashFormat::Sha512),
    );
    conformance(
        "resource-packs",
        vec![
            step(&v1),
            // CA-T13-04: um mundo criado no teste continua depois de remover um mod.
            step_after(&v2, |game| {
                write(game, "saves/Mundo de teste/level.dat", b"mundo");
                write(game, "mods/manual-do-jogador.jar", b"jar posto a mao");
            }),
            step(&v2),
        ],
    )
    .await;
}

#[tokio::test(flavor = "multi_thread")]
async fn conformidade_indice_em_subpasta() {
    let mut v1 = TestPack::default()
        .with_mod(TestMod::new("base"))
        .with_file("config/x.toml", "x = 1\n");
    v1.index_dir = "meta".to_owned();
    conformance("indice-em-subpasta", vec![step(&v1), step(&v1)]).await;
}
