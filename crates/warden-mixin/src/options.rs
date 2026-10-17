//! Opções que desligam mixins (S-R5-1 §6 itens 1 e 5): uma classe de mixin desligada some do
//! cruzamento.
//!
//! - **Pelo pacote**: os mods de otimização (Lithium, Radium, Canary, `ModernFix`, Sodium…) dão a
//!   cada pacote de mixin uma opção com o mesmo nome (`.../mixin/world/player_chunk_tick/X` →
//!   `mixin.world.player_chunk_tick`). Outros mods desligam opções no próprio descritor
//!   (`"lithium:options": {"mixin.chunk.palette": false}` no `fabric.mod.json`) e o pack, nas
//!   configs (`config/lithium.properties`).
//! - **Por dado curado**: opções que não seguem o pacote e ligam uma config inteira
//!   (`useSmallThreadingDetector` do `FerriteCore` liga `ferritecore.threaddetec.mixin.json`),
//!   em `data/mixin-options.toml`.
//!
//! As opções desligadas viram textos `mod:mixin.a.b` (pacote) e `config:<nome da config>`
//! (config inteira).

use std::collections::BTreeSet;
use std::path::Path;
use std::sync::OnceLock;

use serde::Deserialize;

/// Famílias que leem `<família>:options` nos descritores de outros mods.
const OPTION_FAMILIES: [&str; 3] = ["lithium", "canary", "radium"];

/// Um arquivo de config do pack que desliga mixins pelo pacote.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PackageOptionsFile {
    /// Nome do arquivo em `config/` (`lithium.properties`).
    pub arquivo: String,
    /// Ids dos mods cujas opções o arquivo controla.
    pub mods: Vec<String>,
    /// De onde veio a informação.
    pub origem: String,
}

/// Uma opção que liga ou desliga configs de mixin inteiras.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConfigOption {
    /// Ids do mod (as versões para cada loader).
    pub mods: Vec<String>,
    /// Arquivos em `config/` onde a opção pode estar (`.properties` ou `.toml`).
    pub arquivos: Vec<String>,
    /// Chave.
    pub chave: String,
    /// Valor quando o arquivo não existe ou não traz a chave.
    pub padrao: bool,
    /// Configs de mixin que só valem com a opção ligada.
    pub configs: Vec<String>,
    /// De onde veio a informação.
    pub origem: String,
}

/// O conteúdo de `data/mixin-options.toml`.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OptionsData {
    /// Arquivos que desligam mixins pelo pacote.
    pub arquivo: Vec<PackageOptionsFile>,
    /// Opções que ligam configs inteiras.
    pub opcao: Vec<ConfigOption>,
}

const OPTIONS_TOML: &str = include_str!("../data/mixin-options.toml");

/// Os dados embutidos (validados pelos testes; inválidos viram listas vazias).
#[must_use]
pub fn options_data() -> &'static OptionsData {
    static DATA: OnceLock<OptionsData> = OnceLock::new();
    DATA.get_or_init(|| {
        parse_options_data(OPTIONS_TOML).unwrap_or_else(|error| {
            tracing::error!(%error, "mixin-options.toml inválido");
            OptionsData {
                arquivo: Vec::new(),
                opcao: Vec::new(),
            }
        })
    })
}

/// Lê e valida um `mixin-options.toml`.
///
/// # Errors
///
/// TOML inválido, campo desconhecido, texto vazio ou opção sem config.
pub fn parse_options_data(text: &str) -> Result<OptionsData, String> {
    let data: OptionsData = toml::from_str(text).map_err(|e| e.to_string())?;
    for file in &data.arquivo {
        if file.arquivo.trim().is_empty() || file.mods.is_empty() || file.origem.trim().is_empty() {
            return Err(format!("arquivo incompleto: {}", file.arquivo));
        }
        if file.arquivo.contains(['/', '\\']) {
            return Err(format!("arquivo fora de config/: {}", file.arquivo));
        }
    }
    for option in &data.opcao {
        if option.mods.is_empty()
            || option.arquivos.is_empty()
            || option.chave.trim().is_empty()
            || option.configs.is_empty()
            || option.origem.trim().is_empty()
        {
            return Err(format!("opção incompleta: {}", option.chave));
        }
        if option.arquivos.iter().any(|a| a.contains(['/', '\\'])) {
            return Err(format!("arquivo fora de config/: {}", option.chave));
        }
    }
    Ok(data)
}

/// `.../mixin/chunk/palette/PalettedContainerMixin` → `mixin.chunk.palette`.
#[must_use]
pub fn mixin_option(class_name: &str) -> Option<String> {
    let start = class_name.find("/mixin/")?;
    let rest = &class_name[start + "/mixin/".len()..];
    let package = rest.rsplit_once('/').map(|(package, _)| package)?;
    Some(format!("mixin.{}", package.replace('/', ".")))
}

/// A classe `class_name` do mod `mod_id` foi desligada por alguma opção (dela ou de um pacote
/// acima: desligar `mixin.chunk` desliga `mixin.chunk.palette`), ou a config dela inteira.
#[must_use]
pub fn is_disabled(
    mod_id: &str,
    class_name: &str,
    config: &str,
    disabled: &BTreeSet<String>,
) -> bool {
    if disabled.contains(&format!("config:{config}")) {
        return true;
    }
    let Some(option) = mixin_option(class_name) else {
        return false;
    };
    let mut prefix = String::new();
    for part in option.split('.') {
        if !prefix.is_empty() {
            prefix.push('.');
        }
        prefix.push_str(part);
        if disabled.contains(&format!("{mod_id}:{prefix}")) {
            return true;
        }
    }
    false
}

/// `lithium:options` (e `canary:`/`radium:`) num descritor: `família:opção` para cada opção
/// posta em `false`. Vale para o `fabric.mod.json` (objeto) e para o `mods.toml` (tabela).
#[must_use]
pub fn declared_disables(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for family in OPTION_FAMILIES {
        let key = format!("{family}:options");
        let mut from = 0;
        while let Some(found) = text[from..].find(&key) {
            let start = from + found + key.len();
            let mut end = (start + 2000).min(text.len());
            while !text.is_char_boundary(end) {
                end -= 1;
            }
            let window = &text[start..end];
            // Pula o `"`/`]`/`=`/`{` logo depois da chave; para no fim do bloco.
            let body_start = window.find(['{', ']']).map_or(0, |i| i + 1);
            let body = &window[body_start..];
            let stop = body.find(['}', '[']).unwrap_or(body.len());
            for part in body[..stop].split([',', '\n']) {
                let part = part.replace('"', "");
                if let Some((option, value)) = part.split_once(['=', ':'])
                    && value.trim() == "false"
                    && option.trim().starts_with("mixin.")
                {
                    out.push(format!("{family}:{}", option.trim()));
                }
            }
            from = start;
        }
    }
    out
}

/// `chave = valor` de um `.properties` ou de um TOML simples (sem seções que importem aqui).
fn key_values(text: &str) -> impl Iterator<Item = (&str, &str)> {
    text.lines().filter_map(|line| {
        let line = line.trim();
        if line.starts_with(['#', '!', '[']) {
            return None;
        }
        let (key, value) = line.split_once('=')?;
        let value = value.split('#').next().unwrap_or(value);
        Some((key.trim().trim_matches('"'), value.trim().trim_matches('"')))
    })
}

/// As opções desligadas pelas configs do pack (`<instância>/config`).
#[must_use]
pub fn disabled_by_pack_configs(config_dir: &Path, data: &OptionsData) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for file in &data.arquivo {
        let Ok(text) = std::fs::read_to_string(config_dir.join(&file.arquivo)) else {
            continue;
        };
        for (key, value) in key_values(&text) {
            if value == "false" && key.starts_with("mixin.") {
                for module in &file.mods {
                    out.insert(format!("{module}:{key}"));
                }
            }
        }
    }
    for option in &data.opcao {
        let mut enabled = option.padrao;
        for name in &option.arquivos {
            let Ok(text) = std::fs::read_to_string(config_dir.join(name)) else {
                continue;
            };
            if let Some((_, value)) = key_values(&text).find(|(key, _)| *key == option.chave) {
                enabled = value.eq_ignore_ascii_case("true");
                break;
            }
        }
        if !enabled {
            out.extend(option.configs.iter().map(|c| format!("config:{c}")));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dados_embutidos_validos() {
        let data = parse_options_data(OPTIONS_TOML).unwrap();
        assert!(
            data.arquivo
                .iter()
                .any(|f| f.arquivo == "lithium.properties")
        );
        assert!(data.opcao.iter().any(|o| {
            o.chave == "useSmallThreadingDetector"
                && o.configs
                    .contains(&"ferritecore.threaddetec.mixin.json".to_owned())
        }));
        assert_eq!(options_data().arquivo.len(), data.arquivo.len());
    }

    #[test]
    fn esquema_recusa_o_que_nao_serve() {
        for bad in [
            "arquivo = []\nopcao = []\nextra = 1",
            "opcao = []\n[[arquivo]]\narquivo = \"\"\nmods = [\"a\"]\norigem = \"x\"",
            "opcao = []\n[[arquivo]]\narquivo = \"../a\"\nmods = [\"a\"]\norigem = \"x\"",
            "arquivo = []\n[[opcao]]\nmods = [\"a\"]\narquivos = [\"a\"]\nchave = \"k\"\npadrao = false\nconfigs = []\norigem = \"x\"",
            "isto não é toml",
        ] {
            assert!(parse_options_data(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn opcao_pelo_pacote() {
        assert_eq!(
            mixin_option("me/jellysquid/mods/lithium/mixin/chunk/palette/PalettedContainerMixin")
                .as_deref(),
            Some("mixin.chunk.palette")
        );
        assert_eq!(mixin_option("a/b/C"), None);
        let disabled: BTreeSet<String> = ["lithium:mixin.chunk".to_owned()].into();
        assert!(is_disabled(
            "lithium",
            "x/mixin/chunk/palette/A",
            "l.json",
            &disabled
        ));
        assert!(!is_disabled(
            "lithium",
            "x/mixin/chunky/A",
            "l.json",
            &disabled
        ));
        assert!(!is_disabled(
            "radium",
            "x/mixin/chunk/palette/A",
            "l.json",
            &disabled
        ));
        let config: BTreeSet<String> = ["config:f.json".to_owned()].into();
        assert!(is_disabled("ferritecore", "a/B", "f.json", &config));
    }

    #[test]
    fn opcoes_desligadas_nos_descritores() {
        let fabric = r#"{"custom": {"lithium:options": {"mixin.chunk.palette": false, "mixin.alloc": true}}}"#;
        assert_eq!(
            declared_disables(fabric),
            vec!["lithium:mixin.chunk.palette"]
        );
        let toml = "[\"lithium:options\"]\n\"mixin.world.player_chunk_tick\" = false\n\"mixin.x\" = true\n[[mods]]\nmodId=\"a\"";
        assert_eq!(
            declared_disables(toml),
            vec!["lithium:mixin.world.player_chunk_tick"]
        );
        assert!(declared_disables("nada aqui ç").is_empty());
        // Texto cortado no meio de um caractere de vários bytes não derruba.
        let long = format!("canary:options{}", "ç".repeat(3000));
        assert!(declared_disables(&long).is_empty());
    }

    #[test]
    fn configs_do_pack() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("lithium.properties"),
            "# comentário\nmixin.ai.pathing=false\nmixin.alloc = true\nmixin.gen=false # fim\n",
        )
        .unwrap();
        std::fs::write(
            dir.path().join("ferritecore-mixin.toml"),
            "#x\nuseSmallThreadingDetector = false\n",
        )
        .unwrap();
        let data = parse_options_data(OPTIONS_TOML).unwrap();
        let disabled = disabled_by_pack_configs(dir.path(), &data);
        assert!(disabled.contains("lithium:mixin.ai.pathing"));
        assert!(disabled.contains("lithium:mixin.gen"));
        assert!(!disabled.contains("lithium:mixin.alloc"));
        assert!(disabled.contains("config:ferritecore.threaddetec.mixin.json"));

        // Sem o arquivo vale o padrão (desligado); ligado no arquivo, a config fica.
        let empty = tempfile::tempdir().unwrap();
        assert!(
            disabled_by_pack_configs(empty.path(), &data)
                .contains("config:ferritecore.threaddetec.mixin.json")
        );
        std::fs::write(
            empty.path().join("ferritecore.mixin.properties"),
            "useSmallThreadingDetector = true\n",
        )
        .unwrap();
        assert!(
            !disabled_by_pack_configs(empty.path(), &data)
                .contains("config:ferritecore.threaddetec.mixin.json")
        );
    }
}
