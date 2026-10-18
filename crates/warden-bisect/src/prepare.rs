//! O que cada rodada precisa além dos mods (S-R5-3 §4 e §14 item 7; CA-T25-05 e CA-T25-08).
//!
//! - `-Dfabric.noGui=true` em todo Fabric e Quilt: o jogo sai com erro em vez de abrir a janela
//!   de erro (que ficaria aberta até alguém fechar);
//! - `-Dfml.queryResult=confirm` no Forge 1.8 a 1.12.2 (o `StartupQuery` dos mods ausentes do
//!   mundo; verificado no 1.12.2, nas outras faixas por inferência do spike);
//! - `showLoadWarnings = false` no `config/forge-client.toml` do Forge 1.20+ (a tela de avisos
//!   de carregamento segura o Quick Play no 1.20.1);
//! - `onboardAccessibility:false` no `options.txt` na 1.21+ (é a `warden-launcher` que sabe a
//!   faixa: `quick_play::needs_accessibility_onboarding_off`);
//! - o limite do "jogo vivo e calado": 3× o intervalo entre `Starting integrated minecraft
//!   server` e `logged in` da rodada de controle, no mínimo 60 s.

use warden_perf::Loader;

use crate::watch::FREEZE_FLOOR_MS;

/// Tempo-limite de uma rodada sem nenhuma medida.
pub const DEFAULT_MAX_ROUND_MS: u64 = 10 * 60_000;

/// `(maior, menor)` da versão (`1.20.1` → `(1, 20)`; `26.2` → `(26, 2)`).
fn version(minecraft: &str) -> Option<(u32, u32)> {
    let mut parts = minecraft.trim().split('.');
    let major = parts.next()?.parse().ok()?;
    let minor: String = parts
        .next()
        .unwrap_or("0")
        .chars()
        .take_while(char::is_ascii_digit)
        .collect();
    Some((major, minor.parse().unwrap_or(0)))
}

/// Propriedades de sistema da abertura de cada rodada.
#[must_use]
pub fn system_props(minecraft: &str, loader: Loader) -> Vec<(String, String)> {
    let mut props = Vec::new();
    match loader {
        Loader::Fabric | Loader::Quilt => props.push(("fabric.noGui".into(), "true".into())),
        Loader::Forge => {
            if matches!(version(minecraft), Some((1, minor)) if (8..=12).contains(&minor)) {
                props.push(("fml.queryResult".into(), "confirm".into()));
            }
        }
        Loader::Vanilla | Loader::NeoForge => {}
    }
    props
}

/// Se a instância da rodada precisa de `showLoadWarnings = false` (Forge 1.20+).
#[must_use]
pub fn disable_forge_load_warnings(minecraft: &str, loader: Loader) -> bool {
    loader == Loader::Forge
        && version(minecraft).is_some_and(|(major, minor)| major > 1 || minor >= 20)
}

/// O limite do "jogo vivo e calado" a partir do intervalo medido na rodada de controle.
#[must_use]
pub fn freeze_ms(control_interval_ms: Option<u64>) -> u64 {
    control_interval_ms
        .map_or(FREEZE_FLOOR_MS, |interval| interval.saturating_mul(3))
        .max(FREEZE_FLOOR_MS)
}

/// O tempo-limite de uma rodada: 4× o W da rodada 0 (mínimo 5 min); sem W, 10 min.
#[must_use]
pub fn max_round_ms(world_ms: Option<u64>) -> u64 {
    world_ms.map_or(DEFAULT_MAX_ROUND_MS, |world| {
        world.saturating_mul(4).max(5 * 60_000)
    })
}

/// O loader do jogo a partir do nome do `pack.toml` (`None` = vanilla).
#[must_use]
pub fn perf_loader(name: Option<&str>) -> Loader {
    match name {
        Some("fabric") => Loader::Fabric,
        Some("quilt") => Loader::Quilt,
        Some("forge") => Loader::Forge,
        Some("neoforge") => Loader::NeoForge,
        _ => Loader::Vanilla,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// CA-T25-05: `-Dfml.queryResult=confirm` no Forge 1.12.2 e `-Dfabric.noGui` no Fabric.
    #[test]
    fn ca_t25_05_propriedades_por_faixa() {
        let fabric = system_props("1.20.1", Loader::Fabric);
        assert_eq!(fabric, [("fabric.noGui".to_owned(), "true".to_owned())]);
        assert_eq!(system_props("1.20.1", Loader::Quilt), fabric);
        assert_eq!(
            system_props("1.12.2", Loader::Forge),
            [("fml.queryResult".to_owned(), "confirm".to_owned())]
        );
        assert!(system_props("1.7.10", Loader::Forge).is_empty());
        assert!(system_props("1.20.1", Loader::Forge).is_empty());
        assert!(system_props("1.21.1", Loader::NeoForge).is_empty());
    }

    #[test]
    fn avisos_do_forge_e_limites() {
        assert!(disable_forge_load_warnings("1.20.1", Loader::Forge));
        assert!(disable_forge_load_warnings("26.2", Loader::Forge));
        assert!(!disable_forge_load_warnings("1.19.2", Loader::Forge));
        assert!(!disable_forge_load_warnings("1.20.1", Loader::NeoForge));
        assert_eq!(freeze_ms(None), 60_000);
        assert_eq!(freeze_ms(Some(15_000)), 60_000);
        assert_eq!(freeze_ms(Some(30_000)), 90_000);
        assert_eq!(max_round_ms(None), 600_000);
        assert_eq!(max_round_ms(Some(69_000)), 300_000);
        assert_eq!(max_round_ms(Some(150_000)), 600_000);
        assert_eq!(perf_loader(Some("quilt")), Loader::Quilt);
        assert_eq!(perf_loader(None), Loader::Vanilla);
    }
}
