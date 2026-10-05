//! Crate `warden-java`: escolher, baixar e validar o Java certo para cada pack
//! (ARCHITECTURE §7.3; ADR-0012 e ADR-0029; ROADMAP L-01).
//!
//! - Política "o mais novo que funciona" com o motivo à vista: [`policy`], sobre a tabela de
//!   compatibilidade versionada em `data/compatibility.toml` ([`compat`]).
//! - Fontes: Adoptium Temurin JRE ([`adoptium`], padrão, conferido por SHA-256) e o runtime
//!   oficial da Mojang ([`mojang`], alternativa, conferido arquivo a arquivo por SHA-1).
//! - Instalação atômica em `shared/runtimes/<id>/` ([`store`]): extração em pasta temporária,
//!   validação executando o Java ([`probe`]) e renomeação no fim. Um download interrompido
//!   nunca deixa Java parcial instalado.
//! - O serviço [`JavaRuntimes`] junta tudo: `java_choice`, garantir o Java antes do teste,
//!   procurar atualizações (a antiga é removida quando nenhum jogo a usa), listar com os packs
//!   que usam cada Java e remover.
//!
//! Limites: esta crate não abre o jogo (é a `warden-launcher`, que recebe o Java pronto) e não
//! lê packs: quem chama passa a versão do Minecraft, o loader e a escolha do usuário. Só Java
//! de 64 bits, Windows e Linux.

pub mod adoptium;
pub mod archive;
pub mod compat;
mod error;
mod manager;
pub mod mojang;
pub mod policy;
pub mod probe;
mod runtime;
pub mod store;
mod version;

pub use compat::{CompatibilityTable, LoaderKind};
pub use error::{Error, JavaErrorCode, JavaSource, Result};
pub use manager::{
    JavaOverview, JavaRuntimes, JavaRuntimesConfig, PackJavaInput, PackJavaUnresolved, PackJavaUse,
    RuntimeLease, RuntimeRow, RuntimeUpdate, UpdateReport, stages,
};
pub use policy::{
    JavaChoice, JavaChoiceReason, JavaChoiceRequest, JavaDecision, JavaRequirement,
    VersionJavaRequirement,
};
pub use probe::{JavaInfo, JavaProbe, ProcessProbe};
pub use runtime::{Arch, InstalledRuntime, METADATA_FILE, Os, Platform, RuntimeId, RuntimeSource};
pub use version::{JavaVersion, MinecraftVersion, compare_dotted, loader_version_numbers};
