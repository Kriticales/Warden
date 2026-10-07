//! Kits de desempenho (SPEC T03 e T08; ADR-0033; R1 §2.3.5).
//!
//! Os dados ficam em `data/kits.toml` (embutido no binário): cada kit vale para um loader e uma
//! faixa de versões do Minecraft e lista os mods com o lado de cada um. O kit nunca entra em
//! silêncio: a tela mostra os mods com caixas marcadas e, fora do assistente, passa pelo diálogo
//! de dependências (`add_plan` e `add_apply` com vários itens).

use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

use crate::initial_mods::{DataSide, DataSource, KitChoice, compare_versions, version_in_range};
use crate::search::SourceId;
use crate::side::SideChoice;
use crate::{Error, ProjectErrorCode as Code, Result};

const DATA: &str = include_str!("../data/kits.toml");

/// Um mod de um kit.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct KitItemData {
    /// Fonte.
    pub source: DataSource,
    /// ID do projeto.
    pub project: String,
    /// Nome (para a lista e para o relatório do `check-kits`).
    pub name: String,
    /// Lado.
    #[serde(default)]
    pub side: Option<DataSide>,
    /// Vem marcado (padrão: sim).
    #[serde(default = "yes")]
    pub checked: bool,
    /// Código da observação (`worldgen`, `shaders`), traduzido pela interface.
    #[serde(default)]
    pub note: Option<String>,
}

fn yes() -> bool {
    true
}

/// Um kit nos dados.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct KitData {
    /// Identificador estável.
    pub id: String,
    /// Nome.
    pub name: String,
    /// Frase curta.
    pub description: String,
    /// Loader (`forge`, `neoforge`, `fabric`).
    pub loader: String,
    /// Primeira versão do Minecraft (inclusiva).
    pub from: String,
    /// Última versão do Minecraft (inclusiva).
    #[serde(default)]
    pub to: Option<String>,
    /// Versões que o `check-kits` consulta.
    #[serde(default)]
    pub check: Vec<String>,
    /// Mods.
    pub item: Vec<KitItemData>,
}

impl KitData {
    /// Se o kit vale para o loader e a versão.
    #[must_use]
    pub fn matches(&self, loader: &str, minecraft: &str) -> bool {
        self.loader == loader && version_in_range(minecraft, Some(&self.from), self.to.as_deref())
    }
}

/// O arquivo `kits.toml`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct KitsData {
    /// Versão do formato.
    pub schema: u32,
    /// Kits.
    pub kit: Vec<KitData>,
}

impl KitsData {
    /// Lê um texto no formato de `kits.toml` e confere as regras. Erro [`Code::Internal`].
    pub fn parse(text: &str) -> Result<Self> {
        let data: Self = toml::from_str(text)
            .map_err(|e| Error::new(Code::Internal, format!("kits.toml: {e}")))?;
        data.validate()?;
        Ok(data)
    }

    fn validate(&self) -> Result<()> {
        let bad = |reason: String| Error::new(Code::Internal, format!("kits.toml: {reason}"));
        if self.schema != 1 {
            return Err(bad(format!("schema {} não é conhecido", self.schema)));
        }
        let mut ids = std::collections::BTreeSet::new();
        for kit in &self.kit {
            if !ids.insert(kit.id.as_str()) {
                return Err(bad(format!("kit {} repetido", kit.id)));
            }
            if !matches!(kit.loader.as_str(), "forge" | "neoforge" | "fabric") {
                return Err(bad(format!(
                    "{}: loader {} desconhecido",
                    kit.id, kit.loader
                )));
            }
            if kit.item.is_empty() || kit.name.trim().is_empty() {
                return Err(bad(format!("{}: sem nome ou sem mods", kit.id)));
            }
            for version in std::iter::once(&kit.from)
                .chain(kit.to.iter())
                .chain(&kit.check)
            {
                if compare_versions(version, version).is_none() {
                    return Err(bad(format!("{}: versão {version} inválida", kit.id)));
                }
            }
            if let Some(to) = &kit.to
                && compare_versions(&kit.from, to) == Some(std::cmp::Ordering::Greater)
            {
                return Err(bad(format!("{}: faixa ao contrário", kit.id)));
            }
            let mut projects = std::collections::BTreeSet::new();
            for item in &kit.item {
                let valid = match item.source {
                    DataSource::Modrinth => {
                        item.project.len() == 8
                            && item.project.chars().all(|c| c.is_ascii_alphanumeric())
                    }
                    DataSource::Curseforge => {
                        !item.project.is_empty() && item.project.chars().all(|c| c.is_ascii_digit())
                    }
                };
                if !valid {
                    return Err(bad(format!(
                        "{}: ID inválido em {}: {}",
                        kit.id, item.name, item.project
                    )));
                }
                if !projects.insert((item.source == DataSource::Modrinth, item.project.as_str())) {
                    return Err(bad(format!("{}: {} repetido", kit.id, item.name)));
                }
            }
        }
        Ok(())
    }
}

/// Os kits embutidos no binário.
///
/// # Panics
///
/// Nunca com os dados do repositório: um teste confere o arquivo.
#[must_use]
pub fn embedded() -> &'static KitsData {
    static CELL: OnceLock<KitsData> = OnceLock::new();
    CELL.get_or_init(|| {
        KitsData::parse(DATA).unwrap_or_else(|error| {
            // Os dados são do repositório e conferidos por teste; chegar aqui é um bug.
            unreachable!("dados embutidos inválidos: {error}")
        })
    })
}

/// Um mod de um kit, como a interface o mostra.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct KitItem {
    /// Fonte.
    pub source: SourceId,
    /// ID do projeto.
    pub project_id: String,
    /// Nome.
    pub name: String,
    /// Lado.
    pub side: Option<SideChoice>,
    /// Vem marcado.
    pub checked: bool,
    /// Código da observação (`worldgen`, `shaders`).
    pub note: Option<String>,
}

/// Um kit oferecido ao pack.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct Kit {
    /// Identificador estável.
    pub id: String,
    /// Nome.
    pub name: String,
    /// Frase curta.
    pub description: String,
    /// Mods.
    pub items: Vec<KitItem>,
}

/// Os kits que servem ao loader e à versão do Minecraft (vanilla ou sem kit: lista vazia).
#[must_use]
pub fn kits_for(minecraft: &str, loader: Option<&str>) -> Vec<Kit> {
    let Some(loader) = loader else {
        return Vec::new();
    };
    embedded()
        .kit
        .iter()
        .filter(|kit| kit.matches(loader, minecraft))
        .map(|kit| Kit {
            id: kit.id.clone(),
            name: kit.name.clone(),
            description: kit.description.clone(),
            items: kit
                .item
                .iter()
                .map(|item| KitItem {
                    source: item.source.source(),
                    project_id: item.project.clone(),
                    name: item.name.clone(),
                    side: item.side.map(DataSide::choice),
                    checked: item.checked,
                    note: item.note.clone(),
                })
                .collect(),
        })
        .collect()
}

/// Um item a pedir ao plano.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KitWanted {
    /// Fonte.
    pub source: SourceId,
    /// ID do projeto.
    pub project: String,
    /// Nome.
    pub name: String,
    /// Lado.
    pub side: Option<SideChoice>,
}

/// Os itens do kit escolhido que continuaram marcados. Erro [`Code::InvalidInput`] se o kit não
/// existe ou não serve ao pack, ou se a seleção traz um projeto que não é do kit.
pub fn wanted_for(minecraft: &str, loader: &str, choice: &KitChoice) -> Result<Vec<KitWanted>> {
    let kit = embedded()
        .kit
        .iter()
        .find(|kit| kit.id == choice.id && kit.matches(loader, minecraft))
        .ok_or_else(|| {
            Error::new(
                Code::InvalidInput,
                format!("o kit {} não serve a este pack", choice.id),
            )
            .param("field", "kit")
        })?;
    for project in &choice.projects {
        if !kit.item.iter().any(|item| &item.project == project) {
            return Err(Error::new(
                Code::InvalidInput,
                format!("{project} não é do kit {}", kit.id),
            )
            .param("field", "kit"));
        }
    }
    Ok(kit
        .item
        .iter()
        .filter(|item| choice.projects.contains(&item.project))
        .map(|item| KitWanted {
            source: item.source.source(),
            project: item.project.clone(),
            name: item.name.clone(),
            side: item.side.map(DataSide::choice),
        })
        .collect())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    #[test]
    fn os_dados_embutidos_sao_validos() {
        let kits = &embedded().kit;
        assert_eq!(kits.len(), 6);
        // Cada kit tem ao menos um item marcado e todos os itens do Modrinth têm ID de 8 letras.
        for kit in kits {
            assert!(kit.item.iter().any(|item| item.checked), "{}", kit.id);
        }
    }

    #[test]
    fn cada_faixa_tem_no_maximo_um_kit_por_loader() {
        // Uma versão não pode cair em dois kits do mesmo loader (a lista da tela ficaria ambígua).
        for loader in ["forge", "neoforge", "fabric"] {
            for minecraft in [
                "1.7.10", "1.12.2", "1.16.5", "1.18.2", "1.19.2", "1.19.4", "1.20.1", "1.20.4",
                "1.21", "1.21.1", "26.3",
            ] {
                let found = kits_for(minecraft, Some(loader));
                assert!(found.len() <= 1, "{loader} {minecraft}: {}", found.len());
            }
        }
    }

    #[test]
    fn kit_pela_faixa() {
        let ids = |minecraft, loader| {
            kits_for(minecraft, Some(loader))
                .into_iter()
                .map(|kit| kit.id)
                .collect::<Vec<_>>()
        };
        assert_eq!(ids("1.21.1", "fabric"), ["fabric-moderno"]);
        assert_eq!(ids("26.3", "neoforge"), ["neoforge-moderno"]);
        assert_eq!(ids("1.20.1", "forge"), ["forge-1-20-1"]);
        assert_eq!(ids("1.19.2", "forge"), ["forge-1-18-a-1-19"]);
        assert_eq!(ids("1.12.2", "forge"), ["forge-1-12-2"]);
        assert_eq!(ids("1.7.10", "forge"), ["forge-1-7-10"]);
        assert!(ids("1.20.1", "fabric").is_empty());
        assert!(kits_for("1.21.1", None).is_empty());
    }

    #[test]
    fn c2me_vem_desmarcado_com_aviso() {
        let kit = &kits_for("1.21.1", Some("fabric"))[0];
        let c2me = kit.items.iter().find(|item| item.name == "C2ME").unwrap();
        assert!(!c2me.checked);
        assert_eq!(c2me.note.as_deref(), Some("worldgen"));
    }

    #[test]
    fn escolha_do_kit_so_aceita_itens_do_kit() {
        let kit = &kits_for("1.21.1", Some("fabric"))[0];
        let choice = KitChoice {
            id: kit.id.clone(),
            projects: kit
                .items
                .iter()
                .take(2)
                .map(|item| item.project_id.clone())
                .collect(),
        };
        assert_eq!(wanted_for("1.21.1", "fabric", &choice).unwrap().len(), 2);
        let wrong_band = wanted_for("1.20.1", "forge", &choice).unwrap_err();
        assert_eq!(wrong_band.code, Code::InvalidInput);
        let alien = KitChoice {
            id: kit.id.clone(),
            projects: vec!["AAAAAAAA".into()],
        };
        assert!(wanted_for("1.21.1", "fabric", &alien).is_err());
    }

    #[test]
    fn dados_invalidos_sao_recusados() {
        let ok = "schema = 1\n[[kit]]\nid = \"a\"\nname = \"A\"\ndescription = \"d\"\nloader = \"fabric\"\nfrom = \"1.21\"\n[[kit.item]]\nsource = \"modrinth\"\nproject = \"AANobbMI\"\nname = \"Sodium\"\n";
        assert!(KitsData::parse(ok).is_ok());
        for bad in [
            ok.replace("AANobbMI", "naoexiste-id"),
            ok.replace("fabric", "quilt"),
            ok.replace("1.21", "um.vinte"),
            format!(
                "{ok}[[kit.item]]\nsource = \"modrinth\"\nproject = \"AANobbMI\"\nname = \"Outra vez\"\n"
            ),
            format!("{ok}{ok}").replace("schema = 1\n", "").replacen(
                "[[kit]]",
                "schema = 1\n[[kit]]",
                1,
            ),
        ] {
            assert!(KitsData::parse(&bad).is_err(), "{bad}");
        }
    }
}
