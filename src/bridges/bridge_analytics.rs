//! Analytics bridge — ALICE-Analytics ↔ ALICE-Bridge
//!
//! Exports device telemetry as analytics metrics.
//! Device state changes → metric events for probabilistic aggregation.

use crate::device::ActuatorType;

/// Device metric event for analytics pipeline.
#[derive(Debug, Clone)]
pub struct DeviceMetricEvent {
    /// Device ID.
    pub device_id: String,
    /// Metric name (e.g., "intensity", "latency", "`error_rate`").
    pub metric_name: String,
    /// Metric value.
    pub value: f64,
    /// Timestamp (epoch milliseconds).
    pub timestamp_ms: u64,
    /// Actuator type tag.
    pub actuator_type: ActuatorType,
    /// Additional tags for bucketing.
    pub tags: Vec<(String, String)>,
}

/// Convert a device intensity sample to a metric event.
#[inline]
#[must_use]
pub fn intensity_to_metric(
    device_id: &str,
    intensity: f64,
    actuator_type: ActuatorType,
    timestamp_ms: u64,
) -> DeviceMetricEvent {
    DeviceMetricEvent {
        device_id: device_id.to_string(),
        metric_name: "device.intensity".to_string(),
        value: intensity,
        timestamp_ms,
        actuator_type,
        tags: vec![
            ("type".to_string(), actuator_type.as_str().to_string()),
            (
                "safety_critical".to_string(),
                actuator_type.is_safety_critical().to_string(),
            ),
        ],
    }
}

/// Device health summary for analytics aggregation.
#[derive(Debug, Clone, Copy)]
pub struct DeviceHealthSummary {
    /// Total devices registered.
    pub total_devices: u32,
    /// Devices currently active.
    pub active_devices: u32,
    /// Devices with safety limiter engaged.
    pub safety_engaged_count: u32,
    /// Average intensity across active devices.
    pub avg_intensity: f64,
    /// Maximum intensity across active devices.
    pub max_intensity: f64,
    /// Total error count across all devices.
    pub total_errors: u32,
}

/// Compute health score [0.0, 1.0] from device health summary.
///
/// 1.0 = all devices healthy, 0.0 = all devices in error state.
#[inline]
#[must_use]
pub fn health_score(summary: &DeviceHealthSummary) -> f64 {
    if summary.total_devices == 0 {
        return 1.0; // デバイスなし → 問題なし
    }
    let active_ratio = f64::from(summary.active_devices) / f64::from(summary.total_devices);
    let safety_penalty =
        f64::from(summary.safety_engaged_count) / f64::from(summary.total_devices) * 0.3;
    let error_penalty = (f64::from(summary.total_errors) / 100.0).clamp(0.0, 0.3);
    (active_ratio - safety_penalty - error_penalty).clamp(0.0, 1.0)
}

/// Device latency metric for protocol performance tracking.
#[derive(Debug, Clone, Copy)]
pub struct DeviceLatencyMetric {
    /// Round-trip latency (milliseconds).
    pub rtt_ms: f64,
    /// Command-to-ack latency (milliseconds).
    pub cmd_latency_ms: f64,
    /// Protocol type (0=Buttplug, 1=MQTT, 2=REST, 3=OSC, 4=WebSocket).
    pub protocol_type: u8,
}

/// Convert latency metric to analytics event.
#[inline]
#[must_use]
pub fn latency_to_metric(
    device_id: &str,
    latency: &DeviceLatencyMetric,
    timestamp_ms: u64,
) -> DeviceMetricEvent {
    let protocol_name = match latency.protocol_type {
        0 => "buttplug",
        1 => "mqtt",
        2 => "rest",
        3 => "osc",
        4 => "websocket",
        _ => "unknown",
    };
    DeviceMetricEvent {
        device_id: device_id.to_string(),
        metric_name: "device.latency_ms".to_string(),
        value: latency.rtt_ms,
        timestamp_ms,
        actuator_type: ActuatorType::Custom,
        tags: vec![
            ("protocol".to_string(), protocol_name.to_string()),
            (
                "cmd_latency_ms".to_string(),
                format!("{:.1}", latency.cmd_latency_ms),
            ),
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn intensity_metric_fields() {
        let event = intensity_to_metric("dev:0", 0.75, ActuatorType::Vibrate, 1000);
        assert_eq!(event.device_id, "dev:0");
        assert_eq!(event.metric_name, "device.intensity");
        assert!((event.value - 0.75).abs() < 1e-6);
        assert_eq!(event.tags.len(), 2);
    }

    #[test]
    fn intensity_metric_safety_critical_tag() {
        let event = intensity_to_metric("dev:0", 0.5, ActuatorType::Heat, 1000);
        assert_eq!(event.tags[1].1, "true");
    }

    #[test]
    fn health_score_no_devices() {
        let summary = DeviceHealthSummary {
            total_devices: 0,
            active_devices: 0,
            safety_engaged_count: 0,
            avg_intensity: 0.0,
            max_intensity: 0.0,
            total_errors: 0,
        };
        assert!((health_score(&summary) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn health_score_all_active() {
        let summary = DeviceHealthSummary {
            total_devices: 10,
            active_devices: 10,
            safety_engaged_count: 0,
            avg_intensity: 0.5,
            max_intensity: 0.8,
            total_errors: 0,
        };
        assert!((health_score(&summary) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn health_score_with_safety() {
        let summary = DeviceHealthSummary {
            total_devices: 10,
            active_devices: 10,
            safety_engaged_count: 5,
            avg_intensity: 0.5,
            max_intensity: 0.8,
            total_errors: 0,
        };
        let score = health_score(&summary);
        assert!(score < 1.0);
        assert!(score > 0.5);
    }

    #[test]
    fn health_score_with_errors() {
        let summary = DeviceHealthSummary {
            total_devices: 10,
            active_devices: 10,
            safety_engaged_count: 0,
            avg_intensity: 0.5,
            max_intensity: 0.8,
            total_errors: 100,
        };
        let score = health_score(&summary);
        assert!((score - 0.7).abs() < 1e-6);
    }

    #[test]
    fn latency_metric_fields() {
        let latency = DeviceLatencyMetric {
            rtt_ms: 15.5,
            cmd_latency_ms: 8.2,
            protocol_type: 1, // MQTT
        };
        let event = latency_to_metric("dev:0", &latency, 2000);
        assert_eq!(event.metric_name, "device.latency_ms");
        assert!((event.value - 15.5).abs() < 1e-6);
        assert_eq!(event.tags[0].1, "mqtt");
    }

    #[test]
    fn latency_unknown_protocol() {
        let latency = DeviceLatencyMetric {
            rtt_ms: 10.0,
            cmd_latency_ms: 5.0,
            protocol_type: 255,
        };
        let event = latency_to_metric("dev:0", &latency, 0);
        assert_eq!(event.tags[0].1, "unknown");
    }

    #[test]
    fn health_score_clamped() {
        let summary = DeviceHealthSummary {
            total_devices: 10,
            active_devices: 0,
            safety_engaged_count: 10,
            avg_intensity: 0.0,
            max_intensity: 0.0,
            total_errors: 500,
        };
        let score = health_score(&summary);
        assert!(score >= 0.0);
    }
}
