//! Bibliotecas conhecidas: mods que existem para outros mods usarem.
//!
//! Serve só quando não há histórico de adições: um mod que nenhum outro exige é, em geral, um
//! mod que o usuário escolheu; uma biblioteca que ninguém exige é sobra. A lista é uma
//! heurística (ids em minúsculas, como o `modid` declarado no jar) e fica curta de propósito:
//! o jar que se declara `LIBRARY` (`FMLModType`) já entra pelo descritor.

/// Ids de bibliotecas conhecidas, em minúsculas e em ordem alfabética.
pub const KNOWN_LIBRARIES: &[&str] = &[
    "architectury",
    "azurelib",
    "balm",
    "bookshelf",
    "cardinal-components-base",
    "citadel",
    "cloth-config",
    "cloth_config",
    "collective",
    "creativecore",
    "curios",
    "fabric",
    "fabric-api",
    "fabric-language-kotlin",
    "forge_config_api_port",
    "forgeconfigapiport",
    "geckolib",
    "geckolib3",
    "glitchcore",
    "kotlinforforge",
    "midnightlib",
    "owo",
    "player-animation-lib",
    "playeranimator",
    "puzzleslib",
    "resourcefullib",
    "supermartijn642configlib",
    "supermartijn642corelib",
    "terrablender",
    "trinkets",
    "yungsapi",
];

/// `true` se o id (sem diferenciar maiúsculas) é de uma biblioteca conhecida.
#[must_use]
pub fn is_known_library(id: &str) -> bool {
    let id = id.to_ascii_lowercase();
    KNOWN_LIBRARIES.binary_search(&id.as_str()).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lista_ordenada_sem_repeticao_e_em_minusculas() {
        for pair in KNOWN_LIBRARIES.windows(2) {
            assert!(pair[0] < pair[1], "{} antes de {}", pair[0], pair[1]);
        }
        for id in KNOWN_LIBRARIES {
            assert_eq!(*id, id.to_ascii_lowercase());
        }
    }

    #[test]
    fn reconhece_sem_diferenciar_maiusculas() {
        assert!(is_known_library("Fabric-API"));
        assert!(is_known_library("geckolib"));
        assert!(!is_known_library("create"));
    }
}
