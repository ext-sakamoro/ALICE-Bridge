//! Protocol adapters — pluggable transport backends for device communication.

#[cfg(feature = "websocket")]
pub mod buttplug;
#[cfg(feature = "mqtt")]
pub mod mqtt;
pub mod osc;
#[cfg(feature = "rest")]
pub mod rest;
#[cfg(feature = "websocket")]
pub mod websocket;

use std::future::Future;
use std::pin::Pin;

use crate::device::{ActuatorType, Device};

/// Protocol transport error.
#[derive(Debug, thiserror::Error)]
pub enum ProtocolError {
    #[error("connection failed: {0}")]
    Connection(String),
    #[error("disconnected")]
    Disconnected,
    #[error("device not found: {0}")]
    DeviceNotFound(String),
    #[error("command rejected: {0}")]
    CommandRejected(String),
    #[error("timeout after {0}ms")]
    Timeout(u64),
    #[error("protocol error: {0}")]
    Protocol(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

/// Protocol adapter configuration.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ProtocolConfig {
    /// Connection endpoint (URL, host:port, etc.)
    pub endpoint: String,
    /// Connection timeout in milliseconds.
    pub timeout_ms: u64,
    /// Client identifier.
    pub client_name: String,
    /// Optional authentication credentials.
    pub auth: Option<AuthConfig>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AuthConfig {
    pub username: String,
    pub password: String,
}

impl Default for ProtocolConfig {
    fn default() -> Self {
        Self {
            endpoint: "ws://localhost:12345".into(),
            timeout_ms: 5000,
            client_name: "ALICE-Bridge".into(),
            auth: None,
        }
    }
}

/// Trait for protocol adapters.
///
/// Each protocol (Buttplug, MQTT, REST, OSC, WebSocket) implements this trait,
/// providing a uniform interface for device discovery and command dispatch.
pub trait Protocol: Send + Sync {
    /// Connect to the protocol endpoint.
    fn connect(&mut self) -> Pin<Box<dyn Future<Output = Result<(), ProtocolError>> + Send + '_>>;

    /// Disconnect from the protocol endpoint.
    fn disconnect(
        &mut self,
    ) -> Pin<Box<dyn Future<Output = Result<(), ProtocolError>> + Send + '_>>;

    /// Scan for devices. Returns discovered devices.
    fn scan(
        &mut self,
        duration_ms: u64,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<Device>, ProtocolError>> + Send + '_>>;

    /// Send a scalar command (vibration, heat, e-stim, etc.)
    fn scalar_cmd(
        &self,
        device_id: &str,
        intensity: f64,
        actuator_type: ActuatorType,
        actuator_index: u32,
    ) -> Pin<Box<dyn Future<Output = Result<(), ProtocolError>> + Send + '_>>;

    /// Send a linear command (stroker position).
    fn linear_cmd(
        &self,
        device_id: &str,
        position: f64,
        duration_ms: u32,
    ) -> Pin<Box<dyn Future<Output = Result<(), ProtocolError>> + Send + '_>>;

    /// Send a rotate command.
    fn rotate_cmd(
        &self,
        device_id: &str,
        speed: f64,
        clockwise: bool,
    ) -> Pin<Box<dyn Future<Output = Result<(), ProtocolError>> + Send + '_>>;

    /// Stop a specific device.
    fn stop_device(
        &self,
        device_id: &str,
    ) -> Pin<Box<dyn Future<Output = Result<(), ProtocolError>> + Send + '_>>;

    /// Stop all devices.
    fn stop_all(&self) -> Pin<Box<dyn Future<Output = Result<(), ProtocolError>> + Send + '_>>;

    /// Whether the protocol is currently connected.
    fn is_connected(&self) -> bool;

    /// Protocol name for logging/identification.
    fn name(&self) -> &str;
}
