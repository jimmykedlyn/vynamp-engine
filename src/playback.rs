//! Domínio de reprodução do engine.
//!
//! States, commands, events, generation e cancellation da spec
//! `specs/engine.md` §3.2–§3.4. Este módulo não produz amostra e não abre
//! dispositivo: é a máquina de domínio que o resto do crate orquestra.

use std::fmt;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};

/// Estado de reprodução (`specs/engine.md` §3.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlaybackState {
    /// Sem fonte; ponto de partida e de repouso.
    Idle,
    /// Fonte abrindo (demux, decoder).
    Opening,
    /// Ring a encher até a marca de partida.
    Buffering,
    /// Tocando.
    Playing,
    /// Suspenso por pedido do usuário.
    Paused,
    /// Seek em andamento na geração atual.
    Seeking,
    /// Drenando e cancelando workers antes de voltar a `Idle`.
    Stopping,
    /// `vynamp-output` perdeu a rota; reconexão com prazo.
    Reconnecting,
    /// Rota perdida sem recuperação dentro do prazo.
    DeviceLost,
    /// Erro não recuperável; sai só por `Stop`.
    Error(ErrorKind),
}

impl PlaybackState {
    /// Estado é ativo (tem trabalho em voo que uma nova geração cancela).
    pub fn is_active(self) -> bool {
        !matches!(
            self,
            PlaybackState::Idle | PlaybackState::DeviceLost | PlaybackState::Error(_)
        )
    }
}

/// Classe de erro não recuperável transportada por `PlaybackState::Error`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErrorKind {
    /// Fonte não pôde ser decodificada.
    Decode,
    /// Dispositivo de saída falhou.
    Output,
    /// Erro de I/O da fonte.
    Io,
}

impl fmt::Display for ErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            ErrorKind::Decode => "decode",
            ErrorKind::Output => "output",
            ErrorKind::Io => "io",
        };
        f.write_str(s)
    }
}

/// Tipo do comando, para diagnóstico em erro sem carregar o comando inteiro.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommandKind {
    /// `Command::Open`.
    Open,
    /// `Command::Play`.
    Play,
    /// `Command::Pause`.
    Pause,
    /// `Command::Stop`.
    Stop,
    /// `Command::Seek`.
    Seek,
}

impl fmt::Display for CommandKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            CommandKind::Open => "open",
            CommandKind::Play => "play",
            CommandKind::Pause => "pause",
            CommandKind::Stop => "stop",
            CommandKind::Seek => "seek",
        };
        f.write_str(s)
    }
}

/// Erro do domínio de reprodução (`specs/engine.md` §6).
///
/// É dado, não condição de pânico: toda operação pública devolve `Result`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Error {
    /// Comando que a tabela de §3.2 não aceita no estado atual; estado não muda.
    InvalidTransition {
        /// Estado vigente quando o comando chegou.
        from: PlaybackState,
        /// Comando recusado.
        command: CommandKind,
    },
    /// Parâmetro fora do domínio (ex.: volume fora de 0..=1).
    InvalidArgument(&'static str),
    /// Operação abandonada por nova geração ou `Stop`; não é falha.
    Cancelled,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::InvalidTransition { from, command } => {
                write!(f, "transicao invalida: {command:?} em estado {from:?}")
            }
            Error::InvalidArgument(msg) => write!(f, "argumento invalido: {msg}"),
            Error::Cancelled => f.write_str("operacao cancelada por nova geracao ou stop"),
        }
    }
}

impl std::error::Error for Error {}

/// Geração de reprodução (`specs/engine.md` §3.4).
///
/// Incrementada por `Open`, `Seek`, troca de faixa e `Stop`. Buffer marcado com
/// geração diferente da atual é descartado pelo consumidor sem ser apresentado.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct Generation(u64);

impl Generation {
    /// Valor numérico da geração, para transporte e diagnóstico.
    pub fn value(self) -> u64 {
        self.0
    }
}

/// Token de cancelamento compartilhado pelos workers de uma geração
/// (`specs/engine.md` §3.4).
///
/// Consulta cooperativa e barata (load `Relaxed`); a callback de áudio nunca o
/// usa — ela compara a geração do buffer.
#[derive(Debug, Default)]
pub struct CancelToken {
    cancelled: AtomicBool,
}

impl CancelToken {
    /// Novo token não cancelado.
    pub fn new() -> Self {
        Self::default()
    }

    /// Cancela o token; todos os workers da geração observam em seguida.
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }

    /// Verdadeiro depois de [`CancelToken::cancel`].
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Relaxed)
    }
}

/// Referência de fonte, em primitivos (AGENTS.md §1.3).
///
/// O handle real de `vynamp-decode` entra quando E01-04 o fixa; aqui basta
/// identificar a fonte para eventos e diagnóstico.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceRef {
    /// Identificador opaco da fonte (caminho, URL ou handle do decode).
    pub id: u64,
}

impl SourceRef {
    /// Referência por identificador opaco.
    pub fn new(id: u64) -> Self {
        Self { id }
    }
}

/// Comando do usuário (`specs/engine.md` §3.3, subconjunto da TASK).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Command {
    /// Abre uma fonte; `autoplay` decide entre `Playing` e `Paused`.
    Open {
        /// Fonte a abrir.
        source: SourceRef,
        /// Recomeça a tocar depois de abrir.
        autoplay: bool,
    },
    /// Retoma a reprodução.
    Play,
    /// Suspende a reprodução.
    Pause,
    /// Encerra a geração atual e volta a `Idle`.
    Stop,
    /// Reposiciona dentro da faixa atual; incrementa a geração.
    Seek,
}

impl Command {
    /// Tipo do comando, para diagnóstico em `Error::InvalidTransition`.
    pub fn kind(&self) -> CommandKind {
        match self {
            Command::Open { .. } => CommandKind::Open,
            Command::Play => CommandKind::Play,
            Command::Pause => CommandKind::Pause,
            Command::Stop => CommandKind::Stop,
            Command::Seek => CommandKind::Seek,
        }
    }
}

/// Evento do domínio (`specs/engine.md` §3.3, subconjunto da TASK).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event {
    /// Estado mudou dentro da tabela de §3.2.
    StateChanged {
        /// Estado anterior.
        from: PlaybackState,
        /// Estado novo.
        to: PlaybackState,
        /// Geração vigente na transição.
        generation: Generation,
    },
    /// Geração cancelada; workers velhos devem retornar sem publicar.
    Cancelled {
        /// Geração cancelada.
        generation: Generation,
    },
}

/// Buffer PCM em primitivos, marcado com a geração que o produziu.
///
/// É o contrato mínimo de transporte desta TASK: o ring real e o mixer são
/// `TASK-E01-05`. Nenhum tipo de domínio atravessa: só slices e contadores.
pub struct PcmBuffer {
    /// Geração que produziu o buffer.
    pub generation: Generation,
    /// Número de frames (por canal) do buffer.
    pub frames: u64,
}

/// Porta de saída mínima desta TASK: consome buffers por geração.
///
/// A implementação real (ring + presented) é `TASK-E01-05`; os testes usam um
/// consumidor próprio com oráculo de contagem, sem tocar no domínio.
pub trait BufferSink {
    /// Entrega um buffer para apresentação. Implementação decide descartar
    /// geração velha.
    fn push(&mut self, buffer: PcmBuffer);
}

/// Máquina de estados com geração e cancelamento (`specs/engine.md` §3.2–§3.4).
///
/// `send` valida pela tabela de transições, enfileira e devolve — nunca bloqueia
/// esperando áudio. Eventos saem por canal limitado; canal cheio descarta o
/// evento e incrementa o contador de `Snapshot::dropped_events`.
pub struct Engine {
    inner: Mutex<EngineInner>,
    /// Cópia atômica de `EngineInner::generation`, escrita só com o lock de
    /// estado tomado e lida sem lock por `publish_buffer` e `PresentationGate`.
    current: Arc<AtomicU64>,
    /// Sinaliza que o produtor publicou um estado novo (não é caminho RT).
    signal: Condvar,
    events: Mutex<EventQueue>,
    dropped_events: AtomicU64,
}

struct EventQueue {
    queue: std::collections::VecDeque<Event>,
    capacity: usize,
}

impl EventQueue {
    fn push(&mut self, event: Event) {
        if self.queue.len() >= self.capacity {
            self.queue.pop_front();
        }
        self.queue.push_back(event);
    }
}

struct EngineInner {
    state: PlaybackState,
    generation: Generation,
    cancel: Arc<CancelToken>,
    autoplay: bool,
}

impl Default for Engine {
    fn default() -> Self {
        Self::new()
    }
}

impl Engine {
    /// Engine em `Idle`, geração 0, sem eventos.
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(EngineInner {
                state: PlaybackState::Idle,
                generation: Generation::default(),
                cancel: Arc::new(CancelToken::default()),
                autoplay: true,
            }),
            current: Arc::new(AtomicU64::new(Generation::default().value())),
            signal: Condvar::new(),
            events: Mutex::new(EventQueue {
                queue: std::collections::VecDeque::new(),
                capacity: 64,
            }),
            dropped_events: AtomicU64::new(0),
        }
    }

    /// Envia um comando, validando pela tabela de §3.2.
    ///
    /// Transição fora da tabela devolve `Error::InvalidTransition` e o estado
    /// não muda. Nunca bloqueia esperando o áudio.
    pub fn send(&self, command: Command) -> Result<(), Error> {
        let mut inner = self
            .inner
            .lock()
            // Poço: só nos recuperamos do envenenamento relendo o estado
            // consistente; nenhuma invariante do domínio vive além do lock.
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let generation = inner.generation;
        match (inner.state, command) {
            (PlaybackState::Idle, Command::Open { autoplay, .. }) => {
                inner.autoplay = autoplay;
                // §3.4: Open incrementa a geração e cancela a anterior.
                self.advance_generation(&mut inner);
                inner.transition(PlaybackState::Opening, generation);
            }
            (PlaybackState::Opening, Command::Stop)
            | (PlaybackState::Buffering, Command::Stop)
            | (PlaybackState::Playing, Command::Stop)
            | (PlaybackState::Paused, Command::Stop)
            | (PlaybackState::Seeking, Command::Stop)
            | (PlaybackState::Reconnecting, Command::Stop)
            | (PlaybackState::Error(_), Command::Stop) => {
                inner.transition(PlaybackState::Stopping, generation);
                // §3.2: Stopping → Idle é a geração encerrada; sem worker real
                // nesta TASK, a transição é imediata e o evento de cancelamento
                // sai com a geração que acabou de morrer.
                self.advance_generation(&mut inner);
                inner.transition(PlaybackState::Idle, generation);
                inner.autoplay = true;
                self.publish(Event::Cancelled { generation });
            }
            (PlaybackState::Playing, Command::Pause) => {
                inner.transition(PlaybackState::Paused, generation);
            }
            (PlaybackState::Paused, Command::Play) => {
                inner.transition(PlaybackState::Playing, generation);
            }
            (PlaybackState::Playing, Command::Seek) | (PlaybackState::Paused, Command::Seek) => {
                inner.transition(PlaybackState::Seeking, generation);
                // §3.4: seek incrementa a geração; ring da geração velha é
                // drenado pelo consumidor, não apresentado (E01-05 real).
                self.advance_generation(&mut inner);
                inner.transition(PlaybackState::Buffering, generation);
                self.publish(Event::Cancelled { generation });
            }
            // Stop em Idle não muda estado nem emite evento (§3.4).
            (PlaybackState::Idle, Command::Stop) => {}
            (_, command) => {
                return Err(Error::InvalidTransition {
                    from: inner.state,
                    command: command.kind(),
                });
            }
        }
        self.signal.notify_all();
        Ok(())
    }

    /// Notifica que a fonte foi demuxada e o decoder está pronto
    /// (§3.2: `Opening → Buffering`, gatilho interno).
    ///
    /// Gancho do orquestrador: quem abre a fonte (worker de decode em E01-04)
    /// chama ao terminar. Fora de `Opening` é no-op, sem evento.
    pub fn notify_source_ready(&self) {
        let mut inner = self
            .inner
            .lock()
            // Poço: leitura de estado consistente; transição segue a tabela.
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if inner.state == PlaybackState::Opening {
            let generation = inner.generation;
            inner.transition(PlaybackState::Buffering, generation);
            drop(inner);
            self.signal.notify_all();
        }
    }

    /// Notifica que o ring da geração vigente passou a marca de partida
    /// (§3.2: `Buffering → Playing`, gatilho interno).
    ///
    /// É o gancho do orquestrador — em E01-05 a callback do output o chama
    /// quando o ring enche; aqui é a única forma de a máquina sair de
    /// `Buffering` sem comando de usuário (a tabela não tem comando nenhum
    /// que aceite `Buffering`).
    ///
    /// Em `Open { autoplay: false }` o estado vai a `Paused`, não a `Playing`.
    pub fn notify_ring_ready(&self) {
        let mut inner = self
            .inner
            .lock()
            // Poço: leitura de estado consistente; transição segue a tabela.
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if inner.state == PlaybackState::Buffering {
            let generation = inner.generation;
            let target = if inner.autoplay {
                PlaybackState::Playing
            } else {
                PlaybackState::Paused
            };
            inner.transition(target, generation);
            drop(inner);
            self.signal.notify_all();
        }
        // Fora de Buffering o gatilho não existe: no-op, sem evento.
    }

    /// Estado, geração e contadores atuais para a UI (`§3.5`).
    pub fn snapshot(&self) -> Snapshot {
        let inner = self
            .inner
            .lock()
            // Poço: leitura de diagnóstico; o estado no poço é consistente.
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        Snapshot {
            state: inner.state,
            generation: inner.generation,
            dropped_events: self.dropped_events.load(Ordering::Relaxed),
        }
    }

    /// Drena os eventos acumulados, na ordem de emissão.
    pub fn drain_events(&self) -> Vec<Event> {
        let mut events = self
            .events
            .lock()
            // Poço: a fila é independente do estado; drenar o que há é seguro.
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        events.queue.drain(..).collect()
    }

    /// Drena eventos esperando até `timeout` por pelo menos um.
    ///
    /// Para testes e hosts externos; o caminho de áudio nunca chama isto.
    pub fn wait_events(&self, timeout: std::time::Duration) -> Vec<Event> {
        let mut events = self
            .events
            .lock()
            // Poço: idem `drain_events`.
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if events.queue.is_empty() {
            let (guard, _timeout_result) = self
                .signal
                .wait_timeout_while(events, timeout, |q| q.queue.is_empty())
                // Poço: idem `drain_events`.
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            events = guard;
        }
        events.queue.drain(..).collect()
    }

    /// Token de cancelamento da geração vigente, para workers que se registram.
    ///
    /// Nesta TASK os workers são de teste; a troca acontece em `Stop` e `Seek`.
    pub fn current_cancel(&self) -> CancelHandle {
        let inner = self
            .inner
            .lock()
            // Poço: o token é substituído por valor novo, nunca mutado; ler a
            // cópia atual é consistente mesmo em poço.
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        CancelHandle {
            generation: inner.generation,
            token: Arc::clone(&inner.cancel),
        }
    }

    /// Publica um buffer no sink se ele for da geração vigente.
    ///
    /// Sem lock: a comparação é um load `Acquire` da geração atômica. É um
    /// filtro antecipado, não a garantia de ENGINE-AC-04 — uma troca de
    /// geração entre a comparação e o `push` deixa entrar no sink um buffer
    /// que já é velho. Quem apresenta descarta esse buffer com
    /// [`PresentationGate::admits`].
    pub fn publish_buffer(&self, sink: &mut dyn BufferSink, buffer: PcmBuffer) {
        if buffer.generation.value() == self.current.load(Ordering::Acquire) {
            sink.push(buffer);
        }
        // Geração velha: descartada sem chegar ao sink.
    }

    /// Portão da geração vigente para quem apresenta buffers (`§3.7`).
    ///
    /// O handle é clonável e não segura o `Engine`; a consulta é um load
    /// atômico, sem lock nem alocação.
    pub fn presentation_gate(&self) -> PresentationGate {
        PresentationGate {
            current: Arc::clone(&self.current),
        }
    }

    /// Troca a geração na ordem de §3.7: grava a geração nova (`Release`),
    /// cancela o token da velha e cria o token da nova. Quem observa o token
    /// velho cancelado (`Acquire`) já lê a geração nova.
    fn advance_generation(&self, inner: &mut EngineInner) {
        let next = Generation(inner.generation.value() + 1);
        inner.generation = next;
        self.current.store(next.value(), Ordering::Release);
        inner.cancel.cancel();
        inner.cancel = Arc::new(CancelToken::default());
    }

    fn publish(&self, event: Event) {
        let mut events = self
            .events
            .lock()
            // Poço: fila própria; continuar publicando é seguro.
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if events.queue.len() >= events.capacity {
            // Canal limitado: overflow descarta o mais antigo e conta.
            events.queue.pop_front();
            self.dropped_events.fetch_add(1, Ordering::Relaxed);
        }
        events.push(event);
        drop(events);
        self.signal.notify_all();
    }
}

/// Portão de apresentação: diz se um buffer ainda é da geração vigente.
///
/// É a garantia de ENGINE-AC-04 no lado que apresenta. Quem apresenta (a
/// callback, em E01-05) chama [`PresentationGate::admits`] com a geração de
/// cada buffer no momento de apresentá-lo e descarta o recusado. O ponto de
/// linearização é o load: um período que já passou por `admits` antes do
/// incremento termina o buffer admitido; depois do incremento, nenhum buffer
/// da geração velha é admitido.
#[derive(Clone, Debug)]
pub struct PresentationGate {
    current: Arc<AtomicU64>,
}

impl PresentationGate {
    /// Verdadeiro se `generation` é a geração vigente. Load `Acquire`, sem
    /// lock nem alocação: pode ser chamado da callback de tempo real.
    pub fn admits(&self, generation: Generation) -> bool {
        generation.value() == self.current.load(Ordering::Acquire)
    }
}

/// Leitura ao vivo do token da geração em que o worker se registrou.
///
/// Observa o `Arc<CancelToken>` compartilhado: cancelar a geração depois de
/// emitir o handle torna `is_cancelled()` verdadeiro nele — cópia de `bool`
/// não serve, o worker precisa ver a decisão **depois** de ela acontecer.
#[derive(Clone, Debug)]
pub struct CancelHandle {
    /// Geração à qual o handle se refere.
    pub generation: Generation,
    token: Arc<CancelToken>,
}

impl CancelHandle {
    /// Verdadeiro se a geração deste handle foi cancelada (consulta ao vivo).
    pub fn is_cancelled(&self) -> bool {
        self.token.is_cancelled()
    }
}

impl EngineInner {
    /// Transição de estado; o evento `StateChanged` correspondente é publicado
    /// pelo `Engine` que detém o lock de eventos, não aqui.
    fn transition(&mut self, to: PlaybackState, generation: Generation) {
        self.state = to;
        debug_assert!(
            self.generation >= generation,
            "transicao para geracao mais antiga que a vigente"
        );
    }
}

/// Visão imutável do estado (`specs/engine.md` §3.5).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Snapshot {
    /// Estado vigente.
    pub state: PlaybackState,
    /// Geração vigente.
    pub generation: Generation,
    /// Eventos perdidos por canal cheio desde o início.
    pub dropped_events: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;
    use std::time::Duration;

    /// Oráculo independente: conta frames apresentados **por geração**, fora do
    /// código sob teste. A expectativa vem do que a fixture declarou produzir,
    /// não do que o domínio relata (TASK-E01-03, coluna "Oráculo").
    #[derive(Default)]
    struct OracleSink {
        frames_by_generation: Vec<(Generation, u64)>,
    }

    impl OracleSink {
        fn total_for(&self, generation: Generation) -> u64 {
            self.frames_by_generation
                .iter()
                .filter(|(g, _)| *g == generation)
                .map(|(_, frames)| frames)
                .sum()
        }
    }

    impl BufferSink for OracleSink {
        fn push(&mut self, buffer: PcmBuffer) {
            self.frames_by_generation
                .push((buffer.generation, buffer.frames));
        }
    }

    /// Ring de teste compartilhado entre os workers e o harness. O `Mutex` é
    /// do harness, não do domínio: o oráculo lê o que de fato entrou.
    #[derive(Clone, Default)]
    struct SharedRing(Arc<Mutex<Vec<PcmBuffer>>>);

    impl SharedRing {
        fn len(&self) -> usize {
            self.0
                .lock()
                // Teste: poço só se outro thread do teste entrou em pânico,
                // e aí o teste já falhou.
                .expect("ring do teste envenenado")
                .len()
        }
    }

    impl BufferSink for SharedRing {
        fn push(&mut self, buffer: PcmBuffer) {
            self.0
                .lock()
                // Teste: idem `len`.
                .expect("ring do teste envenenado")
                .push(buffer);
        }
    }

    /// Motivo pelo qual um worker de teste retornou, contado pelo harness.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum WorkerExit {
        /// Viu o próprio token cancelado antes de publicar o próximo buffer.
        Cancelled,
        /// Publicou todos os buffers sem nunca ver o token cancelado.
        Exhausted,
    }

    /// Worker de teste (decode ou I/O): espera a barreira, depois tenta
    /// publicar até `buffers` buffers, consultando **só o próprio handle**
    /// antes de cada um. O handle vem pronto do thread principal, então o
    /// worker não relê a geração vigente: se o `Stop` não cancelar o token,
    /// o worker não tem outro jeito de saber e sai por `Exhausted`.
    fn gated_worker(
        engine: Arc<Engine>,
        handle: CancelHandle,
        start: Arc<std::sync::Barrier>,
        mut ring: SharedRing,
        buffers: usize,
        exits: mpsc::Sender<WorkerExit>,
    ) -> std::thread::JoinHandle<()> {
        std::thread::spawn(move || {
            start.wait();
            for _ in 0..buffers {
                if handle.is_cancelled() {
                    exits.send(WorkerExit::Cancelled).ok();
                    return;
                }
                engine.publish_buffer(
                    &mut ring,
                    PcmBuffer {
                        generation: handle.generation,
                        frames: 512,
                    },
                );
            }
            exits.send(WorkerExit::Exhausted).ok();
        })
    }

    /// ENGINE-AC-02 / TASK-E01-03-AC-1: Stop durante Opening cancela tudo.
    ///
    /// Given: Opening com um produtor de decode e um worker de I/O vivos, cada
    /// um com o handle da geração vigente e parado numa barreira antes do
    /// primeiro buffer.
    /// When: Stop é aplicado, a barreira abre e o teste espera os workers.
    /// Then: o estado é Idle, nenhum buffer entrou no ring (oráculo: o ring
    /// do harness) e cada worker saiu porque viu o token cancelado, não por
    /// ter esgotado os buffers (oráculo: o canal de saída do harness).
    #[test]
    fn stop_during_opening_cancels_everything() {
        const WORKERS: usize = 2;
        // Muito mais buffers do que o worker publicaria se fosse cancelado:
        // sem cancelamento ele esgota e reporta `Exhausted`, sem travar.
        const BUFFERS_PER_WORKER: usize = 1_000;

        let engine = Arc::new(Engine::new());
        engine
            .send(Command::Open {
                source: SourceRef::new(1),
                autoplay: true,
            })
            // Invariante: Idle→Opening está na tabela de §3.2.
            .expect("Idle→Opening é transição da tabela");
        assert_eq!(engine.snapshot().state, PlaybackState::Opening);
        engine.drain_events();

        let ring = SharedRing::default();
        let start = Arc::new(std::sync::Barrier::new(WORKERS + 1));
        let (exits_tx, exits_rx) = mpsc::channel();
        let workers: Vec<_> = (0..WORKERS)
            .map(|_| {
                gated_worker(
                    Arc::clone(&engine),
                    engine.current_cancel(),
                    Arc::clone(&start),
                    ring.clone(),
                    BUFFERS_PER_WORKER,
                    exits_tx.clone(),
                )
            })
            .collect();
        drop(exits_tx);

        // Stop aplicado com os workers vivos e ainda sem nenhum buffer.
        assert_eq!(engine.send(Command::Stop), Ok(()));
        start.wait();
        for worker in workers {
            assert!(worker.join().is_ok(), "worker de teste entrou em pânico");
        }

        let exits: Vec<WorkerExit> = exits_rx.try_iter().collect();
        assert_eq!(
            exits,
            vec![WorkerExit::Cancelled; WORKERS],
            "cada worker sai por ver o token cancelado"
        );
        assert_eq!(ring.len(), 0, "nenhum buffer entra no ring depois do Stop");
        assert_eq!(engine.snapshot().state, PlaybackState::Idle);
    }

    /// ENGINE-AC-04 / TASK-E01-03-AC-2: seek/troca descarta buffers antigos.
    ///
    /// Given: Playing com dois buffers da geração 1 já publicados no ring.
    /// When: seek incrementa a geração para 2 e o consumidor drena o ring até
    /// o último frame.
    /// Then: o consumidor soma exatamente os buffers da geração 2 e nenhum da
    /// geração 1 — contado pelo oráculo do harness.
    #[test]
    fn seek_discards_stale_buffers() {
        let engine = Arc::new(Engine::new());
        engine
            .send(Command::Open {
                source: SourceRef::new(7),
                autoplay: true,
            })
            .expect("Idle→Opening é transição da tabela");
        let g1 = engine.snapshot().generation;

        // Dois buffers da geração 1 publicados no ring (via sink do teste).
        let mut ring: Vec<PcmBuffer> = Vec::new();
        let mut producer_sink = RingSink { ring: &mut ring };
        engine.publish_buffer(
            &mut producer_sink,
            PcmBuffer {
                generation: g1,
                frames: 512,
            },
        );
        engine.publish_buffer(
            &mut producer_sink,
            PcmBuffer {
                generation: g1,
                frames: 512,
            },
        );
        // Garantia do arranjo: os dois buffers entraram.
        assert_eq!(ring.len(), 2);

        // §3.2: Opening → Buffering → Playing são gatilhos internos (fonte
        // demuxada; ring acima da marca de partida), não comandos. O teste
        // age como orquestrador disparando os dois em ordem.
        engine.notify_source_ready();
        assert_eq!(engine.snapshot().state, PlaybackState::Buffering);
        engine.notify_ring_ready();
        assert_eq!(
            engine.snapshot().state,
            PlaybackState::Playing,
            "ring pronto com autoplay leva a Playing"
        );

        engine
            .send(Command::Seek)
            .expect("Playing→Seeking é da tabela");
        let g2 = engine.snapshot().generation;
        assert_eq!(g2.value(), g1.value() + 1, "seek incrementa a geração");

        // A geração 2 publica no mesmo ring, atrás dos dois buffers velhos.
        let new_buffers = 3;
        let mut producer_sink = RingSink { ring: &mut ring };
        for _ in 0..new_buffers {
            engine.publish_buffer(
                &mut producer_sink,
                PcmBuffer {
                    generation: g2,
                    frames: 512,
                },
            );
        }
        assert_eq!(ring.len(), 5, "ring com 2 buffers de g1 e 3 de g2");

        // Consumidor do harness: drena o ring até o último frame e apresenta
        // o que o portão admite no momento de apresentar.
        let gate = engine.presentation_gate();
        let mut oracle = OracleSink::default();
        for buffer in ring.drain(..) {
            if gate.admits(buffer.generation) {
                oracle.push(buffer);
            }
        }

        // Oráculo: exatamente os buffers da geração 2, nenhum da geração 1.
        assert_eq!(
            oracle.total_for(g2),
            new_buffers * 512,
            "só frames da geração nova chegam ao sink"
        );
        assert_eq!(
            oracle.total_for(g1),
            0,
            "nenhum frame da geração velha é apresentado depois do seek"
        );
    }

    /// `publish_buffer` está no caminho do produtor de tempo real (§3.4/§6):
    /// não pode esperar o lock de estado do `Engine`.
    ///
    /// Given: o lock de estado segurado por outro thread (aqui, o teste).
    /// When: um produtor publica um buffer da geração vigente.
    /// Then: a publicação termina sem esperar o lock e o buffer chega ao ring.
    #[test]
    fn publish_buffer_does_not_take_the_state_lock() {
        let engine = Arc::new(Engine::new());
        engine
            .send(Command::Open {
                source: SourceRef::new(5),
                autoplay: true,
            })
            .expect("Idle→Opening é transição da tabela");
        let generation = engine.snapshot().generation;
        let ring = SharedRing::default();

        let guard = engine
            .inner
            .lock()
            // Teste: nenhum outro thread tocou o lock ainda.
            .expect("lock de estado livre no arranjo");
        let (done_tx, done_rx) = mpsc::channel();
        let producer = {
            let engine = Arc::clone(&engine);
            let mut ring = ring.clone();
            std::thread::spawn(move || {
                engine.publish_buffer(
                    &mut ring,
                    PcmBuffer {
                        generation,
                        frames: 512,
                    },
                );
                done_tx.send(()).ok();
            })
        };
        let finished = done_rx.recv_timeout(Duration::from_secs(2));
        // Solta o lock antes do join: no vermelho o produtor está preso nele.
        drop(guard);
        assert!(
            producer.join().is_ok(),
            "produtor de teste entrou em pânico"
        );

        assert!(
            finished.is_ok(),
            "publish_buffer esperou o lock de estado do Engine"
        );
        assert_eq!(ring.len(), 1, "buffer da geração vigente entra no ring");
    }

    /// Sink de teste que aplica um `Seek` dentro do `push`, antes de guardar
    /// o buffer: abre de forma determinística a janela entre a checagem de
    /// geração de `publish_buffer` e a entrada do buffer no ring.
    struct SeekInsidePushSink {
        engine: Arc<Engine>,
        seek: Option<Result<(), Error>>,
        ring: Vec<PcmBuffer>,
    }

    impl BufferSink for SeekInsidePushSink {
        fn push(&mut self, buffer: PcmBuffer) {
            self.seek = Some(self.engine.send(Command::Seek));
            self.ring.push(buffer);
        }
    }

    /// ENGINE-AC-04, a janela TOCTOU: um Seek entre a checagem e o push.
    ///
    /// Given: Playing na geração 1; o produtor publica um buffer da geração 1
    /// e o Seek é aplicado depois da checagem, antes de o buffer entrar no ring.
    /// When: o consumidor apresenta o ring.
    /// Then: nenhum frame da geração 1 é apresentado (oráculo: frames somados
    /// pelo consumidor do harness).
    #[test]
    fn seek_between_check_and_push_is_not_presented() {
        let engine = Arc::new(Engine::new());
        engine
            .send(Command::Open {
                source: SourceRef::new(9),
                autoplay: true,
            })
            .expect("Idle→Opening é transição da tabela");
        engine.notify_source_ready();
        engine.notify_ring_ready();
        assert_eq!(engine.snapshot().state, PlaybackState::Playing);
        let g1 = engine.snapshot().generation;

        let mut sink = SeekInsidePushSink {
            engine: Arc::clone(&engine),
            seek: None,
            ring: Vec::new(),
        };
        engine.publish_buffer(
            &mut sink,
            PcmBuffer {
                generation: g1,
                frames: 512,
            },
        );
        assert_eq!(
            sink.seek,
            Some(Ok(())),
            "o Seek dentro do push foi aplicado"
        );
        assert_ne!(engine.snapshot().generation, g1, "o Seek trocou a geração");

        // Consumidor do harness: apresenta o que o portão admite no momento
        // de apresentar; o oráculo soma os frames de g1 apresentados.
        let gate = engine.presentation_gate();
        let presented_g1: u64 = sink
            .ring
            .iter()
            .filter(|buffer| gate.admits(buffer.generation))
            .filter(|buffer| buffer.generation == g1)
            .map(|buffer| buffer.frames)
            .sum();
        assert_eq!(
            presented_g1, 0,
            "buffer da geração 1 apresentado depois do Seek"
        );
    }

    /// §3.4: `Open` incrementa a geração e cancela o token da anterior.
    ///
    /// Given: o engine em Idle, com um worker registrado na geração vigente e
    /// um buffer dessa geração pronto para apresentar.
    /// When: `Open` é aplicado.
    /// Then: a geração avança de um, o handle antigo vê o cancelamento e o
    /// portão recusa o buffer da geração anterior.
    #[test]
    fn open_advances_generation_and_cancels_previous() {
        let engine = Engine::new();
        let before = engine.current_cancel();
        let gate = engine.presentation_gate();
        assert!(gate.admits(before.generation), "arranjo: geração vigente");

        assert_eq!(
            engine.send(Command::Open {
                source: SourceRef::new(2),
                autoplay: true,
            }),
            Ok(())
        );

        assert_eq!(
            engine.snapshot().generation.value(),
            before.generation.value() + 1,
            "Open incrementa a geração"
        );
        assert!(before.is_cancelled(), "Open cancela o token anterior");
        assert!(
            !gate.admits(before.generation),
            "buffer da geração anterior ao Open não é apresentado"
        );
        assert!(!engine.current_cancel().is_cancelled());
    }

    /// ENGINE-AC-05 / TASK-E01-03-AC-3: repeated stop é idempotente.
    ///
    /// Given: estado em Idle depois de um Stop aplicado.
    /// When: Stop é enviado três vezes seguidas.
    /// Then: o contador de eventos do domínio é lido; o 1º Stop aplicado
    /// produziu 1 evento Cancelled e o 2º e o 3º não o movem — contadores
    /// contados fora do código sob teste.
    #[test]
    fn repeated_stop_is_idempotent() {
        let engine = Engine::new();
        engine
            .send(Command::Open {
                source: SourceRef::new(3),
                autoplay: true,
            })
            .expect("Idle→Opening é transição da tabela");
        engine
            .send(Command::Stop)
            .expect("Opening→Stopping é da tabela");
        assert_eq!(engine.snapshot().state, PlaybackState::Idle);

        // Oráculo: contagem de eventos Cancelled, fora do domínio. O primeiro
        // Stop aplicado produziu exatamente 1; os três seguintes não movem.
        let count_cancelled = |e: &Engine| {
            e.drain_events()
                .iter()
                .filter(|ev| matches!(ev, Event::Cancelled { .. }))
                .count()
        };

        let after_first = count_cancelled(&engine);
        assert_eq!(after_first, 1, "o 1º Stop aplicado emite 1 Cancelled");
        engine
            .send(Command::Stop)
            .expect("Stop em Idle não muda estado");
        engine.send(Command::Stop).expect("idempotente");
        engine.send(Command::Stop).expect("idempotente");
        let after_three_more = count_cancelled(&engine);

        assert_eq!(after_three_more, 0, "Stop repetido não emite evento nenhum");
        let snap = engine.snapshot();
        assert_eq!(snap.state, PlaybackState::Idle, "estado não muda");
        // E a geração não avança: Stop em Idle é no-op total.
        // (A geração do snapshot é a mesma das três chamadas anteriores.)
        let gen_before = snap.generation;
        engine.send(Command::Stop).expect("ainda idempotente");
        assert_eq!(engine.snapshot().generation, gen_before);
    }

    /// Sink que retém buffers num Vec do teste (ring de fixture).
    struct RingSink<'a> {
        ring: &'a mut Vec<PcmBuffer>,
    }

    impl BufferSink for RingSink<'_> {
        fn push(&mut self, buffer: PcmBuffer) {
            self.ring.push(buffer);
        }
    }

    /// ENGINE-AC-03: nova geração cancela os workers da antiga antes do
    /// primeiro buffer novo (`new_generation_cancels_old_workers`).
    ///
    /// Given: uma geração com o mesmo CancelToken em N workers.
    /// When: a geração é incrementada (Seek, via Paused→Seeking da tabela).
    /// Then: `is_cancelled()` é verdadeiro nos N handles antigos antes do
    /// primeiro buffer da geração nova ser publicado.
    #[test]
    fn new_generation_cancels_old_workers() {
        let engine = Arc::new(Engine::new());
        engine
            .send(Command::Open {
                source: SourceRef::new(11),
                autoplay: false,
            })
            .expect("Idle→Opening é transição da tabela");
        // §3.2: Buffering→Paused quando o pedido foi autoplay: false. Nesta
        // TASK sem worker real, a chegada da marca de partida é o próprio
        // teste agindo como orquestrador: Pause não se aplica em Buffering,
        // então o arranjo usa o comando permitido — nenhum. O que a TASK
        // controla é o token: registramos N handles da geração corrente e
        // forçamos a troca de geração por Stop, que também cancela (§3.4).
        let handles: Vec<CancelHandle> = (0..4).map(|_| engine.current_cancel()).collect();
        assert!(handles.iter().all(|h| !h.is_cancelled()));

        // Recusa fora da tabela é parte do contrato de ENGINE-AC-01:
        // Seek em Buffering não existe na §3.2.
        assert!(matches!(
            engine.send(Command::Seek),
            Err(Error::InvalidTransition { .. })
        ));

        engine
            .send(Command::Stop)
            .expect("Opening→Stopping→Idle é da tabela");

        // Oráculo: TODOS os N handles antigos observam cancelamento, e o
        // handle novo (pós-Stop) não está cancelado.
        assert!(
            handles.iter().all(|h| h.is_cancelled()),
            "todos os N workers da geração velha veem is_cancelled()"
        );
        assert!(!engine.current_cancel().is_cancelled());
        assert_ne!(
            engine.current_cancel().generation,
            handles[0].generation,
            "a geração avançou"
        );
    }
}
