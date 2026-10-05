//! Limitador por servidor (ARCHITECTURE §17).
//!
//! Cada servidor tem até três freios, conforme a sua [`HostPolicy`]:
//! - taxa média (`governor`, algoritmo GCRA), para ficar abaixo das 300 requisições por minuto
//!   do Modrinth;
//! - número de requisições simultâneas (semáforo), para as 4 da CurseForge;
//! - bloqueio temporário quando o servidor manda esperar (429 com `Retry-After` ou
//!   `X-Ratelimit-Reset`, ou `X-Ratelimit-Remaining: 0`): todas as requisições ao mesmo
//!   servidor esperam juntas, não só a que levou o 429.
//!
//! O tempo vem do [`Timer`] do cliente, inclusive o relógio do `governor`, para os testes
//! rodarem sem esperar de verdade.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use governor::clock::Clock;
use governor::middleware::NoOpMiddleware;
use governor::nanos::Nanos;
use governor::state::{InMemoryState, NotKeyed};
use governor::{Quota, RateLimiter};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};
use warden_core::CancellationToken;

use crate::config::{HostPolicy, HttpConfig};
use crate::error::{Error, Result};
use crate::timer::Timer;

/// Relógio do `governor` lido do [`Timer`] do cliente.
#[derive(Debug, Clone)]
struct TimerClock(Arc<dyn Timer>);

impl Clock for TimerClock {
    type Instant = Nanos;

    fn now(&self) -> Nanos {
        Nanos::from(self.0.now())
    }
}

type HostRateLimiter = RateLimiter<NotKeyed, InMemoryState, TimerClock, NoOpMiddleware<Nanos>>;

#[derive(Debug)]
struct HostState {
    rate: Option<HostRateLimiter>,
    slots: Option<Arc<Semaphore>>,
    /// Até quando (no relógio do [`Timer`]) o servidor pediu para não mandar nada.
    blocked_until: Mutex<Option<Duration>>,
}

/// Vaga obtida para uma requisição; segurar enquanto a resposta estiver sendo lida.
#[derive(Debug)]
pub(crate) struct Slot {
    _permit: Option<OwnedSemaphorePermit>,
}

/// Limitador de todos os servidores de um cliente.
#[derive(Debug)]
pub(crate) struct HostLimiter {
    timer: Arc<dyn Timer>,
    config: Arc<HttpConfig>,
    hosts: Mutex<HashMap<String, Arc<HostState>>>,
}

impl HostLimiter {
    pub(crate) fn new(config: Arc<HttpConfig>, timer: Arc<dyn Timer>) -> Self {
        Self {
            timer,
            config,
            hosts: Mutex::new(HashMap::new()),
        }
    }

    fn state(&self, host: &str) -> Arc<HostState> {
        let host = host.to_ascii_lowercase();
        let mut hosts = self.hosts.lock().unwrap_or_else(PoisonError::into_inner);
        Arc::clone(hosts.entry(host.clone()).or_insert_with(|| {
            Arc::new(new_state(
                self.config.policy_for(&host),
                TimerClock(Arc::clone(&self.timer)),
            ))
        }))
    }

    /// Espera a vez de mandar uma requisição a `host`.
    pub(crate) async fn acquire(
        &self,
        host: &str,
        cancel: Option<&CancellationToken>,
    ) -> Result<Slot> {
        let state = self.state(host);
        let permit = match &state.slots {
            Some(slots) => Some(
                cancellable(cancel, Arc::clone(slots).acquire_owned())
                    .await?
                    .map_err(|_| Error::Internal("semáforo do servidor fechado".into()))?,
            ),
            None => None,
        };
        loop {
            let now = self.timer.now();
            let blocked = *state
                .blocked_until
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            if let Some(wait) = blocked
                .and_then(|until| until.checked_sub(now))
                .filter(|w| !w.is_zero())
            {
                tracing::debug!(host, espera_ms = wait.as_millis(), "servidor pediu espera");
                cancellable(cancel, self.timer.sleep(wait)).await?;
                continue;
            }
            if let Some(rate) = &state.rate
                && let Err(not_until) = rate.check()
            {
                let wait = not_until.wait_time_from(Nanos::from(now));
                cancellable(cancel, self.timer.sleep(wait)).await?;
                continue;
            }
            return Ok(Slot { _permit: permit });
        }
    }

    /// Bloqueia `host` por `wait` a partir de agora (o maior bloqueio vale).
    pub(crate) fn block(&self, host: &str, wait: Duration) {
        let state = self.state(host);
        let until = self.timer.now() + wait;
        let mut blocked = state
            .blocked_until
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if blocked.is_none_or(|current| current < until) {
            *blocked = Some(until);
        }
    }
}

fn new_state(policy: HostPolicy, clock: TimerClock) -> HostState {
    let rate = policy.per_minute.and_then(|per_minute| {
        let period = Duration::from_secs(60) / per_minute.get();
        Quota::with_period(period)
            .map(|quota| RateLimiter::direct_with_clock(quota.allow_burst(policy.burst), clock))
    });
    HostState {
        rate,
        slots: policy
            .max_concurrent
            .map(|max| Arc::new(Semaphore::new(max.max(1)))),
        blocked_until: Mutex::new(None),
    }
}

/// Roda `future` até o fim ou até o cancelamento.
pub(crate) async fn cancellable<F: std::future::Future>(
    cancel: Option<&CancellationToken>,
    future: F,
) -> Result<F::Output> {
    match cancel {
        Some(cancel) => tokio::select! {
            biased;
            () = cancel.cancelled() => Err(Error::Cancelled),
            output = future => Ok(output),
        },
        None => Ok(future.await),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::nonzero;
    use crate::timer::ManualTimer;

    fn limiter(policy: HostPolicy) -> (HostLimiter, ManualTimer) {
        let timer = ManualTimer::new();
        let config = HttpConfig::default().with_host("h", policy);
        (
            HostLimiter::new(Arc::new(config), Arc::new(timer.clone())),
            timer,
        )
    }

    #[tokio::test]
    async fn taxa_espaca_depois_da_rajada() {
        // 60 por minuto com rajada de 2: as duas primeiras saem juntas, as seguintes a cada 1 s.
        let (limiter, timer) = limiter(HostPolicy::rate(nonzero(60), nonzero(2)));
        for _ in 0..5 {
            limiter.acquire("h", None).await.unwrap();
        }
        assert_eq!(timer.total_slept(), Duration::from_secs(3));
        assert!(timer.sleeps().iter().all(|s| *s <= Duration::from_secs(1)));
    }

    #[tokio::test]
    async fn bloqueio_vale_para_o_servidor_inteiro() {
        let (limiter, timer) = limiter(HostPolicy::concurrency(8));
        limiter.block("H", Duration::from_secs(7));
        limiter.block("h", Duration::from_secs(3)); // bloqueio menor não encurta o maior
        limiter.acquire("h", None).await.unwrap();
        assert_eq!(timer.sleeps(), vec![Duration::from_secs(7)]);
        // Outro servidor não espera.
        limiter.acquire("outro", None).await.unwrap();
        assert_eq!(timer.sleeps().len(), 1);
        // Passado o bloqueio, não espera de novo.
        limiter.acquire("h", None).await.unwrap();
        assert_eq!(timer.sleeps().len(), 1);
    }

    #[tokio::test]
    async fn simultaneas_limitadas_e_cancelamento() {
        let (limiter, _timer) = limiter(HostPolicy::concurrency(1));
        let first = limiter.acquire("h", None).await.unwrap();
        let cancel = CancellationToken::new();
        cancel.cancel();
        let second = limiter.acquire("h", Some(&cancel)).await;
        assert!(matches!(second, Err(Error::Cancelled)));
        drop(first);
        limiter
            .acquire("h", Some(&CancellationToken::new()))
            .await
            .unwrap();
    }
}
