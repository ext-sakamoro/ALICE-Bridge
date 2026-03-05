//! Gradual ramp — smooth intensity increase at session start.

use serde::{Deserialize, Serialize};

/// Ramp curve type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RampCurve {
    Linear,
    EaseIn,
    EaseOut,
    EaseInOut,
}

/// Applies a ramp-up envelope to intensity values.
///
/// During the ramp period, output is scaled from 0.0 to 1.0
/// following a configurable curve. After completion, passthrough.
#[derive(Debug)]
pub struct GradualRamp {
    duration_secs: f64,
    curve: RampCurve,
    start_time: Option<f64>,
    completed: bool,
}

impl GradualRamp {
    #[must_use]
    pub const fn new(duration_secs: f64, curve: RampCurve) -> Self {
        Self {
            duration_secs: duration_secs.max(0.01),
            curve,
            start_time: None,
            completed: false,
        }
    }

    /// Begin the ramp-up period.
    pub const fn start(&mut self, now_secs: f64) {
        self.start_time = Some(now_secs);
        self.completed = false;
    }

    /// Apply ramp envelope to an intensity value.
    pub fn apply(&mut self, intensity: f64, now_secs: f64) -> f64 {
        let Some(start) = self.start_time else {
            return intensity; // Not started
        };

        if self.completed {
            return intensity;
        }

        let elapsed = now_secs - start;
        if elapsed >= self.duration_secs {
            self.completed = true;
            return intensity;
        }

        let t = elapsed / self.duration_secs;
        let scale = self.curve_fn(t);
        intensity * scale
    }

    /// Reset ramp state.
    pub const fn reset(&mut self) {
        self.start_time = None;
        self.completed = false;
    }

    #[must_use]
    pub const fn is_active(&self) -> bool {
        self.start_time.is_some() && !self.completed
    }

    #[must_use]
    pub const fn is_completed(&self) -> bool {
        self.completed
    }

    #[must_use]
    pub fn progress(&self, now_secs: f64) -> f64 {
        match self.start_time {
            None => 0.0,
            Some(start) => {
                if self.completed {
                    return 1.0;
                }
                ((now_secs - start) / self.duration_secs).clamp(0.0, 1.0)
            }
        }
    }

    fn curve_fn(&self, t: f64) -> f64 {
        match self.curve {
            RampCurve::Linear => t,
            RampCurve::EaseIn => t * t,
            RampCurve::EaseOut => (1.0 - t).mul_add(-(1.0 - t), 1.0),
            RampCurve::EaseInOut => {
                if t < 0.5 {
                    2.0 * t * t
                } else {
                    (2.0 * (1.0 - t)).mul_add(-(1.0 - t), 1.0)
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linear_ramp() {
        let mut ramp = GradualRamp::new(10.0, RampCurve::Linear);
        ramp.start(0.0);

        // At t=0: scale=0.0
        assert!((ramp.apply(1.0, 0.0) - 0.0).abs() < 1e-6);
        // At t=5: scale=0.5
        assert!((ramp.apply(1.0, 5.0) - 0.5).abs() < 1e-6);
        // At t=10: completed, passthrough
        assert!((ramp.apply(1.0, 10.0) - 1.0).abs() < 1e-6);
        assert!(ramp.is_completed());
    }

    #[test]
    fn ease_in() {
        let mut ramp = GradualRamp::new(10.0, RampCurve::EaseIn);
        ramp.start(0.0);
        // At t=5: scale = (0.5)^2 = 0.25
        assert!((ramp.apply(1.0, 5.0) - 0.25).abs() < 1e-6);
    }

    #[test]
    fn not_started_passthrough() {
        let mut ramp = GradualRamp::new(5.0, RampCurve::Linear);
        assert!((ramp.apply(0.8, 1.0) - 0.8).abs() < 1e-6);
    }

    #[test]
    fn reset_restarts() {
        let mut ramp = GradualRamp::new(5.0, RampCurve::Linear);
        ramp.start(0.0);
        let _ = ramp.apply(1.0, 6.0);
        assert!(ramp.is_completed());

        ramp.reset();
        assert!(!ramp.is_completed());
        assert!(!ramp.is_active());
    }

    #[test]
    fn ease_out() {
        let mut ramp = GradualRamp::new(10.0, RampCurve::EaseOut);
        ramp.start(0.0);
        // At t=5: t=0.5, ease_out = 1 - (1-0.5)^2 = 1 - 0.25 = 0.75
        let v = ramp.apply(1.0, 5.0);
        assert!((v - 0.75).abs() < 1e-6);
    }

    #[test]
    fn ease_in_out_first_half() {
        let mut ramp = GradualRamp::new(10.0, RampCurve::EaseInOut);
        ramp.start(0.0);
        // At t=2.5: t=0.25, first half: 2 * 0.25^2 = 0.125
        let v = ramp.apply(1.0, 2.5);
        assert!((v - 0.125).abs() < 1e-6);
    }

    #[test]
    fn ease_in_out_second_half() {
        let mut ramp = GradualRamp::new(10.0, RampCurve::EaseInOut);
        ramp.start(0.0);
        // At t=7.5: t=0.75, second half: 1 - 2*(1-0.75)^2 = 1 - 2*0.0625 = 0.875
        let v = ramp.apply(1.0, 7.5);
        assert!((v - 0.875).abs() < 1e-6);
    }

    #[test]
    fn progress_not_started() {
        let ramp = GradualRamp::new(5.0, RampCurve::Linear);
        assert!((ramp.progress(1.0) - 0.0).abs() < 1e-6);
    }

    #[test]
    fn progress_midway() {
        let mut ramp = GradualRamp::new(10.0, RampCurve::Linear);
        ramp.start(0.0);
        assert!((ramp.progress(5.0) - 0.5).abs() < 1e-6);
    }

    #[test]
    fn progress_after_completion() {
        let mut ramp = GradualRamp::new(5.0, RampCurve::Linear);
        ramp.start(0.0);
        let _ = ramp.apply(1.0, 6.0);
        assert!((ramp.progress(10.0) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn is_active_during_ramp() {
        let mut ramp = GradualRamp::new(5.0, RampCurve::Linear);
        assert!(!ramp.is_active());
        ramp.start(0.0);
        assert!(ramp.is_active());
        let _ = ramp.apply(1.0, 6.0);
        assert!(!ramp.is_active()); // completed
    }

    #[test]
    fn intensity_scaled_during_ramp() {
        let mut ramp = GradualRamp::new(4.0, RampCurve::Linear);
        ramp.start(0.0);
        // At t=2: scale=0.5, intensity=0.6 → 0.3
        let v = ramp.apply(0.6, 2.0);
        assert!((v - 0.3).abs() < 1e-6);
    }

    #[test]
    fn completed_passthrough() {
        let mut ramp = GradualRamp::new(1.0, RampCurve::Linear);
        ramp.start(0.0);
        let _ = ramp.apply(1.0, 2.0); // completes
        assert!(ramp.is_completed());
        // After completion, passthrough regardless of value
        let v = ramp.apply(0.42, 3.0);
        assert!((v - 0.42).abs() < 1e-6);
    }

    #[test]
    fn curve_enum_eq() {
        assert_eq!(RampCurve::Linear, RampCurve::Linear);
        assert_ne!(RampCurve::EaseIn, RampCurve::EaseOut);
    }
}
