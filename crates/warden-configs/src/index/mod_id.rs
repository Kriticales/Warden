//! O mod dono de um arquivo, deduzido do caminho (filtro "Só deste mod").
//!
//! Os mods gravam as configs com o próprio id: `config/<id>-common.toml`, `config/<id>.json`,
//! `config/<id>/…`. Dedução, não certeza: um arquivo que não segue o costume aparece com o nome
//! que tem. `options.txt` e os arquivos soltos da raiz não são de mod.

/// Pastas em que ficam as configs, antes do nome do mod.
const CONFIG_DIRS: [&str; 2] = ["config", "defaultconfigs"];

/// Sufixos do lado da config quando o mod separa com `_` (`farmersdelight_common.toml`).
const SIDE_SUFFIXES: [&str; 3] = ["_common", "_client", "_server"];

/// O id do mod dono de `path` (relativo à origem, com `/`), em minúsculas.
#[must_use]
pub(crate) fn mod_id(path: &str) -> Option<String> {
    let parts: Vec<&str> = path.split('/').filter(|part| !part.is_empty()).collect();
    let rest: &[&str] = match parts.as_slice() {
        [first, rest @ ..]
            if CONFIG_DIRS
                .iter()
                .any(|dir| first.eq_ignore_ascii_case(dir)) =>
        {
            rest
        }
        // `saves/<mundo>/serverconfig/<arquivo>`
        [saves, _, server, rest @ ..]
            if saves.eq_ignore_ascii_case("saves")
                && server.eq_ignore_ascii_case("serverconfig") =>
        {
            rest
        }
        [first, _, ..] if first.eq_ignore_ascii_case("kubejs") => return Some("kubejs".into()),
        _ => return None,
    };
    let name = match rest {
        [] => return None,
        [file] => stem(file),
        // Uma subpasta: o nome dela (`config/jei/jei-client.ini` → `jei`).
        [folder, ..] => folder,
    };
    let first = name.split('-').next().unwrap_or(name).trim();
    let id = SIDE_SUFFIXES
        .iter()
        .find_map(|suffix| first.strip_suffix(suffix))
        .unwrap_or(first)
        .to_ascii_lowercase();
    (!id.is_empty()).then_some(id)
}

/// O nome sem a última extensão.
fn stem(file: &str) -> &str {
    file.rsplit_once('.').map_or(file, |(stem, _)| stem)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deduz_pelo_nome_do_arquivo_ou_da_pasta() {
        let cases = [
            ("config/create-common.toml", Some("create")),
            ("config/waystones-common.toml", Some("waystones")),
            ("defaultconfigs/epicfight-server.toml", Some("epicfight")),
            ("config/sodium-options.json", Some("sodium")),
            ("config/jei/jei-client.ini", Some("jei")),
            ("config/coldsweat/world_settings.toml", Some("coldsweat")),
            ("config/BetterF3.json", Some("betterf3")),
            ("config/farmersdelight_common.toml", Some("farmersdelight")),
            (
                "saves/Mundo/serverconfig/create-server.toml",
                Some("create"),
            ),
            ("kubejs/config/common.properties", Some("kubejs")),
            ("kubejs/server_scripts/x.js", Some("kubejs")),
            ("options.txt", None),
            ("config", None),
            ("scripts/receitas.zs", None),
            ("config/.toml", None),
            ("config/_client.toml", None),
            (
                "config/create_central_kitchen.toml",
                Some("create_central_kitchen"),
            ),
        ];
        for (path, expected) in cases {
            assert_eq!(mod_id(path).as_deref(), expected, "{path}");
        }
    }
}
