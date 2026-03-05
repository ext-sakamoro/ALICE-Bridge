//! Real-time signal processing bridge.
//!
//! Takes raw intensity values from any input source, applies temporal
//! smoothing and speed limiting, then produces device-ready commands.

use std::collections::VecDeque;

/// A single device command ready for output.
#[derive(Debug, Clone, Copy)]
pub struct BridgeAction {
    /// Target position [0.0, 1.0].
    pub position: f64,
    /// Time to reach position (ms).
    pub duration_ms: u32,
    /// Timestamp when this action was generated (monotonic seconds).
    pub timestamp: f64,
}

/// Named input source with its own smoothing buffer.
struct InputSource {
    name: String,
    buffer: VecDeque<f64>,
    weight: f64,
    last_update: f64,
    active: bool,
}

/// Real-time signal processor between input sources and device output.
///
/// Supports any number of named input sources with configurable weights.
/// Applies temporal smoothing and speed limiting.
pub struct SignalBridge {
    sources: Vec<InputSource>,
    smooth_window: usize,
    speed_limit: f64,
    min_position: f64,
    max_position: f64,
    output_interval_ms: u32,
    last_position: f64,
    last_tick: f64,
    tick_count: u64,
    /// Staleness threshold in seconds.
    stale_threshold: f64,
}

impl SignalBridge {
    #[must_use]
    pub fn new(
        smooth_window: usize,
        speed_limit: f64,
        min_position: f64,
        max_position: f64,
        output_interval_ms: u32,
    ) -> Self {
        Self {
            sources: Vec::new(),
            smooth_window: smooth_window.max(1),
            speed_limit,
            min_position,
            max_position,
            output_interval_ms,
            last_position: f64::midpoint(min_position, max_position),
            last_tick: 0.0,
            tick_count: 0,
            stale_threshold: 0.5,
        }
    }

    /// Register a named input source with a weight.
    pub fn add_source(&mut self, name: &str, weight: f64) {
        self.sources.push(InputSource {
            name: name.to_string(),
            buffer: VecDeque::with_capacity(self.smooth_window),
            weight: weight.max(0.0),
            last_update: 0.0,
            active: false,
        });
    }

    /// Update a named input source with a new value.
    pub fn update(&mut self, source_name: &str, intensity: f64, now_secs: f64) {
        for src in &mut self.sources {
            if src.name == source_name {
                let clamped = intensity.clamp(0.0, 1.0);
                if src.buffer.len() >= self.smooth_window {
                    src.buffer.pop_front();
                }
                src.buffer.push_back(clamped);
                src.active = true;
                src.last_update = now_secs;
                return;
            }
        }
    }

    /// Generate the next device action.
    pub fn tick(&mut self, now_secs: f64) -> BridgeAction {
        let dt = if self.last_tick > 0.0 {
            now_secs - self.last_tick
        } else {
            0.0
        };
        self.last_tick = now_secs;

        // Mark stale sources
        for src in &mut self.sources {
            if now_secs - src.last_update > self.stale_threshold {
                src.active = false;
            }
        }

        // Fuse active sources
        let raw = self.compute_fused();

        // Map to position range
        let range = self.max_position - self.min_position;
        let mut target = raw.mul_add(range, self.min_position);

        // Speed limiting
        if dt > 0.0 {
            let max_delta = self.speed_limit * dt;
            let delta = target - self.last_position;
            if delta.abs() > max_delta {
                let sign = if delta > 0.0 { 1.0 } else { -1.0 };
                target = self.last_position + sign * max_delta;
            }
        }

        target = target.clamp(self.min_position, self.max_position);
        self.last_position = target;
        self.tick_count += 1;

        BridgeAction {
            position: target,
            duration_ms: self.output_interval_ms,
            timestamp: now_secs,
        }
    }

    fn compute_fused(&self) -> f64 {
        let mut weighted_sum = 0.0;
        let mut total_weight = 0.0;

        for src in &self.sources {
            if !src.active || src.buffer.is_empty() {
                continue;
            }
            let smoothed = weighted_average(&src.buffer);
            weighted_sum += src.weight * smoothed;
            total_weight += src.weight;
        }

        if total_weight < 1e-10 {
            return 0.0;
        }
        weighted_sum / total_weight
    }

    #[must_use]
    pub const fn tick_count(&self) -> u64 {
        self.tick_count
    }

    #[must_use]
    pub fn active_sources(&self) -> Vec<&str> {
        self.sources
            .iter()
            .filter(|s| s.active)
            .map(|s| s.name.as_str())
            .collect()
    }

    #[must_use]
    pub const fn last_position(&self) -> f64 {
        self.last_position
    }
}

/// Weighted average favoring recent values.
#[allow(clippy::cast_precision_loss)]
fn weighted_average(buf: &VecDeque<f64>) -> f64 {
    if buf.is_empty() {
        return 0.0;
    }
    let mut wsum = 0.0;
    let mut wtotal = 0.0;
    for (i, &v) in buf.iter().enumerate() {
        let w = (i + 1) as f64;
        wsum += w * v;
        wtotal += w;
    }
    wsum / wtotal
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_source() {
        let mut bridge = SignalBridge::new(5, 500.0, 0.0, 1.0, 50);
        bridge.add_source("osc", 1.0);

        bridge.update("osc", 0.8, 0.0);
        let action = bridge.tick(0.0);
        assert!(action.position > 0.0);
    }

    #[test]
    fn weighted_fusion() {
        let mut bridge = SignalBridge::new(1, 500.0, 0.0, 1.0, 50);
        bridge.add_source("a", 0.6);
        bridge.add_source("b", 0.4);

        bridge.update("a", 1.0, 0.0);
        bridge.update("b", 0.0, 0.0);
        let action = bridge.tick(0.0);
        // Fused: (0.6 * 1.0 + 0.4 * 0.0) / (0.6 + 0.4) = 0.6
        assert!((action.position - 0.6).abs() < 0.1);
    }

    #[test]
    fn speed_limit() {
        let mut bridge = SignalBridge::new(1, 1.0, 0.0, 1.0, 50);
        bridge.add_source("src", 1.0);

        // Start at 0.5
        bridge.update("src", 0.5, 1.0);
        bridge.tick(1.0);

        // Jump to 1.0 — speed limit should cap the delta
        bridge.update("src", 1.0, 1.1);
        let action = bridge.tick(1.1);
        // Max delta = 1.0 * 0.1 = 0.1 from 0.5 → capped at 0.6
        assert!(action.position <= 0.61);
    }

    #[test]
    fn stale_source_ignored() {
        let mut bridge = SignalBridge::new(1, 500.0, 0.0, 1.0, 50);
        bridge.add_source("osc", 1.0);

        bridge.update("osc", 0.8, 1.0);
        bridge.tick(1.0);

        // Tick at t=2.0 — source is stale (>0.5s since last update at 1.0)
        let action = bridge.tick(2.0);
        // No active sources → fused = 0.0, speed-limited toward 0.0
        assert!(action.position < 0.8);
    }

    #[test]
    fn weighted_average_fn() {
        let mut buf = VecDeque::new();
        buf.push_back(0.0);
        buf.push_back(1.0);
        // Weighted: (1*0.0 + 2*1.0) / (1+2) = 0.6667
        let avg = weighted_average(&buf);
        assert!((avg - 0.6667).abs() < 0.001);
    }

    #[test]
    fn weighted_average_empty() {
        let buf = VecDeque::new();
        assert!((weighted_average(&buf) - 0.0).abs() < 1e-6);
    }

    #[test]
    fn weighted_average_single() {
        let mut buf = VecDeque::new();
        buf.push_back(0.42);
        assert!((weighted_average(&buf) - 0.42).abs() < 1e-6);
    }

    #[test]
    fn no_sources_returns_zero() {
        let mut bridge = SignalBridge::new(5, 500.0, 0.0, 1.0, 50);
        let action = bridge.tick(1.0);
        // No sources → fused = 0.0
        assert!((action.position - 0.0).abs() < 0.5 + 1e-6);
    }

    #[test]
    fn position_clamped_to_range() {
        let mut bridge = SignalBridge::new(1, 10000.0, 0.2, 0.8, 50);
        bridge.add_source("src", 1.0);
        bridge.update("src", 1.0, 1.0);
        let action = bridge.tick(1.0);
        assert!(action.position <= 0.8 + 1e-6);
        assert!(action.position >= 0.2 - 1e-6);
    }

    #[test]
    fn input_clamped_to_0_1() {
        let mut bridge = SignalBridge::new(1, 500.0, 0.0, 1.0, 50);
        bridge.add_source("src", 1.0);
        bridge.update("src", 2.0, 1.0); // should be clamped to 1.0
        let action = bridge.tick(1.0);
        assert!(action.position <= 1.0 + 1e-6);
    }

    #[test]
    fn negative_input_clamped() {
        let mut bridge = SignalBridge::new(1, 500.0, 0.0, 1.0, 50);
        bridge.add_source("src", 1.0);
        bridge.update("src", -1.0, 1.0); // should be clamped to 0.0
        let action = bridge.tick(1.0);
        assert!(action.position >= 0.0 - 1e-6);
    }

    #[test]
    fn large_speed_limit_no_restriction() {
        let mut bridge = SignalBridge::new(1, 100_000.0, 0.0, 1.0, 50);
        bridge.add_source("src", 1.0);
        bridge.update("src", 0.0, 1.0);
        bridge.tick(1.0);
        bridge.update("src", 1.0, 1.1);
        let action = bridge.tick(1.1);
        // With huge speed limit, should reach target
        assert!((action.position - 1.0).abs() < 0.01);
    }

    #[test]
    fn tick_count_increments() {
        let mut bridge = SignalBridge::new(1, 500.0, 0.0, 1.0, 50);
        assert_eq!(bridge.tick_count(), 0);
        bridge.tick(1.0);
        assert_eq!(bridge.tick_count(), 1);
        bridge.tick(1.1);
        assert_eq!(bridge.tick_count(), 2);
    }

    #[test]
    fn last_position_tracks() {
        let mut bridge = SignalBridge::new(1, 10000.0, 0.0, 1.0, 50);
        bridge.add_source("src", 1.0);
        bridge.update("src", 0.7, 1.0);
        bridge.tick(1.0);
        assert!((bridge.last_position() - 0.7).abs() < 0.01);
    }

    #[test]
    fn update_unknown_source_ignored() {
        let mut bridge = SignalBridge::new(1, 500.0, 0.0, 1.0, 50);
        bridge.add_source("osc", 1.0);
        bridge.update("nonexistent", 0.8, 1.0);
        let active = bridge.active_sources();
        assert!(active.is_empty());
    }

    #[test]
    fn active_sources_after_stale() {
        let mut bridge = SignalBridge::new(1, 500.0, 0.0, 1.0, 50);
        bridge.add_source("a", 1.0);
        bridge.add_source("b", 1.0);
        bridge.update("a", 0.5, 1.0);
        bridge.update("b", 0.5, 1.0);
        bridge.tick(1.0);
        assert_eq!(bridge.active_sources().len(), 2);
        // Only update "a" — "b" goes stale
        bridge.update("a", 0.5, 2.0);
        bridge.tick(2.0);
        assert_eq!(bridge.active_sources().len(), 1);
        assert_eq!(bridge.active_sources()[0], "a");
    }

    #[test]
    fn bridge_action_fields() {
        let mut bridge = SignalBridge::new(1, 500.0, 0.0, 1.0, 50);
        bridge.add_source("src", 1.0);
        bridge.update("src", 0.5, 1.0);
        let action = bridge.tick(1.0);
        assert_eq!(action.duration_ms, 50);
        assert!((action.timestamp - 1.0).abs() < 1e-6);
    }

    #[test]
    fn smoothing_with_multiple_updates() {
        let mut bridge = SignalBridge::new(3, 10000.0, 0.0, 1.0, 50);
        bridge.add_source("src", 1.0);
        // Push 3 values: 0.0, 0.5, 1.0
        bridge.update("src", 0.0, 1.0);
        bridge.update("src", 0.5, 1.0);
        bridge.update("src", 1.0, 1.0);
        let action = bridge.tick(1.0);
        // Weighted average: (1*0.0 + 2*0.5 + 3*1.0) / (1+2+3) = 4.0/6.0 ≈ 0.6667
        assert!((action.position - 0.6667).abs() < 0.01);
    }
}
