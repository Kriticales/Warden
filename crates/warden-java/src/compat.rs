//! Tabela de compatibilidade do Java (`data/compatibility.toml`; ADR-0029).
//!
//! A tabela é embutida no binário e validada ao ser lida ([`CompatibilityTable::parse`]):
//! esquema conhecido, ids únicos, faixas com início antes do fim e sem sobreposição, majors
//! dentro de `majors`, motivo em português em toda regra. Os testes da crate garantem que a
//! tabela embutida passa; se não passar, [`CompatibilityTable::builtin`] devolve
//! [`Error::CompatibilityTable`] em vez de entrar em pânico.

use std::collections::BTreeSet;
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::policy::JavaChoiceReason;
use crate::version::{MinecraftVersion, compare_dotted};

/// A tabela que acompanha o Warden.
pub const BUILTIN_TABLE: &str = include_str!("../data/compatibility.toml");

/// Versão do formato da tabela que esta crate entende.
const SCHEMA: u32 = 1;

/// Loader do pack, como a política do Java o vê.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, specta::Type,
)]
#[serde(rename_all = "camelCase")]
pub enum LoaderKind {
    /// Sem loader.
    Vanilla,
    /// Fabric.
    Fabric,
    /// Forge.
    Forge,
    /// `NeoForge`.
    #[serde(rename = "neoforge")]
    NeoForge,
    /// Quilt.
    Quilt,
    /// `LiteLoader`.
    #[serde(rename = "liteloader")]
    LiteLoader,
}

impl From<warden_packwiz::Loader> for LoaderKind {
    fn from(loader: warden_packwiz::Loader) -> Self {
        match loader {
            warden_packwiz::Loader::Fabric => Self::Fabric,
            warden_packwiz::Loader::Forge => Self::Forge,
            warden_packwiz::Loader::NeoForge => Self::NeoForge,
            warden_packwiz::Loader::Quilt => Self::Quilt,
            warden_packwiz::Loader::LiteLoader => Self::LiteLoader,
        }
    }
}

/// Faixa de versões do Minecraft: `from` (inclusiva) até `below` (exclusiva).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MinecraftRange {
    /// Primeira versão da faixa.
    pub from: MinecraftVersion,
    /// Primeira versão fora da faixa.
    pub below: MinecraftVersion,
}

impl MinecraftRange {
    /// Se `version` está na faixa.
    #[must_use]
    pub fn contains(&self, version: &MinecraftVersion) -> bool {
        *version >= self.from && *version < self.below
    }

    /// Se as duas faixas têm alguma versão em comum.
    #[must_use]
    pub fn overlaps(&self, other: &Self) -> bool {
        self.from < other.below && other.from < self.below
    }
}

/// Regra de loader que obriga um Java (regras b e c da ARCHITECTURE §7.3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoaderRule {
    /// Identificador estável da regra.
    pub id: String,
    /// A faixa em texto curto ("até o 1.12.2").
    pub label: String,
    /// Loader a que a regra se aplica.
    pub loader: LoaderKind,
    /// Versões do Minecraft.
    pub minecraft: MinecraftRange,
    /// Só versões do loader abaixo desta (exclusiva), quando houver.
    pub loader_below: Option<String>,
    /// Major obrigatório.
    pub java: u32,
    /// Atualização máxima do major (o `312` de "8 ≤ u312").
    pub max_update: Option<u32>,
    /// Motivo, como código estável.
    pub reason: JavaChoiceReason,
    /// Motivo em português.
    pub motivo: String,
    /// De onde veio a regra.
    pub fonte: String,
}

impl LoaderRule {
    /// Se a regra vale para o pack. Uma versão de loader ilegível conta como abaixo do limite
    /// (o Java mais restrito também abre as versões novas do loader).
    #[must_use]
    pub fn applies(
        &self,
        minecraft: &MinecraftVersion,
        loader: LoaderKind,
        loader_version: Option<&str>,
    ) -> bool {
        if loader != self.loader || !self.minecraft.contains(minecraft) {
            return false;
        }
        match (&self.loader_below, loader_version) {
            (None, _) | (Some(_), None) => true,
            (Some(limit), Some(version)) => {
                compare_dotted(version, limit).is_none_or(std::cmp::Ordering::is_lt)
            }
        }
    }
}

/// Faixa de versões do Minecraft com os majors provados (regra d da ARCHITECTURE §7.3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RangeRule {
    /// Identificador estável da faixa.
    pub id: String,
    /// A faixa em texto curto ("1.17 a 1.20.4").
    pub label: String,
    /// Versões do Minecraft.
    pub minecraft: MinecraftRange,
    /// Loaders a que a faixa se restringe (vazio = todos).
    pub loaders: Vec<LoaderKind>,
    /// Majors provados, em ordem crescente.
    pub java: Vec<u32>,
    /// Motivo em português.
    pub motivo: String,
    /// De onde veio a regra.
    pub fonte: String,
}

impl RangeRule {
    /// O major mais novo provado.
    #[must_use]
    pub fn newest(&self) -> u32 {
        self.java.last().copied().unwrap_or(8)
    }
}

/// A tabela inteira, já validada.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompatibilityTable {
    /// Majors que o Warden baixa, em ordem crescente.
    pub majors: Vec<u32>,
    /// Regras de loader, na ordem do arquivo.
    pub loader_rules: Vec<LoaderRule>,
    /// Faixas, na ordem do arquivo.
    pub ranges: Vec<RangeRule>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawTable {
    schema: u32,
    majors: Vec<u32>,
    #[serde(default)]
    loader_rule: Vec<RawLoaderRule>,
    #[serde(default)]
    range: Vec<RawRange>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawLoaderRule {
    id: String,
    label: String,
    loader: LoaderKind,
    minecraft_from: String,
    minecraft_below: String,
    loader_below: Option<String>,
    java: u32,
    max_update: Option<u32>,
    reason: JavaChoiceReason,
    motivo: String,
    fonte: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawRange {
    id: String,
    label: String,
    minecraft_from: String,
    minecraft_below: String,
    #[serde(default)]
    loaders: Vec<LoaderKind>,
    java: Vec<u32>,
    motivo: String,
    fonte: String,
}

fn invalid(message: impl Into<String>) -> Error {
    Error::CompatibilityTable(message.into())
}

fn range(id: &str, from: &str, below: &str) -> Result<MinecraftRange> {
    let parse = |text: &str| {
        MinecraftVersion::parse(text)
            .ok_or_else(|| invalid(format!("{id}: versão do Minecraft ilegível {text:?}")))
    };
    let range = MinecraftRange {
        from: parse(from)?,
        below: parse(below)?,
    };
    if range.from >= range.below {
        return Err(invalid(format!(
            "{id}: minecraft_from ({from}) precisa vir antes de minecraft_below ({below})"
        )));
    }
    Ok(range)
}

fn check_text(id: &str, field: &str, text: &str) -> Result<()> {
    if text.trim().is_empty() {
        return Err(invalid(format!("{id}: {field} vazio")));
    }
    Ok(())
}

impl CompatibilityTable {
    /// A tabela embutida, lida e validada uma vez.
    pub fn builtin() -> Result<&'static Self> {
        static TABLE: OnceLock<std::result::Result<CompatibilityTable, String>> = OnceLock::new();
        TABLE
            .get_or_init(|| Self::parse(BUILTIN_TABLE).map_err(|error| error.to_string()))
            .as_ref()
            .map_err(|message| Error::CompatibilityTable(message.clone()))
    }

    /// Lê e valida uma tabela.
    #[allow(clippy::too_many_lines)] // uma validação por campo, em sequência; dividir esconde a ordem
    pub fn parse(text: &str) -> Result<Self> {
        let raw: RawTable =
            toml::from_str(text).map_err(|error| invalid(format!("TOML inválido: {error}")))?;
        if raw.schema != SCHEMA {
            return Err(invalid(format!(
                "schema {} desconhecido (esperado {SCHEMA})",
                raw.schema
            )));
        }
        if raw.majors.is_empty() {
            return Err(invalid("majors vazio"));
        }
        if raw.majors.windows(2).any(|pair| pair[0] >= pair[1]) || raw.majors[0] < 8 {
            return Err(invalid(
                "majors precisa estar em ordem crescente, sem repetição, a partir do 8",
            ));
        }
        let majors: BTreeSet<u32> = raw.majors.iter().copied().collect();
        let mut ids = BTreeSet::new();
        let mut check_id = |id: &str| {
            if id.trim().is_empty() || !ids.insert(id.to_owned()) {
                return Err(invalid(format!("id vazio ou repetido: {id:?}")));
            }
            Ok(())
        };

        let mut loader_rules = Vec::with_capacity(raw.loader_rule.len());
        for rule in raw.loader_rule {
            check_id(&rule.id)?;
            for (field, text) in [
                ("label", &rule.label),
                ("motivo", &rule.motivo),
                ("fonte", &rule.fonte),
            ] {
                check_text(&rule.id, field, text)?;
            }
            if rule.loader == LoaderKind::Vanilla {
                return Err(invalid(format!(
                    "{}: regra de loader para vanilla",
                    rule.id
                )));
            }
            if !majors.contains(&rule.java) {
                return Err(invalid(format!(
                    "{}: Java {} fora de majors",
                    rule.id, rule.java
                )));
            }
            if !rule.reason.is_loader_rule() {
                return Err(invalid(format!(
                    "{}: reason {:?} não é motivo de regra de loader",
                    rule.id, rule.reason
                )));
            }
            if let Some(limit) = &rule.loader_below
                && crate::version::loader_version_numbers(limit).is_none()
            {
                return Err(invalid(format!(
                    "{}: loader_below ilegível {limit:?}",
                    rule.id
                )));
            }
            let minecraft = range(&rule.id, &rule.minecraft_from, &rule.minecraft_below)?;
            loader_rules.push(LoaderRule {
                id: rule.id,
                label: rule.label,
                loader: rule.loader,
                minecraft,
                loader_below: rule.loader_below,
                java: rule.java,
                max_update: rule.max_update,
                reason: rule.reason,
                motivo: rule.motivo,
                fonte: rule.fonte,
            });
        }

        let mut ranges = Vec::with_capacity(raw.range.len());
        for rule in raw.range {
            check_id(&rule.id)?;
            for (field, text) in [
                ("label", &rule.label),
                ("motivo", &rule.motivo),
                ("fonte", &rule.fonte),
            ] {
                check_text(&rule.id, field, text)?;
            }
            if rule.java.is_empty() || rule.java.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(invalid(format!(
                    "{}: java precisa ter majors em ordem crescente, sem repetição",
                    rule.id
                )));
            }
            if let Some(major) = rule.java.iter().find(|major| !majors.contains(major)) {
                return Err(invalid(format!("{}: Java {major} fora de majors", rule.id)));
            }
            if rule.loaders.contains(&LoaderKind::Vanilla) && rule.loaders.len() > 1 {
                return Err(invalid(format!(
                    "{}: vanilla misturado com loaders",
                    rule.id
                )));
            }
            let minecraft = range(&rule.id, &rule.minecraft_from, &rule.minecraft_below)?;
            ranges.push(RangeRule {
                id: rule.id,
                label: rule.label,
                minecraft,
                loaders: rule.loaders,
                java: rule.java,
                motivo: rule.motivo,
                fonte: rule.fonte,
            });
        }

        let table = Self {
            majors: raw.majors,
            loader_rules,
            ranges,
        };
        table.check_overlaps()?;
        Ok(table)
    }

    /// Faixas sem sobreposição: entre as gerais; entre as de um mesmo loader; e entre as
    /// regras de um mesmo loader.
    fn check_overlaps(&self) -> Result<()> {
        let pairs =
            |count: usize| (0..count).flat_map(move |a| ((a + 1)..count).map(move |b| (a, b)));
        for (a, b) in pairs(self.ranges.len()) {
            let (left, right) = (&self.ranges[a], &self.ranges[b]);
            let same_scope = if left.loaders.is_empty() || right.loaders.is_empty() {
                left.loaders.is_empty() && right.loaders.is_empty()
            } else {
                left.loaders
                    .iter()
                    .any(|loader| right.loaders.contains(loader))
            };
            if same_scope && left.minecraft.overlaps(&right.minecraft) {
                return Err(invalid(format!(
                    "faixas sobrepostas: {} e {}",
                    left.id, right.id
                )));
            }
        }
        for (a, b) in pairs(self.loader_rules.len()) {
            let (left, right) = (&self.loader_rules[a], &self.loader_rules[b]);
            if left.loader == right.loader && left.minecraft.overlaps(&right.minecraft) {
                return Err(invalid(format!(
                    "regras de loader sobrepostas: {} e {}",
                    left.id, right.id
                )));
            }
        }
        Ok(())
    }

    /// O major mais novo que o Warden baixa.
    #[must_use]
    pub fn newest_major(&self) -> u32 {
        self.majors.last().copied().unwrap_or(8)
    }

    /// A regra de loader que vale para o pack, se houver.
    #[must_use]
    pub fn loader_rule_for(
        &self,
        minecraft: &MinecraftVersion,
        loader: LoaderKind,
        loader_version: Option<&str>,
    ) -> Option<&LoaderRule> {
        self.loader_rules
            .iter()
            .find(|rule| rule.applies(minecraft, loader, loader_version))
    }

    /// A faixa que vale para o pack: primeiro as que citam o loader, depois as gerais.
    #[must_use]
    pub fn range_for(
        &self,
        minecraft: &MinecraftVersion,
        loader: LoaderKind,
    ) -> Option<&RangeRule> {
        let in_range = |rule: &&RangeRule| rule.minecraft.contains(minecraft);
        self.ranges
            .iter()
            .filter(|rule| rule.loaders.contains(&loader))
            .find(in_range)
            .or_else(|| {
                self.ranges
                    .iter()
                    .filter(|rule| rule.loaders.is_empty())
                    .find(in_range)
            })
    }

    /// O menor major da lista que atende a uma exigência mínima (`javaVersion` do JSON). Sem
    /// nenhum, a própria exigência (a fonte pode publicá-la).
    #[must_use]
    pub fn smallest_major_at_least(&self, required: u32) -> u32 {
        self.majors
            .iter()
            .copied()
            .find(|major| *major >= required)
            .unwrap_or(required)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mc(text: &str) -> MinecraftVersion {
        MinecraftVersion::parse(text).unwrap()
    }

    #[test]
    fn tabela_embutida_e_valida() {
        let table = CompatibilityTable::builtin().unwrap();
        assert_eq!(table.majors, vec![8, 17, 21, 25]);
        assert_eq!(table.newest_major(), 25);
        assert_eq!(table.loader_rules.len(), 2);
        assert_eq!(table.ranges.len(), 4);
        for rule in &table.loader_rules {
            assert!(rule.motivo.len() > 40, "{}: motivo curto demais", rule.id);
        }
        for rule in &table.ranges {
            assert!(rule.motivo.len() > 40, "{}: motivo curto demais", rule.id);
        }
    }

    #[test]
    fn faixas_cobrem_de_1_0_ate_26_x_sem_buracos() {
        let table = CompatibilityTable::builtin().unwrap();
        let mut general: Vec<&RangeRule> = table
            .ranges
            .iter()
            .filter(|r| r.loaders.is_empty())
            .collect();
        general.sort_by(|a, b| a.minecraft.from.cmp(&b.minecraft.from));
        assert_eq!(general[0].minecraft.from, mc("1.0"));
        for pair in general.windows(2) {
            // 1.22 a 26.0 não existe: o Minecraft pulou para a numeração por ano.
            if pair[0].minecraft.below == mc("1.22") {
                assert_eq!(pair[1].minecraft.from, mc("26.0"));
            } else {
                assert_eq!(
                    pair[0].minecraft.below, pair[1].minecraft.from,
                    "{}",
                    pair[0].id
                );
            }
        }
    }

    fn table_with(extra: &str) -> Result<CompatibilityTable> {
        CompatibilityTable::parse(&format!("schema = 1\nmajors = [8, 17, 21, 25]\n{extra}"))
    }

    const RANGE_A: &str = "[[range]]\nid = \"a\"\nlabel = \"a\"\nminecraft_from = \"1.0\"\nminecraft_below = \"1.17\"\njava = [8]\nmotivo = \"m\"\nfonte = \"f\"\n";

    fn expect_invalid(extra: &str, needle: &str) {
        match table_with(extra) {
            Err(Error::CompatibilityTable(message)) => {
                assert!(message.contains(needle), "{message:?} sem {needle:?}");
            }
            other => panic!("esperado erro com {needle:?}, veio {other:?}"),
        }
    }

    #[test]
    fn recusa_faixas_sobrepostas() {
        let overlapping = RANGE_A
            .replace("id = \"a\"", "id = \"b\"")
            .replace("1.0", "1.16");
        expect_invalid(
            &format!("{RANGE_A}{overlapping}"),
            "faixas sobrepostas: a e b",
        );
    }

    #[test]
    fn faixa_de_loader_pode_cobrir_uma_geral_mas_nao_outra_do_mesmo_loader() {
        let fabric = RANGE_A
            .replace("id = \"a\"", "id = \"fabric\"")
            .replace("java = [8]", "loaders = [\"fabric\"]\njava = [8, 17]");
        assert!(table_with(&format!("{RANGE_A}{fabric}")).is_ok());
        let fabric_quilt = fabric
            .replace("id = \"fabric\"", "id = \"fq\"")
            .replace("[\"fabric\"]", "[\"quilt\", \"fabric\"]");
        expect_invalid(
            &format!("{RANGE_A}{fabric}{fabric_quilt}"),
            "faixas sobrepostas: fabric e fq",
        );
    }

    #[test]
    fn recusa_major_fora_da_lista_e_fora_de_ordem() {
        expect_invalid(
            &RANGE_A.replace("java = [8]", "java = [16]"),
            "Java 16 fora de majors",
        );
        expect_invalid(
            &RANGE_A.replace("java = [8]", "java = [17, 8]"),
            "ordem crescente",
        );
        expect_invalid(
            &RANGE_A.replace("java = [8]", "java = []"),
            "ordem crescente",
        );
        assert!(matches!(
            CompatibilityTable::parse("schema = 1\nmajors = [17, 8]\n"),
            Err(Error::CompatibilityTable(_))
        ));
        assert!(matches!(
            CompatibilityTable::parse("schema = 1\nmajors = []\n"),
            Err(Error::CompatibilityTable(_))
        ));
        assert!(matches!(
            CompatibilityTable::parse("schema = 2\nmajors = [8]\n"),
            Err(Error::CompatibilityTable(_))
        ));
    }

    #[test]
    fn recusa_faixa_invertida_versao_ilegivel_e_texto_vazio() {
        expect_invalid(&RANGE_A.replace("1.17", "1.0"), "precisa vir antes");
        expect_invalid(&RANGE_A.replace("1.17", "24w14a"), "ilegível");
        expect_invalid(
            &RANGE_A.replace("motivo = \"m\"", "motivo = \" \""),
            "motivo vazio",
        );
        expect_invalid(&format!("{RANGE_A}{RANGE_A}"), "repetido");
        expect_invalid(&RANGE_A.replace("fonte", "fontes"), "TOML inválido");
    }

    const LOADER_RULE: &str = "[[loader_rule]]\nid = \"r\"\nlabel = \"r\"\nloader = \"forge\"\nminecraft_from = \"1.0\"\nminecraft_below = \"1.13\"\njava = 8\nreason = \"FORGE_LEGACY_JAVA8\"\nmotivo = \"m\"\nfonte = \"f\"\n";

    #[test]
    fn regras_de_loader_validadas() {
        assert!(table_with(LOADER_RULE).is_ok());
        expect_invalid(
            &LOADER_RULE.replace("java = 8", "java = 11"),
            "Java 11 fora de majors",
        );
        expect_invalid(
            &LOADER_RULE.replace("FORGE_LEGACY_JAVA8", "NEWEST_AVAILABLE"),
            "não é motivo de regra de loader",
        );
        expect_invalid(&LOADER_RULE.replace("\"forge\"", "\"vanilla\""), "vanilla");
        expect_invalid(
            &LOADER_RULE.replace("java = 8", "java = 8\nloader_below = \"x\""),
            "loader_below ilegível",
        );
        let second = LOADER_RULE
            .replace("id = \"r\"", "id = \"s\"")
            .replace("1.0", "1.12");
        expect_invalid(
            &format!("{LOADER_RULE}{second}"),
            "regras de loader sobrepostas",
        );
    }

    #[test]
    fn regra_de_loader_com_limite_de_versao() {
        let table = CompatibilityTable::builtin().unwrap();
        let rule = table
            .loader_rule_for(&mc("1.16.5"), LoaderKind::Forge, Some("36.2.25"))
            .unwrap();
        assert_eq!(rule.id, "forge-1.16.5-anterior-ao-36.2.26");
        assert!(
            table
                .loader_rule_for(&mc("1.16.5"), LoaderKind::Forge, Some("36.2.26"))
                .is_none()
        );
        assert!(
            table
                .loader_rule_for(&mc("1.16.5"), LoaderKind::Forge, Some("1.16.5-36.2.34"))
                .is_none()
        );
        // Versão do loader desconhecida: o Java 8 até a u312 abre qualquer Forge 1.16.5.
        assert!(
            table
                .loader_rule_for(&mc("1.16.5"), LoaderKind::Forge, None)
                .is_some()
        );
        assert!(
            table
                .loader_rule_for(&mc("1.16.5"), LoaderKind::Fabric, Some("36.2.25"))
                .is_none()
        );
    }

    #[test]
    fn faixa_de_loader_vale_antes_da_geral() {
        let fabric = RANGE_A
            .replace("id = \"a\"", "id = \"fabric\"")
            .replace("java = [8]", "loaders = [\"fabric\"]\njava = [8, 17]");
        let table = table_with(&format!("{RANGE_A}{fabric}")).unwrap();
        assert_eq!(
            table
                .range_for(&mc("1.12.2"), LoaderKind::Fabric)
                .unwrap()
                .id,
            "fabric"
        );
        assert_eq!(
            table
                .range_for(&mc("1.12.2"), LoaderKind::Forge)
                .unwrap()
                .id,
            "a"
        );
        assert_eq!(
            table
                .range_for(&mc("1.12.2"), LoaderKind::Fabric)
                .unwrap()
                .newest(),
            17
        );
        assert!(table.range_for(&mc("1.17"), LoaderKind::Forge).is_none());
    }

    #[test]
    fn menor_major_que_atende() {
        let table = CompatibilityTable::builtin().unwrap();
        assert_eq!(table.smallest_major_at_least(16), 17);
        assert_eq!(table.smallest_major_at_least(17), 17);
        assert_eq!(table.smallest_major_at_least(22), 25);
        assert_eq!(table.smallest_major_at_least(27), 27);
    }

    #[test]
    fn loader_do_packwiz() {
        assert_eq!(
            LoaderKind::from(warden_packwiz::Loader::NeoForge),
            LoaderKind::NeoForge
        );
        assert_eq!(
            serde_json::to_value(LoaderKind::NeoForge).unwrap(),
            serde_json::json!("neoforge")
        );
    }
}
