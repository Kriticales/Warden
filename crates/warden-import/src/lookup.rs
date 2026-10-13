//! Consultas às lojas durante a importação: jars soltos identificados pelo hash (Modrinth pelo
//! sha512; CurseForge pela impressão murmur2, só com a chave), o lado corrigido pelo Modrinth
//! (SHA-1 → `POST /v2/version_files`) e a licença do modpack.
//!
//! Falha de uma loja nunca é erro da importação: o jar fica como arquivo local com o motivo e a
//! loja vai para `unavailable_sources`. Sem a chave da CurseForge, nenhuma requisição vai para
//! a CurseForge.

use std::collections::{HashMap, HashSet};

use warden_core::CancellationToken;
use warden_curseforge::CurseforgeClient;
use warden_modrinth::{HashAlgorithm, ModrinthClient, Version};
use warden_packwiz::hash::{curseforge_fingerprint, hash_bytes};
use warden_packwiz::{CurseForgeFile, HashFormat, Metafile, ModrinthFile, Side};
use warden_project::add::modrinth::{project_side, side_of};

use crate::model::{ItemSource, LocalReason, PackLicense};

/// Os clientes das lojas. `curseforge` só existe com a chave.
#[derive(Clone, Copy)]
pub struct Stores<'a> {
    /// Modrinth (com o cache de metadados do app).
    pub modrinth: Option<&'a ModrinthClient>,
    /// CurseForge, com a chave do usuário.
    pub curseforge: Option<&'a CurseforgeClient>,
}

/// Hashes de um jar solto.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct JarFacts {
    pub(crate) sha1: String,
    pub(crate) sha256: String,
    pub(crate) sha512: String,
    pub(crate) murmur2: u32,
}

impl JarFacts {
    pub(crate) fn of(bytes: &[u8]) -> Self {
        Self {
            sha1: hash_bytes(HashFormat::Sha1, bytes),
            sha256: hash_bytes(HashFormat::Sha256, bytes),
            sha512: hash_bytes(HashFormat::Sha512, bytes),
            murmur2: curseforge_fingerprint(bytes),
        }
    }
}

/// Um jar reconhecido numa loja.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Found {
    pub(crate) source: ItemSource,
    pub(crate) slug: String,
    pub(crate) metafile: Metafile,
}

/// O resultado da procura de vários jars, na ordem pedida.
#[derive(Debug, Default)]
pub(crate) struct Identified {
    pub(crate) found: Vec<Result<Found, LocalReason>>,
    pub(crate) unavailable: Vec<String>,
}

fn side_of_version(version: &Version, project: Option<&warden_modrinth::Project>) -> Side {
    if version.environment.is_some() {
        return side_of(version.environment).0.side();
    }
    project
        .and_then(project_side)
        .map_or(Side::Both, |(choice, _)| choice.side())
}

async fn projects_by_id(
    client: &ModrinthClient,
    ids: Vec<String>,
    cancel: &CancellationToken,
) -> Result<HashMap<String, warden_modrinth::Project>, warden_modrinth::Error> {
    let mut ids = ids;
    ids.sort();
    ids.dedup();
    if ids.is_empty() {
        return Ok(HashMap::new());
    }
    Ok(client
        .projects(&ids, Some(cancel))
        .await?
        .into_iter()
        .map(|project| (project.id.clone(), project))
        .collect())
}

async fn modrinth_found(
    client: &ModrinthClient,
    jars: &[JarFacts],
    cancel: &CancellationToken,
) -> Result<Vec<Option<Found>>, warden_modrinth::Error> {
    let hashes: Vec<String> = jars.iter().map(|jar| jar.sha512.clone()).collect();
    let versions = client
        .version_files(&hashes, HashAlgorithm::Sha512, Some(cancel))
        .await?;
    let projects = projects_by_id(
        client,
        versions.values().map(|v| v.project_id.clone()).collect(),
        cancel,
    )
    .await?;
    Ok(jars
        .iter()
        .map(|jar| {
            let version = versions.get(&jar.sha512)?;
            let file = version
                .files
                .iter()
                .find(|file| file.hashes.sha512.eq_ignore_ascii_case(&jar.sha512))?;
            let project = projects.get(&version.project_id);
            let title = project.map_or_else(
                || file.filename.trim_end_matches(".jar").to_owned(),
                |project| project.title.clone(),
            );
            let slug = project.map_or_else(|| title.clone(), |project| project.slug.clone());
            let metafile = Metafile::modrinth(
                &ModrinthFile {
                    title,
                    project_id: version.project_id.clone(),
                    version_id: version.id.clone(),
                    filename: file.filename.clone(),
                    url: file.url.clone(),
                    hash_format: HashFormat::Sha512,
                    hash: jar.sha512.clone(),
                },
                side_of_version(version, project),
            );
            Some(Found {
                source: ItemSource::Modrinth,
                slug,
                metafile,
            })
        })
        .collect())
}

async fn curseforge_found(
    client: &CurseforgeClient,
    jars: &[(usize, &JarFacts)],
    cancel: &CancellationToken,
) -> Result<HashMap<usize, Found>, warden_curseforge::Error> {
    let prints: Vec<u32> = jars.iter().map(|(_, jar)| jar.murmur2).collect();
    let matches = client.fingerprints(&prints, Some(cancel)).await?;
    if matches.matches.is_empty() {
        return Ok(HashMap::new());
    }
    let ids: Vec<u64> = matches
        .matches
        .values()
        .map(|found| found.id)
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    let projects: HashMap<u64, warden_curseforge::Mod> = client
        .projects(&ids, Some(cancel))
        .await?
        .into_iter()
        .map(|project| (project.id, project))
        .collect();
    let mut result = HashMap::new();
    for (position, jar) in jars {
        let Some(found) = matches.matches.get(&jar.murmur2) else {
            continue;
        };
        let (Ok(project_id), Ok(file_id)) = (u32::try_from(found.id), u32::try_from(found.file.id))
        else {
            continue;
        };
        let project = projects.get(&found.id);
        let name = project.map_or_else(
            || found.file.display_name.clone(),
            |project| project.name.clone(),
        );
        let slug = project.map_or_else(|| name.clone(), |project| project.slug.clone());
        let metafile = Metafile::curseforge(
            &CurseForgeFile {
                name,
                project_id,
                file_id,
                filename: found.file.file_name.clone(),
                hash_format: HashFormat::Sha1,
                hash: jar.sha1.clone(),
            },
            Side::Both,
        );
        result.insert(
            *position,
            Found {
                source: ItemSource::Curseforge,
                slug,
                metafile,
            },
        );
    }
    Ok(result)
}

/// Procura os jars no Modrinth e, para os que faltarem, na CurseForge.
pub(crate) async fn identify(
    stores: Stores<'_>,
    jars: &[JarFacts],
    cancel: &CancellationToken,
) -> Identified {
    let mut result = Identified {
        found: Vec::with_capacity(jars.len()),
        unavailable: Vec::new(),
    };
    if jars.is_empty() {
        return result;
    }
    let mut modrinth_failed = false;
    let mut found: Vec<Option<Found>> = vec![None; jars.len()];
    match stores.modrinth {
        Some(client) => match modrinth_found(client, jars, cancel).await {
            Ok(list) => found = list,
            Err(error) => {
                tracing::warn!(%error, "procura pelo hash no Modrinth falhou na importação");
                modrinth_failed = true;
                result.unavailable.push("Modrinth".to_owned());
            }
        },
        None => modrinth_failed = true,
    }
    let mut curseforge_failed = false;
    if let Some(client) = stores.curseforge {
        let missing: Vec<(usize, &JarFacts)> = jars
            .iter()
            .enumerate()
            .filter(|(position, _)| found[*position].is_none())
            .collect();
        if !missing.is_empty() {
            match curseforge_found(client, &missing, cancel).await {
                Ok(map) => {
                    for (position, item) in map {
                        found[position] = Some(item);
                    }
                }
                Err(error) => {
                    tracing::warn!(%error, "procura pela impressão na CurseForge falhou na importação");
                    curseforge_failed = true;
                    result.unavailable.push("CurseForge".to_owned());
                }
            }
        }
    }
    result.found = found
        .into_iter()
        .map(|item| {
            item.ok_or(if modrinth_failed || curseforge_failed {
                LocalReason::LookupFailed
            } else if stores.curseforge.is_none() {
                LocalReason::NoCurseforgeKey
            } else {
                LocalReason::NotFound
            })
        })
        .collect();
    result
}

/// O lado de cada arquivo (pelo SHA-1) segundo o Modrinth: o `environment` da versão ou, sem
/// ele, o do projeto. Arquivos que o Modrinth não conhece ficam de fora. Falha da consulta
/// devolve `None` (o lado fica como veio).
pub(crate) async fn modrinth_sides(
    client: Option<&ModrinthClient>,
    sha1s: &[String],
    cancel: &CancellationToken,
) -> Option<HashMap<String, Side>> {
    let client = client?;
    if sha1s.is_empty() {
        return Some(HashMap::new());
    }
    let run = async {
        let versions = client
            .version_files(sha1s, HashAlgorithm::Sha1, Some(cancel))
            .await?;
        let projects = projects_by_id(
            client,
            versions.values().map(|v| v.project_id.clone()).collect(),
            cancel,
        )
        .await?;
        Ok::<_, warden_modrinth::Error>(
            versions
                .iter()
                .map(|(sha1, version)| {
                    (
                        sha1.to_ascii_lowercase(),
                        side_of_version(version, projects.get(&version.project_id)),
                    )
                })
                .collect(),
        )
    };
    match run.await {
        Ok(map) => Some(map),
        Err(error) => {
            tracing::warn!(%error, "correção do lado pelo Modrinth falhou na importação");
            None
        }
    }
}

/// Licenças livres conhecidas (prefixos SPDX).
const FREE_LICENSES: [&str; 21] = [
    "MIT",
    "Apache-",
    "GPL-",
    "LGPL-",
    "AGPL-",
    "MPL-",
    "BSD-",
    "ISC",
    "Zlib",
    "Unlicense",
    "CC0-",
    "CC-BY-4",
    "CC-BY-3",
    "CC-BY-SA-",
    "EPL-",
    "EUPL-",
    "OSL-",
    "0BSD",
    "BSL-",
    "WTFPL",
    "Artistic-",
];

/// Uma licença do Modrinth no formato da tela.
#[must_use]
pub(crate) fn license_of(license: &warden_modrinth::License) -> Option<PackLicense> {
    let id = license.id.trim();
    if id.is_empty() {
        return None;
    }
    let free = !id.starts_with("LicenseRef-")
        && !id.eq_ignore_ascii_case("ARR")
        && !id.starts_with("PolyForm-")
        && FREE_LICENSES.iter().any(|prefix| id.starts_with(prefix));
    let name = if license.name.trim().is_empty() {
        if id == "LicenseRef-All-Rights-Reserved" || id.eq_ignore_ascii_case("ARR") {
            "All Rights Reserved".to_owned()
        } else {
            id.to_owned()
        }
    } else {
        license.name.trim().to_owned()
    };
    Some(PackLicense {
        id: id.to_owned(),
        name,
        free,
    })
}

/// A licença do modpack publicado no Modrinth, achado pelo sha512 do próprio arquivo.
pub(crate) async fn modpack_license(
    client: Option<&ModrinthClient>,
    sha512: &str,
    cancel: &CancellationToken,
) -> Option<PackLicense> {
    let client = client?;
    let versions = client
        .version_files(&[sha512.to_owned()], HashAlgorithm::Sha512, Some(cancel))
        .await
        .ok()?;
    let version = versions.get(&sha512.to_ascii_lowercase())?;
    let project = client
        .projects(std::slice::from_ref(&version.project_id), Some(cancel))
        .await
        .ok()?
        .into_iter()
        .next()?;
    project.license.as_ref().and_then(license_of)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    fn license(id: &str, name: &str) -> warden_modrinth::License {
        warden_modrinth::License {
            id: id.to_owned(),
            name: name.to_owned(),
            url: None,
        }
    }

    #[test]
    fn licencas_livres_e_reservadas() {
        let lgpl = license_of(&license("LGPL-3.0-only", "")).unwrap();
        assert!(lgpl.free);
        assert_eq!(lgpl.name, "LGPL-3.0-only");
        let arr = license_of(&license("LicenseRef-All-Rights-Reserved", "")).unwrap();
        assert!(!arr.free);
        assert_eq!(arr.name, "All Rights Reserved");
        let custom = license_of(&license("LicenseRef-Custom", "Minha licença")).unwrap();
        assert!(!custom.free);
        assert_eq!(custom.name, "Minha licença");
        assert!(!license_of(&license("PolyForm-Shield-1.0.0", "")).unwrap().free);
        assert!(license_of(&license("", "")).is_none());
    }
}
