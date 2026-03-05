//! Streaming bridge — ALICE-Streaming-Protocol ↔ ALICE-Bridge
//!
//! Synchronizes device commands with ASP stream frames.
//! Frame timing → device output synchronization.

use crate::bridge::BridgeAction;

/// Stream-synchronized device command.
///
/// Links a device action to a specific stream frame for A/V sync.
#[derive(Debug, Clone)]
pub struct StreamSyncCommand {
    /// ASP frame number.
    pub frame_number: u64,
    /// Presentation timestamp (PTS, milliseconds from stream start).
    pub pts_ms: u64,
    /// Target device ID.
    pub device_id: String,
    /// Device intensity [0.0, 1.0].
    pub intensity: f64,
    /// Duration of this command (milliseconds).
    pub duration_ms: u32,
}

/// Convert a `BridgeAction` with stream timing to a sync command.
#[inline]
#[must_use]
pub fn bridge_action_to_stream_sync(
    action: &BridgeAction,
    frame_number: u64,
    pts_ms: u64,
    device_id: &str,
) -> StreamSyncCommand {
    StreamSyncCommand {
        frame_number,
        pts_ms,
        device_id: device_id.to_string(),
        intensity: action.position,
        duration_ms: action.duration_ms,
    }
}

/// Convert a stream sync command back to a `BridgeAction`.
#[inline]
#[must_use]
pub fn stream_sync_to_bridge_action(cmd: &StreamSyncCommand) -> BridgeAction {
    BridgeAction {
        position: cmd.intensity,
        duration_ms: cmd.duration_ms,
        #[allow(clippy::cast_precision_loss)]
        timestamp: cmd.pts_ms as f64 / 1000.0,
    }
}

/// Stream frame haptic metadata embedded in ASP delta packets.
#[derive(Debug, Clone, Copy)]
pub struct FrameHapticMetadata {
    /// Frame number.
    pub frame_number: u64,
    /// Dominant motion magnitude in the frame [0.0, 1.0].
    pub motion_magnitude: f32,
    /// Scene change indicator (true if I-frame boundary).
    pub scene_change: bool,
    /// Audio amplitude (RMS) for this frame window [0.0, 1.0].
    pub audio_rms: f32,
}

/// Convert frame metadata to haptic intensity.
///
/// Combines motion and audio for synchronized haptic feedback.
#[inline]
#[must_use]
pub fn frame_metadata_to_intensity(meta: &FrameHapticMetadata) -> f64 {
    let motion = f64::from(meta.motion_magnitude);
    let audio = f64::from(meta.audio_rms);
    // シーンチェンジ時はパルス
    let scene_boost = if meta.scene_change { 0.3 } else { 0.0 };
    (motion.mul_add(0.5, audio * 0.3) + scene_boost).clamp(0.0, 1.0)
}

/// Stream jitter buffer status for adaptive device output.
#[derive(Debug, Clone, Copy)]
pub struct JitterBufferStatus {
    /// Current buffer depth (frames).
    pub buffer_depth: u32,
    /// Target buffer depth (frames).
    pub target_depth: u32,
    /// Estimated jitter (milliseconds).
    pub jitter_ms: f64,
    /// Underrun count.
    pub underrun_count: u32,
}

/// Compute adaptive output interval based on jitter buffer state.
///
/// Returns recommended output interval in milliseconds.
#[inline]
#[must_use]
pub fn adaptive_output_interval(status: &JitterBufferStatus, base_interval_ms: u32) -> u32 {
    if status.underrun_count > 0 && status.buffer_depth < status.target_depth {
        // バッファ不足時は出力間隔を広げてバッファリングを促進
        base_interval_ms.saturating_mul(2).min(200)
    } else if status.buffer_depth > status.target_depth * 2 {
        // バッファ過多時は間隔を縮めて遅延を減らす
        (base_interval_ms / 2).max(10)
    } else {
        base_interval_ms
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bridge_action_to_stream_sync_fields() {
        let action = BridgeAction {
            position: 0.6,
            duration_ms: 50,
            timestamp: 1.0,
        };
        let cmd = bridge_action_to_stream_sync(&action, 120, 4000, "dev:0");
        assert_eq!(cmd.frame_number, 120);
        assert_eq!(cmd.pts_ms, 4000);
        assert!((cmd.intensity - 0.6).abs() < 1e-6);
    }

    #[test]
    fn stream_sync_roundtrip() {
        let action = BridgeAction {
            position: 0.5,
            duration_ms: 33,
            timestamp: 2.0,
        };
        let cmd = bridge_action_to_stream_sync(&action, 60, 2000, "dev:0");
        let back = stream_sync_to_bridge_action(&cmd);
        assert!((back.position - 0.5).abs() < 1e-6);
        assert_eq!(back.duration_ms, 33);
    }

    #[test]
    fn frame_metadata_motion_only() {
        let meta = FrameHapticMetadata {
            frame_number: 1,
            motion_magnitude: 0.8,
            scene_change: false,
            audio_rms: 0.0,
        };
        let intensity = frame_metadata_to_intensity(&meta);
        assert!((intensity - 0.4).abs() < 1e-6);
    }

    #[test]
    fn frame_metadata_scene_change() {
        let meta = FrameHapticMetadata {
            frame_number: 100,
            motion_magnitude: 0.0,
            scene_change: true,
            audio_rms: 0.0,
        };
        let intensity = frame_metadata_to_intensity(&meta);
        assert!((intensity - 0.3).abs() < 1e-6);
    }

    #[test]
    fn frame_metadata_combined() {
        let meta = FrameHapticMetadata {
            frame_number: 50,
            motion_magnitude: 1.0,
            scene_change: true,
            audio_rms: 1.0,
        };
        let intensity = frame_metadata_to_intensity(&meta);
        // 1.0*0.5 + 1.0*0.3 + 0.3 = 1.1 → clamped to 1.0
        assert!((intensity - 1.0).abs() < 1e-6);
    }

    #[test]
    fn adaptive_interval_normal() {
        let status = JitterBufferStatus {
            buffer_depth: 3,
            target_depth: 3,
            jitter_ms: 5.0,
            underrun_count: 0,
        };
        assert_eq!(adaptive_output_interval(&status, 50), 50);
    }

    #[test]
    fn adaptive_interval_underrun() {
        let status = JitterBufferStatus {
            buffer_depth: 1,
            target_depth: 3,
            jitter_ms: 20.0,
            underrun_count: 2,
        };
        assert_eq!(adaptive_output_interval(&status, 50), 100);
    }

    #[test]
    fn adaptive_interval_buffer_full() {
        let status = JitterBufferStatus {
            buffer_depth: 10,
            target_depth: 3,
            jitter_ms: 2.0,
            underrun_count: 0,
        };
        assert_eq!(adaptive_output_interval(&status, 50), 25);
    }

    #[test]
    fn adaptive_interval_min_clamp() {
        let status = JitterBufferStatus {
            buffer_depth: 100,
            target_depth: 3,
            jitter_ms: 1.0,
            underrun_count: 0,
        };
        assert!(adaptive_output_interval(&status, 10) >= 10);
    }
}
