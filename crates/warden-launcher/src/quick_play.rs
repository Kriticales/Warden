//! Entrar direto no mundo antes de saber o `releaseTime` (ARCHITECTURE §7.8; ROADMAP L-08).
//!
//! A linha de comando decide pelo `releaseTime` do jogo instalado ([`crate::command`]); a
//! interface (Ajustes do teste) e o Testar, antes de instalar, só têm a versão do `pack.toml`.
//! Este módulo responde pela versão:
//!
//! - [`quick_play_support`]: o "Entrar direto no mundo de teste" existe a partir do Minecraft
//!   1.20 (snapshot `23w14a`); nas versões anteriores a opção aparece desabilitada com o motivo
//!   (CA-T11-05). Texto que não é versão conhecida vale como "não dá para saber" e a opção fica
//!   desabilitada também (melhor abrir no menu do que deixar o jogo parado).
//! - [`needs_accessibility_onboarding_off`] e [`disable_accessibility_onboarding`]: na 1.21 e
//!   mais novas, a tela de boas-vindas de acessibilidade da primeira abertura segura o Quick Play
//!   até alguém fechá-la (S-R5-3 §4.1); a instância recebe `onboardAccessibility:false` no
//!   `options.txt` antes de abrir.

use std::fs;
use std::io;
use std::path::Path;

use serde::Serialize;
use warden_java::MinecraftVersion;

/// Se a versão aceita entrar direto num mundo (Quick Play singleplayer).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum QuickPlaySupport {
    /// 1.20 ou mais nova (ou snapshot a partir do `23w14a`).
    Supported,
    /// Anterior à 1.20: só o menu principal.
    TooOld,
    /// Versão que o Warden não reconhece.
    Unknown,
}

impl QuickPlaySupport {
    /// Se dá para entrar direto no mundo.
    #[must_use]
    pub fn is_supported(self) -> bool {
        self == Self::Supported
    }
}

/// A primeira versão de lançamento com Quick Play.
const QUICK_PLAY_RELEASE: [u32; 2] = [1, 20];
/// O primeiro snapshot com Quick Play (`23w14a`).
const QUICK_PLAY_SNAPSHOT: (u32, u32) = (23, 14);
/// A primeira versão de lançamento com a tela de boas-vindas de acessibilidade que segura o
/// Quick Play (S-R5-3 §4.1: a 1.20.1 não segura; a 1.21.1 e as 26.x seguram).
const ONBOARDING_RELEASE: [u32; 2] = [1, 21];
/// O primeiro snapshot da 1.21 (`24w18a`).
const ONBOARDING_SNAPSHOT: (u32, u32) = (24, 18);

/// Nome da opção no `options.txt`.
const ONBOARD_ACCESSIBILITY: &str = "onboardAccessibility";

/// O snapshot semanal (`23w14a` → `(23, 14)`), se for um.
fn weekly_snapshot(text: &str) -> Option<(u32, u32)> {
    let text = text.trim();
    let (year, rest) = text.split_once('w')?;
    if year.len() != 2 || !year.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let week = rest.get(..2)?;
    let letter = rest.get(2..)?;
    if !week.bytes().all(|b| b.is_ascii_digit())
        || letter.len() != 1
        || !letter.bytes().all(|b| b.is_ascii_lowercase())
    {
        return None;
    }
    Some((year.parse().ok()?, week.parse().ok()?))
}

/// Se a versão é `release` ou mais nova (`None` = não reconhecida).
fn at_least(minecraft: &str, release: [u32; 2], snapshot: (u32, u32)) -> Option<bool> {
    if let Some(week) = weekly_snapshot(minecraft) {
        return Some(week >= snapshot);
    }
    let version = MinecraftVersion::parse(minecraft)?;
    let parts = version.parts();
    Some(parts.first().copied().unwrap_or(0) > 1 || parts >= release.as_slice())
}

/// Se a versão do Minecraft aceita "Entrar direto no mundo de teste".
#[must_use]
pub fn quick_play_support(minecraft: &str) -> QuickPlaySupport {
    match at_least(minecraft, QUICK_PLAY_RELEASE, QUICK_PLAY_SNAPSHOT) {
        Some(true) => QuickPlaySupport::Supported,
        Some(false) => QuickPlaySupport::TooOld,
        None => QuickPlaySupport::Unknown,
    }
}

/// Se a versão mostra a tela de boas-vindas de acessibilidade que segura o Quick Play.
#[must_use]
pub fn needs_accessibility_onboarding_off(minecraft: &str) -> bool {
    at_least(minecraft, ONBOARDING_RELEASE, ONBOARDING_SNAPSHOT).unwrap_or(false)
}

/// Grava `onboardAccessibility:false` no `options.txt` de `game_dir`, mantendo as outras
/// linhas (e o fim de linha) como estão. Sem `options.txt`, cria um só com essa linha (o jogo
/// completa o resto na primeira abertura). Devolve se mudou o arquivo.
///
/// # Errors
///
/// Erro de leitura ou gravação do arquivo.
pub fn disable_accessibility_onboarding(game_dir: &Path) -> io::Result<bool> {
    let path = game_dir.join("options.txt");
    let wanted = format!("{ONBOARD_ACCESSIBILITY}:false");
    let text = match fs::read(&path) {
        Ok(bytes) => String::from_utf8_lossy(&bytes).into_owned(),
        Err(error) if error.kind() == io::ErrorKind::NotFound => String::new(),
        Err(error) => return Err(error),
    };
    let newline = if text.contains("\r\n") { "\r\n" } else { "\n" };
    let mut found = false;
    let mut changed = false;
    let mut lines: Vec<String> = Vec::new();
    for line in text.lines() {
        if line.split_once(':').map(|(key, _)| key) == Some(ONBOARD_ACCESSIBILITY) {
            found = true;
            changed |= line != wanted;
            lines.push(wanted.clone());
        } else {
            lines.push(line.to_owned());
        }
    }
    if !found {
        lines.push(wanted);
        changed = true;
    }
    if !changed {
        return Ok(false);
    }
    let mut out = lines.join(newline);
    out.push_str(newline);
    fs::create_dir_all(game_dir)?;
    warden_core::atomic_write(&path, out.as_bytes()).map_err(io::Error::other)?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quick_play_a_partir_da_1_20() {
        for version in [
            "1.20",
            "1.20.1",
            "1.20.4",
            "1.21.1",
            "26.3",
            "1.20-pre1",
            "23w14a",
        ] {
            assert_eq!(
                quick_play_support(version),
                QuickPlaySupport::Supported,
                "{version}"
            );
        }
        for version in ["1.19.2", "1.19.4", "1.12.2", "1.7.10", "23w13a", "22w45a"] {
            assert_eq!(
                quick_play_support(version),
                QuickPlaySupport::TooOld,
                "{version}"
            );
        }
        for version in ["", "b1.7.3", "abc", "1"] {
            assert_eq!(
                quick_play_support(version),
                QuickPlaySupport::Unknown,
                "{version}"
            );
            assert!(!quick_play_support(version).is_supported());
        }
    }

    #[test]
    fn boas_vindas_de_acessibilidade_a_partir_da_1_21() {
        for version in ["1.21", "1.21.1", "26.2", "26.3", "24w18a"] {
            assert!(needs_accessibility_onboarding_off(version), "{version}");
        }
        for version in ["1.20.1", "1.20.6", "1.19.2", "24w14a", "nada"] {
            assert!(!needs_accessibility_onboarding_off(version), "{version}");
        }
    }

    #[test]
    fn options_txt_criado_quando_falta() {
        let dir = tempfile::tempdir().unwrap();
        assert!(disable_accessibility_onboarding(dir.path()).unwrap());
        assert_eq!(
            fs::read_to_string(dir.path().join("options.txt")).unwrap(),
            "onboardAccessibility:false\n"
        );
        assert!(!disable_accessibility_onboarding(dir.path()).unwrap());
    }

    #[test]
    fn options_txt_mantem_as_outras_linhas() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("options.txt");
        fs::write(
            &path,
            "version:3955\r\nonboardAccessibility:true\r\nlang:pt_br\r\n",
        )
        .unwrap();
        assert!(disable_accessibility_onboarding(dir.path()).unwrap());
        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            "version:3955\r\nonboardAccessibility:false\r\nlang:pt_br\r\n"
        );

        fs::write(&path, "version:3955\nlang:pt_br\n").unwrap();
        assert!(disable_accessibility_onboarding(dir.path()).unwrap());
        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            "version:3955\nlang:pt_br\nonboardAccessibility:false\n"
        );
    }
}
