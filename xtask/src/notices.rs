//! `cargo xtask notices`: gera os avisos de terceiros exibidos em "Sobre o Warden" (A-02;
//! ADR-0004; SPEC T23).
//!
//! Junta, num JSON lido pela interface (`features/about/licenses/generated/avisos.json`, fora
//! do git), tudo o que vai dentro do app instalado:
//!
//! - **Rust:** as crates ligadas ao executável do app (sem as de build e de teste), pelo
//!   `cargo-about` em modo offline, com as licenças aceitas lidas do `deny.toml`. O motor do
//!   launcher (`portablemc`) entra por aqui, como dependência da `warden-launcher`.
//! - **JavaScript:** os pacotes de produção do frontend (`pnpm licenses list --prod`), com o
//!   texto lido da pasta de cada pacote.
//! - **packwiz:** o próprio packwiz (`third_party/packwiz/LICENSE`), a biblioteca padrão do Go e
//!   os módulos Go compilados no sidecar (`go version -m`), com o texto lido do cache de
//!   módulos do Go.
//! - **Fontes:** as fontes embutidas (`design/system/fonts/OFL-*.txt`).
//!
//! Textos iguais são guardados uma vez só. O JSON não leva nenhum caminho local.
//!
//! Antes de gravar, confere a lista ([`verify`]) e falha se: algum componente ficou sem licença
//! (nem expressão SPDX nem texto); faltar um dos componentes cujo crédito aparece em Sobre
//! ([`REQUIRED`]); alguma dependência direta do app (Rust ou npm) não estiver nos avisos ou não
//! estiver registrada no `THIRD_PARTY.md` com a mesma licença; o packwiz ou uma fonte embutida
//! não estiver no `THIRD_PARTY.md`. O `cargo xtask check` roda esta tarefa.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context as _, Result, bail};
use cargo_metadata::{DependencyKind, Metadata};
use serde::{Deserialize, Serialize};

use crate::deps;
use crate::packwiz::{Layout, TARGETS, host_target};
use crate::setup::installed;
use crate::util::{Cmd, desktop_dir, find_program, workspace_root};

/// Versão fixada do `cargo-about` (o binário só existe com a feature `cli`).
pub const CARGO_ABOUT: (&str, &str) = ("cargo-about", "0.9.2");

/// Versão do formato do JSON (a interface confere).
pub const SCHEMA: u32 = 1;

/// Fontes embutidas: arquivo da licença em `design/system/fonts/` e nome exibido.
const FONTS: &[(&str, &str)] = &[
    ("OFL-IBMPlexMono.txt", "IBM Plex Mono"),
    ("OFL-Manrope.txt", "Manrope"),
    ("OFL-PixelifySans.txt", "Pixelify Sans"),
];

/// Saída lida pela interface.
pub fn output_path() -> PathBuf {
    desktop_dir()
        .join("src")
        .join("features")
        .join("about")
        .join("licenses")
        .join("generated")
        .join("avisos.json")
}

// ---------------------------------------------------------------------------------------------
// Formato da saída

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct Notices {
    pub schema: u32,
    pub groups: Vec<Group>,
    /// Textos de licença sem repetição; os itens apontam pelo índice.
    pub texts: Vec<String>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct Group {
    /// `rust`, `npm`, `packwiz` ou `fontes` (a interface traduz).
    pub id: &'static str,
    pub items: Vec<Item>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct Item {
    pub name: String,
    pub version: String,
    /// Expressão SPDX; vazia quando não foi possível reconhecer (o texto continua lá).
    pub license: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    pub texts: Vec<usize>,
}

/// Monta o JSON, guardando cada texto uma vez só.
#[derive(Default)]
pub struct Builder {
    groups: Vec<Group>,
    texts: Vec<String>,
    index: HashMap<String, usize>,
}

impl Builder {
    /// Índice do texto (normalizado: fim de linha LF, sem espaços nas pontas).
    pub fn text(&mut self, text: &str) -> usize {
        let normalized = text.replace("\r\n", "\n").trim().to_owned();
        if let Some(&index) = self.index.get(&normalized) {
            return index;
        }
        let index = self.texts.len();
        self.texts.push(normalized.clone());
        self.index.insert(normalized, index);
        index
    }

    /// Acrescenta um grupo; os itens ficam em ordem de nome (sem diferenciar maiúsculas) e
    /// versão, menos os `fixed` primeiros, que ficam no topo na ordem dada.
    pub fn group(&mut self, id: &'static str, mut items: Vec<Item>, fixed: usize) {
        let fixed = fixed.min(items.len());
        items[fixed..].sort_by(|a, b| {
            (a.name.to_lowercase(), &a.version).cmp(&(b.name.to_lowercase(), &b.version))
        });
        items.dedup_by(|a, b| a.name == b.name && a.version == b.version);
        self.groups.push(Group { id, items });
    }

    pub fn finish(self) -> Notices {
        Notices {
            schema: SCHEMA,
            groups: self.groups,
            texts: self.texts,
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Rust (cargo-about)

#[derive(Deserialize)]
pub struct AboutOutput {
    licenses: Vec<AboutLicense>,
    crates: Vec<AboutCrate>,
}

#[derive(Deserialize)]
struct AboutLicense {
    text: String,
    used_by: Vec<AboutUse>,
}

#[derive(Deserialize)]
struct AboutUse {
    #[serde(rename = "crate")]
    krate: AboutPackage,
}

#[derive(Deserialize)]
struct AboutCrate {
    package: AboutPackage,
    license: String,
}

#[derive(Deserialize)]
struct AboutPackage {
    id: String,
    name: String,
    version: String,
    repository: Option<String>,
    homepage: Option<String>,
}

/// Converte a saída JSON do `cargo-about` em itens.
pub fn rust_items(builder: &mut Builder, about: &AboutOutput) -> Vec<Item> {
    let mut texts_of: HashMap<&str, Vec<usize>> = HashMap::new();
    for license in &about.licenses {
        let index = builder.text(&license.text);
        for usage in &license.used_by {
            let list = texts_of.entry(usage.krate.id.as_str()).or_default();
            if !list.contains(&index) {
                list.push(index);
            }
        }
    }
    about
        .crates
        .iter()
        .map(|krate| Item {
            name: krate.package.name.clone(),
            version: krate.package.version.clone(),
            license: krate.license.clone(),
            url: krate
                .package
                .repository
                .clone()
                .or_else(|| krate.package.homepage.clone()),
            texts: texts_of
                .get(krate.package.id.as_str())
                .cloned()
                .unwrap_or_default(),
        })
        .collect()
}

/// Licenças aceitas (`[licenses] allow` do `deny.toml`), as mesmas que o `cargo deny` aceita.
pub fn accepted_licenses(deny_toml: &str) -> Result<Vec<String>> {
    let value: toml::Table = toml::from_str(deny_toml).context("deny.toml inválido")?;
    let allow = value
        .get("licenses")
        .and_then(|licenses| licenses.get("allow"))
        .and_then(toml::Value::as_array)
        .context("deny.toml sem [licenses] allow")?;
    allow
        .iter()
        .map(|item| {
            item.as_str()
                .map(str::to_owned)
                .context("licença do deny.toml não é texto")
        })
        .collect()
}

/// Configuração do `cargo-about`: só o que é ligado ao executável, nos alvos do app.
pub fn about_config(accepted: &[String]) -> String {
    let mut table = toml::Table::new();
    table.insert(
        "accepted".into(),
        toml::Value::Array(accepted.iter().cloned().map(toml::Value::String).collect()),
    );
    table.insert(
        "targets".into(),
        toml::Value::Array(
            TARGETS
                .iter()
                .map(|target| toml::Value::String(target.triple.to_owned()))
                .collect(),
        ),
    );
    table.insert("ignore-build-dependencies".into(), true.into());
    table.insert("ignore-dev-dependencies".into(), true.into());
    let mut private = toml::Table::new();
    private.insert("ignore".into(), true.into());
    table.insert("private".into(), toml::Value::Table(private));
    toml::to_string(&table).expect("tabela TOML simples")
}

fn ensure_cargo_about() -> Result<()> {
    let (name, version) = CARGO_ABOUT;
    let list = Cmd::cargo().args(["install", "--list"]).read()?;
    if installed(&list)
        .iter()
        .any(|(n, v)| n == name && v == version)
    {
        return Ok(());
    }
    Cmd::cargo()
        .args([
            "install",
            name,
            "--version",
            version,
            "--locked",
            "--features",
            "cli",
        ])
        .run()
}

fn rust(builder: &mut Builder) -> Result<()> {
    ensure_cargo_about()?;
    let root = workspace_root();
    let accepted = accepted_licenses(
        &fs::read_to_string(root.join("deny.toml")).context("falha ao ler o deny.toml")?,
    )?;
    let dir = tempfile::tempdir().context("falha ao criar pasta temporária")?;
    let config = dir.path().join("about.toml");
    fs::write(&config, about_config(&accepted))?;
    let manifest = desktop_dir().join("src-tauri").join("Cargo.toml");
    // Com a saída redirecionada, o cargo-about recusa rodar no PowerShell: grava em arquivo.
    let output = dir.path().join("about.json");
    Cmd::cargo()
        .args([
            "about",
            "generate",
            "--format",
            "json",
            "--offline",
            "--locked",
        ])
        .args(["-c"])
        .args([&config])
        .args(["-m"])
        .args([&manifest])
        .args(["-o"])
        .args([&output])
        .read()?;
    let json = fs::read_to_string(&output).context("o cargo-about não gravou a saída")?;
    let about: AboutOutput =
        serde_json::from_str(&json).context("saída inesperada do cargo-about")?;
    let items = rust_items(builder, &about);
    if items.is_empty() {
        bail!("o cargo-about não listou nenhuma crate");
    }
    builder.group("rust", items, 0);
    Ok(())
}

// ---------------------------------------------------------------------------------------------
// JavaScript (pnpm)

#[derive(Deserialize)]
struct PnpmPackage {
    name: String,
    versions: Vec<String>,
    paths: Vec<PathBuf>,
    license: String,
    homepage: Option<String>,
}

/// Arquivos de licença de uma pasta (LICENSE, LICENCE, UNLICENSE, COPYING, NOTICE, com ou sem
/// extensão).
pub fn license_files(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut files: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_file()))
        .map(|entry| entry.path())
        .filter(|path| {
            let name = path
                .file_name()
                .map(|name| name.to_string_lossy().to_uppercase())
                .unwrap_or_default();
            let stem = name.split(['.', '-', '_']).next().unwrap_or_default();
            matches!(
                stem,
                "LICENSE" | "LICENCE" | "UNLICENSE" | "COPYING" | "NOTICE"
            )
        })
        .collect();
    files.sort();
    files
}

fn texts_in(builder: &mut Builder, dir: &Path) -> Vec<usize> {
    let mut indices = Vec::new();
    for file in license_files(dir) {
        if let Ok(text) = fs::read_to_string(&file)
            && !text.trim().is_empty()
        {
            let index = builder.text(&text);
            if !indices.contains(&index) {
                indices.push(index);
            }
        }
    }
    indices
}

/// Converte a saída de `pnpm licenses list --json` em itens, lendo os textos das pastas.
pub fn npm_items(builder: &mut Builder, json: &str) -> Result<Vec<Item>> {
    let by_license: HashMap<String, Vec<PnpmPackage>> =
        serde_json::from_str(json).context("saída inesperada do pnpm licenses")?;
    let mut items = Vec::new();
    for package in by_license.into_values().flatten() {
        for (position, version) in package.versions.iter().enumerate() {
            let texts = package
                .paths
                .get(position)
                .map(|dir| texts_in(builder, dir))
                .unwrap_or_default();
            items.push(Item {
                name: package.name.clone(),
                version: version.clone(),
                license: if package.license == "Unknown" {
                    String::new()
                } else {
                    package.license.clone()
                },
                url: package.homepage.clone(),
                texts,
            });
        }
    }
    Ok(items)
}

fn npm(builder: &mut Builder) -> Result<()> {
    let json = Cmd::pnpm()?
        .cwd(desktop_dir())
        .args(["licenses", "list", "--json", "--prod"])
        .read()?;
    let items = npm_items(builder, &json)?;
    if items.is_empty() {
        bail!("o pnpm não listou nenhum pacote; rode `cargo xtask setup`");
    }
    builder.group("npm", items, 0);
    Ok(())
}

// ---------------------------------------------------------------------------------------------
// packwiz e Go

/// Módulo Go compilado num executável.
#[derive(Debug, PartialEq, Eq)]
pub struct GoModule {
    pub path: String,
    pub version: String,
}

/// Lê `go version -m`: a versão do Go e os módulos `dep` (ou o substituto `=>`, quando houver).
pub fn parse_go_version_m(text: &str) -> (Option<String>, Vec<GoModule>) {
    let mut go = None;
    let mut modules: Vec<GoModule> = Vec::new();
    for line in text.lines() {
        let fields: Vec<&str> = line.split('\t').map(str::trim).collect();
        match fields.as_slice() {
            [first, ..] if first.ends_with(char::is_alphanumeric) && first.contains(": go") => {
                go = first.rsplit(": ").next().map(str::to_owned);
            }
            [_, "dep", path, version, ..] => modules.push(GoModule {
                path: (*path).to_owned(),
                version: (*version).to_owned(),
            }),
            [_, "=>", path, version, ..] => {
                if let Some(last) = modules.last_mut() {
                    (*path).clone_into(&mut last.path);
                    (*version).clone_into(&mut last.version);
                }
            }
            _ => {}
        }
    }
    (go, modules)
}

/// Caminho do módulo no cache do Go: maiúsculas viram `!` + minúscula.
pub fn escape_module_path(path: &str) -> String {
    let mut out = String::with_capacity(path.len());
    for c in path.chars() {
        if c.is_ascii_uppercase() {
            out.push('!');
            out.push(c.to_ascii_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}

/// Reconhece as licenças mais comuns pelo texto (os módulos Go não declaram licença).
pub fn guess_spdx(text: &str) -> Option<&'static str> {
    let flat = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let has = |needle: &str| flat.contains(needle);
    if has("Apache License") && has("Version 2.0") {
        Some("Apache-2.0")
    } else if has("Mozilla Public License") && (has("Version 2.0") || has("version 2.0")) {
        Some("MPL-2.0")
    } else if has("Permission is hereby granted, free of charge") {
        Some("MIT")
    } else if has("Redistribution and use in source and binary forms") {
        if has("Neither the name") || has("names of its contributors") {
            Some("BSD-3-Clause")
        } else {
            Some("BSD-2-Clause")
        }
    } else if has("Permission to use, copy, modify, and/or distribute") {
        if has("provided that the above copyright notice") {
            Some("ISC")
        } else {
            Some("0BSD")
        }
    } else if has("This is free and unencumbered software released into the public domain") {
        Some("Unlicense")
    } else if has("CC0 1.0 Universal") {
        Some("CC0-1.0")
    } else {
        None
    }
}

/// Seção "License" de um README em Markdown (módulos que não têm arquivo de licença), sem as
/// cercas de código.
pub fn readme_license_section(readme: &str) -> Option<String> {
    let heading_level = |line: &str| {
        let level = line.chars().take_while(|&c| c == '#').count();
        (level > 0 && line[level..].starts_with(' ')).then_some(level)
    };
    let mut lines = readme.lines();
    let level = lines.by_ref().find_map(|line| {
        let level = heading_level(line)?;
        let title = line[level..].trim().to_lowercase();
        (title.starts_with("license") || title.starts_with("licence")).then_some(level)
    })?;
    let body: Vec<&str> = lines
        .take_while(|line| heading_level(line).is_none_or(|other| other > level))
        .filter(|line| !line.trim_start().starts_with("```"))
        .collect();
    let text = body
        .join(
            "
",
        )
        .trim()
        .to_owned();
    (!text.is_empty()).then_some(text)
}

fn item_from_dir(builder: &mut Builder, name: &str, version: &str, dir: &Path) -> Item {
    let texts = texts_in(builder, dir);
    let license = texts
        .first()
        .and_then(|&index| guess_spdx(&builder.texts[index]))
        .unwrap_or_default()
        .to_owned();
    Item {
        name: name.to_owned(),
        version: version.to_owned(),
        license,
        url: None,
        texts,
    }
}

fn go_env(name: &str) -> Result<PathBuf> {
    let value = Cmd::new(find_program("go")?)
        .args(["env", name])
        .read()?
        .trim()
        .to_owned();
    if value.is_empty() {
        bail!("`go env {name}` vazio");
    }
    Ok(PathBuf::from(value))
}

fn packwiz(builder: &mut Builder) -> Result<()> {
    let layout = Layout::current()?;
    // Prefere o executável do Windows (o do instalador); os módulos são os mesmos nos dois.
    let binary = TARGETS
        .iter()
        .map(|target| layout.binary(target))
        .find(|path| path.is_file())
        .or_else(|| host_target().map(|target| layout.binary(target)))
        .context("alvo sem sidecar do packwiz")?;
    if !binary.is_file() {
        bail!("o sidecar do packwiz não existe; rode `cargo xtask build-packwiz`");
    }
    let go = find_program("go")?;
    let listing = Cmd::new(&go)
        .args(["version", "-m"])
        .args([&binary])
        .read()?;
    let (go_version, modules) = parse_go_version_m(&listing);
    let commit = fs::read_to_string(layout.commit_file())
        .map(|text| text.trim().chars().take(12).collect::<String>())
        .unwrap_or_default();

    let mut items = Vec::new();
    let mut packwiz = item_from_dir(builder, "packwiz", &commit, &layout.third_party);
    packwiz.url = Some("https://github.com/packwiz/packwiz".to_owned());
    items.push(packwiz);
    let goroot = go_env("GOROOT")?;
    let mut stdlib = item_from_dir(
        builder,
        "Go (biblioteca padrão)",
        go_version.as_deref().unwrap_or_default(),
        &goroot,
    );
    stdlib.url = Some("https://go.dev".to_owned());
    items.push(stdlib);

    let cache = go_env("GOMODCACHE")?;
    for module in modules {
        let dir = cache.join(format!(
            "{}@{}",
            escape_module_path(&module.path),
            module.version
        ));
        if !dir.is_dir() {
            // Cache do Go limpo (ou restaurado pela metade na CI): baixa só este módulo.
            Cmd::new(&go)
                .args(["mod", "download", "-x"])
                .args([format!("{}@{}", module.path, module.version)])
                .cwd(std::env::temp_dir())
                .env("GOFLAGS", "-mod=mod")
                .run()?;
            if !dir.is_dir() {
                bail!(
                    "o módulo Go {}@{} não foi encontrado em {}",
                    module.path,
                    module.version,
                    cache.display()
                );
            }
        }
        let mut item = item_from_dir(builder, &module.path, &module.version, &dir);
        if item.texts.is_empty()
            && let Some(section) = fs::read_to_string(dir.join("README.md"))
                .ok()
                .as_deref()
                .and_then(readme_license_section)
        {
            guess_spdx(&section)
                .unwrap_or_default()
                .clone_into(&mut item.license);
            item.texts.push(builder.text(&section));
        }
        if item.texts.is_empty() {
            bail!(
                "o módulo Go {}@{} não tem arquivo de licença",
                module.path,
                module.version
            );
        }
        item.url = Some(format!("https://pkg.go.dev/{}", module.path));
        items.push(item);
    }
    builder.group("packwiz", items, 2);
    Ok(())
}

// ---------------------------------------------------------------------------------------------
// Fontes

fn fonts(builder: &mut Builder) -> Result<()> {
    let dir = workspace_root().join("design").join("system").join("fonts");
    let mut items = Vec::new();
    for (file, name) in FONTS {
        let text = fs::read_to_string(dir.join(file))
            .with_context(|| format!("falha ao ler design/system/fonts/{file}"))?;
        items.push(Item {
            name: (*name).to_owned(),
            version: String::new(),
            license: "OFL-1.1".to_owned(),
            url: None,
            texts: vec![builder.text(&text)],
        });
    }
    builder.group("fontes", items, 0);
    Ok(())
}

// ---------------------------------------------------------------------------------------------
// Conferência

/// Componentes que precisam estar nos avisos: os créditos de Sobre dependem deles (o do motor do
/// launcher só aparece quando a crate está na lista) e, sem eles, a geração saiu incompleta.
pub const REQUIRED: &[(&str, &str)] = &[
    ("rust", "tauri"),
    ("rust", "portablemc"),
    ("npm", "react"),
    ("packwiz", "packwiz"),
];

/// Pacote do app (a `warden-app`): raiz das dependências que vão no executável.
const APP_PACKAGE: &str = "warden-app";

/// Dependências diretas que vão dentro do app, por grupo dos avisos.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Direct {
    /// Dependências normais das crates do workspace ligadas à `warden-app` (sem as do próprio
    /// workspace).
    pub rust: BTreeSet<String>,
    /// `dependencies` do `apps/desktop/package.json`.
    pub npm: BTreeSet<String>,
}

/// Dependências normais (não as de build nem as de teste) das crates do workspace alcançáveis a
/// partir da `warden-app`.
pub fn direct_rust_dependencies(metadata: &Metadata) -> BTreeSet<String> {
    let members: BTreeMap<&str, &cargo_metadata::Package> = metadata
        .workspace_packages()
        .into_iter()
        .map(|package| (package.name.as_str(), package))
        .collect();
    let mut seen = BTreeSet::new();
    let mut pending = vec![APP_PACKAGE];
    let mut external = BTreeSet::new();
    while let Some(name) = pending.pop() {
        if !seen.insert(name) {
            continue;
        }
        let Some(package) = members.get(name) else {
            continue;
        };
        for dependency in &package.dependencies {
            if dependency.kind != DependencyKind::Normal {
                continue;
            }
            if let Some((member, _)) = members.get_key_value(dependency.name.as_str()) {
                pending.push(member);
            } else {
                external.insert(dependency.name.clone());
            }
        }
    }
    external
}

/// `dependencies` de um `package.json`.
pub fn direct_npm_dependencies(package_json: &str) -> Result<BTreeSet<String>> {
    let value: serde_json::Value =
        serde_json::from_str(package_json).context("package.json inválido")?;
    Ok(value
        .get("dependencies")
        .and_then(serde_json::Value::as_object)
        .map(|deps| deps.keys().cloned().collect())
        .unwrap_or_default())
}

/// Componentes registrados numa seção do `THIRD_PARTY.md` (a que começa com `## <titulo>`):
/// cada nome entre crases da primeira coluna de uma tabela → a coluna "Licença" da linha.
pub fn registered(markdown: &str, title: &str) -> BTreeMap<String, String> {
    let mut found = BTreeMap::new();
    let mut inside = false;
    let mut license_column = None;
    for line in markdown.lines() {
        if let Some(heading) = line.strip_prefix("## ") {
            inside = heading.starts_with(title);
            license_column = None;
            continue;
        }
        if !inside || !line.starts_with('|') {
            continue;
        }
        let cells: Vec<&str> = line
            .trim()
            .trim_matches('|')
            .split('|')
            .map(str::trim)
            .collect();
        let Some(column) = license_column else {
            license_column = cells.iter().position(|cell| *cell == "Licença");
            continue;
        };
        if cells
            .iter()
            .all(|cell| cell.chars().all(|c| c == '-' || c == ':'))
        {
            continue;
        }
        let license = cells.get(column).copied().unwrap_or_default();
        for name in cells[0].split('`').skip(1).step_by(2) {
            found
                .entry(name.to_owned())
                .or_insert_with(|| license.to_owned());
        }
    }
    found
}

/// Identificadores de uma expressão SPDX, sem os operadores e sem a ordem: `(MIT OR
/// Apache-2.0)` e `Apache-2.0 OR MIT` dão o mesmo conjunto.
pub fn license_ids(expression: &str) -> BTreeSet<String> {
    expression
        .split(|c: char| c.is_whitespace() || c == '(' || c == ')')
        .filter(|word| !word.is_empty() && !matches!(*word, "OR" | "AND" | "WITH"))
        .map(str::to_owned)
        .collect()
}

/// Problemas da lista gerada, uma frase por problema (vazia: tudo certo).
pub fn verify(notices: &Notices, direct: &Direct, third_party: &str) -> Vec<String> {
    let mut problems = Vec::new();
    let group = |id: &str| notices.groups.iter().find(|group| group.id == id);

    for group in &notices.groups {
        for item in &group.items {
            if item.license.trim().is_empty() && item.texts.is_empty() {
                problems.push(format!(
                    "{} {}@{}: sem licença (nem expressão SPDX nem texto)",
                    group.id, item.name, item.version
                ));
            }
        }
    }

    for (id, name) in REQUIRED {
        if !group(id).is_some_and(|group| group.items.iter().any(|item| item.name == *name)) {
            problems.push(format!("{id} {name}: faltou nos avisos"));
        }
    }

    for (id, names, title) in [
        ("rust", &direct.rust, "Crates Rust"),
        ("npm", &direct.npm, "Pacotes npm"),
    ] {
        let rows = registered(third_party, title);
        let items: Vec<&Item> = group(id)
            .map(|group| group.items.iter().collect())
            .unwrap_or_default();
        for name in names {
            let versions: Vec<&&Item> = items.iter().filter(|item| item.name == *name).collect();
            if versions.is_empty() {
                problems.push(format!(
                    "{id} {name}: dependência direta do app fora dos avisos"
                ));
            }
            let Some(license) = rows.get(name) else {
                problems.push(format!(
                    "{id} {name}: dependência direta do app sem linha em \"{title}\" no THIRD_PARTY.md"
                ));
                continue;
            };
            let expected = license_ids(license);
            if !versions.is_empty()
                && !versions
                    .iter()
                    .any(|item| license_ids(&item.license) == expected)
            {
                let found: Vec<&str> = versions.iter().map(|item| item.license.as_str()).collect();
                problems.push(format!(
                    "{id} {name}: licença no THIRD_PARTY.md ({license}) diferente da detectada ({})",
                    found.join("; ")
                ));
            }
        }
    }

    if !third_party.contains("packwiz/packwiz") {
        problems.push("packwiz: sem registro no THIRD_PARTY.md".to_owned());
    }
    let fonts = registered(third_party, "Fontes");
    for (_, name) in FONTS {
        if !fonts.contains_key(*name) {
            problems.push(format!(
                "fontes {name}: sem linha em \"Fontes\" no THIRD_PARTY.md"
            ));
        }
    }
    problems
}

/// Dependências diretas do app, lidas do workspace.
fn direct() -> Result<Direct> {
    let metadata = deps::metadata(&workspace_root().join("Cargo.toml"))?;
    let package_json = fs::read_to_string(desktop_dir().join("package.json"))
        .context("falha ao ler apps/desktop/package.json")?;
    Ok(Direct {
        rust: direct_rust_dependencies(&metadata),
        npm: direct_npm_dependencies(&package_json)?,
    })
}

// ---------------------------------------------------------------------------------------------

/// `cargo xtask notices`.
pub fn run() -> Result<()> {
    let mut builder = Builder::default();
    rust(&mut builder)?;
    npm(&mut builder)?;
    packwiz(&mut builder)?;
    fonts(&mut builder)?;
    let notices = builder.finish();
    let third_party = fs::read_to_string(workspace_root().join("THIRD_PARTY.md"))
        .context("falha ao ler o THIRD_PARTY.md")?;
    let problems = verify(&notices, &direct()?, &third_party);
    if !problems.is_empty() {
        bail!(
            "avisos de terceiros incompletos ({}):\n  - {}",
            problems.len(),
            problems.join("\n  - ")
        );
    }
    let json = serde_json::to_string(&notices)?;
    let path = output_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&path, json.as_bytes())
        .with_context(|| format!("falha ao gravar {}", path.display()))?;
    for group in &notices.groups {
        println!("notices: {} — {} itens", group.id, group.items.len());
    }
    println!(
        "notices: {} textos de licença, {} KiB em {}",
        notices.texts.len(),
        json.len() / 1024,
        path.strip_prefix(workspace_root())
            .unwrap_or(&path)
            .display()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn textos_iguais_sao_guardados_uma_vez() {
        let mut builder = Builder::default();
        let a = builder.text("MIT License\r\n\r\nPermission...\r\n");
        let b = builder.text("  MIT License\n\nPermission...");
        let c = builder.text("Apache License");
        assert_eq!(a, b);
        assert_ne!(a, c);
        assert_eq!(
            builder.finish().texts,
            ["MIT License\n\nPermission...", "Apache License"]
        );
    }

    fn item(name: &str, version: &str) -> Item {
        Item {
            name: name.into(),
            version: version.into(),
            license: "MIT".into(),
            url: None,
            texts: vec![],
        }
    }

    #[test]
    fn grupo_ordena_sem_maiusculas_mantem_os_fixos_e_tira_repetidos() {
        let mut builder = Builder::default();
        builder.group(
            "packwiz",
            vec![
                item("packwiz", "abc"),
                item("zeta", "1"),
                item("Alpha", "2"),
                item("alpha", "1"),
                item("zeta", "1"),
            ],
            1,
        );
        let names: Vec<_> = builder.finish().groups[0]
            .items
            .iter()
            .map(|item| format!("{}@{}", item.name, item.version))
            .collect();
        assert_eq!(names, ["packwiz@abc", "alpha@1", "Alpha@2", "zeta@1"]);
    }

    #[test]
    fn converte_a_saida_do_cargo_about() {
        let json = r#"{
          "overview": [],
          "licenses": [
            {"name": "MIT License", "id": "MIT", "text": "MIT texto", "used_by": [
              {"crate": {"id": "a@1", "name": "a", "version": "1.0.0", "repository": "https://ex/a", "homepage": null}},
              {"crate": {"id": "b@2", "name": "b", "version": "2.0.0", "repository": null, "homepage": "https://b"}}
            ]},
            {"name": "Apache", "id": "Apache-2.0", "text": "Apache texto", "used_by": [
              {"crate": {"id": "a@1", "name": "a", "version": "1.0.0", "repository": "https://ex/a", "homepage": null}}
            ]}
          ],
          "crates": [
            {"package": {"id": "a@1", "name": "a", "version": "1.0.0", "repository": "https://ex/a", "homepage": null}, "license": "MIT AND Apache-2.0"},
            {"package": {"id": "b@2", "name": "b", "version": "2.0.0", "repository": null, "homepage": "https://b"}, "license": "MIT"}
          ]
        }"#;
        let about: AboutOutput = serde_json::from_str(json).unwrap();
        let mut builder = Builder::default();
        let items = rust_items(&mut builder, &about);
        assert_eq!(
            items,
            [
                Item {
                    name: "a".into(),
                    version: "1.0.0".into(),
                    license: "MIT AND Apache-2.0".into(),
                    url: Some("https://ex/a".into()),
                    texts: vec![0, 1],
                },
                Item {
                    name: "b".into(),
                    version: "2.0.0".into(),
                    license: "MIT".into(),
                    url: Some("https://b".into()),
                    texts: vec![0],
                },
            ]
        );
    }

    #[test]
    fn licencas_aceitas_vem_do_deny_toml_do_repositorio() {
        let deny = fs::read_to_string(workspace_root().join("deny.toml")).unwrap();
        let accepted = accepted_licenses(&deny).unwrap();
        assert!(accepted.iter().any(|license| license == "MIT"));
        assert!(accepted.iter().any(|license| license == "Apache-2.0"));
        let config: toml::Table = toml::from_str(&about_config(&accepted)).unwrap();
        assert_eq!(config["ignore-dev-dependencies"].as_bool(), Some(true));
        assert_eq!(config["ignore-build-dependencies"].as_bool(), Some(true));
        assert_eq!(config["private"]["ignore"].as_bool(), Some(true));
        assert_eq!(config["targets"].as_array().unwrap().len(), TARGETS.len());
        assert!(accepted_licenses("[bans]\n").is_err());
    }

    #[test]
    fn converte_a_saida_do_pnpm_lendo_os_textos() {
        let dir = tempfile::tempdir().unwrap();
        let pkg = dir.path().join("react");
        fs::create_dir_all(&pkg).unwrap();
        fs::write(pkg.join("LICENSE"), "MIT License react").unwrap();
        fs::write(pkg.join("package.json"), "{}").unwrap();
        let json = serde_json::json!({
            "MIT": [{"name": "react", "versions": ["19.3.0"], "paths": [pkg], "license": "MIT", "homepage": "https://react.dev"}],
            "Unknown": [{"name": "sem-licenca", "versions": ["1.0.0"], "paths": [dir.path().join("nao-existe")], "license": "Unknown"}]
        })
        .to_string();
        let mut builder = Builder::default();
        let mut items = npm_items(&mut builder, &json).unwrap();
        items.sort_by(|a, b| a.name.cmp(&b.name));
        assert_eq!(items[0].name, "react");
        assert_eq!(items[0].texts, [0]);
        assert_eq!(items[0].url.as_deref(), Some("https://react.dev"));
        assert_eq!(items[1].license, "");
        assert!(items[1].texts.is_empty());
        assert_eq!(builder.finish().texts, ["MIT License react"]);
    }

    #[test]
    fn arquivos_de_licenca_reconhecidos() {
        let dir = tempfile::tempdir().unwrap();
        for name in [
            "LICENSE",
            "license.md",
            "LICENCE.txt",
            "COPYING",
            "NOTICE",
            "LICENSE-MIT",
            "LICENSE_APACHE",
            "UNLICENSE",
            "README.md",
            "licenses.json.bak",
        ] {
            fs::write(dir.path().join(name), "x").unwrap();
        }
        fs::create_dir(dir.path().join("LICENSES")).unwrap();
        let mut names: Vec<String> = license_files(dir.path())
            .iter()
            .map(|path| path.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        names.sort();
        assert_eq!(
            names,
            [
                "COPYING",
                "LICENCE.txt",
                "LICENSE",
                "LICENSE-MIT",
                "LICENSE_APACHE",
                "NOTICE",
                "UNLICENSE",
                "license.md"
            ]
        );
        assert!(license_files(&dir.path().join("nao-existe")).is_empty());
    }

    #[test]
    fn licenca_da_secao_do_readme() {
        let readme = "# titlecase

Porta do Python.

## License

```
The MIT License (MIT)

Permission is hereby granted, free of charge, to any person
```

### Detalhe
ainda dentro

## Outra seção
fora
";
        let section = readme_license_section(readme).unwrap();
        assert_eq!(
            section,
            "The MIT License (MIT)

Permission is hereby granted, free of charge, to any person

### Detalhe
ainda dentro"
        );
        assert_eq!(guess_spdx(&section), Some("MIT"));
        assert_eq!(
            readme_license_section(
                "# x
## Licenses

## Fim
"
            ),
            None
        );
        assert_eq!(
            readme_license_section(
                "# x
sem licença
"
            ),
            None
        );
        assert_eq!(
            readme_license_section(
                "#License
texto
"
            ),
            None
        );
    }

    #[test]
    fn le_go_version_m() {
        let text = "C:\\x\\packwiz.exe: go1.26.5\n\tpath\tgithub.com/packwiz/packwiz\n\tmod\tgithub.com/packwiz/packwiz\t(devel)\t\n\tdep\tgithub.com/BurntSushi/toml\tv1.5.0\th1:abc=\n\tdep\tgolang.org/x/sys\tv0.35.0\th1:def=\n\t=>\tgolang.org/x/sys\tv0.36.0\th1:ghi=\n\tbuild\tCGO_ENABLED=0\n";
        let (go, modules) = parse_go_version_m(text);
        assert_eq!(go.as_deref(), Some("go1.26.5"));
        assert_eq!(
            modules,
            [
                GoModule {
                    path: "github.com/BurntSushi/toml".into(),
                    version: "v1.5.0".into()
                },
                GoModule {
                    path: "golang.org/x/sys".into(),
                    version: "v0.36.0".into()
                },
            ]
        );
    }

    #[test]
    fn caminho_de_modulo_no_cache_do_go() {
        assert_eq!(
            escape_module_path("github.com/BurntSushi/toml"),
            "github.com/!burnt!sushi/toml"
        );
        assert_eq!(escape_module_path("golang.org/x/sys"), "golang.org/x/sys");
    }

    #[test]
    fn reconhece_licencas_comuns() {
        let cases = [
            (
                "Apache License\n Version 2.0, January 2004",
                Some("Apache-2.0"),
            ),
            ("Mozilla Public License Version 2.0", Some("MPL-2.0")),
            (
                "MIT License\nPermission is hereby granted, free of charge, to any",
                Some("MIT"),
            ),
            (
                "Redistribution and use in source and binary forms ... Neither the name of Google",
                Some("BSD-3-Clause"),
            ),
            (
                "Redistribution and use in source and\n binary forms, with or without",
                Some("BSD-2-Clause"),
            ),
            (
                "Permission to use, copy, modify, and/or distribute this software ... provided that the above copyright notice",
                Some("ISC"),
            ),
            (
                "Permission to use, copy, modify, and/or distribute this software for any purpose with or without fee is hereby granted.",
                Some("0BSD"),
            ),
            (
                "This is free and unencumbered software released into the public domain.",
                Some("Unlicense"),
            ),
            (
                "Creative Commons Legal Code

CC0 1.0 Universal",
                Some("CC0-1.0"),
            ),
            ("Todos os direitos reservados", None),
        ];
        for (text, expected) in cases {
            assert_eq!(guess_spdx(text), expected, "{text}");
        }
    }

    #[test]
    fn fontes_embutidas_tem_licenca() {
        let mut builder = Builder::default();
        fonts(&mut builder).unwrap();
        let notices = builder.finish();
        assert_eq!(notices.groups[0].items.len(), FONTS.len());
        assert!(
            notices
                .texts
                .iter()
                .all(|text| text.contains("SIL OPEN FONT LICENSE"))
        );
    }

    #[test]
    fn json_nao_leva_caminhos_e_tem_o_formato_da_interface() {
        let mut builder = Builder::default();
        let text = builder.text("MIT");
        builder.group(
            "rust",
            vec![Item {
                name: "a".into(),
                version: "1".into(),
                license: "MIT".into(),
                url: None,
                texts: vec![text],
            }],
            0,
        );
        let json = serde_json::to_value(builder.finish()).unwrap();
        assert_eq!(
            json,
            serde_json::json!({
                "schema": SCHEMA,
                "groups": [{"id": "rust", "items": [{"name": "a", "version": "1", "license": "MIT", "texts": [0]}]}],
                "texts": ["MIT"]
            })
        );
    }

    // --- Conferência ------------------------------------------------------------------------

    const THIRD_PARTY: &str = "\
# Terceiros

## Crates Rust (crates.io)

| Crate | Versão | Licença | Usada em | Para quê |
|---|---|---|---|---|
| `tauri` | 2 | Apache-2.0 OR MIT | `warden-app` | App. |
| `portablemc` | =5.0.5 | Apache-2.0 | `warden-launcher` | Motor. |

## Ferramentas instaladas por `cargo xtask setup`

| Ferramenta | Versão | Licença | Para quê |
|---|---|---|---|
| `cargo-about` | 0.9.2 | MIT | Avisos. |

## Pacotes npm (`apps/desktop`)

| Pacote | Versão | Licença | Para quê |
|---|---|---|---|
| `react`, `react-dom` | 19 | MIT | Interface. |

## Sidecar do packwiz

| Origem | Versão | Licença | Para quê |
|---|---|---|---|
| [packwiz/packwiz](https://github.com/packwiz/packwiz) | ef87d96 | MIT | Refresh. |

## Fontes embutidas (`design/system/fonts/`)

| Fonte | Licença | Para quê |
|---|---|---|
| `IBM Plex Mono` | OFL-1.1 | Mono. |
| `Manrope` | OFL-1.1 | Texto. |
| `Pixelify Sans` | OFL-1.1 | Títulos. |
";

    fn licensed(name: &str, license: &str) -> Item {
        Item {
            name: name.into(),
            version: "1".into(),
            license: license.into(),
            url: None,
            texts: vec![0],
        }
    }

    /// Avisos completos, coerentes com `THIRD_PARTY`.
    fn complete() -> (Notices, Direct) {
        let notices = Notices {
            schema: SCHEMA,
            groups: vec![
                Group {
                    id: "rust",
                    items: vec![
                        licensed("tauri", "MIT OR Apache-2.0"),
                        licensed("portablemc", "Apache-2.0"),
                    ],
                },
                Group {
                    id: "npm",
                    items: vec![licensed("react", "MIT")],
                },
                Group {
                    id: "packwiz",
                    items: vec![licensed("packwiz", "MIT")],
                },
            ],
            texts: vec!["MIT".into()],
        };
        let direct = Direct {
            rust: ["tauri", "portablemc"].map(String::from).into(),
            npm: ["react"].map(String::from).into(),
        };
        (notices, direct)
    }

    #[test]
    fn avisos_completos_passam() {
        let (notices, direct) = complete();
        assert_eq!(verify(&notices, &direct, THIRD_PARTY), Vec::<String>::new());
    }

    #[test]
    fn componente_sem_licenca_falha() {
        let (mut notices, direct) = complete();
        notices.groups[1].items.push(Item {
            name: "sem-nada".into(),
            version: "2.0.0".into(),
            license: String::new(),
            url: None,
            texts: vec![],
        });
        // Só a expressão SPDX (pacote npm sem arquivo LICENSE) ainda vale como licença.
        notices.groups[1].items.push(Item {
            texts: vec![],
            ..licensed("so-spdx", "MIT")
        });
        assert_eq!(
            verify(&notices, &direct, THIRD_PARTY),
            ["npm sem-nada@2.0.0: sem licença (nem expressão SPDX nem texto)"]
        );
    }

    #[test]
    fn falta_o_motor_do_launcher_ou_o_packwiz() {
        let (mut notices, mut direct) = complete();
        notices.groups[0]
            .items
            .retain(|item| item.name != "portablemc");
        notices.groups.retain(|group| group.id != "packwiz");
        direct.rust.remove("portablemc");
        assert_eq!(
            verify(&notices, &direct, THIRD_PARTY),
            [
                "rust portablemc: faltou nos avisos",
                "packwiz packwiz: faltou nos avisos"
            ]
        );
    }

    #[test]
    fn dependencia_direta_fora_do_third_party_ou_dos_avisos_falha() {
        let (notices, mut direct) = complete();
        direct.npm.insert("zustand".into());
        direct.rust.insert("serde".into());
        assert_eq!(
            verify(&notices, &direct, THIRD_PARTY),
            [
                "rust serde: dependência direta do app fora dos avisos",
                "rust serde: dependência direta do app sem linha em \"Crates Rust\" no THIRD_PARTY.md",
                "npm zustand: dependência direta do app fora dos avisos",
                "npm zustand: dependência direta do app sem linha em \"Pacotes npm\" no THIRD_PARTY.md",
            ]
        );
    }

    #[test]
    fn licenca_diferente_do_third_party_falha() {
        let (mut notices, direct) = complete();
        notices.groups[1].items[0].license = "Apache-2.0".into();
        assert_eq!(
            verify(&notices, &direct, THIRD_PARTY),
            ["npm react: licença no THIRD_PARTY.md (MIT) diferente da detectada (Apache-2.0)"]
        );
    }

    #[test]
    fn packwiz_e_fontes_precisam_de_registro() {
        let (notices, direct) = complete();
        let sem = THIRD_PARTY
            .replace(
                "[packwiz/packwiz](https://github.com/packwiz/packwiz)",
                "packwiz",
            )
            .replace("| `Manrope` | OFL-1.1 | Texto. |\n", "");
        assert_eq!(
            verify(&notices, &direct, &sem),
            [
                "packwiz: sem registro no THIRD_PARTY.md",
                "fontes Manrope: sem linha em \"Fontes\" no THIRD_PARTY.md"
            ]
        );
    }

    #[test]
    fn registro_le_so_a_secao_pedida_e_varios_nomes_por_linha() {
        let npm = registered(THIRD_PARTY, "Pacotes npm");
        assert_eq!(
            npm.into_iter().collect::<Vec<_>>(),
            [
                ("react".to_owned(), "MIT".to_owned()),
                ("react-dom".to_owned(), "MIT".to_owned())
            ]
        );
        let rust = registered(THIRD_PARTY, "Crates Rust");
        assert!(rust.contains_key("tauri") && !rust.contains_key("cargo-about"));
        assert!(
            !rust.contains_key("warden-app"),
            "só a primeira coluna conta"
        );
    }

    #[test]
    fn identificadores_spdx_sem_ordem_nem_parenteses() {
        assert_eq!(
            license_ids("(MIT OR Apache-2.0)"),
            license_ids("Apache-2.0 OR MIT")
        );
        assert_eq!(
            license_ids("Apache-2.0 WITH LLVM-exception OR MIT"),
            ["Apache-2.0", "LLVM-exception", "MIT"]
                .map(String::from)
                .into()
        );
        assert_ne!(license_ids("MIT"), license_ids("MIT OR Apache-2.0"));
    }

    #[test]
    fn dependencias_npm_so_as_de_producao() {
        let json = r#"{"dependencies": {"react": "19"}, "devDependencies": {"vitest": "5"}}"#;
        assert_eq!(
            direct_npm_dependencies(json).unwrap(),
            ["react"].map(String::from).into()
        );
        assert!(direct_npm_dependencies("{}").unwrap().is_empty());
    }

    #[test]
    fn dependencias_rust_do_app_vem_do_workspace_real() {
        let metadata = deps::metadata(&workspace_root().join("Cargo.toml")).unwrap();
        let direct = direct_rust_dependencies(&metadata);
        // Da própria warden-app e de crates do workspace ligadas a ela (o motor do launcher).
        assert!(direct.contains("tauri"));
        assert!(direct.contains("portablemc"));
        // Nem crates do workspace, nem as só de teste, nem as do xtask.
        assert!(!direct.iter().any(|name| name.starts_with("warden-")));
        assert!(!direct.contains("proptest"));
        assert!(!direct.contains("wiremock"));
        assert!(!direct.contains("clap"));
    }

    #[test]
    fn third_party_do_repositorio_tem_as_fontes_e_o_packwiz() {
        let markdown = fs::read_to_string(workspace_root().join("THIRD_PARTY.md")).unwrap();
        let fonts = registered(&markdown, "Fontes");
        assert!(FONTS.iter().all(|(_, name)| fonts.contains_key(*name)));
        assert!(markdown.contains("packwiz/packwiz"));
        assert_eq!(
            registered(&markdown, "Pacotes npm")["dompurify"],
            "MPL-2.0 OR Apache-2.0"
        );
    }
}
