//! Texto de `@Mod(dependencies = "...")` do Forge 1.7.10 a 1.12.2.
//!
//! Formato: itens separados por `;`, cada um `instruções:alvo[@faixa]`, como
//! `required-after:forge@[14.23.5.2847,);after:jei`. As instruções, separadas por `-`, são
//! `client`/`server` (lado), `required` e `before`/`after` (ordem). A faixa é Maven.
//!
//! Comportamento do `DependencyParser` do Forge 1.12.x
//! (<https://github.com/MinecraftForge/MinecraftForge>, branch `1.12.x`, LGPL-2.1), reescrito a
//! partir da leitura; o 1.7.10 aceita um subconjunto (`required-before`, `required-after`,
//! `before`, `after`). Ler a anotação do bytecode fica para o backlog (ROADMAP P1-06); aqui só o
//! texto.

use crate::model::{Dependency, DependencyKind, LoadOrdering, Side};
use crate::range::VersionRange;
use crate::version::maven::MavenRange;

/// Resultado da leitura: as dependências válidas e os itens recusados (com o motivo).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ModAnnotationDependencies {
    /// Dependências lidas, na ordem do texto.
    pub dependencies: Vec<Dependency>,
    /// Itens que o Forge recusaria: `(item, motivo)`.
    pub rejected: Vec<(String, String)>,
}

/// Lê o texto de `@Mod.dependencies`. Itens inválidos vão para `rejected` sem interromper.
#[must_use]
pub fn parse_mod_dependencies(text: &str) -> ModAnnotationDependencies {
    let mut out = ModAnnotationDependencies::default();
    for item in text.split(';').map(str::trim).filter(|s| !s.is_empty()) {
        match parse_item(item) {
            Ok(Some(dep)) => out.dependencies.push(dep),
            Ok(None) => {}
            Err(reason) => out.rejected.push((item.to_owned(), reason)),
        }
    }
    out
}

fn parse_item(item: &str) -> Result<Option<Dependency>, String> {
    let parts: Vec<&str> = item
        .split(':')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect();
    let [instructions, target] = parts.as_slice() else {
        return Err("o item precisa de duas partes separadas por ':'".into());
    };
    let target_is_all = target.starts_with('*');
    if target_is_all && target.len() > 1 {
        return Err("'*' não pode vir com outro texto".into());
    }
    let mut side: Option<Side> = None;
    let mut required = false;
    let mut ordering: Option<LoadOrdering> = None;
    for instruction in instructions
        .split('-')
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        match instruction {
            "client" | "server" => {
                if side.is_some() {
                    return Err("só um lado (client ou server) pode ser indicado".into());
                }
                side = Some(if instruction == "client" {
                    Side::Client
                } else {
                    Side::Server
                });
            }
            "required" => {
                if required {
                    return Err("'required' só pode aparecer uma vez".into());
                }
                if target_is_all {
                    return Err("não dá para exigir tudo (*)".into());
                }
                required = true;
            }
            "before" | "after" => {
                if ordering.is_some() {
                    return Err("'before' ou 'after' só pode aparecer uma vez".into());
                }
                ordering = Some(if instruction == "before" {
                    LoadOrdering::Before
                } else {
                    LoadOrdering::After
                });
            }
            other => return Err(format!("instrução desconhecida '{other}'")),
        }
    }
    let mut pieces = target.split('@');
    let id = pieces.next().unwrap_or_default().trim();
    let range_text = pieces.next();
    if pieces.next().is_some() {
        return Err("mais de um '@' no alvo".into());
    }
    if target_is_all && range_text.is_some() {
        return Err("não dá para ter faixa de versão com '*'".into());
    }
    if id.is_empty() {
        return Err("alvo vazio".into());
    }
    if !required && ordering.is_none() {
        return Err("é preciso 'required', 'before' ou 'after'".into());
    }
    let range = match range_text {
        None => VersionRange::Any,
        Some(spec) => {
            MavenRange::parse(spec).map_err(|e| format!("faixa inválida: {e}"))?;
            VersionRange::maven(spec)
        }
    };
    if target_is_all {
        // Só ordem em relação a todos os mods: não é dependência de ninguém.
        return Ok(None);
    }
    Ok(Some(Dependency {
        id: id.to_owned(),
        kind: if required {
            DependencyKind::Required
        } else {
            DependencyKind::Optional
        },
        range,
        side: side.unwrap_or_default(),
        ordering: ordering.unwrap_or_default(),
        reason: None,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn dependencias_tipicas_do_forge_1_12() {
        let parsed = parse_mod_dependencies(
            "required-after:forge@[14.23.5.2847,);after:jei;required-before:baubles@[1.5.2,);\
             client-required-after:ctm",
        );
        assert!(parsed.rejected.is_empty(), "{:?}", parsed.rejected);
        let deps = parsed.dependencies;
        assert_eq!(deps.len(), 4);
        assert_eq!(deps[0].id, "forge");
        assert_eq!(deps[0].kind, DependencyKind::Required);
        assert_eq!(deps[0].ordering, LoadOrdering::After);
        assert_eq!(deps[0].range, VersionRange::maven("[14.23.5.2847,)"));
        assert_eq!(deps[1].id, "jei");
        assert_eq!(deps[1].kind, DependencyKind::Optional);
        assert_eq!(deps[1].range, VersionRange::Any);
        assert_eq!(deps[2].ordering, LoadOrdering::Before);
        assert_eq!(deps[3].side, Side::Client);
    }

    #[test]
    fn itens_recusados_nao_interrompem() {
        let parsed = parse_mod_dependencies(
            "required-after:Forge@[10.13.4.1614,);bogus;after:*;required-after:*;\
             sideways:x;after:a@[1.0;client-server-after:y;after:@[1,);required:z",
        );
        let ids: Vec<&str> = parsed.dependencies.iter().map(|d| d.id.as_str()).collect();
        assert_eq!(ids, ["Forge", "z"]);
        assert_eq!(parsed.rejected.len(), 6, "{:?}", parsed.rejected);
        assert!(parse_mod_dependencies("").dependencies.is_empty());
        assert!(parse_mod_dependencies(" ; ;").rejected.is_empty());
        let parsed = parse_mod_dependencies("after:*@[1,);before:*x;required-required-after:a");
        assert_eq!(parsed.rejected.len(), 3);
        let parsed = parse_mod_dependencies("before-after:a;client:b;a@b@c:x;after:a@1@2");
        assert_eq!(parsed.rejected.len(), 4);
    }

    proptest! {
        #[test]
        fn nunca_entra_em_panico(text in ".{0,60}") {
            let _ = parse_mod_dependencies(&text);
        }
    }
}
