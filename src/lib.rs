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

pub mod ble;
pub mod bridge;
pub mod bridges;
pub mod device;
pub mod distributed;
pub mod feedback;
pub mod protocol;
pub mod safety;
pub mod script;
pub mod sensor;

pub use ble::{BleDevice, BleManager, BleState, GattCharacteristic};
pub use bridge::SignalBridge;
pub use device::{Actuator, ActuatorType, Device, DeviceId, DeviceManager, DeviceMapping};
pub use distributed::{NodeInfo, NodeRegistry, NodeStatus, RouteMessage};
pub use feedback::{FeedbackController, PidConfig, PidController};
pub use protocol::{Protocol, ProtocolConfig, ProtocolError};
pub use safety::{EmergencyStop, GradualRamp, IntensityLimiter, SafetyLimits};
pub use script::{PlayState, Script, ScriptPlayer, ScriptRecorder};
pub use sensor::{SensorReading, SensorRegistry, SensorType};
