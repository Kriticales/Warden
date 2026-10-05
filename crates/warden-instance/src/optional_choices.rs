//! Escolhas dos mods opcionais na instância de teste:
//! `instances/<pack-id>/state/optional-choices.json` (ARCHITECTURE §8.1; SPEC T11).
//!
//! Dado de máquina, fora do pack: ligar um opcional no teste não conta como alteração do pack.
//! Um item sem escolha gravada usa o `default` da tabela `[option]` do `.pw.toml` ("ligado por
//! padrão"). A chave é o caminho do metafile no pack (`mods/sodium.pw.toml`), estável enquanto o
//! item existir.

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::Path;

use serde::{Deserialize, Serialize};
use warden_core::atomic_write;
use warden_packwiz::ModOption;

use crate::error::{Error, Result};

/// Nome do arquivo na pasta `state/`.
pub const OPTIONAL_CHOICES_FILE: &str = "optional-choices.json";

/// Escolhas por metafile (`true` = ligado).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct OptionalChoices {
    /// Caminho do metafile no pack → ligado.
    pub choices: BTreeMap<String, bool>,
}

impl OptionalChoices {
    /// Lê de `state_dir`. Sem arquivo ou ilegível: sem escolhas (vale o padrão de cada item).
    pub fn load(state_dir: &Path) -> Result<Self> {
        let path = state_dir.join(OPTIONAL_CHOICES_FILE);
        match fs::read(&path) {
            Ok(bytes) => Ok(serde_json::from_slice(&bytes).unwrap_or_else(|error| {
                tracing::warn!(%error, caminho = %path.display(), "escolhas de opcionais ilegíveis; usando os padrões");
                Self::default()
            })),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Self::default()),
            Err(error) => Err(Error::io("ler", path, error)),
        }
    }

    /// Grava em `state_dir` (escrita atômica).
    pub fn save(&self, state_dir: &Path) -> Result<()> {
        fs::create_dir_all(state_dir).map_err(|e| Error::io("criar a pasta", state_dir, e))?;
        let mut bytes = serde_json::to_vec_pretty(self)
            .map_err(|error| Error::Internal(format!("opcionais em JSON: {error}")))?;
        bytes.push(b'\n');
        atomic_write(&state_dir.join(OPTIONAL_CHOICES_FILE), &bytes)?;
        Ok(())
    }

    /// Liga ou desliga um opcional.
    pub fn set(&mut self, metafile: &str, enabled: bool) {
        self.choices.insert(metafile.to_owned(), enabled);
    }

    /// Se o item entra na instância: não opcional sempre entra; opcional segue a escolha
    /// gravada ou, sem ela, o `default`.
    #[must_use]
    pub fn is_enabled(&self, metafile: &str, option: Option<&ModOption>) -> bool {
        match option {
            Some(option) if option.optional => self
                .choices
                .get(metafile)
                .copied()
                .unwrap_or(option.default),
            _ => true,
        }
    }

    /// Todos os opcionais ligados (o que o packwiz-installer faz sem janela, `-g`; usado pelo
    /// teste de conformidade).
    #[must_use]
    pub fn all_enabled<'a>(metafiles: impl IntoIterator<Item = &'a str>) -> Self {
        Self {
            choices: metafiles
                .into_iter()
                .map(|path| (path.to_owned(), true))
                .collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn option(default: bool) -> ModOption {
        ModOption {
            optional: true,
            description: String::new(),
            default,
        }
    }

    #[test]
    fn escolha_gravada_vence_o_padrao() {
        let mut choices = OptionalChoices::default();
        assert!(choices.is_enabled("mods/a.pw.toml", None));
        assert!(!choices.is_enabled("mods/a.pw.toml", Some(&option(false))));
        assert!(choices.is_enabled("mods/a.pw.toml", Some(&option(true))));
        choices.set("mods/a.pw.toml", true);
        assert!(choices.is_enabled("mods/a.pw.toml", Some(&option(false))));
        choices.set("mods/a.pw.toml", false);
        assert!(!choices.is_enabled("mods/a.pw.toml", Some(&option(true))));
        let not_optional = ModOption {
            optional: false,
            description: String::new(),
            default: false,
        };
        assert!(choices.is_enabled("mods/a.pw.toml", Some(&not_optional)));
    }

    #[test]
    fn grava_e_le() {
        let dir = crate::test_support::temp_dir();
        assert_eq!(
            OptionalChoices::load(dir.path()).unwrap(),
            OptionalChoices::default()
        );
        let choices = OptionalChoices::all_enabled(["mods/a.pw.toml", "mods/b.pw.toml"]);
        choices.save(dir.path()).unwrap();
        assert_eq!(OptionalChoices::load(dir.path()).unwrap(), choices);
        fs::write(dir.path().join(OPTIONAL_CHOICES_FILE), b"[").unwrap();
        assert_eq!(
            OptionalChoices::load(dir.path()).unwrap(),
            OptionalChoices::default()
        );
    }
}
