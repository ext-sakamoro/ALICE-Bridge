//! OSC (Open Sound Control) protocol adapter.
//!
//! Receives and sends OSC messages over UDP. Used for `VRChat` avatar
//! contacts, `TouchDesigner`, and other OSC-compatible applications.
//! No external dependencies — pure tokio UDP.

use std::future::Future;
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::Arc;

use tokio::net::UdpSocket;
use tokio::sync::Mutex;
use tracing::info;

use crate::device::{ActuatorType, Device};
use crate::protocol::{Protocol, ProtocolConfig, ProtocolError};

/// OSC protocol adapter (UDP).
pub struct OscAdapter {
    config: ProtocolConfig,
    socket: Arc<Mutex<Option<UdpSocket>>>,
    target_addr: SocketAddr,
    connected: bool,
}

impl OscAdapter {
    pub fn new(config: ProtocolConfig, target_addr: SocketAddr) -> Self {
        Self {
            config,
            socket: Arc::new(Mutex::new(None)),
            target_addr,
            connected: false,
        }
    }

    /// Encode an OSC message (address + single f32 argument).
    /// Minimal OSC encoding without external dependencies.
    fn encode_osc_float(address: &str, value: f32) -> Vec<u8> {
        let mut buf = Vec::with_capacity(64);

        // Address string (null-terminated, padded to 4-byte boundary)
        buf.extend_from_slice(address.as_bytes());
        buf.push(0);
        while buf.len() % 4 != 0 {
            buf.push(0);
        }

        // Type tag string ",f\0\0"
        buf.extend_from_slice(b",f\0\0");

        // Float argument (big-endian)
        buf.extend_from_slice(&value.to_be_bytes());

        buf
    }

    async fn send_osc(&self, address: &str, value: f32) -> Result<(), ProtocolError> {
        let sock_guard = self.socket.lock().await;
        let sock = sock_guard.as_ref().ok_or(ProtocolError::Disconnected)?;
        let data = Self::encode_osc_float(address, value);
        sock.send_to(&data, self.target_addr)
            .await
            .map_err(ProtocolError::Io)?;
        drop(sock_guard);
        Ok(())
    }
}

impl Protocol for OscAdapter {
    fn connect(&mut self) -> Pin<Box<dyn Future<Output = Result<(), ProtocolError>> + Send + '_>> {
        Box::pin(async move {
            let bind_addr: SocketAddr = self
                .config
                .endpoint
                .parse()
                .map_err(|e: std::net::AddrParseError| ProtocolError::Connection(e.to_string()))?;
            let socket = UdpSocket::bind(bind_addr)
                .await
                .map_err(|e| ProtocolError::Connection(e.to_string()))?;
            info!(
                bind = %bind_addr,
                target = %self.target_addr,
                "OSC connected"
            );
            *self.socket.lock().await = Some(socket);
            self.connected = true;
            Ok(())
        })
    }

    fn disconnect(
        &mut self,
    ) -> Pin<Box<dyn Future<Output = Result<(), ProtocolError>> + Send + '_>> {
        Box::pin(async move {
            *self.socket.lock().await = None;
            self.connected = false;
            info!("OSC disconnected");
            Ok(())
        })
    }

    fn scan(
        &mut self,
        _duration_ms: u64,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<Device>, ProtocolError>> + Send + '_>> {
        // OSC devices are configured, not discovered
        Box::pin(async move { Ok(Vec::new()) })
    }

    fn scalar_cmd(
        &self,
        device_id: &str,
        intensity: f64,
        _actuator_type: ActuatorType,
        _actuator_index: u32,
    ) -> Pin<Box<dyn Future<Output = Result<(), ProtocolError>> + Send + '_>> {
        let address = format!("/alice/bridge/{device_id}/intensity");
        #[allow(clippy::cast_possible_truncation)]
        Box::pin(async move {
            self.send_osc(&address, intensity.clamp(0.0, 1.0) as f32)
                .await
        })
    }

    fn linear_cmd(
        &self,
        device_id: &str,
        position: f64,
        _duration_ms: u32,
    ) -> Pin<Box<dyn Future<Output = Result<(), ProtocolError>> + Send + '_>> {
        let address = format!("/alice/bridge/{device_id}/position");
        #[allow(clippy::cast_possible_truncation)]
        Box::pin(async move {
            self.send_osc(&address, position.clamp(0.0, 1.0) as f32)
                .await
        })
    }

    fn rotate_cmd(
        &self,
        device_id: &str,
        speed: f64,
        clockwise: bool,
    ) -> Pin<Box<dyn Future<Output = Result<(), ProtocolError>> + Send + '_>> {
        let address = format!("/alice/bridge/{device_id}/rotate");
        #[allow(clippy::cast_possible_truncation)]
        Box::pin(async move {
            let signed = if clockwise { speed } else { -speed };
            self.send_osc(&address, signed.clamp(-1.0, 1.0) as f32)
                .await
        })
    }

    fn stop_device(
        &self,
        device_id: &str,
    ) -> Pin<Box<dyn Future<Output = Result<(), ProtocolError>> + Send + '_>> {
        let address = format!("/alice/bridge/{device_id}/stop");
        Box::pin(async move { self.send_osc(&address, 1.0).await })
    }

    fn stop_all(&self) -> Pin<Box<dyn Future<Output = Result<(), ProtocolError>> + Send + '_>> {
        Box::pin(async move { self.send_osc("/alice/bridge/stop_all", 1.0).await })
    }

    fn is_connected(&self) -> bool {
        self.connected
    }

    fn name(&self) -> &'static str {
        "osc"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn osc_encode_float() {
        let data = OscAdapter::encode_osc_float("/test", 0.5);
        // Address: "/test\0" padded to 8 bytes
        assert_eq!(&data[..5], b"/test");
        assert_eq!(data[5], 0);
        // Padding
        assert_eq!(data.len() % 4, 0);
        // Type tag: ",f\0\0"
        let tag_offset = 8; // "/test\0\0\0" = 8 bytes
        assert_eq!(&data[tag_offset..tag_offset + 4], b",f\0\0");
        // Float value
        let float_bytes = &data[tag_offset + 4..tag_offset + 8];
        let val = f32::from_be_bytes(float_bytes.try_into().unwrap());
        assert!((val - 0.5).abs() < 1e-6);
    }
}
