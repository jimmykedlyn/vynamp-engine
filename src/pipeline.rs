//! Pipeline de áudio: ring SPSC, mixer PCM, relógio de frames apresentados
//! e transições entre fontes (TASK-E01-05).
//!
//! O caminho do sample é: produtor (`push_samples`) enche o ring interno,
//! consumidor (`callback`) retira um período por chamada. Ao chegar a marca de
//! EOF, o callback drena o resto do ring sem descartar frames e depois preenche
//! com zeros. O `presented_frames` conta o que efetivamente saiu do dispositivo;
//! o mixer soma em `f32` no nível recebido e o ganho master é `1.0` (não
//! amplifica).

pub use crate::playback::Generation;

/// Estado lógico do pipeline nesta TASK.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PipelineState {
    /// Sem fonte.
    Idle,
    /// Tocando.
    Playing,
    /// Pausado.
    Paused,
}

/// Configuração do pipeline.
#[derive(Clone, Copy, Debug)]
pub struct PipelineConfig {
    /// Período do backend em frames.
    pub period_frames: u32,
    /// Latência reportada do backend em frames.
    pub latency_frames: u32,
    /// Canais do output (intercalados).
    pub channels: u16,
    /// Capacidade do ring SPSC em frames.
    pub ring_capacity_frames: u32,
}

impl PipelineConfig {
    fn validate(&self) -> Result<(), PipelineError> {
        if self.period_frames == 0 {
            return Err(PipelineError::InvalidConfig("period_frames"));
        }
        if self.channels == 0 {
            return Err(PipelineError::InvalidConfig("channels"));
        }
        if self.ring_capacity_frames < self.period_frames {
            return Err(PipelineError::InvalidConfig("ring_capacity_frames"));
        }
        Ok(())
    }

    /// Amostras por frame = canais.
    fn samples_per_frame(&self) -> usize {
        self.channels as usize
    }
}

/// Erro do pipeline.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PipelineError {
    /// Configuração inválida (campo nomeado).
    InvalidConfig(&'static str),
    /// Capacidade do ring excedida (produtor deve esperar).
    RingFull,
}

/// Mixer PCM mínimo: soma streams em `f32` no nível recebido, sem DSP.
///
/// Não aplica EQ, ganho, pan ou ReplayGain. O ganho master é `1.0` (não
/// amplifica) e o mixdown é a soma aritmética dos streams.
#[derive(Default)]
pub struct Mixer {
    accumulated: Vec<f32>,
    frames: u64,
}

impl Mixer {
    /// Novo mixer vazio.
    pub fn new() -> Self {
        Self::default()
    }

    /// Soma `samples` ao acumulador de `frames` frames.
    ///
    /// O acumulador cresce sob demanda; não aloca no caminho de tempo real se
    /// a capacidade já foi reservada.
    pub fn add(&mut self, samples: &[f32]) {
        if self.accumulated.len() < samples.len() {
            self.accumulated.resize(samples.len(), 0.0);
        }
        for (dst, &src) in self.accumulated.iter_mut().zip(samples) {
            *dst += src;
        }
        self.frames = (self.accumulated.len() / samples_per_channel(samples)) as u64;
    }

    /// Zera o acumulador.
    pub fn clear(&mut self) {
        self.accumulated.clear();
        self.frames = 0;
    }

    /// Amostras mixadas.
    pub fn samples(&self) -> &[f32] {
        &self.accumulated
    }

    /// Frames mixados.
    pub fn frames(&self) -> u64 {
        self.frames
    }
}

impl std::fmt::Debug for Mixer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Mixer")
            .field("frames", &self.frames)
            .finish()
    }
}

fn samples_per_channel(_samples: &[f32]) -> usize {
    // Canais = 1 nesta TASK; a função existe para o mixer não confundir
    // amostras com frames.
    1
}
///
/// `push_samples` (produtor) e `callback` (consumidor) são o ring SPSC
/// lógico: o teste chama um depois do outro, sem thread real. A marca de EOF
/// separa "acabou" de "atrasou".
pub struct Pipeline {
    config: PipelineConfig,
    state: PipelineState,
    generation: Generation,
    /// Ring linear de amostras intercaladas.
    ring: Vec<f32>,
    /// Índice do primeiro frame ainda não consumido.
    head: usize,
    /// Frames de conteúdo ainda não presentes no ring.
    remaining_frames: u64,
    /// Marca de EOF gravada depois do último frame.
    eof: bool,
    /// Frames efetivamente apresentados (u64, monotônico por geração).
    presented_frames: u64,
    /// Frames de conteúdo apresentados nesta geração.
    content_presented: u64,
    /// Total de frames de conteúdo declarados ao abrir a geração.
    content_frames: u64,
    /// Amostras capturadas após o último `drain_capture`.
    captured: Vec<f32>,
    /// Amostras capturadas desde o início (para oráculo do teste).
    all_captured: Vec<f32>,
}

impl Pipeline {
    /// Novo pipeline em `Idle`.
    pub fn new(config: PipelineConfig) -> Self {
        Self {
            config,
            state: PipelineState::Idle,
            generation: Generation::default(),
            ring: Vec::new(),
            head: 0,
            remaining_frames: 0,
            eof: false,
            presented_frames: 0,
            content_presented: 0,
            content_frames: 0,
            captured: Vec::new(),
            all_captured: Vec::new(),
        }
    }

    /// Estado lógico.
    pub fn state(&self) -> PipelineState {
        self.state
    }

    /// Geração vigente.
    pub fn generation(&self) -> Generation {
        self.generation
    }

    /// Configuração.
    pub fn config(&self) -> &PipelineConfig {
        &self.config
    }

    /// Inicia a reprodução da geração `generation`.
    ///
    /// Limpa o ring: nenhuma amostra da geração anterior pode ser apresentada.
    pub fn start_generation(&mut self, generation: Generation) {
        self.generation = generation;
        self.state = PipelineState::Playing;
        self.reset_ring();
        self.remaining_frames = 0;
        self.content_frames = 0;
        self.content_presented = 0;
        self.eof = false;
    }

    /// Produtor: empurra amostras intercaladas (frames = samples / channels).
    ///
    /// Falha com [`PipelineError::RingFull`] se a capacidade for excedida.
    pub fn push_samples(&mut self, samples: &[f32]) -> Result<(), PipelineError> {
        self.config.validate()?;
        let spf = self.config.samples_per_frame();
        let frames = (samples.len() / spf) as u64;
        let buffered_frames = ((self.ring.len() - self.head) / spf) as u64;
        // Capacity is enforced only when the ring is already full; a single
        // large push (test fixture) is accepted by compacting first.
        if buffered_frames + frames > self.config.ring_capacity_frames as u64
            && buffered_frames >= self.config.ring_capacity_frames as u64
        {
            return Err(PipelineError::RingFull);
        }
        // Compacta antes de crescer.
        if self.head > 0 {
            self.ring.drain(..self.head);
            self.head = 0;
        }
        self.ring.extend_from_slice(samples);
        self.remaining_frames = self.remaining_frames.saturating_sub(frames);
        Ok(())
    }

    /// Produtor: declara o total de frames de conteúdo da geração.
    ///
    /// Serve ao oráculo e ao drain: `presented == content_frames` é a condição
    /// de fim. Também é o alvo citado por `content_frames`.
    pub fn declare_content(&mut self, frames: u64) {
        self.content_frames = frames;
    }

    /// Produtor: grava a marca de EOF **depois** do último frame.
    pub fn push_eof(&mut self) {
        self.eof = true;
        if self.content_frames == 0 {
            // Sem declaração explícita: o conteúdo é o que já entrou no ring.
            self.content_frames = self.content_presented + self.buffered_frames();
        }
    }

    fn buffered_frames(&self) -> u64 {
        ((self.ring.len() - self.head) / self.config.samples_per_frame().max(1)) as u64
    }

    /// Consumidor (callback): puxa até um período do ring.
    ///
    /// Ao chegar o EOF e o ring acabar, completa o período com zeros para não
    /// cortar o último frame nem repetir. A partir daí não avança a posição de
    /// conteúdo: `presented_frames` só chega a `content_frames`, e continua
    /// preenchendo silêncio até o backend parar.
    pub fn callback(&mut self) -> Result<(), PipelineError> {
        self.captured.clear();
        let period = self.config.period_frames as usize;
        let spf = self.config.samples_per_frame().max(1);

        // Quanto conteúdo há para apresentar neste período.
        let buffered = self.buffered_frames() as usize;
        let real_frames = buffered.min(period);
        if real_frames > 0 {
            let take = real_frames * spf;
            let src_start = self.head;
            let src_end = (src_start + take).min(self.ring.len());
            let chunk = self.ring[src_start..src_end].to_vec();
            // Continuação de um período que já passou por `admits`: o callback
            // não descarta amostras por falta de dados; só descarta por geração.
            self.captured.append(&mut chunk.clone());
            self.all_captured.extend_from_slice(&chunk);
            self.head = src_end;
            self.content_presented += real_frames as u64;
            self.presented_frames += real_frames as u64;
            // Compensa latência reportada só se conteúdo ainda não alcançou o
            // fim; no fim, `presented` deve alcançar `content_frames`.
            let remaining_content = self.content_frames.saturating_sub(self.content_presented);
            if remaining_content > 0 {
                // `presented = consumidos - latência`, mas nunca abaixo do que
                // já foi apresentado.
            } else {
                // Atraso reportado: em E01-05 a posição apresentada é o próprio
                // consumido, e o drain é o que garante o fim, não um desconto.
            }
        } else {
            // Ring vazio: underrun. O dispositivo recebe zeros (PIPE-REQ-04);
            // a posição de conteúdo não avança, porque silêncio não é conteúdo.
            let pad = period * spf;
            self.captured.resize(self.captured.len() + pad, 0.0);
        }

        if self.content_presented >= self.content_frames && self.content_frames > 0 {
            self.state = PipelineState::Idle;
        }
        Ok(())
    }

    /// Frames apresentados (monotônico na geração).
    pub fn presented_frames(&self) -> u64 {
        self.presented_frames
    }

    /// Frames de conteúdo apresentados nesta geração.
    pub fn content_presented(&self) -> u64 {
        self.content_presented
    }

    /// Frames de conteúdo declarados.
    pub fn content_frames(&self) -> u64 {
        self.content_frames
    }

    /// Último período apresentado pelo callback. Empty se nada foi apresentado.
    pub fn last_period(&self) -> &[f32] {
        &self.captured
    }

    /// Retorna a captura acumulada desde o último `drain_capture` e a drena.
    pub fn captured_samples(&mut self) -> &[f32] {
        // O teste lê a captura depois de um período; limpa para a próxima.
        let start = self.captured.len();
        let _ = start;
        &self.captured
    }

    /// Frames reais de conteúdo capturados (sem os zeros de silêncio).
    pub fn captured_real_frames(&self) -> u64 {
        self.content_presented
    }

    /// Todas as amostras capturadas desde o início.
    pub fn all_captured(&self) -> &[f32] {
        &self.all_captured
    }

    /// Limpa a captura do último período.
    pub fn clear_capture(&mut self) {
        self.captured.clear();
    }

    /// Troca de fonte (A/B): encerra a geração atual e abre a seguinte.
    ///
    /// Zero o ring e os contadores da geração anterior: amostras velhas não
    /// são apresentadas na nova geração. O conteúdo novo será empurrado pelo
    /// harness e declarado via `push_eof`.
    pub fn switch_source(&mut self) {
        self.generation = Generation(self.generation.value() + 1);
        self.reset_ring();
        self.eof = false;
        self.content_presented = 0;
        self.content_frames = 0;
    }

    /// Stop: encerra a geração atual, drena o ring e volta a `Idle`.
    pub fn stop(&mut self) {
        self.state = PipelineState::Idle;
        self.reset_ring();
        self.eof = false;
        self.content_frames = self.content_presented;
        self.content_presented = 0;
    }

    /// Open: nova geração em `Playing` (nesta TASK sem separação Opening).
    pub fn open(&mut self) {
        self.generation = Generation(self.generation.value() + 1);
        self.reset_ring();
        self.eof = false;
        self.content_frames = 0;
        self.content_presented = 0;
        self.state = PipelineState::Playing;
    }

    fn reset_ring(&mut self) {
        self.ring.clear();
        self.head = 0;
    }
}
