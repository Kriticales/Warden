//! `META-INF/MANIFEST.MF` (seção principal), conforme a especificação de jars do Java: linhas
//! `Nome: valor`, continuação começando com um espaço, fim da seção principal na primeira linha
//! em branco. Nomes sem diferença entre maiúsculas e minúsculas.

use crate::model::ManifestInfo;

/// Atributos da seção principal, na ordem do arquivo (nome como escrito, valor sem espaços nas
/// pontas).
pub(crate) fn main_attributes(text: &str) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
    for line in normalized.split('\n') {
        if line.is_empty() {
            if out.is_empty() {
                continue;
            }
            break;
        }
        if let Some(rest) = line.strip_prefix(' ') {
            if let Some((_, value)) = out.last_mut() {
                value.push_str(rest);
            }
            continue;
        }
        if let Some((name, value)) = line.split_once(':') {
            out.push((name.trim().to_owned(), value.trim_start().to_owned()));
        }
    }
    for (_, value) in &mut out {
        let trimmed = value.trim().to_owned();
        *value = trimmed;
    }
    out
}

fn split_list(value: &str, separators: &[char]) -> Vec<String> {
    value
        .split(separators)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect()
}

pub(crate) fn parse(text: &str) -> ManifestInfo {
    let attributes = main_attributes(text);
    let get = |name: &str| {
        attributes
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.clone())
            .filter(|v| !v.is_empty())
    };
    ManifestInfo {
        fml_core_plugin: get("FMLCorePlugin"),
        fml_core_plugin_contains_mod: get("FMLCorePluginContainsFMLMod")
            .is_some_and(|v| v.eq_ignore_ascii_case("true")),
        tweak_class: get("TweakClass"),
        mixin_configs: get("MixinConfigs")
            .map(|v| split_list(&v, &[',']))
            .unwrap_or_default(),
        fml_mod_type: get("FMLModType"),
        fml_access_transformer: get("FMLAT"),
        contained_deps: get("ContainedDeps")
            .map(|v| split_list(&v, &[' ', ',']))
            .unwrap_or_default(),
        embedded_dependencies_mod: get("Embedded-Dependencies-Mod"),
        implementation_title: get("Implementation-Title"),
        implementation_version: get("Implementation-Version"),
        automatic_module_name: get("Automatic-Module-Name"),
        multi_release: get("Multi-Release").is_some_and(|v| v.eq_ignore_ascii_case("true")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn manifesto_de_coremod_1_7_10() {
        let info = parse(
            "Manifest-Version: 1.0\r\nFMLCorePlugin: codechicken.core.launch.CodeChickenCo\r\n reCorePlugin\r\nFMLCorePluginContainsFMLMod: true\r\nfmlat: cc_at.cfg\r\n\r\nName: x/y\r\nTweakClass: nao.conta\r\n",
        );
        assert_eq!(
            info.fml_core_plugin.as_deref(),
            Some("codechicken.core.launch.CodeChickenCoreCorePlugin")
        );
        assert!(info.fml_core_plugin_contains_mod);
        assert_eq!(info.fml_access_transformer.as_deref(), Some("cc_at.cfg"));
        assert_eq!(info.tweak_class, None, "seções por entrada não contam");
        assert!(info.is_coremod());
    }

    #[test]
    fn manifesto_moderno() {
        let info = parse(
            "\nManifest-Version: 1.0\nMixinConfigs: a.mixins.json, b.mixins.json,\nImplementation-Version: 2.4.1\nFMLModType: GAMELIBRARY\nContainedDeps: lib-a.jar lib-b.jar\nMulti-Release: true\nEmbedded-Dependencies-Mod: META-INF/jarjar/m\n od.jar\nAutomatic-Module-Name: x.y\nImplementation-Title: X\nlinha sem dois pontos\n",
        );
        assert_eq!(info.mixin_configs, ["a.mixins.json", "b.mixins.json"]);
        assert_eq!(info.implementation_version.as_deref(), Some("2.4.1"));
        assert_eq!(info.fml_mod_type.as_deref(), Some("GAMELIBRARY"));
        assert_eq!(info.contained_deps, ["lib-a.jar", "lib-b.jar"]);
        assert!(info.multi_release);
        assert_eq!(
            info.embedded_dependencies_mod.as_deref(),
            Some("META-INF/jarjar/mod.jar")
        );
        assert!(!info.is_coremod());
        assert_eq!(
            parse(" continuação órfã\nTweakClass:\n"),
            ManifestInfo::default()
        );
    }

    proptest! {
        #[test]
        fn nunca_entra_em_panico(text in ".{0,200}") {
            let _ = parse(&text);
        }
    }
}
