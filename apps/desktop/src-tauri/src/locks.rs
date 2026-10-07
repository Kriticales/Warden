//! Travas por pack (ARCHITECTURE §15; ADR-0019).
//!
//! Um `RwLock` por [`PackId`]: leituras (inventário, detalhes, diagnóstico rápido) rodam em
//! paralelo; mutações são exclusivas e entram em fila, na ordem de chegada (o `RwLock` do Tokio
//! é justo). Packs diferentes nunca esperam um pelo outro. Enquanto espera, a operação aparece
//! em Tarefas como "Aguardando outra operação neste pack".

use std::collections::HashMap;
use std::sync::{Arc, Mutex, PoisonError};

use tokio::sync::{OwnedRwLockReadGuard, OwnedRwLockWriteGuard, RwLock};
use warden_core::PackId;
use warden_project::watch::{WriteLedger, WriteScope};

use crate::operations::OperationHandle;

/// Trava de leitura de um pack (solta ao sair de escopo).
pub type PackReadGuard = OwnedRwLockReadGuard<()>;

/// Trava de escrita de um pack (solta ao sair de escopo). Enquanto existe, o vigia de mudanças
/// externas (A-05) trata os eventos do pack como escritas do próprio Warden.
#[derive(Debug)]
pub struct PackWriteGuard {
    _lock: OwnedRwLockWriteGuard<()>,
    _scope: WriteScope,
}

/// As travas de todos os packs.
#[derive(Debug, Default)]
pub struct PackLocks {
    locks: Mutex<HashMap<PackId, Arc<RwLock<()>>>>,
    ledger: WriteLedger,
}

impl PackLocks {
    /// Travas vazias.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Registro das escritas do Warden, lido pelo vigia de mudanças externas.
    #[must_use]
    pub fn ledger(&self) -> &WriteLedger {
        &self.ledger
    }

    fn write_guard(&self, pack: PackId, lock: OwnedRwLockWriteGuard<()>) -> PackWriteGuard {
        PackWriteGuard {
            _lock: lock,
            _scope: self.ledger.begin(pack),
        }
    }

    fn lock_for(&self, pack: PackId) -> Arc<RwLock<()>> {
        let mut locks = self.locks.lock().unwrap_or_else(PoisonError::into_inner);
        // Esquece as travas que ninguém usa (só o mapa as guarda), para o mapa não crescer
        // com packs fechados.
        locks.retain(|id, lock| *id == pack || Arc::strong_count(lock) > 1);
        Arc::clone(locks.entry(pack).or_default())
    }

    /// Trava de leitura.
    pub async fn read(&self, pack: PackId) -> PackReadGuard {
        self.lock_for(pack).read_owned().await
    }

    /// Trava de escrita.
    pub async fn write(&self, pack: PackId) -> PackWriteGuard {
        let lock = self.lock_for(pack).write_owned().await;
        self.write_guard(pack, lock)
    }

    /// Trava de leitura para uma operação: se precisar esperar, a operação aparece como
    /// aguardando.
    pub async fn read_for(&self, pack: PackId, operation: &OperationHandle) -> PackReadGuard {
        let lock = self.lock_for(pack);
        if let Ok(guard) = Arc::clone(&lock).try_read_owned() {
            return guard;
        }
        operation.set_waiting_for_lock(true);
        let guard = lock.read_owned().await;
        operation.set_waiting_for_lock(false);
        guard
    }

    /// Trava de escrita para uma operação: se precisar esperar, a operação aparece como
    /// aguardando.
    pub async fn write_for(&self, pack: PackId, operation: &OperationHandle) -> PackWriteGuard {
        let lock = self.lock_for(pack);
        if let Ok(guard) = Arc::clone(&lock).try_write_owned() {
            return self.write_guard(pack, guard);
        }
        operation.set_waiting_for_lock(true);
        let guard = lock.write_owned().await;
        operation.set_waiting_for_lock(false);
        self.write_guard(pack, guard)
    }

    /// Quantos packs têm trava em uso (testes).
    #[cfg(test)]
    fn tracked(&self) -> usize {
        self.locks
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::operations::tests::channel_registry;
    use crate::operations::{OperationKind, OperationState};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;
    use tokio::sync::{Barrier, oneshot};

    const KIND: OperationKind = OperationKind::new("teste.escrita");
    const LIMIT: Duration = Duration::from_secs(10);

    /// Critério 4 da F0-05: duas escritas no mesmo pack são serializadas; a segunda espera
    /// (aparecendo como aguardando) e só entra depois que a primeira sai.
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn f0_05_ca4_escritas_no_mesmo_pack_sao_serializadas() {
        let locks = Arc::new(PackLocks::new());
        let (registry, mut notices) = channel_registry();
        let pack = PackId::new();
        let inside = Arc::new(AtomicUsize::new(0));
        let max_inside = Arc::new(AtomicUsize::new(0));

        let first = registry.start(KIND, Some(pack), true);
        let first_guard = locks.write_for(pack, &first).await;
        inside.fetch_add(1, Ordering::SeqCst);
        max_inside.fetch_max(1, Ordering::SeqCst);

        let second = registry.start(KIND, Some(pack), true);
        let second_id = second.id();
        let (entered_tx, entered_rx) = oneshot::channel();
        let task = {
            let locks = Arc::clone(&locks);
            let inside = Arc::clone(&inside);
            let max_inside = Arc::clone(&max_inside);
            tokio::spawn(async move {
                let guard = locks.write_for(pack, &second).await;
                let now = inside.fetch_add(1, Ordering::SeqCst) + 1;
                max_inside.fetch_max(now, Ordering::SeqCst);
                let _ = entered_tx.send(());
                inside.fetch_sub(1, Ordering::SeqCst);
                drop(guard);
                second.finish(Ok(()))
            })
        };

        // A segunda operação passa a aguardar a trava.
        tokio::time::timeout(LIMIT, async {
            while let Some(snapshot) = notices.recv().await {
                if snapshot.id == second_id && snapshot.state == OperationState::WaitingForLock {
                    break;
                }
            }
        })
        .await
        .expect("a segunda escrita não ficou aguardando");

        // A primeira ainda está dentro; a segunda não entrou.
        assert_eq!(inside.load(Ordering::SeqCst), 1);
        inside.fetch_sub(1, Ordering::SeqCst);
        drop(first_guard);
        first.finish(Ok(())).unwrap();

        tokio::time::timeout(LIMIT, entered_rx)
            .await
            .expect("a segunda escrita não entrou depois da primeira")
            .unwrap();
        task.await.unwrap().unwrap();
        assert_eq!(
            max_inside.load(Ordering::SeqCst),
            1,
            "duas escritas ao mesmo tempo"
        );
        assert_eq!(
            registry.get(second_id).unwrap().state,
            OperationState::Succeeded
        );
    }

    /// Muitas escritas concorrentes no mesmo pack: nunca duas ao mesmo tempo.
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn f0_05_ca4_muitas_escritas_nunca_se_sobrepoem() {
        let locks = Arc::new(PackLocks::new());
        let pack = PackId::new();
        let inside = Arc::new(AtomicUsize::new(0));
        let max_inside = Arc::new(AtomicUsize::new(0));
        let tasks: Vec<_> = (0..32)
            .map(|_| {
                let locks = Arc::clone(&locks);
                let inside = Arc::clone(&inside);
                let max_inside = Arc::clone(&max_inside);
                tokio::spawn(async move {
                    let _guard = locks.write(pack).await;
                    let now = inside.fetch_add(1, Ordering::SeqCst) + 1;
                    max_inside.fetch_max(now, Ordering::SeqCst);
                    for _ in 0..10 {
                        tokio::task::yield_now().await;
                    }
                    inside.fetch_sub(1, Ordering::SeqCst);
                })
            })
            .collect();
        for task in tasks {
            tokio::time::timeout(LIMIT, task).await.unwrap().unwrap();
        }
        assert_eq!(max_inside.load(Ordering::SeqCst), 1);
    }

    /// Critério 4 da F0-05: escritas em packs diferentes rodam em paralelo. As duas seguram a
    /// trava e esperam uma pela outra numa barreira: se fossem serializadas, travariam.
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn f0_05_ca4_escritas_em_packs_diferentes_rodam_em_paralelo() {
        let locks = Arc::new(PackLocks::new());
        let (registry, _notices) = channel_registry();
        let barrier = Arc::new(Barrier::new(2));
        let tasks: Vec<_> = [PackId::new(), PackId::new()]
            .into_iter()
            .map(|pack| {
                let locks = Arc::clone(&locks);
                let barrier = Arc::clone(&barrier);
                let handle = registry.start(KIND, Some(pack), true);
                tokio::spawn(async move {
                    let _guard = locks.write_for(pack, &handle).await;
                    barrier.wait().await;
                    handle.finish(Ok(()))
                })
            })
            .collect();
        for task in tasks {
            tokio::time::timeout(LIMIT, task)
                .await
                .expect("packs diferentes esperaram um pelo outro")
                .unwrap()
                .unwrap();
        }
    }

    /// Leituras no mesmo pack rodam juntas; a escrita espera as leituras.
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn leituras_em_paralelo_e_escrita_exclusiva() {
        let locks = Arc::new(PackLocks::new());
        let (registry, _notices) = channel_registry();
        let pack = PackId::new();
        let reader = registry.start(KIND, Some(pack), true);
        let first = locks.read_for(pack, &reader).await;
        let second = tokio::time::timeout(LIMIT, locks.read(pack))
            .await
            .expect("duas leituras deveriam rodar juntas");
        let mut write = Box::pin(locks.write(pack));
        assert!(
            futures_poll_once(&mut write).await.is_none(),
            "escrita entrou com leituras em andamento"
        );
        drop(first);
        drop(second);
        let _write = tokio::time::timeout(LIMIT, write).await.unwrap();
        reader.finish(Ok(())).unwrap();
    }

    /// Faz um único `poll` do futuro.
    async fn futures_poll_once<F: std::future::Future + Unpin>(
        future: &mut F,
    ) -> Option<F::Output> {
        std::future::poll_fn(|context| {
            std::task::Poll::Ready(match std::pin::Pin::new(&mut *future).poll(context) {
                std::task::Poll::Ready(output) => Some(output),
                std::task::Poll::Pending => None,
            })
        })
        .await
    }

    #[tokio::test]
    async fn travas_sem_uso_sao_esquecidas() {
        let locks = PackLocks::new();
        for _ in 0..10 {
            drop(locks.write(PackId::new()).await);
        }
        let held = locks.read(PackId::new()).await;
        assert!(locks.tracked() <= 2, "{} travas guardadas", locks.tracked());
        drop(held);
    }

    /// A-05: enquanto a trava de escrita existe, o vigia trata o pack como em escrita do
    /// Warden; leituras e outros packs não.
    #[tokio::test]
    async fn a05_trava_de_escrita_marca_o_pack_no_registro_de_escritas() {
        let locks = PackLocks::new();
        let pack = PackId::new();
        let now = std::time::Instant::now;
        assert!(!locks.ledger().covers(pack, now()));
        drop(locks.read(pack).await);
        assert!(!locks.ledger().covers(pack, now()));
        let guard = locks.write(pack).await;
        assert!(locks.ledger().covers(pack, now()));
        assert!(!locks.ledger().covers(PackId::new(), now()));
        drop(guard);
        // Logo depois ainda vale (folga para os eventos atrasados do sistema).
        assert!(locks.ledger().covers(pack, now()));
    }
}
