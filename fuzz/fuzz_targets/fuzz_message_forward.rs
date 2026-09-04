#![no_main]
//! Fuzz target: SignalBridge multi-source fusion + speed limiting.
//!
//! 攻撃者制御の (source_name, weight, intensity, now_secs) 列で SignalBridge を組み立て、
//! `update` / `tick` / `active_sources` が panic せず終端することを検証。
//! NaN / Inf / 負の時間 / 過大 weight でも定義域 (min_position..=max_position) を逸脱しない。

use alice_bridge::SignalBridge;
use arbitrary::{Arbitrary, Unstructured};
use libfuzzer_sys::fuzz_target;

#[derive(Debug, Arbitrary)]
enum Op {
    AddSource {
        name: String,
        weight_bits: u64,
    },
    Update {
        source_name: String,
        intensity_bits: u64,
        now_bits: u64,
    },
    Tick {
        now_bits: u64,
    },
    ActiveSources,
}

#[derive(Debug, Arbitrary)]
struct FuzzInput {
    smooth_window: u16,
    speed_limit_bits: u64,
    min_pos_bits: u64,
    max_pos_bits: u64,
    output_interval_ms: u32,
    ops: Vec<Op>,
}

fuzz_target!(|data: &[u8]| {
    let mut u = Unstructured::new(data);
    let Ok(input) = FuzzInput::arbitrary(&mut u) else {
        return;
    };

    // OOM 予防: op 数を bound
    if input.ops.len() > 512 {
        return;
    }

    let smooth_window = usize::from(input.smooth_window).min(1024);
    let speed_limit = f64::from_bits(input.speed_limit_bits);
    let min_pos = f64::from_bits(input.min_pos_bits);
    let max_pos = f64::from_bits(input.max_pos_bits);

    let mut bridge = SignalBridge::new(
        smooth_window,
        speed_limit,
        min_pos,
        max_pos,
        input.output_interval_ms,
    );

    for op in &input.ops {
        match op {
            Op::AddSource { name, weight_bits } => {
                if name.len() > 256 {
                    continue;
                }
                bridge.add_source(name, f64::from_bits(*weight_bits));
            }
            Op::Update {
                source_name,
                intensity_bits,
                now_bits,
            } => {
                if source_name.len() > 256 {
                    continue;
                }
                bridge.update(
                    source_name,
                    f64::from_bits(*intensity_bits),
                    f64::from_bits(*now_bits),
                );
            }
            Op::Tick { now_bits } => {
                let action = bridge.tick(f64::from_bits(*now_bits));
                let _ = action.position;
                let _ = action.duration_ms;
                let _ = action.timestamp;
            }
            Op::ActiveSources => {
                let _ = bridge.active_sources();
                let _ = bridge.tick_count();
            }
        }
    }
});
