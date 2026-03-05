//! Kinematics bridge — ALICE-Kinematics ↔ ALICE-Bridge
//!
//! Maps skeletal joint angles and motion intents to device commands.
//! Bone movements → haptic feedback for motion capture replay.

use crate::device::ActuatorType;

/// Joint haptic descriptor for skeleton-driven feedback.
#[derive(Debug, Clone, Copy)]
pub struct JointHapticDescriptor {
    /// Joint index in the skeleton.
    pub joint_index: u16,
    /// Joint angle (radians).
    pub angle: f32,
    /// Angular velocity (rad/s).
    pub angular_velocity: f32,
    /// Joint torque (N·m).
    pub torque: f32,
    /// Whether joint is at its limit.
    pub at_limit: bool,
}

/// Convert joint descriptor to haptic intensity.
///
/// Intensity driven by angular velocity and torque.
#[inline]
#[must_use]
pub fn joint_to_haptic_intensity(
    desc: &JointHapticDescriptor,
    max_angular_vel: f32,
    max_torque: f32,
) -> f64 {
    let vel_ratio = if max_angular_vel > 1e-6 {
        (desc.angular_velocity.abs() / max_angular_vel).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let torque_ratio = if max_torque > 1e-6 {
        (desc.torque.abs() / max_torque).clamp(0.0, 1.0)
    } else {
        0.0
    };
    // 関節リミットに達した場合は強いフィードバック
    let limit_boost = if desc.at_limit { 0.3 } else { 0.0 };
    (f64::from(vel_ratio) * 0.4 + f64::from(torque_ratio) * 0.3 + limit_boost).clamp(0.0, 1.0)
}

/// Skeleton pose haptic snapshot.
///
/// Captures the overall body motion intensity for whole-body haptic feedback.
#[derive(Debug, Clone, Copy)]
pub struct SkeletonHapticSnapshot {
    /// Total kinetic energy of the skeleton (J).
    pub kinetic_energy: f64,
    /// Maximum joint angular velocity in the pose (rad/s).
    pub max_angular_velocity: f32,
    /// Number of joints at their limits.
    pub joints_at_limit: u16,
    /// Total joint count.
    pub total_joints: u16,
    /// Root position.
    pub root_position: [f32; 3],
    /// Root velocity.
    pub root_velocity: [f32; 3],
}

/// Convert skeleton snapshot to overall haptic intensity.
#[inline]
#[must_use]
pub fn skeleton_to_haptic_intensity(snap: &SkeletonHapticSnapshot, max_energy: f64) -> f64 {
    if max_energy < 1e-10 {
        return 0.0;
    }
    let energy_ratio = (snap.kinetic_energy / max_energy).clamp(0.0, 1.0);
    let limit_ratio = if snap.total_joints > 0 {
        f64::from(snap.joints_at_limit) / f64::from(snap.total_joints)
    } else {
        0.0
    };
    (energy_ratio * 0.7 + limit_ratio * 0.3).clamp(0.0, 1.0)
}

/// Recommended actuator type for joint feedback.
#[inline]
#[must_use]
pub fn joint_actuator_type(desc: &JointHapticDescriptor) -> ActuatorType {
    if desc.at_limit {
        ActuatorType::Linear // 関節リミット → 強い線形フィードバック
    } else if desc.angular_velocity.abs() > 3.0 {
        ActuatorType::Vibrate // 速い動き → 振動
    } else {
        ActuatorType::Position // ゆっくりな動き → ポジション
    }
}

/// Intent haptic descriptor — from ALICE-Kinematics' compact intent format.
#[derive(Debug, Clone, Copy)]
pub struct IntentHapticDescriptor {
    /// Intent type (0=Reach, 1=Grasp, 2=Point, 3=Retract, 4=Release).
    pub intent_type: u8,
    /// Target position.
    pub target: [f32; 3],
    /// Duration (seconds).
    pub duration_secs: f32,
    /// Priority (0=low, 3=high).
    pub priority: u8,
}

/// Convert intent to haptic intensity based on type and priority.
#[inline]
#[must_use]
pub fn intent_to_haptic_intensity(desc: &IntentHapticDescriptor) -> f64 {
    let base = match desc.intent_type {
        0 => 0.3, // Reach
        1 => 0.6, // Grasp
        3 => 0.4, // Retract
        4 => 0.2, // Release
        _ => 0.1, // Point / unknown
    };
    let priority_scale = 0.5 + f64::from(desc.priority) * 0.5 / 3.0;
    (base * priority_scale).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn joint_zero_motion() {
        let desc = JointHapticDescriptor {
            joint_index: 0,
            angle: 0.0,
            angular_velocity: 0.0,
            torque: 0.0,
            at_limit: false,
        };
        assert!((joint_to_haptic_intensity(&desc, 10.0, 100.0) - 0.0).abs() < 1e-6);
    }

    #[test]
    fn joint_at_limit_boost() {
        let desc = JointHapticDescriptor {
            joint_index: 0,
            angle: 1.5,
            angular_velocity: 0.0,
            torque: 0.0,
            at_limit: true,
        };
        let intensity = joint_to_haptic_intensity(&desc, 10.0, 100.0);
        assert!((intensity - 0.3).abs() < 1e-6);
    }

    #[test]
    fn joint_full_velocity() {
        let desc = JointHapticDescriptor {
            joint_index: 5,
            angle: 0.5,
            angular_velocity: 10.0,
            torque: 0.0,
            at_limit: false,
        };
        let intensity = joint_to_haptic_intensity(&desc, 10.0, 100.0);
        assert!((intensity - 0.4).abs() < 1e-6);
    }

    #[test]
    fn skeleton_zero_energy() {
        let snap = SkeletonHapticSnapshot {
            kinetic_energy: 0.0,
            max_angular_velocity: 0.0,
            joints_at_limit: 0,
            total_joints: 20,
            root_position: [0.0; 3],
            root_velocity: [0.0; 3],
        };
        assert!((skeleton_to_haptic_intensity(&snap, 100.0) - 0.0).abs() < 1e-6);
    }

    #[test]
    fn skeleton_full_energy() {
        let snap = SkeletonHapticSnapshot {
            kinetic_energy: 100.0,
            max_angular_velocity: 5.0,
            joints_at_limit: 0,
            total_joints: 20,
            root_position: [0.0; 3],
            root_velocity: [1.0, 0.0, 0.0],
        };
        assert!((skeleton_to_haptic_intensity(&snap, 100.0) - 0.7).abs() < 1e-6);
    }

    #[test]
    fn joint_actuator_at_limit() {
        let desc = JointHapticDescriptor {
            joint_index: 0,
            angle: 0.0,
            angular_velocity: 0.0,
            torque: 0.0,
            at_limit: true,
        };
        assert_eq!(joint_actuator_type(&desc), ActuatorType::Linear);
    }

    #[test]
    fn joint_actuator_fast_vibrate() {
        let desc = JointHapticDescriptor {
            joint_index: 0,
            angle: 0.0,
            angular_velocity: 5.0,
            torque: 0.0,
            at_limit: false,
        };
        assert_eq!(joint_actuator_type(&desc), ActuatorType::Vibrate);
    }

    #[test]
    fn intent_grasp_high_priority() {
        let desc = IntentHapticDescriptor {
            intent_type: 1, // Grasp
            target: [0.0; 3],
            duration_secs: 0.5,
            priority: 3,
        };
        let intensity = intent_to_haptic_intensity(&desc);
        assert!(intensity > 0.5);
    }

    #[test]
    fn intent_point_low() {
        let desc = IntentHapticDescriptor {
            intent_type: 2, // Point
            target: [1.0, 0.0, 0.0],
            duration_secs: 1.0,
            priority: 0,
        };
        let intensity = intent_to_haptic_intensity(&desc);
        assert!(intensity < 0.2);
    }

    #[test]
    fn skeleton_zero_max_energy() {
        let snap = SkeletonHapticSnapshot {
            kinetic_energy: 50.0,
            max_angular_velocity: 0.0,
            joints_at_limit: 0,
            total_joints: 0,
            root_position: [0.0; 3],
            root_velocity: [0.0; 3],
        };
        assert!((skeleton_to_haptic_intensity(&snap, 0.0) - 0.0).abs() < 1e-6);
    }
}
