//! Crate `warden-jarmeta`.
//!
//! Lê os metadados de mods direto dos jars, sem extrair nada para o disco (R3 §4; ARCHITECTURE
//! §3): `fabric.mod.json` (com `jars[]` recursivo e `provides`), `quilt.mod.json` (para
//! identificar), `META-INF/mods.toml`, `META-INF/neoforge.mods.toml`,
//! `META-INF/jarjar/metadata.json`, `mcmod.info` tolerante, `META-INF/MANIFEST.MF` (coremods) e a
//! versão das classes Java. Tudo vira o modelo normalizado de [`JarMetadata`] (R3 §4.5).
//!
//! Também traz os avaliadores de faixa nos três dialetos ([`VersionRange`]: Fabric, Maven e
//! `@Mod.dependencies` em texto) e o `FlexVer` ([`version::flexver`]).
//!
//! Limites: a crate não decide nada sobre o pack (isso é do diagnóstico, `warden-diagnostics`),
//! não acessa a rede e não guarda cache (os índices com cache por hash são da tarefa D-05).
//! Entrada é tratada como não confiável: nenhum pânico, nenhum arquivo descompactado além dos
//! [`Limits`].
//!
//! ```no_run
//! use std::path::Path;
//! use warden_jarmeta::{Limits, Loader, read_jar_file};
//!
//! let meta = read_jar_file(Path::new("mods/sodium.jar"), &Limits::default())?;
//! for module in meta.mods_for_loader(Loader::Fabric) {
//!     let accepts = module
//!         .minecraft
//!         .as_ref()
//!         .map_or(Ok(true), |range| range.matches("1.20.1"))?;
//!     println!("{} {:?} aceita 1.20.1: {accepts}", module.id, module.version);
//! }
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

mod descriptor;
mod error;
mod lenient_json;
mod limits;
mod model;
mod range;
mod read;
mod text;
pub mod version;

pub use error::{Error, JarmetaErrorCode, LimitKind};
pub use limits::Limits;
pub use model::{
    ClassVersion, Dependency, DependencyKind, DescriptorKind, Environment, JarJarEntry,
    JarMetadata, LanguageLoader, LoadOrdering, Loader, ManifestInfo, ModMetadata, NestedJar, Side,
    Warning, WarningCode,
};
pub use range::{RangeDialect, RangeError, VersionRange};
pub use read::{read_jar_bytes, read_jar_file};
pub use version::maven::MavenFlavor;
pub use version::mod_annotation::{ModAnnotationDependencies, parse_mod_dependencies};
