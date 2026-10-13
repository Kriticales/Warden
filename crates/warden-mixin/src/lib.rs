//! Crate `warden-mixin`.
//!
//! Raio-x de mixins (camadas A e B da R5A §5; ARCHITECTURE §9.6; ADR-0034 e ADR-0056): o que
//! cada mod altera no código do jogo e quais mods alteram o mesmo ponto.
//!
//! - [`scan`]: camada A, as configs de mixin do loader do pack e os refmaps de cada jar (com
//!   os embutidos); camada B ([`class`]), as anotações das classes de mixin lidas com o
//!   `cafebabe`, sem decodificar o bytecode.
//! - [`options`]: opções que desligam mixins (descritores de outros mods, configs do pack e o
//!   dado curado `data/mixin-options.toml`).
//! - [`overlap`]: o cruzamento por ponto de aplicação, com regras, rebaixadores e elevadores.
//! - [`cache`]: um arquivo binário por pack em `cache/xray/`, com as leituras por hash do jar.
//! - [`pack`]: o pack inteiro (leitura paralela, cache, cruzamento) e o relatório de um mod.
//!
//! Implementação própria a partir do protótipo do spike S-R5-1 (sem o intermed, ADR-0056).
//! Nada aqui bloqueia o teste: a interface diz "alteram o mesmo ponto do jogo", nunca "são
//! incompatíveis". Entrada não confiável: jar malformado ou ofuscado vira um problema de
//! leitura daquele jar, nunca um erro do pack inteiro nem um pânico.
//!
//! ```no_run
//! use std::path::PathBuf;
//! use warden_jarmeta::{Limits, Loader};
//! use warden_mixin::overlap::CrossContext;
//! use warden_mixin::pack::{PackJar, PackXray, scan_pack};
//!
//! let jars = vec![PackJar { item: "mods/sodium.pw.toml".into(), path: PathBuf::from("mods/sodium.jar") }];
//! let scan = scan_pack(&jars, Loader::Fabric, None, &Limits::default());
//! let xray = PackXray::build(scan, &CrossContext::default());
//! for overlap in &xray.overlaps {
//!     println!("{} {} {:?}", overlap.target_class, overlap.method, overlap.risk);
//! }
//! ```

pub mod cache;
mod class;
mod error;
pub mod model;
pub mod options;
pub mod overlap;
pub mod pack;
mod refmap;
pub mod scan;

pub use error::MixinErrorCode;
pub use refmap::method_name;
