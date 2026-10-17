//! Pack grande gerado para as medições de desempenho (A-04; SPEC §8): 300 mods e 500 configs,
//! sem baixar nada.
//!
//! - **Mods:** um `.pw.toml` do Modrinth por mod (IDs inventados, lado variado), com o hash
//!   SHA-512 do jar sintético correspondente. Com [`Opcoes::jars`], os jars são gravados
//!   naquela pasta, como numa instância de teste já sincronizada.
//! - **Jars sintéticos:** só o que os índices de pacotes da D-05 e as regras do pré-teste
//!   leem: `fabric.mod.json` (com dependências entre os mods), a config de mixin, um arquivo de
//!   idioma e classes mínimas (cabeçalho com a versão) em vários pacotes. Em dois tipos:
//!   - **mínimos** (poucos KB): cada 25º jar traz 5 jars embutidos (`jars[]`); cada 100º é
//!     grande (150 pacotes);
//!   - **calibrados** pelos packs reais ([`Opcoes::tamanho_real`]): número de entradas,
//!     classes, pacotes, embutidos e tamanho de cada jar seguem as [`FAIXAS`], medidas nos
//!     jars dos mods de três packs reais grandes do Modrinth.
//! - **Configs:** o corpus de configs reais da `warden-configs` (`tests/corpus/`), repetido em
//!   lotes (`config/lote<n>/`), como no teste da C-05.
//! - `pack.toml` (Fabric 1.20.1) e `index.toml` com os hashes certos: o pack é válido para o
//!   leitor do Warden e para o packwiz.
//!
//! Incluído pelos benchmarks das crates com `#[path = ".../pack_grande.rs"] mod pack_grande;`.
//! Usa só `sha2` e `zip` (dependências de desenvolvimento de quem inclui).

#![allow(dead_code, unreachable_pub)] // Cada benchmark usa uma parte do gerador.

use std::fmt::Write as _;
use std::fs;
use std::io::{Cursor, Write as _};
use std::path::{Path, PathBuf};

use sha2::{Digest as _, Sha256, Sha512};
use zip::CompressionMethod;
use zip::write::{SimpleFileOptions, ZipWriter};

/// Quantos mods tem o pack das metas da SPEC §8.
pub const MODS: usize = 300;
/// Quantas configs tem o pack das metas da SPEC §8.
pub const CONFIGS: usize = 500;
/// Versão do Minecraft do pack gerado.
pub const MINECRAFT: &str = "1.20.1";
/// Versão do Fabric Loader do pack gerado.
pub const FABRIC: &str = "0.16.5";

/// O que gerar.
#[derive(Debug, Clone)]
pub struct Opcoes {
    /// Número de mods.
    pub mods: usize,
    /// Número de configs.
    pub configs: usize,
    /// Pasta onde gravar os jars (`None`: só os metafiles).
    pub jars: Option<PathBuf>,
    /// Jars calibrados pelos packs reais ([`FAIXAS`]: ~730 entradas e ~2,6 MB em média,
    /// ~780 MB nos 300 mods). Sem isso, cada jar tem poucos KB.
    pub tamanho_real: bool,
}

impl Default for Opcoes {
    fn default() -> Self {
        Self {
            mods: MODS,
            configs: CONFIGS,
            jars: None,
            tamanho_real: false,
        }
    }
}

/// Um mod gerado.
#[derive(Debug, Clone)]
pub struct ModGerado {
    /// Mod id declarado no `fabric.mod.json`.
    pub id: String,
    /// Caminho do metafile, relativo à pasta do pack (`mods/<id>.pw.toml`).
    pub metafile: String,
    /// Nome do jar.
    pub arquivo: String,
    /// ID do projeto no Modrinth (inventado).
    pub projeto: String,
    /// Jar gravado, quando [`Opcoes::jars`] foi dado.
    pub jar: Option<PathBuf>,
}

/// O pack gerado.
#[derive(Debug, Clone)]
pub struct PackGerado {
    /// Pasta do pack.
    pub raiz: PathBuf,
    /// Os mods, na ordem de geração.
    pub mods: Vec<ModGerado>,
    /// As configs, relativas à pasta do pack.
    pub configs: Vec<String>,
}

/// Gera o pack em `raiz` (criada se não existir).
pub fn gerar(raiz: &Path, opcoes: &Opcoes) -> PackGerado {
    fs::create_dir_all(raiz.join("mods")).expect("pasta mods");
    if let Some(dir) = &opcoes.jars {
        fs::create_dir_all(dir).expect("pasta dos jars");
    }
    // (caminho, hash sha256, é metafile)
    let mut indice: Vec<(String, String, bool)> = Vec::new();
    let mut mods = Vec::with_capacity(opcoes.mods);
    for n in 0..opcoes.mods {
        let id = mod_id(n);
        let arquivo = format!("{id}-1.0.{n}.jar");
        let bytes = jar_sintetico(n, opcoes.tamanho_real);
        let jar = opcoes.jars.as_ref().map(|dir| {
            let path = dir.join(&arquivo);
            fs::write(&path, &bytes).expect("gravar jar");
            path
        });
        let projeto = format!("Wb{n:06}");
        let metafile = format!("mods/{id}.pw.toml");
        let texto = metafile_toml(n, &id, &arquivo, &projeto, &bytes);
        fs::write(raiz.join(&metafile), &texto).expect("gravar metafile");
        indice.push((metafile.clone(), sha256(texto.as_bytes()), true));
        mods.push(ModGerado {
            id,
            metafile,
            arquivo,
            projeto,
            jar,
        });
    }
    let mut configs = Vec::with_capacity(opcoes.configs);
    let corpus = corpus();
    assert!(!corpus.is_empty(), "corpus de configs vazio");
    for (posicao, (pasta, nome, bytes)) in corpus.iter().cycle().take(opcoes.configs).enumerate() {
        let lote = posicao / corpus.len();
        let relativo = if pasta == "options" {
            let haste = nome.trim_end_matches(".txt");
            format!("config/lote{lote}/{haste}/options.txt")
        } else {
            format!("config/lote{lote}/{nome}")
        };
        let path = raiz.join(&relativo);
        fs::create_dir_all(path.parent().expect("pai")).expect("pasta da config");
        fs::write(&path, bytes).expect("gravar config");
        indice.push((relativo.clone(), sha256(bytes), false));
        configs.push(relativo);
    }
    indice.sort_by(|a, b| a.0.cmp(&b.0));
    let mut index = String::from("hash-format = \"sha256\"\n");
    for (arquivo, hash, metafile) in &indice {
        let _ = write!(
            index,
            "\n[[files]]\nfile = {}\nhash = \"{hash}\"\n",
            toml_str(arquivo)
        );
        if *metafile {
            index.push_str("metafile = true\n");
        }
    }
    fs::write(raiz.join("index.toml"), &index).expect("gravar index.toml");
    let pack = format!(
        "name = \"Pack Grande\"\nauthor = \"Warden\"\nversion = \"1.0.0\"\n\
         pack-format = \"packwiz:1.1.0\"\n\n[index]\nfile = \"index.toml\"\n\
         hash-format = \"sha256\"\nhash = \"{}\"\n\n[versions]\nminecraft = \"{MINECRAFT}\"\n\
         fabric = \"{FABRIC}\"\n",
        sha256(index.as_bytes())
    );
    fs::write(raiz.join("pack.toml"), pack).expect("gravar pack.toml");
    PackGerado {
        raiz: raiz.to_path_buf(),
        mods,
        configs,
    }
}

/// Mod id do mod `n`.
pub fn mod_id(n: usize) -> String {
    format!("bench_mod_{n:03}")
}

/// Pacote Java base do mod `n` (com `.`).
pub fn pacote(n: usize) -> String {
    format!("dev.warden.bench.mod{n:03}")
}

fn metafile_toml(n: usize, id: &str, arquivo: &str, projeto: &str, jar: &[u8]) -> String {
    let side = match n % 10 {
        0 => "client",
        5 => "server",
        _ => "both",
    };
    format!(
        "name = \"Mod de Bancada {n:03}\"\nfilename = \"{arquivo}\"\nside = \"{side}\"\n\n\
         [download]\nurl = \"https://cdn.modrinth.com/data/{projeto}/versions/Wv{n:06}/{arquivo}\"\n\
         hash-format = \"sha512\"\nhash = \"{}\"\n\n[update]\n[update.modrinth]\n\
         mod-id = \"{projeto}\"\nversion = \"Wv{n:06}\"\n# {id}\n",
        hex(&Sha512::digest(jar))
    )
}

/// Uma faixa de jars reais: as médias de um vigésimo dos jars, ordenados pelo tamanho.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Faixa {
    /// Tamanho do arquivo, em KiB.
    pub kib: usize,
    /// Entradas do zip.
    pub entradas: usize,
    /// Arquivos `.class`.
    pub classes: usize,
    /// Pacotes Java com classes.
    pub pacotes: usize,
    /// Jars embutidos.
    pub embutidos: usize,
}

/// Os jars dos mods de três packs reais grandes do Modrinth (Prominence II 4.1.4, Fabric 1.20.1,
/// 405 mods; Better MC Forge v59, Forge 1.20.1, 309 mods; Better MC Fabric v40, Fabric 1.20.1,
/// 313 mods), medidos em 17/10/2026: 895 jars únicos, ordenados pelo tamanho e divididos em 20
/// faixas; cada faixa tem a média dos jars dela (a média, e não a mediana, para o total bater:
/// a última faixa tem os mods de dezenas de MB). No todo: 2,6 MiB, 727 entradas, 24 pacotes e
/// 1 embutido por jar em média. O método está no `docs/DESEMPENHO.md`.
pub const FAIXAS: [Faixa; 20] = [
    faixa(10, 18, 4, 3, 0),
    faixa(26, 34, 13, 5, 0),
    faixa(46, 48, 14, 7, 0),
    faixa(68, 59, 22, 7, 0),
    faixa(94, 86, 32, 10, 0),
    faixa(126, 93, 37, 11, 0),
    faixa(175, 124, 40, 11, 1),
    faixa(214, 114, 45, 12, 1),
    faixa(257, 143, 47, 12, 1),
    faixa(332, 213, 78, 22, 1),
    faixa(421, 182, 77, 17, 1),
    faixa(522, 319, 141, 29, 0),
    faixa(624, 398, 120, 25, 1),
    faixa(769, 407, 122, 19, 1),
    faixa(1001, 602, 187, 33, 1),
    faixa(1298, 695, 279, 43, 2),
    faixa(1777, 684, 238, 47, 4),
    faixa(2761, 1727, 431, 53, 2),
    faixa(6128, 2510, 453, 47, 2),
    faixa(36579, 6025, 639, 74, 2),
];

const fn faixa(
    kib: usize,
    entradas: usize,
    classes: usize,
    pacotes: usize,
    embutidos: usize,
) -> Faixa {
    Faixa {
        kib,
        entradas,
        classes,
        pacotes,
        embutidos,
    }
}

/// A faixa do jar `n` calibrado: cada 20 mods seguidos passam por todas, misturadas (7 e 20
/// são primos entre si).
pub fn faixa_do_mod(n: usize) -> Faixa {
    FAIXAS[(n * 7) % FAIXAS.len()]
}

/// O jar sintético do mod `n`: determinístico, um por mod. `calibrado`: com o perfil de
/// [`faixa_do_mod`]; senão, mínimo.
pub fn jar_sintetico(n: usize, calibrado: bool) -> Vec<u8> {
    let id = mod_id(n);
    let base = pacote(n);
    let faixa = calibrado.then(|| faixa_do_mod(n));
    let (pacotes, classes_por_pacote, quantos_embutidos) = match faixa {
        Some(f) => (f.pacotes, f.classes.div_ceil(f.pacotes).max(1), f.embutidos),
        None if n % 100 == 99 => (150, 12, 0),
        None => (8 + n % 25, 12, if n % 25 == 24 { 5 } else { 0 }),
    };
    let embutidos: Vec<String> = (0..quantos_embutidos)
        .map(|k| format!("META-INF/jars/{id}-modulo{k}.jar"))
        .collect();
    // Um terço dos mods depende do anterior: as regras de dependência têm o que conferir.
    let mut depende = format!(
        "\"fabricloader\": \">=0.14.0\", \"minecraft\": \"~{MINECRAFT}\", \"java\": \">=17\""
    );
    if n % 3 == 1 {
        let _ = write!(depende, ", \"{}\": \"*\"", mod_id(n - 1));
    }
    let jars_json: Vec<String> = embutidos
        .iter()
        .map(|path| format!("{{\"file\": \"{path}\"}}"))
        .collect();
    let fabric_mod_json = format!(
        "{{\"schemaVersion\": 1, \"id\": \"{id}\", \"version\": \"1.0.{n}\", \
         \"name\": \"Mod de Bancada {n:03}\", \"environment\": \"*\", \
         \"entrypoints\": {{\"main\": [\"{base}.Principal\"]}}, \
         \"mixins\": [\"{id}.mixins.json\"], \"depends\": {{{depende}}}, \
         \"jars\": [{}]}}",
        jars_json.join(", ")
    );
    let mixins = format!(
        "{{\"required\": true, \"package\": \"{base}.mixin\", \"compatibilityLevel\": \"JAVA_17\", \
         \"mixins\": [\"MixinMundo\", \"MixinEntidade\"], \"client\": [\"MixinTela\"]}}"
    );
    let mut jar = Jar::default()
        .arquivo("fabric.mod.json", fabric_mod_json.as_bytes())
        .arquivo(&format!("{id}.mixins.json"), mixins.as_bytes())
        .arquivo(
            &format!("assets/{id}/lang/en_us.json"),
            format!("{{\"item.{id}.coisa\": \"Coisa {n}\"}}").as_bytes(),
        )
        .arquivo(
            "META-INF/MANIFEST.MF",
            b"Manifest-Version: 1.0\r\nCreated-By: Warden A-04\r\n\r\n",
        );
    let raiz_classes = base.replace('.', "/");
    jar = jar.classe(&format!("{raiz_classes}/Principal.class"));
    for nome in ["MixinMundo", "MixinEntidade", "MixinTela"] {
        jar = jar.classe(&format!("{raiz_classes}/mixin/{nome}.class"));
    }
    for p in 0..pacotes {
        for c in 0..classes_por_pacote {
            jar = jar.classe(&format!("{raiz_classes}/p{p:03}/Classe{c:02}.class"));
        }
    }
    for (k, path) in embutidos.iter().enumerate() {
        jar = jar.guardado(path, &jar_embutido(&id, k));
    }
    let Some(faixa) = faixa else {
        return jar.montar();
    };
    // O resto das entradas são recursos (modelos, texturas, receitas), como nos jars reais.
    for k in jar.entradas.len()..faixa.entradas {
        jar = jar.arquivo(
            &format!("assets/{id}/models/item/m{k:05}.json"),
            format!("{{\"parent\": \"item/generated\", \"textures\": {{\"layer0\": \"{id}:item/m{k}\"}}}}")
                .as_bytes(),
        );
    }
    // E um arquivo de dados sem compressão completa o tamanho.
    let sem_dados = jar.clone().montar().len();
    let alvo = faixa.kib << 10;
    if alvo > sem_dados + 256 {
        jar = jar.guardado(
            &format!("assets/{id}/textures/atlas.bin"),
            &dados(n, alvo - sem_dados - 256),
        );
    }
    jar.montar()
}

/// Bytes pseudoaleatórios (xorshift), que não comprimem: o jar fica com o tamanho pedido.
fn dados(semente: usize, tamanho: usize) -> Vec<u8> {
    let mut estado = (semente as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1;
    let mut bytes = Vec::with_capacity(tamanho + 8);
    while bytes.len() < tamanho {
        estado ^= estado << 13;
        estado ^= estado >> 7;
        estado ^= estado << 17;
        bytes.extend_from_slice(&estado.to_le_bytes());
    }
    bytes.truncate(tamanho);
    bytes
}

fn jar_embutido(dono: &str, k: usize) -> Vec<u8> {
    let id = format!("{dono}_modulo{k}");
    let base = format!("dev/warden/bench/{dono}/modulo{k}");
    let mut jar = Jar::default().arquivo(
        "fabric.mod.json",
        format!("{{\"schemaVersion\": 1, \"id\": \"{id}\", \"version\": \"1.0.0\"}}").as_bytes(),
    );
    for p in 0..5 {
        for c in 0..6 {
            jar = jar.classe(&format!("{base}/p{p}/Classe{c}.class"));
        }
    }
    jar.montar()
}

/// Monta um zip na memória.
#[derive(Clone, Default)]
struct Jar {
    entradas: Vec<(String, Vec<u8>, CompressionMethod)>,
}

impl Jar {
    fn arquivo(mut self, nome: &str, bytes: &[u8]) -> Self {
        self.entradas
            .push((nome.to_owned(), bytes.to_vec(), CompressionMethod::Deflated));
        self
    }

    fn guardado(mut self, nome: &str, bytes: &[u8]) -> Self {
        self.entradas
            .push((nome.to_owned(), bytes.to_vec(), CompressionMethod::Stored));
        self
    }

    /// Classe mínima: o cabeçalho com a versão 61 (Java 17).
    fn classe(self, nome: &str) -> Self {
        self.arquivo(nome, &[0xCA, 0xFE, 0xBA, 0xBE, 0, 0, 0, 61])
    }

    fn montar(self) -> Vec<u8> {
        let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
        for (nome, bytes, metodo) in self.entradas {
            let options = SimpleFileOptions::default()
                .compression_method(metodo)
                .compression_level((metodo == CompressionMethod::Deflated).then_some(1));
            writer.start_file(nome, options).expect("entrada do zip");
            writer.write_all(&bytes).expect("gravar no zip");
        }
        writer.finish().expect("fechar o zip").into_inner()
    }
}

/// Pasta do corpus de configs reais da `warden-configs`.
pub fn pasta_do_corpus() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("warden-configs")
        .join("tests")
        .join("corpus")
}

/// Os arquivos do corpus: (pasta do formato, nome, bytes), em ordem.
fn corpus() -> Vec<(String, String, Vec<u8>)> {
    let mut arquivos = Vec::new();
    for pasta in ["toml", "json", "json5", "properties", "cfg", "options"] {
        let dir = pasta_do_corpus().join(pasta);
        let mut nomes: Vec<PathBuf> = fs::read_dir(&dir)
            .unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
            .map(|entry| entry.expect("entrada").path())
            .filter(|path| path.is_file())
            .collect();
        nomes.sort();
        for path in nomes {
            let nome = path
                .file_name()
                .expect("nome")
                .to_string_lossy()
                .into_owned();
            if nome.starts_with('.') || path.extension().is_some_and(|e| e == "md") {
                continue;
            }
            arquivos.push((pasta.to_owned(), nome, fs::read(&path).expect("ler corpus")));
        }
    }
    arquivos
}

fn sha256(bytes: &[u8]) -> String {
    hex(&Sha256::digest(bytes))
}

fn hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        let _ = write!(out, "{byte:02x}");
    }
    out
}

fn toml_str(text: &str) -> String {
    format!("\"{}\"", text.replace('\\', "\\\\").replace('"', "\\\""))
}
