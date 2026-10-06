//! Índices pacote → mod e config de mixin → mod na passagem completa (D-05; ARCHITECTURE §9.6).
//!
//! Depois que o pack foi copiado para a instância, [`build_index`] monta o índice dos jars
//! (com o cache `cache/jarindex/` por hash, então a segunda passagem com o mesmo pack não abre
//! nenhum jar). O índice serve ao console agrupado, à busca do culpado e ao spark; aqui ele já
//! liga o padrão "falha de Mixin" do catálogo ao mod dono da config ([`attribute_mixin_owners`]):
//! o log diz `Mixin apply failed epicfight.mixins.json:…`, o índice diz que essa config é do Epic
//! Fight.

use std::path::Path;

use warden_jarmeta::Limits;
use warden_jarmeta::index::{BuildStats, JarIndexCache, PackIndex, PackJar, build_pack_index};

use super::{Input, Pass};
use crate::model::{Evidence, ItemRef, SuggestedFix};
use crate::postcrash::Analysis;

/// Resultado da montagem do índice.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexPass {
    /// O índice do pack (vazio na passagem rápida).
    pub index: PackIndex,
    /// Números da montagem (jars abertos, vindos do cache…).
    pub stats: BuildStats,
    /// Itens cujo jar não deu para indexar (caminho fora da instância, arquivo ausente ou
    /// ilegível). As regras do pré-teste já avisam sobre jars ilegíveis; o índice só fica sem
    /// eles.
    pub skipped: Vec<String>,
}

/// Monta o índice dos jars do pack, na instância já sincronizada.
///
/// Só a passagem completa monta (na rápida os jars ainda não foram baixados). Entram os itens
/// cujo arquivo é um `.jar`, procurado na pasta do metafile (`mods/x.pw.toml` → `mods/<arquivo>`).
/// `cache_dir` é a pasta `cache/` dos dados do Warden; o índice fica em `cache/jarindex/`.
#[must_use]
pub fn build_index(input: &Input, instance_dir: &Path, cache_dir: &Path) -> IndexPass {
    let loader = Some(input.pack.loader);
    if input.pass != Pass::Complete {
        return IndexPass {
            index: PackIndex::new(Vec::new(), loader),
            stats: BuildStats::default(),
            skipped: Vec::new(),
        };
    }
    let mut skipped = Vec::new();
    let mut jars = Vec::new();
    for item in &input.items {
        if !is_jar(&item.filename) {
            continue;
        }
        let folder = item.path.rsplit_once('/').map_or("", |(folder, _)| folder);
        let relative = if folder.is_empty() {
            item.filename.clone()
        } else {
            format!("{folder}/{}", item.filename)
        };
        match warden_core::resolve_inside(instance_dir, &relative) {
            Ok(path) => jars.push(PackJar::new(item.path.clone(), path)),
            Err(_) => skipped.push(item.path.clone()),
        }
    }
    let cache = JarIndexCache::in_cache_dir(cache_dir);
    let build = build_pack_index(&jars, loader, Some(&cache), &Limits::default());
    skipped.extend(build.failures.into_iter().map(|failure| failure.item));
    skipped.sort();
    IndexPass {
        index: build.index,
        stats: build.stats,
        skipped,
    }
}

fn is_jar(filename: &str) -> bool {
    Path::new(filename)
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("jar"))
}

/// Liga os achados de falha de Mixin (os que trazem o parâmetro `mixin_config`) ao mod dono da
/// config: acrescenta o item do pack e o id do mod, o parâmetro `mod` quando o log não disse, a
/// evidência do descritor que declara a config e, se o achado não tinha correção para o item,
/// "atualizar" e "remover" o item. Achados sem config ou com config sem dono ficam como estão.
pub fn attribute_mixin_owners(analysis: &mut Analysis, index: &PackIndex) {
    for entry in &mut analysis.findings {
        let Some(config) = entry.finding.param("mixin_config").map(str::to_owned) else {
            continue;
        };
        let Some(owner) = index.mixin_owner(&config) else {
            continue;
        };
        let item = ItemRef::Metafile {
            path: owner.owner.item.to_owned(),
        };
        let finding = &mut entry.finding;
        finding.push_item(item.clone());
        if let Some(mod_id) = owner.owner.mod_id {
            finding.push_item(ItemRef::ModId {
                id: mod_id.to_owned(),
            });
            if finding.param("mod").is_none() {
                *finding = finding.clone().with_param("mod", mod_id);
            }
        }
        let descriptor = owner.declared_in.path();
        let file_in_jar = if owner.owner.jar.is_empty() {
            descriptor.to_owned()
        } else {
            format!("{}!/{descriptor}", owner.owner.jar)
        };
        finding.push_evidence(Evidence::JarMetadata {
            item: item.clone(),
            file_in_jar,
            excerpt: config.clone(),
        });
        let targets_item = finding.fixes().iter().any(|fix| {
            matches!(
                fix,
                SuggestedFix::UpdateItem { item: target } | SuggestedFix::RemoveItem { item: target }
                    if *target == item
            )
        });
        if !targets_item {
            *finding = finding
                .clone()
                .with_fix(SuggestedFix::UpdateItem { item: item.clone() })
                .with_fix(SuggestedFix::RemoveItem { item });
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, HashMap};
    use std::io::{Cursor, Write as _};

    use serde_json::Value;
    use warden_jarmeta::Loader;
    use warden_packwiz::Side;
    use zip::write::{SimpleFileOptions, ZipWriter};

    use super::*;
    use crate::model::Severity;
    use crate::postcrash::{AnalysisContext, LogLoader, analyze_text};
    use crate::pretest::{PackContext, PackItem};

    /// Esqueleto de um jar real (descritores, pacotes e configs de mixin), versionado pela D-05
    /// em `warden-jarmeta/tests/indice/esqueletos.json`.
    fn real_skeleton(name: &str) -> Value {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../warden-jarmeta/tests/indice/esqueletos.json");
        let all: BTreeMap<String, Value> =
            serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        all[name].clone()
    }

    /// Remonta o jar: descritores sem alteração, uma classe vazia por pacote, as configs.
    fn rebuild(skeleton: &Value) -> Vec<u8> {
        let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
        let options = SimpleFileOptions::default();
        let empty = serde_json::Map::new();
        for (name, content) in skeleton["arquivos"].as_object().unwrap_or(&empty) {
            zip.start_file(name.as_str(), options).unwrap();
            let bytes: Vec<u8> = match content {
                Value::String(text) => text.as_bytes().to_vec(),
                other => serde_json::from_value(other.clone()).unwrap(),
            };
            zip.write_all(&bytes).unwrap();
        }
        for package in skeleton["pacotes"].as_array().into_iter().flatten() {
            let name = format!("{}/X.class", package.as_str().unwrap().replace('.', "/"));
            zip.start_file(name, options).unwrap();
            zip.write_all(&[0xCA, 0xFE, 0xBA, 0xBE, 0, 0, 0, 65])
                .unwrap();
        }
        for config in skeleton["mixins"].as_array().into_iter().flatten() {
            zip.start_file(config.as_str().unwrap(), options).unwrap();
            zip.write_all(b"{}").unwrap();
        }
        zip.finish().unwrap().into_inner()
    }

    fn item(path: &str, filename: &str) -> PackItem {
        PackItem {
            path: path.into(),
            filename: filename.into(),
            side: Side::Both,
            file_hash: None,
            api: None,
            jar: None,
            accepted_minecraft_versions: vec![],
            resource_pack_format: None,
        }
    }

    fn input(pass: Pass, items: Vec<PackItem>) -> Input {
        Input {
            pack: PackContext {
                minecraft: "1.21.1".into(),
                minecraft_release_target: None,
                loader: Loader::NeoForge,
                loader_version: "21.1.252".into(),
                language_loader_versions: HashMap::new(),
                java_major: 21,
                minimum_java_major: Some(21),
                resource_pack_format: None,
                acceptable_game_versions: vec![],
            },
            items,
            pass,
        }
    }

    /// Linha no formato do Mixin no Forge/NeoForge (S-R5-3, `spike-forge-mixin`), com a config
    /// real do Epic Fight.
    const LOG: &str = "[12:00:01] [main/ERROR] [mixin/]: Mixin apply failed epicfight.mixins.json:MixinLivingEntity -> net.minecraft.world.entity.LivingEntity: org.spongepowered.asm.mixin.injection.throwables.InvalidInjectionException Critical injection failure: @Inject annotation on epicfight$hurt could not find any targets matching 'hurt' in net/minecraft/world/entity/LivingEntity.\n";

    #[test]
    fn ca_d05_falha_de_mixin_aponta_o_epic_fight() {
        let dir = tempfile::tempdir().unwrap();
        let instance = dir.path().join("instancia");
        std::fs::create_dir_all(instance.join("mods")).unwrap();
        std::fs::write(
            instance.join("mods/epic-fight-21.17.3.1-mc1.21.1-neoforge.jar"),
            rebuild(&real_skeleton("neoforge-1.21.1-epic-fight")),
        )
        .unwrap();
        std::fs::write(
            instance.join("mods/sodium-neoforge-0.8.13.jar"),
            rebuild(&real_skeleton("neoforge-1.21.1-sodium")),
        )
        .unwrap();
        let items = vec![
            item(
                "mods/epic-fight.pw.toml",
                "epic-fight-21.17.3.1-mc1.21.1-neoforge.jar",
            ),
            item("mods/sodium.pw.toml", "sodium-neoforge-0.8.13.jar"),
            item("resourcepacks/x.pw.toml", "x.zip"),
            item("mods/sumiu.pw.toml", "sumiu.jar"),
            item("mods/fora.pw.toml", "../../fora.jar"),
        ];
        let cache = dir.path().join("cache");
        let pass = build_index(&input(Pass::Complete, items.clone()), &instance, &cache);
        assert_eq!(pass.stats.jars_opened, 2);
        assert_eq!(pass.skipped, ["mods/fora.pw.toml", "mods/sumiu.pw.toml"]);

        let context = AnalysisContext {
            loader: Some(LogLoader::NeoForge),
            minecraft: Some("1.21.1".into()),
        };
        let mut analysis = analyze_text("latest.log", LOG, &context).unwrap();
        attribute_mixin_owners(&mut analysis, &pass.index);
        let mixin = analysis
            .findings
            .iter()
            .find(|f| f.finding.rule().as_str() == "E_MIXIN")
            .expect("achado de mixin");
        let finding = &mixin.finding;
        assert_eq!(finding.severity(), Severity::Error);
        assert_eq!(finding.param("mixin_config"), Some("epicfight.mixins.json"));
        assert_eq!(finding.param("mod"), Some("epicfight"));
        let epic = ItemRef::Metafile {
            path: "mods/epic-fight.pw.toml".into(),
        };
        assert!(finding.items().contains(&epic), "{:?}", finding.items());
        assert!(finding.items().contains(&ItemRef::ModId {
            id: "epicfight".into()
        }));
        assert!(finding.evidence().contains(&Evidence::JarMetadata {
            item: epic.clone(),
            file_in_jar: "META-INF/neoforge.mods.toml".into(),
            excerpt: "epicfight.mixins.json".into(),
        }));
        // A evidência principal continua sendo a linha do log.
        assert!(matches!(finding.primary_evidence(), Evidence::Log { .. }));
        assert!(
            finding
                .fixes()
                .contains(&SuggestedFix::UpdateItem { item: epic.clone() })
        );
        assert!(
            finding
                .fixes()
                .contains(&SuggestedFix::RemoveItem { item: epic })
        );

        // Segunda passagem com o mesmo pack: nenhum jar aberto.
        let again = build_index(&input(Pass::Complete, items.clone()), &instance, &cache);
        assert_eq!(again.stats.jars_opened, 0);
        assert_eq!(again.stats.cache_hits, 2);
        assert_eq!(again.index, pass.index);

        // A passagem rápida não monta nada.
        let quick = build_index(&input(Pass::Quick, items), &instance, &cache);
        assert!(quick.index.items().is_empty());
        assert_eq!(quick.stats, BuildStats::default());
    }

    #[test]
    fn mixin_do_sodium_no_jar_embutido_e_config_sem_dono() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("mods")).unwrap();
        std::fs::write(
            dir.path().join("mods/sodium.jar"),
            rebuild(&real_skeleton("neoforge-1.21.1-sodium")),
        )
        .unwrap();
        let pass = build_index(
            &input(
                Pass::Complete,
                vec![item("mods/sodium.pw.toml", "sodium.jar")],
            ),
            dir.path(),
            &dir.path().join("cache"),
        );
        let log = "Mixin apply for mod sodium failed sodium-neoforge.mixins.json:MixinX -> net.minecraft.client.Minecraft: erro\nMixin apply failed desconhecida.mixins.json:MixinY -> net.minecraft.client.Minecraft: erro\n";
        let mut analysis = analyze_text("latest.log", log, &AnalysisContext::default()).unwrap();
        let before = analysis.clone();
        attribute_mixin_owners(&mut analysis, &pass.index);
        let sodium = analysis
            .findings
            .iter()
            .find(|f| f.finding.param("mixin_config") == Some("sodium-neoforge.mixins.json"))
            .unwrap();
        let item = ItemRef::Metafile {
            path: "mods/sodium.pw.toml".into(),
        };
        assert!(sodium.finding.items().contains(&item));
        // O log já trazia o mod e as correções pelo id; o item do pack entra junto.
        assert_eq!(sodium.finding.param("mod"), Some("sodium"));
        assert!(
            sodium
                .finding
                .fixes()
                .contains(&SuggestedFix::RemoveItem { item })
        );
        let unknown_before = before
            .findings
            .iter()
            .find(|f| f.finding.param("mixin_config") == Some("desconhecida.mixins.json"))
            .unwrap();
        let unknown_after = analysis
            .findings
            .iter()
            .find(|f| f.finding.param("mixin_config") == Some("desconhecida.mixins.json"))
            .unwrap();
        assert_eq!(unknown_before, unknown_after);
    }
}
