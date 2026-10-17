//! Rótulos de config tirados dos arquivos de idioma do jar (camada 4 do formulário; R5B §2.2;
//! ARCHITECTURE §10.1).
//!
//! Vários mods trazem, em `assets/<namespace>/lang/<idioma>.json`, o nome bonito e a ajuda de
//! cada chave de config, com chaves de tradução em padrões conhecidos:
//!
//! - NeoForge (`ConfigurationScreen`): `<modid>.configuration.<chave>` e `….tooltip`;
//! - `AutoConfig` (Cloth Config): `text.autoconfig.<nome>.option.<campo>` e `….@Tooltip`;
//! - owo-config: `text.config.<nome>.option.<campo>` e `….tooltip`;
//! - `MidnightLib`: `<modid>.midnightconfig.<campo>` e `….tooltip`.
//!
//! [`read_config_lang_file`] lê só os arquivos `pt_br.json` e `en_us.json` (sem extrair nada,
//! com o limite de [`Limits::max_descriptor_bytes`] contado nos bytes descompactados) e guarda
//! só as chaves desses padrões ([`is_config_key`]); o resto do arquivo de idioma (nomes de
//! itens, mensagens) não interessa ao formulário e ficaria na memória à toa. Quem procura um
//! rótulo recebe o texto em português quando o mod traduziu, senão o inglês
//! ([`ConfigLang::get`]).
//!
//! Limites: só o jar de cima (o mod que a pessoa adicionou); os `.lang` do Minecraft 1.12 e
//! anteriores não entram, porque as configs dessa época não usam chaves desses padrões.

use std::collections::BTreeMap;
use std::fs::File;
use std::io::{BufReader, Read, Seek};
use std::path::Path;

use serde::{Deserialize, Serialize};
use zip::ZipArchive;

use crate::error::{Error, LimitKind};
use crate::lenient_json;
use crate::limits::Limits;
use crate::text;

/// Idioma de um rótulo.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, specta::Type,
)]
#[serde(rename_all = "camelCase")]
pub enum LangLocale {
    /// `pt_br.json`.
    PtBr,
    /// `en_us.json`.
    EnUs,
}

impl LangLocale {
    /// Nome do arquivo de idioma sem a extensão (`pt_br`).
    #[must_use]
    pub fn code(self) -> &'static str {
        match self {
            Self::PtBr => "pt_br",
            Self::EnUs => "en_us",
        }
    }

    fn from_file_name(name: &str) -> Option<Self> {
        match name.to_ascii_lowercase().as_str() {
            "pt_br.json" => Some(Self::PtBr),
            "en_us.json" => Some(Self::EnUs),
            _ => None,
        }
    }
}

/// Um rótulo encontrado: o texto e o idioma de onde veio.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LangText<'a> {
    /// O texto como o mod escreveu (pode ter códigos de formatação `§`).
    pub text: &'a str,
    /// O idioma.
    pub locale: LangLocale,
}

/// As traduções de uma chave nos dois idiomas lidos.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Translations {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pt_br: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    en_us: Option<String>,
}

/// Rótulos de config de um ou mais jars, por chave de tradução.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfigLang {
    entries: BTreeMap<String, Translations>,
}

impl ConfigLang {
    /// O rótulo da chave: o português quando existe, senão o inglês.
    #[must_use]
    pub fn get(&self, key: &str) -> Option<LangText<'_>> {
        let found = self.entries.get(key)?;
        if let Some(text) = found.pt_br.as_deref() {
            return Some(LangText {
                text,
                locale: LangLocale::PtBr,
            });
        }
        found.en_us.as_deref().map(|text| LangText {
            text,
            locale: LangLocale::EnUs,
        })
    }

    /// Quantas chaves têm pelo menos um rótulo.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// `true` sem nenhum rótulo.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Junta os rótulos de outro jar. Numa chave que os dois têm, o primeiro a chegar fica (dois
    /// mods com a mesma chave de tradução é engano de um deles; o jogo também usa um só).
    pub fn merge(&mut self, other: Self) {
        for (key, incoming) in other.entries {
            let current = self.entries.entry(key).or_default();
            if current.pt_br.is_none() {
                current.pt_br = incoming.pt_br;
            }
            if current.en_us.is_none() {
                current.en_us = incoming.en_us;
            }
        }
    }

    fn insert(&mut self, key: String, text: String, locale: LangLocale) {
        let current = self.entries.entry(key).or_default();
        let slot = match locale {
            LangLocale::PtBr => &mut current.pt_br,
            LangLocale::EnUs => &mut current.en_us,
        };
        if slot.is_none() {
            *slot = Some(text);
        }
    }
}

/// `true` se a chave de tradução segue um dos padrões de rótulo de config conhecidos.
#[must_use]
pub fn is_config_key(key: &str) -> bool {
    key.starts_with("text.autoconfig.")
        || key.starts_with("text.config.")
        || key.contains(".configuration.")
        || key.contains(".midnightconfig.")
}

/// `assets/<namespace>/lang/<arquivo>`: o idioma, se for um dos lidos.
fn lang_file_locale(name: &str) -> Option<LangLocale> {
    let mut parts = name.split('/');
    let (Some(assets), Some(namespace), Some(lang), Some(file), None) = (
        parts.next(),
        parts.next(),
        parts.next(),
        parts.next(),
        parts.next(),
    ) else {
        return None;
    };
    (assets == "assets" && !namespace.is_empty() && lang == "lang")
        .then(|| LangLocale::from_file_name(file))
        .flatten()
}

/// Lê os rótulos de config de um jar no disco.
///
/// # Errors
///
/// [`Error::Read`] se o arquivo não abre, [`Error::LimitExceeded`] se passa do tamanho ou do
/// número de entradas e [`Error::InvalidArchive`] se não é um zip. Um arquivo de idioma
/// ilegível, grande demais ou que não é JSON é pulado em silêncio: falta de rótulo nunca impede
/// o formulário.
pub fn read_config_lang_file(path: &Path, limits: &Limits) -> Result<ConfigLang, Error> {
    let read_error = |source| Error::Read {
        path: path.to_owned(),
        source,
    };
    let file = File::open(path).map_err(read_error)?;
    let size = file.metadata().map_err(read_error)?.len();
    if size > limits.max_file_bytes {
        return Err(Error::LimitExceeded {
            kind: LimitKind::FileSize,
            limit: limits.max_file_bytes,
            actual: size,
        });
    }
    read_config_lang(BufReader::new(file), limits)
}

/// Lê os rótulos de config de um jar já na memória. Erros como em [`read_config_lang_file`].
pub fn read_config_lang_bytes(bytes: &[u8], limits: &Limits) -> Result<ConfigLang, Error> {
    let size = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
    if size > limits.max_file_bytes {
        return Err(Error::LimitExceeded {
            kind: LimitKind::FileSize,
            limit: limits.max_file_bytes,
            actual: size,
        });
    }
    read_config_lang(std::io::Cursor::new(bytes), limits)
}

fn read_config_lang<R: Read + Seek>(reader: R, limits: &Limits) -> Result<ConfigLang, Error> {
    let mut zip = ZipArchive::new(reader).map_err(|error| Error::InvalidArchive {
        detail: error.to_string(),
    })?;
    if zip.len() > limits.max_entries {
        return Err(Error::LimitExceeded {
            kind: LimitKind::EntryCount,
            limit: u64::try_from(limits.max_entries).unwrap_or(u64::MAX),
            actual: u64::try_from(zip.len()).unwrap_or(u64::MAX),
        });
    }
    let files: Vec<(usize, LangLocale)> = (0..zip.len())
        .filter_map(|index| {
            let name = zip.name_for_index(index)?.ok()?;
            lang_file_locale(&name).map(|locale| (index, locale))
        })
        .collect();
    let mut lang = ConfigLang::default();
    for (index, locale) in files {
        let Some(bytes) = read_capped(&mut zip, index, limits.max_descriptor_bytes) else {
            continue;
        };
        let decoded = text::decode(&bytes);
        let Ok(parsed) = lenient_json::parse(&decoded.text) else {
            continue;
        };
        let serde_json::Value::Object(map) = parsed.value else {
            continue;
        };
        for (key, value) in map {
            if let serde_json::Value::String(text) = value
                && is_config_key(&key)
            {
                lang.insert(key, text, locale);
            }
        }
    }
    Ok(lang)
}

/// Bytes de uma entrada, ou `None` se ilegível ou acima de `max` descompactado.
fn read_capped<R: Read + Seek>(zip: &mut ZipArchive<R>, index: usize, max: u64) -> Option<Vec<u8>> {
    let file = zip.by_index(index).ok()?;
    let mut bytes = Vec::new();
    file.take(max.saturating_add(1))
        .read_to_end(&mut bytes)
        .ok()?;
    (u64::try_from(bytes.len()).unwrap_or(u64::MAX) <= max).then_some(bytes)
}

#[cfg(test)]
mod tests {
    use std::io::Write as _;

    use zip::write::SimpleFileOptions;

    use super::*;

    fn jar(files: &[(&str, &[u8])]) -> Vec<u8> {
        let mut out = std::io::Cursor::new(Vec::new());
        let mut writer = zip::ZipWriter::new(&mut out);
        for (name, bytes) in files {
            writer
                .start_file(*name, SimpleFileOptions::default())
                .unwrap();
            writer.write_all(bytes).unwrap();
        }
        writer.finish().unwrap();
        out.into_inner()
    }

    #[test]
    fn reconhece_os_padroes_de_chave() {
        for key in [
            "create.configuration.maxRotationSpeed",
            "text.autoconfig.inmis.option.enableTrinkets",
            "text.config.owo-test.option.number",
            "visualworkbench.midnightconfig.enabled",
        ] {
            assert!(is_config_key(key), "{key}");
        }
        for key in ["item.create.wrench", "configuration", "text.autoconfig"] {
            assert!(!is_config_key(key), "{key}");
        }
        assert_eq!(
            lang_file_locale("assets/inmis/lang/pt_BR.json"),
            Some(LangLocale::PtBr)
        );
        assert_eq!(lang_file_locale("assets/inmis/lang/de_de.json"), None);
        assert_eq!(lang_file_locale("assets/lang/en_us.json"), None);
        assert_eq!(lang_file_locale("assets//lang/en_us.json"), None);
        assert_eq!(lang_file_locale("data/x/lang/en_us.json"), None);
        assert_eq!(lang_file_locale("assets/x/lang/a/en_us.json"), None);
        assert_eq!(LangLocale::PtBr.code(), "pt_br");
        assert_eq!(LangLocale::EnUs.code(), "en_us");
    }

    #[test]
    fn le_so_as_chaves_de_config_e_prefere_portugues() {
        let bytes = jar(&[
            (
                "assets/inmis/lang/en_us.json",
                br#"{"text.autoconfig.inmis.option.enableTrinkets": "Enable Trinkets",
                     "text.autoconfig.inmis.option.size": "Size",
                     "item.inmis.backpack": "Backpack", "x.configuration.y": 3}"#,
            ),
            (
                "assets/inmis/lang/pt_br.json",
                "\u{feff}{\"text.autoconfig.inmis.option.enableTrinkets\": \"Ativar berloques\",}"
                    .as_bytes(),
            ),
            (
                "assets/inmis/lang/de_de.json",
                br#"{"text.config.a.option.b": "B"}"#,
            ),
            ("assets/outro/lang/en_us.json", b"nao e json"),
        ]);
        let lang = read_config_lang_bytes(&bytes, &Limits::default()).unwrap();
        assert_eq!(lang.len(), 2);
        assert!(!lang.is_empty());
        assert_eq!(
            lang.get("text.autoconfig.inmis.option.enableTrinkets"),
            Some(LangText {
                text: "Ativar berloques",
                locale: LangLocale::PtBr
            })
        );
        assert_eq!(
            lang.get("text.autoconfig.inmis.option.size"),
            Some(LangText {
                text: "Size",
                locale: LangLocale::EnUs
            })
        );
        assert_eq!(lang.get("item.inmis.backpack"), None);
        assert_eq!(lang.get("text.config.a.option.b"), None);
    }

    #[test]
    fn juntar_mantem_o_primeiro_e_completa_o_que_falta() {
        let first = read_config_lang_bytes(
            &jar(&[(
                "assets/a/lang/en_us.json",
                br#"{"a.configuration.x": "First"}"#,
            )]),
            &Limits::default(),
        )
        .unwrap();
        let second = read_config_lang_bytes(
            &jar(&[
                (
                    "assets/b/lang/en_us.json",
                    br#"{"a.configuration.x": "Second"}"#,
                ),
                (
                    "assets/b/lang/pt_br.json",
                    br#"{"a.configuration.x": "Segundo"}"#,
                ),
            ]),
            &Limits::default(),
        )
        .unwrap();
        let mut merged = first;
        merged.merge(second);
        assert_eq!(merged.get("a.configuration.x").unwrap().text, "Segundo");
        assert_eq!(
            merged.entries["a.configuration.x"].en_us.as_deref(),
            Some("First")
        );
    }

    #[test]
    fn limites_e_arquivos_invalidos() {
        let big = format!("{{\"a.configuration.x\": \"{}\"}}", "x".repeat(2048));
        let bytes = jar(&[("assets/a/lang/en_us.json", big.as_bytes())]);
        let small = Limits {
            max_descriptor_bytes: 1024,
            ..Limits::default()
        };
        assert!(read_config_lang_bytes(&bytes, &small).unwrap().is_empty());
        let few = Limits {
            max_entries: 0,
            ..Limits::default()
        };
        assert!(matches!(
            read_config_lang_bytes(&bytes, &few),
            Err(Error::LimitExceeded {
                kind: LimitKind::EntryCount,
                ..
            })
        ));
        let tiny = Limits {
            max_file_bytes: 4,
            ..Limits::default()
        };
        assert!(matches!(
            read_config_lang_bytes(&bytes, &tiny),
            Err(Error::LimitExceeded {
                kind: LimitKind::FileSize,
                ..
            })
        ));
        assert!(matches!(
            read_config_lang_bytes(b"nada", &Limits::default()),
            Err(Error::InvalidArchive { .. })
        ));
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("mod.jar");
        std::fs::write(&path, &bytes).unwrap();
        assert_eq!(
            read_config_lang_file(&path, &Limits::default())
                .unwrap()
                .len(),
            1
        );
        assert!(matches!(
            read_config_lang_file(&path, &tiny),
            Err(Error::LimitExceeded { .. })
        ));
        assert!(matches!(
            read_config_lang_file(&dir.path().join("falta.jar"), &Limits::default()),
            Err(Error::Read { .. })
        ));
    }
}
