//! Device manager — registry and dispatch for connected devices.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use tracing::info;

use super::actuator::{Actuator, ActuatorType};

/// Unique device identifier (protocol-prefixed, e.g., "buttplug:3", "mqtt:servo1").
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DeviceId(pub String);

impl std::fmt::Display for DeviceId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// A discovered or configured device with its capabilities.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Device {
    pub id: DeviceId,
    pub name: String,
    /// Protocol that manages this device.
    pub protocol: String,
    /// Available actuators.
    pub actuators: Vec<Actuator>,
    /// Protocol-specific metadata.
    pub metadata: HashMap<String, String>,
}

impl Device {
    #[must_use]
    pub fn has_type(&self, atype: ActuatorType) -> bool {
        self.actuators.iter().any(|a| a.actuator_type == atype)
    }

    #[must_use]
    pub fn has_linear(&self) -> bool {
        self.has_type(ActuatorType::Linear)
    }

    #[must_use]
    pub fn has_vibration(&self) -> bool {
        self.has_type(ActuatorType::Vibrate)
    }

    #[must_use]
    pub fn has_heat(&self) -> bool {
        self.has_type(ActuatorType::Heat)
    }

    #[must_use]
    pub fn has_electrostim(&self) -> bool {
        self.has_type(ActuatorType::Electrostimulate)
    }

    #[must_use]
    pub fn supported_types(&self) -> Vec<ActuatorType> {
        let mut types: Vec<ActuatorType> = self
            .actuators
            .iter()
            .map(|a| a.actuator_type)
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();
        types.sort_by_key(|t| t.as_str().to_string());
        types
    }
}

/// Central device registry — tracks all connected devices across protocols.
pub struct DeviceManager {
    devices: HashMap<String, Device>,
}

impl DeviceManager {
    #[must_use]
    pub fn new() -> Self {
        Self {
            devices: HashMap::new(),
        }
    }

    /// Register a discovered device.
    pub fn register(&mut self, device: Device) {
        info!(
            id = %device.id.0,
            name = device.name,
            protocol = device.protocol,
            actuators = device.actuators.len(),
            "Device registered"
        );
        self.devices.insert(device.id.0.clone(), device);
    }

    /// Remove a device.
    pub fn unregister(&mut self, device_id: &str) -> Option<Device> {
        let removed = self.devices.remove(device_id);
        if let Some(ref dev) = removed {
            info!(id = device_id, name = dev.name, "Device unregistered");
        }
        removed
    }

    /// Get a device by ID.
    #[must_use]
    pub fn get(&self, device_id: &str) -> Option<&Device> {
        self.devices.get(device_id)
    }

    /// List all registered devices.
    #[must_use]
    pub fn list(&self) -> Vec<&Device> {
        self.devices.values().collect()
    }

    /// Find devices by actuator type.
    #[must_use]
    pub fn find_by_type(&self, atype: ActuatorType) -> Vec<&Device> {
        self.devices
            .values()
            .filter(|d| d.has_type(atype))
            .collect()
    }

    /// Find devices by protocol.
    #[must_use]
    pub fn find_by_protocol(&self, protocol: &str) -> Vec<&Device> {
        self.devices
            .values()
            .filter(|d| d.protocol == protocol)
            .collect()
    }

    /// Total number of registered devices.
    #[must_use]
    pub fn count(&self) -> usize {
        self.devices.len()
    }

    /// Clear all devices.
    pub fn clear(&mut self) {
        self.devices.clear();
    }
}

impl Default for DeviceManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_device(id: &str, name: &str, protocol: &str, types: &[ActuatorType]) -> Device {
        Device {
            id: DeviceId(id.into()),
            name: name.into(),
            protocol: protocol.into(),
            actuators: types
                .iter()
                .enumerate()
                .map(|(i, t)| Actuator {
                    #[allow(clippy::cast_possible_truncation)]
                    index: i as u32,
                    actuator_type: *t,
                    description: t.as_str().into(),
                    step_count: 100,
                })
                .collect(),
            metadata: HashMap::new(),
        }
    }

    #[test]
    fn register_and_find() {
        let mut mgr = DeviceManager::new();
        mgr.register(test_device(
            "bp:0",
            "Handy",
            "buttplug",
            &[ActuatorType::Linear],
        ));
        mgr.register(test_device(
            "bp:1",
            "Lovense",
            "buttplug",
            &[ActuatorType::Vibrate],
        ));
        mgr.register(test_device(
            "mqtt:0",
            "ESP32",
            "mqtt",
            &[ActuatorType::Vibrate, ActuatorType::Heat],
        ));

        assert_eq!(mgr.count(), 3);
        assert_eq!(mgr.find_by_type(ActuatorType::Vibrate).len(), 2);
        assert_eq!(mgr.find_by_type(ActuatorType::Linear).len(), 1);
        assert_eq!(mgr.find_by_type(ActuatorType::Heat).len(), 1);
        assert_eq!(mgr.find_by_protocol("buttplug").len(), 2);
        assert_eq!(mgr.find_by_protocol("mqtt").len(), 1);
    }

    #[test]
    fn unregister() {
        let mut mgr = DeviceManager::new();
        mgr.register(test_device(
            "bp:0",
            "Dev",
            "buttplug",
            &[ActuatorType::Vibrate],
        ));
        assert_eq!(mgr.count(), 1);
        mgr.unregister("bp:0");
        assert_eq!(mgr.count(), 0);
    }

    #[test]
    fn device_capabilities() {
        let dev = test_device(
            "test:0",
            "Multi",
            "test",
            &[
                ActuatorType::Vibrate,
                ActuatorType::Heat,
                ActuatorType::Linear,
            ],
        );
        assert!(dev.has_vibration());
        assert!(dev.has_heat());
        assert!(dev.has_linear());
        assert!(!dev.has_electrostim());
        assert_eq!(dev.supported_types().len(), 3);
    }

    #[test]
    fn device_id_display() {
        let id = DeviceId("buttplug:42".into());
        assert_eq!(format!("{id}"), "buttplug:42");
    }

    #[test]
    fn device_manager_default() {
        let mgr = DeviceManager::default();
        assert_eq!(mgr.count(), 0);
        assert!(mgr.list().is_empty());
    }

    #[test]
    fn clear_removes_all() {
        let mut mgr = DeviceManager::new();
        mgr.register(test_device("a:0", "A", "a", &[ActuatorType::Vibrate]));
        mgr.register(test_device("b:0", "B", "b", &[ActuatorType::Heat]));
        assert_eq!(mgr.count(), 2);
        mgr.clear();
        assert_eq!(mgr.count(), 0);
    }

    #[test]
    fn get_nonexistent() {
        let mgr = DeviceManager::new();
        assert!(mgr.get("nonexistent").is_none());
    }

    #[test]
    fn unregister_nonexistent_returns_none() {
        let mut mgr = DeviceManager::new();
        assert!(mgr.unregister("nope").is_none());
    }

    #[test]
    fn find_by_type_empty() {
        let mgr = DeviceManager::new();
        assert!(mgr.find_by_type(ActuatorType::Vibrate).is_empty());
    }

    #[test]
    fn find_by_protocol_empty() {
        let mgr = DeviceManager::new();
        assert!(mgr.find_by_protocol("mqtt").is_empty());
    }

    #[test]
    fn supported_types_sorted() {
        let dev = test_device(
            "test:0",
            "Multi",
            "test",
            &[
                ActuatorType::Vibrate,
                ActuatorType::Heat,
                ActuatorType::Linear,
            ],
        );
        let types = dev.supported_types();
        let strs: Vec<&str> = types.iter().map(ActuatorType::as_str).collect();
        let mut sorted = strs.clone();
        sorted.sort_unstable();
        assert_eq!(strs, sorted);
    }

    #[test]
    fn device_no_actuators() {
        let dev = test_device("test:0", "Empty", "test", &[]);
        assert!(!dev.has_vibration());
        assert!(!dev.has_heat());
        assert!(!dev.has_linear());
        assert!(!dev.has_electrostim());
        assert!(dev.supported_types().is_empty());
    }

    #[test]
    fn register_overwrites_same_id() {
        let mut mgr = DeviceManager::new();
        mgr.register(test_device(
            "bp:0",
            "Old",
            "buttplug",
            &[ActuatorType::Vibrate],
        ));
        mgr.register(test_device(
            "bp:0",
            "New",
            "buttplug",
            &[ActuatorType::Heat],
        ));
        assert_eq!(mgr.count(), 1);
        assert_eq!(mgr.get("bp:0").unwrap().name, "New");
    }
}
