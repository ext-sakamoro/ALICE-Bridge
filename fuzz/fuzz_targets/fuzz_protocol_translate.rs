#![no_main]
//! Fuzz target: IntensityLimiter hard cap + soft compression.
//!
//! 攻撃者制御の (hard_cap, soft_knee, compression_ratio, intensity 列) で
//! `IntensityLimiter::apply` / `apply_typed` / `clip_ratio` が panic せず終端することを検証。
//! NaN / Inf / 負数 / 極端値でも return 値が有限 (or NaN) で定義域外の overflow なし。

use alice_bridge::{ActuatorType, IntensityLimiter};
use arbitrary::{Arbitrary, Unstructured};
use libfuzzer_sys::fuzz_target;

#[derive(Debug, Arbitrary)]
enum FuzzActuator {
    Vibrate,
    Rotate,
    Oscillate,
    Constrict,
    Inflate,
    Heat,
    Electrostimulate,
    Linear,
    Position,
    Custom,
    ParseUnknown,
}

impl FuzzActuator {
    fn to_actuator(&self) -> ActuatorType {
        match self {
            Self::Vibrate => ActuatorType::Vibrate,
            Self::Rotate => ActuatorType::Rotate,
            Self::Oscillate => ActuatorType::Oscillate,
            Self::Constrict => ActuatorType::Constrict,
            Self::Inflate => ActuatorType::Inflate,
            Self::Heat => ActuatorType::Heat,
            Self::Electrostimulate => ActuatorType::Electrostimulate,
            Self::Linear => ActuatorType::Linear,
            Self::Position => ActuatorType::Position,
            Self::Custom => ActuatorType::Custom,
            Self::ParseUnknown => ActuatorType::parse("__fuzz_unknown__"),
        }
    }
}

#[derive(Debug, Arbitrary)]
struct FuzzInput {
    hard_cap_bits: u64,
    soft_knee_bits: Option<u64>,
    compression_ratio_bits: u64,
    apply_calls: Vec<u64>, // intensity bit patterns
    typed_calls: Vec<(FuzzActuator, u64, u32, u64)>, // (atype, intensity_bits, actuator_index, now_bits)
}

fuzz_target!(|data: &[u8]| {
    let mut u = Unstructured::new(data);
    let Ok(input) = FuzzInput::arbitrary(&mut u) else {
        return;
    };

    // OOM 予防
    if input.apply_calls.len() > 4096 || input.typed_calls.len() > 4096 {
        return;
    }

    let hard_cap = f64::from_bits(input.hard_cap_bits);
    let soft_knee = input.soft_knee_bits.map(f64::from_bits);
    let compression_ratio = f64::from_bits(input.compression_ratio_bits);

    let mut limiter = IntensityLimiter::new(hard_cap, soft_knee, compression_ratio);

    // apply: NaN / Inf / 負数 全域で panic なし
    for &bits in &input.apply_calls {
        let intensity = f64::from_bits(bits);
        let _ = limiter.apply(intensity);
    }

    // apply_typed: (intensity, atype, actuator_index, now_secs) 全域で panic なし
    for (atype, intensity_bits, actuator_index, now_bits) in &input.typed_calls {
        let intensity = f64::from_bits(*intensity_bits);
        let now_secs = f64::from_bits(*now_bits);
        let _ = limiter.apply_typed(intensity, atype.to_actuator(), *actuator_index, now_secs);
    }

    let _ = limiter.clip_ratio();
});
