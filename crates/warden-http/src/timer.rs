//! Relógio e esperas do cliente, injetáveis (QUALITY §4.1: sem `sleep` real nos testes).
//!
//! Toda espera do cliente (limitador por host, 429, novas tentativas) passa por um [`Timer`].
//! No app, [`SystemTimer`] usa o relógio monotônico e o `tokio::time::sleep`. Nos testes,
//! [`ManualTimer`] não dorme: avança o relógio virtual na hora e anota cada espera pedida,
//! para o teste conferir que o Warden esperou o que o servidor mandou.

use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

/// Futuro de uma espera.
pub type Sleep = Pin<Box<dyn Future<Output = ()> + Send>>;

/// Relógio monotônico e esperas.
pub trait Timer: Send + Sync + fmt::Debug + 'static {
    /// Tempo decorrido desde uma origem fixa do relógio.
    fn now(&self) -> Duration;

    /// Espera `duration`.
    fn sleep(&self, duration: Duration) -> Sleep;
}

/// Relógio real: `Instant` e `tokio::time::sleep`.
#[derive(Debug, Clone, Copy)]
pub struct SystemTimer {
    origin: Instant,
}

impl SystemTimer {
    /// Relógio com origem agora.
    #[must_use]
    pub fn new() -> Self {
        Self {
            origin: Instant::now(),
        }
    }
}

impl Default for SystemTimer {
    fn default() -> Self {
        Self::new()
    }
}

impl Timer for SystemTimer {
    fn now(&self) -> Duration {
        self.origin.elapsed()
    }

    fn sleep(&self, duration: Duration) -> Sleep {
        Box::pin(tokio::time::sleep(duration))
    }
}

/// Relógio virtual para testes: `sleep` volta na hora, avança o relógio e fica anotado.
#[derive(Debug, Clone, Default)]
pub struct ManualTimer {
    state: Arc<Mutex<ManualState>>,
}

#[derive(Debug, Default)]
struct ManualState {
    now: Duration,
    sleeps: Vec<Duration>,
}

impl ManualTimer {
    /// Relógio parado no zero.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Avança o relógio sem anotar espera (o tempo "passou" por fora).
    pub fn advance(&self, duration: Duration) {
        self.lock().now += duration;
    }

    /// Esperas pedidas até agora, na ordem.
    #[must_use]
    pub fn sleeps(&self) -> Vec<Duration> {
        self.lock().sleeps.clone()
    }

    /// Soma das esperas pedidas.
    #[must_use]
    pub fn total_slept(&self) -> Duration {
        self.lock().sleeps.iter().sum()
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, ManualState> {
        // O estado é só um contador; um pânico em outra thread de teste não o corrompe.
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl Timer for ManualTimer {
    fn now(&self) -> Duration {
        self.lock().now
    }

    fn sleep(&self, duration: Duration) -> Sleep {
        let mut state = self.lock();
        state.now += duration;
        state.sleeps.push(duration);
        Box::pin(std::future::ready(()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn relogio_manual_avanca_e_anota() {
        let timer = ManualTimer::new();
        assert_eq!(timer.now(), Duration::ZERO);
        timer.sleep(Duration::from_secs(2)).await;
        timer.advance(Duration::from_millis(500));
        timer.sleep(Duration::from_secs(1)).await;
        assert_eq!(timer.now(), Duration::from_millis(3500));
        assert_eq!(
            timer.sleeps(),
            vec![Duration::from_secs(2), Duration::from_secs(1)]
        );
        assert_eq!(timer.total_slept(), Duration::from_secs(3));
    }

    #[tokio::test]
    async fn relogio_do_sistema_anda() {
        let timer = SystemTimer::default();
        let before = timer.now();
        timer.sleep(Duration::from_millis(1)).await;
        assert!(timer.now() > before);
    }
}
