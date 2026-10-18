//! Crate `warden-diagnostics`.
//!
//! Regras pré-teste, análise pós-crash, redação de dados pessoais, grafo, nota de saúde e
//! travamentos (ARCHITECTURE §3 e §9).
//!
//! - [`model`]: o achado ([`Finding`]) com evidência obrigatória, usado por todas as regras
//!   (D-02; as regras pré-teste da D-01 usam o mesmo modelo).
//! - [`postcrash`]: análise dos logs de uma sessão ou de qualquer texto de log (D-02).
//! - [`console`]: console agrupado por mod, com repetições, stack traces recolhidos e ruído
//!   conhecido (D-09).
//! - [`graph`]: grafo de dependências e as consultas "quem depende", "por que está no pack" e
//!   bibliotecas sem uso (D-07).
//! - [`redact`]: redação de dados pessoais antes de qualquer envio (D-02).
//! - [`signature`] e [`crash_history`]: assinatura de travamento e travamentos agrupados pela
//!   causa (D-06).
//!
//! Limites: a crate não abre processos nem faz rede; recebe textos, pastas e metadados e
//! devolve achados.

pub mod console;
pub mod crash_history;
mod error;
pub mod graph;
pub mod model;
pub mod postcrash;
pub mod pretest;
pub mod redact;
pub mod signature;

pub use error::{DiagnosticsError, DiagnosticsErrorCode, Result};
pub use model::{
    Evidence, Finding, FindingSide, ItemRef, MAX_EXCERPT_CHARS, RuleCode, Severity, SideChoice,
    Source, SuggestedFix,
};
pub use postcrash::{
    Analysis, AnalysisContext, AnalyzedSource, LogLoader, LogSource, PostCrashFinding,
    SessionFiles, SessionOutcome, SourceKind, analyze, analyze_session, analyze_text,
    collect_session,
};
pub use redact::{ExtraRule, Placeholder, Redacted, RedactionProfile, Redactor, redact};
pub use signature::{CrashSignature, SIGNATURE_VERSION, signature};
