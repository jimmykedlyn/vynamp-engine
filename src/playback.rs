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
/// Incrementada por `Open`, `Seek` e `Stop` (fora de `Idle`). A troca de faixa
/// de §3.4 ainda não existe: depende de `Enqueue` (`specs/engine.md` §3.7).
/// Buffer marcado com geração diferente da atual é descartado sem ser
/// apresentado.
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
/// Consulta cooperativa e barata: `cancel` grava com `Release` e `is_cancelled`
/// lê com `Acquire`, então quem vê o token cancelado vê também o que foi escrito
/// antes do cancelamento (a geração nova, em `Engine`). A callback de áudio nunca o
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
        self.cancelled.store(true, Ordering::Release);
    }

    /// Verdadeiro depois de [`CancelToken::cancel`].
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
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
        /// Toca (`Playing`) ou fica em `Paused` quando o ring fica pronto.
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
    /// Enfileira `event`; com a fila cheia, descarta o mais antigo antes.
    /// Devolve `true` se descartou: é o único lugar que trata o overflow.
    fn push(&mut self, event: Event) -> bool {
        let dropped = self.queue.len() >= self.capacity;
        if dropped {
            self.queue.pop_front();
        }
        self.queue.push_back(event);
        dropped
    }
}

struct EngineInner {
    state: PlaybackState,
    generation: Generation,
    cancel: Arc<CancelToken>,
    /// Destino de `Buffering` quando o ring fica pronto: `Playing` ou
    /// `Paused`. O `Open` grava pelo `autoplay`; o `Seek` grava o estado em
    /// que chegou, para voltar a ele (§3.7).
    resume: PlaybackState,
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
                resume: PlaybackState::Playing,
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
        match (inner.state, command) {
            (PlaybackState::Idle, Command::Open { autoplay, .. }) => {
                inner.resume = if autoplay {
                    PlaybackState::Playing
                } else {
                    PlaybackState::Paused
                };
                // §3.4: Open incrementa a geração e cancela a anterior. Idle
                // não tem trabalho em voo, então não sai `Cancelled` (§3.7).
                self.advance_generation(&mut inner);
                self.transition(&mut inner, PlaybackState::Opening);
            }
            (PlaybackState::Opening, Command::Stop)
            | (PlaybackState::Buffering, Command::Stop)
            | (PlaybackState::Playing, Command::Stop)
            | (PlaybackState::Paused, Command::Stop)
            | (PlaybackState::Seeking, Command::Stop)
            | (PlaybackState::Reconnecting, Command::Stop)
            | (PlaybackState::Error(_), Command::Stop) => {
                self.transition(&mut inner, PlaybackState::Stopping);
                // §3.2: Stopping → Idle é a geração encerrada; sem worker real
                // nesta TASK, a transição é imediata e o evento de cancelamento
                // sai com a geração que acabou de morrer.
                let ended = self.advance_generation(&mut inner);
                self.publish(Event::Cancelled { generation: ended });
                self.transition(&mut inner, PlaybackState::Idle);
            }
            (PlaybackState::Playing, Command::Pause) => {
                self.transition(&mut inner, PlaybackState::Paused);
            }
            (PlaybackState::Paused, Command::Play) => {
                self.transition(&mut inner, PlaybackState::Playing);
            }
            (PlaybackState::Playing, Command::Seek) | (PlaybackState::Paused, Command::Seek) => {
                // §3.7: depois do Seek volta ao estado em que ele chegou.
                inner.resume = inner.state;
                self.transition(&mut inner, PlaybackState::Seeking);
                // §3.4: seek incrementa a geração; ring da geração velha é
                // drenado pelo consumidor, não apresentado (E01-05 real).
                let ended = self.advance_generation(&mut inner);
                self.publish(Event::Cancelled { generation: ended });
                self.transition(&mut inner, PlaybackState::Buffering);
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
            self.transition(&mut inner, PlaybackState::Buffering);
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
    /// O destino é `Playing` ou `Paused`: depois de um `Open`, pelo
    /// `autoplay`; depois de um `Seek`, o estado em que o `Seek` chegou.
    pub fn notify_ring_ready(&self) {
        let mut inner = self
            .inner
            .lock()
            // Poço: leitura de estado consistente; transição segue a tabela.
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if inner.state == PlaybackState::Buffering {
            let target = inner.resume;
            self.transition(&mut inner, target);
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
    /// Nesta TASK os workers são de teste; a troca acontece em `Open`, `Seek` e `Stop`.
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

    /// Ponto único de mudança de estado: grava `to` e emite o `StateChanged`
    /// com a geração vigente no estado novo (§3.7). Chamado com o lock de
    /// estado tomado; a ordem de locks é sempre estado → eventos.
    fn transition(&self, inner: &mut EngineInner, to: PlaybackState) {
        let from = inner.state;
        inner.state = to;
        self.publish(Event::StateChanged {
            from,
            to,
            generation: inner.generation,
        });
    }

    /// Troca a geração na ordem de §3.7: grava a geração nova (`Release`),
    /// cancela o token da velha e cria o token da nova. Quem observa o token
    /// velho cancelado (`Acquire`) já lê a geração nova. Devolve a geração
    /// encerrada.
    fn advance_generation(&self, inner: &mut EngineInner) -> Generation {
        let ended = inner.generation;
        let next = Generation(ended.value() + 1);
        inner.generation = next;
        self.current.store(next.value(), Ordering::Release);
        inner.cancel.cancel();
        inner.cancel = Arc::new(CancelToken::default());
        ended
    }

    fn publish(&self, event: Event) {
        let mut events = self
            .events
            .lock()
            // Poço: fila própria; continuar publicando é seguro.
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if events.push(event) {
            // Canal limitado: o overflow descartou o mais antigo; conta.
            self.dropped_events.fetch_add(1, Ordering::Relaxed);
        }
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

    /// Geração vigente. Load `Acquire`, sem lock nem alocação.
    ///
    /// Serve a quem apresenta e precisa saber **qual** é a geração (para
    /// zerar a base do relógio quando ela troca); `Generation` continua opaca,
    /// só o `Engine` a incrementa.
    pub fn current(&self) -> Generation {
        Generation(self.current.load(Ordering::Acquire))
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

    /// Contador de eventos do harness: drena a fila do engine e guarda tudo o
    /// que saiu, de qualquer variante, na ordem. O oráculo é o que o harness
    /// viu sair, não um contador do domínio.
    #[derive(Default)]
    struct EventLog {
        seen: Vec<Event>,
    }

    impl EventLog {
        /// Drena o engine e devolve só os eventos novos desta leitura.
        fn take(&mut self, engine: &Engine) -> Vec<Event> {
            let new = engine.drain_events();
            self.seen.extend(new.iter().copied());
            new
        }

        fn count(&self) -> usize {
            self.seen.len()
        }
    }

    fn changed(from: PlaybackState, to: PlaybackState, generation: u64) -> Event {
        Event::StateChanged {
            from,
            to,
            generation: Generation(generation),
        }
    }

    fn cancelled(generation: u64) -> Event {
        Event::Cancelled {
            generation: Generation(generation),
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
    /// do harness), cada worker saiu porque viu o token cancelado, não por
    /// ter esgotado os buffers (oráculo: o canal de saída do harness), e os
    /// eventos que saíram são exatamente os da sequência de §3.7 (oráculo: o
    /// `EventLog` do harness, que conta todas as variantes).
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
        let mut log = EventLog::default();
        assert_eq!(
            log.take(&engine),
            vec![changed(PlaybackState::Idle, PlaybackState::Opening, 1)],
            "Open emite só Idle→Opening, já na geração 1"
        );

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
        assert_eq!(
            log.take(&engine),
            vec![
                changed(PlaybackState::Opening, PlaybackState::Stopping, 1),
                cancelled(1),
                changed(PlaybackState::Stopping, PlaybackState::Idle, 2),
            ],
            "Stop em Opening emite exatamente esta sequência"
        );
        assert_eq!(log.count(), 4, "total de eventos do Open e do Stop");
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

    /// §3.7: toda transição emite `StateChanged`, com a geração vigente no
    /// estado novo, inclusive as dos gatilhos internos.
    ///
    /// Given: o engine em Idle.
    /// When: percorre Open, fonte pronta, ring pronto, Pause, Play, Seek, ring
    /// pronto e Stop.
    /// Then: o `EventLog` do harness vê exatamente a sequência abaixo, uma
    /// entrada por transição, e nada mais.
    #[test]
    fn every_transition_emits_state_changed() {
        use PlaybackState::{Buffering, Idle, Opening, Paused, Playing, Seeking, Stopping};
        let engine = Engine::new();
        let mut log = EventLog::default();

        assert_eq!(
            engine.send(Command::Open {
                source: SourceRef::new(4),
                autoplay: true,
            }),
            Ok(())
        );
        engine.notify_source_ready();
        engine.notify_ring_ready();
        assert_eq!(engine.send(Command::Pause), Ok(()));
        assert_eq!(engine.send(Command::Play), Ok(()));
        assert_eq!(engine.send(Command::Seek), Ok(()));
        engine.notify_ring_ready();
        assert_eq!(engine.send(Command::Stop), Ok(()));

        assert_eq!(
            log.take(&engine),
            vec![
                changed(Idle, Opening, 1),
                changed(Opening, Buffering, 1),
                changed(Buffering, Playing, 1),
                changed(Playing, Paused, 1),
                changed(Paused, Playing, 1),
                changed(Playing, Seeking, 1),
                cancelled(1),
                changed(Seeking, Buffering, 2),
                changed(Buffering, Playing, 2),
                changed(Playing, Stopping, 2),
                cancelled(2),
                changed(Stopping, Idle, 3),
            ]
        );

        // Gatilho fora do estado de origem: no-op, sem evento.
        engine.notify_source_ready();
        engine.notify_ring_ready();
        assert_eq!(log.take(&engine), Vec::new());
    }

    /// Leva o engine até `target` (Playing ou Paused), passando pelo Open
    /// com `autoplay` e, se preciso, por um Pause ou Play.
    fn engine_in(autoplay: bool, target: PlaybackState) -> Engine {
        let engine = Engine::new();
        assert_eq!(
            engine.send(Command::Open {
                source: SourceRef::new(6),
                autoplay,
            }),
            Ok(())
        );
        engine.notify_source_ready();
        engine.notify_ring_ready();
        let command = match (engine.snapshot().state, target) {
            (PlaybackState::Playing, PlaybackState::Paused) => Some(Command::Pause),
            (PlaybackState::Paused, PlaybackState::Playing) => Some(Command::Play),
            _ => None,
        };
        if let Some(command) = command {
            assert_eq!(engine.send(command), Ok(()));
        }
        assert_eq!(engine.snapshot().state, target, "arranjo");
        engine
    }

    /// §3.7: Seek em Paused volta a Paused, mesmo aberto com `autoplay`.
    ///
    /// Given: aberto com `autoplay: true`, tocando e depois pausado.
    /// When: Seek e o ring da geração nova fica pronto.
    /// Then: o estado é Paused, o de antes do Seek.
    #[test]
    fn seek_while_paused_resumes_paused() {
        let engine = engine_in(true, PlaybackState::Paused);
        assert_eq!(engine.send(Command::Seek), Ok(()));
        engine.notify_ring_ready();
        assert_eq!(engine.snapshot().state, PlaybackState::Paused);
    }

    /// §3.7: Seek em Playing volta a Playing, mesmo aberto sem `autoplay`.
    ///
    /// Given: aberto com `autoplay: false`, pausado e depois tocando por Play.
    /// When: Seek e o ring da geração nova fica pronto.
    /// Then: o estado é Playing, o de antes do Seek.
    #[test]
    fn seek_while_playing_resumes_playing() {
        let engine = engine_in(false, PlaybackState::Playing);
        assert_eq!(engine.send(Command::Seek), Ok(()));
        engine.notify_ring_ready();
        assert_eq!(engine.snapshot().state, PlaybackState::Playing);
    }

    /// §3.3/§3.7: fila de eventos limitada; o overflow descarta o mais
    /// antigo e conta em `dropped_events`. Teste de caracterização.
    ///
    /// Given: nenhum consumidor drenando a fila de 64 eventos.
    /// When: 3 transições do Open e 70 de Pause/Play são aplicadas.
    /// Then: a fila guarda os 64 eventos mais recentes e `dropped_events`
    /// conta os 9 perdidos (oráculo: aritmética das transições do harness).
    #[test]
    fn event_overflow_drops_oldest_and_counts() {
        const CAPACITY: usize = 64;
        const TOGGLES: usize = 70;
        let engine = engine_in(true, PlaybackState::Playing);
        for i in 0..TOGGLES {
            let command = if i % 2 == 0 {
                Command::Pause
            } else {
                Command::Play
            };
            assert_eq!(engine.send(command), Ok(()));
        }
        let emitted = 3 + TOGGLES;

        let events = engine.drain_events();
        assert_eq!(events.len(), CAPACITY, "a fila guarda só a capacidade");
        assert_eq!(
            engine.snapshot().dropped_events,
            (emitted - CAPACITY) as u64,
            "cada evento descartado é contado uma vez"
        );
        // O mais recente é o último toggle (Pause → Play, i ímpar).
        assert_eq!(
            events.last().copied(),
            Some(changed(PlaybackState::Paused, PlaybackState::Playing, 1))
        );
    }

    /// `publish_buffer` descarta, antes do sink, buffer de geração velha.
    ///
    /// Given: Playing na geração 1, e um Seek leva a geração a 2.
    /// When: o produtor publica um buffer da geração 1 e depois um da 2.
    /// Then: o ring do harness recebe só o da geração 2.
    #[test]
    fn publish_buffer_drops_stale_generation() {
        let engine = engine_in(true, PlaybackState::Playing);
        let g1 = engine.snapshot().generation;
        assert_eq!(engine.send(Command::Seek), Ok(()));
        let g2 = engine.snapshot().generation;
        assert_ne!(g1, g2, "arranjo: o Seek trocou a geração");

        let mut ring = SharedRing::default();
        for generation in [g1, g2] {
            engine.publish_buffer(
                &mut ring,
                PcmBuffer {
                    generation,
                    frames: 512,
                },
            );
        }

        let generations: Vec<Generation> = ring
            .0
            .lock()
            // Teste: nenhum outro thread usa este ring.
            .expect("ring do teste envenenado")
            .iter()
            .map(|buffer| buffer.generation)
            .collect();
        assert_eq!(generations, vec![g2], "buffer da geração 1 chegou ao sink");
    }

    /// Stop em `from`, posto à mão (nenhuma API pública leva o engine lá
    /// nesta TASK): devolve `Ok`, chega a Idle e emite a sequência de §3.7.
    fn assert_stop_from(from: PlaybackState) {
        let engine = engine_in(true, PlaybackState::Playing);
        engine
            .inner
            .lock()
            // Teste: nenhum outro thread tocou o lock.
            .expect("lock de estado livre no arranjo")
            .state = from;
        let generation = engine.snapshot().generation.value();
        engine.drain_events();

        assert_eq!(
            engine.send(Command::Stop),
            Ok(()),
            "Stop aceito em {from:?}"
        );
        assert_eq!(engine.snapshot().state, PlaybackState::Idle);
        assert_eq!(
            engine.drain_events(),
            vec![
                changed(from, PlaybackState::Stopping, generation),
                cancelled(generation),
                changed(PlaybackState::Stopping, PlaybackState::Idle, generation + 1),
            ]
        );
    }

    /// §3.2: Stop sai de `Reconnecting` para Idle.
    #[test]
    fn stop_from_reconnecting_goes_idle() {
        assert_stop_from(PlaybackState::Reconnecting);
    }

    /// §3.2: `Error` sai só por Stop → Idle (ENGINE-REQ-14).
    #[test]
    fn stop_from_error_goes_idle() {
        assert_stop_from(PlaybackState::Error(ErrorKind::Decode));
    }

    /// ENGINE-AC-05 / TASK-E01-03-AC-3: repeated stop é idempotente.
    ///
    /// Given: o estado em Idle depois de um Stop aplicado.
    /// When: Stop é enviado três vezes seguidas e o contador de eventos é lido.
    /// Then: o 1º Stop aplicado moveu o contador com a sequência exata de §3.7,
    /// com uma única transição para Stopping; os três seguintes devolvem `Ok`
    /// e não movem o contador, de nenhuma variante. Estado, geração e
    /// `dropped_events` também não mudam. O contador é o `EventLog` do harness.
    #[test]
    fn repeated_stop_is_idempotent() {
        let engine = Engine::new();
        let mut log = EventLog::default();
        assert_eq!(
            engine.send(Command::Open {
                source: SourceRef::new(3),
                autoplay: true,
            }),
            Ok(())
        );
        assert_eq!(engine.send(Command::Stop), Ok(()));
        assert_eq!(
            log.take(&engine),
            vec![
                changed(PlaybackState::Idle, PlaybackState::Opening, 1),
                changed(PlaybackState::Opening, PlaybackState::Stopping, 1),
                cancelled(1),
                changed(PlaybackState::Stopping, PlaybackState::Idle, 2),
            ]
        );
        let stops = log
            .seen
            .iter()
            .filter(|event| {
                matches!(
                    event,
                    Event::StateChanged {
                        to: PlaybackState::Stopping,
                        ..
                    }
                )
            })
            .count();
        assert_eq!(stops, 1, "o 1º Stop aplicado é uma transição para Stopping");
        let counted = log.count();
        let before = engine.snapshot();
        assert_eq!(before.state, PlaybackState::Idle);

        for _ in 0..3 {
            assert_eq!(engine.send(Command::Stop), Ok(()), "Stop em Idle é Ok");
        }

        assert_eq!(log.take(&engine), Vec::new(), "Stop em Idle não emite nada");
        assert_eq!(log.count(), counted, "o contador não se move");
        assert_eq!(engine.snapshot(), before, "estado e geração não mudam");
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
    /// Given: aberto com `autoplay: false`, já em Paused, com N workers
    /// registrados no mesmo `CancelToken` da geração vigente.
    /// When: Seek (Paused→Seeking da tabela) incrementa a geração.
    /// Then: assim que o `send` devolve, e antes de qualquer buffer da
    /// geração nova, os N handles antigos veem `is_cancelled()`, o handle da
    /// geração nova não está cancelado e o portão já recusa a velha.
    #[test]
    fn new_generation_cancels_old_workers() {
        const WORKERS: usize = 4;
        let engine = engine_in(false, PlaybackState::Paused);
        let handles: Vec<CancelHandle> = (0..WORKERS).map(|_| engine.current_cancel()).collect();
        assert!(handles.iter().all(|h| !h.is_cancelled()), "arranjo");

        assert_eq!(engine.send(Command::Seek), Ok(()));

        let cancelled = handles.iter().filter(|h| h.is_cancelled()).count();
        assert_eq!(
            cancelled, WORKERS,
            "os N workers da geração velha veem o Seek"
        );
        let fresh = engine.current_cancel();
        assert!(!fresh.is_cancelled(), "o token da geração nova está vivo");
        assert_eq!(
            fresh.generation.value(),
            handles[0].generation.value() + 1,
            "o Seek avançou a geração de um"
        );
        let gate = engine.presentation_gate();
        assert!(!gate.admits(handles[0].generation));
        assert!(gate.admits(fresh.generation));

        // ENGINE-AC-01: Seek em Buffering não está na tabela de §3.2.
        assert!(matches!(
            engine.send(Command::Seek),
            Err(Error::InvalidTransition {
                from: PlaybackState::Buffering,
                command: CommandKind::Seek,
            })
        ));
    }
}
