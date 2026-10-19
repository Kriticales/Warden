//! O texto inicial do consentimento (ARCHITECTURE §9.5 "Consentimento e transparência"; SPEC
//! T14 ✦ passo 1; CA-T14-03): o sintoma, o que a pessoa contou, a visão geral do pack, os achados
//! do Warden e o que foi escolhido para analisar (o último travamento, outra sessão, um log do
//! computador ou só o pack), **já redigido**. É exatamente o texto que o consentimento mostra e
//! que vai na primeira mensagem.
//!
//! E o retrato do pack no começo da conversa ([`snapshot`]), para avisar ao continuar que o pack
//! mudou (CA-T14-15).

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::Path;

use warden_ai::conversation::{AiSymptom, ConversationOrigin, PackSnapshot};
use warden_launcher::outcome::Outcome;

use super::read::read_log_text;
use super::{LOCAL_FILE_SESSION, PackTools};
use crate::error::AppError;
use crate::test_session::TestSessionSummary;

/// Linhas do log que entram no texto inicial.
const LOG_EXCERPT: usize = 40;

/// O que analisar, já resolvido.
#[derive(Debug, Clone)]
pub(crate) enum Subject {
    /// Uma sessão do Testar.
    Session {
        /// A sessão.
        summary: Box<TestSessionSummary>,
        /// É o último travamento.
        last_crash: bool,
    },
    /// Um log do computador: o nome e o texto já lido (até 20 MB).
    LocalFile {
        /// Nome do arquivo, sem a pasta.
        file_name: String,
        /// O texto lido.
        raw: String,
    },
    /// Só o pack.
    PackOnly,
}

/// O texto inicial pronto.
#[derive(Debug, Clone)]
pub(crate) struct Initial {
    /// O texto exato que vai à IA.
    pub(crate) text: String,
    /// A origem gravada na conversa.
    pub(crate) origin: ConversationOrigin,
    /// O log do computador redigido (as ferramentas de log leem dele).
    pub(crate) local_log: Option<String>,
}

pub(super) fn symptom_text(symptom: AiSymptom) -> &'static str {
    match symptom {
        AiSymptom::CrashOnStart => "o jogo trava ao abrir (fecha antes do menu principal)",
        AiSymptom::CrashInWorld => {
            "o jogo trava ao entrar no mundo (abre, mas fecha ao carregar ou jogar)"
        }
        AiSymptom::Slow => "o jogo está lento (quadros por segundo baixos ou travadinhas)",
        AiSymptom::Other => "outra coisa (veja o que a pessoa contou)",
    }
}

/// Linhas que parecem erro (para o trecho do texto inicial).
fn is_error_line(line: &str) -> bool {
    let lower = line.to_lowercase();
    ["error", "fatal", "exception", "caused by", "crash"]
        .iter()
        .any(|word| lower.contains(word))
}

/// O trecho do log: as últimas linhas com erro, com o número da linha.
pub(super) fn excerpt(text: &str) -> String {
    let lines: Vec<(usize, &str)> = text
        .lines()
        .enumerate()
        .filter(|(_, line)| is_error_line(line))
        .collect();
    let start = lines.len().saturating_sub(LOG_EXCERPT);
    let mut out = String::new();
    for (index, line) in &lines[start..] {
        let _ = writeln!(out, "L{}: {line}", index + 1);
    }
    if out.is_empty() {
        out.push_str("(nenhuma linha com erro)\n");
    }
    out
}

/// A assinatura do travamento gravada na sessão (D-06), se houver.
fn signature_of(tools: &PackTools<'_>, session: &str) -> Option<String> {
    let dir = crate::test_session::session_dir(&tools.state.paths, tools.pack_id, session).ok()?;
    let record = dir.read_record().ok()?;
    record
        .extra
        .get(crate::commands::crashes::SESSION_FIELD)?
        .pointer("/signature/hash")?
        .as_str()
        .map(ToOwned::to_owned)
}

/// O texto de uma ferramenta, os itens um por linha (ou a mensagem de erro dela).
async fn tool_text(tools: &PackTools<'_>, name: &str, args: &serde_json::Value) -> String {
    super::read::call(tools, name, args).await.map_or_else(
        |message| message,
        |output| {
            output
                .items
                .into_iter()
                .map(|item| item.text)
                .collect::<Vec<_>>()
                .join("\n")
        },
    )
}

/// Monta o texto inicial.
pub(crate) async fn build(
    tools: &PackTools<'_>,
    symptom: AiSymptom,
    subject: Subject,
    note: Option<&str>,
) -> Result<Initial, AppError> {
    let mut text = String::new();
    let _ = writeln!(text, "Sintoma: {}.", symptom_text(symptom));
    if let Some(note) = note.map(str::trim).filter(|note| !note.is_empty()) {
        let _ = writeln!(text, "O que a pessoa contou: {note}");
    }
    let overview = tool_text(tools, "get_pack_overview", &serde_json::json!({})).await;
    let _ = writeln!(text, "\n{overview}");

    let findings = super::read::call(tools, "get_findings", &serde_json::json!({}))
        .await
        .map(|output| output.items)
        .unwrap_or_default();
    let real: Vec<_> = findings
        .iter()
        .filter(|item| item.id != "finding:nenhum")
        .collect();
    if real.is_empty() {
        let _ = writeln!(text, "\nAchados do Warden: nenhum.");
    } else {
        let _ = writeln!(
            text,
            "\nAchados do Warden: {} (consulte get_findings).",
            real.len()
        );
        for item in real.iter().take(10) {
            let _ = writeln!(text, "- {}", item.id);
        }
    }

    let (origin, local_log) = subject_section(tools, subject, &mut text).await?;
    // Tudo redigido de novo no fim (o texto inteiro revela nomes que um pedaço sozinho não
    // revela).
    let text = tools.redactor.redact(text.trim_end()).text;
    Ok(Initial {
        text,
        origin,
        local_log,
    })
}

/// A parte "Analisar: …" do texto, a origem gravada e o log do computador redigido.
async fn subject_section(
    tools: &PackTools<'_>,
    subject: Subject,
    text: &mut String,
) -> Result<(ConversationOrigin, Option<String>), AppError> {
    Ok(match subject {
        Subject::Session {
            summary,
            last_crash,
        } => {
            let what = if last_crash {
                "o último travamento"
            } else if summary.outcome == Outcome::Crashed {
                "um teste que travou"
            } else {
                "um teste"
            };
            let line = tool_text(
                tools,
                "get_session",
                &serde_json::json!({ "session_id": summary.id }),
            )
            .await;
            let _ = writeln!(text, "\nAnalisar: {what} (sessão {}).\n{line}", summary.id);
            let dir =
                crate::test_session::session_dir(&tools.state.paths, tools.pack_id, &summary.id)?;
            let log = read_log_text(&dir.output_log()).unwrap_or_default();
            let _ = write!(
                text,
                "\nÚltimas linhas com erro do log da sessão (sem dados pessoais):\n{}",
                excerpt(&log)
            );
            let _ = writeln!(
                text,
                "\nUse as ferramentas com session_id \"{}\" para ler o log, o crash report e o resto.",
                summary.id
            );
            (
                ConversationOrigin::Session {
                    session_id: summary.id.clone(),
                    last_crash,
                    signature: signature_of(tools, &summary.id),
                },
                None,
            )
        }
        Subject::LocalFile { file_name, raw } => {
            let redacted = tools.redactor.redact(&raw).text;
            let lines = redacted.lines().count();
            let _ = writeln!(
                text,
                "\nAnalisar: um log ou crash report escolhido no computador (\"{file_name}\", {lines} linhas)."
            );
            let _ = write!(
                text,
                "\nÚltimas linhas com erro do arquivo (sem dados pessoais):\n{}",
                excerpt(&redacted)
            );
            let _ = writeln!(
                text,
                "\nUse search_log e read_log com session_id \"{LOCAL_FILE_SESSION}\" para ler o arquivo."
            );
            (ConversationOrigin::LocalFile { file_name }, Some(redacted))
        }
        Subject::PackOnly => {
            let _ = writeln!(text, "\nAnalisar: só o pack, sem log.");
            (ConversationOrigin::PackOnly, None)
        }
    })
}

/// O retrato do pack agora: o resumo da árvore e o hash de cada item no índice.
pub(crate) fn snapshot(root: &Path) -> PackSnapshot {
    let Ok(pack) = warden_packwiz::read_pack(root) else {
        return PackSnapshot::default();
    };
    let index_file = pack.pack.value.index.file.clone();
    let tree_hash = crate::test_session::pack_tree_hash(root, &index_file).unwrap_or_default();
    let items: BTreeMap<String, String> = pack
        .index
        .as_ref()
        .map(|index| {
            index
                .value
                .files
                .iter()
                .filter(|entry| {
                    ["mods/", "resourcepacks/", "shaderpacks/"]
                        .iter()
                        .any(|folder| entry.file.starts_with(folder))
                })
                .map(|entry| (entry.file.clone(), entry.hash.clone()))
                .collect()
        })
        .unwrap_or_default();
    PackSnapshot { tree_hash, items }
}
