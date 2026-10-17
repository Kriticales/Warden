//! `W_CONFIG_RANGE` na verificação do pack (SPEC T14, CA-T14-10; C-06): as configs do pack
//! (`config/` e `defaultconfigs/`, TOML e `.cfg` do Forge antigo) com valor fora da faixa ou
//! dos valores permitidos que o próprio arquivo declara. A `warden-configs` acha as chaves; a
//! `warden-diagnostics` monta o achado, com **Restaurar padrão** quando o arquivo informa o
//! padrão.

use std::fs;
use std::path::Path;

use warden_configs::MAX_STRUCTURED_BYTES;
use warden_configs::defaults::{RangeLimit, range_issues};
use warden_diagnostics::Finding;
use warden_diagnostics::pretest::config_range::{ConfigLimit, ConfigRangeIssue, findings};

/// Pastas do pack com configs conferidas.
const FOLDERS: [&str; 2] = ["config", "defaultconfigs"];

/// Profundidade máxima de pastas dentro de cada uma (configs ficam a poucos níveis).
const MAX_DEPTH: usize = 6;

/// Os achados de faixa das configs do pack em `root`. Arquivos que não se leem são pulados.
pub(crate) fn config_range_findings(root: &Path) -> Vec<Finding> {
    let mut files = Vec::new();
    for folder in FOLDERS {
        collect(&root.join(folder), folder, 0, &mut files);
    }
    files.sort();
    let mut issues = Vec::new();
    for relative in files {
        let Ok(bytes) = fs::read(root.join(&relative)) else {
            continue;
        };
        for issue in range_issues(&relative, &bytes) {
            issues.push(ConfigRangeIssue {
                file: relative.clone(),
                key: issue.key,
                excerpt: issue.excerpt,
                value: issue.value,
                limit: match issue.limit {
                    RangeLimit::Range { text, .. } => ConfigLimit::Range(text),
                    RangeLimit::Values(values) => ConfigLimit::Values(values),
                },
                default: issue.default.map(|default| default.value),
            });
        }
    }
    findings(&issues)
}

/// Junta em `out` os caminhos relativos (com `/`) dos TOML e `.cfg` de até 2 MB.
fn collect(dir: &Path, relative: &str, depth: usize, out: &mut Vec<String>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let path = format!("{relative}/{name}");
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        if kind.is_dir() {
            if depth < MAX_DEPTH {
                collect(&entry.path(), &path, depth + 1, out);
            }
            continue;
        }
        let wanted = Path::new(&name)
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("toml") || ext.eq_ignore_ascii_case("cfg"));
        let small = entry
            .metadata()
            .is_ok_and(|meta| meta.len() <= MAX_STRUCTURED_BYTES as u64);
        if kind.is_file() && wanted && small {
            out.push(path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ca_t14_10_acha_a_chave_fora_da_faixa_nas_configs_do_pack() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        fs::create_dir_all(root.join("config/sub")).unwrap();
        fs::create_dir_all(root.join("defaultconfigs")).unwrap();
        fs::create_dir_all(root.join("mods")).unwrap();
        let toml = "[general]\n\t#Default: 64\n\t#Range: 1 ~ 512\n\tmaxChunks = 900\n";
        fs::write(root.join("config/sub/chunks-server.toml"), toml).unwrap();
        fs::write(root.join("defaultconfigs/chunks-server.toml"), toml).unwrap();
        fs::write(root.join("config/ok.toml"), "#Range: 1 ~ 2\na = 1\n").unwrap();
        // Fora das pastas de config e em formato sem faixa: não conta.
        fs::write(root.join("mods/x.pw.toml"), toml).unwrap();
        fs::write(root.join("config/a.json"), "{\"a\": 900}").unwrap();
        let found = config_range_findings(root);
        let files: Vec<&str> = found.iter().filter_map(|f| f.param("config_file")).collect();
        assert_eq!(
            files,
            ["config/sub/chunks-server.toml", "defaultconfigs/chunks-server.toml"]
        );
        assert!(found.iter().all(|f| f.rule().as_str() == "W_CONFIG_RANGE"));
        assert_eq!(found[0].param("config_default"), Some("64"));
        assert_eq!(found[0].fixes().len(), 1);
        // "Ignorar este aviso" vale por chave: cada achado tem o seu identificador.
        let ids: std::collections::BTreeSet<String> = found
            .iter()
            .map(|finding| super::super::report::finding_id(finding, &[]))
            .collect();
        assert_eq!(ids.len(), 2);
    }
}
