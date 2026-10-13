//! Console agrupado (ARCHITECTURE §9.8; SPEC T13 "Mostrar: Linha a linha · Agrupado por mod ·
//! Só problemas"; ROADMAP D-09).
//!
//! Recebe as linhas do jogo (as mesmas `LogLine` do supervisor, já lidas do `output.log`) em
//! lotes e mantém, de forma incremental:
//!
//! - a **atribuição** de cada linha a um mod ([`attribute`]): jar ou módulo no frame do stack
//!   trace, *handler* do mixin no Fabric, "Mixins in Stacktrace", config de mixin citada,
//!   logger e o índice pacote → mod da D-05; sem sinal, "Desconhecido"; jogo, loader e Java,
//!   "Jogo e loader";
//! - a **forma** de cada linha ([`normalize`]): números, coordenadas, UUIDs, hexadecimais, ids
//!   e caminhos viram marcadores, e linhas com a mesma forma viram um item com contagem,
//!   primeira e última hora;
//! - o **stack trace** como um item só (as linhas `at …` sem nível juntam-se à linha anterior,
//!   mesmo chegando no lote seguinte), com a primeira linha de mod;
//! - o **ruído conhecido** (`data/log-noise.toml`), que ganha o selo "comum, geralmente
//!   inofensivo" e fica fora de "Só problemas".
//!
//! A interface recebe os grupos já calculados (nada de agrupar no frontend) e pede os itens de
//! cada grupo aberto com limite, então um log de centenas de milhares de linhas não pesa na
//! tela. "Copiar" devolve as linhas originais, remontadas a partir da forma e dos valores de
//! cada ocorrência. Nada disso altera o log gravado.

mod attribute;
mod group;
mod noise;
mod normalize;

use serde::{Deserialize, Serialize};

pub use attribute::{ModDirectory, Owner};
pub use group::{ConsoleGrouper, LinesTarget};

/// Nível de uma linha (o do log4j).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, specta::Type,
)]
#[serde(rename_all = "camelCase")]
pub enum LineLevel {
    /// `TRACE`.
    Trace,
    /// `DEBUG`.
    Debug,
    /// `INFO`.
    Info,
    /// `WARN`.
    Warn,
    /// `ERROR`.
    Error,
    /// `FATAL`.
    Fatal,
}

/// Uma linha do jogo para o agrupador.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LogEntry<'a> {
    /// Número da linha na sessão (crescente).
    pub seq: u32,
    /// Nível (sem nível: texto puro, como as linhas de um stack trace).
    pub level: Option<LineLevel>,
    /// Logger (`net.minecraft.client.Minecraft`, `minecraft/Minecraft`).
    pub logger: Option<&'a str>,
    /// Hora escrita na linha de texto (`19:53:26`).
    pub time: Option<&'a str>,
    /// Carimbo do evento XML (milissegundos desde 1970).
    pub timestamp_ms: Option<i64>,
    /// A mensagem (num evento XML com exceção, já com o stack trace).
    pub text: &'a str,
}

/// Hora de uma ocorrência: o carimbo (eventos XML) ou a hora escrita pelo jogo.
#[derive(Debug, Clone, Default, PartialEq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct Stamp {
    /// Milissegundos desde 1970.
    pub at_ms: Option<f64>,
    /// `HH:MM:SS` escrito pelo jogo.
    pub time: Option<String>,
}

/// Gravidade de um item no console agrupado.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum ItemSeverity {
    /// `ERROR`, `FATAL` ou stack trace sem nível.
    Error,
    /// `WARN`.
    Warning,
    /// O resto.
    Info,
}

/// Tipo de grupo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum GroupKind {
    /// Um item do pack.
    Item,
    /// Um mod citado que o índice do pack não conhece.
    Mod,
    /// "Jogo e loader".
    Game,
    /// "Desconhecido".
    Unknown,
}

/// Um grupo (um mod, "Jogo e loader" ou "Desconhecido").
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct GroupSummary {
    /// Chave estável (`item:mods/sodium.pw.toml`, `mod:epicfight`, `game`, `unknown`).
    pub key: String,
    /// Tipo.
    pub kind: GroupKind,
    /// O metafile do item do pack, quando é um item.
    pub item: Option<String>,
    /// Id do mod que deu o sinal, quando houver.
    pub mod_id: Option<String>,
    /// Entradas (linhas e stack traces) do grupo.
    pub entries: u32,
    /// Entradas de erro (com os stack traces).
    pub errors: u32,
    /// Entradas de aviso.
    pub warnings: u32,
    /// Entradas que fazem parte de um item repetido.
    pub repeated: u32,
    /// Itens distintos (os que casam com a busca, quando há busca).
    pub items: u32,
}

/// Um item: uma linha (ou forma de linha) com as repetições.
#[derive(Debug, Clone, PartialEq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ItemView {
    /// Id do item nesta sessão (para abrir e copiar).
    pub id: u32,
    /// Chave do grupo.
    pub group: String,
    /// Gravidade.
    pub severity: ItemSeverity,
    /// Nível escrito no log.
    pub level: Option<LineLevel>,
    /// Logger.
    pub logger: Option<String>,
    /// A primeira linha: a original, se aparece uma vez; a forma (`Missing texture for {id}`),
    /// se repete.
    pub text: String,
    /// Quantas vezes apareceu.
    pub count: u32,
    /// Primeira vez.
    pub first: Stamp,
    /// Última vez.
    pub last: Stamp,
    /// Id da regra de ruído conhecido ("comum, geralmente inofensivo").
    pub noise: Option<String>,
    /// É um stack trace.
    pub trace: bool,
    /// Linhas além da primeira (o stack trace recolhido).
    pub extra_lines: u32,
    /// A primeira linha de mod do stack trace (pulando Java, jogo e loader).
    pub mod_frame: Option<String>,
}

/// Uma linha original (para copiar e para abrir um stack trace).
#[derive(Debug, Clone, PartialEq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct OriginalLine {
    /// Número da linha na sessão.
    pub seq: u32,
    /// Nível.
    pub level: Option<LineLevel>,
    /// Logger.
    pub logger: Option<String>,
    /// Hora.
    pub stamp: Stamp,
    /// O texto como saiu (com o stack trace, quando houver).
    pub text: String,
}
