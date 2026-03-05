//! Presence bridge — ALICE-Presence ↔ ALICE-Bridge
//!
//! Converts proximity events and crossing records into device triggers.
//! Distance-based haptic feedback for spatial awareness.

/// Proximity event for device triggering.
#[derive(Debug, Clone, Copy)]
pub struct ProximityDeviceEvent {
    /// Distance to the nearest peer (metres).
    pub distance: f64,
    /// Rate of distance change (m/s, negative = approaching).
    pub approach_rate: f64,
    /// Crossing status: 0=Separated, 1=Crossing, 2=Stable.
    pub crossing_status: u8,
    /// Peer ID.
    pub peer_id: u32,
    /// Timestamp (monotonic seconds).
    pub timestamp: f64,
}

/// Crossing status constants.
pub const CROSSING_SEPARATED: u8 = 0;
pub const CROSSING_CROSSING: u8 = 1;
pub const CROSSING_STABLE: u8 = 2;

/// Convert proximity event to haptic intensity.
///
/// Closer distance → stronger feedback. Approaching boosts intensity.
#[inline]
#[must_use]
pub fn proximity_to_haptic_intensity(event: &ProximityDeviceEvent, max_range: f64) -> f64 {
    if max_range < 1e-10 {
        return 0.0;
    }
    // 距離に反比例する基本強度
    let distance_ratio = 1.0 - (event.distance / max_range).clamp(0.0, 1.0);
    // 接近時のブースト
    let approach_boost = if event.approach_rate < 0.0 {
        (-event.approach_rate / 5.0).clamp(0.0, 0.3)
    } else {
        0.0
    };
    // クロッシングイベント時は強いフィードバック
    let crossing_boost = match event.crossing_status {
        CROSSING_CROSSING => 0.2,
        _ => 0.0,
    };
    (distance_ratio + approach_boost + crossing_boost).clamp(0.0, 1.0)
}

/// Whether a proximity event should trigger device activation.
#[inline]
#[must_use]
pub fn should_activate_device(event: &ProximityDeviceEvent, threshold: f64) -> bool {
    event.distance < threshold
}

/// Haptic pattern for crossing events.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CrossingHapticPattern {
    /// No haptic feedback.
    None,
    /// Single pulse on crossing boundary.
    Pulse,
    /// Sustained vibration while in proximity.
    Sustained,
    /// Increasing intensity as distance decreases.
    Gradient,
}

/// Determine haptic pattern from crossing status and distance.
#[inline]
#[must_use]
pub fn crossing_to_haptic_pattern(
    event: &ProximityDeviceEvent,
    near_threshold: f64,
) -> CrossingHapticPattern {
    match event.crossing_status {
        CROSSING_CROSSING => CrossingHapticPattern::Pulse,
        CROSSING_STABLE if event.distance < near_threshold => CrossingHapticPattern::Sustained,
        CROSSING_STABLE => CrossingHapticPattern::Gradient,
        _ => CrossingHapticPattern::None,
    }
}

/// Group proximity state for multi-peer scenarios.
#[derive(Debug, Clone, Copy)]
pub struct GroupProximityState {
    /// Number of peers within range.
    pub peers_in_range: u32,
    /// Nearest peer distance.
    pub nearest_distance: f64,
    /// Average distance to all peers in range.
    pub average_distance: f64,
}

/// Convert group proximity to haptic intensity.
#[inline]
#[must_use]
pub fn group_proximity_to_intensity(state: &GroupProximityState, max_range: f64) -> f64 {
    if state.peers_in_range == 0 || max_range < 1e-10 {
        return 0.0;
    }
    let nearest_ratio = 1.0 - (state.nearest_distance / max_range).clamp(0.0, 1.0);
    let crowd_factor = (f64::from(state.peers_in_range) / 10.0).clamp(0.0, 1.0);
    (nearest_ratio * 0.7 + crowd_factor * 0.3).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_event(distance: f64, approach_rate: f64, status: u8) -> ProximityDeviceEvent {
        ProximityDeviceEvent {
            distance,
            approach_rate,
            crossing_status: status,
            peer_id: 1,
            timestamp: 1.0,
        }
    }

    #[test]
    fn zero_distance_full_intensity() {
        let event = make_event(0.0, 0.0, CROSSING_STABLE);
        assert!((proximity_to_haptic_intensity(&event, 10.0) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn max_distance_zero_intensity() {
        let event = make_event(10.0, 0.0, CROSSING_SEPARATED);
        assert!((proximity_to_haptic_intensity(&event, 10.0) - 0.0).abs() < 1e-6);
    }

    #[test]
    fn approaching_boost() {
        let stationary = make_event(5.0, 0.0, CROSSING_STABLE);
        let approaching = make_event(5.0, -3.0, CROSSING_STABLE);
        let i_stat = proximity_to_haptic_intensity(&stationary, 10.0);
        let i_appr = proximity_to_haptic_intensity(&approaching, 10.0);
        assert!(i_appr > i_stat);
    }

    #[test]
    fn crossing_boost() {
        let stable = make_event(3.0, 0.0, CROSSING_STABLE);
        let crossing = make_event(3.0, 0.0, CROSSING_CROSSING);
        let i_stable = proximity_to_haptic_intensity(&stable, 10.0);
        let i_crossing = proximity_to_haptic_intensity(&crossing, 10.0);
        assert!(i_crossing > i_stable);
    }

    #[test]
    fn should_activate_within_threshold() {
        let event = make_event(2.0, 0.0, CROSSING_STABLE);
        assert!(should_activate_device(&event, 5.0));
        assert!(!should_activate_device(&event, 1.0));
    }

    #[test]
    fn crossing_pattern_separated() {
        let event = make_event(10.0, 0.0, CROSSING_SEPARATED);
        assert_eq!(
            crossing_to_haptic_pattern(&event, 2.0),
            CrossingHapticPattern::None
        );
    }

    #[test]
    fn crossing_pattern_pulse() {
        let event = make_event(3.0, -1.0, CROSSING_CROSSING);
        assert_eq!(
            crossing_to_haptic_pattern(&event, 2.0),
            CrossingHapticPattern::Pulse
        );
    }

    #[test]
    fn crossing_pattern_sustained() {
        let event = make_event(1.0, 0.0, CROSSING_STABLE);
        assert_eq!(
            crossing_to_haptic_pattern(&event, 2.0),
            CrossingHapticPattern::Sustained
        );
    }

    #[test]
    fn group_proximity_empty() {
        let state = GroupProximityState {
            peers_in_range: 0,
            nearest_distance: 100.0,
            average_distance: 100.0,
        };
        assert!((group_proximity_to_intensity(&state, 10.0) - 0.0).abs() < 1e-6);
    }

    #[test]
    fn group_proximity_close_crowd() {
        let state = GroupProximityState {
            peers_in_range: 5,
            nearest_distance: 1.0,
            average_distance: 3.0,
        };
        let intensity = group_proximity_to_intensity(&state, 10.0);
        assert!(intensity > 0.5);
    }
}
