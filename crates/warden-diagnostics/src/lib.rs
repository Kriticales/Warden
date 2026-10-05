//! Crate `warden-diagnostics`.
//!
//! Regras pré-teste, análise pós-crash, redação de dados pessoais, grafo, nota de saúde e
//! travamentos (ARCHITECTURE §3 e §9).
//!
//! - [`model`]: o achado ([`Finding`]) com evidência obrigatória, usado por todas as regras
//!   (D-02; as regras pré-teste da D-01 usam o mesmo modelo).
//! - [`postcrash`]: análise dos logs de uma sessão ou de qualquer texto de log (D-02).
//! - [`redact`]: redação de dados pessoais antes de qualquer envio (D-02).
//!
//! Limites: a crate não abre processos nem faz rede; recebe textos, pastas e metadados e
//! devolve achados.

mod error;
pub mod model;
pub mod postcrash;
pub mod redact;

pub use error::{DiagnosticsError, DiagnosticsErrorCode, Result};
pub use model::{
    Evidence, Finding, ItemRef, MAX_EXCERPT_CHARS, RuleCode, Severity, SideChoice, Source,
    SuggestedFix,
};
pub use postcrash::{
    Analysis, AnalysisContext, AnalyzedSource, LogLoader, LogSource, PostCrashFinding,
    SessionFiles, SessionOutcome, SourceKind, analyze, analyze_session, analyze_text,
    collect_session,
};
pub use redact::{ExtraRule, Placeholder, Redacted, RedactionProfile, Redactor, redact};
