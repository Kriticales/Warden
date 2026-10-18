//! Da falha de uma rodada ao [`Verdict`] (ARCHITECTURE §9.7 item 8; CA-T25-02 e CA-T25-07).
//!
//! - Java errado: [`Verdict::Environment`] (a busca para: não é mod).
//! - Classe ou mod de um item **desligado** que um item ligado procurou
//!   (`NoClassDefFoundError`/`ClassNotFoundException`, `Mod X requires Y`, a lista do NeoForge
//!   e as linhas do Fabric): [`Verdict::Inferred`] ("o mod X usa Y sem declarar a dependência"),
//!   e a rodada é refeita com a ordem nova.
//! - Mesma assinatura do problema: [`Verdict::FailedSame`]; outra: [`Verdict::FailedOther`]
//!   ("travou diferente", não conta). A assinatura é a da rodada (âncora + exceção) ou a da D-06
//!   (`warden_diagnostics::signature`, calculada pelo app com o log e o crash report da rodada):
//!   qualquer uma das duas igual basta.
//! - Na rodada 0 ainda não há assinatura: a primeira falha no momento escolhido pelo usuário
//!   ("trava ao abrir", "ao entrar no mundo", a linha do log) vira o alvo.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::search::Verdict;
use crate::watch::{Anchor, Failure, Stage};

/// O que o usuário diz que acontece.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Symptom {
    /// Trava ao abrir o jogo (antes do menu).
    CrashOnOpen,
    /// Trava ao entrar no mundo.
    CrashOnWorld,
    /// Aparece uma mensagem no log.
    LogLine {
        /// O trecho procurado.
        text: String,
    },
    /// Outra coisa: o usuário diz se aconteceu (modo assistido).
    Assisted,
}

/// A assinatura do problema (fixada na rodada 0).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct Target {
    /// Assinatura da rodada (âncora + exceção).
    pub signature: Option<String>,
    /// Hash da assinatura da D-06, quando calculado.
    pub crash_hash: Option<String>,
}

impl Target {
    /// Se a falha tem a mesma assinatura.
    #[must_use]
    pub fn matches(&self, failure: &Failure, crash_hash: Option<&str>) -> bool {
        self.signature.as_deref() == Some(failure.signature.as_str())
            || (self.crash_hash.is_some() && self.crash_hash.as_deref() == crash_hash)
    }
}

/// Onde achar os itens do pack pelo id do mod e pela classe.
pub trait PackLookup {
    /// O item (caminho) que traz o mod com o id.
    fn item_of_mod(&self, mod_id: &str) -> Option<String>;
    /// O item (caminho) que traz a classe (`a.b.C`).
    fn item_of_class(&self, class: &str) -> Option<String>;
}

/// A dependência não declarada que explica a falha, se houver: um item ligado procurou
/// classe ou mod de um item desligado.
#[must_use]
pub fn inferred(
    failure: &Failure,
    enabled: &BTreeSet<String>,
    lookup: &dyn PackLookup,
) -> Option<(String, String)> {
    for missing in &failure.missing_classes {
        let Some(to) = lookup.item_of_class(&missing.class) else {
            continue;
        };
        if enabled.contains(&to) {
            continue;
        }
        let Some(from) = missing
            .needed_by
            .as_deref()
            .and_then(|id| lookup.item_of_mod(id))
        else {
            continue;
        };
        if enabled.contains(&from) && from != to {
            return Some((from, to));
        }
    }
    for hint in &failure.hints {
        let (Some(from), Some(to)) = (lookup.item_of_mod(&hint.from), lookup.item_of_mod(&hint.to))
        else {
            continue;
        };
        if enabled.contains(&from) && !enabled.contains(&to) && from != to {
            return Some((from, to));
        }
    }
    None
}

/// Se a falha é a que o usuário descreveu (rodada 0, sem alvo ainda).
#[must_use]
pub fn fits_symptom(symptom: &Symptom, failure: &Failure) -> bool {
    match symptom {
        Symptom::CrashOnOpen => failure.stage == Stage::Opening,
        Symptom::CrashOnWorld => failure.stage == Stage::World,
        Symptom::LogLine { .. } => failure.anchor == Anchor::LogLine,
        Symptom::Assisted => true,
    }
}

/// O veredito de uma rodada que falhou. `target` vazio = rodada 0 (a falha certa vira o
/// alvo, que é preenchido aqui).
pub fn judge_failure(
    failure: &Failure,
    crash_hash: Option<&str>,
    symptom: &Symptom,
    target: &mut Target,
    enabled: &BTreeSet<String>,
    lookup: &dyn PackLookup,
) -> Verdict {
    if failure.anchor.is_environment() {
        return Verdict::Environment;
    }
    if let Some((from, to)) = inferred(failure, enabled, lookup) {
        return Verdict::Inferred { from, to };
    }
    // Modo assistido: o jogo travou sozinho, a rodada é marcada sem perguntar.
    if *symptom == Symptom::Assisted {
        return Verdict::FailedSame;
    }
    if target.signature.is_none() && target.crash_hash.is_none() {
        if !fits_symptom(symptom, failure) {
            return Verdict::FailedOther;
        }
        target.signature = Some(failure.signature.clone());
        target.crash_hash = crash_hash.map(str::to_owned);
        return Verdict::FailedSame;
    }
    if target.matches(failure, crash_hash) {
        Verdict::FailedSame
    } else {
        Verdict::FailedOther
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;
    use crate::watch::{DependencyHint, MissingClass};

    struct Lookup {
        mods: HashMap<&'static str, &'static str>,
        packages: HashMap<&'static str, &'static str>,
    }

    impl PackLookup for Lookup {
        fn item_of_mod(&self, mod_id: &str) -> Option<String> {
            self.mods.get(mod_id).map(|&path| path.to_owned())
        }

        fn item_of_class(&self, class: &str) -> Option<String> {
            self.packages
                .iter()
                .find(|(package, _)| class.starts_with(*package))
                .map(|(_, &path)| path.to_owned())
        }
    }

    fn lookup() -> Lookup {
        Lookup {
            mods: HashMap::from([
                ("supplementaries", "mods/supplementaries.pw.toml"),
                ("moonlight", "mods/moonlight.pw.toml"),
            ]),
            packages: HashMap::from([("net.mehvahdjukaar.moonlight.", "mods/moonlight.pw.toml")]),
        }
    }

    fn failure(anchor: Anchor, signature: &str) -> Failure {
        Failure {
            anchor,
            stage: Stage::World,
            signature: signature.into(),
            at_ms: 0,
            hints: Vec::new(),
            missing_classes: Vec::new(),
        }
    }

    fn enabled(paths: &[&str]) -> BTreeSet<String> {
        paths.iter().map(|&path| path.to_owned()).collect()
    }

    #[test]
    fn ca_t25_02_classe_de_mod_desligado_vira_dependencia_inferida() {
        let mut failure = failure(Anchor::LoadingError, "carregamento | x");
        failure.missing_classes.push(MissingClass {
            class: "net.mehvahdjukaar.moonlight.api.platform.ClientHelper$EntityLayerEvent".into(),
            needed_by: Some("supplementaries".into()),
        });
        let mut target = Target {
            signature: Some("entrar | y".into()),
            crash_hash: None,
        };
        let verdict = judge_failure(
            &failure,
            None,
            &Symptom::CrashOnWorld,
            &mut target,
            &enabled(&["mods/supplementaries.pw.toml"]),
            &lookup(),
        );
        assert_eq!(
            verdict,
            Verdict::Inferred {
                from: "mods/supplementaries.pw.toml".into(),
                to: "mods/moonlight.pw.toml".into(),
            }
        );
        // Com o dono da classe ligado, não há o que inferir: é "travou diferente".
        let verdict = judge_failure(
            &failure,
            None,
            &Symptom::CrashOnWorld,
            &mut target,
            &enabled(&["mods/supplementaries.pw.toml", "mods/moonlight.pw.toml"]),
            &lookup(),
        );
        assert_eq!(verdict, Verdict::FailedOther);
    }

    #[test]
    fn dica_do_log_tambem_infere() {
        let mut failure = failure(Anchor::MissingDependency, "dependencia | a>b");
        failure.hints.push(DependencyHint {
            from: "supplementaries".into(),
            to: "moonlight".into(),
        });
        let found = inferred(
            &failure,
            &enabled(&["mods/supplementaries.pw.toml"]),
            &lookup(),
        );
        assert_eq!(
            found,
            Some((
                "mods/supplementaries.pw.toml".into(),
                "mods/moonlight.pw.toml".into()
            ))
        );
    }

    #[test]
    fn ca_t25_02_outra_assinatura_nao_conta_e_ambiente_para() {
        let mut target = Target::default();
        let set = enabled(&[]);
        // Rodada 0: a falha no momento certo vira o alvo.
        let first = failure(
            Anchor::JoinWorld,
            "entrar | java.lang.IllegalStateException: x",
        );
        assert_eq!(
            judge_failure(
                &first,
                Some("h1"),
                &Symptom::CrashOnWorld,
                &mut target,
                &set,
                &lookup()
            ),
            Verdict::FailedSame
        );
        assert_eq!(target.crash_hash.as_deref(), Some("h1"));
        let other = failure(
            Anchor::CrashReport,
            "crash | java.lang.NullPointerException",
        );
        assert_eq!(
            judge_failure(
                &other,
                Some("h2"),
                &Symptom::CrashOnWorld,
                &mut target,
                &set,
                &lookup()
            ),
            Verdict::FailedOther
        );
        // A mesma assinatura da D-06 basta, mesmo com outra âncora.
        assert_eq!(
            judge_failure(
                &other,
                Some("h1"),
                &Symptom::CrashOnWorld,
                &mut target,
                &set,
                &lookup()
            ),
            Verdict::FailedSame
        );
        let java = failure(Anchor::JavaVersion, "java | x");
        assert_eq!(
            judge_failure(
                &java,
                None,
                &Symptom::CrashOnWorld,
                &mut target,
                &set,
                &lookup()
            ),
            Verdict::Environment
        );
    }

    #[test]
    fn rodada_0_no_momento_errado_nao_vira_alvo() {
        let mut target = Target::default();
        let mut opening = failure(Anchor::CrashReport, "crash | x");
        opening.stage = Stage::Opening;
        let verdict = judge_failure(
            &opening,
            None,
            &Symptom::CrashOnWorld,
            &mut target,
            &enabled(&[]),
            &lookup(),
        );
        assert_eq!(verdict, Verdict::FailedOther);
        assert_eq!(target, Target::default());
        assert!(fits_symptom(&Symptom::CrashOnOpen, &opening));
        let line = failure(Anchor::LogLine, "linha | x");
        assert!(fits_symptom(&Symptom::LogLine { text: "x".into() }, &line));
        // Modo assistido: travou sozinho = marcado sem perguntar.
        assert_eq!(
            judge_failure(
                &opening,
                None,
                &Symptom::Assisted,
                &mut target,
                &enabled(&[]),
                &lookup()
            ),
            Verdict::FailedSame
        );
    }
}
