//! Camada 4: o rótulo e a ajuda de uma chave no arquivo de idioma do jar (R5B §2.2; lidos por
//! `warden_jarmeta::lang`).
//!
//! As chaves de tradução procuradas, a partir do nome do arquivo e do caminho da chave:
//!
//! - `NeoForge`, `config/<modid>-<client|common|server|startup>.toml`:
//!   `<modid>.configuration.<chave>` (o nome da chave; também o caminho inteiro com pontos);
//! - `AutoConfig`, `config/<nome>.json|json5|toml`: `text.autoconfig.<nome>.option.<caminho>`
//!   (e só o nome do campo);
//! - `owo-config`, `config/<nome>.json5`: `text.config.<nome>.option.<caminho>`;
//! - `MidnightLib`, `config/<modid>.json`: `<modid>.midnightconfig.<campo>`.
//!
//! A ajuda é a mesma chave com `.tooltip` (`NeoForge`, `owo-config`, `MidnightLib`) ou `.@Tooltip`
//! (`AutoConfig`). Códigos de formatação do Minecraft (`§a`) saem do texto.

use warden_jarmeta::lang::{ConfigLang, LangLocale};

use crate::format::ConfigFormat;
use crate::path::{KeyPath, PathSegment};

/// Rótulo encontrado para uma chave.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Found {
    pub text: String,
    pub tooltip: Option<String>,
    pub key: String,
    pub locale: LangLocale,
}

/// Sufixos do nome de config do `ModConfigSpec` (`<modid>-common.toml`).
const SPEC_SUFFIXES: [&str; 4] = ["-client", "-common", "-server", "-startup"];

/// As chaves de tradução candidatas, na ordem de preferência.
pub(crate) fn candidates(file_name: &str, format: ConfigFormat, path: &KeyPath) -> Vec<String> {
    let Some(stem) = stem(file_name) else {
        return Vec::new();
    };
    let names: Vec<&str> = path
        .segments()
        .iter()
        .filter_map(|segment| match segment {
            PathSegment::Key(key) => Some(key.as_str()),
            PathSegment::Index(_) => None,
        })
        .collect();
    if names.len() != path.len() {
        return Vec::new();
    }
    let (Some(leaf), dotted) = (names.last(), names.join(".")) else {
        return Vec::new();
    };
    let mut keys = Vec::new();
    match format {
        ConfigFormat::Toml => {
            if let Some(modid) = SPEC_SUFFIXES
                .iter()
                .find_map(|suffix| stem.strip_suffix(suffix))
            {
                keys.push(format!("{modid}.configuration.{dotted}"));
                keys.push(format!("{modid}.configuration.{leaf}"));
            }
            keys.push(format!("text.autoconfig.{stem}.option.{dotted}"));
        }
        ConfigFormat::Json | ConfigFormat::Json5 => {
            keys.push(format!("text.autoconfig.{stem}.option.{dotted}"));
            keys.push(format!("text.autoconfig.{stem}.option.{leaf}"));
            keys.push(format!("text.config.{stem}.option.{dotted}"));
            keys.push(format!("{stem}.midnightconfig.{leaf}"));
        }
        ConfigFormat::Properties | ConfigFormat::LegacyCfg | ConfigFormat::OptionsTxt => {}
    }
    keys.dedup();
    keys
}

/// O nome do arquivo sem a pasta e sem a extensão.
fn stem(file_name: &str) -> Option<&str> {
    let name = file_name.rsplit(['/', '\\']).next()?;
    let stem = name.rsplit_once('.').map_or(name, |(stem, _)| stem);
    (!stem.is_empty()).then_some(stem)
}

/// Tira os códigos de formatação (`§` e o caractere seguinte) e os espaços das pontas.
fn plain(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c == '§' {
            chars.next();
        } else {
            out.push(c);
        }
    }
    out.trim().to_owned()
}

/// O primeiro rótulo que existe entre as candidatas, com a ajuda.
pub(crate) fn find(
    lang: &ConfigLang,
    file_name: &str,
    format: ConfigFormat,
    path: &KeyPath,
) -> Option<Found> {
    candidates(file_name, format, path)
        .into_iter()
        .find_map(|key| {
            let label = lang.get(&key)?;
            let text = plain(label.text);
            if text.is_empty() {
                return None;
            }
            let tooltip = [".tooltip", ".@Tooltip", ".@Tooltip[0]"]
                .iter()
                .find_map(|suffix| lang.get(&format!("{key}{suffix}")))
                .map(|tip| plain(tip.text))
                .filter(|tip| !tip.is_empty());
            Some(Found {
                text,
                tooltip,
                locale: label.locale,
                key,
            })
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn path(keys: &[&str]) -> KeyPath {
        KeyPath::from_keys(keys.iter().copied())
    }

    #[test]
    fn candidatas_por_formato() {
        assert_eq!(
            candidates(
                "config/create-common.toml",
                ConfigFormat::Toml,
                &path(&["kinetics", "maxSpeed"])
            ),
            [
                "create.configuration.kinetics.maxSpeed",
                "create.configuration.maxSpeed",
                "text.autoconfig.create-common.option.kinetics.maxSpeed",
            ]
        );
        assert_eq!(
            candidates(
                "config/inmis.json",
                ConfigFormat::Json,
                &path(&["enableTrinkets"])
            ),
            [
                "text.autoconfig.inmis.option.enableTrinkets",
                "text.config.inmis.option.enableTrinkets",
                "inmis.midnightconfig.enableTrinkets",
            ]
        );
        assert!(candidates("options.txt", ConfigFormat::OptionsTxt, &path(&["fov"])).is_empty());
        assert!(
            candidates(
                "a.json",
                ConfigFormat::Json,
                &KeyPath::root().child_index(0)
            )
            .is_empty()
        );
        assert!(candidates("a.json", ConfigFormat::Json, &KeyPath::root()).is_empty());
        assert!(candidates(".json", ConfigFormat::Json, &path(&["a"])).is_empty());
        assert_eq!(stem("semextensao"), Some("semextensao"));
    }

    #[test]
    fn texto_sem_codigos_de_formatacao() {
        assert_eq!(plain(" §aVerde§r normal §"), "Verde normal");
    }
}
