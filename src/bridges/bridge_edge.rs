//! Edge bridge — ALICE-Edge ↔ ALICE-Bridge
//!
//! Connects `IoT` edge nodes with the device management layer.
//! Edge sensor data → device triggers, device state → edge telemetry.

use crate::device::{ActuatorType, DeviceMapping};

/// Edge sensor reading for device control.
#[derive(Debug, Clone, Copy)]
pub struct EdgeSensorReading {
    /// Sensor ID.
    pub sensor_id: u32,
    /// Sensor value (raw, domain-specific).
    pub value: f64,
    /// Sensor value normalized to [0.0, 1.0].
    pub normalized: f64,
    /// Timestamp (epoch milliseconds).
    pub timestamp_ms: u64,
    /// Sensor type (0=temperature, 1=pressure, 2=humidity, 3=motion, 4=light).
    pub sensor_type: u8,
}

/// Sensor type constants.
pub const SENSOR_TEMPERATURE: u8 = 0;
pub const SENSOR_PRESSURE: u8 = 1;
pub const SENSOR_HUMIDITY: u8 = 2;
pub const SENSOR_MOTION: u8 = 3;
pub const SENSOR_LIGHT: u8 = 4;

/// Convert edge sensor reading to device intensity.
///
/// Motion sensors map directly; temperature/humidity map via threshold.
#[inline]
#[must_use]
pub fn sensor_to_device_intensity(reading: &EdgeSensorReading) -> f64 {
    match reading.sensor_type {
        SENSOR_MOTION | SENSOR_LIGHT => reading.normalized.clamp(0.0, 1.0),
        SENSOR_TEMPERATURE | SENSOR_HUMIDITY | SENSOR_PRESSURE => {
            // 閾値ベース: 0.5を中心に偏差を強度に変換
            ((reading.normalized - 0.5).abs() * 2.0).clamp(0.0, 1.0)
        }
        _ => 0.0,
    }
}

/// Recommended actuator type for sensor readings.
#[inline]
#[must_use]
pub const fn sensor_actuator_type(reading: &EdgeSensorReading) -> ActuatorType {
    match reading.sensor_type {
        SENSOR_TEMPERATURE => ActuatorType::Heat,
        SENSOR_MOTION => ActuatorType::Vibrate,
        _ => ActuatorType::Custom,
    }
}

/// Edge device registration descriptor.
#[derive(Debug, Clone)]
pub struct EdgeDeviceDescriptor {
    /// Edge node ID.
    pub node_id: String,
    /// Protocol hint ("mqtt", "rest", "websocket").
    pub protocol_hint: String,
    /// Endpoint address.
    pub endpoint: String,
    /// Supported actuator types.
    pub actuator_types: Vec<ActuatorType>,
    /// Maximum update rate (Hz).
    pub max_update_rate_hz: u32,
}

/// Create a `DeviceMapping` from an edge device descriptor.
#[inline]
#[must_use]
pub fn edge_device_to_mapping(desc: &EdgeDeviceDescriptor) -> DeviceMapping {
    DeviceMapping {
        device_id: desc.node_id.clone(),
        label: format!("edge_{}", desc.node_id),
        scale: 1.0,
        offset: 0.0,
        invert: false,
        delay_ms: 0,
        source_filter: desc.protocol_hint.clone(),
        group: "edge".into(),
    }
}

/// Device telemetry report for edge uplink.
#[derive(Debug, Clone, Copy)]
pub struct DeviceTelemetryReport {
    /// Current device intensity [0.0, 1.0].
    pub intensity: f64,
    /// Device active (true) or idle (false).
    pub active: bool,
    /// Safety limiter engaged.
    pub safety_engaged: bool,
    /// Uptime (seconds).
    pub uptime_secs: f64,
    /// Error count since last report.
    pub error_count: u32,
}

/// Convert device state to edge telemetry value (normalized).
#[inline]
#[must_use]
pub const fn device_state_to_edge_value(report: &DeviceTelemetryReport) -> f64 {
    if !report.active {
        return 0.0;
    }
    if report.safety_engaged {
        return -1.0; // 負値でアラート
    }
    report.intensity
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn motion_sensor_passthrough() {
        let reading = EdgeSensorReading {
            sensor_id: 0,
            value: 100.0,
            normalized: 0.7,
            timestamp_ms: 1000,
            sensor_type: SENSOR_MOTION,
        };
        assert!((sensor_to_device_intensity(&reading) - 0.7).abs() < 1e-6);
    }

    #[test]
    fn temperature_sensor_threshold() {
        let reading = EdgeSensorReading {
            sensor_id: 1,
            value: 30.0,
            normalized: 0.8, // 偏差 0.3 → 強度 0.6
            timestamp_ms: 1000,
            sensor_type: SENSOR_TEMPERATURE,
        };
        assert!((sensor_to_device_intensity(&reading) - 0.6).abs() < 1e-6);
    }

    #[test]
    fn temperature_sensor_center() {
        let reading = EdgeSensorReading {
            sensor_id: 1,
            value: 25.0,
            normalized: 0.5, // 偏差 0.0 → 強度 0.0
            timestamp_ms: 1000,
            sensor_type: SENSOR_TEMPERATURE,
        };
        assert!((sensor_to_device_intensity(&reading) - 0.0).abs() < 1e-6);
    }

    #[test]
    fn unknown_sensor_zero() {
        let reading = EdgeSensorReading {
            sensor_id: 99,
            value: 100.0,
            normalized: 1.0,
            timestamp_ms: 0,
            sensor_type: 255,
        };
        assert!((sensor_to_device_intensity(&reading) - 0.0).abs() < 1e-6);
    }

    #[test]
    fn sensor_actuator_temp_heat() {
        let reading = EdgeSensorReading {
            sensor_id: 0,
            value: 0.0,
            normalized: 0.5,
            timestamp_ms: 0,
            sensor_type: SENSOR_TEMPERATURE,
        };
        assert_eq!(sensor_actuator_type(&reading), ActuatorType::Heat);
    }

    #[test]
    fn sensor_actuator_motion_vibrate() {
        let reading = EdgeSensorReading {
            sensor_id: 0,
            value: 0.0,
            normalized: 0.5,
            timestamp_ms: 0,
            sensor_type: SENSOR_MOTION,
        };
        assert_eq!(sensor_actuator_type(&reading), ActuatorType::Vibrate);
    }

    #[test]
    fn edge_device_mapping_fields() {
        let desc = EdgeDeviceDescriptor {
            node_id: "esp32-001".into(),
            protocol_hint: "mqtt".into(),
            endpoint: "192.168.1.100:1883".into(),
            actuator_types: vec![ActuatorType::Vibrate],
            max_update_rate_hz: 50,
        };
        let mapping = edge_device_to_mapping(&desc);
        assert_eq!(mapping.device_id, "esp32-001");
        assert_eq!(mapping.source_filter, "mqtt");
        assert_eq!(mapping.group, "edge");
    }

    #[test]
    fn device_telemetry_active() {
        let report = DeviceTelemetryReport {
            intensity: 0.6,
            active: true,
            safety_engaged: false,
            uptime_secs: 100.0,
            error_count: 0,
        };
        assert!((device_state_to_edge_value(&report) - 0.6).abs() < 1e-6);
    }

    #[test]
    fn device_telemetry_inactive() {
        let report = DeviceTelemetryReport {
            intensity: 0.6,
            active: false,
            safety_engaged: false,
            uptime_secs: 100.0,
            error_count: 0,
        };
        assert!((device_state_to_edge_value(&report) - 0.0).abs() < 1e-6);
    }

    #[test]
    fn device_telemetry_safety_alert() {
        let report = DeviceTelemetryReport {
            intensity: 0.8,
            active: true,
            safety_engaged: true,
            uptime_secs: 50.0,
            error_count: 1,
        };
        assert!((device_state_to_edge_value(&report) - (-1.0)).abs() < 1e-6);
    }
}
