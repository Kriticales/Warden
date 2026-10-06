//! Índices pacote → jar → mod e config de mixin → mod (ARCHITECTURE §9.6; R5A §7.1 e §5.2).
//!
//! Respondem "de quem é esta classe?" e "de quem é esta config de mixin?" para o console
//! agrupado, o padrão de falha de Mixin do catálogo, a dependência inferida da busca do culpado
//! e o agrupamento dos perfis do spark.
//!
//! Duas camadas:
//!
//! - [`JarIndex`]: o índice de **um** jar (com os embutidos), que só depende dos bytes do
//!   arquivo. Fica no cache em `cache/jarindex/<sha256 do jar>.json` ([`JarIndexCache`]); a
//!   segunda passagem com o mesmo pack não abre nenhum jar.
//! - [`PackIndex`]: os índices dos jars de um pack juntos, com as consultas. Um pacote que
//!   aparece em mais de um jar do mesmo item (módulos da Fabric API que repetem um pacote) fica
//!   com o jar de topo, que é o mod que o usuário adicionou; entre itens diferentes não há como
//!   escolher, e a consulta diz quais são os candidatos.
//!
//! Os jars embutidos seguidos são os que o loader carrega (os mesmos de
//! [`JarMetadata::nested`](crate::JarMetadata)): `jars[]` do Fabric e do Quilt,
//! `META-INF/jarjar/metadata.json`, `ContainedDeps` e `Embedded-Dependencies-Mod`.
//!
//! As configs de mixin vêm de `fabric.mod.json` (`mixins`), `quilt.mod.json` (`mixin`),
//! `[[mixins]]` do `neoforge.mods.toml` (ou do `mods.toml`) e `MixinConfigs` do
//! `MANIFEST.MF` (Forge).
//!
//! ```no_run
//! use std::path::PathBuf;
//! use warden_jarmeta::Limits;
//! use warden_jarmeta::index::{JarIndexCache, PackJar, build_pack_index};
//!
//! let cache = JarIndexCache::new(PathBuf::from("dados/cache/jarindex"));
//! let jars = vec![PackJar::new("mods/epic-fight.pw.toml", "instancia/mods/epicfight.jar")];
//! let build = build_pack_index(&jars, None, Some(&cache), &Limits::default());
//! if let Some(owner) = build.index.mixin_owner("epicfight.mixins.json") {
//!     println!("{} ({:?})", owner.owner.item, owner.owner.mod_id);
//! }
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

mod build;
mod cache;
mod pack;
mod query;

use serde::{Deserialize, Serialize};

use crate::model::{DescriptorKind, Loader};

pub use build::{index_jar_bytes, index_jar_file, sha256_hex};
pub use cache::{CACHE_DIR_NAME, JarIndexCache};
pub use pack::{BuildStats, JarFailure, PackIndexBuild, PackJar, build_pack_index};
pub use query::{JarOwner, MixinOwner, Ownership, PackIndex};

/// Versão do formato serializado de [`JarIndex`]. Um cache de outra versão é ignorado e refeito.
pub const INDEX_FORMAT: u32 = 1;

/// Índice de um jar e dos jars embutidos nele. Só depende dos bytes do arquivo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JarIndex {
    /// [`INDEX_FORMAT`] de quando foi montado.
    pub format: u32,
    /// SHA-256 do arquivo do jar, em hexadecimal minúsculo.
    pub sha256: String,
    /// O jar de cima (sempre o primeiro, com caminho vazio) e os embutidos, em profundidade.
    pub jars: Vec<IndexedJar>,
    /// Configs de mixin declaradas pelos descritores de todos os jars.
    pub mixin_configs: Vec<MixinConfigEntry>,
}

/// Um jar dentro do índice.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexedJar {
    /// Caminho dentro do jar de cima: vazio para ele mesmo; `META-INF/jars/a.jar` para um
    /// embutido; `a.jar!/b.jar` para um embutido de embutido (o mesmo de
    /// [`JarMetadata::walk`](crate::JarMetadata::walk)).
    pub path: String,
    /// Posição do jar que o contém em [`JarIndex::jars`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<u32>,
    /// Mods declarados no jar, na ordem dos descritores.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub mods: Vec<IndexedMod>,
    /// Artefato do `META-INF/jarjar/metadata.json` que declarou este embutido
    /// (`grupo:artefato`), para reconhecer cópias de uma biblioteca sem descritor de mod.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artifact: Option<String>,
    /// Versão do artefato do jarjar.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artifact_version: Option<String>,
    /// Pacotes Java com pelo menos uma classe neste jar (`net.fabricmc.fabric.api.util`), em
    /// ordem. Classes de `META-INF/versions/<n>/` contam no pacote normal.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub packages: Vec<String>,
}

impl IndexedJar {
    /// O mod que representa este jar para o loader dado: o primeiro que o loader leria e, se
    /// nenhum, o primeiro declarado.
    #[must_use]
    pub fn primary_mod(&self, loader: Option<Loader>) -> Option<&str> {
        self.primary(loader).map(|m| m.id.as_str())
    }

    fn primary(&self, loader: Option<Loader>) -> Option<&IndexedMod> {
        loader
            .and_then(|l| self.mods.iter().find(|m| m.loaders.contains(&l)))
            .or_else(|| self.mods.first())
    }

    /// O que identifica o conteúdo do jar entre cópias: o mod principal ou, sem mod, o
    /// artefato do jarjar. Duas cópias da mesma coisa (o `fabric-api-base` embutido no Sodium e
    /// na Fabric API) são um mod só para o loader, que carrega uma delas.
    pub(crate) fn identity(&self, loader: Option<Loader>) -> Option<(&str, Option<&str>)> {
        if let Some(module) = self.primary(loader) {
            return Some((module.id.as_str(), module.version.as_deref()));
        }
        self.artifact
            .as_deref()
            .map(|artifact| (artifact, self.artifact_version.as_deref()))
    }
}

/// Mod declarado num jar.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexedMod {
    /// Id do mod.
    pub id: String,
    /// Versão declarada.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// Descritor de onde veio.
    pub source: DescriptorKind,
    /// Loaders que leriam este mod neste jar (pela preferência de descritores de
    /// [`JarMetadata::mods_for_loader`](crate::JarMetadata::mods_for_loader)).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub loaders: Vec<Loader>,
}

/// Uma config de mixin declarada.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MixinConfigEntry {
    /// Nome da config como o descritor escreve (`epicfight.mixins.json`), que é o mesmo que o
    /// Mixin escreve no log.
    pub config: String,
    /// Posição do jar em [`JarIndex::jars`].
    pub jar: u32,
    /// Descritor que declarou.
    pub declared_in: DescriptorKind,
    /// Mod do descritor que declarou. Vazio no `MixinConfigs` do manifesto, que vale para o jar
    /// inteiro: aí o dono é o mod principal do jar ([`IndexedJar::primary_mod`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mod_id: Option<String>,
    /// O arquivo da config existe em algum jar deste índice (o Sodium para NeoForge declara as
    /// configs no jar de cima e guarda os arquivos no embutido). Uma config declarada e ausente
    /// faz o jogo travar ao carregar, mas o dono continua sendo quem declarou.
    pub present: bool,
}
