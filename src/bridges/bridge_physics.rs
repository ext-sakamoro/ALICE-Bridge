//! Physics bridge — ALICE-Physics ↔ ALICE-Bridge
//!
//! Converts collision forces and impulses into haptic device commands.
//! Deterministic Fix128 physics → f64 intensity for actuators.

use crate::bridge::SignalBridge;
use crate::device::ActuatorType;

/// Physics collision event for haptic feedback.
#[derive(Debug, Clone, Copy)]
pub struct CollisionHapticEvent {
    /// Collision impulse magnitude (N·s), from physics solver.
    pub impulse_magnitude: f64,
    /// Contact point in world space.
    pub contact_point: [f32; 3],
    /// Contact normal direction.
    pub contact_normal: [f32; 3],
    /// Relative velocity at contact (m/s).
    pub relative_velocity: f64,
    /// Body mass of the impacting object (kg).
    pub body_mass: f64,
}

/// Convert collision event to haptic intensity [0.0, 1.0].
///
/// Intensity is proportional to impulse, scaled by reference thresholds.
#[inline]
#[must_use]
pub fn collision_to_haptic_intensity(event: &CollisionHapticEvent, max_impulse: f64) -> f64 {
    if max_impulse < 1e-10 {
        return 0.0;
    }
    (event.impulse_magnitude / max_impulse).clamp(0.0, 1.0)
}

/// Recommended actuator type for a collision event.
///
/// Light impacts → Vibrate, heavy impacts → Linear.
#[inline]
#[must_use]
pub fn collision_actuator_type(event: &CollisionHapticEvent) -> ActuatorType {
    if event.impulse_magnitude > 50.0 {
        ActuatorType::Linear
    } else {
        ActuatorType::Vibrate
    }
}

/// Physics force feedback descriptor for continuous haptic effects.
#[derive(Debug, Clone, Copy)]
pub struct ForceHapticDescriptor {
    /// Force magnitude (N).
    pub force_magnitude: f64,
    /// Force direction in world space (normalized).
    pub force_direction: [f32; 3],
    /// Application point offset from body center.
    pub offset: [f32; 3],
    /// Force type identifier (gravity, wind, contact, etc.).
    pub force_type: u8,
}

/// Force type constants.
pub const FORCE_GRAVITY: u8 = 0;
pub const FORCE_WIND: u8 = 1;
pub const FORCE_CONTACT: u8 = 2;
pub const FORCE_SPRING: u8 = 3;
pub const FORCE_DRAG: u8 = 4;

/// Convert a force descriptor to sustained haptic intensity.
///
/// Gravity is filtered out; wind and contact scale linearly.
#[inline]
#[must_use]
pub fn force_to_haptic_intensity(desc: &ForceHapticDescriptor, max_force: f64) -> f64 {
    // 重力は触覚フィードバックに含めない
    if desc.force_type == FORCE_GRAVITY {
        return 0.0;
    }
    if max_force < 1e-10 {
        return 0.0;
    }
    (desc.force_magnitude / max_force).clamp(0.0, 1.0)
}

/// Feed a collision event into a `SignalBridge` as a "physics" source.
#[inline]
pub fn feed_collision_to_signal_bridge(
    bridge: &mut SignalBridge,
    event: &CollisionHapticEvent,
    max_impulse: f64,
    now_secs: f64,
) {
    let intensity = collision_to_haptic_intensity(event, max_impulse);
    bridge.update("physics", intensity, now_secs);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_event(impulse: f64, velocity: f64) -> CollisionHapticEvent {
        CollisionHapticEvent {
            impulse_magnitude: impulse,
            contact_point: [0.0; 3],
            contact_normal: [0.0, 1.0, 0.0],
            relative_velocity: velocity,
            body_mass: 10.0,
        }
    }

    #[test]
    fn zero_impulse_zero_intensity() {
        let event = make_event(0.0, 0.0);
        assert!((collision_to_haptic_intensity(&event, 100.0) - 0.0).abs() < 1e-6);
    }

    #[test]
    fn full_impulse_full_intensity() {
        let event = make_event(100.0, 5.0);
        assert!((collision_to_haptic_intensity(&event, 100.0) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn half_impulse_half_intensity() {
        let event = make_event(50.0, 3.0);
        assert!((collision_to_haptic_intensity(&event, 100.0) - 0.5).abs() < 1e-6);
    }

    #[test]
    fn clamped_at_one() {
        let event = make_event(200.0, 10.0);
        assert!((collision_to_haptic_intensity(&event, 100.0) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn zero_max_impulse_returns_zero() {
        let event = make_event(50.0, 5.0);
        assert!((collision_to_haptic_intensity(&event, 0.0) - 0.0).abs() < 1e-6);
    }

    #[test]
    fn light_collision_vibrate() {
        let event = make_event(10.0, 1.0);
        assert_eq!(collision_actuator_type(&event), ActuatorType::Vibrate);
    }

    #[test]
    fn heavy_collision_linear() {
        let event = make_event(100.0, 10.0);
        assert_eq!(collision_actuator_type(&event), ActuatorType::Linear);
    }

    #[test]
    fn gravity_force_filtered() {
        let desc = ForceHapticDescriptor {
            force_magnitude: 100.0,
            force_direction: [0.0, -1.0, 0.0],
            offset: [0.0; 3],
            force_type: FORCE_GRAVITY,
        };
        assert!((force_to_haptic_intensity(&desc, 100.0) - 0.0).abs() < 1e-6);
    }

    #[test]
    fn wind_force_scales() {
        let desc = ForceHapticDescriptor {
            force_magnitude: 30.0,
            force_direction: [1.0, 0.0, 0.0],
            offset: [0.0; 3],
            force_type: FORCE_WIND,
        };
        assert!((force_to_haptic_intensity(&desc, 100.0) - 0.3).abs() < 1e-6);
    }

    #[test]
    fn feed_collision_updates_bridge() {
        let mut bridge = SignalBridge::new(5, 500.0, 0.0, 1.0, 50);
        bridge.add_source("physics", 1.0);
        let event = make_event(80.0, 5.0);
        feed_collision_to_signal_bridge(&mut bridge, &event, 100.0, 1.0);
        let action = bridge.tick(1.0);
        assert!(action.position > 0.0);
    }
}
