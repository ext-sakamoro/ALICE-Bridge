//! Intensity limiter — hard cap, soft compression, and per-actuator-type safety.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::device::ActuatorType;

/// Safety limits for a specific actuator type.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SafetyLimits {
    /// Maximum allowed intensity [0.0, 1.0].
    pub max_intensity: f64,
    /// Maximum intensity change per second.
    pub ramp_rate: f64,
    /// Minimum time between intensity increases (ms).
    pub cooldown_ms: u64,
    /// Auto-disable after this duration (0 = disabled).
    pub auto_shutoff_ms: u64,
}

impl Default for SafetyLimits {
    fn default() -> Self {
        Self {
            max_intensity: 1.0,
            ramp_rate: 10.0,
            cooldown_ms: 0,
            auto_shutoff_ms: 0,
        }
    }
}

/// Default safety limits per actuator type.
pub fn default_limits() -> HashMap<ActuatorType, SafetyLimits> {
    let mut m = HashMap::new();
    m.insert(
        ActuatorType::Vibrate,
        SafetyLimits {
            max_intensity: 1.0,
            ramp_rate: 10.0,
            ..Default::default()
        },
    );
    m.insert(
        ActuatorType::Rotate,
        SafetyLimits {
            max_intensity: 1.0,
            ramp_rate: 5.0,
            ..Default::default()
        },
    );
    m.insert(
        ActuatorType::Constrict,
        SafetyLimits {
            max_intensity: 0.8,
            ramp_rate: 1.0,
            cooldown_ms: 500,
            ..Default::default()
        },
    );
    m.insert(
        ActuatorType::Inflate,
        SafetyLimits {
            max_intensity: 0.8,
            ramp_rate: 1.0,
            cooldown_ms: 500,
            ..Default::default()
        },
    );
    m.insert(
        ActuatorType::Heat,
        SafetyLimits {
            max_intensity: 0.7,
            ramp_rate: 0.5,
            cooldown_ms: 1000,
            auto_shutoff_ms: 300_000, // 5 minutes
        },
    );
    m.insert(
        ActuatorType::Electrostimulate,
        SafetyLimits {
            max_intensity: 0.5,
            ramp_rate: 0.3,
            cooldown_ms: 200,
            auto_shutoff_ms: 60_000, // 1 minute
        },
    );
    m
}

/// Configurable intensity limiter with hard cap and soft compression.
#[derive(Debug)]
pub struct IntensityLimiter {
    /// Absolute maximum output [0.0, 1.0].
    hard_cap: f64,
    /// Threshold where compression starts. None = no soft limiting.
    soft_knee: Option<f64>,
    /// How much signal above knee passes through (0.0 = brick wall, 1.0 = no compression).
    compression_ratio: f64,
    /// Per-actuator-type safety limits.
    type_limits: HashMap<ActuatorType, SafetyLimits>,
    /// Per-actuator state: (`current_intensity`, `last_increase_time`, `start_time`, active).
    states: HashMap<(ActuatorType, u32), ActuatorState>,
    clip_count: u64,
    total_count: u64,
}

#[derive(Debug, Default)]
struct ActuatorState {
    current: f64,
    last_increase: f64, // monotonic seconds
    start_time: f64,
    active: bool,
}

impl IntensityLimiter {
    #[must_use]
    pub fn new(hard_cap: f64, soft_knee: Option<f64>, compression_ratio: f64) -> Self {
        let hard_cap = hard_cap.clamp(0.0, 1.0);
        let soft_knee = soft_knee.map(|k| if k >= hard_cap { hard_cap * 0.8 } else { k });

        Self {
            hard_cap,
            soft_knee,
            compression_ratio: compression_ratio.clamp(0.0, 1.0),
            type_limits: default_limits(),
            states: HashMap::new(),
            clip_count: 0,
            total_count: 0,
        }
    }

    /// Set custom safety limits for an actuator type.
    pub fn set_limits(&mut self, atype: ActuatorType, limits: SafetyLimits) {
        self.type_limits.insert(atype, limits);
    }

    /// Apply intensity limiting (hard cap + soft compression only).
    pub fn apply(&mut self, intensity: f64) -> f64 {
        self.total_count += 1;

        if intensity <= 0.0 {
            return 0.0;
        }

        let mut v = intensity;

        // Soft compression
        if let Some(knee) = self.soft_knee {
            if v > knee {
                let excess = v - knee;
                v = excess.mul_add(self.compression_ratio, knee);
            }
        }

        // Hard cap
        if v >= self.hard_cap {
            self.clip_count += 1;
            return self.hard_cap;
        }

        v
    }

    /// Apply type-aware safety limiting with ramp rate, cooldown, and auto-shutoff.
    #[allow(clippy::cast_precision_loss)]
    pub fn apply_typed(
        &mut self,
        intensity: f64,
        atype: ActuatorType,
        actuator_index: u32,
        now_secs: f64,
    ) -> f64 {
        let basic = self.apply(intensity);
        let limits = self.type_limits.get(&atype).cloned().unwrap_or_default();

        // Clamp to type max
        let mut target = basic.min(limits.max_intensity);

        let key = (atype, actuator_index);
        let state = self.states.entry(key).or_default();

        // Auto-shutoff
        if limits.auto_shutoff_ms > 0 && state.active {
            let elapsed_ms = (now_secs - state.start_time) * 1000.0;
            if elapsed_ms >= limits.auto_shutoff_ms as f64 {
                target = 0.0;
                state.active = false;
            }
        }

        // Ramp rate limiting
        if state.active && target > state.current {
            let dt = if state.last_increase > 0.0 {
                now_secs - state.last_increase
            } else {
                0.0
            };
            let max_increase = if dt > 0.0 { limits.ramp_rate * dt } else { 0.0 };

            // Cooldown check
            if limits.cooldown_ms > 0 {
                let time_since_ms = (now_secs - state.last_increase) * 1000.0;
                if time_since_ms < limits.cooldown_ms as f64 {
                    target = state.current;
                }
            }

            // Ramp rate
            let delta = target - state.current;
            if max_increase > 0.0 && delta > max_increase {
                target = state.current + max_increase;
            }
        }

        // Update state
        if target > state.current {
            state.last_increase = now_secs;
        }
        state.current = target;
        if target > 0.0 && !state.active {
            state.active = true;
            state.start_time = now_secs;
        } else if target == 0.0 {
            state.active = false;
        }

        target
    }

    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn clip_ratio(&self) -> f64 {
        if self.total_count == 0 {
            return 0.0;
        }
        self.clip_count as f64 / self.total_count as f64
    }

    pub const fn reset_stats(&mut self) {
        self.clip_count = 0;
        self.total_count = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hard_cap() {
        let mut lim = IntensityLimiter::new(0.8, None, 0.3);
        assert!((lim.apply(0.5) - 0.5).abs() < 1e-6);
        assert!((lim.apply(0.9) - 0.8).abs() < 1e-6);
        assert!((lim.apply(1.0) - 0.8).abs() < 1e-6);
    }

    #[test]
    fn soft_compression() {
        let mut lim = IntensityLimiter::new(1.0, Some(0.6), 0.3);
        // Below knee: passthrough
        assert!((lim.apply(0.5) - 0.5).abs() < 1e-6);
        // Above knee: compressed
        // 0.8 -> 0.6 + (0.8 - 0.6) * 0.3 = 0.6 + 0.06 = 0.66
        assert!((lim.apply(0.8) - 0.66).abs() < 1e-6);
    }

    #[test]
    fn negative_clamp() {
        let mut lim = IntensityLimiter::new(1.0, None, 0.3);
        assert!((lim.apply(-0.5) - 0.0).abs() < 1e-6);
    }

    #[test]
    fn type_max_limits() {
        let mut lim = IntensityLimiter::new(1.0, None, 0.3);
        // Heat max = 0.7
        let v = lim.apply_typed(1.0, ActuatorType::Heat, 0, 0.0);
        assert!(v <= 0.7 + 1e-6);
        // E-stim max = 0.5
        let v = lim.apply_typed(1.0, ActuatorType::Electrostimulate, 0, 0.0);
        assert!(v <= 0.5 + 1e-6);
    }

    #[test]
    fn clip_ratio() {
        let mut lim = IntensityLimiter::new(0.5, None, 0.3);
        lim.apply(0.3);
        lim.apply(0.6);
        lim.apply(0.7);
        // 2 out of 3 clipped
        assert!((lim.clip_ratio() - 2.0 / 3.0).abs() < 1e-6);
    }

    #[test]
    fn zero_intensity_passthrough() {
        let mut lim = IntensityLimiter::new(1.0, Some(0.5), 0.3);
        assert!((lim.apply(0.0) - 0.0).abs() < 1e-6);
    }

    #[test]
    fn soft_knee_auto_adjusted_when_above_cap() {
        // knee=0.9, cap=0.8 → knee auto-adjusted to 0.8*0.8=0.64
        let mut lim = IntensityLimiter::new(0.8, Some(0.9), 0.5);
        // 0.7 is above adjusted knee (0.64), so compression applies
        let v = lim.apply(0.7);
        // 0.64 + (0.7 - 0.64) * 0.5 = 0.64 + 0.03 = 0.67
        assert!((v - 0.67).abs() < 1e-6);
    }

    #[test]
    fn hard_cap_clamp_above_one() {
        let mut lim = IntensityLimiter::new(1.5, None, 0.3);
        // hard_cap clamped to 1.0
        assert!((lim.apply(1.0) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn apply_typed_vibrate_no_reduction() {
        let mut lim = IntensityLimiter::new(1.0, None, 0.3);
        let v = lim.apply_typed(0.8, ActuatorType::Vibrate, 0, 0.0);
        assert!((v - 0.8).abs() < 1e-6);
    }

    #[test]
    fn apply_typed_constrict_max() {
        let mut lim = IntensityLimiter::new(1.0, None, 0.3);
        let v = lim.apply_typed(1.0, ActuatorType::Constrict, 0, 0.0);
        assert!(v <= 0.8 + 1e-6);
    }

    #[test]
    fn apply_typed_inflate_max() {
        let mut lim = IntensityLimiter::new(1.0, None, 0.3);
        let v = lim.apply_typed(1.0, ActuatorType::Inflate, 0, 0.0);
        assert!(v <= 0.8 + 1e-6);
    }

    #[test]
    fn apply_typed_rotate_max() {
        let mut lim = IntensityLimiter::new(1.0, None, 0.3);
        let v = lim.apply_typed(1.0, ActuatorType::Rotate, 0, 0.0);
        assert!(v <= 1.0 + 1e-6);
    }

    #[test]
    fn reset_stats_clears() {
        let mut lim = IntensityLimiter::new(0.5, None, 0.3);
        lim.apply(0.6);
        lim.apply(0.7);
        assert!(lim.clip_ratio() > 0.0);
        lim.reset_stats();
        assert!((lim.clip_ratio() - 0.0).abs() < 1e-6);
    }

    #[test]
    fn clip_ratio_no_calls() {
        let lim = IntensityLimiter::new(0.5, None, 0.3);
        assert!((lim.clip_ratio() - 0.0).abs() < 1e-6);
    }

    #[test]
    fn set_custom_limits() {
        let mut lim = IntensityLimiter::new(1.0, None, 0.3);
        lim.set_limits(
            ActuatorType::Vibrate,
            SafetyLimits {
                max_intensity: 0.3,
                ..Default::default()
            },
        );
        let v = lim.apply_typed(1.0, ActuatorType::Vibrate, 0, 0.0);
        assert!(v <= 0.3 + 1e-6);
    }

    #[test]
    fn multiple_actuator_indices_independent() {
        let mut lim = IntensityLimiter::new(1.0, None, 0.3);
        let v0 = lim.apply_typed(0.5, ActuatorType::Heat, 0, 0.0);
        let v1 = lim.apply_typed(0.3, ActuatorType::Heat, 1, 0.0);
        // Both should get their own initial values (within type max)
        assert!(v0 > 0.0);
        assert!(v1 > 0.0);
        assert!((v0 - v1).abs() > 1e-6);
    }

    #[test]
    fn estim_auto_shutoff() {
        let mut lim = IntensityLimiter::new(1.0, None, 0.3);
        // Default E-Stim auto_shutoff = 60000ms = 60s
        let v = lim.apply_typed(0.3, ActuatorType::Electrostimulate, 0, 0.0);
        assert!(v > 0.0);
        // After 61 seconds → auto-shutoff
        let v = lim.apply_typed(0.3, ActuatorType::Electrostimulate, 0, 61.0);
        assert!((v - 0.0).abs() < 1e-6);
    }

    #[test]
    fn below_knee_no_compression() {
        let mut lim = IntensityLimiter::new(1.0, Some(0.8), 0.3);
        assert!((lim.apply(0.5) - 0.5).abs() < 1e-6);
        assert!((lim.apply(0.79) - 0.79).abs() < 1e-6);
    }
}
