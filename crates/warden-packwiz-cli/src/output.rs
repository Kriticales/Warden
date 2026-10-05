//! Decodificação da saída do packwiz (ARCHITECTURE §6.3).
//!
//! A saída é lida em bytes, cortada em linhas completas (o corte respeita pedaços que chegam
//! partidos no meio de uma linha ou de um caractere UTF-8), decodificada como UTF-8 com
//! substituição e limpa:
//!
//! - sequências ANSI (CSI, OSC e as de dois bytes) e caracteres de controle somem;
//! - um `\r` sozinho redesenha a linha: vale o texto depois do último `\r`;
//! - linhas da barra de progresso do `mpb` (`Refreshing index... 56 % [====>---] 0s`, que o
//!   packwiz imprime mesmo sem terminal, precedidas de `ESC[1A ESC[J`) viram
//!   [`Decoded::Progress`], fora da lista de linhas;
//! - linhas vazias depois da limpeza são descartadas.
//!
//! Também reconhece, nas linhas limpas, as mensagens que decidem o resultado: falta da chave
//! da CurseForge (patch 0001), downloads manuais e downloads que falharam.

use std::sync::LazyLock;

use regex::Regex;

use crate::error::ManualDownload;

/// Começo da mensagem do patch 0001 quando falta `WARDEN_CURSEFORGE_API_KEY` (mesmo texto da
/// constante `MISSING_KEY_MESSAGE` do `xtask/src/packwiz.rs`).
pub const MISSING_KEY_MESSAGE: &str = "WARDEN_CURSEFORGE_API_KEY ausente";

/// Tamanho máximo de uma linha guardada; o resto é cortado (protege a memória de uma saída
/// sem quebras de linha).
pub const MAX_LINE_BYTES: usize = 16 * 1024;

/// Texto que substitui a chave, se ela aparecer na saída.
pub const REDACTED: &str = "[chave omitida]";

/// Uma linha decodificada.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decoded {
    /// Linha de texto limpa.
    Line(String),
    /// Linha da barra de progresso: rótulo e porcentagem (0 a 100).
    Progress {
        /// Rótulo antes da porcentagem (`Refreshing index...`).
        label: String,
        /// Porcentagem.
        percent: u8,
    },
}

/// Corta bytes em linhas completas e as limpa.
#[derive(Debug, Default)]
pub struct LineDecoder {
    pending: Vec<u8>,
    /// A linha atual passou do limite: o resto dela é descartado até o próximo `\n`.
    overflow: bool,
}

impl LineDecoder {
    /// Decodificador vazio.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Acrescenta bytes e devolve as linhas que ficaram completas.
    pub fn push(&mut self, bytes: &[u8]) -> Vec<Decoded> {
        let mut out = Vec::new();
        let mut rest = bytes;
        while let Some(end) = rest.iter().position(|&byte| byte == b'\n') {
            self.append(&rest[..end]);
            self.flush_into(&mut out);
            rest = &rest[end + 1..];
        }
        self.append(rest);
        out
    }

    /// Fim da saída: devolve a última linha, mesmo sem `\n`.
    pub fn finish(&mut self) -> Vec<Decoded> {
        let mut out = Vec::new();
        self.flush_into(&mut out);
        out
    }

    fn append(&mut self, bytes: &[u8]) {
        if self.overflow {
            return;
        }
        let room = MAX_LINE_BYTES.saturating_sub(self.pending.len());
        if bytes.len() > room {
            self.pending.extend_from_slice(&bytes[..room]);
            self.overflow = true;
        } else {
            self.pending.extend_from_slice(bytes);
        }
    }

    fn flush_into(&mut self, out: &mut Vec<Decoded>) {
        let line = std::mem::take(&mut self.pending);
        self.overflow = false;
        if let Some(decoded) = clean_line(&line) {
            out.push(decoded);
        }
    }
}

/// Limpa uma linha (sem o `\n`): `\r`, ANSI, controles, UTF-8 inválido e barra de progresso.
/// Devolve `None` se nada sobrar.
#[must_use]
pub fn clean_line(raw: &[u8]) -> Option<Decoded> {
    let raw = raw.strip_suffix(b"\r").unwrap_or(raw);
    let stripped = strip_ansi(raw);
    // `\r` sozinho volta ao começo da linha: o texto depois dele cobre o anterior.
    let visible = stripped
        .rsplit(|&byte| byte == b'\r')
        .find(|segment| segment.iter().any(|byte| !byte.is_ascii_whitespace()))
        .unwrap_or_default();
    let text = String::from_utf8_lossy(visible);
    let text: String = text
        .chars()
        .filter(|ch| *ch == '\t' || !ch.is_control())
        .collect();
    let text = text.trim_end();
    if text.trim().is_empty() {
        return None;
    }
    if let Some(progress) = parse_progress(text) {
        return Some(progress);
    }
    Some(Decoded::Line(text.to_owned()))
}

/// Remove sequências de escape ANSI e controles C0 (menos `\t` e `\r`, tratados depois).
#[must_use]
pub fn strip_ansi(raw: &[u8]) -> Vec<u8> {
    const ESC: u8 = 0x1b;
    const BEL: u8 = 0x07;
    let mut out = Vec::with_capacity(raw.len());
    let mut index = 0;
    while index < raw.len() {
        let byte = raw[index];
        if byte != ESC {
            if byte == b'\t' || byte == b'\r' || byte >= 0x20 {
                out.push(byte);
            }
            index += 1;
            continue;
        }
        index += 1;
        let Some(&kind) = raw.get(index) else { break };
        index += 1;
        match kind {
            // CSI: parâmetros 0x30–0x3F, intermediários 0x20–0x2F, final 0x40–0x7E.
            b'[' => {
                while let Some(&next) = raw.get(index) {
                    index += 1;
                    if (0x40..=0x7e).contains(&next) {
                        break;
                    }
                    if !(0x20..=0x3f).contains(&next) {
                        // Sequência malformada: o byte inesperado não é engolido.
                        index -= 1;
                        break;
                    }
                }
            }
            // OSC, DCS, SOS, PM, APC: texto até BEL ou ESC \.
            b']' | b'P' | b'X' | b'^' | b'_' => {
                while let Some(&next) = raw.get(index) {
                    index += 1;
                    if next == BEL {
                        break;
                    }
                    if next == ESC && raw.get(index) == Some(&b'\\') {
                        index += 1;
                        break;
                    }
                }
            }
            // Seleção de conjunto de caracteres (`ESC ( B`): um byte a mais.
            b'(' | b')' | b'*' | b'+' => index += 1,
            // Demais sequências de dois bytes (`ESC M`, `ESC 7`…).
            _ => {}
        }
    }
    out
}

static PROGRESS: LazyLock<Option<Regex>> = LazyLock::new(|| {
    // `<rótulo> <n> % [<barra>] <ETA ou done>`, como o `mpb` v4 do packwiz desenha.
    Regex::new(r"^(?P<label>.*?)\s*(?P<percent>\d{1,3}) %\s*\[[^\]]*\](?:\s.*)?$").ok()
});

fn parse_progress(text: &str) -> Option<Decoded> {
    let captures = PROGRESS.as_ref()?.captures(text)?;
    let percent: u8 = captures["percent"].parse().ok()?;
    Some(Decoded::Progress {
        label: captures["label"].trim().to_owned(),
        percent: percent.min(100),
    })
}

/// Troca as ocorrências do segredo por [`REDACTED`]. Segredo vazio não muda nada.
#[must_use]
pub fn redact(line: &str, secret: Option<&str>) -> String {
    match secret.map(str::trim) {
        Some(secret) if !secret.is_empty() && line.contains(secret) => {
            line.replace(secret, REDACTED)
        }
        _ => line.to_owned(),
    }
}

/// A saída diz que faltou a chave da CurseForge (patch 0001).
#[must_use]
pub fn mentions_missing_key(lines: &[String]) -> bool {
    lines.iter().any(|line| line.contains(MISSING_KEY_MESSAGE))
}

static MANUAL_HEADER: LazyLock<Option<Regex>> =
    LazyLock::new(|| Regex::new(r"^Found \d+ manual downloads;").ok());
static MANUAL_ITEM: LazyLock<Option<Regex>> =
    LazyLock::new(|| Regex::new(r"^(?P<name>.+) \((?P<file>[^()]+)\) from (?P<url>\S+)$").ok());
static DOWNLOAD_FAILED: LazyLock<Option<Regex>> =
    LazyLock::new(|| Regex::new(r"^Download of (?P<what>.+ \([^()]+\)) failed").ok());

/// Downloads manuais listados por `cmdshared.ListManualDownloads`: a lista depois de
/// `Found N manual downloads; ...`, até `Once you have done so...`. `None` se a saída não
/// tem essa lista.
#[must_use]
pub fn manual_downloads(lines: &[String]) -> Option<Vec<ManualDownload>> {
    let header = MANUAL_HEADER.as_ref()?;
    let item = MANUAL_ITEM.as_ref()?;
    let start = lines.iter().position(|line| header.is_match(line))?;
    let files = lines[start + 1..]
        .iter()
        .take_while(|line| !line.starts_with("Once you have done so"))
        .filter_map(|line| item.captures(line))
        .map(|captures| ManualDownload {
            name: captures["name"].to_owned(),
            file_name: captures["file"].to_owned(),
            url: captures["url"].to_owned(),
        })
        .collect();
    Some(files)
}

/// Arquivos das linhas `Download of <nome> (<arquivo>) failed: ...`.
#[must_use]
pub fn failed_downloads(lines: &[String]) -> Vec<String> {
    let Some(pattern) = DOWNLOAD_FAILED.as_ref() else {
        return Vec::new();
    };
    lines
        .iter()
        .filter_map(|line| pattern.captures(line))
        .map(|captures| captures["what"].to_owned())
        .collect()
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    fn lines(decoded: &[Decoded]) -> Vec<String> {
        decoded
            .iter()
            .filter_map(|item| match item {
                Decoded::Line(text) => Some(text.clone()),
                Decoded::Progress { .. } => None,
            })
            .collect()
    }

    fn decode_all(bytes: &[u8]) -> Vec<Decoded> {
        let mut decoder = LineDecoder::new();
        let mut out = decoder.push(bytes);
        out.extend(decoder.finish());
        out
    }

    #[test]
    fn barra_do_mpb_vira_progresso() {
        let raw = b"Loading modpack...\nRefreshing index... 27 % [====>----] 0s\n\x1b[1A\x1b[JRefreshing index... 100 % [=========] done\nIndex refreshed!\n";
        let decoded = decode_all(raw);
        assert_eq!(
            decoded,
            vec![
                Decoded::Line("Loading modpack...".to_owned()),
                Decoded::Progress {
                    label: "Refreshing index...".to_owned(),
                    percent: 27
                },
                Decoded::Progress {
                    label: "Refreshing index...".to_owned(),
                    percent: 100
                },
                Decoded::Line("Index refreshed!".to_owned()),
            ]
        );
    }

    #[test]
    fn crlf_e_retorno_de_carro() {
        let raw = b"um\r\ndois\r\n50%\rcarregando\rpronto\n   \n\r\n";
        assert_eq!(lines(&decode_all(raw)), ["um", "dois", "pronto"]);
    }

    #[test]
    fn retorno_no_fim_mantem_o_texto() {
        // `texto\r` seguido de nada visível: o texto anterior continua valendo.
        assert_eq!(
            clean_line(b"texto\r   "),
            Some(Decoded::Line("texto".to_owned()))
        );
    }

    #[test]
    fn ansi_variado_some() {
        let raw = b"\x1b[31mvermelho\x1b[0m \x1b]0;titulo\x07fim \x1b]8;;http://x\x1b\\link\x1b]8;;\x1b\\ \x1b(Bok\x1bM\n";
        assert_eq!(lines(&decode_all(raw)), ["vermelho fim link ok"]);
    }

    #[test]
    fn csi_malformado_nao_engole_texto() {
        assert_eq!(strip_ansi(b"a\x1b[12\x01b"), b"ab");
        assert_eq!(strip_ansi(b"fim\x1b"), b"fim");
        assert_eq!(strip_ansi(b"fim\x1b["), b"fim");
    }

    #[test]
    fn utf8_partido_entre_pedacos() {
        let text = "Ação concluída ✓\n".as_bytes();
        let mut decoder = LineDecoder::new();
        let mut out = Vec::new();
        for byte in text {
            out.extend(decoder.push(std::slice::from_ref(byte)));
        }
        out.extend(decoder.finish());
        assert_eq!(lines(&out), ["Ação concluída ✓"]);
    }

    #[test]
    fn utf8_invalido_vira_substituicao() {
        assert_eq!(
            clean_line(b"a\xffb"),
            Some(Decoded::Line("a\u{fffd}b".to_owned()))
        );
    }

    #[test]
    fn linha_gigante_e_cortada() {
        let mut raw = vec![b'x'; MAX_LINE_BYTES * 3];
        raw.extend_from_slice(b"\nok\n");
        let decoded = decode_all(&raw);
        assert_eq!(decoded.len(), 2);
        let Decoded::Line(first) = &decoded[0] else {
            panic!("{decoded:?}")
        };
        assert_eq!(first.len(), MAX_LINE_BYTES);
        assert_eq!(decoded[1], Decoded::Line("ok".to_owned()));
    }

    #[test]
    fn ultima_linha_sem_quebra() {
        assert_eq!(lines(&decode_all(b"a\nsem quebra")), ["a", "sem quebra"]);
    }

    #[test]
    fn texto_com_porcentagem_nao_e_barra() {
        assert_eq!(
            clean_line(b"Progress 50 % done"),
            Some(Decoded::Line("Progress 50 % done".to_owned()))
        );
        assert_eq!(
            clean_line(b"999 % [==]"),
            Some(Decoded::Line("999 % [==]".to_owned()))
        );
        assert_eq!(
            clean_line(b"0 % [-----]"),
            Some(Decoded::Progress {
                label: String::new(),
                percent: 0
            })
        );
    }

    #[test]
    fn redacao_da_chave() {
        assert_eq!(
            redact("chave=abc123 fim", Some("abc123")),
            "chave=[chave omitida] fim"
        );
        assert_eq!(redact("nada aqui", Some("abc123")), "nada aqui");
        assert_eq!(redact("x", Some("   ")), "x");
        assert_eq!(redact("x", None), "x");
    }

    #[test]
    fn chave_ausente_e_reconhecida() {
        let saida = vec![
            "Failed to get files: WARDEN_CURSEFORGE_API_KEY ausente: a chave da CurseForge não foi informada.".to_owned(),
        ];
        assert!(mentions_missing_key(&saida));
        assert!(!mentions_missing_key(&["Index refreshed!".to_owned()]));
    }

    #[test]
    fn downloads_manuais_de_verdade() {
        // Formato de `cmdshared.ListManualDownloads` (packwiz ef87d96).
        let saida: Vec<String> = [
            "Loading modpack...",
            "Found 2 manual downloads; these mods are unable to be downloaded by packwiz (due to API limitations) and must be manually downloaded:",
            "OptiFine (OptiFine_1.20.1_HD_U_I6.jar) from https://www.curseforge.com/minecraft/mc-mods/optifine/files/1",
            "Mod (com parênteses) (mod-1.0.jar) from https://example.com/mod",
            "Once you have done so, place these files in C:\\cache\\import and re-run this command.",
        ]
        .map(str::to_owned)
        .to_vec();
        let files = manual_downloads(&saida).unwrap();
        assert_eq!(files.len(), 2);
        assert_eq!(files[0].name, "OptiFine");
        assert_eq!(files[0].file_name, "OptiFine_1.20.1_HD_U_I6.jar");
        assert_eq!(files[1].name, "Mod (com parênteses)");
        assert_eq!(files[1].url, "https://example.com/mod");
        assert!(manual_downloads(&["Index refreshed!".to_owned()]).is_none());
    }

    #[test]
    fn downloads_que_falharam() {
        let saida: Vec<String> = [
            "Download of Sodium (sodium-0.6.jar) failed: hash mismatch",
            "Sodium (sodium-0.6.jar) added to zip",
            "Download of A (B) (c.jar) failed: 404",
        ]
        .map(str::to_owned)
        .to_vec();
        assert_eq!(
            failed_downloads(&saida),
            ["Sodium (sodium-0.6.jar)", "A (B) (c.jar)"]
        );
    }

    proptest! {
        /// Nenhuma linha limpa tem ESC ou outro controle (fora `\t`).
        #[test]
        fn linhas_sem_controles(bytes in proptest::collection::vec(any::<u8>(), 0..512)) {
            for item in decode_all(&bytes) {
                if let Decoded::Line(text) = item {
                    prop_assert!(!text.is_empty());
                    prop_assert!(text.chars().all(|ch| ch == '\t' || !ch.is_control()), "{text:?}");
                }
            }
        }

        /// O resultado não depende de como os bytes chegam em pedaços.
        #[test]
        fn pedacos_nao_mudam_o_resultado(
            bytes in proptest::collection::vec(any::<u8>(), 0..512),
            cuts in proptest::collection::vec(0usize..512, 0..12),
        ) {
            let whole = decode_all(&bytes);
            let mut cuts: Vec<usize> = cuts.into_iter().map(|cut| cut.min(bytes.len())).collect();
            cuts.sort_unstable();
            let mut decoder = LineDecoder::new();
            let mut parts = Vec::new();
            let mut start = 0;
            for cut in cuts {
                parts.extend(decoder.push(&bytes[start..cut]));
                start = cut;
            }
            parts.extend(decoder.push(&bytes[start..]));
            parts.extend(decoder.finish());
            prop_assert_eq!(whole, parts);
        }

        /// Texto comum (sem controles nem barra) passa intacto, sem os espaços do fim.
        #[test]
        fn texto_comum_passa(text in "[a-zA-Zà-ú0-9 .,:!()/-]{1,80}") {
            let decoded = decode_all(format!("{text}\n").as_bytes());
            let expected = text.trim_end();
            if expected.trim().is_empty() {
                prop_assert!(decoded.is_empty());
            } else {
                prop_assert_eq!(decoded, vec![Decoded::Line(expected.to_owned())]);
            }
        }
    }
}
