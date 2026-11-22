//! PIPE-REQ-03: o callback do pipeline não aloca (TASK-E01-05).
//!
//! Receita de `quality/02-performance.md`: contador de alocação **por thread**
//! (um contador global contaria as seções de outros testes do binário) mais
//! guarda estática sobre a região `RT-BEGIN`/`RT-END` do fonte.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

use vynamp_engine::pipeline::{Pipeline, PipelineConfig};
use vynamp_engine::playback::{Command, Engine, SourceRef};

thread_local! {
    // `const` init: acessar não aloca nem registra destrutor.
    static ALLOCS: Cell<u64> = const { Cell::new(0) };
}

struct Counting;

// SAFETY: delega integralmente ao `System`, que cumpre o contrato de
// `GlobalAlloc`; só soma um contador da thread antes de delegar, sem tocar no
// layout nem no ponteiro. `try_with` evita pânico durante o encerramento do TLS.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let _ = ALLOCS.try_with(|c| c.set(c.get() + 1));
        // SAFETY: mesmo `layout` recebido do chamador, repassado sem alteração.
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: `ptr`/`layout` vieram de `alloc` acima, que delegou ao System.
        unsafe { System.dealloc(ptr, layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let _ = ALLOCS.try_with(|c| c.set(c.get() + 1));
        // SAFETY: argumentos repassados sem alteração; `ptr` veio deste alocador.
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static GLOBAL: Counting = Counting;

fn allocs() -> u64 {
    ALLOCS.with(Cell::get)
}

/// Callback com ring cheio, underrun, pausa, EOF e troca de geração: zero
/// alocações nesta thread.
#[test]
fn callback_allocates_nothing() {
    let engine = Engine::new();
    engine
        .send(Command::Open {
            source: SourceRef::new(1),
            autoplay: true,
        })
        .expect("open");
    let generation = engine.snapshot().generation;
    let config = PipelineConfig {
        period_frames: 256,
        latency_frames: 256,
        channels: 2,
        ring_capacity_frames: 2048,
    };
    let pipeline = Pipeline::new(config, engine.presentation_gate()).expect("config");
    let control = pipeline.control();
    let (mut producer, mut consumer) = pipeline.split();
    let source: Vec<f32> = (0..2 * 1000).map(|k| k as f32).collect();
    producer.push(generation, &source).expect("push");
    producer.push_eof(generation);
    let mut out = vec![0.0_f32; 2 * 256];

    // Mede só as chamadas do callback: `Engine::send` publica evento (aloca) e
    // não é caminho de tempo real.
    let mut inside = 0u64;
    let mut measure = |consumer: &mut vynamp_engine::pipeline::Consumer| {
        let start = allocs();
        consumer.callback(&mut out);
        inside += allocs() - start;
    };
    for _ in 0..4 {
        measure(&mut consumer); // conteudo, cauda parcial, EOF
    }
    control.set_paused(true);
    measure(&mut consumer); // pausa
    control.set_paused(false);
    engine.send(Command::Stop).expect("stop");
    measure(&mut consumer); // geracao nova, ring sem etiqueta: underrun
    let (before, after) = (0, inside);

    assert_eq!(after - before, 0, "o callback alocou");
    // O contador funciona: alocar aqui o move.
    let probe = allocs();
    let v = vec![1u8; 64];
    assert!(
        allocs() > probe,
        "contador de alocacao nao enxerga alocacoes"
    );
    drop(v);
}

/// Guarda estática: a região RT do fonte não contém alocação, lock, I/O nem log.
#[test]
fn rt_region_has_no_forbidden_tokens() {
    let source = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/pipeline.rs"))
        .expect("le src/pipeline.rs");
    let begin = source.find("// RT-BEGIN").expect("marca RT-BEGIN");
    let end = source.find("// RT-END").expect("marca RT-END");
    assert!(begin < end);
    let region: String = source[begin..end]
        .lines()
        .map(|l| l.split("//").next().unwrap_or(""))
        .collect::<Vec<_>>()
        .join("\n");
    for token in [
        "Vec", "vec!", "Box", "String", "format!", "to_vec", "clone", "collect", "push(", "extend",
        "resize", "Mutex", "RwLock", "lock(", "println", "eprintln", "log::", "File", "Condvar",
        "sleep", "unwrap", "expect", "panic",
    ] {
        assert!(
            !region.contains(token),
            "token proibido na regiao RT: {token}"
        );
    }
}
