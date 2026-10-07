//! Arestas inferidas, guardadas por pack (ARCHITECTURE §9.6).
//!
//! A busca do culpado (D-12) e a análise do log descobrem dependências que o jar não declara
//! ("o mod A só funciona com o B"). Elas valem para o pack, não para a sessão: ficam num arquivo
//! JSON por pack e entram no grafo como ligações do tipo [`crate::graph::RelationKind::Inferred`].
//! Quem escolhe onde o arquivo mora é o chamador (o app grava em `<dados>/graph/<pack>.json`).

use std::fs;
use std::io::ErrorKind;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::{DiagnosticsError, Result};

/// De onde veio a inferência.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, specta::Type,
)]
#[serde(rename_all = "camelCase")]
pub enum InferredSource {
    /// Busca do culpado (bisseção).
    Bisect,
    /// Análise de um log ou crash report.
    Log,
}

/// Uma dependência não declarada: `from` só funciona com `to` no pack.
#[derive(
    Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, specta::Type,
)]
#[serde(rename_all = "camelCase")]
pub struct InferredEdge {
    /// Caminho do item que precisa.
    pub from: String,
    /// Caminho do item de que precisa.
    pub to: String,
    /// Origem da inferência.
    pub source: InferredSource,
    /// Nota curta para mostrar ("a busca do culpado travou sem este mod").
    #[serde(default)]
    pub note: String,
}

/// O arquivo de arestas inferidas de um pack.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InferredStore {
    /// Versão do formato.
    #[serde(default = "schema")]
    pub schema: u32,
    /// As arestas, sem repetir `from` + `to`.
    #[serde(default)]
    pub edges: Vec<InferredEdge>,
}

fn schema() -> u32 {
    1
}

impl InferredStore {
    /// Lê o arquivo. Arquivo ausente = nenhuma aresta; arquivo ilegível ou com JSON inválido é
    /// erro (o chamador registra e segue sem arestas inferidas).
    pub fn load(path: &Path) -> Result<Self> {
        let text = match fs::read_to_string(path) {
            Ok(text) => text,
            Err(error) if error.kind() == ErrorKind::NotFound => return Ok(Self::default()),
            Err(error) => return Err(DiagnosticsError::io(path, error)),
        };
        serde_json::from_str(&text).map_err(|error| {
            DiagnosticsError::Internal(format!("arestas inferidas ilegíveis em {path:?}: {error}"))
        })
    }

    /// Grava o arquivo por inteiro (escreve num temporário ao lado e troca, para não deixar
    /// um arquivo pela metade).
    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| DiagnosticsError::io(parent, error))?;
        }
        let text = serde_json::to_string_pretty(self)
            .map_err(|error| DiagnosticsError::Internal(error.to_string()))?;
        let temp = path.with_extension("json.tmp");
        fs::write(&temp, text).map_err(|error| DiagnosticsError::io(&temp, error))?;
        fs::rename(&temp, path).map_err(|error| DiagnosticsError::io(path, error))
    }

    /// Acrescenta (ou troca a nota e a origem de) uma aresta. Devolve `true` se era nova.
    pub fn add(&mut self, edge: InferredEdge) -> bool {
        if let Some(known) = self
            .edges
            .iter_mut()
            .find(|known| known.from == edge.from && known.to == edge.to)
        {
            *known = edge;
            return false;
        }
        self.edges.push(edge);
        true
    }

    /// Tira as arestas que citam o item (quando ele sai do pack).
    pub fn forget_item(&mut self, path: &str) {
        self.edges
            .retain(|edge| edge.from != path && edge.to != path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn edge(from: &str, to: &str) -> InferredEdge {
        InferredEdge {
            from: from.into(),
            to: to.into(),
            source: InferredSource::Bisect,
            note: "nota".into(),
        }
    }

    #[test]
    fn arquivo_ausente_e_vazio_e_grava_e_le_de_volta() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("graph").join("pack.json");
        assert_eq!(
            InferredStore::load(&file).unwrap(),
            InferredStore::default()
        );

        let mut store = InferredStore::default();
        assert!(store.add(edge("mods/a.pw.toml", "mods/b.pw.toml")));
        store.save(&file).unwrap();
        let back = InferredStore::load(&file).unwrap();
        assert_eq!(back, store);
        assert!(!file.with_extension("json.tmp").exists());
    }

    #[test]
    fn repetir_a_aresta_troca_em_vez_de_duplicar() {
        let mut store = InferredStore::default();
        assert!(store.add(edge("a", "b")));
        let mut again = edge("a", "b");
        again.source = InferredSource::Log;
        assert!(!store.add(again));
        assert_eq!(store.edges.len(), 1);
        assert_eq!(store.edges[0].source, InferredSource::Log);
    }

    #[test]
    fn esquecer_o_item_tira_as_duas_pontas() {
        let mut store = InferredStore::default();
        store.add(edge("a", "b"));
        store.add(edge("c", "a"));
        store.add(edge("c", "d"));
        store.forget_item("a");
        assert_eq!(store.edges, vec![edge("c", "d")]);
    }

    #[test]
    fn json_invalido_e_erro_e_formato_antigo_sem_campos_extras_e_lido() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("pack.json");
        fs::write(&file, "{ nao e json").unwrap();
        assert!(InferredStore::load(&file).is_err());
        fs::write(&file, r#"{"edges":[{"from":"a","to":"b","source":"log"}]}"#).unwrap();
        let store = InferredStore::load(&file).unwrap();
        assert_eq!(store.schema, 1);
        assert_eq!(store.edges[0].note, "");
    }
}
