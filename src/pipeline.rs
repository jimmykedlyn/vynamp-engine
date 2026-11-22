//! Pipeline de áudio (TASK-E01-05): ring SPSC entre decode e callback, dreno de
//! EOF, relógio de frames apresentados e descarte por geração.
//!
//! # Desenho
//!
//! [`Pipeline::new`] aloca **uma vez** o ring (`Box<[AtomicU32]>` com os bits de
//! cada `f32`; sem `unsafe`, `#![forbid(unsafe_code)]` vale) e os contadores
//! atômicos, depois o divide em [`Producer`] (worker de decode/DSP) e
//! [`Consumer`] (callback do output). [`PipelineControl`] é o handle clonável de
//! pausa e leitura de contadores.
//!
//! Contadores `u64` monotônicos publicados com `Release` e lidos com `Acquire`
//! (`specs/pipeline.md` §3.2); o índice no ring é `posição % capacidade`.
//!
//! # Geração
//!
//! A geração pertence ao `Engine`: o pipeline não a cria nem a incrementa. O
//! consumidor lê a vigente pelo [`PresentationGate`]; o produtor etiqueta cada
//! push com a geração do buffer. Na primeira gravação de uma geração nova o
//! produtor publica `gen_start` (posição do ring onde ela começa) e depois a
//! etiqueta. O consumidor só lê quando a etiqueta é a geração vigente e, ao
//! armar, pula tudo antes de `gen_start`: o que sobrou da geração velha nunca
//! sai (`specs/pipeline.md` §6). Um produtor atrasado que grava com etiqueta
//! velha depois do Stop não é lido, porque a etiqueta não bate com a vigente.
//!
//! # Limites conhecidos
//!
//! - Uma geração velha que ainda ocupa o ring libera o espaço só quando o
//!   consumidor arma a geração nova; até lá `push` pode devolver erro de falta
//!   de espaço. O produtor tenta de novo depois do próximo callback.
//! - Mixer e ramp de play/stop (citados na "Entrega" da TASK) não existem aqui:
//!   nenhum critério de aceite os cobre e um mixer sem teste seria stub.
//! - A corrida com threads reais (produtor passa em `admits`, a geração muda e
//!   ele grava depois) é coberta pela etiqueta, mas os testes são de uma thread
//!   só; a prova com threads fica para E01-10.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};

use crate::playback::{Error, Generation, PresentationGate};

/// Sem marca de EOF gravada.
const NO_EOF: u64 = u64::MAX;
/// Sem etiqueta de geração gravada.
const NO_TAG: u64 = u64::MAX;

/// Configuração do pipeline.
#[derive(Clone, Copy, Debug)]
pub struct PipelineConfig {
    /// Período do backend em frames; deve ser maior que zero.
    pub period_frames: u32,
    /// Latência reportada do backend em frames, descontada do relógio.
    pub latency_frames: u32,
    /// Canais do output (intercalados); deve ser maior que zero.
    pub channels: u16,
    /// Capacidade do ring em frames; no mínimo um período.
    pub ring_capacity_frames: u32,
}

impl PipelineConfig {
    fn validate(&self) -> Result<(), Error> {
        if self.period_frames == 0 {
            return Err(Error::InvalidArgument("period_frames"));
        }
        if self.channels == 0 {
            return Err(Error::InvalidArgument("channels"));
        }
        if self.ring_capacity_frames < self.period_frames {
            return Err(Error::InvalidArgument("ring_capacity_frames"));
        }
        Ok(())
    }
}

/// Destino de um push aceito pela API.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PushOutcome {
    /// Amostras gravadas no ring.
    Accepted,
    /// A geração do buffer já não é a vigente: descartado sem gravar.
    Stale,
}

/// Estado compartilhado entre produtor, consumidor e controle.
struct Shared {
    config: PipelineConfig,
    gate: PresentationGate,
    /// Bits `f32` das amostras intercaladas: `capacity_frames * channels`.
    data: Box<[AtomicU32]>,
    /// Frames gravados desde o início (só o produtor escreve).
    written: AtomicU64,
    /// Frames lidos ou descartados desde o início (só o consumidor escreve).
    read: AtomicU64,
    /// Posição do ring onde começa a geração etiquetada em `tag`.
    gen_start: AtomicU64,
    /// Geração do conteúdo que o produtor está gravando.
    tag: AtomicU64,
    /// Posição onde a geração etiquetada termina; `NO_EOF` se ainda aberta.
    eof_pos: AtomicU64,
    paused: AtomicBool,
    /// Frames de conteúdo lidos na geração vigente do consumidor.
    content_read: AtomicU64,
    presented: AtomicU64,
    underruns: AtomicU64,
}

impl Shared {
    fn capacity(&self) -> u64 {
        u64::from(self.config.ring_capacity_frames)
    }

    fn channels(&self) -> usize {
        usize::from(self.config.channels)
    }

    fn buffered(&self) -> u64 {
        let written = self.written.load(Ordering::Acquire);
        let read = self.read.load(Ordering::Acquire);
        written.saturating_sub(read)
    }
}

/// Fábrica do ring: aloca tudo aqui e entrega as três pontas.
pub struct Pipeline {
    shared: Arc<Shared>,
}

impl Pipeline {
    /// Aloca o ring e os contadores. `gate` vem de
    /// [`Engine::presentation_gate`](crate::playback::Engine::presentation_gate).
    ///
    /// Configuração inválida devolve `Error::InvalidArgument` com o campo.
    pub fn new(config: PipelineConfig, gate: PresentationGate) -> Result<Self, Error> {
        config.validate()?;
        let samples = config.ring_capacity_frames as usize * usize::from(config.channels);
        let data: Box<[AtomicU32]> = (0..samples).map(|_| AtomicU32::new(0)).collect();
        Ok(Self {
            shared: Arc::new(Shared {
                config,
                gate,
                data,
                written: AtomicU64::new(0),
                read: AtomicU64::new(0),
                gen_start: AtomicU64::new(0),
                tag: AtomicU64::new(NO_TAG),
                eof_pos: AtomicU64::new(NO_EOF),
                paused: AtomicBool::new(false),
                content_read: AtomicU64::new(0),
                presented: AtomicU64::new(0),
                underruns: AtomicU64::new(0),
            }),
        })
    }

    /// Handle de controle e leitura; clonável, serve a qualquer thread.
    pub fn control(&self) -> PipelineControl {
        PipelineControl {
            shared: Arc::clone(&self.shared),
        }
    }

    /// Divide em produtor e consumidor (SPSC: um de cada).
    pub fn split(self) -> (Producer, Consumer) {
        (
            Producer {
                shared: Arc::clone(&self.shared),
                eof_sent: false,
            },
            Consumer {
                shared: self.shared,
                generation: NO_TAG,
                armed: false,
                output_frames: 0,
                content_read: 0,
                presented: 0,
            },
        )
    }
}

/// Lado produtor: o worker de decode/DSP.
pub struct Producer {
    shared: Arc<Shared>,
    eof_sent: bool,
}

impl Producer {
    /// Frames que cabem agora no ring.
    pub fn free_frames(&self) -> u32 {
        let shared = &self.shared;
        let free = shared.capacity().saturating_sub(shared.buffered());
        // `free <= capacity`, que é `u32`.
        u32::try_from(free).unwrap_or(u32::MAX)
    }

    /// Grava amostras intercaladas da geração `generation`.
    ///
    /// - geração que não é a vigente: `Ok(Stale)`, nada é gravado;
    /// - tamanho que não é múltiplo de `channels`: erro, nada é gravado;
    /// - mais frames do que cabe: erro, **sem escrita parcial**;
    /// - gravar depois do EOF da mesma geração: erro.
    pub fn push(&mut self, generation: Generation, samples: &[f32]) -> Result<PushOutcome, Error> {
        let shared = &self.shared;
        let channels = shared.channels();
        if !samples.len().is_multiple_of(channels) {
            return Err(Error::InvalidArgument("samples nao multiplo de channels"));
        }
        if !shared.gate.admits(generation) {
            return Ok(PushOutcome::Stale);
        }
        let tag = generation.value();
        let same = shared.tag.load(Ordering::Relaxed) == tag;
        if same && self.eof_sent {
            return Err(Error::InvalidArgument("push apos EOF"));
        }
        let frames = (samples.len() / channels) as u64;
        if frames > shared.capacity().saturating_sub(shared.buffered()) {
            return Err(Error::InvalidArgument("ring sem espaco para o push"));
        }
        let written = shared.written.load(Ordering::Relaxed);
        if !same {
            self.begin_generation(tag, written);
        }
        let shared = &self.shared;
        let capacity = shared.capacity();
        for (i, sample) in samples.iter().enumerate() {
            let frame = written + (i / channels) as u64;
            let slot = (frame % capacity) as usize * channels + i % channels;
            shared.data[slot].store(sample.to_bits(), Ordering::Relaxed);
        }
        shared.written.store(written + frames, Ordering::Release);
        Ok(PushOutcome::Accepted)
    }

    /// Grava a marca de EOF da geração, **depois** do último frame.
    ///
    /// Idempotente. Geração que não é a vigente devolve `Stale`.
    pub fn push_eof(&mut self, generation: Generation) -> PushOutcome {
        if !self.shared.gate.admits(generation) {
            return PushOutcome::Stale;
        }
        let tag = generation.value();
        let written = self.shared.written.load(Ordering::Relaxed);
        if self.shared.tag.load(Ordering::Relaxed) != tag {
            // Faixa vazia: a geração começa e termina na mesma posição.
            self.begin_generation(tag, written);
        }
        self.shared.eof_pos.store(written, Ordering::Release);
        self.eof_sent = true;
        PushOutcome::Accepted
    }

    /// Publica o início de uma geração: `eof_pos`, `gen_start` e só então a
    /// etiqueta, para o consumidor que vê a etiqueta ver o resto.
    fn begin_generation(&mut self, tag: u64, written: u64) {
        let shared = &self.shared;
        shared.eof_pos.store(NO_EOF, Ordering::Relaxed);
        shared.gen_start.store(written, Ordering::Relaxed);
        shared.tag.store(tag, Ordering::Release);
        self.eof_sent = false;
    }
}

/// Lado consumidor: a callback do output.
pub struct Consumer {
    shared: Arc<Shared>,
    /// Geração vigente vista no último callback.
    generation: u64,
    /// `true` depois de pular o que veio antes de `gen_start`.
    armed: bool,
    /// Frames entregues ao device nesta geração, silêncio incluído.
    output_frames: u64,
    content_read: u64,
    presented: u64,
}

impl Consumer {
    /// Preenche `out` (amostras intercaladas) e devolve os frames de conteúdo
    /// copiados. O resto do buffer, inclusive qualquer sobra que não complete
    /// um frame, é zerado.
    ///
    /// Tempo real: sem alocação, lock, I/O nem log. Só loads e stores atômicos
    /// e escrita em `out`.
    pub fn callback(&mut self, out: &mut [f32]) -> usize {
        // RT-BEGIN
        let shared = &*self.shared;
        let channels = shared.channels();
        let out_frames = out.len() / channels;
        let current = shared.gate.current().value();
        if current != self.generation {
            // Geração nova: base do relógio zerada, ring ainda por armar.
            self.generation = current;
            self.armed = false;
            self.output_frames = 0;
            self.content_read = 0;
            self.presented = 0;
            shared.content_read.store(0, Ordering::Relaxed);
            shared.presented.store(0, Ordering::Release);
        }
        out.fill(0.0);
        if shared.paused.load(Ordering::Acquire) {
            // Pausa: silêncio, nada é lido e o relógio não avança.
            return 0;
        }
        if !self.armed && shared.tag.load(Ordering::Acquire) == current {
            let start = shared.gen_start.load(Ordering::Relaxed);
            let read = shared.read.load(Ordering::Relaxed);
            if start > read {
                // O que sobrou da geração velha é descartado, não tocado.
                shared.read.store(start, Ordering::Release);
            }
            self.armed = true;
        }
        let mut copied = 0usize;
        let mut drained = false;
        if self.armed {
            // EOF antes do disponível: não confunde "acabou" com "atrasou".
            let eof = shared.eof_pos.load(Ordering::Acquire);
            let written = shared.written.load(Ordering::Acquire);
            let read = shared.read.load(Ordering::Relaxed);
            let available = written.saturating_sub(read);
            copied = available.min(out_frames as u64) as usize;
            let capacity = shared.capacity();
            for frame in 0..copied {
                let base = ((read + frame as u64) % capacity) as usize * channels;
                for channel in 0..channels {
                    out[frame * channels + channel] =
                        f32::from_bits(shared.data[base + channel].load(Ordering::Relaxed));
                }
            }
            shared.read.store(read + copied as u64, Ordering::Release);
            drained = eof != NO_EOF && read + copied as u64 >= eof;
        }
        if copied < out_frames && !drained {
            shared.underruns.fetch_add(1, Ordering::Relaxed);
        }
        self.output_frames += out_frames as u64;
        self.content_read += copied as u64;
        let delayed = self
            .output_frames
            .saturating_sub(u64::from(shared.config.latency_frames));
        self.presented = self.content_read.min(self.presented.max(delayed));
        shared
            .content_read
            .store(self.content_read, Ordering::Relaxed);
        shared.presented.store(self.presented, Ordering::Release);
        // RT-END
        copied
    }
}

/// Handle clonável: pausa e leitura de contadores.
#[derive(Clone)]
pub struct PipelineControl {
    shared: Arc<Shared>,
}

impl PipelineControl {
    /// Pausa ou retoma o callback. Pausado, ele entrega silêncio, não consome o
    /// ring, não avança o relógio e não conta underrun.
    pub fn set_paused(&self, paused: bool) {
        self.shared.paused.store(paused, Ordering::Release);
    }

    /// Verdadeiro se o callback está pausado.
    pub fn is_paused(&self) -> bool {
        self.shared.paused.load(Ordering::Acquire)
    }

    /// Frames apresentados na geração vigente: `min(lido, saído − latência)`,
    /// monotônico e nunca acima do conteúdo que saiu do ring.
    pub fn presented_frames(&self) -> u64 {
        self.shared.presented.load(Ordering::Acquire)
    }

    /// Frames de conteúdo lidos do ring na geração vigente (sem silêncio).
    pub fn content_read_frames(&self) -> u64 {
        self.shared.content_read.load(Ordering::Relaxed)
    }

    /// Callbacks que não completaram o período com conteúdo nem com EOF.
    pub fn underruns(&self) -> u64 {
        self.shared.underruns.load(Ordering::Relaxed)
    }

    /// Frames no ring ainda não lidos nem descartados.
    pub fn buffered_frames(&self) -> u64 {
        self.shared.buffered()
    }
}
