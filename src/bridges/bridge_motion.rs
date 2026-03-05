//! Motion bridge — ALICE-Motion ↔ ALICE-Bridge
//!
//! Converts motion plan trajectories into device mapping parameters.
//! S-curve / trapezoidal velocity profiles → intensity curves for actuators.

use crate::device::DeviceMapping;

/// Motion plan descriptor for device control.
///
/// Extracted from ALICE-Motion's `MotionPlan` to drive actuator intensity
/// along a trajectory over time.
#[derive(Debug, Clone, Copy)]
pub struct MotionDeviceDescriptor {
    /// Normalized progress along the motion path [0.0, 1.0].
    pub progress: f64,
    /// Instantaneous velocity magnitude (m/s), normalized to [0.0, 1.0].
    pub velocity_normalized: f64,
    /// Instantaneous acceleration magnitude, normalized to [0.0, 1.0].
    pub acceleration_normalized: f64,
    /// Total duration of the motion plan (seconds).
    pub total_duration_secs: f64,
    /// Current position on the path.
    pub position: [f32; 3],
}

/// Convert a motion descriptor into device intensity.
///
/// Maps velocity-based intensity: faster motion → stronger feedback.
#[inline]
#[must_use]
pub fn motion_to_device_intensity(desc: &MotionDeviceDescriptor) -> f64 {
    // ベース強度: 速度に比例
    let base = desc.velocity_normalized;
    // 加速度ブースト: 加速・減速時にフィードバック増加
    let accel_boost = desc.acceleration_normalized * 0.3;
    (base + accel_boost).clamp(0.0, 1.0)
}

/// Create a `DeviceMapping` from a motion descriptor.
///
/// Sets scale based on velocity and delay based on motion progress.
#[inline]
#[must_use]
pub fn motion_to_device_mapping(desc: &MotionDeviceDescriptor, device_id: &str) -> DeviceMapping {
    DeviceMapping {
        device_id: device_id.to_string(),
        label: format!("motion_{device_id}"),
        scale: desc.velocity_normalized.clamp(0.0, 2.0),
        offset: desc.acceleration_normalized * 0.1,
        invert: false,
        delay_ms: 0,
        source_filter: "motion".into(),
        group: "motion".into(),
    }
}

/// Trajectory sample for continuous device control.
#[derive(Debug, Clone, Copy)]
pub struct TrajectorySample {
    /// Time offset from motion start (seconds).
    pub time_secs: f64,
    /// Position on the curve.
    pub position: [f32; 3],
    /// Tangent direction (normalized).
    pub tangent: [f32; 3],
    /// Curvature at this point.
    pub curvature: f32,
    /// Velocity magnitude (m/s).
    pub speed: f32,
}

/// Convert a trajectory sample to device intensity.
///
/// Combines speed and curvature: sharp turns at speed → high intensity.
#[inline]
#[must_use]
pub fn trajectory_sample_to_intensity(
    sample: &TrajectorySample,
    max_speed: f32,
    max_curvature: f32,
) -> f64 {
    let speed_ratio = if max_speed > 1e-6 {
        (sample.speed / max_speed).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let curvature_ratio = if max_curvature > 1e-6 {
        (sample.curvature / max_curvature).clamp(0.0, 1.0)
    } else {
        0.0
    };
    // 曲率×速度 で「遠心力的」強度
    let combined = f64::from(speed_ratio) * 0.6 + f64::from(speed_ratio * curvature_ratio) * 0.4;
    combined.clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn motion_to_intensity_zero_velocity() {
        let desc = MotionDeviceDescriptor {
            progress: 0.0,
            velocity_normalized: 0.0,
            acceleration_normalized: 0.0,
            total_duration_secs: 1.0,
            position: [0.0; 3],
        };
        assert!((motion_to_device_intensity(&desc) - 0.0).abs() < 1e-6);
    }

    #[test]
    fn motion_to_intensity_full_speed() {
        let desc = MotionDeviceDescriptor {
            progress: 0.5,
            velocity_normalized: 1.0,
            acceleration_normalized: 0.0,
            total_duration_secs: 2.0,
            position: [1.0, 0.0, 0.0],
        };
        assert!((motion_to_device_intensity(&desc) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn motion_to_intensity_with_accel_boost() {
        let desc = MotionDeviceDescriptor {
            progress: 0.5,
            velocity_normalized: 0.5,
            acceleration_normalized: 1.0,
            total_duration_secs: 2.0,
            position: [0.0; 3],
        };
        let intensity = motion_to_device_intensity(&desc);
        // 0.5 + 1.0*0.3 = 0.8
        assert!((intensity - 0.8).abs() < 1e-6);
    }

    #[test]
    fn motion_to_mapping_fields() {
        let desc = MotionDeviceDescriptor {
            progress: 0.5,
            velocity_normalized: 0.8,
            acceleration_normalized: 0.2,
            total_duration_secs: 3.0,
            position: [0.0; 3],
        };
        let mapping = motion_to_device_mapping(&desc, "dev:0");
        assert_eq!(mapping.device_id, "dev:0");
        assert!((mapping.scale - 0.8).abs() < 1e-6);
        assert_eq!(mapping.source_filter, "motion");
        assert_eq!(mapping.group, "motion");
    }

    #[test]
    fn trajectory_sample_speed_only() {
        let sample = TrajectorySample {
            time_secs: 0.5,
            position: [1.0, 0.0, 0.0],
            tangent: [1.0, 0.0, 0.0],
            curvature: 0.0,
            speed: 5.0,
        };
        let intensity = trajectory_sample_to_intensity(&sample, 10.0, 1.0);
        // speed_ratio = 0.5, curvature_ratio = 0.0 → 0.5*0.6 = 0.3
        assert!((intensity - 0.3).abs() < 1e-6);
    }

    #[test]
    fn trajectory_sample_high_curvature() {
        let sample = TrajectorySample {
            time_secs: 1.0,
            position: [0.0; 3],
            tangent: [0.0, 1.0, 0.0],
            curvature: 1.0,
            speed: 10.0,
        };
        let intensity = trajectory_sample_to_intensity(&sample, 10.0, 1.0);
        // speed=1.0, curvature=1.0 → 1.0*0.6 + 1.0*1.0*0.4 = 1.0
        assert!((intensity - 1.0).abs() < 1e-6);
    }

    #[test]
    fn trajectory_sample_zero_max_speed() {
        let sample = TrajectorySample {
            time_secs: 0.0,
            position: [0.0; 3],
            tangent: [1.0, 0.0, 0.0],
            curvature: 0.5,
            speed: 5.0,
        };
        let intensity = trajectory_sample_to_intensity(&sample, 0.0, 1.0);
        assert!((intensity - 0.0).abs() < 1e-6);
    }

    #[test]
    fn motion_intensity_clamps_to_one() {
        let desc = MotionDeviceDescriptor {
            progress: 1.0,
            velocity_normalized: 1.0,
            acceleration_normalized: 1.0,
            total_duration_secs: 1.0,
            position: [0.0; 3],
        };
        let intensity = motion_to_device_intensity(&desc);
        assert!(intensity <= 1.0);
    }
}
