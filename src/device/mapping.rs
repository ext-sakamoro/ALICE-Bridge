//! Device mapping — per-device intensity scaling, inversion, and delay.

use serde::{Deserialize, Serialize};

/// Configuration for a single device in a multi-device setup.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceMapping {
    /// Target device ID.
    pub device_id: String,
    /// Human-readable label.
    pub label: String,
    /// Intensity multiplier [0.0, 2.0].
    pub scale: f64,
    /// Baseline intensity offset [0.0, 1.0].
    pub offset: f64,
    /// Invert intensity (1.0 - value).
    pub invert: bool,
    /// Delay offset in milliseconds.
    pub delay_ms: u32,
    /// Input source filter ("all", "osc", "capture", "pattern", etc.)
    pub source_filter: String,
    /// Device group for group control.
    pub group: String,
}

impl Default for DeviceMapping {
    fn default() -> Self {
        Self {
            device_id: String::new(),
            label: String::new(),
            scale: 1.0,
            offset: 0.0,
            invert: false,
            delay_ms: 0,
            source_filter: "all".into(),
            group: "default".into(),
        }
    }
}

/// Computed command for a single device after mapping.
#[derive(Debug, Clone)]
pub struct MappedCommand {
    pub device_id: String,
    pub label: String,
    pub position: f64,
    pub group: String,
}

impl DeviceMapping {
    /// Apply mapping to a raw intensity value.
    #[must_use]
    pub fn apply(&self, intensity: f64, source: &str) -> Option<MappedCommand> {
        // Source filter
        if self.source_filter != "all" && self.source_filter != source {
            return None;
        }

        let mut value = intensity.mul_add(self.scale, self.offset);
        if self.invert {
            value = 1.0 - value;
        }
        value = value.clamp(0.0, 1.0);

        Some(MappedCommand {
            device_id: self.device_id.clone(),
            label: self.label.clone(),
            position: (value * 10000.0).round() / 10000.0,
            group: self.group.clone(),
        })
    }
}

/// Multi-device mapper — applies per-device mappings and supports delay buffers.
pub struct MultiMapper {
    mappings: Vec<DeviceMapping>,
    delay_buffers: Vec<Vec<(u64, f64)>>,
}

impl MultiMapper {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            mappings: Vec::new(),
            delay_buffers: Vec::new(),
        }
    }

    pub fn add(&mut self, mapping: DeviceMapping) {
        self.delay_buffers.push(Vec::new());
        self.mappings.push(mapping);
    }

    pub fn remove(&mut self, device_id: &str) {
        if let Some(idx) = self.mappings.iter().position(|m| m.device_id == device_id) {
            self.mappings.remove(idx);
            self.delay_buffers.remove(idx);
        }
    }

    /// Compute per-device commands from a single intensity.
    pub fn compute(
        &mut self,
        intensity: f64,
        source: &str,
        timestamp_ms: u64,
    ) -> Vec<MappedCommand> {
        let mut commands = Vec::with_capacity(self.mappings.len());

        for (i, mapping) in self.mappings.iter().enumerate() {
            let Some(mut cmd) = mapping.apply(intensity, source) else {
                continue;
            };

            // Delay handling
            if mapping.delay_ms > 0 && timestamp_ms > 0 {
                let buf = &mut self.delay_buffers[i];
                buf.push((timestamp_ms, cmd.position));
                let target_time = timestamp_ms.saturating_sub(u64::from(mapping.delay_ms));
                let mut delayed_value = 0.0;
                while let Some(&(t, v)) = buf.first() {
                    if t <= target_time {
                        delayed_value = v;
                        buf.remove(0);
                    } else {
                        break;
                    }
                }
                cmd.position = delayed_value;
            }

            commands.push(cmd);
        }

        commands
    }

    /// Compute for a specific group only.
    pub fn compute_group(
        &mut self,
        group: &str,
        intensity: f64,
        source: &str,
        timestamp_ms: u64,
    ) -> Vec<MappedCommand> {
        self.compute(intensity, source, timestamp_ms)
            .into_iter()
            .filter(|c| c.group == group)
            .collect()
    }

    #[must_use]
    pub const fn count(&self) -> usize {
        self.mappings.len()
    }

    #[must_use]
    pub fn groups(&self) -> Vec<String> {
        let mut groups: Vec<String> = self
            .mappings
            .iter()
            .map(|m| m.group.clone())
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();
        groups.sort();
        groups
    }
}

impl Default for MultiMapper {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_mapping() {
        let m = DeviceMapping {
            device_id: "dev:0".into(),
            scale: 0.5,
            offset: 0.1,
            ..Default::default()
        };
        let cmd = m.apply(0.8, "all").unwrap();
        // 0.8 * 0.5 + 0.1 = 0.5
        assert!((cmd.position - 0.5).abs() < 1e-4);
    }

    #[test]
    fn invert_mapping() {
        let m = DeviceMapping {
            device_id: "dev:0".into(),
            invert: true,
            ..Default::default()
        };
        let cmd = m.apply(0.3, "all").unwrap();
        assert!((cmd.position - 0.7).abs() < 1e-4);
    }

    #[test]
    fn source_filter() {
        let m = DeviceMapping {
            device_id: "dev:0".into(),
            source_filter: "osc".into(),
            ..Default::default()
        };
        assert!(m.apply(0.5, "osc").is_some());
        assert!(m.apply(0.5, "capture").is_none());
        assert!(m.apply(0.5, "all").is_none());
    }

    #[test]
    fn multi_mapper() {
        let mut mm = MultiMapper::new();
        mm.add(DeviceMapping {
            device_id: "dev:0".into(),
            label: "Primary".into(),
            ..Default::default()
        });
        mm.add(DeviceMapping {
            device_id: "dev:1".into(),
            label: "Secondary".into(),
            scale: 0.5,
            ..Default::default()
        });

        let cmds = mm.compute(0.8, "all", 0);
        assert_eq!(cmds.len(), 2);
        assert!((cmds[0].position - 0.8).abs() < 1e-4);
        assert!((cmds[1].position - 0.4).abs() < 1e-4);
    }

    #[test]
    fn clamp_overflow() {
        let m = DeviceMapping {
            device_id: "dev:0".into(),
            scale: 2.0,
            offset: 0.5,
            ..Default::default()
        };
        let cmd = m.apply(1.0, "all").unwrap();
        // 1.0 * 2.0 + 0.5 = 2.5 -> clamped to 1.0
        assert!((cmd.position - 1.0).abs() < 1e-4);
    }

    #[test]
    fn default_mapping_passthrough() {
        let m = DeviceMapping {
            device_id: "dev:0".into(),
            ..Default::default()
        };
        let cmd = m.apply(0.42, "all").unwrap();
        assert!((cmd.position - 0.42).abs() < 1e-4);
    }

    #[test]
    fn zero_scale() {
        let m = DeviceMapping {
            device_id: "dev:0".into(),
            scale: 0.0,
            ..Default::default()
        };
        let cmd = m.apply(0.8, "all").unwrap();
        assert!((cmd.position - 0.0).abs() < 1e-4);
    }

    #[test]
    fn negative_intensity_clamped() {
        let m = DeviceMapping {
            device_id: "dev:0".into(),
            ..Default::default()
        };
        let cmd = m.apply(-0.5, "all").unwrap();
        assert!((cmd.position - 0.0).abs() < 1e-4);
    }

    #[test]
    fn invert_plus_scale() {
        let m = DeviceMapping {
            device_id: "dev:0".into(),
            scale: 0.5,
            invert: true,
            ..Default::default()
        };
        // 0.6 * 0.5 = 0.3, inverted = 0.7
        let cmd = m.apply(0.6, "all").unwrap();
        assert!((cmd.position - 0.7).abs() < 1e-4);
    }

    #[test]
    fn source_filter_all_passthrough() {
        let m = DeviceMapping {
            device_id: "dev:0".into(),
            source_filter: "all".into(),
            ..Default::default()
        };
        assert!(m.apply(0.5, "osc").is_some());
        assert!(m.apply(0.5, "capture").is_some());
        assert!(m.apply(0.5, "anything").is_some());
    }

    #[test]
    fn mapped_command_fields() {
        let m = DeviceMapping {
            device_id: "dev:42".into(),
            label: "MyDevice".into(),
            group: "group_a".into(),
            ..Default::default()
        };
        let cmd = m.apply(0.5, "all").unwrap();
        assert_eq!(cmd.device_id, "dev:42");
        assert_eq!(cmd.label, "MyDevice");
        assert_eq!(cmd.group, "group_a");
    }

    #[test]
    fn multi_mapper_remove() {
        let mut mm = MultiMapper::new();
        mm.add(DeviceMapping {
            device_id: "dev:0".into(),
            ..Default::default()
        });
        mm.add(DeviceMapping {
            device_id: "dev:1".into(),
            ..Default::default()
        });
        assert_eq!(mm.count(), 2);
        mm.remove("dev:0");
        assert_eq!(mm.count(), 1);
        let cmds = mm.compute(0.5, "all", 0);
        assert_eq!(cmds.len(), 1);
        assert_eq!(cmds[0].device_id, "dev:1");
    }

    #[test]
    fn multi_mapper_remove_nonexistent() {
        let mut mm = MultiMapper::new();
        mm.add(DeviceMapping {
            device_id: "dev:0".into(),
            ..Default::default()
        });
        mm.remove("nonexistent");
        assert_eq!(mm.count(), 1);
    }

    #[test]
    fn compute_group_filters() {
        let mut mm = MultiMapper::new();
        mm.add(DeviceMapping {
            device_id: "dev:0".into(),
            group: "A".into(),
            ..Default::default()
        });
        mm.add(DeviceMapping {
            device_id: "dev:1".into(),
            group: "B".into(),
            ..Default::default()
        });
        let cmds = mm.compute_group("A", 0.5, "all", 0);
        assert_eq!(cmds.len(), 1);
        assert_eq!(cmds[0].device_id, "dev:0");
    }

    #[test]
    fn groups_returns_unique_sorted() {
        let mut mm = MultiMapper::new();
        mm.add(DeviceMapping {
            device_id: "dev:0".into(),
            group: "B".into(),
            ..Default::default()
        });
        mm.add(DeviceMapping {
            device_id: "dev:1".into(),
            group: "A".into(),
            ..Default::default()
        });
        mm.add(DeviceMapping {
            device_id: "dev:2".into(),
            group: "B".into(),
            ..Default::default()
        });
        let groups = mm.groups();
        assert_eq!(groups, vec!["A", "B"]);
    }

    #[test]
    fn multi_mapper_default() {
        let mm = MultiMapper::default();
        assert_eq!(mm.count(), 0);
        assert!(mm.groups().is_empty());
    }

    #[test]
    fn offset_only() {
        let m = DeviceMapping {
            device_id: "dev:0".into(),
            scale: 0.0,
            offset: 0.3,
            ..Default::default()
        };
        let cmd = m.apply(0.0, "all").unwrap();
        assert!((cmd.position - 0.3).abs() < 1e-4);
    }
}
