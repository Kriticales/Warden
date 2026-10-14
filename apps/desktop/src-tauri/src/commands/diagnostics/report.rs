//! O relatório do diagnóstico como a interface o mostra: os achados com os itens já ligados ao
//! pack (caminho e nome), a marca "ignorado por você" e a redação de dados pessoais.
//!
//! - O último relatório do pré-teste fica em `<dados>/diagnostics/<pack>.json`, com o resumo do
//!   pack ([`DiagnosticsReport::pack_hash`]) para a interface saber que ele ficou velho.
//! - "Ignorar este aviso neste pack" grava o [`ReportFinding::id`] em `.warden/project.toml`,
//!   tabela `[diagnostics]`, chave `ignored` (lista de textos). O resto do arquivo não muda.
//! - Tudo o que vai para a tela passa pela redação (`warden_diagnostics::Redactor`): trechos de
//!   log e parâmetros nunca mostram o nome do usuário do computador, do jogador, IPs ou chaves.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use warden_core::PackId;
use warden_diagnostics::{Evidence, Finding, ItemRef, Redactor, Severity, SuggestedFix};
use warden_project::inventory::InventoryItem;

use crate::error::AppError;

/// Arquivo do Warden dentro do pack.
pub(crate) const PROJECT_FILE: &str = ".warden/project.toml";
/// Tabela dos ajustes do diagnóstico em `.warden/project.toml`.
const TABLE: &str = "diagnostics";
/// Chave com os achados ignorados.
const IGNORED_KEY: &str = "ignored";

/// Passagem que gerou o relatório.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) enum CheckPass {
    /// Com os dados do pack e das APIs (antes de baixar).
    Quick,
    /// Também com os jars (depois de copiar o pack para a instância).
    Complete,
}

impl CheckPass {
    pub(crate) fn domain(self) -> warden_diagnostics::pretest::Pass {
        match self {
            Self::Quick => warden_diagnostics::pretest::Pass::Quick,
            Self::Complete => warden_diagnostics::pretest::Pass::Complete,
        }
    }
}

/// Um item citado num achado, ligado ao pack quando dá.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ReportItem {
    /// Caminho do item no pack (`None`: o mod não está no pack, ou o log não diz qual é).
    pub(crate) path: Option<String>,
    /// Nome para mostrar: o do pack, o declarado no jar ou o id/arquivo do log.
    pub(crate) name: String,
}

/// Um achado como a interface mostra.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ReportFinding {
    /// Identificador estável no pack: regra e itens (`W_SIDE|mods/xaero.pw.toml`). É o que
    /// "Ignorar este aviso neste pack" grava.
    pub(crate) id: String,
    /// O achado (com as evidências e as correções sugeridas), já redigido.
    pub(crate) finding: Finding,
    /// Os itens de `finding.items`, na mesma ordem, ligados ao pack.
    pub(crate) items: Vec<ReportItem>,
    /// Padrão do catálogo de logs que gerou o achado (só no pós-crash).
    pub(crate) pattern: Option<String>,
    /// Nomes dos projetos citados nas correções pelo id (a dependência que falta, quando a
    /// fonte a conhece).
    pub(crate) labels: BTreeMap<String, String>,
    /// Ignorado por você neste pack (não bloqueia o teste nem conta no contador).
    pub(crate) ignored: bool,
}

/// O último relatório do pré-teste de um pack.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DiagnosticsReport {
    /// Passagem que valeu: "Verificar agora" e a "Verificação final" pedem a completa, que vira
    /// rápida quando ainda falta o jar de algum mod.
    pub(crate) pass: CheckPass,
    /// Quando a verificação terminou (milissegundos desde 1970).
    #[specta(type = specta_typescript::Number)]
    pub(crate) checked_at_ms: u64,
    /// Itens conferidos (mods, resource packs e shaders).
    pub(crate) item_count: u32,
    /// O pack tem loader e versão do Minecraft conhecidos (vanilla: nada a conferir).
    pub(crate) checked_mods: bool,
    /// Achados, do mais grave ao menos grave.
    pub(crate) findings: Vec<ReportFinding>,
    /// Resumo do `pack.toml` e do índice na hora da verificação.
    pub(crate) pack_hash: Option<String>,
    /// O pack mudou depois desta verificação (calculado de novo a cada leitura).
    pub(crate) stale: bool,
}

impl DiagnosticsReport {
    /// Achados de erro que valem (não ignorados): os que bloqueiam o teste.
    pub(crate) fn blocking_errors(&self) -> usize {
        self.findings
            .iter()
            .filter(|entry| !entry.ignored && entry.finding.severity() == Severity::Error)
            .count()
    }
}

/// Liga os itens dos achados ao pack.
pub(crate) struct ItemNames<'a> {
    by_path: HashMap<&'a str, &'a InventoryItem>,
    by_file: HashMap<String, &'a InventoryItem>,
    mod_paths: &'a HashMap<String, String>,
    mod_names: &'a HashMap<String, String>,
    /// Nomes de projetos das fontes pelo id (dependências que faltam).
    project_names: HashMap<String, String>,
}

impl<'a> ItemNames<'a> {
    pub(crate) fn new(
        items: &'a [InventoryItem],
        mod_paths: &'a HashMap<String, String>,
        mod_names: &'a HashMap<String, String>,
    ) -> Self {
        Self {
            by_path: items.iter().map(|item| (item.path.as_str(), item)).collect(),
            by_file: items
                .iter()
                .filter_map(|item| Some((item.file_name.as_ref()?.to_lowercase(), item)))
                .collect(),
            mod_paths,
            mod_names,
            project_names: HashMap::new(),
        }
    }

    /// O mesmo, com os nomes de projetos das fontes.
    pub(crate) fn with_project_names(mut self, names: HashMap<String, String>) -> Self {
        self.project_names = names;
        self
    }

    /// O nome de uma dependência pelo id: o projeto da fonte ou o declarado num jar do pack.
    fn dependency_name(&self, id: &str) -> Option<String> {
        self.project_names
            .get(id)
            .or_else(|| self.mod_names.get(id))
            .cloned()
    }

    fn item(&self, path: &str) -> Option<&InventoryItem> {
        self.by_path.get(path).copied()
    }

    /// O item do pack e o nome de uma referência.
    pub(crate) fn resolve(&self, reference: &ItemRef) -> ReportItem {
        match reference {
            ItemRef::Metafile { path } => ReportItem {
                path: self.item(path).map(|item| item.path.clone()),
                name: self
                    .item(path)
                    .map_or_else(|| file_stem(path), |item| item.name.clone()),
            },
            ItemRef::ModId { id } => {
                let item = self.mod_paths.get(id).and_then(|path| self.item(path));
                ReportItem {
                    path: item.map(|item| item.path.clone()),
                    name: item.map_or_else(
                        || self.mod_names.get(id).cloned().unwrap_or_else(|| id.clone()),
                        |item| item.name.clone(),
                    ),
                }
            }
            ItemRef::JarFile { name } => {
                let item = self.by_file.get(&name.to_lowercase()).copied();
                ReportItem {
                    path: item.map(|item| item.path.clone()),
                    name: item.map_or_else(|| name.clone(), |item| item.name.clone()),
                }
            }
        }
    }
}

fn file_stem(path: &str) -> String {
    let name = path.rsplit('/').next().unwrap_or(path);
    name.strip_suffix(".pw.toml").unwrap_or(name).to_owned()
}

/// Identificador estável do achado no pack: a regra e os itens (caminho no pack ou, fora dele,
/// a referência do achado), em ordem.
pub(crate) fn finding_id(finding: &Finding, items: &[ReportItem]) -> String {
    let mut parts: BTreeSet<String> = BTreeSet::new();
    for (reference, item) in finding.items().iter().zip(items) {
        parts.insert(match (&item.path, reference) {
            (Some(path), _) | (None, ItemRef::Metafile { path }) => path.clone(),
            (None, ItemRef::ModId { id }) => format!("mod:{id}"),
            (None, ItemRef::JarFile { name }) => format!("jar:{name}"),
        });
    }
    // Achado sem item (Java do pack): a correção sugerida o distingue.
    if parts.is_empty() {
        for fix in finding.fixes() {
            if let SuggestedFix::AddDependency { mod_id, .. } = fix {
                parts.insert(format!("mod:{mod_id}"));
            }
        }
    }
    // Achado de uma chave de config (C-06): o arquivo e a chave o distinguem.
    if let Some(key) = finding.param("config_key") {
        let file = finding.param("config_file").unwrap_or_default();
        parts.insert(format!("config:{file}#{key}"));
    }
    let mut id = finding.rule().to_string();
    for part in parts {
        id.push('|');
        id.push_str(&part);
    }
    id
}

/// O achado com trechos e parâmetros redigidos.
pub(crate) fn redact_finding(finding: &Finding, redactor: &Redactor) -> Finding {
    let text = |value: &str| redactor.redact(value).text;
    let evidence: Vec<Evidence> = finding
        .evidence()
        .iter()
        .map(|evidence| match evidence {
            Evidence::JarMetadata {
                item,
                file_in_jar,
                excerpt,
            } => Evidence::JarMetadata {
                item: item.clone(),
                file_in_jar: file_in_jar.clone(),
                excerpt: text(excerpt),
            },
            Evidence::Log {
                file,
                line,
                excerpt,
            } => Evidence::Log {
                file: text(file),
                line: *line,
                excerpt: text(excerpt),
            },
            Evidence::PackFile { path, excerpt } => Evidence::PackFile {
                path: path.clone(),
                excerpt: text(excerpt),
            },
            other => other.clone(),
        })
        .collect();
    let mut evidence = evidence.into_iter();
    let first = evidence
        .next()
        .unwrap_or_else(|| finding.primary_evidence().clone());
    let mut redacted = Finding::new(
        finding.rule().clone(),
        finding.severity(),
        finding.title_key(),
        first,
    );
    for (name, value) in finding.params() {
        redacted = redacted.with_param(name.clone(), text(value));
    }
    for item in finding.items() {
        redacted = redacted.with_item(item.clone());
    }
    for more in evidence {
        redacted = redacted.with_evidence(more);
    }
    for fix in finding.fixes() {
        redacted = redacted.with_fix(fix.clone());
    }
    redacted
}

/// Monta a entrada do relatório para um achado.
pub(crate) fn report_finding(
    finding: &Finding,
    pattern: Option<String>,
    names: &ItemNames<'_>,
    redactor: &Redactor,
) -> ReportFinding {
    let items: Vec<ReportItem> = finding
        .items()
        .iter()
        .map(|reference| names.resolve(reference))
        .collect();
    let labels = finding
        .fixes()
        .iter()
        .filter_map(|fix| match fix {
            SuggestedFix::AddDependency { mod_id, .. } => {
                Some((mod_id.clone(), names.dependency_name(mod_id)?))
            }
            _ => None,
        })
        .collect();
    ReportFinding {
        id: finding_id(finding, &items),
        finding: redact_finding(finding, redactor),
        items,
        pattern,
        labels,
        ignored: false,
    }
}

/// Ids de projeto do Modrinth citados em "Adicionar dependência" (a fonte declarou a
/// dependência), para buscar os nomes no cache.
pub(crate) fn modrinth_dependency_ids(findings: &[Finding]) -> Vec<String> {
    let mut ids = BTreeSet::new();
    for finding in findings {
        let from_modrinth = finding.evidence().iter().any(|evidence| {
            matches!(
                evidence,
                Evidence::Api {
                    source: warden_diagnostics::Source::Modrinth,
                    ..
                }
            )
        });
        if !from_modrinth {
            continue;
        }
        for fix in finding.fixes() {
            if let SuggestedFix::AddDependency { mod_id, .. } = fix {
                ids.insert(mod_id.clone());
            }
        }
    }
    ids.into_iter().collect()
}

/// Ordena do mais grave ao menos grave, mantendo a ordem das regras dentro da gravidade.
pub(crate) fn sort_findings(findings: &mut [ReportFinding]) {
    findings.sort_by_key(|entry| entry.finding.severity());
}

/// Marca os achados ignorados.
pub(crate) fn apply_ignored(findings: &mut [ReportFinding], ignored: &BTreeSet<String>) {
    for entry in findings {
        entry.ignored = ignored.contains(&entry.id);
    }
}

// ---------------------------------------------------------------------------------------------
// "Ignorar este aviso neste pack" (`.warden/project.toml`)
// ---------------------------------------------------------------------------------------------

fn project_error(detail: impl Into<String>) -> AppError {
    AppError::new(warden_project::ProjectErrorCode::InvalidPack).with_detail(detail)
}

/// Os achados ignorados do pack. Arquivo ausente ou sem a tabela: nenhum. Arquivo ilegível
/// (editado à mão): nenhum, com o problema registrado.
pub(crate) fn read_ignored(root: &Path) -> BTreeSet<String> {
    let text = match fs::read_to_string(root.join(PROJECT_FILE)) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return BTreeSet::new(),
        Err(error) => {
            tracing::warn!(%error, "{PROJECT_FILE} ilegível; nenhum aviso ignorado");
            return BTreeSet::new();
        }
    };
    match parse_ignored(&text) {
        Ok(ignored) => ignored,
        Err(error) => {
            tracing::warn!(detail = ?error.detail, "{PROJECT_FILE} inválido; nenhum aviso ignorado");
            BTreeSet::new()
        }
    }
}

/// Lê a lista `[diagnostics] ignored` de um texto de `.warden/project.toml`.
pub(crate) fn parse_ignored(text: &str) -> Result<BTreeSet<String>, AppError> {
    let doc: toml_edit::DocumentMut = text
        .parse()
        .map_err(|error| project_error(format!("{PROJECT_FILE}: {error}")))?;
    Ok(doc
        .get(TABLE)
        .and_then(|table| table.get(IGNORED_KEY))
        .and_then(toml_edit::Item::as_array)
        .map(|array| {
            array
                .iter()
                .filter_map(|value| value.as_str().map(ToOwned::to_owned))
                .collect()
        })
        .unwrap_or_default())
}

/// O texto de `.warden/project.toml` com o achado acrescentado (ou tirado) da lista de
/// ignorados. O resto do arquivo (outras tabelas, comentários, ordem) não muda.
pub(crate) fn with_ignored(text: &str, id: &str, ignored: bool) -> Result<String, AppError> {
    let mut doc: toml_edit::DocumentMut = text
        .parse()
        .map_err(|error| project_error(format!("{PROJECT_FILE}: {error}")))?;
    let mut current = parse_ignored(text)?;
    if ignored {
        current.insert(id.to_owned());
    } else {
        current.remove(id);
    }
    if !doc.contains_key(TABLE) {
        doc.insert(TABLE, toml_edit::Item::Table(toml_edit::Table::new()));
    }
    let table = doc
        .get_mut(TABLE)
        .and_then(toml_edit::Item::as_table_like_mut)
        .ok_or_else(|| project_error(format!("{PROJECT_FILE}: [{TABLE}] inválida")))?;
    let mut array = toml_edit::Array::new();
    for value in &current {
        array.push(value.as_str());
    }
    table.insert(IGNORED_KEY, toml_edit::value(array));
    Ok(doc.to_string())
}

/// Grava o achado como ignorado (ou não) no pack. Devolve se o arquivo mudou.
pub(crate) fn write_ignored(root: &Path, id: &str, ignored: bool) -> Result<bool, AppError> {
    let path = root.join(PROJECT_FILE);
    let before = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(error) => return Err(project_error(format!("ler {PROJECT_FILE}: {error}"))),
    };
    let after = with_ignored(&before, id, ignored)?;
    if after == before {
        return Ok(false);
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| project_error(format!("criar a pasta .warden: {error}")))?;
    }
    warden_core::atomic_write(&path, after.as_bytes())
        .map_err(|error| AppError::from_domain(&error))?;
    Ok(true)
}

// ---------------------------------------------------------------------------------------------
// O último relatório (`<dados>/diagnostics/<pack>.json`)
// ---------------------------------------------------------------------------------------------

/// Pasta dos relatórios.
pub(crate) fn reports_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("diagnostics")
}

fn report_file(data_dir: &Path, pack_id: PackId) -> PathBuf {
    reports_dir(data_dir).join(format!("{pack_id}.json"))
}

/// Grava o relatório. Falha de disco só fica registrada: o relatório continua valendo na
/// resposta.
pub(crate) fn store_report(data_dir: &Path, pack_id: PackId, report: &DiagnosticsReport) {
    let path = report_file(data_dir, pack_id);
    let result = serde_json::to_vec_pretty(report)
        .map_err(|error| error.to_string())
        .and_then(|bytes| {
            fs::create_dir_all(reports_dir(data_dir)).map_err(|error| error.to_string())?;
            warden_core::atomic_write(&path, &bytes).map_err(|error| error.to_string())
        });
    if let Err(error) = result {
        tracing::warn!(%error, "relatório do diagnóstico não gravado");
    }
}

/// O último relatório gravado. Arquivo ausente ou ilegível (de outra versão): nenhum.
pub(crate) fn load_report(data_dir: &Path, pack_id: PackId) -> Option<DiagnosticsReport> {
    let bytes = fs::read(report_file(data_dir, pack_id)).ok()?;
    serde_json::from_slice(&bytes)
        .inspect_err(|error| tracing::info!(%error, "relatório do diagnóstico ilegível"))
        .ok()
}
