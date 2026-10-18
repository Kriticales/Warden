//! Pack **real** para as medições de desempenho (A-04; SPEC §8), além do gerado.
//!
//! Os benchmarks medem sempre o pack gerado ([`super::pack_grande`]); com a variável
//! `WARDEN_A04_PACK_REAL` apontando para uma pasta preparada por
//! `cargo bench -p warden-jarmeta --bench preparar_pack_real` (um `.mrpack` público do
//! Modrinth baixado só para a medida, nunca versionado), medem também esse pack. A pasta tem:
//!
//! - `modrinth.index.json`: o índice do `.mrpack`;
//! - `instancia/mods/<arquivo>.jar`: os jars dos mods do índice, conferidos pelo SHA-1;
//! - `instancia/config/**`: as configs dos `overrides/` do `.mrpack`.
//!
//! [`gerar`] monta com isso um pack no formato packwiz (um `.pw.toml` do Modrinth por mod, as
//! configs, `index.toml` e `pack.toml` com o loader do `.mrpack`), e os jars ficam onde estão,
//! como numa instância de teste já sincronizada. O passo a passo está no `docs/DESEMPENHO.md`.
//!
//! Incluído pelos benchmarks com `#[path = ".../pack_real.rs"] mod pack_real;`, junto do
//! `pack_grande.rs` (usa os tipos dele). Usa só `serde_json`, `sha2` e a biblioteca padrão.

#![allow(dead_code, unreachable_pub)] // Cada benchmark usa uma parte.

use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

use sha2::{Digest as _, Sha256};

use super::pack_grande::{ModGerado, PackGerado};

/// Variável com a pasta do pack real preparado.
pub const VARIAVEL: &str = "WARDEN_A04_PACK_REAL";

/// O pack real preparado.
#[derive(Debug, Clone)]
pub struct PackReal {
    /// A pasta preparada.
    pub pasta: PathBuf,
    /// Nome e versão do pack (do `modrinth.index.json`), para os rótulos das medidas.
    pub nome: String,
    /// Versão do Minecraft.
    pub minecraft: String,
    /// Loader (`fabric`, `quilt`, `forge` ou `neoforge`) e a versão dele.
    pub loader: (String, String),
    /// Os mods do índice: (caminho no `.mrpack`, SHA-512, URL de download, lado).
    mods: Vec<ModReal>,
}

#[derive(Debug, Clone)]
struct ModReal {
    arquivo: String,
    sha512: String,
    url: String,
    side: &'static str,
}

impl PackReal {
    /// O pack da variável [`VARIAVEL`], se ela foi definida. Pasta incompleta é erro: quem
    /// define a variável quer a medida.
    pub fn do_ambiente() -> Option<Self> {
        let pasta = PathBuf::from(std::env::var_os(VARIAVEL)?);
        Some(Self::abrir(&pasta))
    }

    /// Lê a pasta preparada.
    pub fn abrir(pasta: &Path) -> Self {
        let indice = pasta.join("modrinth.index.json");
        let texto = fs::read_to_string(&indice)
            .unwrap_or_else(|e| panic!("{VARIAVEL}: {}: {e}", indice.display()));
        let json: serde_json::Value = serde_json::from_str(&texto).expect("modrinth.index.json");
        let deps = &json["dependencies"];
        let minecraft = deps["minecraft"]
            .as_str()
            .expect("versão do Minecraft")
            .to_owned();
        let loader = ["fabric-loader", "quilt-loader", "forge", "neoforge"]
            .iter()
            .find_map(|chave| {
                deps[*chave].as_str().map(|versao| {
                    let nome = chave.trim_end_matches("-loader");
                    (nome.to_owned(), versao.to_owned())
                })
            })
            .expect("loader do .mrpack");
        let mut mods = Vec::new();
        for arquivo in json["files"].as_array().expect("files") {
            let caminho = arquivo["path"].as_str().expect("path");
            let Some(nome) = caminho.strip_prefix("mods/") else {
                continue;
            };
            let jar = Path::new(nome)
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("jar"));
            if !jar || nome.contains('/') {
                continue;
            }
            let env = &arquivo["env"];
            let side = match (env["client"].as_str(), env["server"].as_str()) {
                (_, Some("unsupported")) => "client",
                (Some("unsupported"), _) => "server",
                _ => "both",
            };
            mods.push(ModReal {
                arquivo: nome.to_owned(),
                sha512: arquivo["hashes"]["sha512"]
                    .as_str()
                    .expect("sha512")
                    .to_owned(),
                url: arquivo["downloads"][0]
                    .as_str()
                    .expect("download")
                    .to_owned(),
                side,
            });
        }
        let nome = format!(
            "{} {}",
            json["name"].as_str().unwrap_or("pack real"),
            json["versionId"].as_str().unwrap_or("")
        );
        let pack = Self {
            pasta: pasta.to_path_buf(),
            nome: nome.trim().to_owned(),
            minecraft,
            loader,
            mods,
        };
        for m in &pack.mods {
            let jar = pack.mods_dir().join(&m.arquivo);
            assert!(
                jar.is_file(),
                "{VARIAVEL}: falta {} (prepare de novo)",
                m.arquivo
            );
        }
        pack
    }

    /// Quantos mods (jars do índice).
    pub fn quantos_mods(&self) -> usize {
        self.mods.len()
    }

    /// A instância (com `mods/` e `config/`).
    pub fn instancia(&self) -> PathBuf {
        self.pasta.join("instancia")
    }

    fn mods_dir(&self) -> PathBuf {
        self.instancia().join("mods")
    }

    /// Rótulo das medidas: "Prominence II 4.1.4, 405 mods".
    pub fn rotulo(&self) -> String {
        format!("pack real {}, {} mods", self.nome, self.mods.len())
    }

    /// Monta o pack no formato packwiz em `raiz` (criada se não existir). Os jars não são
    /// copiados: [`ModGerado::jar`] aponta para a instância preparada.
    pub fn gerar(&self, raiz: &Path) -> PackGerado {
        fs::create_dir_all(raiz.join("mods")).expect("pasta mods");
        let mut indice: Vec<(String, String, bool)> = Vec::new();
        let mut mods = Vec::with_capacity(self.mods.len());
        let mut usados = std::collections::BTreeSet::new();
        for m in &self.mods {
            let haste = Path::new(&m.arquivo)
                .file_stem()
                .map_or_else(|| m.arquivo.clone(), |s| s.to_string_lossy().into_owned());
            let haste = haste.as_str();
            let mut id: String = haste
                .chars()
                .map(|c| {
                    if c.is_ascii_alphanumeric() {
                        c.to_ascii_lowercase()
                    } else {
                        '-'
                    }
                })
                .collect();
            while !usados.insert(id.clone()) {
                id.push('-');
            }
            // https://cdn.modrinth.com/data/<projeto>/versions/<versão>/<arquivo>
            let partes: Vec<&str> = m.url.split('/').collect();
            let projeto = partes.get(4).copied().unwrap_or("desconhecido").to_owned();
            let versao = partes.get(6).copied().unwrap_or("desconhecida");
            let metafile = format!("mods/{id}.pw.toml");
            let texto = format!(
                "name = {}\nfilename = {}\nside = \"{}\"\n\n[download]\nurl = {}\n\
                 hash-format = \"sha512\"\nhash = \"{}\"\n\n[update]\n[update.modrinth]\n\
                 mod-id = {}\nversion = {}\n",
                toml_str(haste),
                toml_str(&m.arquivo),
                m.side,
                toml_str(&m.url),
                m.sha512,
                toml_str(&projeto),
                toml_str(versao),
            );
            fs::write(raiz.join(&metafile), &texto).expect("gravar metafile");
            indice.push((metafile.clone(), sha256(texto.as_bytes()), true));
            mods.push(ModGerado {
                id,
                metafile,
                arquivo: m.arquivo.clone(),
                projeto,
                jar: Some(self.mods_dir().join(&m.arquivo)),
            });
        }
        let configs = self.copiar_configs(raiz, &mut indice);
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
            "name = {}\nauthor = \"Warden\"\nversion = \"1.0.0\"\n\
             pack-format = \"packwiz:1.1.0\"\n\n[index]\nfile = \"index.toml\"\n\
             hash-format = \"sha256\"\nhash = \"{}\"\n\n[versions]\nminecraft = {}\n{} = {}\n",
            toml_str(&self.nome),
            sha256(index.as_bytes()),
            toml_str(&self.minecraft),
            self.loader.0,
            toml_str(&self.loader.1),
        );
        fs::write(raiz.join("pack.toml"), pack).expect("gravar pack.toml");
        PackGerado {
            raiz: raiz.to_path_buf(),
            mods,
            configs,
        }
    }

    /// Copia as configs da instância para `raiz/config/` e as acrescenta ao índice.
    fn copiar_configs(&self, raiz: &Path, indice: &mut Vec<(String, String, bool)>) -> Vec<String> {
        let mut configs = Vec::new();
        let origem = self.instancia().join("config");
        let mut pilha = vec![origem.clone()];
        while let Some(dir) = pilha.pop() {
            let Ok(entradas) = fs::read_dir(&dir) else {
                continue;
            };
            for entrada in entradas {
                let path = entrada.expect("entrada").path();
                if path.is_dir() {
                    pilha.push(path);
                    continue;
                }
                let relativo = path.strip_prefix(&origem).expect("dentro de config");
                let relativo = format!("config/{}", relativo.to_string_lossy().replace('\\', "/"));
                let bytes = fs::read(&path).expect("ler config");
                let destino = raiz.join(&relativo);
                fs::create_dir_all(destino.parent().expect("pai")).expect("pasta da config");
                fs::write(&destino, &bytes).expect("gravar config");
                indice.push((relativo.clone(), sha256(&bytes), false));
                configs.push(relativo);
            }
        }
        configs.sort();
        configs
    }
}

fn sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .fold(String::with_capacity(64), |mut out, byte| {
            let _ = write!(out, "{byte:02x}");
            out
        })
}

fn toml_str(text: &str) -> String {
    format!("\"{}\"", text.replace('\\', "\\\\").replace('"', "\\\""))
}
