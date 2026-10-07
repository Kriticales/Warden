//! Validação dos arquivos gerados para outros launchers (`.mrpack` e zip da CurseForge).
//!
//! Reaproveitada pela E-04 (instância pronta para o Prism) e, na leitura, pela P1-19
//! (importar): [`read_mrpack`] aceita `env` fora da especificação (como `unknown`, que a
//! importação encontra em arquivos reais); [`validate_mrpack`] com `strict` reprova esses valores
//! e tudo mais que o Warden nunca gera (ARCHITECTURE §12).

use std::collections::{BTreeMap, BTreeSet};
use std::fs::File;
use std::io::Read;
use std::path::Path;

use serde::Deserialize;

use super::model::FormatValidation;
use crate::{Error, ExportErrorCode, Result};

/// Arquivo do índice do `.mrpack`.
pub const MRPACK_INDEX: &str = "modrinth.index.json";
/// Manifesto do zip da CurseForge.
pub const CURSEFORGE_MANIFEST: &str = "manifest.json";
/// Pastas de arquivos que o `.mrpack` copia para a instância.
pub const MRPACK_OVERRIDE_DIRS: [&str; 3] = ["overrides/", "client-overrides/", "server-overrides/"];

/// Limite de leitura do JSON do índice (proteção contra zip malicioso).
const MAX_JSON_BYTES: u64 = 32 * 1024 * 1024;
/// Valores de `env` da especificação do `.mrpack`.
const MRPACK_ENV_VALUES: [&str; 3] = ["required", "optional", "unsupported"];
/// Chaves de `dependencies` da especificação.
const MRPACK_LOADERS: [&str; 4] = ["forge", "neoforge", "fabric-loader", "quilt-loader"];

/// Ambiente de um arquivo do `.mrpack`; os valores ficam como vieram.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct MrpackEnv {
    /// Cliente.
    #[serde(default)]
    pub client: String,
    /// Servidor.
    #[serde(default)]
    pub server: String,
}

/// Um item de `files` do `modrinth.index.json`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MrpackFile {
    /// Destino na instância.
    #[serde(default)]
    pub path: String,
    /// Hashes por algoritmo.
    #[serde(default)]
    pub hashes: BTreeMap<String, String>,
    /// Ambiente.
    #[serde(default)]
    pub env: Option<MrpackEnv>,
    /// Endereços de download.
    #[serde(default)]
    pub downloads: Vec<String>,
    /// Tamanho em bytes.
    #[serde(default)]
    pub file_size: u64,
}

/// O `modrinth.index.json`, lido sem julgar os valores.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MrpackIndex {
    /// Versão do formato (1).
    #[serde(default)]
    pub format_version: u32,
    /// Jogo (`minecraft`).
    #[serde(default)]
    pub game: String,
    /// Versão do pack.
    #[serde(default)]
    pub version_id: String,
    /// Nome do pack.
    #[serde(default)]
    pub name: String,
    /// Resumo.
    #[serde(default)]
    pub summary: Option<String>,
    /// Arquivos por referência.
    #[serde(default)]
    pub files: Vec<MrpackFile>,
    /// Minecraft e loader.
    #[serde(default)]
    pub dependencies: BTreeMap<String, String>,
}

/// Um `.mrpack` aberto: o índice e os nomes das entradas do zip.
#[derive(Debug, Clone)]
pub struct MrpackArchive {
    /// O índice.
    pub index: MrpackIndex,
    /// Nomes das entradas (diretórios terminam em `/`).
    pub entries: Vec<String>,
}

/// O que conferir no `.mrpack`.
#[derive(Debug, Clone, Default)]
pub struct MrpackRules {
    /// Reprova o que o Warden nunca gera: `env` fora da especificação, loader repetido,
    /// versão ou nome vazios.
    pub strict: bool,
    /// Caminhos que podem aparecer em `overrides/` (sem o prefixo); `None` não confere.
    pub allowed_overrides: Option<BTreeSet<String>>,
    /// Jars que podem aparecer em `overrides/` (confirmados pela pessoa); `None` não confere.
    pub allowed_jars: Option<BTreeSet<String>>,
}

/// O que conferir no zip da CurseForge.
#[derive(Debug, Clone, Default)]
pub struct CurseforgeRules {
    /// Caminhos que podem aparecer em `overrides/` (sem o prefixo); `None` não confere.
    pub allowed_overrides: Option<BTreeSet<String>>,
    /// Jars que podem aparecer em `overrides/` (confirmados pela pessoa); `None` não confere.
    pub allowed_jars: Option<BTreeSet<String>>,
}

fn unreadable(path: &Path, reason: impl std::fmt::Display) -> Error {
    Error::new(
        ExportErrorCode::UnreadableArchive,
        format!("{}: {reason}", path.display()),
    )
}

fn open_zip(path: &Path) -> Result<zip::ZipArchive<File>> {
    let file = File::open(path).map_err(|error| unreadable(path, error))?;
    zip::ZipArchive::new(file).map_err(|error| unreadable(path, format!("não é um zip: {error}")))
}

fn read_json_entry<T: for<'de> Deserialize<'de>>(
    archive: &mut zip::ZipArchive<File>,
    path: &Path,
    name: &str,
) -> Result<T> {
    let entry = archive
        .by_name(name)
        .map_err(|_| unreadable(path, format!("falta {name}")))?;
    let mut text = Vec::new();
    entry
        .take(MAX_JSON_BYTES + 1)
        .read_to_end(&mut text)
        .map_err(|error| unreadable(path, format!("{name}: {error}")))?;
    if u64::try_from(text.len()).unwrap_or(u64::MAX) > MAX_JSON_BYTES {
        return Err(unreadable(path, format!("{name} é grande demais")));
    }
    serde_json::from_slice(&text).map_err(|error| unreadable(path, format!("{name}: {error}")))
}

/// Abre um `.mrpack` sem julgar valores: `env: "unknown"` e outros desvios da especificação
/// passam (é a leitura da importação).
pub fn read_mrpack(path: &Path) -> Result<MrpackArchive> {
    let mut archive = open_zip(path)?;
    let entries = archive.file_names().map(str::to_owned).collect();
    let index = read_json_entry(&mut archive, path, MRPACK_INDEX)?;
    Ok(MrpackArchive { index, entries })
}

fn entry_name_problem(name: &str) -> Option<&'static str> {
    if name.contains('\\') {
        return Some("usa barra invertida");
    }
    if name.starts_with('/') || name.as_bytes().get(1) == Some(&b':') {
        return Some("caminho absoluto");
    }
    if name.split('/').any(|segment| segment == "..") {
        return Some("sobe para fora da pasta (..)");
    }
    None
}

fn is_jar(name: &str) -> bool {
    name.to_ascii_lowercase().ends_with(".jar")
}

fn is_hex(text: &str, length: usize) -> bool {
    text.len() == length
        && text
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

/// Confere as entradas de override: nome seguro, só arquivos permitidos, nenhum jar sem
/// confirmação. Devolve os arquivos (sem o prefixo) e os jars embutidos.
fn check_overrides(
    entries: &[String],
    prefixes: &[&str],
    allowed: Option<&BTreeSet<String>>,
    allowed_jars: Option<&BTreeSet<String>>,
    problems: &mut Vec<String>,
) -> (usize, Vec<String>) {
    let mut count = 0;
    let mut jars = Vec::new();
    for name in entries {
        let Some((prefix, rest)) = prefixes
            .iter()
            .find_map(|prefix| name.strip_prefix(prefix).map(|rest| (*prefix, rest)))
        else {
            continue;
        };
        if rest.is_empty() || name.ends_with('/') {
            continue;
        }
        count += 1;
        if let Some(allowed) = allowed
            && !allowed.contains(rest)
        {
            problems.push(format!(
                "{prefix}{rest} está no arquivo, mas não é um arquivo do pack"
            ));
        }
        if is_jar(rest) {
            jars.push(name.clone());
            if let Some(allowed_jars) = allowed_jars
                && !allowed_jars.contains(rest)
            {
                problems.push(format!(
                    "{name} é um jar de terceiros sem a confirmação de licença"
                ));
            }
        }
    }
    (count, jars)
}

fn invalid(path: &Path, problems: &[String]) -> Error {
    let list = problems
        .iter()
        .map(|problem| format!("- {problem}"))
        .collect::<Vec<_>>()
        .join("\n");
    Error::new(
        ExportErrorCode::InvalidOutput,
        format!("{} não passou na validação", path.display()),
    )
    .with_param("count", problems.len().to_string())
    .with_detail(format!(
        "O arquivo gerado reprovou em {} conferência(s):\n{list}",
        problems.len()
    ))
}

/// Valida o `.mrpack`: zip legível, `modrinth.index.json` conforme a especificação, entradas
/// com nomes seguros e `overrides/` só com o que as regras permitem.
pub fn validate_mrpack(path: &Path, rules: &MrpackRules) -> Result<FormatValidation> {
    let archive = read_mrpack(path)?;
    let index = &archive.index;
    let mut problems = Vec::new();
    if index.format_version != 1 {
        problems.push(format!(
            "formatVersion deveria ser 1 e é {}",
            index.format_version
        ));
    }
    if index.game != "minecraft" {
        problems.push(format!("game deveria ser \"minecraft\" e é {:?}", index.game));
    }
    if !index.dependencies.contains_key("minecraft") {
        problems.push("dependencies não tem a versão do Minecraft".to_owned());
    }
    if rules.strict {
        if index.version_id.trim().is_empty() {
            problems.push("versionId está vazio (o pack.toml precisa de version)".to_owned());
        }
        if index.name.trim().is_empty() {
            problems.push("name está vazio".to_owned());
        }
        let loaders = index
            .dependencies
            .keys()
            .filter(|key| MRPACK_LOADERS.contains(&key.as_str()))
            .count();
        if loaders > 1 {
            problems.push("dependencies tem mais de um loader".to_owned());
        }
        for key in index.dependencies.keys() {
            if key != "minecraft" && !MRPACK_LOADERS.contains(&key.as_str()) {
                problems.push(format!("dependencies tem a chave desconhecida {key:?}"));
            }
        }
    }
    let mut seen = BTreeSet::new();
    for file in &index.files {
        if file.path.is_empty() || entry_name_problem(&file.path).is_some() {
            problems.push(format!("files tem o caminho inválido {:?}", file.path));
            continue;
        }
        if !seen.insert(file.path.clone()) {
            problems.push(format!("files repete {}", file.path));
        }
        for algorithm in [("sha1", 40), ("sha512", 128)] {
            match file.hashes.get(algorithm.0) {
                Some(hash) if is_hex(hash, algorithm.1) => {}
                _ => problems.push(format!(
                    "{} não tem um {} válido (hexadecimal minúsculo)",
                    file.path, algorithm.0
                )),
            }
        }
        if file.downloads.is_empty() {
            problems.push(format!("{} não tem endereço de download", file.path));
        }
        for download in &file.downloads {
            if !download.starts_with("https://") {
                problems.push(format!("{} baixa de um endereço sem https", file.path));
            }
        }
        if rules.strict
            && let Some(env) = &file.env
        {
            for (side, value) in [("client", &env.client), ("server", &env.server)] {
                if !MRPACK_ENV_VALUES.contains(&value.as_str()) {
                    problems.push(format!(
                        "{} tem env.{side} = {value:?}, fora da especificação",
                        file.path
                    ));
                }
            }
        }
    }
    for name in &archive.entries {
        if let Some(problem) = entry_name_problem(name) {
            problems.push(format!("a entrada {name:?} {problem}"));
        }
        let known = name == MRPACK_INDEX
            || MRPACK_OVERRIDE_DIRS
                .iter()
                .any(|prefix| name.starts_with(prefix));
        if !known {
            problems.push(format!("a entrada {name:?} não faz parte do formato"));
        }
    }
    let (overrides, embedded_jars) = check_overrides(
        &archive.entries,
        &MRPACK_OVERRIDE_DIRS,
        rules.allowed_overrides.as_ref(),
        rules.allowed_jars.as_ref(),
        &mut problems,
    );
    if !problems.is_empty() {
        return Err(invalid(path, &problems));
    }
    Ok(FormatValidation {
        entries: archive.entries.len(),
        references: index.files.len(),
        overrides,
        embedded_jars,
    })
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CurseforgeLoader {
    #[serde(default)]
    id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CurseforgeMinecraft {
    #[serde(default)]
    version: String,
    #[serde(default)]
    mod_loaders: Vec<CurseforgeLoader>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CurseforgeFile {
    #[serde(default, rename = "projectID")]
    project_id: u64,
    #[serde(default, rename = "fileID")]
    file_id: u64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CurseforgeManifest {
    #[serde(default)]
    minecraft: Option<CurseforgeMinecraft>,
    #[serde(default)]
    manifest_type: String,
    #[serde(default)]
    manifest_version: u32,
    #[serde(default)]
    name: String,
    #[serde(default)]
    version: String,
    #[serde(default)]
    files: Vec<CurseforgeFile>,
    #[serde(default)]
    overrides: String,
}

/// Valida o zip da CurseForge: `manifest.json` conforme o formato, entradas com nomes seguros
/// e `overrides/` só com o que as regras permitem, sem jar de terceiros não confirmado.
pub fn validate_curseforge(path: &Path, rules: &CurseforgeRules) -> Result<FormatValidation> {
    let mut archive = open_zip(path)?;
    let entries: Vec<String> = archive.file_names().map(str::to_owned).collect();
    let manifest: CurseforgeManifest = read_json_entry(&mut archive, path, CURSEFORGE_MANIFEST)?;
    let mut problems = Vec::new();
    if manifest.manifest_type != "minecraftModpack" {
        problems.push(format!(
            "manifestType deveria ser \"minecraftModpack\" e é {:?}",
            manifest.manifest_type
        ));
    }
    if manifest.manifest_version != 1 {
        problems.push(format!(
            "manifestVersion deveria ser 1 e é {}",
            manifest.manifest_version
        ));
    }
    if manifest.name.trim().is_empty() {
        problems.push("name está vazio".to_owned());
    }
    if manifest.version.trim().is_empty() {
        problems.push("version está vazio (o pack.toml precisa de version)".to_owned());
    }
    if manifest.overrides != "overrides" {
        problems.push(format!(
            "overrides deveria ser \"overrides\" e é {:?}",
            manifest.overrides
        ));
    }
    match &manifest.minecraft {
        None => problems.push("falta a tabela minecraft".to_owned()),
        Some(minecraft) => {
            if minecraft.version.trim().is_empty() {
                problems.push("minecraft.version está vazio".to_owned());
            }
            if minecraft.mod_loaders.iter().any(|loader| loader.id.is_empty()) {
                problems.push("há um modLoader sem id".to_owned());
            }
        }
    }
    for file in &manifest.files {
        if file.project_id == 0 || file.file_id == 0 {
            problems.push("files tem um item sem projectID ou fileID".to_owned());
        }
    }
    for name in &entries {
        if let Some(problem) = entry_name_problem(name) {
            problems.push(format!("a entrada {name:?} {problem}"));
        }
        let known = name == CURSEFORGE_MANIFEST
            || name == "modlist.html"
            || name.starts_with("overrides/");
        if !known {
            problems.push(format!("a entrada {name:?} não faz parte do formato"));
        }
    }
    let (overrides, embedded_jars) = check_overrides(
        &entries,
        &["overrides/"],
        rules.allowed_overrides.as_ref(),
        rules.allowed_jars.as_ref(),
        &mut problems,
    );
    if !problems.is_empty() {
        return Err(invalid(path, &problems));
    }
    Ok(FormatValidation {
        entries: entries.len(),
        references: manifest.files.len(),
        overrides,
        embedded_jars,
    })
}

#[cfg(test)]
mod tests {
    use std::io::Write as _;

    use serde_json::json;
    use zip::write::SimpleFileOptions;

    use super::*;

    const SHA1: &str = "0123456789abcdef0123456789abcdef01234567";

    fn sha512() -> String {
        "ab".repeat(64)
    }

    fn write_zip(path: &Path, entries: &[(&str, &[u8])]) {
        let mut zip = zip::ZipWriter::new(File::create(path).unwrap());
        for (name, bytes) in entries {
            zip.start_file(*name, SimpleFileOptions::default()).unwrap();
            zip.write_all(bytes).unwrap();
        }
        zip.finish().unwrap();
    }

    fn mrpack_index(env: &str) -> Vec<u8> {
        json!({
            "formatVersion": 1,
            "game": "minecraft",
            "versionId": "1.0.0",
            "name": "Teste",
            "files": [{
                "path": "mods/a.jar",
                "hashes": {"sha1": SHA1, "sha512": sha512()},
                "env": {"client": env, "server": "required"},
                "downloads": ["https://cdn.modrinth.com/data/x/a.jar"],
                "fileSize": 10
            }],
            "dependencies": {"minecraft": "1.21.1", "fabric-loader": "0.16.0"}
        })
        .to_string()
        .into_bytes()
    }

    #[test]
    fn mrpack_valido_passa_e_conta() {
        let temp = tempfile::tempdir().unwrap();
        let file = temp.path().join("a.mrpack");
        write_zip(
            &file,
            &[
                (MRPACK_INDEX, &mrpack_index("required")),
                ("overrides/config/a.txt", b"x"),
            ],
        );
        let rules = MrpackRules {
            strict: true,
            allowed_overrides: Some(BTreeSet::from(["config/a.txt".to_owned()])),
            allowed_jars: Some(BTreeSet::new()),
        };
        let report = validate_mrpack(&file, &rules).unwrap();
        assert_eq!(report.references, 1);
        assert_eq!(report.overrides, 1);
        assert!(report.embedded_jars.is_empty());
    }

    #[test]
    fn env_unknown_e_lido_mas_reprovado_na_geracao() {
        let temp = tempfile::tempdir().unwrap();
        let file = temp.path().join("a.mrpack");
        write_zip(&file, &[(MRPACK_INDEX, &mrpack_index("unknown"))]);
        let archive = read_mrpack(&file).unwrap();
        assert_eq!(
            archive.index.files[0].env.as_ref().unwrap().client,
            "unknown"
        );
        validate_mrpack(&file, &MrpackRules::default()).unwrap();
        let error = validate_mrpack(
            &file,
            &MrpackRules {
                strict: true,
                ..MrpackRules::default()
            },
        )
        .unwrap_err();
        assert!(error.to_string().contains("não passou"));
        assert!(
            warden_core::DomainError::detail(&error)
                .unwrap()
                .contains("env.client")
        );
    }

    #[test]
    fn override_fora_do_indice_e_jar_sem_confirmacao_reprovam() {
        let temp = tempfile::tempdir().unwrap();
        let file = temp.path().join("a.mrpack");
        write_zip(
            &file,
            &[
                (MRPACK_INDEX, &mrpack_index("required")),
                ("overrides/config/a.txt", b"x"),
                ("overrides/mods/b.jar", b"jar"),
                ("overrides/../fuga.txt", b"x"),
            ],
        );
        let rules = MrpackRules {
            strict: true,
            allowed_overrides: Some(BTreeSet::from(["config/a.txt".to_owned()])),
            allowed_jars: Some(BTreeSet::new()),
        };
        let detail = warden_core::DomainError::detail(&validate_mrpack(&file, &rules).unwrap_err())
            .unwrap();
        assert!(detail.contains("mods/b.jar está no arquivo"), "{detail}");
        assert!(detail.contains("jar de terceiros sem a confirmação"), "{detail}");
        assert!(detail.contains("sobe para fora"), "{detail}");
    }

    #[test]
    fn zip_quebrado_ou_sem_indice_e_ilegivel() {
        use warden_core::{DomainCode, DomainError as _};
        let temp = tempfile::tempdir().unwrap();
        let broken = temp.path().join("b.mrpack");
        std::fs::write(&broken, b"nao e zip").unwrap();
        let empty = temp.path().join("c.mrpack");
        write_zip(&empty, &[("outra.txt", b"x")]);
        for file in [&broken, &empty] {
            let error = read_mrpack(file).unwrap_err();
            assert_eq!(
                error.code(),
                DomainCode::Domain(ExportErrorCode::UnreadableArchive)
            );
        }
    }

    fn cf_manifest() -> Vec<u8> {
        json!({
            "minecraft": {"version": "1.21.1", "modLoaders": [{"id": "fabric-0.16.0", "primary": true}]},
            "manifestType": "minecraftModpack",
            "manifestVersion": 1,
            "name": "Teste",
            "version": "1.0.0",
            "author": "",
            "projectID": 0,
            "files": [{"projectID": 10, "fileID": 20, "required": true}],
            "overrides": "overrides"
        })
        .to_string()
        .into_bytes()
    }

    #[test]
    fn zip_da_curseforge_valido_e_jar_de_terceiros_so_com_confirmacao() {
        let temp = tempfile::tempdir().unwrap();
        let file = temp.path().join("a.zip");
        write_zip(
            &file,
            &[
                (CURSEFORGE_MANIFEST, &cf_manifest()),
                ("modlist.html", b"<ul></ul>"),
                ("overrides/mods/local.jar", b"jar"),
            ],
        );
        let confirmed = CurseforgeRules {
            allowed_overrides: Some(BTreeSet::from(["mods/local.jar".to_owned()])),
            allowed_jars: Some(BTreeSet::from(["mods/local.jar".to_owned()])),
        };
        let report = validate_curseforge(&file, &confirmed).unwrap();
        assert_eq!(report.references, 1);
        assert_eq!(report.embedded_jars, ["overrides/mods/local.jar"]);
        let unconfirmed = CurseforgeRules {
            allowed_overrides: confirmed.allowed_overrides.clone(),
            allowed_jars: Some(BTreeSet::new()),
        };
        let detail =
            warden_core::DomainError::detail(&validate_curseforge(&file, &unconfirmed).unwrap_err())
                .unwrap();
        assert!(detail.contains("sem a confirmação de licença"), "{detail}");
    }

    #[test]
    fn manifesto_da_curseforge_invalido_reprova() {
        let temp = tempfile::tempdir().unwrap();
        let file = temp.path().join("a.zip");
        let bad = json!({"manifestType": "outro", "manifestVersion": 2, "files": [{"projectID": 0, "fileID": 0}]})
            .to_string();
        write_zip(&file, &[(CURSEFORGE_MANIFEST, bad.as_bytes())]);
        let detail = warden_core::DomainError::detail(
            &validate_curseforge(&file, &CurseforgeRules::default()).unwrap_err(),
        )
        .unwrap();
        for expected in ["manifestType", "manifestVersion", "falta a tabela minecraft", "projectID"] {
            assert!(detail.contains(expected), "{expected}: {detail}");
        }
    }
}
