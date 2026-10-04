//! Registro de operações longas (ARCHITECTURE §15; ADR-0019; painel Tarefas, T22).
//!
//! Toda operação longa nasce com [`OperationRegistry::start`], que devolve um
//! [`OperationHandle`] com o [`OperationId`] e o [`CancellationToken`]. O handle informa etapa e
//! progresso (implementa [`ProgressSink`]), repassa os eventos ao `Channel` do comando quando
//! houver e termina com [`OperationHandle::finish`]. Cada mudança chega à interface pelo evento
//! `operation-updated` (o progresso, no máximo a cada 100 ms por operação). As últimas 50
//! operações concluídas ficam na lista, com o resultado.
//!
//! `operation_cancel` aciona o token. Quem executa a operação decide onde parar: entre etapas
//! ([`OperationHandle::ensure_not_cancelled`]) ou a qualquer momento de uma espera
//! ([`OperationHandle::run_cancellable`]). Um trecho que não pode ser interrompido (a gravação
//! final de uma transação) chama [`OperationHandle::set_cancellable`] com `false`.

use std::borrow::Cow;
use std::collections::{BTreeMap, VecDeque};
use std::future::Future;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::Serialize;
use warden_core::{CancellationToken, OperationId, PackId, Progress, ProgressSink};

use crate::error::{AppError, AppErrorCode};
use crate::events::{LogLine, OperationEvent};

/// Quantas operações concluídas ficam na lista (T22).
pub(crate) const FINISHED_KEPT: usize = 50;

/// Intervalo mínimo entre dois avisos só de progresso da mesma operação.
const PROGRESS_INTERVAL: Duration = Duration::from_millis(100);

/// Tipo da operação, como chave estável (`"pack.create"`, `"test.start"`…). A interface usa
/// para escolher o texto; cada tarefa define as suas constantes no próprio módulo.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, specta::Type)]
#[serde(transparent)]
#[specta(transparent)]
pub struct OperationKind(#[specta(type = String)] Cow<'static, str>);

impl OperationKind {
    /// Tipo a partir de uma chave fixa.
    #[must_use]
    pub const fn new(key: &'static str) -> Self {
        Self(Cow::Borrowed(key))
    }

    /// A chave.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Situação de uma operação.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum OperationState {
    /// Em andamento.
    Running,
    /// Esperando outra operação no mesmo pack ("Aguardando outra operação neste pack").
    WaitingForLock,
    /// Cancelamento pedido; ainda parando.
    Cancelling,
    /// Terminou bem.
    Succeeded,
    /// Terminou com erro.
    Failed,
    /// Cancelada.
    Cancelled,
}

impl OperationState {
    /// Se já terminou.
    #[must_use]
    pub const fn is_finished(self) -> bool {
        matches!(self, Self::Succeeded | Self::Failed | Self::Cancelled)
    }
}

/// Etapa atual.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct OperationStage {
    /// Identificador estável da etapa.
    pub id: String,
    /// Chave do texto no catálogo.
    pub label_key: String,
}

/// O que a interface sabe de uma operação.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct OperationSnapshot {
    /// Identificador.
    pub id: OperationId,
    /// Tipo.
    pub kind: OperationKind,
    /// Pack em que atua, se for o caso.
    pub pack_id: Option<PackId>,
    /// Situação.
    pub state: OperationState,
    /// Etapa atual.
    pub stage: Option<OperationStage>,
    /// Progresso da etapa atual.
    pub progress: Option<Progress>,
    /// Se "Cancelar" está disponível agora.
    pub cancellable: bool,
    /// Início (milissegundos desde 1970, UTC).
    #[specta(type = specta_typescript::Number)]
    pub started_at_ms: u64,
    /// Fim, quando terminou.
    #[specta(type = Option<specta_typescript::Number>)]
    pub finished_at_ms: Option<u64>,
    /// Erro, quando terminou com erro.
    pub error: Option<AppError>,
}

/// Quem recebe cada mudança (na app, emite `operation-updated`).
pub(crate) type Notifier = Arc<dyn Fn(&OperationSnapshot) + Send + Sync>;

/// Quem recebe os eventos do canal do comando.
pub(crate) type EventForwarder = Box<dyn Fn(OperationEvent) + Send + Sync>;

struct Running {
    snapshot: OperationSnapshot,
    token: CancellationToken,
    last_progress_notice: Option<Instant>,
}

#[derive(Default)]
struct Inner {
    running: BTreeMap<OperationId, Running>,
    finished: VecDeque<OperationSnapshot>,
}

struct Shared {
    inner: Mutex<Inner>,
    notifier: Notifier,
}

impl Shared {
    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Altera uma operação em andamento e avisa (fora da trava).
    fn update(&self, id: OperationId, change: impl FnOnce(&mut Running) -> bool) {
        let notice = {
            let mut inner = self.lock();
            let Some(running) = inner.running.get_mut(&id) else {
                return;
            };
            change(running).then(|| running.snapshot.clone())
        };
        if let Some(snapshot) = notice {
            (self.notifier)(&snapshot);
        }
    }
}

/// Registro das operações do app. Barato de clonar.
#[derive(Clone)]
pub struct OperationRegistry {
    shared: Arc<Shared>,
}

impl std::fmt::Debug for OperationRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let inner = self.shared.lock();
        f.debug_struct("OperationRegistry")
            .field("running", &inner.running.len())
            .field("finished", &inner.finished.len())
            .finish()
    }
}

impl OperationRegistry {
    /// Registro que avisa cada mudança a `notifier`.
    pub(crate) fn new(notifier: Notifier) -> Self {
        Self {
            shared: Arc::new(Shared {
                inner: Mutex::new(Inner::default()),
                notifier,
            }),
        }
    }

    /// Começa uma operação.
    #[must_use = "termine a operação com `finish`"]
    pub fn start(
        &self,
        kind: OperationKind,
        pack_id: Option<PackId>,
        cancellable: bool,
    ) -> OperationHandle {
        let id = OperationId::new();
        let token = CancellationToken::new();
        let snapshot = OperationSnapshot {
            id,
            kind,
            pack_id,
            state: OperationState::Running,
            stage: None,
            progress: None,
            cancellable,
            started_at_ms: now_ms(),
            finished_at_ms: None,
            error: None,
        };
        self.shared.lock().running.insert(
            id,
            Running {
                snapshot: snapshot.clone(),
                token: token.clone(),
                last_progress_notice: None,
            },
        );
        (self.shared.notifier)(&snapshot);
        OperationHandle {
            id,
            token,
            shared: Arc::clone(&self.shared),
            events: None,
            finished: false,
        }
    }

    /// Operações em andamento (mais antigas primeiro) seguidas das concluídas (mais recentes
    /// primeiro).
    #[must_use]
    pub fn list(&self) -> Vec<OperationSnapshot> {
        let inner = self.shared.lock();
        inner
            .running
            .values()
            .map(|running| running.snapshot.clone())
            .chain(inner.finished.iter().cloned())
            .collect()
    }

    /// Uma operação, em andamento ou concluída.
    #[must_use]
    pub fn get(&self, id: OperationId) -> Option<OperationSnapshot> {
        let inner = self.shared.lock();
        inner
            .running
            .get(&id)
            .map(|running| running.snapshot.clone())
            .or_else(|| {
                inner
                    .finished
                    .iter()
                    .find(|snapshot| snapshot.id == id)
                    .cloned()
            })
    }

    /// Pede o cancelamento. Cancelar o que já terminou não faz nada. Erros:
    /// `OPERATION_NOT_FOUND` e `OPERATION_NOT_CANCELLABLE`.
    pub fn cancel(&self, id: OperationId) -> Result<(), AppError> {
        let notice = {
            let mut inner = self.shared.lock();
            let already_finished = inner.finished.iter().any(|snapshot| snapshot.id == id);
            match inner.running.get_mut(&id) {
                Some(running) if !running.snapshot.cancellable => {
                    return Err(
                        AppError::new(AppErrorCode::OperationNotCancellable).in_operation(id)
                    );
                }
                Some(running) => {
                    running.token.cancel();
                    running.snapshot.state = OperationState::Cancelling;
                    Some(running.snapshot.clone())
                }
                None if already_finished => None,
                None => {
                    return Err(AppError::new(AppErrorCode::OperationNotFound)
                        .with_param("operationId", id.to_string()));
                }
            }
        };
        if let Some(snapshot) = notice {
            tracing::info!(operation_id = %id, kind = snapshot.kind.as_str(), "cancelamento pedido");
            (self.shared.notifier)(&snapshot);
        }
        Ok(())
    }
}

/// Uma operação em andamento. Termine com [`OperationHandle::finish`]; se o handle for
/// descartado sem isso (pânico, retorno antecipado), a operação termina como `INTERNAL`.
pub struct OperationHandle {
    id: OperationId,
    token: CancellationToken,
    shared: Arc<Shared>,
    events: Option<EventForwarder>,
    finished: bool,
}

impl std::fmt::Debug for OperationHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OperationHandle")
            .field("id", &self.id)
            .field("cancelled", &self.token.is_cancelled())
            .finish_non_exhaustive()
    }
}

impl OperationHandle {
    /// Repassa os eventos da operação ao canal do comando. Envia `Started` na hora.
    #[must_use]
    pub fn with_events(mut self, forward: impl Fn(OperationEvent) + Send + Sync + 'static) -> Self {
        let kind = self
            .snapshot()
            .map_or_else(|| OperationKind::new("unknown"), |snapshot| snapshot.kind);
        forward(OperationEvent::Started {
            operation_id: self.id,
            kind,
        });
        self.events = Some(Box::new(forward));
        self
    }

    /// Identificador.
    #[must_use]
    pub fn id(&self) -> OperationId {
        self.id
    }

    /// Token de cancelamento (para passar às crates de domínio).
    #[must_use]
    pub fn token(&self) -> &CancellationToken {
        &self.token
    }

    /// Se o cancelamento foi pedido.
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.token.is_cancelled()
    }

    /// `Err(CANCELLED)` se o cancelamento foi pedido; use entre etapas.
    pub fn ensure_not_cancelled(&self) -> Result<(), AppError> {
        if self.is_cancelled() {
            Err(AppError::cancelled().in_operation(self.id))
        } else {
            Ok(())
        }
    }

    /// Roda `work` até terminar ou até o cancelamento ser pedido (o que vier antes). No
    /// cancelamento, `work` é descartado e o resultado é `CANCELLED`.
    pub async fn run_cancellable<T>(
        &self,
        work: impl Future<Output = Result<T, AppError>>,
    ) -> Result<T, AppError> {
        tokio::select! {
            biased;
            () = self.token.cancelled() => Err(AppError::cancelled().in_operation(self.id)),
            result = work => result,
        }
    }

    /// Liga ou desliga "Cancelar" (desligue só no trecho que não pode ser interrompido).
    pub fn set_cancellable(&self, cancellable: bool) {
        self.shared.update(self.id, |running| {
            let changed = running.snapshot.cancellable != cancellable;
            running.snapshot.cancellable = cancellable;
            changed
        });
    }

    /// Marca a espera pela trava do pack (ou o fim dela).
    pub fn set_waiting_for_lock(&self, waiting: bool) {
        self.shared.update(self.id, |running| {
            let next = match (waiting, running.snapshot.state) {
                (true, OperationState::Running) => OperationState::WaitingForLock,
                (false, OperationState::WaitingForLock) => OperationState::Running,
                _ => return false,
            };
            running.snapshot.state = next;
            true
        });
    }

    /// Problema que não interrompe a operação: vai para o canal.
    pub fn warn(&self, error: AppError) {
        tracing::warn!(operation_id = %self.id, code = %error.code, "aviso na operação");
        self.send(OperationEvent::Warning {
            error: error.in_operation(self.id),
        });
    }

    /// Linhas de console para o canal.
    pub fn log(&self, lines: Vec<LogLine>) {
        if !lines.is_empty() {
            self.send(OperationEvent::Log { lines });
        }
    }

    /// Termina a operação com o resultado e o devolve (com o `operationId` no erro).
    pub fn finish<T>(mut self, result: Result<T, AppError>) -> Result<T, AppError> {
        let result = result.map_err(|error| error.in_operation(self.id));
        self.complete(result.as_ref().err().cloned());
        result
    }

    fn snapshot(&self) -> Option<OperationSnapshot> {
        self.shared
            .lock()
            .running
            .get(&self.id)
            .map(|running| running.snapshot.clone())
    }

    fn send(&self, event: OperationEvent) {
        if let Some(forward) = &self.events {
            forward(event);
        }
    }

    fn complete(&mut self, error: Option<AppError>) {
        if self.finished {
            return;
        }
        self.finished = true;
        let snapshot = {
            let mut inner = self.shared.lock();
            let Some(running) = inner.running.remove(&self.id) else {
                return;
            };
            let mut snapshot = running.snapshot;
            snapshot.state = match &error {
                None => OperationState::Succeeded,
                Some(error) if error.is_cancelled() => OperationState::Cancelled,
                Some(_) => OperationState::Failed,
            };
            snapshot.finished_at_ms = Some(now_ms());
            snapshot.cancellable = false;
            snapshot.error = error;
            inner.finished.push_front(snapshot.clone());
            inner.finished.truncate(FINISHED_KEPT);
            snapshot
        };
        match snapshot.state {
            OperationState::Failed => tracing::warn!(
                operation_id = %self.id,
                kind = snapshot.kind.as_str(),
                code = %snapshot.error.as_ref().map(|e| e.code.to_string()).unwrap_or_default(),
                "operação falhou"
            ),
            state => tracing::info!(
                operation_id = %self.id,
                kind = snapshot.kind.as_str(),
                ?state,
                "operação terminou"
            ),
        }
        self.send(OperationEvent::Finished);
        (self.shared.notifier)(&snapshot);
    }
}

impl ProgressSink for OperationHandle {
    fn stage(&self, stage: &str, label_key: &str) {
        self.shared.update(self.id, |running| {
            running.snapshot.stage = Some(OperationStage {
                id: stage.to_owned(),
                label_key: label_key.to_owned(),
            });
            running.snapshot.progress = None;
            running.last_progress_notice = None;
            true
        });
        self.send(OperationEvent::Stage {
            stage: stage.to_owned(),
            label_key: label_key.to_owned(),
        });
    }

    fn progress(&self, progress: Progress) {
        let now = Instant::now();
        self.shared.update(self.id, |running| {
            running.snapshot.progress = Some(progress);
            let done = progress
                .total
                .is_some_and(|total| progress.current >= total);
            let due = running
                .last_progress_notice
                .is_none_or(|last| now.duration_since(last) >= PROGRESS_INTERVAL);
            if done || due {
                running.last_progress_notice = Some(now);
            }
            done || due
        });
        self.send(OperationEvent::Progress(progress));
    }
}

impl Drop for OperationHandle {
    fn drop(&mut self) {
        if !self.finished {
            self.complete(Some(
                AppError::internal("a operação terminou sem informar o resultado")
                    .in_operation(self.id),
            ));
        }
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| {
            u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX)
        })
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::error::ErrorCode;
    use std::sync::Mutex as StdMutex;
    use tokio::sync::mpsc;
    use warden_core::CoreErrorCode;

    /// Registro que guarda cada aviso numa lista.
    pub(crate) fn recording_registry() -> (OperationRegistry, Arc<StdMutex<Vec<OperationSnapshot>>>)
    {
        let seen = Arc::new(StdMutex::new(Vec::new()));
        let sink = Arc::clone(&seen);
        let registry = OperationRegistry::new(Arc::new(move |snapshot: &OperationSnapshot| {
            sink.lock().unwrap().push(snapshot.clone());
        }));
        (registry, seen)
    }

    /// Registro que manda cada aviso para um canal assíncrono.
    pub(crate) fn channel_registry() -> (
        OperationRegistry,
        mpsc::UnboundedReceiver<OperationSnapshot>,
    ) {
        let (sender, receiver) = mpsc::unbounded_channel();
        let registry = OperationRegistry::new(Arc::new(move |snapshot: &OperationSnapshot| {
            let _ = sender.send(snapshot.clone());
        }));
        (registry, receiver)
    }

    const KIND: OperationKind = OperationKind::new("teste.simulada");

    #[test]
    fn ciclo_de_vida_com_sucesso() {
        let (registry, seen) = recording_registry();
        let pack = PackId::new();
        let events = Arc::new(StdMutex::new(Vec::new()));
        let sink = Arc::clone(&events);
        let handle = registry
            .start(KIND, Some(pack), true)
            .with_events(move |event| sink.lock().unwrap().push(event));
        let id = handle.id();
        handle.stage("baixar", "teste.etapa.baixar");
        handle.progress(Progress::items(1, Some(2)));
        handle.progress(Progress::items(2, Some(2)));
        assert_eq!(registry.list()[0].state, OperationState::Running);
        assert_eq!(handle.finish(Ok(7)).unwrap(), 7);

        let snapshot = registry.get(id).unwrap();
        assert_eq!(snapshot.state, OperationState::Succeeded);
        assert_eq!(snapshot.pack_id, Some(pack));
        assert_eq!(snapshot.progress, Some(Progress::items(2, Some(2))));
        assert!(snapshot.finished_at_ms.unwrap() >= snapshot.started_at_ms);
        assert!(!snapshot.cancellable);

        let states: Vec<_> = seen.lock().unwrap().iter().map(|s| s.state).collect();
        assert_eq!(states.first(), Some(&OperationState::Running));
        assert_eq!(states.last(), Some(&OperationState::Succeeded));

        let events = events.lock().unwrap();
        assert!(
            matches!(events[0], OperationEvent::Started { operation_id, .. } if operation_id == id)
        );
        assert!(matches!(events[1], OperationEvent::Stage { .. }));
        assert!(matches!(events.last(), Some(OperationEvent::Finished)));
    }

    #[test]
    fn progresso_frequente_e_agrupado_mas_o_final_sempre_avisa() {
        let (registry, seen) = recording_registry();
        let handle = registry.start(KIND, None, true);
        for current in 0..1000 {
            handle.progress(Progress::bytes(current, Some(1000)));
        }
        handle.progress(Progress::bytes(1000, Some(1000)));
        let notices = seen.lock().unwrap().len();
        // início + primeiro progresso + o final (o resto cai no intervalo de 100 ms).
        assert!(notices < 10, "{notices} avisos");
        let last = seen.lock().unwrap().last().unwrap().progress;
        assert_eq!(last, Some(Progress::bytes(1000, Some(1000))));
        handle.finish(Ok(())).unwrap();
    }

    #[test]
    fn erro_recebe_o_id_da_operacao() {
        let (registry, _) = recording_registry();
        let handle = registry.start(KIND, None, false);
        let id = handle.id();
        let error = handle
            .finish::<()>(Err(AppError::internal("x")))
            .unwrap_err();
        assert_eq!(error.operation_id, Some(id));
        let snapshot = registry.get(id).unwrap();
        assert_eq!(snapshot.state, OperationState::Failed);
        assert_eq!(snapshot.error.unwrap().operation_id, Some(id));
    }

    #[test]
    fn handle_descartado_termina_como_internal() {
        let (registry, _) = recording_registry();
        let id = registry.start(KIND, None, true).id();
        let snapshot = registry.get(id).unwrap();
        assert_eq!(snapshot.state, OperationState::Failed);
        assert_eq!(
            snapshot.error.unwrap().code,
            ErrorCode::App(AppErrorCode::Internal)
        );
    }

    #[test]
    fn cancelar_erros_e_casos_de_borda() {
        let (registry, _) = recording_registry();
        let unknown = OperationId::new();
        assert_eq!(
            registry.cancel(unknown).unwrap_err().code,
            ErrorCode::App(AppErrorCode::OperationNotFound)
        );

        let handle = registry.start(KIND, None, true);
        handle.set_cancellable(false);
        let error = registry.cancel(handle.id()).unwrap_err();
        assert_eq!(
            error.code,
            ErrorCode::App(AppErrorCode::OperationNotCancellable)
        );
        assert!(!handle.is_cancelled());
        handle.set_cancellable(true);
        registry.cancel(handle.id()).unwrap();
        assert!(handle.is_cancelled());
        assert_eq!(
            registry.get(handle.id()).unwrap().state,
            OperationState::Cancelling
        );
        let id = handle.id();
        let error = handle.finish::<()>(handle_err_cancelled()).unwrap_err();
        assert!(error.is_cancelled());
        assert_eq!(registry.get(id).unwrap().state, OperationState::Cancelled);
        // Cancelar de novo, já terminada: nada acontece.
        registry.cancel(id).unwrap();
    }

    fn handle_err_cancelled() -> Result<(), AppError> {
        Err(AppError::cancelled())
    }

    #[test]
    fn guarda_so_as_ultimas_50_concluidas() {
        let (registry, _) = recording_registry();
        let ids: Vec<_> = (0..60)
            .map(|_| {
                let handle = registry.start(KIND, None, true);
                let id = handle.id();
                handle.finish(Ok(())).unwrap();
                id
            })
            .collect();
        let running = registry.start(KIND, None, true);
        let list = registry.list();
        assert_eq!(list.len(), FINISHED_KEPT + 1);
        assert_eq!(list[0].id, running.id());
        assert_eq!(list[1].id, ids[59], "concluídas mais recentes primeiro");
        assert!(registry.get(ids[0]).is_none());
        running.finish(Ok(())).unwrap();
    }

    #[test]
    fn espera_pela_trava_aparece_no_estado() {
        let (registry, _) = recording_registry();
        let handle = registry.start(KIND, None, true);
        handle.set_waiting_for_lock(true);
        assert_eq!(
            registry.get(handle.id()).unwrap().state,
            OperationState::WaitingForLock
        );
        handle.set_waiting_for_lock(false);
        assert_eq!(
            registry.get(handle.id()).unwrap().state,
            OperationState::Running
        );
        handle.finish(Ok(())).unwrap();
    }

    #[test]
    fn aviso_vai_para_o_canal_com_o_id() {
        let (registry, _) = recording_registry();
        let events = Arc::new(StdMutex::new(Vec::new()));
        let sink = Arc::clone(&events);
        let handle = registry
            .start(KIND, None, true)
            .with_events(move |event| sink.lock().unwrap().push(event));
        handle.warn(AppError::internal("aviso"));
        handle.log(Vec::new());
        handle.log(vec![LogLine {
            source: crate::events::LogSource::Packwiz,
            text: "ok".into(),
        }]);
        let id = handle.id();
        handle.finish(Ok(())).unwrap();
        let events = events.lock().unwrap();
        assert!(matches!(
            &events[1],
            OperationEvent::Warning { error } if error.operation_id == Some(id)
        ));
        assert!(matches!(&events[2], OperationEvent::Log { lines } if lines.len() == 1));
        assert_eq!(events.len(), 4);
    }

    /// Critério 5 da F0-05: cancelar uma operação longa simulada encerra em menos de 2 s com
    /// `CANCELLED`.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn f0_05_ca5_cancelar_operacao_longa_encerra_em_menos_de_2s() {
        let (registry, mut notices) = channel_registry();
        let handle = registry.start(KIND, None, true);
        let id = handle.id();

        let task = tokio::spawn(async move {
            let result = handle
                .run_cancellable(async {
                    // Operação longa simulada: 10 minutos de trabalho em passos de 10 ms.
                    for step in 0..60_000u64 {
                        handle.progress(Progress::items(step, Some(60_000)));
                        tokio::time::sleep(Duration::from_millis(10)).await;
                    }
                    Ok(())
                })
                .await;
            handle.finish(result)
        });

        // Espera a operação começar a trabalhar (primeiro aviso de progresso).
        while let Some(snapshot) = notices.recv().await {
            if snapshot.id == id && snapshot.progress.is_some() {
                break;
            }
        }
        let asked = Instant::now();
        registry.cancel(id).unwrap();
        let result = tokio::time::timeout(Duration::from_secs(5), task)
            .await
            .expect("a operação não parou em 5 s")
            .unwrap();
        let elapsed = asked.elapsed();

        assert!(elapsed < Duration::from_secs(2), "parou em {elapsed:?}");
        let error = result.unwrap_err();
        assert_eq!(error.code, ErrorCode::Core(CoreErrorCode::Cancelled));
        assert_eq!(error.operation_id, Some(id));
        let snapshot = registry.get(id).unwrap();
        assert_eq!(snapshot.state, OperationState::Cancelled);
    }

    #[tokio::test]
    async fn ensure_not_cancelled_entre_etapas() {
        let (registry, _) = recording_registry();
        let handle = registry.start(KIND, None, true);
        handle.ensure_not_cancelled().unwrap();
        registry.cancel(handle.id()).unwrap();
        assert!(handle.ensure_not_cancelled().unwrap_err().is_cancelled());
        let finished = handle.run_cancellable(async { Ok(1) }).await;
        assert!(
            finished.unwrap_err().is_cancelled(),
            "cancelamento tem prioridade"
        );
        let _ = handle.finish::<()>(Err(AppError::cancelled()));
    }

    #[test]
    fn tipo_da_operacao_vira_texto() {
        assert_eq!(serde_json::to_value(KIND).unwrap(), "teste.simulada");
        assert_eq!(KIND.as_str(), "teste.simulada");
        assert!(OperationState::Cancelled.is_finished());
        assert!(!OperationState::Cancelling.is_finished());
    }
}
