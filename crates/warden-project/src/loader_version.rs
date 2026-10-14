//! Trocar a versão do loader do pack, sem trocar de loader (SPEC T11, "Editar informações";
//! P1-15).
//!
//! - [`set_loader_version`] muda **só** a linha do loader em `[versions]` do `pack.toml` (edição
//!   mínima da `warden-packwiz`); o resto do arquivo fica byte a byte igual. O `pack.toml` não
//!   entra no índice, então não há `packwiz refresh`.
//! - [`LoaderCheck`] é o que a interface mostra **antes** de confirmar: cada item que fica
//!   incompatível com a versão nova, os que já eram e os que voltam a funcionar. Quem compara as
//!   faixas de versão é o diagnóstico (a regra `E_LOADERVERSION` da D-01, com os jars da
//!   instância de teste), orquestrado pela `warden-app`; aqui fica a classificação, que só
//!   depende das duas listas.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use serde::Serialize;
use warden_packwiz::edit::set_pack_version;
use warden_packwiz::{Loader, PACK_FILE, PackManifest};

use crate::transaction::PackTransaction;
use crate::{Error, ProjectErrorCode as Code, Result};

/// Tamanho máximo de uma versão de loader.
const MAX_VERSION_CHARS: usize = 64;

/// A versão do loader gravada.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct LoaderVersionChanged {
    /// Chave do loader (`fabric`, `forge`, `neoforge`, `quilt`).
    pub loader: String,
    /// Versão de antes.
    pub from: String,
    /// Versão de agora.
    pub to: String,
    /// `false` quando a versão pedida já era a do pack (nada foi gravado).
    pub changed: bool,
}

/// Um item que não aceita uma versão do loader.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct LoaderIssue {
    /// Caminho do item no pack.
    pub path: String,
    /// Nome do item.
    pub name: String,
    /// A faixa de versões do loader que o item declara (`>=0.15.0`, `[47,)`), quando se sabe.
    pub requires: Option<String>,
}

/// A verificação dos itens contra uma versão nova do loader.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct LoaderCheck {
    /// Chave do loader.
    pub loader: String,
    /// Versão atual.
    pub from: String,
    /// Versão verificada.
    pub to: String,
    /// Itens que funcionam com a versão atual e não com a nova.
    pub breaks: Vec<LoaderIssue>,
    /// Itens que não funcionam com nenhuma das duas.
    pub still_broken: Vec<LoaderIssue>,
    /// Itens que não funcionam com a atual e voltam a funcionar com a nova.
    pub fixes: Vec<LoaderIssue>,
    /// Quantos mods foram verificados (com o arquivo lido).
    pub checked: u32,
    /// Nomes dos mods que não puderam ser verificados (o arquivo ainda não foi baixado: o
    /// primeiro teste baixa).
    pub unchecked: Vec<String>,
}

impl LoaderCheck {
    /// Classifica os itens pelas duas verificações (atual e nova). Os resultados ficam em ordem
    /// de nome.
    #[must_use]
    pub fn classify(
        loader: &str,
        from: &str,
        to: &str,
        with_current: Vec<LoaderIssue>,
        with_new: Vec<LoaderIssue>,
        checked: u32,
        mut unchecked: Vec<String>,
    ) -> Self {
        let current: BTreeMap<String, LoaderIssue> = with_current
            .into_iter()
            .map(|issue| (issue.path.clone(), issue))
            .collect();
        let new: BTreeMap<String, LoaderIssue> = with_new
            .into_iter()
            .map(|issue| (issue.path.clone(), issue))
            .collect();
        let mut breaks = Vec::new();
        let mut still_broken = Vec::new();
        for (path, issue) in &new {
            if current.contains_key(path) {
                still_broken.push(issue.clone());
            } else {
                breaks.push(issue.clone());
            }
        }
        let mut fixes: Vec<LoaderIssue> = current
            .into_iter()
            .filter(|(path, _)| !new.contains_key(path))
            .map(|(_, issue)| issue)
            .collect();
        for list in [&mut breaks, &mut still_broken, &mut fixes] {
            list.sort_by(|a, b| {
                a.name
                    .to_lowercase()
                    .cmp(&b.name.to_lowercase())
                    .then_with(|| a.path.cmp(&b.path))
            });
        }
        unchecked.sort_by_key(|name| name.to_lowercase());
        Self {
            loader: loader.to_owned(),
            from: from.to_owned(),
            to: to.to_owned(),
            breaks,
            still_broken,
            fixes,
            checked,
            unchecked,
        }
    }
}

/// O loader do pack e a versão dele, quando há exatamente um.
///
/// Erro [`Code::InvalidPack`] se o `pack.toml` não puder ser lido; [`Code::InvalidInput`]
/// (parâmetro `field = "loader"`) se o pack não tem loader (vanilla) ou tem mais de um.
pub fn current_loader(root: &Path) -> Result<(String, String)> {
    let (_, manifest) = read_manifest(root)?;
    let (loader, version) = single_loader(&manifest)?;
    Ok((loader.key().to_owned(), version))
}

fn single_loader(manifest: &PackManifest) -> Result<(Loader, String)> {
    match manifest.loaders().as_slice() {
        [(loader, version)] => Ok((*loader, (*version).to_owned())),
        _ => Err(Error::new(
            Code::InvalidInput,
            "o pack não tem um loader só para trocar a versão",
        )
        .param("field", "loader")),
    }
}

/// Valida o texto de uma versão de loader (sem espaços, até 64 caracteres, só letras, números
/// e `. + - _`). Devolve o texto sem espaços nas pontas.
pub fn validate_version(version: &str) -> Result<&str> {
    let version = version.trim();
    let reason = if version.is_empty() {
        Some("empty")
    } else if version.chars().count() > MAX_VERSION_CHARS {
        Some("tooLong")
    } else if !version
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '+' | '-' | '_'))
    {
        Some("invalid")
    } else {
        None
    };
    match reason {
        Some(reason) => Err(
            Error::new(Code::InvalidInput, format!("versão do loader: {reason}"))
                .param("field", "loaderVersion")
                .param("reason", reason),
        ),
        None => Ok(version),
    }
}

/// Grava a versão nova do loader, mudando só a linha dele em `[versions]`.
///
/// `expected_from` é a versão que a interface mostrou: se o `pack.toml` mudou depois, nada é
/// gravado ([`Code::PackChangedExternally`]). Erros de [`current_loader`] e
/// [`validate_version`].
pub async fn set_loader_version(
    root: &Path,
    expected_from: &str,
    version: &str,
) -> Result<LoaderVersionChanged> {
    let to = validate_version(version)?;
    let (text, manifest) = read_manifest(root)?;
    let (loader, from) = single_loader(&manifest)?;
    if from != expected_from {
        return Err(Error::new(
            Code::PackChangedExternally,
            "a versão do loader mudou depois de abrir o diálogo",
        )
        .param("path", PACK_FILE));
    }
    let mut result = LoaderVersionChanged {
        loader: loader.key().to_owned(),
        from,
        to: to.to_owned(),
        changed: false,
    };
    if result.from == to {
        return Ok(result);
    }
    let edited = set_pack_version(&text, loader.key(), Some(to))
        .map_err(|e| Error::new(Code::InvalidPack, e.to_string()))?;
    let mut tx = PackTransaction::new(root.to_path_buf());
    tx.write(PACK_FILE, edited.into_bytes())?;
    tx.commit_without_refresh().await?;
    result.changed = true;
    Ok(result)
}

fn read_manifest(root: &Path) -> Result<(String, PackManifest)> {
    let text = fs::read_to_string(root.join(PACK_FILE))
        .map_err(|e| Error::new(Code::InvalidPack, format!("{PACK_FILE}: {e}")))?;
    let manifest = PackManifest::parse(&text)
        .map_err(|e| Error::new(Code::InvalidPack, e.in_file(PACK_FILE).to_string()))?
        .value;
    Ok((text, manifest))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pack(dir: &Path, loaders: &[(Loader, &str)]) -> String {
        let mut manifest = PackManifest::new("Pack", "1.20.1");
        manifest.author = "Autor".into();
        for (loader, version) in loaders {
            manifest.set_loader_version(*loader, version);
        }
        let text = manifest.to_toml_string();
        fs::write(dir.join(PACK_FILE), &text).unwrap();
        text
    }

    fn issue(path: &str, name: &str) -> LoaderIssue {
        LoaderIssue {
            path: path.into(),
            name: name.into(),
            requires: Some(">=0.15".into()),
        }
    }

    #[tokio::test]
    async fn muda_so_a_linha_do_loader() {
        let dir = tempfile::tempdir().unwrap();
        let before = pack(dir.path(), &[(Loader::Fabric, "0.15.11")]);
        let changed = set_loader_version(dir.path(), "0.15.11", " 0.16.14 ")
            .await
            .unwrap();
        assert_eq!(
            changed,
            LoaderVersionChanged {
                loader: "fabric".into(),
                from: "0.15.11".into(),
                to: "0.16.14".into(),
                changed: true,
            }
        );
        let after = fs::read_to_string(dir.path().join(PACK_FILE)).unwrap();
        let diff: Vec<(&str, &str)> = before
            .lines()
            .zip(after.lines())
            .filter(|(a, b)| a != b)
            .collect();
        assert_eq!(diff, [("fabric = \"0.15.11\"", "fabric = \"0.16.14\"")]);
        assert_eq!(before.lines().count(), after.lines().count());
        assert_eq!(
            current_loader(dir.path()).unwrap(),
            ("fabric".into(), "0.16.14".into())
        );
        // A mesma versão não grava nada.
        let again = set_loader_version(dir.path(), "0.16.14", "0.16.14")
            .await
            .unwrap();
        assert!(!again.changed);
        assert_eq!(
            fs::read_to_string(dir.path().join(PACK_FILE)).unwrap(),
            after
        );
    }

    #[tokio::test]
    async fn recusa_sem_gravar() {
        let dir = tempfile::tempdir().unwrap();
        let before = pack(dir.path(), &[(Loader::Forge, "47.2.0")]);
        let stale = set_loader_version(dir.path(), "47.1.0", "47.3.0")
            .await
            .unwrap_err();
        assert_eq!(stale.code, Code::PackChangedExternally);
        for (input, reason) in [("  ", "empty"), ("47 1", "invalid"), ("1\"]", "invalid")] {
            let error = set_loader_version(dir.path(), "47.2.0", input)
                .await
                .unwrap_err();
            assert_eq!(error.code, Code::InvalidInput);
            assert_eq!(error.params["reason"], reason);
        }
        let long = "1".repeat(65);
        assert_eq!(
            validate_version(&long).unwrap_err().params["reason"],
            "tooLong"
        );
        assert_eq!(
            fs::read_to_string(dir.path().join(PACK_FILE)).unwrap(),
            before
        );

        // Vanilla e dois loaders: não há "a" versão do loader.
        let vanilla = tempfile::tempdir().unwrap();
        pack(vanilla.path(), &[]);
        let error = set_loader_version(vanilla.path(), "", "1")
            .await
            .unwrap_err();
        assert_eq!(error.params["field"], "loader");
        let two = tempfile::tempdir().unwrap();
        pack(
            two.path(),
            &[(Loader::Fabric, "0.16.14"), (Loader::Quilt, "0.26.0")],
        );
        assert_eq!(
            current_loader(two.path()).unwrap_err().code,
            Code::InvalidInput
        );
    }

    #[test]
    fn classifica_o_que_quebra_o_que_continua_e_o_que_volta() {
        let check = LoaderCheck::classify(
            "fabric",
            "0.16.14",
            "0.14.21",
            vec![
                issue("mods/velho.pw.toml", "Velho"),
                issue("mods/b.pw.toml", "b"),
            ],
            vec![
                issue("mods/sodium.pw.toml", "Sodium"),
                issue("mods/b.pw.toml", "b"),
                issue("mods/a.pw.toml", "Apple"),
            ],
            5,
            vec!["Zeta".into(), "alfa".into()],
        );
        let names = |list: &[LoaderIssue]| list.iter().map(|i| i.name.clone()).collect::<Vec<_>>();
        assert_eq!(names(&check.breaks), ["Apple", "Sodium"]);
        assert_eq!(names(&check.still_broken), ["b"]);
        assert_eq!(names(&check.fixes), ["Velho"]);
        assert_eq!(check.unchecked, ["alfa", "Zeta"]);
        assert_eq!(check.checked, 5);
    }
}
