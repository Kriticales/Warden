//! Regras determinísticas antes do teste. A passagem rápida usa dados do pack e das APIs;
//! a completa acrescenta metadados dos jars já copiados para a instância.
//!
//! O chamador resolve rede, arquivos e cache. Nenhuma regra baixa arquivos ou altera o pack.

mod curated;
mod mc_version;
mod rules;

use std::collections::HashMap;

use warden_jarmeta::{JarMetadata, Loader};
use warden_packwiz::Side;

use crate::model::{Finding, Source};

/// Passagem do pré-teste.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pass {
    /// Antes de baixar os arquivos.
    Quick,
    /// Depois de copiar o pack para a instância.
    Complete,
}

/// Dados do pack e do ambiente de teste.
#[derive(Debug, Clone)]
pub struct PackContext {
    /// Versão exata do Minecraft.
    pub minecraft: String,
    /// Alvo da versão no JSON da Mojang (`release_target`), necessário para snapshots Fabric.
    pub minecraft_release_target: Option<String>,
    /// Loader usado pelo pack.
    pub loader: Loader,
    /// Versão do loader.
    pub loader_version: String,
    /// Versão instalada de cada linguagem de mod (`javafml`, `lowcodefml`...), quando conhecida.
    pub language_loader_versions: HashMap<String, String>,
    /// Java escolhido para o teste.
    pub java_major: u16,
    /// Java mínimo indicado pelo catálogo da versão do jogo/loader, se conhecido.
    pub minimum_java_major: Option<u16>,
    /// `pack_format` esperado para resource packs nesta versão, se conhecido.
    pub resource_pack_format: Option<u32>,
    /// Versões adicionais aceitas pelo autor do packwiz.
    pub acceptable_game_versions: Vec<String>,
}

/// Relação vinda da API. O campo e a URL identificam a evidência exata.
#[derive(Debug, Clone)]
pub struct ApiRelation {
    /// Projeto relacionado, na mesma fonte.
    pub project_id: String,
    /// Relação incompatível ou recomendada.
    pub kind: ApiRelationKind,
    /// Campo da resposta que declarou a relação.
    pub field: String,
}

/// Tipos de relação de API relevantes ao diagnóstico.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApiRelationKind {
    /// Modrinth `incompatible` ou CurseForge relação 5.
    Incompatible,
    /// Modrinth `required` ou CurseForge relação 3.
    Required,
}

/// Metadados de uma versão da API, já obtidos pelo cliente da fonte.
#[derive(Debug, Clone)]
pub struct ApiMetadata {
    /// Fonte da resposta.
    pub source: Source,
    /// URL exata consultada.
    pub url: String,
    /// ID do projeto, quando a fonte o fornece.
    pub project_id: String,
    /// Slug canônico do projeto para encontrar o mesmo mod vindo das duas fontes.
    pub project_slug: Option<String>,
    /// Versões do Minecraft declaradas para o arquivo.
    pub game_versions: Vec<String>,
    /// Loaders declarados para o arquivo.
    pub loaders: Vec<Loader>,
    /// Versão alpha, beta ou release.
    pub release: ReleaseChannel,
    /// Relações declaradas.
    pub relations: Vec<ApiRelation>,
    /// Projeto arquivado na fonte.
    pub archived: bool,
    /// A CurseForge bloqueia distribuição automática deste arquivo.
    pub download_blocked: bool,
    /// Campo `client_side` do Modrinth, se conhecido.
    pub client_side: Option<bool>,
    /// Campo `server_side` do Modrinth, se conhecido.
    pub server_side: Option<bool>,
}

/// Canal de publicação da versão.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReleaseChannel {
    /// Versão final.
    Release,
    /// Beta.
    Beta,
    /// Alpha.
    Alpha,
}

/// Um item instalado pelo pack. O caminho identifica o metafile ou o arquivo local.
#[derive(Debug, Clone)]
pub struct PackItem {
    /// Caminho relativo à raiz do pack, com `/`.
    pub path: String,
    /// Nome do jar copiado, se conhecido.
    pub filename: String,
    /// Lado gravado no metafile.
    pub side: Side,
    /// Hash do arquivo real, quando conhecido. Deve ser o hash dos bytes, não do metafile.
    pub file_hash: Option<String>,
    /// Metadados de API opcionais.
    pub api: Option<ApiMetadata>,
    /// Metadados extraídos do jar; usados só na passagem completa.
    pub jar: Option<JarMetadata>,
    /// `acceptedMinecraftVersions` do Forge antigo, quando o leitor do jar o fornecer.
    pub accepted_minecraft_versions: Vec<String>,
    /// `pack_format` de um resource pack, quando foi possível lê-lo.
    pub resource_pack_format: Option<u32>,
}

/// Entrada de uma passagem.
#[derive(Debug, Clone)]
pub struct Input {
    /// Pack e ambiente.
    pub pack: PackContext,
    /// Itens do pack.
    pub items: Vec<PackItem>,
    /// Passagem solicitada.
    pub pass: Pass,
}

/// Analisa o pack com as regras que têm evidência disponível nesta passagem.
///
/// Erra somente se um dos catálogos curados embutidos for inválido. Esses catálogos são
/// validados pelos testes para que isso seja uma falha de desenvolvimento, não do usuário.
pub fn analyze(input: &Input) -> crate::Result<Vec<Finding>> {
    rules::analyze(input)
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use proptest::prelude::*;
    use serde_json::json;
    use warden_jarmeta::{ClassVersion, ModMetadata};

    use super::*;

    fn pack(loader: Loader) -> PackContext {
        PackContext {
            minecraft: "1.21.1".into(),
            minecraft_release_target: None,
            loader,
            loader_version: "21.1.252".into(),
            language_loader_versions: HashMap::new(),
            java_major: 21,
            minimum_java_major: Some(21),
            resource_pack_format: None,
            acceptable_game_versions: vec![],
        }
    }

    fn meta(id: &str, loader: Loader) -> ModMetadata {
        let (source, loader_name) = match loader {
            Loader::Fabric => ("fabricModJson", "fabric"),
            Loader::Forge => ("modsToml", "forge"),
            Loader::NeoForge => ("neoForgeModsToml", "neoForge"),
            Loader::Quilt => ("quiltModJson", "quilt"),
        };
        serde_json::from_value(
            json!({"source":source,"loader":loader_name,"id":id,"version":"1.0"}),
        )
        .unwrap()
    }

    fn item(id: &str, loader: Loader) -> PackItem {
        let m = meta(id, loader);
        PackItem {
            path: format!("mods/{id}.pw.toml"),
            filename: format!("{id}.jar"),
            side: Side::Both,
            file_hash: None,
            api: None,
            jar: Some(JarMetadata {
                descriptors: vec![m.source],
                mods: vec![m],
                ..Default::default()
            }),
            accepted_minecraft_versions: vec![],
            resource_pack_format: None,
        }
    }

    fn codes(findings: &[Finding]) -> Vec<&str> {
        findings.iter().map(|f| f.rule().as_str()).collect()
    }

    #[test]
    fn ca_t14_01_neoforge_duplicado_fabric_sem_connector_e_minecraft_errado() {
        #[derive(serde::Deserialize)]
        struct Case {
            minecraft: String,
            loader: String,
            loader_version: String,
            jar: Vec<CaseJar>,
        }
        #[derive(serde::Deserialize)]
        struct CaseJar {
            id: String,
            descriptor: String,
            path: String,
            minecraft_range: Option<String>,
        }
        let case: Case =
            toml::from_str(include_str!("../../tests/packs/neoforge-tres-erros.toml")).unwrap();
        let loader = match case.loader.as_str() {
            "neoforge" => Loader::NeoForge,
            _ => panic!("loader de fixture inválido"),
        };
        let mut context = pack(loader);
        context.minecraft = case.minecraft;
        context.loader_version = case.loader_version;
        let items = case
            .jar
            .into_iter()
            .map(|jar| {
                let descriptor = match jar.descriptor.as_str() {
                    "neoforge" => Loader::NeoForge,
                    "fabric" => Loader::Fabric,
                    _ => panic!("descritor de fixture inválido"),
                };
                let mut item = item(&jar.id, descriptor);
                item.path = jar.path;
                if let Some(range) = jar.minecraft_range {
                    item.jar.as_mut().unwrap().mods[0].minecraft =
                        Some(warden_jarmeta::VersionRange::maven(range));
                }
                item
            })
            .collect();
        let input = Input {
            pack: context,
            pass: Pass::Complete,
            items,
        };
        let findings = analyze(&input).unwrap();
        for code in ["E_DUPLICATE_ID", "E_LOADER", "E_MCVERSION"] {
            assert!(
                codes(&findings).contains(&code),
                "{code}: {:?}",
                codes(&findings)
            );
        }
        let duplicate = findings
            .iter()
            .find(|f| f.rule().as_str() == "E_DUPLICATE_ID")
            .unwrap();
        assert!(
            matches!(duplicate.primary_evidence(), crate::Evidence::JarMetadata { file_in_jar, .. }
            if file_in_jar == "META-INF/neoforge.mods.toml")
        );
        assert!(findings.iter().all(|f| !f.evidence().is_empty()));
    }

    #[test]
    fn ca_t14_01_dependencia_e_java_com_correcoes() {
        let mut main = item("principal", Loader::NeoForge);
        let m = &mut main.jar.as_mut().unwrap().mods[0];
        m.dependencies.push(serde_json::from_value(json!({"id":"biblioteca","kind":"required","range":{"dialect":"maven","spec":"[2,)"}})).unwrap());
        m.dependencies.push(serde_json::from_value(json!({"id":"auxiliar","kind":"optional","range":{"dialect":"maven","spec":"[2,)"}})).unwrap());
        m.dependencies.push(
            serde_json::from_value(
                json!({"id":"perigoso","kind":"incompatible","range":{"dialect":"any"}}),
            )
            .unwrap(),
        );
        m.java = Some(warden_jarmeta::VersionRange::maven("[21,)"));
        main.jar.as_mut().unwrap().class_version = Some(ClassVersion {
            major: 65,
            minor: 0,
        });
        let mut auxiliary = item("auxiliar", Loader::NeoForge);
        auxiliary.jar.as_mut().unwrap().mods[0].version = Some("1.0".into());
        let mut input = Input {
            pack: pack(Loader::NeoForge),
            pass: Pass::Complete,
            items: vec![main, auxiliary, item("perigoso", Loader::NeoForge)],
        };
        input.pack.java_major = 17;
        let found = analyze(&input).unwrap();
        for code in ["E_MISSING_DEP", "E_DEP_VERSION", "E_BREAKS", "E_JAVA"] {
            assert!(codes(&found).contains(&code), "{code}: {:?}", codes(&found));
        }
        assert!(
            found
                .iter()
                .find(|f| f.rule().as_str() == "E_MISSING_DEP")
                .unwrap()
                .fixes()
                .iter()
                .any(|f| matches!(f, crate::SuggestedFix::AddDependency { .. }))
        );
        assert!(
            found
                .iter()
                .find(|f| f.rule().as_str() == "E_JAVA")
                .unwrap()
                .fixes()
                .iter()
                .any(|f| matches!(f, crate::SuggestedFix::ChangeJava { .. }))
        );
    }

    #[test]
    fn passagem_rapida_usa_api_e_nao_usa_jar() {
        let mut modrinth = item("modrinth", Loader::Fabric);
        modrinth.api = Some(ApiMetadata {
            source: Source::Modrinth,
            url: "https://api.modrinth.com/v2/version/x".into(),
            project_id: "x".into(),
            project_slug: None,
            game_versions: vec!["1.20.1".into()],
            loaders: vec![Loader::Fabric],
            release: ReleaseChannel::Beta,
            relations: vec![ApiRelation {
                project_id: "y".into(),
                kind: ApiRelationKind::Required,
                field: "dependencies".into(),
            }],
            archived: false,
            download_blocked: false,
            client_side: Some(true),
            server_side: Some(false),
        });
        let mut input = Input {
            pack: pack(Loader::NeoForge),
            pass: Pass::Quick,
            items: vec![modrinth],
        };
        input.pack.java_major = 17;
        let found = analyze(&input).unwrap();
        for code in [
            "E_LOADER",
            "E_MCVERSION",
            "E_MISSING_DEP",
            "E_JAVA",
            "W_SIDE",
            "I_PRERELEASE",
        ] {
            assert!(codes(&found).contains(&code), "{code}: {:?}", codes(&found));
        }
        assert!(found.iter().all(|f| !f.evidence().is_empty()));
    }

    #[test]
    fn faixas_maven_recomendacao_nao_e_minimo_e_provides_satisfaz() {
        let mut main = item("principal", Loader::Forge);
        main.jar.as_mut().unwrap().mods[0].dependencies.push(
            serde_json::from_value(json!({
                "id":"alias","kind":"required","range":{"dialect":"maven","spec":"1.0"}
            }))
            .unwrap(),
        );
        let mut dep = item("real", Loader::Forge);
        dep.jar.as_mut().unwrap().mods[0]
            .provides
            .push("alias".into());
        dep.jar.as_mut().unwrap().mods[0].version = Some("0.1".into());
        let mut input = Input {
            pack: pack(Loader::Forge),
            pass: Pass::Complete,
            items: vec![main, dep],
        };
        input.pack.minecraft = "1.20.1".into();
        input.pack.loader_version = "47.2.0".into();
        let found = analyze(&input).unwrap();
        assert!(!codes(&found).contains(&"E_MISSING_DEP"));
        assert!(!codes(&found).contains(&"E_DEP_VERSION"));
        assert!(!codes(&found).contains(&"E_DUPLICATE_ID"));
    }

    #[test]
    fn conflitos_curados_e_hash_duplicado() {
        let mut input = Input {
            pack: pack(Loader::Fabric),
            pass: Pass::Complete,
            items: vec![
                item("optifine", Loader::Fabric),
                item("sodium", Loader::Fabric),
            ],
        };
        input.items[0].file_hash = Some("igual".into());
        input.items[1].file_hash = Some("igual".into());
        let found = analyze(&input).unwrap();
        for code in ["E_DUPLICATE_FILE", "E_EXCLUSIVE", "E_KNOWN_CONFLICT"] {
            assert!(codes(&found).contains(&code), "{code}: {:?}", codes(&found));
        }
    }

    #[test]
    fn metadados_reais_do_corpus_sao_aceitos_no_loader_correto() {
        let snapshot = include_str!(
            "../../../warden-jarmeta/tests/snapshots/corpus__neoforge-1.21.1-sodium.snap"
        );
        let json = snapshot.rsplit_once("---\n").unwrap().1;
        let jar: JarMetadata = serde_json::from_str(json).unwrap();
        let mut sodium = item("sodium", Loader::NeoForge);
        sodium.jar = Some(jar);
        let found = analyze(&Input {
            pack: pack(Loader::NeoForge),
            pass: Pass::Complete,
            items: vec![sodium],
        })
        .unwrap();
        assert!(!codes(&found).contains(&"E_LOADER"), "{:?}", codes(&found));
        assert!(
            !codes(&found).contains(&"E_MCVERSION"),
            "{:?}",
            codes(&found)
        );
    }

    #[test]
    fn regras_restantes_do_jar_e_dados_curados() {
        let mut principal = item("principal", Loader::NeoForge);
        let meta = &mut principal.jar.as_mut().unwrap().mods[0];
        meta.loader_version = Some(warden_jarmeta::VersionRange::maven("[22,)"));
        meta.environment = warden_jarmeta::Environment::Client;
        meta.dependencies.push(
            serde_json::from_value(
                json!({"id":"ausente","kind":"recommends","range":{"dialect":"any"}}),
            )
            .unwrap(),
        );
        meta.dependencies.push(
            serde_json::from_value(
                json!({"id":"outro","kind":"discouraged","range":{"dialect":"any"}}),
            )
            .unwrap(),
        );
        let mut input = Input {
            pack: pack(Loader::NeoForge),
            pass: Pass::Complete,
            items: vec![
                principal,
                item("outro", Loader::NeoForge),
                item("starlight", Loader::NeoForge),
            ],
        };
        let mut optifine = item("optifine", Loader::NeoForge);
        optifine.filename = "OptiFine_1.21.1_HD_U.jar".into();
        optifine.jar = Some(JarMetadata::default());
        input.items.push(optifine);
        input.items.push(item("sodium", Loader::NeoForge));
        let found = analyze(&input).unwrap();
        for code in [
            "E_LOADERVERSION",
            "W_SIDE",
            "W_CONFLICTS",
            "W_OBSOLETE",
            "E_KNOWN_CONFLICT",
        ] {
            assert!(codes(&found).contains(&code), "{code}: {:?}", codes(&found));
        }
        assert!(!found.iter().any(|f| f.rule().as_str() == "E_LOADER"
            && f.items().contains(&crate::ItemRef::Metafile {
                path: "mods/optifine.pw.toml".into()
            })));
    }

    #[test]
    fn api_bloqueio_conflito_e_mesmo_projeto_entre_fontes() {
        let api = |source, project_id: &str| ApiMetadata {
            source,
            url: format!("https://example.invalid/{project_id}"),
            project_id: project_id.into(),
            project_slug: Some("mesmo-mod".into()),
            game_versions: vec!["1.21.1".into()],
            loaders: vec![Loader::Fabric],
            release: ReleaseChannel::Release,
            relations: vec![],
            archived: false,
            download_blocked: source == Source::CurseForge,
            client_side: None,
            server_side: None,
        };
        let mut first = item("primeiro", Loader::Fabric);
        first.api = Some(api(Source::Modrinth, "m1"));
        first.api.as_mut().unwrap().relations.push(ApiRelation {
            project_id: "m2".into(),
            kind: ApiRelationKind::Incompatible,
            field: "dependencies[0]".into(),
        });
        let mut second = item("segundo", Loader::Fabric);
        second.api = Some(api(Source::CurseForge, "c2"));
        let mut third = item("terceiro", Loader::Fabric);
        third.api = Some(api(Source::Modrinth, "m2"));
        third.api.as_mut().unwrap().project_slug = None;
        let mut input = Input {
            pack: pack(Loader::Fabric),
            pass: Pass::Quick,
            items: vec![first, second, third],
        };
        input.items[0].resource_pack_format = Some(3);
        input.pack.resource_pack_format = Some(4);
        let found = analyze(&input).unwrap();
        for code in [
            "E_DUPLICATE_FILE",
            "W_INCOMPATIBLE_API",
            "W_DOWNLOAD_BLOCKED",
            "I_PACK_FORMAT",
        ] {
            assert!(codes(&found).contains(&code), "{code}: {:?}", codes(&found));
        }
    }

    #[test]
    fn neoforge_1201_aceita_mods_toml_mas_121_nao() {
        let forge_jar = item("antigo", Loader::Forge);
        let mut input = Input {
            pack: pack(Loader::NeoForge),
            pass: Pass::Complete,
            items: vec![forge_jar],
        };
        input.pack.minecraft = "1.20.1".into();
        assert!(!codes(&analyze(&input).unwrap()).contains(&"E_LOADER"));
        input.pack.minecraft = "1.21.1".into();
        assert!(codes(&analyze(&input).unwrap()).contains(&"E_LOADER"));
    }

    #[test]
    fn dependencias_aninhadas_e_connector_satisfazem_o_pack() {
        let mut main = item("principal", Loader::Fabric);
        main.jar.as_mut().unwrap().mods[0].dependencies.push(serde_json::from_value(json!({
            "id":"embutido","kind":"required","range":{"dialect":"fabric","predicates":[">=1.0"]}
        })).unwrap());
        let nested = item("embutido", Loader::Fabric).jar.unwrap();
        main.jar
            .as_mut()
            .unwrap()
            .nested
            .push(warden_jarmeta::NestedJar {
                path: "META-INF/jars/embutido.jar".into(),
                declared_by: warden_jarmeta::DescriptorKind::FabricModJson,
                jarjar: None,
                metadata: Some(Box::new(nested)),
            });
        let input = Input {
            pack: pack(Loader::Fabric),
            pass: Pass::Complete,
            items: vec![main],
        };
        assert!(!codes(&analyze(&input).unwrap()).contains(&"E_MISSING_DEP"));

        let input = Input {
            pack: pack(Loader::Forge),
            pass: Pass::Complete,
            items: vec![
                item("connector", Loader::Forge),
                item("mod_fabric", Loader::Fabric),
            ],
        };
        assert!(!codes(&analyze(&input).unwrap()).contains(&"E_LOADER"));
    }

    #[test]
    fn coremod_antigo_e_linguagem_do_loader() {
        let mut coremod = item("coremod", Loader::Forge);
        coremod.jar.as_mut().unwrap().mods.clear();
        coremod.jar.as_mut().unwrap().manifest = Some(warden_jarmeta::ManifestInfo {
            fml_core_plugin: Some("exemplo.CorePlugin".into()),
            ..Default::default()
        });
        let mut language = item("linguagem", Loader::Forge);
        language.jar.as_mut().unwrap().language_loader = Some(warden_jarmeta::LanguageLoader {
            name: "javafml".into(),
            version: warden_jarmeta::VersionRange::maven("[50,)"),
        });
        let mut input = Input {
            pack: pack(Loader::Forge),
            pass: Pass::Complete,
            items: vec![coremod, language],
        };
        input.pack.minecraft = "1.12.2".into();
        input.pack.loader_version = "47.1.0".into();
        input
            .pack
            .language_loader_versions
            .insert("javafml".into(), "47".into());
        let found = analyze(&input).unwrap();
        assert!(codes(&found).contains(&"I_LEGACY_COREMOD"));
        assert!(codes(&found).contains(&"E_LOADERVERSION"));
        assert!(!codes(&found).contains(&"E_LOADER"));
    }

    #[test]
    fn prerelease_do_fabric_usa_versao_normalizada() {
        let mut mod_item = item("mod", Loader::Fabric);
        mod_item.jar.as_mut().unwrap().mods[0].minecraft =
            Some(warden_jarmeta::VersionRange::fabric(">=1.21.1-beta.1"));
        let mut input = Input {
            pack: pack(Loader::Fabric),
            pass: Pass::Complete,
            items: vec![mod_item],
        };
        input.pack.minecraft = "1.21.1-pre2".into();
        assert!(!codes(&analyze(&input).unwrap()).contains(&"E_MCVERSION"));
        input.pack.minecraft = "1.21.1-rc1".into();
        assert!(!codes(&analyze(&input).unwrap()).contains(&"E_MCVERSION"));
    }

    #[test]
    fn forge_antigo_respeita_accepted_minecraft_versions() {
        let mut old = item("antigo", Loader::Forge);
        old.jar.as_mut().unwrap().mods[0].minecraft = Some(warden_jarmeta::VersionRange::Exact {
            version: "1.10.2".into(),
        });
        old.accepted_minecraft_versions.push("1.12.2".into());
        let mut input = Input {
            pack: pack(Loader::Forge),
            pass: Pass::Complete,
            items: vec![old],
        };
        input.pack.minecraft = "1.12.2".into();
        assert!(!codes(&analyze(&input).unwrap()).contains(&"E_MCVERSION"));
        input.pack.minecraft = "1.11.2".into();
        assert!(codes(&analyze(&input).unwrap()).contains(&"E_MCVERSION"));
    }

    #[test]
    #[allow(clippy::print_stderr)] // A medição da meta de 3 s precisa constar no relatório do teste.
    fn analise_300_mods_cache_quente_mede_tempo() {
        let input = Input {
            pack: pack(Loader::Fabric),
            pass: Pass::Complete,
            items: (0..300)
                .map(|n| item(&format!("mod_{n}"), Loader::Fabric))
                .collect(),
        };
        let _ = analyze(&input).unwrap();
        let start = Instant::now();
        let findings = analyze(&input).unwrap();
        eprintln!("D-01: 300 mods, cache quente: {:?}", start.elapsed());
        assert!(findings.is_empty());
        assert!(start.elapsed().as_secs_f64() < 3.0);
    }

    proptest! {
        #[test]
        fn todo_achado_tem_evidencia(ids in proptest::collection::vec("[a-z]{1,10}", 0..30)) {
            let input = Input { pack: pack(Loader::Fabric), pass: Pass::Complete,
                items: ids.iter().enumerate().map(|(n, id)| {
                    let mut item = item(id, Loader::Fabric);
                    item.path = format!("mods/{n}.pw.toml"); item
                }).collect() };
            let findings = analyze(&input).unwrap();
            prop_assert!(findings.iter().all(|f| !f.evidence().is_empty()));
        }
    }
}
