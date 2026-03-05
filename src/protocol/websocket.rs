//! Raw WebSocket protocol adapter.
//!
//! Generic WebSocket transport for custom protocols. Sends JSON
//! command payloads and receives responses. Useful for custom
//! device firmware with WebSocket servers.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use futures_util::SinkExt;
use serde_json::{json, Value};
use tokio::sync::Mutex;
use tokio_tungstenite::tungstenite::{self, Message};
use tracing::info;

use crate::device::{ActuatorType, Device};
use crate::protocol::{Protocol, ProtocolConfig, ProtocolError};

type WsStream =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

/// Raw WebSocket protocol adapter.
pub struct WebSocketAdapter {
    config: ProtocolConfig,
    ws: Arc<Mutex<Option<WsStream>>>,
    connected: bool,
}

impl WebSocketAdapter {
    pub fn new(config: ProtocolConfig) -> Self {
        Self {
            config,
            ws: Arc::new(Mutex::new(None)),
            connected: false,
        }
    }

    async fn send_json(&self, payload: &Value) -> Result<(), ProtocolError> {
        let mut ws_guard = self.ws.lock().await;
        let ws = ws_guard.as_mut().ok_or(ProtocolError::Disconnected)?;
        ws.send(Message::Text(payload.to_string().into()))
            .await
            .map_err(|e: tungstenite::Error| ProtocolError::Protocol(e.to_string()))?;
        drop(ws_guard);
        Ok(())
    }
}

impl Protocol for WebSocketAdapter {
    fn connect(&mut self) -> Pin<Box<dyn Future<Output = Result<(), ProtocolError>> + Send + '_>> {
        Box::pin(async move {
            let (ws, _) = tokio_tungstenite::connect_async(&self.config.endpoint)
                .await
                .map_err(|e| ProtocolError::Connection(e.to_string()))?;
            *self.ws.lock().await = Some(ws);
            self.connected = true;
            info!(endpoint = self.config.endpoint, "WebSocket connected");
            Ok(())
        })
    }

    fn disconnect(
        &mut self,
    ) -> Pin<Box<dyn Future<Output = Result<(), ProtocolError>> + Send + '_>> {
        Box::pin(async move {
            if let Some(ws) = self.ws.lock().await.as_mut() {
                let _ = ws.close(None).await;
            }
            *self.ws.lock().await = None;
            self.connected = false;
            info!("WebSocket disconnected");
            Ok(())
        })
    }

    fn scan(
        &mut self,
        _duration_ms: u64,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<Device>, ProtocolError>> + Send + '_>> {
        Box::pin(async move { Ok(Vec::new()) })
    }

    fn scalar_cmd(
        &self,
        device_id: &str,
        intensity: f64,
        actuator_type: ActuatorType,
        actuator_index: u32,
    ) -> Pin<Box<dyn Future<Output = Result<(), ProtocolError>> + Send + '_>> {
        let payload = json!({
            "cmd": "scalar",
            "device": device_id,
            "intensity": intensity.clamp(0.0, 1.0),
            "actuator_type": actuator_type.as_str(),
            "actuator_index": actuator_index,
        });
        Box::pin(async move { self.send_json(&payload).await })
    }

    fn linear_cmd(
        &self,
        device_id: &str,
        position: f64,
        duration_ms: u32,
    ) -> Pin<Box<dyn Future<Output = Result<(), ProtocolError>> + Send + '_>> {
        let payload = json!({
            "cmd": "linear",
            "device": device_id,
            "position": position.clamp(0.0, 1.0),
            "duration_ms": duration_ms,
        });
        Box::pin(async move { self.send_json(&payload).await })
    }

    fn rotate_cmd(
        &self,
        device_id: &str,
        speed: f64,
        clockwise: bool,
    ) -> Pin<Box<dyn Future<Output = Result<(), ProtocolError>> + Send + '_>> {
        let payload = json!({
            "cmd": "rotate",
            "device": device_id,
            "speed": speed.clamp(0.0, 1.0),
            "clockwise": clockwise,
        });
        Box::pin(async move { self.send_json(&payload).await })
    }

    fn stop_device(
        &self,
        device_id: &str,
    ) -> Pin<Box<dyn Future<Output = Result<(), ProtocolError>> + Send + '_>> {
        let payload = json!({"cmd": "stop", "device": device_id});
        Box::pin(async move { self.send_json(&payload).await })
    }

    fn stop_all(&self) -> Pin<Box<dyn Future<Output = Result<(), ProtocolError>> + Send + '_>> {
        let payload = json!({"cmd": "stop_all"});
        Box::pin(async move { self.send_json(&payload).await })
    }

    fn is_connected(&self) -> bool {
        self.connected
    }

    fn name(&self) -> &'static str {
        "websocket"
    }
}
