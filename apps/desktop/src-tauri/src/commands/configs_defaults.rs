//! "Restaurar padrão" das configs (SPEC T12, parte P1; ARCHITECTURE §10.1; C-06).
//!
//! - `config_restore_default`: troca o valor das chaves pedidas (ou de todas as alteradas do
//!   arquivo) pelo padrão que o próprio arquivo informa (`Default:` do NeoForge,
//!   `[default: …]` do Forge antigo) e devolve o texto novo. Nada é gravado: a interface
//!   mostra a diferença e grava pelo `config_write` de sempre (alteração externa, desfazer).
#![allow(clippy::needless_pass_by_value)] // argumentos pertencem ao contrato do Tauri

use warden_configs::defaults::{RestoredDefaults, restore_defaults};

use crate::error::AppError;

/// O texto com o padrão nas chaves `keys` (caminho como o formulário mostra) ou, sem `keys`,
/// em todas as chaves diferentes do padrão. Erros: `configs.KEY_NOT_EDITABLE` para chave sem
/// padrão conhecido ou já no padrão; os da edição pelo formulário.
#[tauri::command]
#[specta::specta]
pub(crate) fn config_restore_default(
    path: String,
    text: String,
    keys: Option<Vec<String>>,
) -> Result<RestoredDefaults, AppError> {
    restore_defaults(&path, &text, keys.as_deref()).map_err(|error| AppError::from_domain(&error))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn restaura_uma_chave_e_recusa_a_sem_padrao() {
        let text = "# Default: 64\n# Range: 1 ~ 512\nmaxChunks = 900\nfree = 3\n";
        let restored = config_restore_default(
            "config/x-server.toml".into(),
            text.into(),
            Some(vec!["maxChunks".into()]),
        )
        .unwrap();
        assert_eq!(restored.keys, ["maxChunks"]);
        assert!(restored.text.contains("maxChunks = 64\n"));
        let error = config_restore_default(
            "config/x-server.toml".into(),
            text.into(),
            Some(vec!["free".into()]),
        )
        .unwrap_err();
        assert_eq!(error.code.domain(), "configs");
        let all = config_restore_default("config/x-server.toml".into(), text.into(), None).unwrap();
        assert_eq!(all.text, restored.text);
    }
}
