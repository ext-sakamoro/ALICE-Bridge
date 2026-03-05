//! Sync bridge — ALICE-Sync ↔ ALICE-Bridge
//!
//! Synchronizes device commands across networked peers.
//! Maps input frames to multi-device control for multiplayer haptics.

use crate::bridge::BridgeAction;
use crate::device::DeviceMapping;

/// Synchronized device command frame.
///
/// Represents a single device state at a specific network tick,
/// suitable for lockstep or rollback synchronization.
#[derive(Debug, Clone)]
pub struct SyncDeviceFrame {
    /// Network tick number.
    pub tick: u64,
    /// Sender peer ID.
    pub peer_id: u32,
    /// Target device ID.
    pub device_id: String,
    /// Device intensity [0.0, 1.0].
    pub intensity: f64,
    /// Device position [0.0, 1.0].
    pub position: f64,
    /// Timestamp (monotonic seconds).
    pub timestamp: f64,
}

/// Convert a `BridgeAction` into a sync-ready frame.
#[inline]
#[must_use]
pub fn bridge_action_to_sync_frame(
    action: &BridgeAction,
    tick: u64,
    peer_id: u32,
    device_id: &str,
) -> SyncDeviceFrame {
    SyncDeviceFrame {
        tick,
        peer_id,
        device_id: device_id.to_string(),
        intensity: action.position,
        position: action.position,
        timestamp: action.timestamp,
    }
}

/// Convert a sync frame back into a `BridgeAction`.
#[inline]
#[must_use]
pub const fn sync_frame_to_bridge_action(frame: &SyncDeviceFrame) -> BridgeAction {
    BridgeAction {
        position: frame.position,
        duration_ms: 50, // デフォルト出力間隔
        timestamp: frame.timestamp,
    }
}

/// Multi-peer device sync state.
///
/// Aggregates device state from multiple peers for consensus.
#[derive(Debug, Clone)]
pub struct MultiPeerDeviceState {
    /// Latest frame from each peer (`peer_id` → frame).
    pub peer_frames: Vec<SyncDeviceFrame>,
    /// Consensus intensity (average of all peers).
    pub consensus_intensity: f64,
}

/// Compute consensus intensity from multiple peer frames.
#[inline]
#[must_use]
pub fn compute_peer_consensus(frames: &[SyncDeviceFrame]) -> f64 {
    if frames.is_empty() {
        return 0.0;
    }
    let sum: f64 = frames.iter().map(|f| f.intensity).sum();
    #[allow(clippy::cast_precision_loss)]
    let count = frames.len() as f64;
    (sum / count).clamp(0.0, 1.0)
}

/// Create device mappings for synchronized multi-peer control.
#[inline]
#[must_use]
pub fn sync_to_device_mappings(frames: &[SyncDeviceFrame], group: &str) -> Vec<DeviceMapping> {
    frames
        .iter()
        .map(|f| DeviceMapping {
            device_id: f.device_id.clone(),
            label: format!("sync_peer_{}", f.peer_id),
            scale: f.intensity,
            offset: 0.0,
            invert: false,
            delay_ms: 0,
            source_filter: "sync".into(),
            group: group.to_string(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bridge_action_to_sync_roundtrip() {
        let action = BridgeAction {
            position: 0.7,
            duration_ms: 50,
            timestamp: 1.5,
        };
        let frame = bridge_action_to_sync_frame(&action, 42, 1, "dev:0");
        assert_eq!(frame.tick, 42);
        assert_eq!(frame.peer_id, 1);
        assert!((frame.intensity - 0.7).abs() < 1e-6);

        let back = sync_frame_to_bridge_action(&frame);
        assert!((back.position - 0.7).abs() < 1e-6);
    }

    #[test]
    fn consensus_empty() {
        assert!((compute_peer_consensus(&[]) - 0.0).abs() < 1e-6);
    }

    #[test]
    fn consensus_single_peer() {
        let frames = vec![SyncDeviceFrame {
            tick: 1,
            peer_id: 0,
            device_id: "dev:0".into(),
            intensity: 0.5,
            position: 0.5,
            timestamp: 1.0,
        }];
        assert!((compute_peer_consensus(&frames) - 0.5).abs() < 1e-6);
    }

    #[test]
    fn consensus_multi_peer() {
        let frames = vec![
            SyncDeviceFrame {
                tick: 1,
                peer_id: 0,
                device_id: "dev:0".into(),
                intensity: 0.2,
                position: 0.2,
                timestamp: 1.0,
            },
            SyncDeviceFrame {
                tick: 1,
                peer_id: 1,
                device_id: "dev:0".into(),
                intensity: 0.8,
                position: 0.8,
                timestamp: 1.0,
            },
        ];
        assert!((compute_peer_consensus(&frames) - 0.5).abs() < 1e-6);
    }

    #[test]
    fn sync_to_mappings_count() {
        let frames = vec![
            SyncDeviceFrame {
                tick: 1,
                peer_id: 0,
                device_id: "dev:0".into(),
                intensity: 0.5,
                position: 0.5,
                timestamp: 1.0,
            },
            SyncDeviceFrame {
                tick: 1,
                peer_id: 1,
                device_id: "dev:1".into(),
                intensity: 0.3,
                position: 0.3,
                timestamp: 1.0,
            },
        ];
        let mappings = sync_to_device_mappings(&frames, "haptic");
        assert_eq!(mappings.len(), 2);
        assert_eq!(mappings[0].group, "haptic");
        assert_eq!(mappings[1].source_filter, "sync");
    }

    #[test]
    fn sync_frame_fields() {
        let frame = SyncDeviceFrame {
            tick: 100,
            peer_id: 42,
            device_id: "dev:99".into(),
            intensity: 0.9,
            position: 0.9,
            timestamp: 2.5,
        };
        assert_eq!(frame.tick, 100);
        assert_eq!(frame.peer_id, 42);
        assert_eq!(frame.device_id, "dev:99");
    }

    #[test]
    fn bridge_action_timestamp_preserved() {
        let action = BridgeAction {
            position: 0.3,
            duration_ms: 100,
            timestamp: 99.5,
        };
        let frame = bridge_action_to_sync_frame(&action, 0, 0, "dev:0");
        assert!((frame.timestamp - 99.5).abs() < 1e-6);
    }

    #[test]
    fn consensus_clamped() {
        let frames = vec![SyncDeviceFrame {
            tick: 1,
            peer_id: 0,
            device_id: "dev:0".into(),
            intensity: 1.5, // 範囲外
            position: 1.5,
            timestamp: 1.0,
        }];
        let c = compute_peer_consensus(&frames);
        assert!(c <= 1.0);
    }

    #[test]
    fn sync_mapping_scale_matches_intensity() {
        let frames = vec![SyncDeviceFrame {
            tick: 1,
            peer_id: 3,
            device_id: "dev:7".into(),
            intensity: 0.65,
            position: 0.65,
            timestamp: 1.0,
        }];
        let mappings = sync_to_device_mappings(&frames, "test");
        assert!((mappings[0].scale - 0.65).abs() < 1e-6);
    }
}
