//! Arquivos de controle e verificação de higiene da pasta do pack (ARCHITECTURE §6.4).
//!
//! - Modelos de `.packwizignore`, `.gitignore` e `.gitattributes` que o Warden cria no pack.
//! - Bloco obrigatório do `.packwizignore`: `.warden/`, `CHANGELOG.md`, `README.md` e as sobras
//!   `*.warden-tmp` nunca entram no índice.
//! - Verificação de higiene: aponta dados de execução, segredos e arquivos de ferramentas,
//!   itens desconhecidos na raiz, arquivos grandes, pastas com cara de cache e links
//!   simbólicos, na pasta e no índice.
//!
//! Os padrões são avaliados com o mesmo matcher do `packwiz refresh` ([`PackwizIgnore`]).

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use crate::error::{Error, Result};
use crate::ignore::PackwizIgnore;
use crate::index::PackIndex;

/// Grupo de um padrão do modelo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PatternCategory {
    /// Bloco obrigatório (arquivos do próprio Warden).
    Required,
    /// Dados de execução do jogo e do launcher (`/logs/`, `/saves/`…).
    Runtime,
    /// Segredos e arquivos de ferramentas (token do KubeJS, `.vscode/`, spark…; decisão D21).
    SecretsAndTools,
    /// Só para o pacote para servidor, ou dados de servidor e instalador.
    ServerOnly,
    /// Lixo em qualquer profundidade (`*.log`, `Thumbs.db`…).
    AnyDepth,
}

/// Uma seção dos modelos de `.packwizignore` e `.gitignore`.
#[derive(Debug, Clone, Copy)]
pub struct TemplateSection {
    /// Grupo dos padrões.
    pub category: PatternCategory,
    /// Comentário da seção no `.packwizignore`.
    pub packwizignore_comment: &'static str,
    /// Comentário da seção no `.gitignore`.
    pub gitignore_comment: &'static str,
    /// Padrões, na ordem gravada.
    pub patterns: &'static [&'static str],
}

/// Linhas do bloco obrigatório do `.packwizignore`.
pub const REQUIRED_PACKWIZIGNORE_LINES: [&str; 4] =
    ["/.warden/", "/CHANGELOG.md", "/README.md", "*.warden-tmp"];

/// Padrões que o `.gitignore` não leva: o que é versionado mas não vai para o jogador.
pub const GIT_TRACKED_PATTERNS: [&str; 4] = [
    "/.warden/",
    "/CHANGELOG.md",
    "/README.md",
    "/server-overrides/",
];

/// Seções dos modelos, na ordem da ARCHITECTURE §6.4.
pub const TEMPLATE_SECTIONS: [TemplateSection; 5] = [
    TemplateSection {
        category: PatternCategory::Required,
        packwizignore_comment: "# Bloco obrigatório (o Warden sempre garante estas linhas):",
        gitignore_comment: "# Arquivos temporários do Warden:",
        patterns: &REQUIRED_PACKWIZIGNORE_LINES,
    },
    TemplateSection {
        category: PatternCategory::Runtime,
        packwizignore_comment: "# Dados de execução do jogo e do launcher:",
        gitignore_comment: "# Dados de execução do jogo e do launcher:",
        patterns: &[
            "/logs/",
            "/crash-reports/",
            "/saves/",
            "/screenshots/",
            "/debug/",
            "/stats/",
            "/natives/",
            "/libraries/",
            "/versions/",
            "/assets/",
            "/resources/",
            "/.mixin.out/",
            "/.fabric/",
            "/.quilt/",
            "/.cache/",
            "/mods/.connector/",
            "/modernfix/",
            "/journeymap/data/",
            "/XaeroWaypoints/",
            "/XaeroWorldMap/",
            "/xaero/",
            "/kubejs/probe/",
            "/kubejs/exported/",
        ],
    },
    TemplateSection {
        category: PatternCategory::SecretsAndTools,
        packwizignore_comment: "# Segredos e arquivos de ferramentas (decisão D21; R5A §2.1, R5B §11):",
        gitignore_comment: "# Segredos e arquivos de ferramentas (decisão D21; R5A §2.1, R5B §11):",
        patterns: &[
            "/kubejs/config/web_server.json",
            "/.probe/",
            "/.vscode/",
            "/local/kubejs/",
            "/config/spark/*.sparkprofile",
            "/config/spark/*.sparkheap",
            "/config/spark/tmp*",
        ],
    },
    TemplateSection {
        category: PatternCategory::ServerOnly,
        packwizignore_comment: "# Só para o pacote para servidor (§12.1); nunca vai para o jogador:",
        gitignore_comment: "# Dados de servidor e do instalador:",
        patterns: &[
            "/server-overrides/",
            "/usercache.json",
            "/usernamecache.json",
            "/launcher_profiles*.json",
            "/servers.dat_old",
            "/command_history.txt",
            "/packwiz.json",
            "/packwiz-installer*.jar",
            "/.packwiz.toml",
        ],
    },
    TemplateSection {
        category: PatternCategory::AnyDepth,
        packwizignore_comment: "# Em qualquer profundidade:",
        gitignore_comment: "# Em qualquer profundidade:",
        patterns: &[
            "jsconfig.json",
            "*.log",
            "*.log.gz",
            "hs_err_pid*.log",
            "replay_pid*.log",
            "*.heapdump",
            "Thumbs.db",
            "desktop.ini",
            "*.tmp",
            "*.bak",
            "*.old",
            "*.disabled",
        ],
    },
];

const PACKWIZIGNORE_HEADER: &str =
    "# Gerado pelo Warden. Nada abaixo deve chegar a quem joga o pack.";
const GITIGNORE_HEADER: &str =
    "# Gerado pelo Warden. Arquivos locais que não entram no histórico do pack.";

/// Conteúdo do `.gitattributes` que o Warden cria: bytes intocados pelo git, para os hashes
/// do índice continuarem valendo.
pub const GITATTRIBUTES_TEMPLATE: &str = "* -text\n";

/// O `.packwizignore` padrão do Warden (ARCHITECTURE §6.4).
#[must_use]
pub fn packwizignore_template() -> String {
    let mut lines = vec![PACKWIZIGNORE_HEADER];
    for section in TEMPLATE_SECTIONS {
        lines.push(section.packwizignore_comment);
        lines.extend_from_slice(section.patterns);
    }
    lines.join("\n") + "\n"
}

/// O `.gitignore` padrão do Warden: os mesmos padrões, exceto os versionados
/// ([`GIT_TRACKED_PATTERNS`]).
#[must_use]
pub fn gitignore_template() -> String {
    let mut lines = vec![GITIGNORE_HEADER];
    for section in TEMPLATE_SECTIONS {
        let patterns: Vec<&str> = section
            .patterns
            .iter()
            .copied()
            .filter(|pattern| !GIT_TRACKED_PATTERNS.contains(pattern))
            .collect();
        if !patterns.is_empty() {
            lines.push(section.gitignore_comment);
            lines.extend(patterns);
        }
    }
    lines.join("\n") + "\n"
}

/// Resultado de [`ensure_required_block`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequiredBlockFix {
    /// Texto final do `.packwizignore`.
    pub text: String,
    /// Linhas acrescentadas (vazio se nada mudou).
    pub added: Vec<&'static str>,
}

/// Garante o bloco obrigatório num `.packwizignore` (`None` se o arquivo não existe).
///
/// Uma linha conta como presente só se, com o arquivo como está, o `refresh` de fato deixa
/// de fora um caminho de exemplo dela (um `!padrão` posterior pode desfazer a linha). As que
/// faltam são acrescentadas no fim, onde nenhuma negação anterior as desfaz, com o mesmo fim
/// de linha do arquivo.
#[must_use]
pub fn ensure_required_block(packwizignore: Option<&str>) -> RequiredBlockFix {
    let current = packwizignore.unwrap_or_default();
    let ignore = PackwizIgnore::for_pack(Some(current));
    let probes = [
        ".warden/estado.json",
        "CHANGELOG.md",
        "README.md",
        "config/x.warden-tmp",
    ];
    let added: Vec<&'static str> = REQUIRED_PACKWIZIGNORE_LINES
        .into_iter()
        .zip(probes)
        .filter(|(_, probe)| !ignore.is_excluded(probe))
        .map(|(line, _)| line)
        .collect();
    if added.is_empty() {
        return RequiredBlockFix {
            text: current.to_owned(),
            added,
        };
    }
    let newline = if current.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    };
    let mut text = current.to_owned();
    if !text.is_empty() && !text.ends_with('\n') {
        text.push_str(newline);
    }
    text.push_str("# Bloco obrigatório do Warden:");
    text.push_str(newline);
    for line in &added {
        text.push_str(line);
        text.push_str(newline);
    }
    RequiredBlockFix { text, added }
}

/// Padrões do modelo que faltam num arquivo de ignorar (comparando linha a linha, sem
/// espaços nas pontas). Usado pela correção de higiene para completar o arquivo.
#[must_use]
pub fn missing_patterns<'a>(existing: &str, patterns: &[&'a str]) -> Vec<&'a str> {
    let present: Vec<&str> = existing.lines().map(str::trim).collect();
    patterns
        .iter()
        .copied()
        .filter(|pattern| !present.contains(pattern))
        .collect()
}

/// Arquivos permitidos na raiz do pack.
pub const ALLOWED_ROOT_FILES: [&str; 11] = [
    "pack.toml",
    "index.toml",
    "options.txt",
    "optionsof.txt",
    "optionsshaders.txt",
    "servers.dat",
    ".packwizignore",
    ".gitignore",
    ".gitattributes",
    "CHANGELOG.md",
    "README.md",
];

/// Pastas permitidas na raiz do pack (conteúdo conhecido e controle).
pub const ALLOWED_ROOT_DIRS: [&str; 14] = [
    "mods",
    "resourcepacks",
    "shaderpacks",
    "config",
    "defaultconfigs",
    "kubejs",
    "scripts",
    "datapacks",
    "openloader",
    "global_packs",
    "paxi",
    "server-overrides",
    ".warden",
    ".git",
];

/// Acima disso, um arquivo que não é jar é apontado (20 MB).
pub const LARGE_FILE_BYTES: u64 = 20 * 1024 * 1024;

/// Acima disso, uma pasta desconhecida na raiz tem cara de cache.
pub const CACHE_LIKE_FILES: usize = 1000;

/// Motivo de um item da verificação de higiene.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HygieneReason {
    /// Casa com um padrão do modelo do Warden.
    MatchesPattern {
        /// A linha do modelo.
        pattern: &'static str,
        /// O grupo da linha.
        category: PatternCategory,
    },
    /// Item na raiz que não é arquivo de controle nem pasta de conteúdo conhecida.
    UnknownRootItem,
    /// Arquivo grande que não é jar.
    LargeFile {
        /// Tamanho em bytes.
        bytes: u64,
    },
    /// Pasta desconhecida com muitos arquivos.
    CacheLikeFolder {
        /// Quantos arquivos ela tem.
        files: usize,
    },
    /// Link simbólico ou junção (o packwiz não os percorre; issue #191).
    Symlink,
}

/// Item apontado pela verificação.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HygieneItem {
    /// Caminho relativo à pasta do pack, com `/`.
    pub path: String,
    /// Se é uma pasta (o item cobre tudo dentro dela).
    pub is_dir: bool,
    /// Se existe na pasta do pack.
    pub on_disk: bool,
    /// Se está no índice (vai para quem joga).
    pub in_index: bool,
    /// Por que foi apontado.
    pub reasons: Vec<HygieneReason>,
}

/// Entrada da pasta do pack, para a verificação.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FsEntry {
    /// Caminho relativo à pasta do pack, com `/`.
    pub path: String,
    /// Tipo da entrada.
    pub kind: FsKind,
    /// Tamanho em bytes (arquivos).
    pub size: u64,
}

/// Tipo de uma [`FsEntry`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FsKind {
    /// Arquivo comum.
    File,
    /// Pasta.
    Dir,
    /// Link simbólico ou junção.
    Symlink,
}

struct HygieneMatcher {
    ignore: PackwizIgnore,
    categories: BTreeMap<&'static str, (&'static str, PatternCategory)>,
}

impl HygieneMatcher {
    fn new() -> Self {
        let mut categories = BTreeMap::new();
        let mut lines = Vec::new();
        for section in TEMPLATE_SECTIONS {
            if section.category == PatternCategory::Required {
                continue;
            }
            for &pattern in section.patterns {
                // A pasta do pacote para servidor é conteúdo legítimo do pack.
                if pattern == "/server-overrides/" {
                    continue;
                }
                categories.insert(pattern, (pattern, section.category));
                lines.push(pattern);
            }
        }
        Self {
            ignore: PackwizIgnore::from_lines(lines),
            categories,
        }
    }

    fn reason(&self, line: &str) -> Option<HygieneReason> {
        self.categories
            .get(line)
            .map(|&(pattern, category)| HygieneReason::MatchesPattern { pattern, category })
    }

    /// Padrão que casa com o caminho; pastas são testadas também com a barra final
    /// (`/logs/` casa com `logs/`, não com `logs`).
    fn check(&self, path: &str, is_dir: bool) -> Option<HygieneReason> {
        let line = self.ignore.matching_line(path).or_else(|| {
            is_dir
                .then(|| self.ignore.matching_line(&format!("{path}/")))
                .flatten()
        })?;
        self.reason(line)
    }
}

/// Verificação de higiene sobre uma lista de entradas da pasta e o índice (função pura).
#[must_use]
pub fn scan_entries(entries: &[FsEntry], index: Option<&PackIndex>) -> Vec<HygieneItem> {
    let matcher = HygieneMatcher::new();
    let mut items: BTreeMap<String, HygieneItem> = BTreeMap::new();
    let mut sorted: Vec<&FsEntry> = entries.iter().collect();
    sorted.sort_by(|a, b| a.path.cmp(&b.path));

    let mut reported_dirs: Vec<String> = Vec::new();
    let mut files_per_root: BTreeMap<&str, usize> = BTreeMap::new();
    for entry in &sorted {
        let root = entry.path.split('/').next().unwrap_or_default();
        if entry.kind == FsKind::File && entry.path.contains('/') {
            *files_per_root.entry(root).or_default() += 1;
        }
    }

    for entry in sorted {
        let path = entry.path.as_str();
        let under_reported = reported_dirs.iter().any(|dir| {
            path.len() > dir.len()
                && path.starts_with(dir.as_str())
                && path.as_bytes()[dir.len()] == b'/'
        });
        if under_reported || path == ".git" || path.starts_with(".git/") {
            continue;
        }
        let is_dir = entry.kind == FsKind::Dir;
        let mut reasons = Vec::new();
        if entry.kind == FsKind::Symlink {
            reasons.push(HygieneReason::Symlink);
        }
        if let Some(reason) = matcher.check(path, is_dir) {
            reasons.push(reason);
            if is_dir {
                reported_dirs.push(path.to_owned());
            }
        }
        let matched_pattern = reasons
            .iter()
            .any(|reason| matches!(reason, HygieneReason::MatchesPattern { .. }));
        if !path.contains('/') && !matched_pattern {
            let allowed = match entry.kind {
                FsKind::Dir => ALLOWED_ROOT_DIRS.contains(&path),
                FsKind::File | FsKind::Symlink => ALLOWED_ROOT_FILES.contains(&path),
            };
            if !allowed {
                reasons.push(HygieneReason::UnknownRootItem);
                if is_dir {
                    let files = files_per_root.get(path).copied().unwrap_or_default();
                    if files > CACHE_LIKE_FILES {
                        reasons.push(HygieneReason::CacheLikeFolder { files });
                    }
                }
            }
        }
        if entry.kind == FsKind::File
            && entry.size > LARGE_FILE_BYTES
            && !std::path::Path::new(path)
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("jar"))
        {
            reasons.push(HygieneReason::LargeFile { bytes: entry.size });
        }
        if !reasons.is_empty() {
            items.insert(
                path.to_owned(),
                HygieneItem {
                    path: path.to_owned(),
                    is_dir,
                    on_disk: true,
                    in_index: false,
                    reasons,
                },
            );
        }
    }

    if let Some(index) = index {
        mark_index_entries(&matcher, index, &mut items);
    }
    items.into_values().collect()
}

/// Marca os itens que estão no índice e acrescenta as entradas do índice que casam com os
/// padrões mesmo sem existir na pasta.
fn mark_index_entries(
    matcher: &HygieneMatcher,
    index: &PackIndex,
    items: &mut BTreeMap<String, HygieneItem>,
) {
    for entry in index.normalized_entries() {
        let file = entry.file.as_str();
        let covering_dir = items
            .values_mut()
            .find(|item| item.is_dir && file.starts_with(&format!("{}/", item.path)));
        if let Some(item) = covering_dir {
            item.in_index = true;
            continue;
        }
        if let Some(item) = items.get_mut(file) {
            item.in_index = true;
            continue;
        }
        let line = matcher.ignore.excluding_line(file);
        if let Some(reason) = line.and_then(|line| matcher.reason(line)) {
            items.insert(
                file.to_owned(),
                HygieneItem {
                    path: file.to_owned(),
                    is_dir: false,
                    on_disk: false,
                    in_index: true,
                    reasons: vec![reason],
                },
            );
        }
    }
}

/// Lê a pasta do pack (sem seguir links) e roda a verificação de higiene.
pub fn scan_dir(root: &Path, index: Option<&PackIndex>) -> Result<Vec<HygieneItem>> {
    let mut entries = Vec::new();
    collect_entries(root, "", &mut entries)?;
    Ok(scan_entries(&entries, index))
}

fn collect_entries(root: &Path, prefix: &str, entries: &mut Vec<FsEntry>) -> Result<()> {
    let directory = if prefix.is_empty() {
        root.to_path_buf()
    } else {
        root.join(prefix)
    };
    let io_error = |action, path: &Path, source| Error::Io {
        action,
        path: path.to_path_buf(),
        source,
    };
    let reader =
        fs::read_dir(&directory).map_err(|source| io_error("listar", &directory, source))?;
    for item in reader {
        let item = item.map_err(|source| io_error("listar", &directory, source))?;
        let name = item.file_name().to_string_lossy().into_owned();
        let path = if prefix.is_empty() {
            name.clone()
        } else {
            format!("{prefix}/{name}")
        };
        if path == ".git" {
            continue;
        }
        let metadata = fs::symlink_metadata(item.path())
            .map_err(|source| io_error("ler", &item.path(), source))?;
        let kind = if metadata.file_type().is_symlink() || is_junction(&metadata) {
            FsKind::Symlink
        } else if metadata.is_dir() {
            FsKind::Dir
        } else {
            FsKind::File
        };
        entries.push(FsEntry {
            path: path.clone(),
            kind,
            size: metadata.len(),
        });
        if kind == FsKind::Dir {
            collect_entries(root, &path, entries)?;
        }
    }
    Ok(())
}

#[cfg(windows)]
fn is_junction(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt as _;
    // FILE_ATTRIBUTE_REPARSE_POINT: junções e outros pontos de nova análise.
    metadata.file_attributes() & 0x400 != 0
}

#[cfg(not(windows))]
fn is_junction(_metadata: &fs::Metadata) -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::index::IndexEntry;

    fn file(path: &str) -> FsEntry {
        FsEntry {
            path: path.to_owned(),
            kind: FsKind::File,
            size: 10,
        }
    }

    fn dir(path: &str) -> FsEntry {
        FsEntry {
            path: path.to_owned(),
            kind: FsKind::Dir,
            size: 0,
        }
    }

    const EXPECTED_PACKWIZIGNORE: &str =
        "# Gerado pelo Warden. Nada abaixo deve chegar a quem joga o pack.
# Bloco obrigatório (o Warden sempre garante estas linhas):
/.warden/
/CHANGELOG.md
/README.md
*.warden-tmp
# Dados de execução do jogo e do launcher:
/logs/
/crash-reports/
/saves/
/screenshots/
/debug/
/stats/
/natives/
/libraries/
/versions/
/assets/
/resources/
/.mixin.out/
/.fabric/
/.quilt/
/.cache/
/mods/.connector/
/modernfix/
/journeymap/data/
/XaeroWaypoints/
/XaeroWorldMap/
/xaero/
/kubejs/probe/
/kubejs/exported/
# Segredos e arquivos de ferramentas (decisão D21; R5A §2.1, R5B §11):
/kubejs/config/web_server.json
/.probe/
/.vscode/
/local/kubejs/
/config/spark/*.sparkprofile
/config/spark/*.sparkheap
/config/spark/tmp*
# Só para o pacote para servidor (§12.1); nunca vai para o jogador:
/server-overrides/
/usercache.json
/usernamecache.json
/launcher_profiles*.json
/servers.dat_old
/command_history.txt
/packwiz.json
/packwiz-installer*.jar
/.packwiz.toml
# Em qualquer profundidade:
jsconfig.json
*.log
*.log.gz
hs_err_pid*.log
replay_pid*.log
*.heapdump
Thumbs.db
desktop.ini
*.tmp
*.bak
*.old
*.disabled
";

    #[test]
    fn modelo_igual_ao_da_arquitetura() {
        assert_eq!(packwizignore_template(), EXPECTED_PACKWIZIGNORE);
    }

    #[test]
    fn gitignore_sem_os_versionados() {
        let text = gitignore_template();
        for pattern in GIT_TRACKED_PATTERNS {
            assert!(!text.lines().any(|line| line == pattern), "{pattern}");
        }
        for line in EXPECTED_PACKWIZIGNORE.lines() {
            if !line.starts_with('#') && !GIT_TRACKED_PATTERNS.contains(&line) {
                assert!(text.lines().any(|l| l == line), "{line}");
            }
        }
        assert!(text.contains("*.warden-tmp"));
        assert_eq!(GITATTRIBUTES_TEMPLATE, "* -text\n");
    }

    #[test]
    fn ca_t04_05_modelos_cobrem_segredos_e_ferramentas() {
        let ignore = PackwizIgnore::for_pack(Some(&packwizignore_template()));
        for path in [
            "kubejs/config/web_server.json",
            ".probe/x.json",
            ".vscode/settings.json",
            "local/kubejs/a.js",
            "config/spark/perfil.sparkprofile",
            "config/spark/heap.sparkheap",
            "config/spark/tmp-1/x",
            ".warden/estado.json",
            "CHANGELOG.md",
            "logs/latest.log",
            "kubejs/assets/x/y.png.bak",
        ] {
            assert!(ignore.is_excluded(path), "{path}");
        }
        for path in [
            "kubejs/assets/x/y.png",
            "config/debug/x.toml",
            "config/spark/config.json",
            "local/outro.txt",
            "mods/sodium.pw.toml",
            "kubejs/config/common.json",
        ] {
            assert!(!ignore.is_excluded(path), "{path}");
        }
        let git = PackwizIgnore::from_lines(gitignore_template().lines());
        for path in [
            "kubejs/config/web_server.json",
            ".probe/x.json",
            ".vscode/settings.json",
            "local/kubejs/a.js",
            "config/spark/perfil.sparkprofile",
        ] {
            assert!(git.is_excluded(path), "{path}");
        }
        assert!(!git.is_excluded("CHANGELOG.md"));
        assert!(!git.is_excluded(".warden/estado.json"));
    }

    #[test]
    fn ca_t04_05_verificacao_aponta_os_cinco() {
        let entries = [
            file("pack.toml"),
            dir("kubejs"),
            dir("kubejs/config"),
            file("kubejs/config/web_server.json"),
            dir(".probe"),
            file(".probe/x.json"),
            dir(".vscode"),
            file(".vscode/settings.json"),
            dir("local"),
            dir("local/kubejs"),
            file("local/kubejs/a.js"),
            dir("config"),
            dir("config/spark"),
            file("config/spark/perfil.sparkprofile"),
        ];
        let items = scan_entries(&entries, None);
        let paths: Vec<&str> = items.iter().map(|item| item.path.as_str()).collect();
        for expected in [
            "kubejs/config/web_server.json",
            ".probe",
            ".vscode",
            "local/kubejs",
            "config/spark/perfil.sparkprofile",
        ] {
            let item = items.iter().find(|item| item.path == expected);
            let item = item.unwrap_or_else(|| panic!("{expected} não apontado: {paths:?}"));
            assert!(
                item.reasons.iter().any(|reason| matches!(
                    reason,
                    HygieneReason::MatchesPattern {
                        category: PatternCategory::SecretsAndTools,
                        ..
                    }
                )),
                "{expected}: {:?}",
                item.reasons
            );
        }
        // `local/` sozinha é só um item desconhecido na raiz (alguns packs a usam).
        let local = items.iter().find(|item| item.path == "local").unwrap();
        assert_eq!(local.reasons, [HygieneReason::UnknownRootItem]);
    }

    #[test]
    fn raiz_grandes_cache_links_e_indice() {
        let mut entries = vec![
            file("pack.toml"),
            file("index.toml"),
            file("notas.txt"),
            dir("mods"),
            FsEntry {
                path: "mods/grande.jar".to_owned(),
                kind: FsKind::File,
                size: LARGE_FILE_BYTES + 1,
            },
            dir("config"),
            FsEntry {
                path: "config/mapa.dat".to_owned(),
                kind: FsKind::File,
                size: LARGE_FILE_BYTES + 1,
            },
            FsEntry {
                path: "atalho".to_owned(),
                kind: FsKind::Symlink,
                size: 0,
            },
            dir("logs"),
            file("logs/latest.log"),
            dir(".git"),
            file(".git/HEAD"),
            dir("cachezinho"),
        ];
        for number in 0..=CACHE_LIKE_FILES {
            entries.push(file(&format!("cachezinho/{number}.bin")));
        }
        let mut index = PackIndex::default();
        index.files.push(IndexEntry::new("logs/latest.log", "h"));
        index
            .files
            .push(IndexEntry::new("saves/mundo/level.dat", "h"));
        index.files.push(IndexEntry::new("config/ok.toml", "h"));
        index.files.push(IndexEntry::new("notas.txt", "h"));
        let items = scan_entries(&entries, Some(&index));
        let find = |path: &str| items.iter().find(|item| item.path == path);

        assert_eq!(
            find("notas.txt").unwrap().reasons,
            [HygieneReason::UnknownRootItem]
        );
        assert!(find("notas.txt").unwrap().in_index);
        assert!(find("mods/grande.jar").is_none());
        assert_eq!(
            find("config/mapa.dat").unwrap().reasons,
            [HygieneReason::LargeFile {
                bytes: LARGE_FILE_BYTES + 1
            }]
        );
        assert_eq!(
            find("atalho").unwrap().reasons,
            [HygieneReason::Symlink, HygieneReason::UnknownRootItem]
        );
        let logs = find("logs").unwrap();
        assert!(logs.is_dir && logs.in_index);
        assert!(find("logs/latest.log").is_none());
        let saves = find("saves/mundo/level.dat").unwrap();
        assert!(!saves.on_disk && saves.in_index);
        assert_eq!(
            find("cachezinho").unwrap().reasons,
            [
                HygieneReason::UnknownRootItem,
                HygieneReason::CacheLikeFolder {
                    files: CACHE_LIKE_FILES + 1
                }
            ]
        );
        assert!(find(".git").is_none());
        assert!(find("config/ok.toml").is_none());
        assert!(find("pack.toml").is_none());
    }

    #[test]
    fn bloco_obrigatorio() {
        let fix = ensure_required_block(None);
        assert_eq!(fix.added, REQUIRED_PACKWIZIGNORE_LINES);
        assert_eq!(
            fix.text,
            "# Bloco obrigatório do Warden:\n/.warden/\n/CHANGELOG.md\n/README.md\n*.warden-tmp\n"
        );
        let fix = ensure_required_block(Some(&packwizignore_template()));
        assert!(fix.added.is_empty());
        assert_eq!(fix.text, packwizignore_template());

        // Arquivo de outro app, com CRLF, sem quebra no fim e com negação que desfaz uma linha.
        let fix = ensure_required_block(Some("/.warden/\r\n!/.warden/estado.json\r\n/README.md"));
        assert_eq!(fix.added, ["/.warden/", "/CHANGELOG.md", "*.warden-tmp"]);
        assert!(fix.text.ends_with(
            "\r\n# Bloco obrigatório do Warden:\r\n/.warden/\r\n/CHANGELOG.md\r\n*.warden-tmp\r\n"
        ));
        let ignore = PackwizIgnore::for_pack(Some(&fix.text));
        assert!(ignore.is_excluded(".warden/estado.json"));
    }

    #[test]
    fn padroes_faltantes() {
        let missing = missing_patterns("/logs/\n  *.log  \n", &["/logs/", "*.log", "/saves/"]);
        assert_eq!(missing, ["/saves/"]);
    }

    #[test]
    fn pasta_real() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        fs::create_dir_all(root.join("logs")).unwrap();
        fs::write(root.join("logs/latest.log"), "x").unwrap();
        fs::create_dir_all(root.join("config")).unwrap();
        fs::write(root.join("config/a.toml"), "x").unwrap();
        fs::write(root.join("pack.toml"), "x").unwrap();
        fs::create_dir_all(root.join(".git")).unwrap();
        fs::write(root.join(".git/HEAD"), "x").unwrap();
        let items = scan_dir(root, None).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].path, "logs");
        assert!(scan_dir(&root.join("nada"), None).is_err());
    }
}
