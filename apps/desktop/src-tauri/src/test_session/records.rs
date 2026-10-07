//! O que o Testar acrescenta ao `session.json` da `warden-launcher` (ARCHITECTURE §7.4) e a
//! leitura das sessões gravadas para a interface (Ver último teste, sessões anteriores).
//!
//! Campos acrescentados (`extra` do `SessionRecord`, em camelCase):
//! - `sessionId`, `packId`, `startedAtMs`, `mode`, `profile`;
//! - `packTree`: `{ hash, head, unsavedFiles }`, a árvore do pack testada. `hash` é
//!   [`pack_tree_hash`] (o aviso "versão não testada" da V-03 calcula o mesmo para a versão
//!   publicada), `head` é o commit da última versão salva e `unsavedFiles`, quantos arquivos
//!   estavam diferentes dela;
//! - `memory` (`{ mb, auto }`), `javaRuntime`, `modCount`;
//! - gancho 1.1 (ADR-0039): `profileSignature` (memória, Java, argumentos, janela e "ao abrir
//!   o jogo", com um `hash` que junta tudo), `machine` (a impressão do computador) e
//!   `firstLaunch` (primeira abertura desta instância).

use std::path::Path;

use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use warden_core::PackId;
use warden_launcher::outcome::Outcome;
use warden_launcher::session::SessionRecord;

use super::TestMode;
use super::launch_plan::MemoryChoice;
use super::system::MachineFingerprint;

/// A árvore do pack testada.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PackTree {
    /// [`pack_tree_hash`].
    pub(crate) hash: String,
    /// Commit da última versão salva (`None` sem histórico).
    pub(crate) head: Option<String>,
    /// Arquivos diferentes da última versão salva.
    pub(crate) unsaved_files: u32,
}

/// Resumo do conteúdo do pack como os jogadores o recebem: SHA-256 do `pack.toml` e do índice
/// (que já traz o hash de cada arquivo do pack). `sha256:<hex>`.
///
/// Erros: disco (o `pack.toml` ou o índice não puderam ser lidos).
pub(crate) fn pack_tree_hash(root: &Path, index_file: &str) -> std::io::Result<String> {
    let mut hasher = Sha256::new();
    for name in ["pack.toml", index_file] {
        let bytes = std::fs::read(root.join(name))?;
        hasher.update(name.as_bytes());
        hasher.update([0]);
        hasher.update(u64::try_from(bytes.len()).unwrap_or(u64::MAX).to_le_bytes());
        hasher.update(&bytes);
    }
    Ok(format!("sha256:{}", hex::encode(hasher.finalize())))
}

/// A assinatura do perfil do teste (gancho 1.1).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProfileSignature {
    /// Memória.
    pub(crate) memory_mb: u32,
    /// Se a memória foi a automática.
    pub(crate) memory_auto: bool,
    /// `auto` ou o id do Java escolhido nos Ajustes.
    pub(crate) java: String,
    /// Major do Java usado.
    pub(crate) java_major: u32,
    /// Argumentos da JVM do teste (com o coletor padrão).
    pub(crate) jvm_args: Vec<String>,
    /// Janela (os perfis com tamanho de janela são da L-08).
    pub(crate) window: Option<String>,
    /// Ao abrir o jogo: `menu` (entrar direto no mundo é da L-08).
    pub(crate) on_open: &'static str,
    /// SHA-256 dos campos acima.
    pub(crate) hash: String,
}

impl ProfileSignature {
    /// Monta e calcula o `hash`.
    pub(crate) fn new(
        memory: MemoryChoice,
        java: String,
        java_major: u32,
        jvm_args: Vec<String>,
    ) -> Self {
        let mut signature = Self {
            memory_mb: memory.mb,
            memory_auto: memory.auto,
            java,
            java_major,
            jvm_args,
            window: None,
            on_open: "menu",
            hash: String::new(),
        };
        let canonical = serde_json::to_vec(&signature).unwrap_or_default();
        signature.hash = format!("sha256:{}", hex::encode(Sha256::digest(&canonical)));
        signature
    }
}

/// Os campos do Testar no `session.json`.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SessionExtras {
    pub(crate) session_id: String,
    pub(crate) pack_id: PackId,
    pub(crate) started_at_ms: u64,
    pub(crate) mode: TestMode,
    pub(crate) profile: String,
    pub(crate) pack_tree: Option<PackTree>,
    pub(crate) memory: MemoryChoice,
    pub(crate) java_runtime: Option<String>,
    pub(crate) mod_count: usize,
    pub(crate) profile_signature: ProfileSignature,
    pub(crate) machine: MachineFingerprint,
    pub(crate) first_launch: bool,
}

impl SessionExtras {
    /// Acrescenta os campos ao registro.
    pub(crate) fn apply(&self, record: &mut SessionRecord) {
        if let Ok(serde_json::Value::Object(map)) = serde_json::to_value(self) {
            record.extra.extend(map);
        }
    }
}

/// Uma sessão gravada, como a interface mostra.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TestSessionSummary {
    /// Nome da pasta da sessão (`2026-10-07_14-03-22`).
    pub(crate) id: String,
    /// Início (milissegundos desde 1970).
    #[specta(type = specta_typescript::Number)]
    pub(crate) started_at_ms: u64,
    /// Fim.
    #[specta(type = specta_typescript::Number)]
    pub(crate) ended_at_ms: u64,
    /// Duração do processo.
    #[specta(type = specta_typescript::Number)]
    pub(crate) duration_ms: u64,
    /// Resultado.
    pub(crate) outcome: Outcome,
    /// Código de saída.
    pub(crate) exit_code: Option<i32>,
    /// Versão do pack testada.
    pub(crate) pack_version: Option<String>,
    /// Versão do Minecraft.
    pub(crate) minecraft: String,
    /// Loader (`Fabric`, `Forge`…; `None` no vanilla).
    pub(crate) loader: Option<String>,
    /// Versão do loader.
    pub(crate) loader_version: Option<String>,
    /// Major do Java.
    pub(crate) java_major: u32,
    /// Memória do teste, em MB.
    pub(crate) memory_mb: Option<u32>,
    /// Modo.
    pub(crate) mode: TestMode,
    /// Crash reports, relativos à pasta do jogo.
    pub(crate) crash_reports: Vec<String>,
    /// `hs_err_pid*.log`, relativos à pasta do jogo.
    pub(crate) hs_err_files: Vec<String>,
}

impl TestSessionSummary {
    /// Resume um registro gravado.
    pub(crate) fn from_record(id: &str, record: &SessionRecord) -> Self {
        let extra = &record.extra;
        let started_at_ms = extra
            .get("startedAtMs")
            .and_then(serde_json::Value::as_u64)
            .or_else(|| parse_rfc3339_utc_ms(&record.started_at))
            .unwrap_or(0);
        let mode = extra
            .get("mode")
            .and_then(|value| serde_json::from_value(value.clone()).ok())
            .unwrap_or_default();
        let memory_mb = extra
            .get("memory")
            .and_then(|memory| memory.get("mb"))
            .and_then(serde_json::Value::as_u64)
            .and_then(|mb| u32::try_from(mb).ok());
        let loader = &record.game.loader;
        let paths = |list: &[std::path::PathBuf]| {
            list.iter()
                .map(|path| path.to_string_lossy().replace('\\', "/"))
                .collect()
        };
        Self {
            id: id.to_owned(),
            started_at_ms,
            ended_at_ms: started_at_ms.saturating_add(record.duration_ms),
            duration_ms: record.duration_ms,
            outcome: record.outcome,
            exit_code: record.exit_code,
            pack_version: record.pack_version.clone(),
            minecraft: record.game.minecraft.as_str().to_owned(),
            loader: loader.version().map(|_| loader.label().to_owned()),
            loader_version: loader.version().map(ToOwned::to_owned),
            java_major: record.java_major,
            memory_mb,
            mode,
            crash_reports: paths(&record.artifacts.crash_reports),
            hs_err_files: paths(&record.artifacts.hs_err_files),
        }
    }
}

/// `2026-10-01T17:05:09Z` → milissegundos desde 1970 (só o formato que a `warden-launcher`
/// grava).
pub(crate) fn parse_rfc3339_utc_ms(text: &str) -> Option<u64> {
    let bytes = text.as_bytes();
    if bytes.len() != 20 || bytes[19] != b'Z' {
        return None;
    }
    let number = |range: std::ops::Range<usize>| text.get(range)?.parse::<i64>().ok();
    let (year, month, day) = (number(0..4)?, number(5..7)?, number(8..10)?);
    let (hour, minute, second) = (number(11..13)?, number(14..16)?, number(17..19)?);
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    // `days_from_civil` de Howard Hinnant (domínio público).
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (month + 9) % 12;
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    let seconds = days * 86_400 + hour * 3600 + minute * 60 + second;
    u64::try_from(seconds).ok().map(|s| s * 1000)
}

/// O texto gravado em `packs.json` como "último teste" (Meus packs): `ok` quando o jogo abriu
/// (fechou normalmente ou foi encerrado por você), `crashed` quando travou.
pub(crate) const fn last_test_value(outcome: Outcome) -> &'static str {
    match outcome {
        Outcome::Crashed => "crashed",
        Outcome::ClosedNormally | Outcome::StoppedByUser => "ok",
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, UNIX_EPOCH};

    use warden_launcher::outcome::CrashArtifacts;
    use warden_launcher::process::GameExit;
    use warden_launcher::{GameSpec, LoaderSpec};

    use super::*;

    #[test]
    fn data_rfc3339_ida_e_volta() {
        for seconds in [0u64, 1_709_208_000, 1_790_874_309, 4_102_444_800] {
            let time = UNIX_EPOCH + Duration::from_secs(seconds);
            let text = warden_launcher::session::rfc3339_utc(time);
            assert_eq!(parse_rfc3339_utc_ms(&text), Some(seconds * 1000), "{text}");
        }
        assert_eq!(parse_rfc3339_utc_ms("2026-13-01T00:00:00Z"), None);
        assert_eq!(parse_rfc3339_utc_ms("ontem"), None);
    }

    #[test]
    fn hash_da_arvore_muda_com_o_indice() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("pack.toml"), "name = \"a\"").unwrap();
        std::fs::write(dir.path().join("index.toml"), "files = []").unwrap();
        let first = pack_tree_hash(dir.path(), "index.toml").unwrap();
        assert!(first.starts_with("sha256:"));
        assert_eq!(first, pack_tree_hash(dir.path(), "index.toml").unwrap());
        std::fs::write(dir.path().join("index.toml"), "files = [1]").unwrap();
        assert_ne!(first, pack_tree_hash(dir.path(), "index.toml").unwrap());
        assert!(pack_tree_hash(dir.path(), "nao-existe.toml").is_err());
    }

    #[test]
    fn assinatura_do_perfil_e_estavel() {
        let memory = MemoryChoice {
            mb: 4096,
            auto: true,
        };
        let a = ProfileSignature::new(memory, "auto".into(), 17, vec!["-XX:+UseG1GC".into()]);
        let b = ProfileSignature::new(memory, "auto".into(), 17, vec!["-XX:+UseG1GC".into()]);
        let c = ProfileSignature::new(memory, "auto".into(), 21, vec!["-XX:+UseG1GC".into()]);
        assert_eq!(a.hash, b.hash);
        assert_ne!(a.hash, c.hash);
        let json = serde_json::to_value(&a).unwrap();
        assert_eq!(json["onOpen"], "menu");
        assert_eq!(json["memoryMb"], 4096);
    }

    #[test]
    fn resumo_le_os_campos_extras() {
        let spec = GameSpec::new(
            "1.12.2",
            LoaderSpec::Forge {
                version: "14.23.5.2860".into(),
            },
        )
        .unwrap();
        let exit = GameExit {
            outcome: Outcome::Crashed,
            exit_code: Some(-1),
            stop_requested: false,
            duration_ms: 48_000,
            artifacts: CrashArtifacts {
                crash_reports: vec![std::path::PathBuf::from("jogo/crash-reports/crash.txt")],
                hs_err_files: Vec::new(),
            },
        };
        let started = UNIX_EPOCH + Duration::from_secs(1_790_874_309);
        let mut record = SessionRecord::from_exit(
            started,
            &exit,
            &spec,
            8,
            Some("1.2.0".into()),
            Path::new("jogo"),
        );
        record
            .extra
            .insert("mode".into(), serde_json::Value::String("normal".into()));
        record.extra.insert(
            "memory".into(),
            serde_json::json!({ "mb": 6144, "auto": true }),
        );
        let summary = TestSessionSummary::from_record("2026-10-01_17-05-09", &record);
        assert_eq!(summary.started_at_ms, 1_790_874_309_000);
        assert_eq!(summary.ended_at_ms, 1_790_874_357_000);
        assert_eq!(summary.loader.as_deref(), Some("Forge"));
        assert_eq!(summary.loader_version.as_deref(), Some("14.23.5.2860"));
        assert_eq!(summary.memory_mb, Some(6144));
        assert_eq!(summary.crash_reports, vec!["crash-reports/crash.txt"]);
        assert_eq!(summary.mode, TestMode::Normal);
        assert_eq!(last_test_value(summary.outcome), "crashed");
        assert_eq!(last_test_value(Outcome::StoppedByUser), "ok");
    }
}
