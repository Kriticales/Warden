//! Fabric (`meta.fabricmc.net/v2`; R2 §3.2).
//!
//! `GET /v2/versions/game` diz para quais versões do Minecraft o Fabric existe (o intermediary
//! existe), e `GET /v2/versions/loader` lista as versões do loader, que não dependem da
//! versão do Minecraft (é o que o próprio packwiz assume). Assim, "Fabric para 1.12.2" é a
//! lista vazia: a 1.12.2 não está em `/versions/game` (o Fabric oficial começa na 1.14).

use serde::Deserialize;

use crate::model::LoaderVersion;

#[derive(Deserialize)]
struct GameVersion {
    version: String,
}

#[derive(Deserialize)]
struct LoaderEntry {
    version: String,
    #[serde(default)]
    stable: bool,
    #[serde(default)]
    maven: Option<String>,
}

/// As versões do Minecraft com Fabric. Erro: o texto do problema.
pub(crate) fn parse_game_versions(body: &[u8]) -> Result<Vec<String>, String> {
    let games: Vec<GameVersion> = serde_json::from_slice(body).map_err(|e| e.to_string())?;
    if games.is_empty() {
        return Err("a lista de versões do jogo do Fabric veio vazia".to_owned());
    }
    Ok(games.into_iter().map(|game| game.version).collect())
}

/// As versões do loader, na ordem da meta (da mais nova para a mais antiga). Erro: o texto
/// do problema.
pub(crate) fn parse_loader_versions(body: &[u8]) -> Result<Vec<LoaderVersion>, String> {
    let loaders: Vec<LoaderEntry> = serde_json::from_slice(body).map_err(|e| e.to_string())?;
    if loaders.is_empty() {
        return Err("a lista de versões do loader do Fabric veio vazia".to_owned());
    }
    Ok(loaders
        .into_iter()
        .filter(|loader| !loader.version.is_empty())
        .map(|loader| LoaderVersion {
            maven: loader
                .maven
                .filter(|maven| !maven.is_empty())
                .unwrap_or_else(|| format!("net.fabricmc:fabric-loader:{}", loader.version)),
            version: loader.version,
            stable: loader.stable,
            recommended: false,
        })
        .collect())
}

/// As versões do loader para `minecraft`: todas, se o Fabric existe para ela; nenhuma, se
/// não.
pub(crate) fn for_minecraft(
    games: &[String],
    loaders: &[LoaderVersion],
    minecraft: &str,
) -> Vec<LoaderVersion> {
    if games.iter().any(|game| game == minecraft) {
        loaders.to_vec()
    } else {
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    #[test]
    fn le_jogo_e_loader() {
        let games =
            parse_game_versions(br#"[{"version":"1.21.1","stable":true},{"version":"1.14"}]"#)
                .unwrap();
        assert_eq!(games, ["1.21.1", "1.14"]);
        let loaders = parse_loader_versions(
            br#"[{"version":"0.19.5","stable":true,"maven":"net.fabricmc:fabric-loader:0.19.5"},
                 {"version":"0.19.4"},{"version":""}]"#,
        )
        .unwrap();
        assert_eq!(loaders.len(), 2);
        assert!(loaders[0].stable);
        assert!(!loaders[1].stable);
        assert_eq!(loaders[1].maven, "net.fabricmc:fabric-loader:0.19.4");
        assert_eq!(for_minecraft(&games, &loaders, "1.14").len(), 2);
        assert!(for_minecraft(&games, &loaders, "1.12.2").is_empty());
    }

    #[test]
    fn recusa_listas_vazias_ou_invalidas() {
        assert!(parse_game_versions(b"[]").is_err());
        assert!(parse_game_versions(b"{").is_err());
        assert!(parse_loader_versions(b"[]").is_err());
        assert!(parse_loader_versions(b"[1]").is_err());
    }

    proptest! {
        #[test]
        fn entrada_aleatoria_nao_entra_em_panico(bytes in proptest::collection::vec(any::<u8>(), 0..256)) {
            let _ = parse_game_versions(&bytes);
            let _ = parse_loader_versions(&bytes);
        }
    }
}
