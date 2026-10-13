//! Detecção da versão do KubeJS e do CraftTweaker (ARCHITECTURE §10.2; R5B §3.1).
//!
//! A versão vem do descritor do jar (`mods.toml`, `fabric.mod.json`, `mcmod.info`, lidos pela
//! `warden-jarmeta`) e, quando o jar ainda não foi baixado ou o descritor traz um marcador como
//! `${file.jarVersion}`, do nome do arquivo no metafile do pack.
//!
//! O número que a interface mostra ("KubeJS 6") é a **linha** do mod, não o número completo:
//!
//! - KubeJS até a 1.21.1: `<Minecraft compactado>.<linha>.<…>`, como `2001.6.5-build.26`
//!   (1.20.1, linha 6), `2101.7.2-build.377` (1.21.1, linha 7) e `1802.5.5` (1.18.2, linha 5);
//! - KubeJS 8 (Minecraft 26.x): `<Minecraft>-<linha>.<…>`, como `26.1.2-8.0.6`;
//! - CraftTweaker: o primeiro número (`14.0.60` → 14; `4.1.20` → 4, o do 1.12.2).

use std::path::Path;

use serde::Serialize;

/// Id do KubeJS nos descritores.
pub const KUBEJS_ID: &str = "kubejs";
/// Id do CraftTweaker nos descritores.
pub const CRAFTTWEAKER_ID: &str = "crafttweaker";

/// A ferramenta de scripts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum ScriptTool {
    /// KubeJS (`kubejs/*_scripts/*.js`).
    Kubejs,
    /// CraftTweaker (`scripts/*.zs`).
    Crafttweaker,
}

/// Uma ferramenta encontrada no pack.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ToolVersion {
    /// A ferramenta.
    pub tool: ScriptTool,
    /// A linha (`6` em "KubeJS 6"); `None` se a versão não pôde ser lida.
    pub major: Option<u32>,
    /// A versão completa, como veio do jar ou do nome do arquivo.
    pub version: Option<String>,
}

/// As ferramentas de scripts do pack.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ScriptEnv {
    /// KubeJS, se o pack tem.
    pub kubejs: Option<ToolVersion>,
    /// CraftTweaker, se o pack tem.
    pub crafttweaker: Option<ToolVersion>,
}

impl ScriptEnv {
    /// A ferramenta dona do arquivo, se o pack a tem: `.js` em `kubejs/` → KubeJS; `.zs` em
    /// `scripts/` → CraftTweaker.
    #[must_use]
    pub fn tool_for(&self, path: &str) -> Option<&ToolVersion> {
        match ScriptFile::classify(path)? {
            ScriptFile::Kubejs(_) => self.kubejs.as_ref(),
            ScriptFile::Crafttweaker => self.crafttweaker.as_ref(),
        }
    }
}

/// Tipo de script do KubeJS, pela pasta.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum KubejsScriptType {
    /// `kubejs/startup_scripts/`.
    Startup,
    /// `kubejs/server_scripts/`.
    Server,
    /// `kubejs/client_scripts/`.
    Client,
}

impl KubejsScriptType {
    /// O nome usado nos logs, nas pastas e nas rotas (`server`).
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Startup => "startup",
            Self::Server => "server",
            Self::Client => "client",
        }
    }

    /// Todos, na ordem em que o jogo os carrega.
    pub const ALL: [Self; 3] = [Self::Startup, Self::Server, Self::Client];
}

/// O que é um arquivo do pack para o editor de scripts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScriptFile {
    /// Script do KubeJS do tipo dado.
    Kubejs(KubejsScriptType),
    /// Script do CraftTweaker.
    Crafttweaker,
}

impl ScriptFile {
    /// Classifica um caminho relativo ao pack (ou à pasta do jogo), com `/`.
    #[must_use]
    pub fn classify(path: &str) -> Option<Self> {
        let path = path.trim_start_matches("./");
        let lower = path.to_ascii_lowercase();
        let extension = Path::new(&lower).extension().and_then(|e| e.to_str());
        if extension == Some("js") {
            let rest = lower.strip_prefix("kubejs/")?;
            let (folder, name) = rest.split_once('/')?;
            if name.is_empty() {
                return None;
            }
            return Some(Self::Kubejs(match folder {
                "startup_scripts" => KubejsScriptType::Startup,
                "server_scripts" => KubejsScriptType::Server,
                "client_scripts" => KubejsScriptType::Client,
                _ => return None,
            }));
        }
        if extension == Some("zs") && lower.starts_with("scripts/") {
            return Some(Self::Crafttweaker);
        }
        None
    }
}

/// Um mod lido de um jar (ou só do nome do arquivo).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DetectedMod {
    /// Id do mod no descritor (`None` quando o jar não pôde ser lido). Um jar lido entra com o
    /// id de cada mod que declara, mesmo os que não são ferramentas de scripts: um addon como o
    /// KubeJS Create (`kubejs_create`) fica de fora pelo id, não pelo nome.
    pub id: Option<String>,
    /// Versão no descritor.
    pub version: Option<String>,
    /// Nome do arquivo (`kubejs-forge-2001.6.5-build.26.jar`).
    pub file_name: String,
}

/// Loaders que aparecem logo depois do nome da ferramenta no nome do arquivo
/// (`kubejs-forge-…`, `CraftTweaker-neoforge-…`).
const LOADERS: [&str; 4] = ["forge", "neoforge", "fabric", "quilt"];

/// A ferramenta pelo nome do arquivo, só para jars que não puderam ser lidos. O nome precisa
/// ser o da ferramenta seguido do loader ou da versão (`kubejs-forge-2001.6.5…`,
/// `kubejs-1.12.2-…`, `CraftTweaker2-1.12-4.1.20`): addons como `kubejs-create-…`,
/// `kubejs-thermal-…` e `kubejs_additions-…` não passam.
fn tool_from_file_name(file_name: &str) -> Option<ScriptTool> {
    let lower = file_name.to_ascii_lowercase();
    let stem = lower.strip_suffix(".jar").unwrap_or(&lower);
    let mut parts = stem.split(['-', '_']);
    let tool = match parts.next()? {
        "kubejs" => ScriptTool::Kubejs,
        "crafttweaker" | "crafttweaker2" => ScriptTool::Crafttweaker,
        _ => return None,
    };
    let next = parts.next()?;
    let loader_or_version =
        LOADERS.contains(&next) || next.starts_with(|c: char| c.is_ascii_digit());
    loader_or_version.then_some(tool)
}

/// De onde veio a ferramenta: o descritor do jar ganha do nome do arquivo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Evidence {
    FileName,
    Descriptor,
}

/// Detecta as ferramentas na lista de mods do pack. O descritor ganha do nome do arquivo, e
/// a ordem da lista não importa: um jar identificado pelo id do mod ganha de um adivinhado pelo
/// nome; entre dois do mesmo tipo, ganha o que tem a versão lida.
#[must_use]
pub fn detect(mods: &[DetectedMod]) -> ScriptEnv {
    let mut env = ScriptEnv::default();
    let mut evidence: [Option<Evidence>; 2] = [None, None];
    for module in mods {
        let id = module.id.as_deref().map(str::to_ascii_lowercase);
        let (tool, found_by) = match id.as_deref() {
            Some(KUBEJS_ID) => (ScriptTool::Kubejs, Evidence::Descriptor),
            Some(CRAFTTWEAKER_ID) => (ScriptTool::Crafttweaker, Evidence::Descriptor),
            Some(_) => continue,
            None => match tool_from_file_name(&module.file_name) {
                Some(tool) => (tool, Evidence::FileName),
                None => continue,
            },
        };
        let usable = module
            .version
            .as_deref()
            .filter(|version| !version.contains("${") && !version.trim().is_empty());
        let major_of = match tool {
            ScriptTool::Kubejs => kubejs_major,
            ScriptTool::Crafttweaker => crafttweaker_major,
        };
        let (major, version) = usable.and_then(major_of).map_or_else(
            || {
                let from_name = file_version(&module.file_name, tool);
                (
                    from_name.as_ref().map(|(m, _)| *m),
                    from_name.map(|(_, v)| v),
                )
            },
            |major| (Some(major), usable.map(ToOwned::to_owned)),
        );
        let found = ToolVersion {
            tool,
            major,
            version,
        };
        let (slot, slot_evidence) = match tool {
            ScriptTool::Kubejs => (&mut env.kubejs, &mut evidence[0]),
            ScriptTool::Crafttweaker => (&mut env.crafttweaker, &mut evidence[1]),
        };
        let better = match (slot.as_ref(), *slot_evidence) {
            (Some(old), Some(old_by)) => {
                found_by > old_by || (found_by == old_by && old.major.is_none() && major.is_some())
            }
            _ => true,
        };
        if better {
            *slot = Some(found);
            *slot_evidence = Some(found_by);
        }
    }
    env
}

/// A linha do KubeJS numa versão.
#[must_use]
pub fn kubejs_major(version: &str) -> Option<u32> {
    let version = version.split('+').next()?.trim();
    let mut dashes = version.split('-');
    let first = dashes.next()?;
    let parts = numbers(first)?;
    match parts.as_slice() {
        // `2001.6.5`: Minecraft compactado (4 dígitos ou mais) e a linha.
        [packed, line, ..] if *packed >= 1000 => Some(*line),
        // `26.1.2-8.0.6`: Minecraft e, depois do traço, a versão do KubeJS.
        [_, _, ..] => dashes
            .next()
            .and_then(numbers)
            .and_then(|after| after.first().copied())
            .filter(|_| first.split('.').count() >= 2),
        _ => None,
    }
}

/// A linha do CraftTweaker numa versão (`14.0.60` → 14).
#[must_use]
pub fn crafttweaker_major(version: &str) -> Option<u32> {
    let version = version.split('+').next()?.trim();
    numbers(version.split('-').next()?)?.first().copied()
}

/// Os números de `a.b.c` (todos precisam ser números).
fn numbers(text: &str) -> Option<Vec<u32>> {
    let parts: Option<Vec<u32>> = text.split('.').map(|part| part.parse().ok()).collect();
    parts.filter(|parts| !parts.is_empty())
}

/// A versão tirada do nome do arquivo: o primeiro trecho (separado por `-`) que a função
/// reconhece, juntando o seguinte quando ele também é número (`26.1.2-8.0.6`). No CraftTweaker,
/// o trecho de versão é o último (`CraftTweaker-forge-1.20.1-14.0.60.jar`).
fn file_version(file_name: &str, tool: ScriptTool) -> Option<(u32, String)> {
    let stem = file_name
        .strip_suffix(".jar")
        .or_else(|| file_name.strip_suffix(".JAR"))
        .unwrap_or(file_name);
    let parts: Vec<&str> = stem.split('-').collect();
    if tool == ScriptTool::Crafttweaker {
        let last = parts.iter().rev().find(|part| numbers(part).is_some())?;
        return crafttweaker_major(last).map(|m| (m, (*last).to_owned()));
    }
    for (index, part) in parts.iter().enumerate() {
        if numbers(part).is_none() {
            continue;
        }
        let candidate = match parts.get(index + 1) {
            Some(next) if numbers(next).is_some() => format!("{part}-{next}"),
            Some(next) if next.starts_with("build") => format!("{part}-{next}"),
            _ => (*part).to_owned(),
        };
        if let Some(found) = kubejs_major(&candidate) {
            return Some((found, candidate));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn jar(id: Option<&str>, version: Option<&str>, file: &str) -> DetectedMod {
        DetectedMod {
            id: id.map(ToOwned::to_owned),
            version: version.map(ToOwned::to_owned),
            file_name: file.to_owned(),
        }
    }

    #[test]
    fn linhas_do_kubejs_por_faixa() {
        assert_eq!(kubejs_major("2001.6.5-build.26"), Some(6));
        assert_eq!(kubejs_major("2001.6.5-build.26+forge"), Some(6));
        assert_eq!(kubejs_major("2101.7.2-build.377"), Some(7));
        assert_eq!(kubejs_major("1802.5.5-build.569"), Some(5));
        assert_eq!(kubejs_major("1605.3.19-build.299"), Some(3));
        assert_eq!(kubejs_major("26.1.2-8.0.6"), Some(8));
        assert_eq!(kubejs_major("x"), None);
        assert_eq!(kubejs_major("6"), None);
    }

    #[test]
    fn linhas_do_crafttweaker() {
        assert_eq!(crafttweaker_major("14.0.60"), Some(14));
        assert_eq!(crafttweaker_major("21.0.38"), Some(21));
        assert_eq!(crafttweaker_major("4.1.20"), Some(4));
        assert_eq!(crafttweaker_major("abc"), None);
    }

    #[test]
    fn detecta_pelo_descritor_e_pelo_nome() {
        let env = detect(&[
            jar(Some("sodium"), Some("0.5.11"), "sodium.jar"),
            jar(
                Some("kubejs"),
                Some("2001.6.5-build.26"),
                "kubejs-forge-2001.6.5-build.26.jar",
            ),
            jar(None, None, "CraftTweaker-forge-1.20.1-14.0.60.jar"),
        ]);
        assert_eq!(
            env.kubejs,
            Some(ToolVersion {
                tool: ScriptTool::Kubejs,
                major: Some(6),
                version: Some("2001.6.5-build.26".into()),
            })
        );
        assert_eq!(
            env.crafttweaker,
            Some(ToolVersion {
                tool: ScriptTool::Crafttweaker,
                major: Some(14),
                version: Some("14.0.60".into()),
            })
        );
    }

    #[test]
    fn marcador_no_descritor_cai_para_o_nome() {
        let env = detect(&[jar(
            Some("kubejs"),
            Some("${file.jarVersion}"),
            "kubejs-neoforge-2101.7.2-build.377.jar",
        )]);
        let kubejs = env.kubejs.unwrap();
        assert_eq!(kubejs.major, Some(7));
        assert_eq!(kubejs.version.as_deref(), Some("2101.7.2-build.377"));

        let env = detect(&[jar(None, None, "kubejs-neoforge-26.1.2-8.0.6.jar")]);
        assert_eq!(env.kubejs.unwrap().major, Some(8));
        let env = detect(&[jar(None, None, "CraftTweaker-1.12-4.1.20.jar")]);
        assert_eq!(env.crafttweaker.unwrap().major, Some(4));
        // Sem número legível: a ferramenta aparece sem linha.
        let env = detect(&[jar(Some("kubejs"), None, "kubejs.jar")]);
        assert_eq!(env.kubejs.unwrap().major, None);
    }

    #[test]
    fn addons_e_pack_sem_ferramenta() {
        let env = detect(&[
            jar(
                Some("kubejs_create"),
                Some("2001.2.5"),
                "kubejs-create-2001.2.5.jar",
            ),
            jar(Some("jei"), Some("15.0"), "jei.jar"),
        ]);
        assert_eq!(env, ScriptEnv::default());
        assert!(env.tool_for("kubejs/server_scripts/a.js").is_none());
    }

    /// KubeJS 6 com o KubeJS Create: o packwiz lista `kubejs-create` antes de `kubejs`, e o
    /// addon não pode virar "KubeJS 2", nem lido (id `kubejs_create`) nem só pelo nome.
    #[test]
    fn kubejs_com_addons_em_qualquer_ordem() {
        let kubejs = jar(
            Some("kubejs"),
            Some("2001.6.5-build.26"),
            "kubejs-forge-2001.6.5-build.26.jar",
        );
        let addons = [
            jar(
                Some("kubejs_create"),
                Some("2001.2.5-build.2"),
                "kubejs-create-forge-2001.2.5-build.2.jar",
            ),
            jar(
                Some("kubejs_thermal"),
                Some("2001.1.10-build.3"),
                "kubejs-thermal-forge-2001.1.10-build.3.jar",
            ),
            jar(Some("probejs"), Some("6.0.1"), "probejs-6.0.1-forge.jar"),
            jar(
                Some("kubejsadditions"),
                Some("4.3.3"),
                "kubejsadditions-forge-4.3.3.jar",
            ),
            // Jars que não puderam ser lidos: só o nome.
            jar(None, None, "kubejs-create-forge-2001.2.5-build.2.jar"),
            jar(None, None, "kubejs-thermal-forge-2001.1.10-build.3.jar"),
            jar(None, None, "kubejs_additions-forge-1.20.1-4.3.3.jar"),
        ];
        for addon in &addons {
            for order in [
                vec![addon.clone(), kubejs.clone()],
                vec![kubejs.clone(), addon.clone()],
            ] {
                let env = detect(&order);
                let found = env.kubejs.unwrap();
                assert_eq!(found.major, Some(6), "{addon:?}");
                assert_eq!(found.version.as_deref(), Some("2001.6.5-build.26"));
            }
            // Sozinho, o addon não é o KubeJS.
            assert_eq!(detect(std::slice::from_ref(addon)), ScriptEnv::default());
        }
        // Todos juntos, nas duas ordens.
        let mut all: Vec<DetectedMod> = addons.to_vec();
        all.push(kubejs.clone());
        assert_eq!(detect(&all).kubejs.unwrap().major, Some(6));
        all.reverse();
        assert_eq!(detect(&all).kubejs.unwrap().major, Some(6));
    }

    /// O nome do arquivo só vale quando o jar não foi lido, e o descritor ganha dele em qualquer
    /// ordem.
    #[test]
    fn descritor_ganha_do_nome_em_qualquer_ordem() {
        let unread = jar(None, None, "kubejs-forge-2001.6.5-build.26.jar");
        let read = jar(
            Some("kubejs"),
            Some("2101.7.2-build.377"),
            "kubejs-neoforge-2101.7.2-build.377.jar",
        );
        assert_eq!(
            detect(&[unread.clone(), read.clone()])
                .kubejs
                .unwrap()
                .major,
            Some(7)
        );
        assert_eq!(detect(&[read, unread]).kubejs.unwrap().major, Some(7));
        assert_eq!(
            detect(&[jar(None, None, "kubejs-1.12.2-1.0.0.jar")])
                .kubejs
                .map(|tool| tool.tool),
            Some(ScriptTool::Kubejs)
        );
        assert!(
            detect(&[jar(None, None, "CraftTweaker2-1.12-4.1.20.jar")])
                .crafttweaker
                .is_some()
        );
    }

    #[test]
    fn classifica_arquivos() {
        assert_eq!(
            ScriptFile::classify("kubejs/server_scripts/receitas.js"),
            Some(ScriptFile::Kubejs(KubejsScriptType::Server))
        );
        assert_eq!(
            ScriptFile::classify("kubejs/startup_scripts/sub/itens.js"),
            Some(ScriptFile::Kubejs(KubejsScriptType::Startup))
        );
        assert_eq!(
            ScriptFile::classify("kubejs/client_scripts/x.js"),
            Some(ScriptFile::Kubejs(KubejsScriptType::Client))
        );
        assert_eq!(
            ScriptFile::classify("scripts/receitas.zs"),
            Some(ScriptFile::Crafttweaker)
        );
        assert_eq!(ScriptFile::classify("kubejs/config/web_server.json"), None);
        assert_eq!(ScriptFile::classify("kubejs/assets/x.js"), None);
        assert_eq!(ScriptFile::classify("config/a.js"), None);
        assert_eq!(ScriptFile::classify("kubejs/server_scripts/"), None);
    }
}
