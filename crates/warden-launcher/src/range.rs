//! Faixas de versão do Minecraft pelos argumentos que mudam (ARCHITECTURE §7.2, §7.4 e §7.8).
//!
//! A decisão usa o `releaseTime` do JSON da versão vanilla, não o texto da versão: assim
//! snapshots (`23w14a`) e a numeração por ano (`26.3`) caem na faixa certa sem tabela de
//! nomes. As datas abaixo vêm do manifesto da Mojang
//! (`crates/warden-catalog/tests/fixtures/http/2026-10-05-mojang-version_manifest_v2.json`).

/// `23w14a`, a primeira versão com Quick Play (`--quickPlay*`); `--server`/`--port` saíram
/// nela.
pub(crate) const QUICK_PLAY_SINCE: &str = "2023-04-05T12:05:17";

/// `1.21.9`, a primeira sem `--userType` e com `--offlineDeveloperMode` (R2 §4.5).
pub(crate) const OFFLINE_DEVELOPER_MODE_SINCE: &str = "2025-09-30T11:58:43";

/// `1.19`: a partir dela todo jogo traz log4j corrigido. Antes (1.7.10 a 1.18.2 e os
/// snapshots entre elas) a configuração de log precisa ser a da Mojang ou uma do loader
/// verificada (R2 §1.7).
pub(crate) const LOG4J_FIXED_SINCE: &str = "2022-06-07T09:42:18";

/// Os 19 primeiros caracteres de um RFC 3339 (`AAAA-MM-DDTHH:MM:SS`): a Mojang sempre grava em
/// UTC (`+00:00`), então comparar o texto compara as datas.
fn instant(release_time: &str) -> &str {
    release_time.get(..19).unwrap_or(release_time)
}

/// Se a versão saiu em `since` ou depois.
fn at_least(release_time: &str, since: &str) -> bool {
    instant(release_time) >= since
}

/// Quick Play existe nesta versão.
pub(crate) fn has_quick_play(release_time: &str) -> bool {
    at_least(release_time, QUICK_PLAY_SINCE)
}

/// `--offlineDeveloperMode` existe nesta versão.
pub(crate) fn has_offline_developer_mode(release_time: &str) -> bool {
    at_least(release_time, OFFLINE_DEVELOPER_MODE_SINCE)
}

/// A versão traz log4j vulnerável sem configuração segura (≤ 1.18.x).
pub(crate) fn needs_safe_log4j_config(release_time: &str) -> bool {
    !at_least(release_time, LOG4J_FIXED_SINCE)
}

/// Data plausível no formato da Mojang.
pub(crate) fn is_release_time(text: &str) -> bool {
    let bytes = text.as_bytes();
    bytes.len() >= 19
        && bytes[4] == b'-'
        && bytes[7] == b'-'
        && bytes[10] == b'T'
        && bytes[13] == b':'
        && bytes[16] == b':'
        && [0..4, 5..7, 8..10, 11..13, 14..16, 17..19]
            .into_iter()
            .all(|range| bytes[range].iter().all(u8::is_ascii_digit))
}

#[cfg(test)]
mod tests {
    use super::*;

    // `releaseTime` reais do manifesto.
    const V1_7_10: &str = "2014-05-14T17:29:23+00:00";
    const V1_18_2: &str = "2022-02-28T10:42:45+00:00";
    const V1_19: &str = "2022-06-07T09:42:18+00:00";
    const V23W14A: &str = "2023-04-05T12:05:17+00:00";
    const V1_20: &str = "2023-06-02T08:36:17+00:00";
    const V1_21_8: &str = "2025-07-17T12:04:02+00:00";
    const V1_21_9: &str = "2025-09-30T11:58:43+00:00";
    const V26_3: &str = "2026-09-15T11:23:02+00:00";

    #[test]
    fn quick_play_a_partir_do_23w14a() {
        assert!(!has_quick_play(V1_19));
        assert!(has_quick_play(V23W14A));
        assert!(has_quick_play(V1_20));
        assert!(has_quick_play(V26_3));
    }

    #[test]
    fn modo_offline_a_partir_da_1_21_9() {
        assert!(!has_offline_developer_mode(V1_21_8));
        assert!(has_offline_developer_mode(V1_21_9));
        assert!(has_offline_developer_mode(V26_3));
    }

    #[test]
    fn log4j_seguro_exigido_ate_a_1_18() {
        assert!(needs_safe_log4j_config(V1_7_10));
        assert!(needs_safe_log4j_config(V1_18_2));
        assert!(!needs_safe_log4j_config(V1_19));
        assert!(!needs_safe_log4j_config(V26_3));
    }

    #[test]
    fn formato_da_data() {
        assert!(is_release_time(V1_7_10));
        assert!(is_release_time("2014-05-14T17:29:23"));
        assert!(!is_release_time(""));
        assert!(!is_release_time("2014-05-14 17:29:23"));
        assert!(!is_release_time("ontem"));
    }
}
