//! Testes de integração do pipeline: ring SPSC, dreno de EOF, relógio
//! apresentado e transições entre gerações (TASK-E01-05).
//!
//! A geração vem do `Engine` real (`presentation_gate`), nunca fabricada.
//! Todos os oráculos comparam bits (`to_bits`), sem tolerância.

use vynamp_engine::pipeline::{
    Consumer, Pipeline, PipelineConfig, PipelineControl, Producer, PushOutcome,
};
use vynamp_engine::playback::{Command, Engine, Error, Generation, SourceRef};

/// Abre uma fonte no engine e devolve a geração vigente.
fn open_source(engine: &Engine) -> Generation {
    engine
        .send(Command::Open {
            source: SourceRef::new(1),
            autoplay: true,
        })
        .expect("open");
    engine.notify_source_ready();
    engine.notify_ring_ready();
    engine.snapshot().generation
}

fn build(engine: &Engine, config: PipelineConfig) -> (Producer, Consumer, PipelineControl) {
    let pipeline = Pipeline::new(config, engine.presentation_gate()).expect("config valida");
    let control = pipeline.control();
    let (producer, consumer) = pipeline.split();
    (producer, consumer, control)
}

fn cfg(period: u32, latency: u32, channels: u16, capacity: u32) -> PipelineConfig {
    PipelineConfig {
        period_frames: period,
        latency_frames: latency,
        channels,
        ring_capacity_frames: capacity,
    }
}

/// Roda um callback sobre um buffer sujo e acrescenta o período à captura.
fn run(consumer: &mut Consumer, period_samples: usize, capture: &mut Vec<f32>) -> usize {
    let mut out = vec![f32::NAN; period_samples];
    let real = consumer.callback(&mut out);
    capture.extend_from_slice(&out);
    real
}

fn bits(samples: &[f32]) -> Vec<u32> {
    samples.iter().map(|s| s.to_bits()).collect()
}

/// ENGINE-AC-06 / TASK-E01-05-AC-1: o EOF drena até o último frame.
///
/// Oráculo: a fonte é a rampa `k + 1`; a saída concatenada é a fonte bit a bit
/// e depois só zeros, sem corte nem repetição.
#[test]
fn eof_drains_to_last_frame() {
    const PERIOD: u32 = 1024;
    const LATENCY: u32 = 1024;
    const CONTENT: usize = 3 * PERIOD as usize + 512;
    let engine = Engine::new();
    let generation = open_source(&engine);
    let (mut producer, mut consumer, control) = build(&engine, cfg(PERIOD, LATENCY, 1, 8192));

    let source: Vec<f32> = (0..CONTENT).map(|k| (k + 1) as f32).collect();
    assert_eq!(
        producer.push(generation, &source),
        Ok(PushOutcome::Accepted)
    );
    assert_eq!(producer.push_eof(generation), PushOutcome::Accepted);

    let mut capture = Vec::new();
    let mut previous = 0u64;
    let mut callbacks = 0;
    while control.presented_frames() < CONTENT as u64 && callbacks < 20 {
        run(&mut consumer, PERIOD as usize, &mut capture);
        callbacks += 1;
        let presented = control.presented_frames();
        assert!(presented >= previous, "presented e monotonico");
        assert!(
            presented <= control.content_read_frames(),
            "presented nunca passa do que saiu do ring"
        );
        if callbacks == 1 {
            assert_eq!(presented, 0, "latencia >= periodo: nada apresentado ainda");
        }
        previous = presented;
    }

    // 3584 + 1024 de latencia = 4608 frames de saida = 5 periodos.
    assert_eq!(callbacks, 5);
    assert_eq!(control.presented_frames(), CONTENT as u64);
    assert_eq!(control.content_read_frames(), CONTENT as u64);
    assert_eq!(capture.len(), 5 * PERIOD as usize);
    assert_eq!(bits(&capture[..CONTENT]), bits(&source), "fonte exata");
    assert!(
        capture[CONTENT..].iter().all(|s| s.to_bits() == 0),
        "depois do EOF so silencio exato"
    );
    assert_eq!(control.underruns(), 0, "EOF nao e underrun");
}

/// ENGINE-AC-07 / TASK-E01-05-AC-2: Stop, Open e Play sem áudio velho.
///
/// A usa valores em `[1, 2)`; B em `[-2, -1)`. O oráculo é a saída de B exata
/// seguida de zeros; qualquer bit de A reprova.
#[test]
fn stop_then_play_has_no_stale_audio() {
    const PERIOD: u32 = 512;
    let engine = Engine::new();
    let gen_a = open_source(&engine);
    let (mut producer, mut consumer, control) = build(&engine, cfg(PERIOD, PERIOD, 1, 8192));

    let track_a: Vec<f32> = (0..4096).map(|k| 1.0 + k as f32 / 8192.0).collect();
    assert_eq!(producer.push(gen_a, &track_a), Ok(PushOutcome::Accepted));
    let mut capture = Vec::new();
    run(&mut consumer, PERIOD as usize, &mut capture);
    run(&mut consumer, PERIOD as usize, &mut capture);
    assert_eq!(
        bits(&capture),
        bits(&track_a[..1024]),
        "A toca antes da pausa"
    );
    assert_eq!(control.buffered_frames(), 4096 - 1024);

    // Pausa: sobra A no ring e o callback nao consome nem apresenta.
    engine.send(Command::Pause).expect("pause");
    control.set_paused(true);
    assert!(control.is_paused());
    let presented_before = control.presented_frames();
    let mut paused = Vec::new();
    for _ in 0..3 {
        assert_eq!(run(&mut consumer, PERIOD as usize, &mut paused), 0);
    }
    assert!(paused.iter().all(|s| s.to_bits() == 0), "pausa e silencio");
    assert_eq!(control.buffered_frames(), 3072, "pausa nao consome o ring");
    assert_eq!(control.presented_frames(), presented_before);
    assert_eq!(control.underruns(), 0, "pausa nao e underrun");

    // Decode em voo: ainda admitido enquanto a geracao A vigora.
    assert_eq!(
        producer.push(gen_a, &track_a[..512]),
        Ok(PushOutcome::Accepted)
    );
    assert_eq!(control.buffered_frames(), 3072 + 512);

    // Stop encerra A; buffer velho tardio e recusado e nao entra no ring.
    engine.send(Command::Stop).expect("stop");
    assert_eq!(
        producer.push(gen_a, &track_a[..512]),
        Ok(PushOutcome::Stale)
    );
    assert_eq!(producer.push_eof(gen_a), PushOutcome::Stale);

    // Open de B e Play.
    let gen_b = open_source(&engine);
    assert!(gen_b > gen_a);
    engine.send(Command::Pause).expect("pause B");
    engine.send(Command::Play).expect("play B");
    control.set_paused(false);
    let track_b: Vec<f32> = (0..2048).map(|k| -2.0 + k as f32 / 4096.0).collect();
    assert_eq!(producer.push(gen_b, &track_b), Ok(PushOutcome::Accepted));
    assert_eq!(producer.push_eof(gen_b), PushOutcome::Accepted);

    // O primeiro callback de B descarta o que sobrou de A no ring.
    let mut out = Vec::new();
    for _ in 0..6 {
        run(&mut consumer, PERIOD as usize, &mut out);
    }
    assert_eq!(bits(&out[..2048]), bits(&track_b), "saida e B exata");
    assert!(
        out[2048..].iter().all(|s| s.to_bits() == 0),
        "depois de B so silencio"
    );
    assert!(
        out.iter().all(|s| *s <= 0.0),
        "nenhuma amostra positiva (A) sai"
    );
    assert_eq!(control.underruns(), 0);
}

/// Decode em voo: o buffer da geração velha que já entrou no ring antes do
/// Stop (corrida admitida por `admits`) é descartado pelo consumidor.
///
/// O produtor grava com a geração A quando o gate ainda a admite; o Stop e o
/// Open de B vêm depois; o ring ainda guarda A quando B toca.
#[test]
fn consumer_drops_ring_content_of_a_past_generation() {
    let engine = Engine::new();
    let gen_a = open_source(&engine);
    let (mut producer, mut consumer, control) = build(&engine, cfg(256, 0, 1, 4096));
    let track_a: Vec<f32> = (0..1000).map(|k| 5.0 + k as f32).collect();
    assert_eq!(producer.push(gen_a, &track_a), Ok(PushOutcome::Accepted));
    assert_eq!(control.buffered_frames(), 1000);

    engine.send(Command::Stop).expect("stop");
    let gen_b = open_source(&engine);
    let track_b: Vec<f32> = (0..300).map(|k| -1.0 - k as f32).collect();
    assert_eq!(producer.push(gen_b, &track_b), Ok(PushOutcome::Accepted));
    assert_eq!(producer.push_eof(gen_b), PushOutcome::Accepted);

    let mut out = Vec::new();
    run(&mut consumer, 256, &mut out);
    run(&mut consumer, 256, &mut out);
    assert_eq!(bits(&out[..300]), bits(&track_b), "so B sai");
    assert!(out[300..].iter().all(|s| s.to_bits() == 0));
    assert_eq!(control.content_read_frames(), 300);
    assert_eq!(control.underruns(), 0);
}

/// ENGINE-AC-08 / TASK-E01-05-AC-3: A/B sem deriva e sem ganho.
///
/// Dois "arquivos" idênticos alternam a cada segundo (48 000 frames, que não é
/// múltiplo do período de 1 024: toda troca exercita a cauda parcial). Cada
/// troca é Stop + Open no engine real, ou seja, uma geração nova.
#[test]
fn ab_switch_has_no_drift_and_no_gain() {
    const PERIOD: u32 = 1024;
    const LATENCY: u32 = 1024;
    const SEGMENT: usize = 48_000;
    const SEGMENTS: usize = 60;
    let total = SEGMENT * SEGMENTS;

    let engine = Engine::new();
    let (mut producer, mut consumer, control) = build(&engine, cfg(PERIOD, LATENCY, 1, 8192));
    // O arquivo: frame k vale k + 1 (inteiro exato em f32 até 2^24).
    let file: Vec<f32> = (0..total).map(|k| (k + 1) as f32).collect();
    let max_source = file.iter().fold(0.0_f32, |m, s| m.max(s.abs()));

    let mut presented_sum = 0u64;
    let mut max_out = 0.0_f32;
    let mut previous_generation: Option<Generation> = None;
    for segment in 0..SEGMENTS {
        if previous_generation.is_some() {
            engine.send(Command::Stop).expect("stop");
        }
        let generation = open_source(&engine);
        if let Some(previous) = previous_generation {
            assert!(generation > previous, "cada troca e uma geracao nova");
        }
        previous_generation = Some(generation);

        let source = &file[segment * SEGMENT..(segment + 1) * SEGMENT];
        let mut pushed = 0usize;
        let mut capture = Vec::new();
        let mut callbacks = 0;
        let mut last_presented = 0u64;
        while callbacks == 0 || control.presented_frames() < SEGMENT as u64 {
            // Produtor enche o que couber; EOF junto do ultimo frame.
            let room = (producer.free_frames() as usize).min(SEGMENT - pushed);
            if room > 0 {
                let outcome = producer.push(generation, &source[pushed..pushed + room]);
                assert_eq!(outcome, Ok(PushOutcome::Accepted));
                pushed += room;
                if pushed == SEGMENT {
                    assert_eq!(producer.push_eof(generation), PushOutcome::Accepted);
                }
            }
            run(&mut consumer, PERIOD as usize, &mut capture);
            callbacks += 1;
            let presented = control.presented_frames();
            if callbacks > 1 {
                assert!(presented >= last_presented, "monotonico na geracao");
            }
            last_presented = presented;
            assert!(callbacks < 100, "segmento nao termina");
        }
        assert_eq!(control.presented_frames(), SEGMENT as u64, "sem deriva");
        assert_eq!(
            bits(&capture[..SEGMENT]),
            bits(source),
            "amostra == fonte na posicao absoluta (sem ganho)"
        );
        assert!(capture[SEGMENT..].iter().all(|s| s.to_bits() == 0));
        max_out = capture.iter().fold(max_out, |m, s| m.max(s.abs()));
        presented_sum += control.presented_frames();
    }
    assert_eq!(
        presented_sum, total as u64,
        "soma dos presented == conteudo"
    );
    assert!(max_out <= max_source, "nenhuma amostra excede a fonte");
    assert_eq!(control.underruns(), 0);
}

/// PIPE-REQ-04: underrun gera zeros, incrementa o contador e não bloqueia.
///
/// Estéreo: 600 frames disponíveis para um período de 1 024. A saída é o que
/// havia, intercalado e exato, depois zeros até o fim do buffer.
#[test]
fn underrun_zero_fills_the_rest_and_counts() {
    let engine = Engine::new();
    let generation = open_source(&engine);
    let (mut producer, mut consumer, control) = build(&engine, cfg(1024, 0, 2, 4096));
    let source: Vec<f32> = (0..1200).map(|k| (k + 1) as f32).collect(); // 600 frames
    assert_eq!(
        producer.push(generation, &source),
        Ok(PushOutcome::Accepted)
    );

    let mut out = vec![f32::NAN; 2048];
    let real = consumer.callback(&mut out);
    assert_eq!(real, 600);
    assert_eq!(bits(&out[..1200]), bits(&source));
    assert!(out[1200..].iter().all(|s| s.to_bits() == 0), "resto zerado");
    assert_eq!(control.underruns(), 1);
    assert_eq!(
        control.presented_frames(),
        600,
        "latencia 0: apresentado = lido"
    );

    // Ring vazio, sem EOF: outro underrun, saida toda zero (buffer sujo antes).
    let mut out = vec![f32::NAN; 2048];
    assert_eq!(consumer.callback(&mut out), 0);
    assert!(out.iter().all(|s| s.to_bits() == 0));
    assert_eq!(control.underruns(), 2);
    assert_eq!(control.presented_frames(), 600, "silencio nao e conteudo");
}

/// PIPE-REQ-06: `presented = lido − latency_frames`, monotônico.
#[test]
fn presented_clock_subtracts_latency() {
    let engine = Engine::new();
    let generation = open_source(&engine);
    let (mut producer, mut consumer, control) = build(&engine, cfg(1024, 2048, 1, 16384));
    let source: Vec<f32> = (0..8192).map(|k| (k + 1) as f32).collect();
    assert_eq!(
        producer.push(generation, &source),
        Ok(PushOutcome::Accepted)
    );
    let mut sink = Vec::new();
    let mut seen = Vec::new();
    for _ in 0..5 {
        run(&mut consumer, 1024, &mut sink);
        seen.push(control.presented_frames());
    }
    assert_eq!(seen, vec![0, 0, 1024, 2048, 3072]);
}

/// Capacidade é aplicada: push que não cabe é recusado inteiro; resto não
/// múltiplo de canais é erro; nada é descartado em silêncio.
#[test]
fn push_enforces_capacity_and_frame_alignment() {
    let engine = Engine::new();
    let generation = open_source(&engine);
    let (mut producer, mut consumer, control) = build(&engine, cfg(256, 0, 2, 1024));
    assert_eq!(producer.free_frames(), 1024);

    let odd = vec![0.5_f32; 3];
    assert_eq!(
        producer.push(generation, &odd),
        Err(Error::InvalidArgument("samples nao multiplo de channels"))
    );
    assert_eq!(control.buffered_frames(), 0);

    let too_big = vec![0.5_f32; 2 * 1025];
    assert_eq!(
        producer.push(generation, &too_big),
        Err(Error::InvalidArgument("ring sem espaco para o push"))
    );
    assert_eq!(control.buffered_frames(), 0, "sem escrita parcial");

    let exact: Vec<f32> = (0..2048).map(|k| k as f32).collect();
    assert_eq!(producer.push(generation, &exact), Ok(PushOutcome::Accepted));
    assert_eq!(producer.free_frames(), 0);
    assert_eq!(
        producer.push(generation, &[1.0, 1.0]),
        Err(Error::InvalidArgument("ring sem espaco para o push"))
    );

    // Consumir libera espaco; a volta do ring preserva a ordem exata.
    let mut capture = Vec::new();
    run(&mut consumer, 512, &mut capture);
    assert_eq!(producer.free_frames(), 256);
    let wrap: Vec<f32> = (0..512).map(|k| 10_000.0 + k as f32).collect();
    assert_eq!(producer.push(generation, &wrap), Ok(PushOutcome::Accepted));
    for _ in 0..4 {
        run(&mut consumer, 512, &mut capture);
    }
    let expected: Vec<f32> = exact.iter().chain(wrap.iter()).copied().collect();
    assert_eq!(bits(&capture[..expected.len()]), bits(&expected));
}

/// Configuração inválida é erro, não pânico.
#[test]
fn invalid_config_is_an_error() {
    let engine = Engine::new();
    for config in [cfg(0, 0, 1, 64), cfg(64, 0, 0, 64), cfg(64, 0, 1, 32)] {
        assert!(matches!(
            Pipeline::new(config, engine.presentation_gate()),
            Err(Error::InvalidArgument(_))
        ));
    }
}
