//! ALICE-Bridge — Universal hardware bridge
//!
//! Protocol-agnostic device communication layer that connects to any hardware
//! via pluggable protocol adapters (Buttplug.io, MQTT, REST, OSC, WebSocket).
//!
//! # Architecture
//!
//! ```text
//! Application (FunForge, VRChat, IoT controller, ...)
//!     |
//!     v
//! ┌─────────────────────────────────┐
//! │  ALICE-Bridge                   │
//! │  ┌───────────┐ ┌─────────────┐ │
//! │  │  Device    │ │   Safety    │ │
//! │  │  Manager   │ │   Layer     │ │
//! │  └─────┬─────┘ └──────┬──────┘ │
//! │        │               │        │
//! │  ┌─────▼───────────────▼──────┐ │
//! │  │     Signal Bridge          │ │
//! │  └─────┬──────────────────────┘ │
//! │        │                        │
//! │  ┌─────▼──────────────────────┐ │
//! │  │  Protocol Adapters         │ │
//! │  │  Buttplug│MQTT│REST│OSC│WS │ │
//! │  └────────────────────────────┘ │
//! └─────────────────────────────────┘
//!     |
//!     v
//! Hardware (750+ devices)
//! ```

pub mod bridge;
pub mod bridges;
pub mod device;
pub mod protocol;
pub mod safety;

pub use bridge::SignalBridge;
pub use device::{Actuator, ActuatorType, Device, DeviceId, DeviceManager, DeviceMapping};
pub use protocol::{Protocol, ProtocolConfig, ProtocolError};
pub use safety::{EmergencyStop, GradualRamp, IntensityLimiter, SafetyLimits};
