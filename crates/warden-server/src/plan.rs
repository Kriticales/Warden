//! A memória do teste com servidor (SPEC T27 passo 2; ARCHITECTURE §7.6; CA-T27-03).
//!
//! Antes de abrir, a interface mostra "Servidor 4 GB + jogo 6 GB = 10 GB · livre agora:
//! 13 GB". Se a soma não couber na memória livre, o aviso sugere "Só o servidor" (ou diminuir a
//! memória); nada abre sem a confirmação da pessoa. A memória livre vem do sistema (lida pela
//! `warden-app`); aqui ficam só as contas, puras.

use serde::Serialize;

/// Memória mínima do servidor local, em MB.
pub const SERVER_MIN_MB: u32 = 2048;
/// Memória máxima escolhida automaticamente para o servidor, em MB.
pub const SERVER_MAX_MB: u32 = 8192;
/// Memória do servidor vanilla (sem mods), em MB.
pub const SERVER_VANILLA_MB: u32 = 1536;

/// Memória automática do servidor: 2 GB mais 16 MB por mod, arredondada para cima em 512 MB,
/// até 8 GB (o servidor não carrega texturas nem sons: precisa de menos que o jogo).
#[must_use]
pub fn server_memory_mb(mod_count: usize) -> u32 {
    if mod_count == 0 {
        return SERVER_VANILLA_MB;
    }
    let mods = u32::try_from(mod_count).unwrap_or(u32::MAX);
    let wanted = SERVER_MIN_MB.saturating_add(mods.saturating_mul(16));
    wanted
        .div_ceil(512)
        .saturating_mul(512)
        .clamp(SERVER_MIN_MB, SERVER_MAX_MB)
}

/// A conta de memória mostrada antes de abrir.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct MemoryPlan {
    /// Memória do servidor, em MB.
    pub server_mb: u32,
    /// Memória do jogo, em MB (`None` em "Só o servidor").
    pub game_mb: Option<u32>,
    /// A soma, em MB.
    pub total_mb: u32,
    /// Memória livre agora, em MB (`None` se o sistema não informou).
    #[specta(type = Option<u32>)]
    pub free_mb: Option<u64>,
    /// Se a soma cabe na memória livre (sem a leitura, considera que cabe).
    pub fits: bool,
}

/// Monta a conta.
#[must_use]
pub fn memory_plan(server_mb: u32, game_mb: Option<u32>, free_mb: Option<u64>) -> MemoryPlan {
    let total_mb = server_mb.saturating_add(game_mb.unwrap_or(0));
    MemoryPlan {
        server_mb,
        game_mb,
        total_mb,
        free_mb,
        fits: free_mb.is_none_or(|free| free >= u64::from(total_mb)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memoria_automatica_do_servidor() {
        assert_eq!(server_memory_mb(0), SERVER_VANILLA_MB);
        assert_eq!(server_memory_mb(1), 2560);
        assert_eq!(server_memory_mb(32), 2560);
        assert_eq!(server_memory_mb(120), 4096);
        assert_eq!(server_memory_mb(10_000), SERVER_MAX_MB);
    }

    #[test]
    fn ca_t27_03_soma_maior_que_a_livre_nao_cabe() {
        let plan = memory_plan(4096, Some(6144), Some(7 * 1024));
        assert_eq!(plan.total_mb, 10_240);
        assert!(!plan.fits);
        // "Só o servidor" cabe.
        assert!(memory_plan(4096, None, Some(7 * 1024)).fits);
        assert!(memory_plan(4096, Some(6144), Some(13 * 1024)).fits);
        // Sem a leitura do sistema, não bloqueia.
        assert!(memory_plan(4096, Some(6144), None).fits);
    }
}
