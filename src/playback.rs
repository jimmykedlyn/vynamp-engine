//! State machine de reprodução, comandos, eventos e cancelamento por geração.

use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

/// Geração monotônica da reprodução.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Generation(pub u64);

/// Token de cancelamento compartilhado pelos workers de uma geração.
#[derive(Clone, Debug)]
pub struct CancelToken {
    cancelled: Arc<AtomicBool>,
}

impl CancelToken {
    pub fn new() -> Self {
        Self {
            cancelled: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }

    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
    }
}

impl Default for CancelToken {
    fn default() -> Self {
        Self::new()
    }
}

/// Estados de reprodução.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlaybackState {
    Idle,
    Opening,
    Buffering,
    Playing,
    Paused,
    Seeking,
    Stopping,
}

/// Comandos de reprodução.
#[derive(Clone, Debug, PartialEq)]
pub enum Command {
    Open,
    Play,
    Pause,
    Stop,
    Seek,
}

/// Kind de comando (para contagem de eventos de domínio).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommandKind {
    Open,
    Play,
    Pause,
    Stop,
    Seek,
}

/// Erros de domínio.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    InvalidTransition,
    Cancelled,
}

/// Eventos de domínio.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event {
    StateChanged {
        from: PlaybackState,
        to: PlaybackState,
        generation: Generation,
    },
    Cancelled {
        generation: Generation,
    },
}

/// Buffer publicado no ring, marcado pela geração que o produziu.
#[derive(Clone, Debug)]
pub struct Buffer {
    pub generation: Generation,
    pub frames: Vec<f32>,
}

/// Motor de reprodução (domínio). Thread-safe o suficiente para testes em processo.
pub struct Engine {
    state: PlaybackState,
    generation: Generation,
    cancel: CancelToken,
    ring: VecDeque<Buffer>,
    event_count: AtomicU64,
    applied_stops: AtomicU64,
    workers_alive: AtomicU64,
}

impl Engine {
    pub fn new() -> Self {
        Self {
            state: PlaybackState::Idle,
            generation: Generation(0),
            cancel: CancelToken::new(),
            ring: VecDeque::new(),
            event_count: AtomicU64::new(0),
            applied_stops: AtomicU64::new(0),
            workers_alive: AtomicU64::new(0),
        }
    }

    pub fn state(&self) -> PlaybackState {
        self.state
    }

    pub fn generation(&self) -> Generation {
        self.generation
    }

    pub fn cancel_token(&self) -> CancelToken {
        self.cancel.clone()
    }

    pub fn event_count(&self) -> u64 {
        self.event_count.load(Ordering::Acquire)
    }

    pub fn applied_stops(&self) -> u64 {
        self.applied_stops.load(Ordering::Acquire)
    }

    pub fn workers_alive(&self) -> u64 {
        self.workers_alive.load(Ordering::Acquire)
    }

    pub fn register_worker(&self) {
        self.workers_alive.fetch_add(1, Ordering::AcqRel);
    }

    pub fn worker_done(&self) {
        self.workers_alive.fetch_sub(1, Ordering::AcqRel);
    }

    pub fn publish_buffer(&mut self, buf: Buffer) {
        // descarta buffers de geração velha na publicação
        if buf.generation < self.generation {
            return;
        }
        self.ring.push_back(buf);
    }

    pub fn ring_len(&self) -> usize {
        self.ring.len()
    }

    /// Drena o ring, contando frames apenas da geração atual.
    pub fn drain_current_generation(&mut self) -> usize {
        let mut frames = 0usize;
        let mut i = 0;
        while i < self.ring.len() {
            if self.ring[i].generation == self.generation {
                frames += self.ring[i].frames.len();
                self.ring.remove(i);
            } else {
                i += 1;
            }
        }
        frames
    }

    pub fn send(&mut self, cmd: Command) -> Result<(), Error> {
        match cmd {
            Command::Open => {
                // cancela geração anterior
                self.cancel.cancel();
                self.cancel = CancelToken::new();
                self.generation = Generation(self.generation.0 + 1);
                let from = self.state;
                self.state = PlaybackState::Opening;
                self.event_count.fetch_add(1, Ordering::AcqRel);
                let _ = (from,);
                Ok(())
            }
            Command::Play => {
                if self.state == PlaybackState::Idle || self.state == PlaybackState::Paused {
                    let from = self.state;
                    self.state = PlaybackState::Playing;
                    self.event_count.fetch_add(1, Ordering::AcqRel);
                    let _ = from;
                    Ok(())
                } else {
                    Err(Error::InvalidTransition)
                }
            }
            Command::Pause => {
                if self.state == PlaybackState::Playing {
                    self.state = PlaybackState::Paused;
                    self.event_count.fetch_add(1, Ordering::AcqRel);
                    Ok(())
                } else {
                    Err(Error::InvalidTransition)
                }
            }
            Command::Stop => {
                if self.state == PlaybackState::Idle {
                    // idempotente: não muda estado nem emite evento
                    return Ok(());
                }
                self.cancel.cancel();
                self.cancel = CancelToken::new();
                self.generation = Generation(self.generation.0 + 1);
                let from = self.state;
                self.state = PlaybackState::Idle;
                self.applied_stops.fetch_add(1, Ordering::AcqRel);
                self.event_count.fetch_add(1, Ordering::AcqRel);
                let _ = from;
                // limpa ring
                self.ring.clear();
                Ok(())
            }
            Command::Seek => {
                self.cancel.cancel();
                self.cancel = CancelToken::new();
                self.generation = Generation(self.generation.0 + 1);
                let from = self.state;
                self.state = PlaybackState::Seeking;
                self.event_count.fetch_add(1, Ordering::AcqRel);
                let _ = from;
                // volta a Playing após seek (domínio simplificado)
                self.state = PlaybackState::Playing;
                Ok(())
            }
        }
    }
}

impl Default for Engine {
    fn default() -> Self {
        Self::new()
    }
}
