//! Peças usadas por mais de um formato.

use warden_packwiz::{Loader, PackManifest};

use crate::error::{Error, ImportErrorCode as Code, Result};

/// Confere os loaders do modpack (como o T04 faz no Abrir pack): Forge, NeoForge, Fabric ou
/// nenhum; Quilt e `LiteLoader` recusados com o nome; mais de um recusado.
pub(crate) fn check_loaders(loaders: &[(Loader, String)]) -> Result<()> {
    if loaders.len() > 1 {
        return Err(Error::new(
            Code::UnsupportedLoader,
            "o modpack declara mais de um loader",
        )
        .with_param("reason", "multiple"));
    }
    if let Some((loader, _)) = loaders.first()
        && matches!(loader, Loader::Quilt | Loader::LiteLoader)
    {
        return Err(Error::new(
            Code::UnsupportedLoader,
            format!("{} ainda não é suportado", loader.key()),
        )
        .with_param("loader", loader.key()));
    }
    Ok(())
}

/// O `pack.toml` do pack novo (nome provisório; a pessoa troca na tela).
pub(crate) fn manifest(
    name: &str,
    fallback_name: &str,
    minecraft: &str,
    loaders: &[(Loader, String)],
) -> Result<PackManifest> {
    let minecraft = minecraft.trim();
    if minecraft.is_empty() || minecraft.len() > 64 || minecraft.chars().any(char::is_control) {
        return Err(Error::new(
            Code::MinecraftMissing,
            "o modpack não informa a versão do Minecraft",
        ));
    }
    check_loaders(loaders)?;
    let name = name.trim();
    let name = if name.is_empty() {
        fallback_name.trim()
    } else {
        name
    };
    let name: String = name.chars().filter(|c| !c.is_control()).take(64).collect();
    let mut manifest = PackManifest::new(&name, minecraft);
    for (loader, version) in loaders {
        manifest.set_loader_version(*loader, version.trim());
    }
    Ok(manifest)
}

/// Pasta e nome de um caminho relativo (`mods/a.jar` → `("mods", "a.jar")`).
pub(crate) fn split_path(path: &str) -> (String, String) {
    match path.rsplit_once('/') {
        Some((dir, name)) => (dir.to_owned(), name.to_owned()),
        None => (String::new(), path.to_owned()),
    }
}

/// Junta pasta e nome, sem barra quando a pasta é a raiz.
pub(crate) fn join(dir: &str, name: &str) -> String {
    if dir.is_empty() {
        name.to_owned()
    } else {
        format!("{dir}/{name}")
    }
}

/// Nome do arquivo sem a extensão (nome provisório do pack).
pub(crate) fn stem(file_name: &str) -> String {
    std::path::Path::new(file_name).file_stem().map_or_else(
        || file_name.to_owned(),
        |stem| stem.to_string_lossy().into_owned(),
    )
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    #[test]
    fn loaders_aceitos_e_recusados() {
        assert!(check_loaders(&[]).is_ok());
        assert!(check_loaders(&[(Loader::NeoForge, "21.1.1".into())]).is_ok());
        let quilt = check_loaders(&[(Loader::Quilt, "0.26".into())]).unwrap_err();
        assert_eq!(quilt.code(), Code::UnsupportedLoader);
        assert_eq!(quilt.params()["loader"], "quilt");
        let two = check_loaders(&[(Loader::Forge, "1".into()), (Loader::Fabric, "2".into())])
            .unwrap_err();
        assert_eq!(two.params()["reason"], "multiple");
    }

    #[test]
    fn caminhos() {
        assert_eq!(split_path("mods/a.jar"), ("mods".into(), "a.jar".into()));
        assert_eq!(split_path("a.jar"), (String::new(), "a.jar".into()));
        assert_eq!(join("", "a"), "a");
        assert_eq!(join("mods", "a"), "mods/a");
        assert_eq!(stem("Create+ 6.0.0.mrpack"), "Create+ 6.0.0");
        assert_eq!(
            manifest("", "Arquivo", "1.21.1", &[]).unwrap().name,
            "Arquivo"
        );
        assert_eq!(
            manifest("X", "Y", " ", &[]).unwrap_err().code(),
            Code::MinecraftMissing
        );
    }
}
