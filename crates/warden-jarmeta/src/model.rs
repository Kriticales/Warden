//! Modelo normalizado dos metadados de um jar (R3 §4.5).
//!
//! Um jar pode trazer vários descritores (mods multi-loader têm `fabric.mod.json` e
//! `META-INF/mods.toml` no mesmo arquivo) e cada descritor pode declarar vários mods. O modelo
//! guarda tudo; quem decide o que vale para um pack é o diagnóstico, com a ajuda de
//! [`JarMetadata::mods_for_loader`].
//!
//! O formato serializado (JSON, `camelCase`) é estável: os dourados do corpus e o cache por
//! hash dos índices (D-05) dependem dele.

use serde::{Deserialize, Serialize};

use crate::range::VersionRange;

/// Loader de mods a que um descritor se destina.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Loader {
    /// Fabric (`fabric.mod.json`).
    Fabric,
    /// Quilt (`quilt.mod.json`; o Quilt também lê `fabric.mod.json`).
    Quilt,
    /// `MinecraftForge`: `META-INF/mods.toml` (1.13+) ou `mcmod.info` (1.7.10 a 1.12.2).
    Forge,
    /// NeoForge (`META-INF/neoforge.mods.toml`; até a 20.4 usava `mods.toml`).
    NeoForge,
}

/// Arquivo de metadados reconhecido dentro do jar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DescriptorKind {
    /// `quilt.mod.json`.
    QuiltModJson,
    /// `fabric.mod.json`.
    FabricModJson,
    /// `META-INF/neoforge.mods.toml`.
    NeoForgeModsToml,
    /// `META-INF/mods.toml`.
    ModsToml,
    /// `mcmod.info`.
    McmodInfo,
    /// `META-INF/jarjar/metadata.json` (jars embutidos do Forge 1.18.2+ e do NeoForge).
    JarJarMetadata,
    /// `META-INF/MANIFEST.MF`.
    Manifest,
}

impl DescriptorKind {
    /// Caminho do arquivo dentro do jar.
    #[must_use]
    pub fn path(self) -> &'static str {
        match self {
            Self::QuiltModJson => "quilt.mod.json",
            Self::FabricModJson => "fabric.mod.json",
            Self::NeoForgeModsToml => "META-INF/neoforge.mods.toml",
            Self::ModsToml => "META-INF/mods.toml",
            Self::McmodInfo => "mcmod.info",
            Self::JarJarMetadata => "META-INF/jarjar/metadata.json",
            Self::Manifest => "META-INF/MANIFEST.MF",
        }
    }
}

/// Lado em que um mod roda (`environment` do Fabric, `clientSideOnly` do Forge...).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Environment {
    /// Cliente e servidor.
    #[default]
    Both,
    /// Só cliente.
    Client,
    /// Só servidor dedicado.
    Server,
}

/// Lado em que uma dependência é exigida (`side` do Forge/NeoForge, `client-`/`server-` do `@Mod`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Side {
    /// Nos dois lados.
    #[default]
    Both,
    /// Só no cliente.
    Client,
    /// Só no servidor.
    Server,
}

/// Ordem de carregamento pedida (`ordering` do Forge/NeoForge, `before`/`after` do `@Mod`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LoadOrdering {
    /// Sem ordem.
    #[default]
    None,
    /// Este mod carrega antes do outro.
    Before,
    /// Este mod carrega depois do outro.
    After,
}

/// Tipo de relação com outro mod, unificando os vocabulários dos loaders (R3 §4.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DependencyKind {
    /// Obrigatória: Fabric `depends`, Quilt `depends` sem `optional`, Forge `mandatory = true`,
    /// NeoForge `type = "required"`, `@Mod` `required-*`.
    Required,
    /// Opcional: Forge `mandatory = false`, NeoForge `type = "optional"`, Quilt `optional = true`,
    /// `@Mod` só com ordem. No Forge/NeoForge, presente fora da faixa também é erro.
    Optional,
    /// Fabric `recommends`: aviso se ausente ou fora da faixa.
    Recommends,
    /// Fabric `suggests`: só informativo.
    Suggests,
    /// Fabric e Quilt `breaks`: impede o carregamento se o outro estiver presente e na faixa.
    Breaks,
    /// Fabric `conflicts`: aviso se o outro estiver presente e na faixa.
    Conflicts,
    /// NeoForge `type = "incompatible"`: impede o carregamento.
    Incompatible,
    /// NeoForge `type = "discouraged"`: só avisa.
    Discouraged,
}

/// Relação declarada com outro mod.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Dependency {
    /// ID do outro mod (ou `minecraft`, `java`, `fabricloader`, `forge`, `neoforge`).
    pub id: String,
    /// Tipo da relação.
    pub kind: DependencyKind,
    /// Faixa de versões a que a relação se aplica.
    pub range: VersionRange,
    /// Lado em que vale.
    #[serde(default, skip_serializing_if = "is_default")]
    pub side: Side,
    /// Ordem de carregamento pedida.
    #[serde(default, skip_serializing_if = "is_default")]
    pub ordering: LoadOrdering,
    /// Motivo escrito pelo autor (NeoForge `reason`, Quilt `reason`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

fn is_default<T: Default + PartialEq>(value: &T) -> bool {
    *value == T::default()
}

/// Um mod declarado num descritor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModMetadata {
    /// Descritor de onde veio.
    pub source: DescriptorKind,
    /// Loader a que o descritor se destina.
    pub loader: Loader,
    /// ID do mod.
    pub id: String,
    /// Versão, com `${file.jarVersion}` já trocado pelo `Implementation-Version` do manifesto.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// Nome para exibição.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Descrição.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Autores.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub authors: Vec<String>,
    /// Licenças declaradas.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub licenses: Vec<String>,
    /// IDs alternativos que este mod também satisfaz (`provides`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub provides: Vec<String>,
    /// Lado em que o mod roda.
    #[serde(default, skip_serializing_if = "is_default")]
    pub environment: Environment,
    /// Relações com outros mods: na ordem dos campos do descritor e, dentro de um mapa
    /// (`depends`, `[[dependencies.<id>]]`...), pela ordem do arquivo ou por id quando o formato
    /// é um mapa sem ordem.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dependencies: Vec<Dependency>,
    /// Faixa de versões do Minecraft (da dependência `minecraft` ou do `mcversion`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub minecraft: Option<VersionRange>,
    /// Faixa de versões do loader (dependência `fabricloader`, `quilt_loader`, `forge` ou
    /// `neoforge`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub loader_version: Option<VersionRange>,
    /// Faixa de versões do Java (Fabric `depends.java`, NeoForge `[features] javaVersion`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub java: Option<VersionRange>,
    /// Configs de mixin declaradas pelo descritor.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub mixins: Vec<String>,
}

/// Linguagem do descritor `mods.toml` (`modLoader` e `loaderVersion`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LanguageLoader {
    /// Nome (`javafml`, `lowcodefml`, `kotlinforforge`...).
    pub name: String,
    /// Faixa de versões aceita da linguagem.
    pub version: VersionRange,
}

/// Dados do `META-INF/MANIFEST.MF` úteis para o diagnóstico.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManifestInfo {
    /// `FMLCorePlugin`: classe de coremod (Forge 1.7.10 a 1.12.2).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fml_core_plugin: Option<String>,
    /// `FMLCorePluginContainsFMLMod`: o coremod também traz um mod normal.
    #[serde(default, skip_serializing_if = "is_default")]
    pub fml_core_plugin_contains_mod: bool,
    /// `TweakClass` (`LaunchWrapper`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tweak_class: Option<String>,
    /// `MixinConfigs`, separados por vírgula.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub mixin_configs: Vec<String>,
    /// `FMLModType` (`MOD`, `LIBRARY`, `GAMELIBRARY`, `LANGPROVIDER`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fml_mod_type: Option<String>,
    /// `FMLAT`: access transformer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fml_access_transformer: Option<String>,
    /// `ContainedDeps` (Forge 1.12.2): jars embutidos em `META-INF/`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub contained_deps: Vec<String>,
    /// `Embedded-Dependencies-Mod`: jar embutido com o mod de verdade, quando o jar de cima é só
    /// um localizador do FML (Sinytra Connector 1.20.1, Kotlin for Forge).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub embedded_dependencies_mod: Option<String>,
    /// `Implementation-Title`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub implementation_title: Option<String>,
    /// `Implementation-Version` (fonte de `${file.jarVersion}`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub implementation_version: Option<String>,
    /// `Automatic-Module-Name`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub automatic_module_name: Option<String>,
    /// `Multi-Release: true`.
    #[serde(default, skip_serializing_if = "is_default")]
    pub multi_release: bool,
}

impl ManifestInfo {
    /// `true` se o jar se declara coremod ou tweaker (achado `W-LEGACY-COREMOD`).
    #[must_use]
    pub fn is_coremod(&self) -> bool {
        self.fml_core_plugin.is_some() || self.tweak_class.is_some()
    }
}

/// Versão das classes Java do jar (`major` 52 = Java 8, 61 = Java 17, 65 = Java 21).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClassVersion {
    /// Versão principal do formato de classe.
    pub major: u16,
    /// Versão secundária (65535 indica recursos de prévia).
    pub minor: u16,
}

impl ClassVersion {
    /// Versão do Java correspondente (`major - 44`; 52 → 8). `None` antes do Java 1.2.
    #[must_use]
    pub fn java_feature_version(self) -> Option<u16> {
        self.major.checked_sub(44).filter(|v| *v >= 2)
    }
}

/// Item de `META-INF/jarjar/metadata.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JarJarEntry {
    /// `identifier.group`.
    pub group: String,
    /// `identifier.artifact`.
    pub artifact: String,
    /// `version.artifactVersion`: versão do jar embutido.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artifact_version: Option<String>,
    /// `version.range`: faixa aceita (Maven).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub range: Option<VersionRange>,
    /// `isObfuscated`.
    #[serde(default, skip_serializing_if = "is_default")]
    pub obfuscated: bool,
}

/// Jar embutido, declarado por um descritor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NestedJar {
    /// Caminho dentro do jar que o contém.
    pub path: String,
    /// Quem o declarou (`fabric.mod.json` `jars[]`, `quilt.mod.json`, `jarjar/metadata.json`,
    /// `ContainedDeps` do manifesto).
    pub declared_by: DescriptorKind,
    /// Dados do `jarjar/metadata.json`, quando é dele.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub jarjar: Option<JarJarEntry>,
    /// Metadados lidos do jar embutido; `None` quando não deu para ler (ver `warnings` do pai).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Box<JarMetadata>>,
}

/// Problema não fatal encontrado na leitura. O jar continua sendo lido.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum WarningCode {
    /// Descritor com JSON inválido mesmo no modo tolerante; foi ignorado.
    InvalidJson,
    /// Descritor só foi lido no modo tolerante (vírgula sobrando, comentário, quebra de linha
    /// dentro de texto...).
    LenientJson,
    /// `mods.toml`/`neoforge.mods.toml` com TOML inválido; foi ignorado.
    InvalidToml,
    /// Texto que não era UTF-8 válido; lido como Windows-1252.
    NotUtf8,
    /// Campo obrigatório ausente (por exemplo, mod sem `id`); o mod foi ignorado.
    MissingField,
    /// Campo com tipo ou valor inesperado; foi ignorado.
    InvalidField,
    /// Faixa de versões que o loader recusaria.
    InvalidVersionRange,
    /// A versão ainda tem um marcador `${...}` que não deu para resolver.
    UnresolvedPlaceholder,
    /// Jar embutido declarado, mas ausente.
    MissingNestedJar,
    /// Jar embutido presente, mas ilegível (corrompido, grande demais ou profundo demais).
    UnreadableNestedJar,
    /// Arquivo de metadados maior que o limite; foi ignorado.
    EntryTooLarge,
    /// Entrada do zip ilegível (compressão não suportada, dados corrompidos).
    UnreadableEntry,
}

/// Aviso de leitura, com o caminho do arquivo dentro do jar.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Warning {
    /// Código estável.
    pub code: WarningCode,
    /// Arquivo dentro do jar (para jars embutidos, o caminho do arquivo no embutido).
    pub path: String,
    /// Detalhe técnico, em português.
    pub detail: String,
}

/// Tudo o que foi lido de um jar.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JarMetadata {
    /// Descritores presentes, na ordem de detecção da R3 §4.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub descriptors: Vec<DescriptorKind>,
    /// Mods declarados, na ordem dos descritores.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub mods: Vec<ModMetadata>,
    /// Linguagem do `mods.toml`/`neoforge.mods.toml`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language_loader: Option<LanguageLoader>,
    /// Dados do manifesto, se houver.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub manifest: Option<ManifestInfo>,
    /// Maior versão de classe encontrada fora de `META-INF/versions/`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub class_version: Option<ClassVersion>,
    /// Jars embutidos declarados.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub nested: Vec<NestedJar>,
    /// Problemas não fatais.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<Warning>,
}

impl JarMetadata {
    /// Mods que o loader do pack leria neste jar, pela ordem de preferência de cada loader:
    ///
    /// - Fabric: `fabric.mod.json`;
    /// - Quilt: `quilt.mod.json`, senão `fabric.mod.json`;
    /// - NeoForge: `neoforge.mods.toml`, senão `mods.toml` (NeoForge 1.20.1 a 20.4), senão
    ///   `fabric.mod.json` (mod Fabric carregado pelo Sinytra Connector);
    /// - Forge: `mods.toml`, senão `mcmod.info`, senão `fabric.mod.json` (Connector).
    ///
    /// Jar sem descritor do loader devolve vazio (achado `E-LOADER` do diagnóstico). Os mods dos
    /// jars embutidos ficam em [`JarMetadata::nested`].
    #[must_use]
    pub fn mods_for_loader(&self, loader: Loader) -> Vec<&ModMetadata> {
        let order: &[DescriptorKind] = match loader {
            Loader::Fabric => &[DescriptorKind::FabricModJson],
            Loader::Quilt => &[DescriptorKind::QuiltModJson, DescriptorKind::FabricModJson],
            Loader::NeoForge => &[
                DescriptorKind::NeoForgeModsToml,
                DescriptorKind::ModsToml,
                DescriptorKind::FabricModJson,
            ],
            Loader::Forge => &[
                DescriptorKind::ModsToml,
                DescriptorKind::McmodInfo,
                DescriptorKind::FabricModJson,
            ],
        };
        order
            .iter()
            .map(|kind| {
                self.mods
                    .iter()
                    .filter(|m| m.source == *kind)
                    .collect::<Vec<_>>()
            })
            .find(|mods| !mods.is_empty())
            .unwrap_or_default()
    }

    /// Como [`JarMetadata::mods_for_loader`], mas para jars que não declaram mod próprio e só
    /// carregam outros (o FML carrega os embutidos no lugar do jar de cima):
    ///
    /// 1. o jar apontado por `Embedded-Dependencies-Mod` no manifesto (Sinytra Connector 1.20.1);
    /// 2. no Forge e no NeoForge, os jars de `META-INF/jarjar/metadata.json` (bibliotecas com
    ///    `FMLModType: LIBRARY`, como o Kotlin for Forge).
    #[must_use]
    pub fn effective_mods_for_loader(&self, loader: Loader) -> Vec<&ModMetadata> {
        let own = self.mods_for_loader(loader);
        if !own.is_empty() {
            return own;
        }
        let nested_meta = |path: &str| {
            self.nested
                .iter()
                .filter(|n| n.path.trim_start_matches('/') == path.trim_start_matches('/'))
                .find_map(|n| n.metadata.as_deref())
        };
        if let Some(embedded) = self
            .manifest
            .as_ref()
            .and_then(|m| m.embedded_dependencies_mod.as_deref())
            .and_then(nested_meta)
        {
            let mods = embedded.effective_mods_for_loader(loader);
            if !mods.is_empty() {
                return mods;
            }
        }
        if matches!(loader, Loader::Forge | Loader::NeoForge) {
            return self
                .nested
                .iter()
                .filter(|n| n.declared_by == DescriptorKind::JarJarMetadata)
                .filter_map(|n| n.metadata.as_deref())
                .flat_map(|m| m.effective_mods_for_loader(loader))
                .collect();
        }
        Vec::new()
    }

    /// `true` se o jar traz descritores de mais de um loader.
    #[must_use]
    pub fn is_multi_loader(&self) -> bool {
        let mut loaders: Vec<Loader> = self.mods.iter().map(|m| m.loader).collect();
        loaders.sort_unstable();
        loaders.dedup();
        loaders.len() > 1
    }

    /// Percorre este jar e todos os embutidos (em profundidade), com o caminho de cada um
    /// (vazio para este jar; `a.jar!/b.jar` para embutidos de embutidos).
    #[must_use]
    pub fn walk(&self) -> Vec<(String, &Self)> {
        let mut out = vec![(String::new(), self)];
        let mut i = 0;
        while let Some((prefix, jar)) = out.get(i).map(|(p, j)| (p.clone(), *j)) {
            for nested in &jar.nested {
                if let Some(meta) = &nested.metadata {
                    let path = if prefix.is_empty() {
                        nested.path.clone()
                    } else {
                        format!("{prefix}!/{}", nested.path)
                    };
                    out.push((path, meta));
                }
            }
            i += 1;
        }
        out
    }
}
