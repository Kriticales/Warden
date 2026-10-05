//! Catálogos embutidos e validados. `OnceLock` evita interpretar TOML a cada análise.

use std::collections::HashSet;
use std::sync::OnceLock;

use serde::Deserialize;

use crate::{DiagnosticsError, Result};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Category {
    pub id: String,
    pub members: Vec<String>,
    pub note: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Conflict {
    pub id: String,
    pub first: String,
    pub second: String,
    pub second_min_version: Option<String>,
    pub severity: String,
    pub note: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Obsolete {
    pub id: String,
    pub mod_id: String,
    pub loader: Option<String>,
    pub min_minecraft: String,
    pub replacement: Option<String>,
    pub note: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Categories {
    category: Vec<Category>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Conflicts {
    conflict: Vec<Conflict>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Obsoletes {
    obsolete: Vec<Obsolete>,
}

pub(super) struct Catalogs {
    pub categories: Vec<Category>,
    pub conflicts: Vec<Conflict>,
    pub obsolete: Vec<Obsolete>,
}

static CATALOGS: OnceLock<std::result::Result<Catalogs, String>> = OnceLock::new();

pub(super) fn get() -> Result<&'static Catalogs> {
    CATALOGS
        .get_or_init(load)
        .as_ref()
        .map_err(|error| DiagnosticsError::Internal(error.clone()))
}

fn load() -> std::result::Result<Catalogs, String> {
    let categories: Categories =
        toml::from_str(include_str!("../../data/exclusive-categories.toml"))
            .map_err(|error| format!("exclusive-categories.toml: {error}"))?;
    let conflicts: Conflicts = toml::from_str(include_str!("../../data/known-conflicts.toml"))
        .map_err(|error| format!("known-conflicts.toml: {error}"))?;
    let obsolete: Obsoletes = toml::from_str(include_str!("../../data/obsolete.toml"))
        .map_err(|error| format!("obsolete.toml: {error}"))?;

    let mut ids = HashSet::new();
    for category in &categories.category {
        validate_id(&category.id, &mut ids)?;
        if category.note.trim().is_empty() || category.members.len() < 2 {
            return Err(format!(
                "categoria {} sem nota ou com menos de dois mods",
                category.id
            ));
        }
        let mut members = HashSet::new();
        if category
            .members
            .iter()
            .any(|id| !valid_mod_id(id) || !members.insert(id))
        {
            return Err(format!(
                "categoria {} com mod inválido ou repetido",
                category.id
            ));
        }
    }
    for conflict in &conflicts.conflict {
        validate_id(&conflict.id, &mut ids)?;
        if !valid_mod_id(&conflict.first)
            || !valid_mod_id(&conflict.second)
            || conflict.first == conflict.second
            || conflict.note.trim().is_empty()
            || !matches!(conflict.severity.as_str(), "error" | "warning")
        {
            return Err(format!("conflito {} inválido", conflict.id));
        }
    }
    for entry in &obsolete.obsolete {
        validate_id(&entry.id, &mut ids)?;
        if !valid_mod_id(&entry.mod_id)
            || entry.note.trim().is_empty()
            || entry.min_minecraft.trim().is_empty()
            || entry
                .loader
                .as_deref()
                .is_some_and(|v| !matches!(v, "forge" | "neoforge" | "fabric"))
            || entry
                .replacement
                .as_deref()
                .is_some_and(|id| !valid_mod_id(id))
        {
            return Err(format!("regra de obsolescência {} inválida", entry.id));
        }
    }
    Ok(Catalogs {
        categories: categories.category,
        conflicts: conflicts.conflict,
        obsolete: obsolete.obsolete,
    })
}

fn valid_mod_id(id: &str) -> bool {
    !id.is_empty()
        && id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'_' | b'-'))
}

fn validate_id(id: &str, ids: &mut HashSet<String>) -> std::result::Result<(), String> {
    if !valid_mod_id(id) || !ids.insert(id.to_owned()) {
        return Err(format!("id curado inválido ou repetido: {id}"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn dados_curados_tem_esquema_valido_e_ids_unicos() {
        let data = super::get().unwrap();
        assert!(data.categories.len() >= 4);
        assert!(data.conflicts.len() >= 5);
        assert!(data.obsolete.len() >= 6);
    }
}
