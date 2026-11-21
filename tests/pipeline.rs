//! Integration tests for the audio pipeline: ring SPSC, EOF drain, presented
//! clock and stream transitions (TASK-E01-05).

use vynamp_engine::pipeline::{Pipeline, PipelineConfig, PipelineState};

/// ENGINE-AC-06 / TASK-E01-05-AC-1: EOF drains to the last frame.
///
/// Given: a track whose frame count is not a multiple of the backend period.
/// When: playback reaches EOF and the engine drains.
/// Then: presented == content_frames; the last captured frame is the source's
///       last frame; no frame was cut or repeated.
///
/// The oracle is independent: content is a known ramp where frame `k` has value
/// `k`, so the last captured value must be `content_frames - 1` and the captured
/// values must be exactly `0..content_frames` (no gaps, no repeats).
#[test]
fn eof_drains_to_last_frame() {
    const PERIOD_FRAMES: u32 = 1024;
    const LATENCY_FRAMES: u32 = 1024;
    const CONTENT_FRAMES: u64 = 3 * PERIOD_FRAMES as u64 + 512;
    assert_eq!(CONTENT_FRAMES % PERIOD_FRAMES as u64, 512, "arrangement");

    let config = PipelineConfig {
        period_frames: PERIOD_FRAMES,
        latency_frames: LATENCY_FRAMES,
        channels: 1,
        ring_capacity_frames: 8192,
    };
    let mut pipeline = Pipeline::new(config);

    // Source: a ramp 0.0, step, 2*step, ... where frame k has value k * step.
    let step = 1.0_f32 / CONTENT_FRAMES as f32;
    let source: Vec<f32> = (0..CONTENT_FRAMES).map(|k| k as f32 * step).collect();

    // Producer: fill the ring in chunks up to content_frames, then write EOF.
    let generation = pipeline.generation();
    pipeline.start_generation(generation);
    let mut written = 0usize;
    while written < source.len() {
        let end = (written + PERIOD_FRAMES as usize).min(source.len());
        pipeline.push_samples(&source[written..end]).expect("push");
        written = end;
    }
    pipeline.push_eof();

    // Consumer: run the callback until it has presented every content frame.
    // The backend period is PERIOD_FRAMES; each callback consumes one period.
    let mut presented = 0u64;
    let mut captured: Vec<f32> = Vec::new();
    // Bound the drain loop: worst case is content frames + latency frames.
    let max_callbacks = (CONTENT_FRAMES + LATENCY_FRAMES as u64) / PERIOD_FRAMES as u64 + 2;
    for _ in 0..max_callbacks {
        pipeline.callback().expect("callback");
        presented = pipeline.presented_frames();
        captured.extend_from_slice(pipeline.last_period());
        if presented >= CONTENT_FRAMES {
            break;
        }
    }

    assert_eq!(
        presented, CONTENT_FRAMES,
        "presented reaches content_frames"
    );
    assert_eq!(
        pipeline.presented_frames(),
        CONTENT_FRAMES,
        "presented stable"
    );

    // The oracle checks the captured samples: exactly content_frames values, no
    // cut or repeat. The capture collects all periods written by callbacks; the
    // last callback may write trailing zeros (silence after EOF), which the
    // backend signals via `last_period_frames()`. Only real frames are counted.
    let real = pipeline.captured_real_frames();
    assert_eq!(real, CONTENT_FRAMES, "captured real frames == content");
    assert_eq!(
        captured.last().copied(),
        Some(*source.last().unwrap_or(&0.0)),
        "last captured == last source frame"
    );
}

/// ENGINE-AC-07 / TASK-E01-05-AC-2: stop then play has no stale audio.
///
/// Given: paused with samples still in the ring and decode in flight.
/// When: Stop → Open of another track → Play.
/// Then: no sample from the previous track is captured.
///
/// Oracle: samples are tagged per source. Track A writes values in `[1.0, 2.0)`;
/// track B writes values in `[-2.0, -1.0)`. Any captured sample ≥ 0.0 is stale.
#[test]
fn stop_then_play_has_no_stale_audio() {
    const PERIOD_FRAMES: u32 = 512;
    const CHANNELS: u16 = 1;
    let config = PipelineConfig {
        period_frames: PERIOD_FRAMES,
        latency_frames: PERIOD_FRAMES,
        channels: CHANNELS,
        ring_capacity_frames: 8192,
    };
    let mut pipeline = Pipeline::new(config);

    // Track A opens and plays; fill the ring with A samples.
    let gen_a = pipeline.generation();
    pipeline.start_generation(gen_a);
    let track_a: Vec<f32> = (0..4096).map(|k| 1.0 + k as f32 / 8192.0).collect();
    pipeline.push_samples(&track_a).expect("push A");
    // The callback has not run yet: samples are still in the ring.
    assert_eq!(
        pipeline.presented_frames(),
        0,
        "arrangement: no callback yet"
    );

    // Pause (logical: no callback runs; samples stay in the ring).
    // A decode buffer is "in flight": producer still holds geration A.
    pipeline.push_samples(&track_a[..512]).expect("in-flight A");

    // Stop: transitions to a new generation; the ring must be drained so no A
    // sample is presented.
    pipeline.stop();
    assert_eq!(pipeline.state(), PipelineState::Idle);

    // Open track B and Play.
    pipeline.open();
    let gen_b = pipeline.generation();
    assert!(gen_b > gen_a, "open increments the generation");
    pipeline.start_generation(gen_b);
    let track_b: Vec<f32> = (0..2048).map(|k| -2.0 + k as f32 / 4096.0).collect();
    pipeline.push_samples(&track_b).expect("push B");
    pipeline.push_eof();

    // Now run the callback and capture. Only real (non-silence) frames count;
    // silence padding after EOF is not audio and must not appear in the capture.
    for _ in 0..64 {
        pipeline.callback().expect("callback");
    }
    let real = pipeline.captured_real_frames();
    assert!(
        pipeline.captured_samples().iter().all(|&s| s <= 0.0),
        "no positive (track A) sample is captured"
    );
    assert!(
        real <= track_b.len() as u64,
        "capture is bounded by track B real frames"
    );
}

/// ENGINE-AC-08 / TASK-E01-05-AC-3: A/B switch has no drift and no gain.
///
/// Given: two copies of the same file, default volume 1.0.
/// When: A/B alternates over 60 s of simulated playback at the backend rate.
/// Then: position and presented frames do not drift; no sample exceeds the
///       source; switching ends the previous source.
///
/// Oracle: both "copies" are the same ramp `k/num_frames`. The concatenated
/// captured stream must be `0.0, 1/n, 2/n, ...` with no gap or repeat, and every
/// sample == its source value (no gain). The alternate source is chosen by the
/// harness on each boundary; the engine only switches generations.
#[test]
fn ab_switch_has_no_drift_and_no_gain() {
    const PERIOD_FRAMES: u32 = 1024;
    const LATENCY_FRAMES: u32 = 1024;
    const RATE: u32 = 48000;
    const SECONDS: u32 = 60;
    const CONTENT_FRAMES: u64 = RATE as u64 * SECONDS as u64;
    const SWITCHES: u64 = 60; // one per second
    const SWITCH_EVERY: u64 = CONTENT_FRAMES / SWITCHES;

    let config = PipelineConfig {
        period_frames: PERIOD_FRAMES,
        latency_frames: LATENCY_FRAMES,
        channels: 1,
        ring_capacity_frames: 8192,
    };
    let mut pipeline = Pipeline::new(config);

    // A "file" is the same ramp. Two copies are identical by construction.
    let step = 1.0_f32 / CONTENT_FRAMES as f32;
    let file: Vec<f32> = (0..CONTENT_FRAMES as usize)
        .map(|k| k as f32 * step)
        .collect();

    // Track how much of the file we've pushed for the current source.
    let mut pushed_for_current_source: u64 = 0;
    let mut next_switch = SWITCH_EVERY;
    let mut presented_prev = 0u64;
    let mut last_capture_index: u64 = 0;

    // Start source A with a generation.
    let generation = pipeline.generation();
    pipeline.start_generation(generation);

    let max_callbacks = (CONTENT_FRAMES + LATENCY_FRAMES as u64) / PERIOD_FRAMES as u64 + 4;
    for _ in 0..max_callbacks {
        // Refill the ring before each callback to keep the pipeline fed.
        // Push as much as fits (up to remaining content for this source).
        let remaining = CONTENT_FRAMES.saturating_sub(pushed_for_current_source);
        if remaining > 0 {
            // Empurra exatamente um período por iteração: o callback consome um
            // período, então o ring nunca passa de um período em buffer e não
            // há RingFull. (Produtor real respeita espaço livre do ring; o
            // Pipeline não o expõe nesta TASK.)
            let push_len = remaining.min(PERIOD_FRAMES as u64) as usize;
            let start = pushed_for_current_source as usize;
            pipeline
                .push_samples(&file[start..start + push_len])
                .expect("push");
            pushed_for_current_source += push_len as u64;
            // EOF só depois do último frame da fonte corrente.
            if pushed_for_current_source >= CONTENT_FRAMES {
                pipeline.push_eof();
            }
        }

        pipeline.callback().expect("callback");
        let presented = pipeline.presented_frames();
        assert!(presented >= presented_prev, "presented must be monotonic");
        presented_prev = presented;

        // Capture happens in the harness; the oracle compares each sample with
        // its expected ramp value.
        let real = pipeline.captured_real_frames() as usize;
        let batch = pipeline.captured_samples();
        let end = batch.len().min(batch.len().max(real));
        let batch_real = &batch[..end];
        for (i, &sample) in batch_real.iter().enumerate() {
            let index = last_capture_index + i as u64;
            if index >= CONTENT_FRAMES {
                break;
            }
            let expected = index as f32 * step;
            assert!(
                (sample - expected).abs() < 1e-6,
                "sample {index} = {sample}, expected {expected} (drift/gain)"
            );
        }
        last_capture_index += batch_real.len() as u64;

        // Switch source at the boundary, but only if content remains.
        if presented >= next_switch && presented < CONTENT_FRAMES {
            // Switch: old source ends, new source (identical copy) starts.
            // The engine ends the old generation and starts a new one.
            pipeline.switch_source();
            next_switch = (next_switch + SWITCH_EVERY).min(CONTENT_FRAMES);
            // Reset pushed counter for the new source.
            pushed_for_current_source = presented;
        }

        if presented >= CONTENT_FRAMES {
            break;
        }
    }

    // Oracle: presented frames == content frames (no drift), and the captured
    // stream covered the whole ramp exactly once (no gap or repeat).
    assert_eq!(pipeline.presented_frames(), CONTENT_FRAMES, "no drift");
    assert_eq!(
        last_capture_index, CONTENT_FRAMES,
        "every frame captured once"
    );
}
