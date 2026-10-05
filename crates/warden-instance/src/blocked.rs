//! Mods da CurseForge com download bloqueado para apps de terceiros (ARCHITECTURE §8.2 item 9;
//! SPEC T20).
//!
//! A materialização não para por causa deles: instala o resto e devolve a lista
//! ([`BlockedFile`]) no relatório. A tela T20 mostra a lista; a pessoa baixa cada arquivo pelo
//! site e escolhe o arquivo baixado, que [`accept_manual_file`] confere pelo hash e guarda no
//! cache de downloads (nunca na pasta do pack). Na próxima materialização o arquivo é achado
//! no cache pelo hash e instalado como qualquer outro.

use std::path::Path;

use serde::{Deserialize, Serialize};
use warden_http::ExpectedHash;
use warden_packwiz::HashFormat;

use crate::downloads::{CachedFile, DownloadCache, FileOrigin};
use crate::error::{Error, Result};

/// Um arquivo que precisa ser baixado à mão.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct BlockedFile {
    /// Caminho do metafile no pack (`mods/entityculling.pw.toml`).
    pub metafile: String,
    /// Nome do item.
    pub name: String,
    /// Nome do arquivo esperado (`filename` do metafile).
    pub file_name: String,
    /// ID do projeto na CurseForge.
    #[specta(type = specta_typescript::Number)]
    pub project_id: u64,
    /// ID do arquivo na CurseForge.
    #[specta(type = specta_typescript::Number)]
    pub file_id: u64,
    /// Página do arquivo no site, para o download manual (só em memória).
    pub page_url: Option<String>,
    /// Formato do hash esperado (`sha1`, `murmur2`…).
    pub hash_format: String,
    /// Hash esperado.
    pub hash: String,
}

impl BlockedFile {
    /// O hash esperado, no tipo da `warden-http`.
    pub fn expected(&self) -> Result<ExpectedHash> {
        let format = HashFormat::parse(&self.hash_format).map_err(Error::Pack)?;
        Ok(ExpectedHash::new(format, self.hash.clone()))
    }
}

/// Confere o arquivo escolhido pela pessoa para um item bloqueado e, se for o certo, guarda no
/// cache de downloads. Hash diferente: [`Error::ManualFileMismatch`] ("Este não é o arquivo
/// certo."), sem copiar nada.
pub fn accept_manual_file(
    cache: &DownloadCache,
    blocked: &BlockedFile,
    file: &Path,
) -> Result<CachedFile> {
    let expected = blocked.expected()?;
    let origin = FileOrigin::manual_curseforge(blocked.project_id, blocked.file_id);
    cache
        .import_file(file, &expected, &origin)?
        .ok_or_else(|| Error::ManualFileMismatch {
            expected_file: blocked.file_name.clone(),
            format: blocked.hash_format.clone(),
        })
}

#[cfg(test)]
mod tests {
    use std::fs;

    use warden_core::DomainError as _;
    use warden_packwiz::hash::hash_bytes;

    use super::*;
    use crate::downloads::OriginSource;

    fn blocked(data: &[u8]) -> BlockedFile {
        BlockedFile {
            metafile: "mods/entityculling.pw.toml".to_owned(),
            name: "Entity Culling".to_owned(),
            file_name: "entityculling.jar".to_owned(),
            project_id: 448_233,
            file_id: 1,
            page_url: None,
            hash_format: "sha1".to_owned(),
            hash: hash_bytes(HashFormat::Sha1, data),
        }
    }

    #[test]
    fn ca_t20_01_hash_errado_recusado_certo_aceito() {
        let dir = crate::test_support::temp_dir();
        let cache = DownloadCache::open(&dir.path().join("downloads")).unwrap();
        let item = blocked(b"jar certo");
        let wrong = dir.path().join("errado.jar");
        fs::write(&wrong, b"jar errado").unwrap();
        let error = accept_manual_file(&cache, &item, &wrong).unwrap_err();
        assert!(
            matches!(error, Error::ManualFileMismatch { .. }),
            "{error:?}"
        );
        assert_eq!(error.params()["file"], "entityculling.jar");

        let right = dir.path().join("certo.jar");
        fs::write(&right, b"jar certo").unwrap();
        let stored = accept_manual_file(&cache, &item, &right).unwrap();
        assert_eq!(fs::read(&stored.path).unwrap(), b"jar certo");
        assert!(stored.path.starts_with(cache.root()));
        let found = cache.find(&item.expected().unwrap()).unwrap().unwrap();
        assert_eq!(found, stored);
        let origins = cache.origins(&stored.hashes.sha256).unwrap();
        assert_eq!(origins[0].source, OriginSource::Manual);
        assert_eq!(origins[0].url, None);
        // O arquivo escolhido continua onde estava.
        assert!(right.exists());
    }

    #[test]
    fn formato_desconhecido_e_erro() {
        let mut item = blocked(b"x");
        item.hash_format = "crc32".to_owned();
        assert!(matches!(item.expected(), Err(Error::Pack(_))));
    }
}
