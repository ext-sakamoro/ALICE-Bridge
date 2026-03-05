//! Telemetry bridge — ALICE-Semantic-Telemetry ↔ ALICE-Bridge
//!
//! Emits device state changes as semantic telemetry events.
//! Maps device lifecycle to observability spans.

/// Semantic event kind for device state changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceEventKind {
    /// Device connected.
    Connected,
    /// Device disconnected.
    Disconnected,
    /// Intensity changed significantly.
    IntensityChange,
    /// Safety limiter engaged.
    SafetyEngaged,
    /// Safety limiter disengaged.
    SafetyDisengaged,
    /// Emergency stop triggered.
    EmergencyStop,
    /// Protocol error.
    ProtocolError,
}

impl DeviceEventKind {
    /// Map to ALICE-Semantic-Telemetry event kind code.
    #[inline]
    #[must_use]
    pub const fn to_event_kind_code(&self) -> u8 {
        match self {
            Self::Connected | Self::Disconnected => 0, // StateTransition
            Self::IntensityChange => 3,                // DataFlow
            Self::SafetyEngaged | Self::SafetyDisengaged => 4, // ThresholdCrossing
            Self::EmergencyStop | Self::ProtocolError => 5, // AnomalyDetected
        }
    }

    /// Severity level (0=Trace..5=Fatal).
    #[inline]
    #[must_use]
    pub const fn severity(&self) -> u8 {
        match self {
            Self::Connected | Self::Disconnected => 2, // Info
            Self::IntensityChange => 1,                // Debug
            Self::SafetyEngaged | Self::SafetyDisengaged | Self::ProtocolError => 3, // Warn
            Self::EmergencyStop => 4,                  // Error
        }
    }
}

/// Semantic telemetry event for device state.
#[derive(Debug, Clone)]
pub struct DeviceTelemetryEvent {
    /// Event kind.
    pub kind: DeviceEventKind,
    /// Device ID.
    pub device_id: String,
    /// Timestamp (nanoseconds, monotonic).
    pub timestamp_ns: u64,
    /// Primary payload (intensity, error code, etc.).
    pub payload: u64,
    /// Secondary payload.
    pub payload2: u64,
    /// Source ID (FNV-1a hash of `device_id`).
    pub source_id: u64,
}

/// FNV-1a hash (file-local, per ALICE bridge convention).
#[inline]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in data {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

/// Create a telemetry event for a device state change.
#[inline]
#[must_use]
pub fn device_state_to_telemetry(
    kind: DeviceEventKind,
    device_id: &str,
    timestamp_ns: u64,
    payload: u64,
) -> DeviceTelemetryEvent {
    DeviceTelemetryEvent {
        kind,
        device_id: device_id.to_string(),
        timestamp_ns,
        payload,
        payload2: 0,
        source_id: fnv1a(device_id.as_bytes()),
    }
}

/// Create a telemetry event for an intensity change.
#[inline]
#[must_use]
pub fn intensity_change_to_telemetry(
    device_id: &str,
    old_intensity: f64,
    new_intensity: f64,
    timestamp_ns: u64,
) -> DeviceTelemetryEvent {
    DeviceTelemetryEvent {
        kind: DeviceEventKind::IntensityChange,
        device_id: device_id.to_string(),
        timestamp_ns,
        payload: old_intensity.to_bits(),
        payload2: new_intensity.to_bits(),
        source_id: fnv1a(device_id.as_bytes()),
    }
}

/// Whether an intensity change is significant enough to emit telemetry.
#[inline]
#[must_use]
pub fn is_significant_change(old: f64, new: f64, threshold: f64) -> bool {
    (new - old).abs() >= threshold
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_kind_codes() {
        assert_eq!(DeviceEventKind::Connected.to_event_kind_code(), 0);
        assert_eq!(DeviceEventKind::IntensityChange.to_event_kind_code(), 3);
        assert_eq!(DeviceEventKind::EmergencyStop.to_event_kind_code(), 5);
    }

    #[test]
    fn event_severity() {
        assert_eq!(DeviceEventKind::Connected.severity(), 2);
        assert_eq!(DeviceEventKind::EmergencyStop.severity(), 4);
        assert_eq!(DeviceEventKind::IntensityChange.severity(), 1);
    }

    #[test]
    fn telemetry_event_source_id() {
        let event = device_state_to_telemetry(DeviceEventKind::Connected, "dev:0", 1000, 0);
        assert_ne!(event.source_id, 0);
        // 同じデバイスIDなら同じハッシュ
        let event2 = device_state_to_telemetry(DeviceEventKind::Disconnected, "dev:0", 2000, 0);
        assert_eq!(event.source_id, event2.source_id);
    }

    #[test]
    fn telemetry_event_different_devices() {
        let e1 = device_state_to_telemetry(DeviceEventKind::Connected, "dev:0", 0, 0);
        let e2 = device_state_to_telemetry(DeviceEventKind::Connected, "dev:1", 0, 0);
        assert_ne!(e1.source_id, e2.source_id);
    }

    #[test]
    fn intensity_change_payloads() {
        let event = intensity_change_to_telemetry("dev:0", 0.3, 0.8, 5000);
        assert_eq!(event.kind, DeviceEventKind::IntensityChange);
        assert_eq!(event.payload, 0.3_f64.to_bits());
        assert_eq!(event.payload2, 0.8_f64.to_bits());
    }

    #[test]
    fn significant_change_detection() {
        assert!(is_significant_change(0.0, 0.5, 0.1));
        assert!(!is_significant_change(0.5, 0.55, 0.1));
        assert!(is_significant_change(0.5, 0.61, 0.1));
    }

    #[test]
    fn fnv1a_deterministic() {
        let h1 = fnv1a(b"test_device");
        let h2 = fnv1a(b"test_device");
        assert_eq!(h1, h2);
        assert_ne!(h1, 0);
    }

    #[test]
    fn fnv1a_different_inputs() {
        let h1 = fnv1a(b"device_a");
        let h2 = fnv1a(b"device_b");
        assert_ne!(h1, h2);
    }
}
