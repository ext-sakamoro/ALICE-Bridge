//! Ecosystem bridge modules — ALICE-Bridge ↔ 9 ALICE crates.
//!
//! Each module defines intermediate data types and conversion functions
//! for connecting hardware I/O to the ALICE ecosystem.

pub mod bridge_analytics;
pub mod bridge_edge;
pub mod bridge_kinematics;
pub mod bridge_motion;
pub mod bridge_physics;
pub mod bridge_presence;
pub mod bridge_streaming;
pub mod bridge_sync;
pub mod bridge_telemetry;
