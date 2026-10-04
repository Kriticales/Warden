//! Versões e predicados de versão do Fabric (e do Quilt, que usa a mesma sintaxe nos casos comuns).
//!
//! Porte do comportamento do fabric-loader (<https://github.com/FabricMC/fabric-loader>, Apache-2.0,
//! commit `c75cac153757b1a75e63e901bed5bc97eff630d3`): `SemanticVersionImpl`, `StringVersion`,
//! `VersionParser`, `VersionPredicateParser` e `VersionComparisonOperator`. O código foi reescrito
//! em Rust a partir da leitura dessas classes (sem cópia literal); os casos de teste do
//! `VersionParsingTests.java` foram portados em `tests` (registro no `THIRD_PARTY.md`).
//!
//! Diferenças conhecidas: só dígitos ASCII contam como dígitos (o Java aceita outros dígitos
//! Unicode em `Integer.parseInt`), e a comparação de texto é por ponto de código (o Java compara
//! unidades UTF-16; só difere com caracteres fora do plano básico).

use std::cmp::Ordering;
use std::fmt;

/// Componente numérico de uma versão semântica, ou o curinga `x`/`X`/`*` dos predicados.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Component {
    Number(u32),
    Wildcard,
}

/// Versão semântica no dialeto do Fabric (`SemVer` com qualquer número de componentes).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SemanticVersion {
    components: Vec<Component>,
    prerelease: Option<String>,
    build: Option<String>,
}

/// Versão como o Fabric a vê: semântica quando possível, senão texto livre.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FabricVersion {
    /// Versão que segue o `SemVer` estendido do Fabric.
    Semantic(SemanticVersion),
    /// Qualquer outro texto (só pode ser comparado por igualdade nos predicados).
    Text(String),
}

/// Erro ao interpretar uma versão ou um predicado do Fabric.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{message}")]
pub struct FabricVersionError {
    message: String,
}

impl FabricVersionError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

fn is_dot_separated_id(text: &str) -> bool {
    // `|[-0-9A-Za-z]+(\.[-0-9A-Za-z]+)*`: vazio, ou partes não vazias separadas por ponto.
    text.is_empty()
        || text.split('.').all(|part| {
            !part.is_empty() && part.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
        })
}

fn is_unsigned_integer(text: &str) -> bool {
    // `0|[1-9][0-9]*`
    text == "0"
        || (text
            .chars()
            .next()
            .is_some_and(|c| ('1'..='9').contains(&c))
            && text.chars().all(|c| c.is_ascii_digit()))
}

impl SemanticVersion {
    /// Interpreta `text` como versão semântica do Fabric.
    ///
    /// Com `allow_wildcards`, aceita `x`, `X` e `*` como componentes finais (forma usada nos
    /// predicados, como `1.20.x`).
    pub fn parse(text: &str, allow_wildcards: bool) -> Result<Self, FabricVersionError> {
        let (rest, build) = match text.find('+') {
            Some(pos) => (&text[..pos], Some(text[pos + 1..].to_owned())),
            None => (text, None),
        };
        let (core, prerelease) = match rest.find('-') {
            Some(pos) => (&rest[..pos], Some(rest[pos + 1..].to_owned())),
            None => (rest, None),
        };
        if let Some(pre) = &prerelease
            && !is_dot_separated_id(pre)
        {
            return Err(FabricVersionError::new(format!(
                "pré-lançamento inválido '{pre}'"
            )));
        }
        if core.ends_with('.') {
            return Err(FabricVersionError::new(
                "componente de versão negativo ou vazio no fim",
            ));
        }
        if core.starts_with('.') {
            return Err(FabricVersionError::new(
                "falta um componente de versão no início",
            ));
        }
        let mut components = Vec::new();
        let mut first_wildcard: Option<usize> = None;
        for (i, part) in core.split('.').enumerate() {
            if allow_wildcards {
                if matches!(part, "x" | "X" | "*") {
                    if prerelease.is_some() {
                        return Err(FabricVersionError::new(
                            "versões de pré-lançamento não podem usar curingas",
                        ));
                    }
                    components.push(Component::Wildcard);
                    first_wildcard.get_or_insert(i);
                    continue;
                } else if components.last() == Some(&Component::Wildcard) {
                    return Err(FabricVersionError::new(
                        "curinga no meio da versão (1.x.2) não é permitido",
                    ));
                }
            }
            if part.trim_matches(|c: char| c <= ' ').is_empty() {
                return Err(FabricVersionError::new("componente de versão vazio"));
            }
            if !part.chars().all(|c| c.is_ascii_digit()) {
                return Err(FabricVersionError::new(format!(
                    "componente de versão não numérico '{part}'"
                )));
            }
            let value: u32 = part
                .parse()
                .ok()
                .filter(|v| i32::try_from(*v).is_ok())
                .ok_or_else(|| {
                    FabricVersionError::new(format!("componente de versão grande demais '{part}'"))
                })?;
            components.push(Component::Number(value));
        }
        if allow_wildcards
            && components.len() == 1
            && components.first() == Some(&Component::Wildcard)
        {
            return Err(FabricVersionError::new(
                "versões da forma 'x' não são permitidas",
            ));
        }
        // 1.x.x vira 1.x
        if let Some(first) = first_wildcard
            && first > 0
        {
            components.truncate(first + 1);
        }
        Ok(Self {
            components,
            prerelease,
            build,
        })
    }

    /// Componente na posição `pos`; depois do fim repete o curinga ou vale 0.
    fn component(&self, pos: usize) -> Component {
        match self.components.get(pos) {
            Some(c) => *c,
            None if self.components.last() == Some(&Component::Wildcard) => Component::Wildcard,
            None => Component::Number(0),
        }
    }

    fn has_wildcard(&self) -> bool {
        self.components.contains(&Component::Wildcard)
    }

    /// Texto normalizado (sem zeros à esquerda), como o `getFriendlyString` do Fabric.
    #[must_use]
    pub fn friendly(&self) -> String {
        let mut out = self
            .components
            .iter()
            .map(|c| match c {
                Component::Number(n) => n.to_string(),
                Component::Wildcard => "x".to_owned(),
            })
            .collect::<Vec<_>>()
            .join(".");
        if let Some(pre) = &self.prerelease {
            out.push('-');
            out.push_str(pre);
        }
        if let Some(build) = &self.build {
            out.push('+');
            out.push_str(build);
        }
        out
    }

    fn compare(&self, other: &Self) -> Ordering {
        let len = self.components.len().max(other.components.len());
        for i in 0..len {
            if let (Component::Number(a), Component::Number(b)) =
                (self.component(i), other.component(i))
            {
                let ord = a.cmp(&b);
                if ord != Ordering::Equal {
                    return ord;
                }
            }
        }
        match (&self.prerelease, &other.prerelease) {
            (None, None) => Ordering::Equal,
            (Some(a), Some(b)) => compare_prerelease(a, b),
            (Some(_), None) => {
                if other.has_wildcard() {
                    Ordering::Equal
                } else {
                    Ordering::Less
                }
            }
            (None, Some(_)) => {
                if self.has_wildcard() {
                    Ordering::Equal
                } else {
                    Ordering::Greater
                }
            }
        }
    }
}

fn compare_prerelease(a: &str, b: &str) -> Ordering {
    let mut left = a.split('.').filter(|p| !p.is_empty());
    let mut right = b.split('.').filter(|p| !p.is_empty());
    loop {
        match (left.next(), right.next()) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(x), Some(y)) => {
                match (is_unsigned_integer(x), is_unsigned_integer(y)) {
                    (true, true) => {
                        let ord = x.len().cmp(&y.len());
                        if ord != Ordering::Equal {
                            return ord;
                        }
                    }
                    (true, false) => return Ordering::Less,
                    (false, true) => return Ordering::Greater,
                    (false, false) => {}
                }
                let ord = x.cmp(y);
                if ord != Ordering::Equal {
                    return ord;
                }
            }
        }
    }
}

impl FabricVersion {
    /// Interpreta uma versão como o `VersionParser` do Fabric: semântica se der, senão texto.
    ///
    /// Erro só para texto vazio.
    pub fn parse(text: &str, allow_wildcards: bool) -> Result<Self, FabricVersionError> {
        if text.is_empty() {
            return Err(FabricVersionError::new("a versão não pode ser vazia"));
        }
        Ok(SemanticVersion::parse(text, allow_wildcards)
            .map_or_else(|_| Self::Text(text.to_owned()), Self::Semantic))
    }

    /// Texto normalizado da versão.
    #[must_use]
    pub fn friendly(&self) -> String {
        match self {
            Self::Semantic(v) => v.friendly(),
            Self::Text(t) => t.clone(),
        }
    }

    /// Ordem do Fabric: semântica com semântica pelo `SemVer` estendido; com texto, pelo texto.
    #[must_use]
    pub fn compare(&self, other: &Self) -> Ordering {
        match (self, other) {
            (Self::Semantic(a), Self::Semantic(b)) => a.compare(b),
            _ => self.friendly().cmp(&other.friendly()),
        }
    }
}

impl fmt::Display for FabricVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.friendly())
    }
}

/// Operador de comparação de um termo do predicado.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Operator {
    GreaterEqual,
    LessEqual,
    Greater,
    Less,
    Equal,
    SameToNextMinor,
    SameToNextMajor,
}

impl Operator {
    /// Na ordem do Java: o mais longo primeiro (`>=` antes de `>`).
    const ALL: [(Self, &'static str); 7] = [
        (Self::GreaterEqual, ">="),
        (Self::LessEqual, "<="),
        (Self::Greater, ">"),
        (Self::Less, "<"),
        (Self::Equal, "="),
        (Self::SameToNextMinor, "~"),
        (Self::SameToNextMajor, "^"),
    ];

    fn min_inclusive(self) -> bool {
        matches!(
            self,
            Self::GreaterEqual | Self::Equal | Self::SameToNextMinor | Self::SameToNextMajor
        )
    }

    fn max_inclusive(self) -> bool {
        matches!(self, Self::LessEqual | Self::Equal)
    }

    fn test(self, a: &FabricVersion, b: &FabricVersion) -> bool {
        match (a, b) {
            (FabricVersion::Semantic(a), FabricVersion::Semantic(b)) => self.test_semantic(a, b),
            _ if self.min_inclusive() || self.max_inclusive() => a.friendly() == b.friendly(),
            _ => false,
        }
    }

    fn test_semantic(self, a: &SemanticVersion, b: &SemanticVersion) -> bool {
        let ord = a.compare(b);
        match self {
            Self::GreaterEqual => ord != Ordering::Less,
            Self::LessEqual => ord != Ordering::Greater,
            Self::Greater => ord == Ordering::Greater,
            Self::Less => ord == Ordering::Less,
            Self::Equal => ord == Ordering::Equal,
            Self::SameToNextMinor => {
                ord != Ordering::Less
                    && a.component(0) == b.component(0)
                    && a.component(1) == b.component(1)
            }
            Self::SameToNextMajor => ord != Ordering::Less && a.component(0) == b.component(0),
        }
    }
}

/// Predicado de versão do Fabric: termos separados por espaço, todos obrigatórios (E).
///
/// Um campo de dependência com lista de predicados é um OU entre eles; isso fica a cargo de
/// [`crate::VersionRange`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersionPredicate {
    terms: Vec<(Operator, FabricVersion)>,
}

impl VersionPredicate {
    /// Interpreta um predicado como `>=0.14.21 <0.16`, `1.20.x`, `~1.2`, `*`.
    pub fn parse(predicate: &str) -> Result<Self, FabricVersionError> {
        let mut terms = Vec::new();
        if predicate.is_empty() || predicate == "*" {
            return Ok(Self { terms });
        }
        for raw in predicate.split(' ') {
            let mut term = raw.trim_matches(|c: char| c <= ' ');
            if term.is_empty() || term == "*" {
                continue;
            }
            let mut operator = Operator::Equal;
            for (op, symbol) in Operator::ALL {
                if let Some(rest) = term.strip_prefix(symbol) {
                    operator = op;
                    term = rest;
                    break;
                }
            }
            let mut version = FabricVersion::parse(term, true)?;
            match &version {
                FabricVersion::Semantic(sem) if sem.has_wildcard() => {
                    if operator != Operator::Equal {
                        return Err(FabricVersionError::new(format!(
                            "predicado inválido '{predicate}': faixas com curinga (.x) só aceitam \
                             igualdade ou nenhum operador"
                        )));
                    }
                    let count = sem.components.len();
                    let mut numbers = Vec::with_capacity(count.saturating_sub(1));
                    for c in sem.components.iter().take(count.saturating_sub(1)) {
                        match c {
                            Component::Number(n) => numbers.push(Component::Number(*n)),
                            Component::Wildcard => {
                                return Err(FabricVersionError::new(format!(
                                    "predicado inválido '{predicate}': curinga sem número antes"
                                )));
                            }
                        }
                    }
                    if numbers.is_empty() {
                        return Err(FabricVersionError::new(format!(
                            "predicado inválido '{predicate}': curinga sem número antes"
                        )));
                    }
                    let lower = SemanticVersion {
                        components: numbers.clone(),
                        prerelease: Some(String::new()),
                        build: sem.build.clone(),
                    };
                    if count <= 3 {
                        operator = if count == 2 {
                            Operator::SameToNextMajor
                        } else {
                            Operator::SameToNextMinor
                        };
                        version = FabricVersion::Semantic(lower);
                    } else {
                        // a.b.c.x vira >=a.b.c- <a.b.(c+1)-
                        terms.push((Operator::GreaterEqual, FabricVersion::Semantic(lower)));
                        if let Some(Component::Number(last)) = numbers.last_mut() {
                            *last = last.saturating_add(1);
                        }
                        version = FabricVersion::Semantic(SemanticVersion {
                            components: numbers,
                            prerelease: Some(String::new()),
                            build: None,
                        });
                        operator = Operator::Less;
                    }
                }
                FabricVersion::Semantic(_) => {}
                FabricVersion::Text(_) => {
                    if !operator.min_inclusive() && !operator.max_inclusive() {
                        return Err(FabricVersionError::new(format!(
                            "predicado inválido '{predicate}': operadores que excluem o limite \
                             exigem versão semântica"
                        )));
                    }
                    operator = Operator::Equal;
                }
            }
            terms.push((operator, version));
        }
        Ok(Self { terms })
    }

    /// `true` se a versão satisfaz todos os termos (predicado vazio aceita tudo).
    #[must_use]
    pub fn test(&self, version: &FabricVersion) -> bool {
        self.terms
            .iter()
            .all(|(op, reference)| op.test(version, reference))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn sem(text: &str) -> FabricVersion {
        FabricVersion::Semantic(SemanticVersion::parse(text, false).expect(text))
    }

    fn parses(text: &str, wildcards: bool) -> bool {
        SemanticVersion::parse(text, wildcards).is_ok()
    }

    /// Casos de `VersionParsingTests.java` do fabric-loader (Apache-2.0), na mesma ordem.
    #[test]
    fn criacao_de_versao_semantica_fabric_loader() {
        assert!(parses("0.3.5", false));
        assert!(parses("0.3.5-beta.2", false));
        assert!(parses("0.3.5-alpha.6+build.120", false));
        assert!(parses("0.3.5+build.3000", false));
        assert!(!parses("0.0.-1", false));
        assert!(!parses(&format!("0.{}.0", i64::from(i32::MAX) + 1), false));
        assert!(!parses("0.-1.0", false));
        assert!(!parses("-1.0.0", false));
        assert!(!parses("", false));
        assert!(!parses("0.0.a", false));
        assert!(!parses("0.a.0", false));
        assert!(!parses("a.0.0", false));
        assert!(!parses("x", true));
        assert!(parses("2.x", true));
        assert!(parses("2.x.x", true));
        assert!(parses("2.X", true));
        assert!(parses("2.*", true));
        assert!(!parses("2.x.1", true));
        assert!(!parses("2.*.1", true));
        assert!(!parses("2.x-alpha.1", true));
        assert!(!parses("2.*-alpha.1", true));
        assert!(!parses("*-alpha.1", true));
        assert!(!parses("2.x", false));
        assert!(!parses("2.X", false));
        assert!(!parses("2.*", false));
        // Exemplos da especificação SemVer.
        assert!(parses("1.0.0-0.3.7", false));
        assert!(parses("1.0.0-x.7.z.92", false));
        assert!(parses("1.0.0+20130313144700", false));
        assert!(parses("1.0.0-beta+exp.sha.5114f85", false));
    }

    /// Confere um predicado contra listas de versões aceitas e recusadas.
    fn check(predicate: &str, accepted: &[&str], rejected: &[&str]) {
        let p = VersionPredicate::parse(predicate).expect(predicate);
        for v in accepted {
            assert!(p.test(&sem(v)), "{predicate} deveria aceitar {v}");
        }
        for v in rejected {
            assert!(!p.test(&sem(v)), "{predicate} deveria recusar {v}");
        }
    }

    /// Casos de predicado de `VersionParsingTests.java` do fabric-loader, um bloco por caso.
    #[test]
    fn predicados_fabric_loader() {
        check(
            ">=0.3.1-beta.2 <0.4.0",
            &[
                "0.3.1-beta.2",
                "0.3.1-beta.2.1",
                "0.3.1-beta.3",
                "0.3.4+build.125",
                "0.3.7",
                "0.4.0-alpha.1",
                "0.3.4-beta.7",
                "0.3.1-beta.11",
            ],
            &["0.3.0", "0.3.1-beta.1", "0.4.0"],
        );
        check(
            ">=0.3.1-beta.2 <0.4.0-",
            &[
                "0.3.1-beta.2",
                "0.3.1-beta.2.1",
                "0.3.1-beta.3",
                "0.3.4+build.125",
                "0.3.7",
                "0.3.4-beta.7",
                "0.3.1-beta.11",
            ],
            &["0.3.0", "0.3.1-beta.1", "0.4.0-alpha.1", "0.4.0"],
        );
        check(
            ">=1.4-",
            &["1.4-beta.2", "1.4+build.125", "1.4", "1.4.2"],
            &["1.3", "1.3.5", "1.3-alpha.1"],
        );
        check(
            "<1.4",
            &["1.3", "1.3.5", "1.3-alpha.1", "1.4-beta.2"],
            &["1.4+build.125", "1.4"],
        );
        check(
            "<1.4-",
            &["1.3", "1.3.5", "1.3-alpha.1"],
            &["1.4-beta.2", "1.4+build.125", "1.4"],
        );
        check(
            ">=0.3.1-beta.8.d.10",
            &[
                "0.3.1-beta.9",
                "0.3.1-beta.11",
                "0.3.1-beta.8.e",
                "0.3.1-beta.8.d.10",
                "0.3.1-beta.9.d.5",
                "0.3.1-beta.final",
                "0.3.1-beta.-final-",
            ],
            &[
                "0.3.1-beta.7",
                "0.3.1-beta.8.d",
                "0.3.1-beta.8.a",
                "0.3.1-alpha.9",
                "0.3.1-beta.8.8",
            ],
        );
    }

    #[test]
    fn predicados_x_e_til_fabric_loader() {
        check(
            "1.3.x",
            &["1.3.0", "1.3.0-alpha.1", "1.3.99"],
            &["1.4.0", "1.2.9", "1.2.9-rc.6", "1.4.0-alpha.1", "2.0.0"],
        );
        check(
            "2.x",
            &["2.0.0", "2.0.0-alpha.1", "2.9.0-beta.2", "2.2.4"],
            &["1.99.99", "3.0.0", "3.0.0-alpha.1"],
        );
        check(
            "~1.2.3",
            &["1.2.3", "1.2.4", "1.2.4-alpha.1"],
            &["1.2.2", "1.2.3-rc.7", "1.3.0", "2.2.0"],
        );
        check(
            "~1.2",
            &["1.2.0", "1.2.1-alpha.3", "1.2.6"],
            &["1.1.9", "1.3.0", "1.2.0-rc.2", "1.3.0-alpha.3"],
        );
        check(
            "~1.2-",
            &["1.2.0", "1.2.1-alpha.3", "1.2.6", "1.2.0-rc.2"],
            &["1.1.9", "1.3.0", "1.3.0-alpha.3"],
        );
        check("~1", &["1.0.0", "1.0.4"], &["0.9.9", "1.1.5", "3.0.5"]);
        check(
            "~1.2.3-beta.2",
            &[
                "1.2.3-beta.2",
                "1.2.3-beta.2.1",
                "1.2.3-beta.3",
                "1.2.3-beta.11",
                "1.2.3-rc.7",
                "1.2.3",
                "1.2.5",
                "1.2.4-alpha.4",
            ],
            &[
                "1.3.0",
                "1.2.2",
                "1.2.3-beta.1",
                "1.2.3-beta.1.9",
                "1.2.3-alpha.4",
            ],
        );
    }

    #[test]
    fn predicados_circunflexo_fabric_loader() {
        check(
            "^1.2.3",
            &["1.2.3", "1.2.4", "1.3.0", "1.2.4-beta.2"],
            &["1.2.2", "1.2.3-beta.2", "2.0.0"],
        );
        check(
            "^0.2.3",
            &["0.2.3", "0.2.4", "0.2.8-beta.2", "0.3.0"],
            &["0.2.0", "0.2.3-rc.8", "1.2.0"],
        );
        check(
            "^1.2.3-beta.2",
            &[
                "1.2.3-beta.2",
                "1.2.3-beta.3",
                "1.2.3-rc.7",
                "1.2.3",
                "1.2.5",
                "1.3.0",
                "1.2.4-alpha.4",
            ],
            &["1.2.2", "2.0.0", "1.2.3-alpha.4"],
        );
        check(
            "^1",
            &["1.0.0", "1.2.4", "1.99.99", "1.2.4-beta.2"],
            &["0.9.6", "1.0.0-rc.5", "2.0.0", "2.0.0-beta.2"],
        );
        check(
            "^1-",
            &["1.0.0", "1.0.0-rc.5", "1.2.4", "1.99.99", "1.2.4-beta.2"],
            &["0.9.0", "0.9.0-rc.5", "2.0.0", "2.0.0-beta.2"],
        );
        check(
            "1.2.3.x",
            &["1.2.3", "1.2.3-", "1.2.3.4", "1.2.3.4.5"],
            &["1.2.2", "1.2.4", "1.2.4-", "1.2", "1.3", "1", "2"],
        );
    }

    #[test]
    fn predicados_reais_de_mods() {
        // fabric.mod.json do Sodium 0.5.13 e da Fabric API.
        check(">=0.14.21", &["0.16.10", "0.14.21"], &["0.14.20"]);
        check("~1.20.1", &["1.20.1", "1.20.2"], &["1.20", "1.21"]);
        check("1.20.x", &["1.20", "1.20.6"], &["1.21", "1.19.4"]);
        check(">=1.20 <1.20.2", &["1.20", "1.20.1"], &["1.20.2"]);
        check("*", &["0.0.1", "99"], &[]);
        check("", &["1.0"], &[]);
        check(">=17", &["17", "21"], &["16"]);
    }

    #[test]
    fn versao_em_texto_so_por_igualdade() {
        let p = VersionPredicate::parse("1.0-SNAPSHOT_abc").expect("texto");
        assert!(p.test(&FabricVersion::Text("1.0-SNAPSHOT_abc".into())));
        assert!(!p.test(&FabricVersion::Text("1.0".into())));
        // Operador inclusivo com texto vira igualdade; exclusivo é erro.
        let p = VersionPredicate::parse(">=abc").expect("inclusivo");
        assert!(p.test(&FabricVersion::Text("abc".into())));
        assert!(!p.test(&FabricVersion::Text("abd".into())));
        assert!(VersionPredicate::parse(">abc").is_err());
        assert!(VersionPredicate::parse("<abc").is_err());
        // Versão semântica contra referência em texto: só se o texto normalizado for igual.
        assert!(
            !VersionPredicate::parse("=abc")
                .expect("=")
                .test(&sem("1.0"))
        );
    }

    #[test]
    fn erros_de_predicado() {
        assert!(VersionPredicate::parse(">=").is_err(), "versão vazia");
        assert!(
            VersionPredicate::parse(">=1.x").is_err(),
            "curinga com operador"
        );
        assert!(VersionPredicate::parse("x.x").is_err(), "só curingas");
    }

    #[test]
    fn normalizacao_e_comparacao() {
        assert_eq!(sem("01.002.3").friendly(), "1.2.3");
        assert_eq!(sem("1.0.0-rc.1+build.5").to_string(), "1.0.0-rc.1+build.5");
        assert_eq!(sem("1.0").compare(&sem("1.0.0")), Ordering::Equal);
        assert_eq!(sem("1.0.0+a").compare(&sem("1.0.0+b")), Ordering::Equal);
        assert_eq!(
            FabricVersion::Text("abc".into()).compare(&sem("1.0")),
            "abc".cmp("1.0")
        );
        assert!(FabricVersion::parse("", false).is_err());
    }

    proptest! {
        #[test]
        fn nunca_entra_em_panico(text in ".{0,40}") {
            let _ = SemanticVersion::parse(&text, true);
            let _ = SemanticVersion::parse(&text, false);
            if let Ok(p) = VersionPredicate::parse(&text) {
                let _ = p.test(&FabricVersion::Text(text.clone()));
            }
        }

        #[test]
        fn ordem_reflexiva_e_antissimetrica(
            a in "[0-9]{1,3}(\\.[0-9]{1,3}){0,3}(-[a-z0-9.]{0,6})?",
            b in "[0-9]{1,3}(\\.[0-9]{1,3}){0,3}(-[a-z0-9.]{0,6})?",
        ) {
            if let (Ok(x), Ok(y)) = (FabricVersion::parse(&a, false), FabricVersion::parse(&b, false)) {
                prop_assert_eq!(x.compare(&x), Ordering::Equal);
                prop_assert_eq!(x.compare(&y), y.compare(&x).reverse());
            }
        }
    }
}
