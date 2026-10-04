//! Formatos de config que o Warden lê e edita sem perda (ARCHITECTURE §10).

use std::fmt;
use std::path::Path;

use serde::{Deserialize, Serialize};

/// Formato de um arquivo de config.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ConfigFormat {
    /// TOML (Forge/NeoForge `ModConfigSpec`, Fabric e outros), lido com `toml_edit`.
    Toml,
    /// JSON e JSONC (JSON com comentários, como o Gson tolerante lê), lido com `jsonc-parser`.
    Json,
    /// JSON5 (Jankson, owo-config, YACL), lido com `jsonc-parser` no modo JSON5.
    Json5,
    /// `.properties` do Java (`java.util.Properties`), parser próprio por linha.
    Properties,
    /// `.cfg` do `Configuration` do Forge antigo (1.7.10–1.12.2), parser próprio por linha.
    LegacyCfg,
    /// `options.txt` do Minecraft (`chave:valor`), parser próprio por linha.
    OptionsTxt,
}

impl ConfigFormat {
    /// Todos os formatos, na ordem da ARCHITECTURE §10.
    pub const ALL: [Self; 6] = [
        Self::Toml,
        Self::Json,
        Self::Json5,
        Self::Properties,
        Self::LegacyCfg,
        Self::OptionsTxt,
    ];

    /// Formato pelo nome do arquivo, sem olhar o conteúdo.
    ///
    /// `.cfg` só vira [`ConfigFormat::LegacyCfg`] se o conteúdo também for do `Configuration` do
    /// Forge; para isso use [`ConfigFormat::detect`]. `optionsof.txt` (do `OptiFine`) usa o formato
    /// do `options.txt`; `optionsshaders.txt` usa `chave=valor`, como um `.properties`.
    #[must_use]
    pub fn from_file_name(path: &Path) -> Option<Self> {
        let name = path.file_name()?.to_str()?.to_ascii_lowercase();
        match name.as_str() {
            "options.txt" | "optionsof.txt" => return Some(Self::OptionsTxt),
            "optionsshaders.txt" => return Some(Self::Properties),
            _ => {}
        }
        let extension = name.rsplit_once('.')?.1;
        match extension {
            "toml" => Some(Self::Toml),
            "json" | "jsonc" | "mcmeta" => Some(Self::Json),
            "json5" => Some(Self::Json5),
            "properties" => Some(Self::Properties),
            "cfg" => Some(Self::LegacyCfg),
            _ => None,
        }
    }

    /// Formato pelo nome e pelo conteúdo: confirma que um `.cfg` é do `Configuration` do Forge
    /// (outros programas também usam `.cfg`, com sintaxe de INI). Devolve `None` quando o
    /// arquivo deve ficar só no editor de texto.
    #[must_use]
    pub fn detect(path: &Path, bytes: &[u8]) -> Option<Self> {
        let format = Self::from_file_name(path)?;
        if format == Self::LegacyCfg && !crate::formats::legacy_cfg::looks_like_forge(bytes) {
            return None;
        }
        Some(format)
    }

    /// Nome curto do formato, para mensagens técnicas.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Toml => "TOML",
            Self::Json => "JSON",
            Self::Json5 => "JSON5",
            Self::Properties => ".properties",
            Self::LegacyCfg => ".cfg do Forge",
            Self::OptionsTxt => "options.txt",
        }
    }
}

impl fmt::Display for ConfigFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn by_name(name: &str) -> Option<ConfigFormat> {
        ConfigFormat::from_file_name(Path::new(name))
    }

    #[test]
    fn reconhece_pelo_nome() {
        assert_eq!(
            by_name("config/create-server.toml"),
            Some(ConfigFormat::Toml)
        );
        assert_eq!(by_name("x.JSON"), Some(ConfigFormat::Json));
        assert_eq!(by_name("x.jsonc"), Some(ConfigFormat::Json));
        assert_eq!(by_name("pack.mcmeta"), Some(ConfigFormat::Json));
        assert_eq!(by_name("x.json5"), Some(ConfigFormat::Json5));
        assert_eq!(by_name("iris.properties"), Some(ConfigFormat::Properties));
        assert_eq!(by_name("forge.cfg"), Some(ConfigFormat::LegacyCfg));
        assert_eq!(by_name("options.txt"), Some(ConfigFormat::OptionsTxt));
        assert_eq!(by_name("optionsof.txt"), Some(ConfigFormat::OptionsTxt));
        assert_eq!(
            by_name("optionsshaders.txt"),
            Some(ConfigFormat::Properties)
        );
        assert_eq!(by_name("notas.txt"), None);
        assert_eq!(by_name("config.yml"), None);
        assert_eq!(by_name("semextensao"), None);
        assert_eq!(by_name(""), None);
    }

    #[test]
    fn cfg_so_e_do_forge_pelo_conteudo() {
        let forge = b"general {\n    B:enabled=true\n}\n";
        let ini = b"[section]\nkey = value\n";
        assert_eq!(
            ConfigFormat::detect(Path::new("a.cfg"), forge),
            Some(ConfigFormat::LegacyCfg)
        );
        assert_eq!(ConfigFormat::detect(Path::new("a.cfg"), ini), None);
        assert_eq!(
            ConfigFormat::detect(Path::new("a.toml"), ini),
            Some(ConfigFormat::Toml)
        );
    }

    #[test]
    fn nomes_para_mensagens() {
        for format in ConfigFormat::ALL {
            assert_eq!(format.to_string(), format.name());
        }
    }
}
