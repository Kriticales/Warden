//! O que vai para o servidor (ARCHITECTURE §12.1 item 1; SPEC T19): as regras por caminho,
//! iguais nas duas variantes, e a leitura dos itens de referência com o lado de cada um.

use std::collections::BTreeMap;

use warden_packwiz::{Metafile, PACK_FILE, Side};

use crate::{Error, ExportErrorCode, Result};

/// Prefixo dos arquivos que valem só no servidor e substituem os do pack.
pub(super) const OVERRIDES_DIR: &str = "server-overrides/";

/// Arquivos de cliente na raiz do pack: nunca vão para o servidor.
const CLIENT_ROOT_FILES: [&str; 5] = [
    "options.txt",
    "optionsof.txt",
    "optionsshaders.txt",
    "servers.dat",
    "servers.dat_old",
];

/// Pastas de cliente: nunca vão para o servidor.
const CLIENT_DIRS: [&str; 3] = ["resourcepacks/", "shaderpacks/", "kubejs/client_scripts/"];

/// O que vai no zip pelo link, além dos scripts: as configs e os scripts do servidor (o
/// installer traz o resto, inclusive os mods).
const LINK_DIRS: [&str; 7] = [
    "config/",
    "defaultconfigs/",
    "kubejs/",
    "scripts/",
    "global_packs/",
    "openloader/",
    "datapacks/",
];

/// O `server.properties` de referência, quando o pack tem um.
const SERVER_PROPERTIES: &str = "server.properties";

/// O aceite da EULA nunca vai no pacote: quem roda o servidor responde ao script.
pub(super) const EULA_FILE: &str = "eula.txt";

/// Onde um arquivo do pack (que não é referência) vai parar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum FilePlace {
    /// Vai para o servidor, no mesmo caminho.
    Server,
    /// Só cliente: fica de fora.
    Client,
    /// De `server-overrides/`: vai para este caminho, por cima do arquivo do pack.
    Override(String),
    /// Nunca vai (`eula.txt`, os arquivos de controle).
    Never,
}

/// Se o caminho está numa pasta de cliente ou é um arquivo de cliente.
pub(super) fn is_client_path(path: &str) -> bool {
    CLIENT_ROOT_FILES.contains(&path) || CLIENT_DIRS.iter().any(|dir| path.starts_with(dir))
}

/// A regra de um arquivo do pack.
pub(super) fn place(path: &str) -> FilePlace {
    if matches!(path, PACK_FILE | "index.toml") {
        return FilePlace::Never;
    }
    if let Some(rest) = path.strip_prefix(OVERRIDES_DIR) {
        if rest.is_empty() || rest == EULA_FILE || is_client_path(rest) {
            return FilePlace::Never;
        }
        return FilePlace::Override(rest.to_owned());
    }
    if path == EULA_FILE {
        return FilePlace::Never;
    }
    if is_client_path(path) {
        FilePlace::Client
    } else {
        FilePlace::Server
    }
}

/// Se o arquivo de servidor vai também no zip pelo link (as configs e os scripts).
pub(super) fn in_link_zip(path: &str) -> bool {
    path == SERVER_PROPERTIES || LINK_DIRS.iter().any(|dir| path.starts_with(dir))
}

/// Pastas cujo conteúdo local é de terceiros (pede confirmação de licença).
pub(super) fn is_third_party_local(path: &str) -> bool {
    path.starts_with("mods/")
}

/// Um mod, datapack, resource pack ou shader de referência (`.pw.toml`).
#[derive(Debug, Clone)]
pub(super) struct RefItem {
    /// Caminho do `.pw.toml`.
    pub(super) meta_path: String,
    /// Nome mostrado.
    pub(super) name: String,
    /// Caminho do arquivo no jogo (`mods/x.jar`).
    pub(super) dest: String,
    /// O lado.
    pub(super) side: Side,
    /// IDs da CurseForge, quando o item vem de lá.
    pub(super) curseforge: Option<(u32, u32)>,
    /// O metafile lido.
    pub(super) meta: Metafile,
}

impl RefItem {
    /// Se o item vai para o servidor: o lado não é "Só cliente" e o arquivo não é de cliente
    /// (resource pack ou shader).
    pub(super) fn on_server(&self) -> bool {
        !matches!(self.side, Side::Client) && !is_client_path(&self.dest)
    }
}

/// Lê um `.pw.toml` da cópia do pack.
pub(super) fn read_ref(path: &str, bytes: &[u8]) -> Result<RefItem> {
    let bad = |what: &str| {
        Error::new(
            ExportErrorCode::ServerPackUnsupportedPack,
            format!("{path}: {what}"),
        )
    };
    let text = std::str::from_utf8(bytes).map_err(|_| bad("não é UTF-8"))?;
    let meta = Metafile::parse(text)
        .map_err(|e| bad(&e.to_string()))?
        .value;
    let filename = meta
        .filename
        .rsplit('/')
        .next()
        .unwrap_or_default()
        .to_owned();
    if filename.is_empty() {
        return Err(bad("sem nome de arquivo"));
    }
    let folder = path.rsplit_once('/').map_or("", |(dir, _)| dir);
    let dest = if folder.is_empty() {
        filename.clone()
    } else {
        format!("{folder}/{filename}")
    };
    let curseforge = if meta.is_curseforge_mode() {
        meta.curseforge_ids()
    } else {
        None
    };
    Ok(RefItem {
        meta_path: path.to_owned(),
        name: if meta.name.trim().is_empty() {
            filename
        } else {
            meta.name.trim().to_owned()
        },
        dest,
        side: meta.side.clone(),
        curseforge,
        meta,
    })
}

/// Os itens de referência da cópia do pack, em ordem de caminho.
pub(super) fn refs(files: &BTreeMap<String, Vec<u8>>) -> Result<Vec<RefItem>> {
    files
        .iter()
        .filter(|(path, _)| path.ends_with(".pw.toml"))
        .map(|(path, bytes)| read_ref(path, bytes))
        .collect()
}

/// Conta por pasta de primeiro nível (`.` para os arquivos soltos na raiz).
pub(super) fn folders<'a>(
    paths: impl IntoIterator<Item = &'a str>,
) -> Vec<super::ServerPackFolder> {
    let mut counts = BTreeMap::<String, usize>::new();
    for path in paths {
        let top = path.split_once('/').map_or(".", |(dir, _)| dir);
        *counts.entry(top.to_owned()).or_default() += 1;
    }
    counts
        .into_iter()
        .map(|(path, files)| super::ServerPackFolder { path, files })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn regras_por_caminho() {
        assert_eq!(place("config/a.toml"), FilePlace::Server);
        assert_eq!(place("kubejs/server_scripts/a.js"), FilePlace::Server);
        assert_eq!(place("kubejs/client_scripts/a.js"), FilePlace::Client);
        assert_eq!(place("options.txt"), FilePlace::Client);
        assert_eq!(place("servers.dat"), FilePlace::Client);
        assert_eq!(place("resourcepacks/a.zip"), FilePlace::Client);
        assert_eq!(place("shaderpacks/a.zip"), FilePlace::Client);
        assert_eq!(place("eula.txt"), FilePlace::Never);
        assert_eq!(place("pack.toml"), FilePlace::Never);
        assert_eq!(place("index.toml"), FilePlace::Never);
        assert_eq!(
            place("server-overrides/config/a.toml"),
            FilePlace::Override("config/a.toml".into())
        );
        assert_eq!(place("server-overrides/eula.txt"), FilePlace::Never);
        assert_eq!(place("server-overrides/options.txt"), FilePlace::Never);
        assert!(in_link_zip("config/a.toml"));
        assert!(in_link_zip("server.properties"));
        assert!(in_link_zip("kubejs/startup_scripts/a.js"));
        assert!(!in_link_zip("mods/local.jar"));
        assert!(is_third_party_local("mods/local.jar"));
    }

    #[test]
    fn pastas_contam_a_raiz_como_ponto() {
        let counted = folders(["config/a", "config/b/c", "server.properties"]);
        assert_eq!(
            counted,
            vec![
                super::super::ServerPackFolder {
                    path: ".".into(),
                    files: 1
                },
                super::super::ServerPackFolder {
                    path: "config".into(),
                    files: 2
                },
            ]
        );
    }
}
