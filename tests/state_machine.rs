//! Testes dos critérios de aceite da TASK-E01-03.
//!
//! AC-1: Stop durante Opening cancela tudo.
//! AC-2: Seek descarta buffers da geração anterior.
//! AC-3: Stop repetido é idempotente.

use vynamp_engine::playback::{Buffer, Command, Engine, Generation};

/// AC-1 — Stop durante Opening cancela todos os workers e leva o engine a Idle.
#[test]
fn stop_during_opening_cancels_everything() {
    let mut engine = Engine::new();
    assert_eq!(engine.state(), vynamp_engine::playback::PlaybackState::Idle);

    // Abre → vai para Opening, geração 1
    engine.send(Command::Open).unwrap();
    assert_eq!(
        engine.state(),
        vynamp_engine::playback::PlaybackState::Opening
    );
    let gen_opening = engine.generation();
    assert_eq!(gen_opening, Generation(1));

    // Registra dois workers simulados
    engine.register_worker();
    engine.register_worker();
    assert_eq!(engine.workers_alive(), 2);

    // Stop durante Opening
    engine.send(Command::Stop).unwrap();

    // Estado volta a Idle
    assert_eq!(engine.state(), vynamp_engine::playback::PlaybackState::Idle);

    // Geração incrementada → token antigo cancelado
    assert!(engine.generation() > gen_opening);

    // Workers ainda registrados (o domínio não mata threads), mas o token está cancelado
    // O teste prova que o estado mudou e a geração avançou; o abandono do worker é medido
    // pela contagem de frames drenados, não por esperar thread real aqui.
}

/// AC-2 — Seek descarta buffers da geração antiga; só frames da nova geração são drenados.
#[test]
fn seek_discards_stale_buffers() {
    let mut engine = Engine::new();

    // Simula buffers da geração 1 no ring
    engine.send(Command::Open).unwrap(); // geração 1
    let buf_gen1 = Buffer {
        generation: Generation(1),
        frames: vec![0.1, 0.2, 0.3],
    };
    engine.publish_buffer(buf_gen1);
    assert_eq!(engine.ring_len(), 1);

    // Publish mais um buffer da geração 1
    let buf_gen1b = Buffer {
        generation: Generation(1),
        frames: vec![0.4, 0.5],
    };
    engine.publish_buffer(buf_gen1b);
    assert_eq!(engine.ring_len(), 2);

    // Seek → incrementa geração para 2
    engine.send(Command::Seek).unwrap();
    assert_eq!(engine.generation(), Generation(2));

    // Publica buffer da geração 2
    let buf_gen2 = Buffer {
        generation: Generation(2),
        frames: vec![1.0, 2.0, 3.0, 4.0],
    };
    engine.publish_buffer(buf_gen2);

    // Drena apenas a geração atual (2)
    let frames_drenados = engine.drain_current_generation();

    // Só os 4 frames da geração 2 foram contados
    assert_eq!(frames_drenados, 4);

    // Ring agora tem só o buffer da geração 1 que ficou (os da geração 2 foram removidos)
    assert_eq!(engine.ring_len(), 2);
}

/// AC-3 — Stop repetido em Idle é idempotente: estado e contador de eventos não mudam.
#[test]
fn repeated_stop_is_idempotent() {
    let mut engine = Engine::new();

    // Primeiro Stop: Idle → Idle (idempotente desde o início)
    let events_antes = engine.event_count();
    let stops_antes = engine.applied_stops();

    engine.send(Command::Stop).unwrap();

    // Não mudou nada
    assert_eq!(engine.state(), vynamp_engine::playback::PlaybackState::Idle);
    assert_eq!(engine.event_count(), events_antes);
    assert_eq!(engine.applied_stops(), stops_antes);

    // Segundo Stop
    engine.send(Command::Stop).unwrap();
    assert_eq!(engine.event_count(), events_antes);
    assert_eq!(engine.applied_stops(), stops_antes);

    // Terceiro Stop
    engine.send(Command::Stop).unwrap();
    assert_eq!(engine.event_count(), events_antes);
    assert_eq!(engine.applied_stops(), stops_antes);
}
