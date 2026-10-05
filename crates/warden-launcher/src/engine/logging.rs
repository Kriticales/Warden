//! Qual configuração do log4j o jogo recebe (ARCHITECTURE §7.4; R2 §1.7 e §3.1).
//!
//! O portablemc passa sempre o `logging.client` do vanilla: ao regravar o JSON do Forge ele
//! descarta o `"logging": {}` com que o Forge anula o herdado (verificado na L-02: os JSON do
//! Forge 1.12.2 e 1.16.5 em `versions/` saem sem a chave). Por isso a regra é pelo loader e
//! pela versão, não pelo arquivo:
//!
//! - NeoForge: nunca a configuração da Mojang (o FML só grava o `debug.log` sem ela; o
//!   NeoForge só existe com log4j corrigido).
//! - Forge: a configuração do próprio Forge (`log4j2.xml` com `%msg{nolookups}`) quando o
//!   jogo já tem log4j corrigido (1.19+) ou quando a versão do Forge está na lista
//!   verificada ([`VERIFIED_FORGE`]); senão fica a da Mojang, que é segura (Forge 1.7.10 e
//!   Forge antigos). Nunca uma versão até a 1.18 sem configuração segura.
//! - Os demais (vanilla, Fabric, Quilt): a da Mojang.

use warden_java::compare_dotted;

use crate::range::needs_safe_log4j_config;
use crate::spec::{LoaderSpec, LoggingConfig};

/// Prefixos do argumento de configuração do log4j nos perfis.
const CONFIG_ARGS: [&str; 2] = ["-Dlog4j.configurationFile=", "-Dlog4j2.configurationFile="];

/// Forge até a 1.18 cujo `log4j2.xml` próprio foi verificado como seguro (`%msg{nolookups}`;
/// R2 §3.1 e a matriz da L-05): a versão mínima de cada linha do Minecraft.
pub(crate) const VERIFIED_FORGE: [(&str, &str); 3] = [
    ("1.12.2", "14.23.5.2860"),
    ("1.16.5", "36.2.34"),
    ("1.18.2", "40.3.0"),
];

/// Se a versão do Forge está na lista verificada.
fn forge_verified(minecraft: &str, forge: &str) -> bool {
    VERIFIED_FORGE.iter().any(|(line, minimum)| {
        *line == minecraft
            && matches!(
                compare_dotted(forge, minimum),
                Some(std::cmp::Ordering::Equal | std::cmp::Ordering::Greater)
            )
    })
}

/// A decisão: a configuração e se o argumento do motor deve sair.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LoggingDecision {
    /// O que o jogo vai usar.
    pub(crate) config: LoggingConfig,
    /// Tirar o `-Dlog4j.configurationFile` que o motor pôs.
    pub(crate) strip_engine_arg: bool,
}

/// Decide a configuração. `release_time` é a do vanilla; `jvm_args`, os do motor.
pub(crate) fn decide(
    loader: &LoaderSpec,
    minecraft: &str,
    release_time: &str,
    jvm_args: &[String],
) -> LoggingDecision {
    let engine_file = engine_config_file(jvm_args);
    let use_loader = LoggingDecision {
        config: LoggingConfig::Loader,
        strip_engine_arg: engine_file.is_some(),
    };
    let own_config = match loader {
        LoaderSpec::NeoForge { .. } => true,
        LoaderSpec::Forge { version } => {
            !needs_safe_log4j_config(release_time) || forge_verified(minecraft, version)
        }
        _ => false,
    };
    if own_config {
        return use_loader;
    }
    LoggingDecision {
        config: engine_file.map_or(LoggingConfig::None, |file| LoggingConfig::Mojang { file }),
        strip_engine_arg: false,
    }
}

/// O nome do arquivo de configuração que o motor pôs nos argumentos.
fn engine_config_file(jvm_args: &[String]) -> Option<String> {
    jvm_args.iter().find_map(|arg| {
        let path = CONFIG_ARGS
            .iter()
            .find_map(|prefix| arg.strip_prefix(prefix))?;
        let name = path.rsplit(['/', '\\']).next().unwrap_or(path);
        Some(name.to_owned())
    })
}

/// Tira os argumentos de configuração do log4j.
pub(crate) fn strip_config_args(jvm_args: &mut Vec<String>) {
    jvm_args.retain(|arg| !CONFIG_ARGS.iter().any(|prefix| arg.starts_with(prefix)));
}

#[cfg(test)]
mod tests {
    use super::*;

    const T1_7_10: &str = "2014-05-14T17:29:23+00:00";
    const T1_12_2: &str = "2017-09-18T08:39:46+00:00";
    const T1_16_5: &str = "2021-01-14T16:05:32+00:00";
    const T1_20_1: &str = "2023-06-12T13:25:51+00:00";

    fn args(file: &str) -> Vec<String> {
        vec![
            "-cp".into(),
            "x.jar".into(),
            format!("-Dlog4j.configurationFile=C:\\shared\\assets\\log_configs\\{file}"),
        ]
    }

    fn forge(version: &str) -> LoaderSpec {
        LoaderSpec::Forge {
            version: version.into(),
        }
    }

    #[test]
    fn fabric_e_vanilla_usam_o_xml_da_mojang() {
        let decision = decide(
            &LoaderSpec::Fabric {
                version: "0.19.5".into(),
            },
            "1.20.1",
            T1_20_1,
            &args("client-1.12.xml"),
        );
        assert_eq!(
            decision.config,
            LoggingConfig::Mojang {
                file: "client-1.12.xml".into()
            }
        );
        assert!(!decision.strip_engine_arg);
    }

    #[test]
    fn forge_verificado_usa_o_proprio() {
        for (minecraft, time, version) in [
            ("1.12.2", T1_12_2, "14.23.5.2860"),
            ("1.16.5", T1_16_5, "36.2.34"),
            ("1.20.1", T1_20_1, "47.4.10"),
        ] {
            let decision = decide(&forge(version), minecraft, time, &args("client-1.12.xml"));
            assert_eq!(decision.config, LoggingConfig::Loader, "{minecraft}");
            assert!(decision.strip_engine_arg);
        }
    }

    #[test]
    fn forge_antigo_nao_verificado_fica_com_o_da_mojang() {
        // Forge 1.12.2 de antes da correção do Log4Shell.
        let decision = decide(
            &forge("14.23.5.2847"),
            "1.12.2",
            T1_12_2,
            &args("client-1.12.xml"),
        );
        assert_eq!(
            decision.config,
            LoggingConfig::Mojang {
                file: "client-1.12.xml".into()
            }
        );
        assert!(!decision.strip_engine_arg);
    }

    #[test]
    fn forge_1_7_10_fica_com_o_xml_corrigido_da_mojang() {
        let decision = decide(
            &forge("10.13.4.1614"),
            "1.7.10",
            T1_7_10,
            &args("client-1.7.xml"),
        );
        assert_eq!(
            decision.config,
            LoggingConfig::Mojang {
                file: "client-1.7.xml".into()
            }
        );
    }

    #[test]
    fn neoforge_nunca_recebe_o_xml_da_mojang() {
        let decision = decide(
            &LoaderSpec::NeoForge {
                version: "21.1.252".into(),
            },
            "1.21.1",
            T1_20_1,
            &args("client-1.12.xml"),
        );
        assert_eq!(decision.config, LoggingConfig::Loader);
        assert!(decision.strip_engine_arg);
        let mut jvm = args("client-1.12.xml");
        strip_config_args(&mut jvm);
        assert_eq!(jvm, vec!["-cp".to_owned(), "x.jar".to_owned()]);
    }

    #[test]
    fn nome_do_arquivo_com_barras_dos_dois_sistemas() {
        assert_eq!(
            engine_config_file(&["-Dlog4j.configurationFile=/a/b/client-1.7.xml".into()]),
            Some("client-1.7.xml".into())
        );
        assert_eq!(engine_config_file(&["-Xmx1G".into()]), None);
    }
}
