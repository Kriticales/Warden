//! Subcomandos descobertos pelo `build.rs`: todo `tasks/<nome>.rs` vira `cargo xtask <nome>`
//! (veja `xtask/build.rs` para a convenção). Não há lista para editar.

include!(concat!(env!("OUT_DIR"), "/tasks_registry.rs"));
