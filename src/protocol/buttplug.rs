//! Buttplug.io v3 WebSocket protocol adapter.
//!
//! Supports 750+ devices via Intiface Central. Handles handshake,
//! device enumeration, capability detection, and all Buttplug v3
//! command types (`ScalarCmd`, `LinearCmd`, `RotateCmd`, `StopDeviceCmd`).

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::sync::Mutex;
use tokio_tungstenite::tungstenite::{self, Message};
use tracing::{info, warn};

use crate::device::{Actuator, ActuatorType, Device, DeviceId};
use crate::protocol::{Protocol, ProtocolConfig, ProtocolError};

type WsStream =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

/// Buttplug.io v3 protocol adapter.
pub struct ButtplugAdapter {
    config: ProtocolConfig,
    ws: Arc<Mutex<Option<WsStream>>>,
    devices: Arc<Mutex<HashMap<String, Device>>>,
    msg_id: Arc<Mutex<u32>>,
    connected: bool,
}

impl ButtplugAdapter {
    pub fn new(config: ProtocolConfig) -> Self {
        Self {
            config,
            ws: Arc::new(Mutex::new(None)),
            devices: Arc::new(Mutex::new(HashMap::new())),
            msg_id: Arc::new(Mutex::new(1)),
            connected: false,
        }
    }

    async fn next_id(&self) -> u32 {
        let mut id = self.msg_id.lock().await;
        let current = *id;
        *id += 1;
        current
    }

    async fn send_msg(&self, msg_type: &str, fields: Value) -> Result<(), ProtocolError> {
        let id = self.next_id().await;
        let mut payload = fields.as_object().cloned().unwrap_or_default();
        payload.insert("Id".into(), json!(id));
        let msg = json!([{ msg_type: payload }]);

        let mut ws_guard = self.ws.lock().await;
        let ws = ws_guard.as_mut().ok_or(ProtocolError::Disconnected)?;
        ws.send(Message::Text(msg.to_string().into()))
            .await
            .map_err(|e: tungstenite::Error| ProtocolError::Protocol(e.to_string()))?;
        drop(ws_guard);
        Ok(())
    }

    #[allow(clippy::significant_drop_tightening)]
    async fn send_and_recv(&self, msg_type: &str, fields: Value) -> Result<Value, ProtocolError> {
        let id = self.next_id().await;
        let mut payload = fields.as_object().cloned().unwrap_or_default();
        payload.insert("Id".into(), json!(id));
        let msg = json!([{ msg_type: payload }]);

        let mut ws_guard = self.ws.lock().await;
        let ws = ws_guard.as_mut().ok_or(ProtocolError::Disconnected)?;
        ws.send(Message::Text(msg.to_string().into()))
            .await
            .map_err(|e: tungstenite::Error| ProtocolError::Protocol(e.to_string()))?;

        while let Some(raw) = ws.next().await {
            let raw =
                raw.map_err(|e: tungstenite::Error| ProtocolError::Protocol(e.to_string()))?;
            if let Message::Text(text) = raw {
                if let Ok(msgs) = serde_json::from_str::<Vec<Value>>(&text) {
                    if let Some(first) = msgs.into_iter().next() {
                        return Ok(first);
                    }
                }
            }
        }
        Err(ProtocolError::Disconnected)
    }

    #[allow(clippy::cast_possible_truncation)]
    fn parse_device(info: &Value) -> Option<Device> {
        let index = info.get("DeviceIndex")?.as_u64()? as u32;
        let name = info.get("DeviceName")?.as_str()?.to_string();
        let device_id = DeviceId(format!("buttplug:{index}"));

        let mut actuators = Vec::new();

        // Parse DeviceMessages for capabilities
        if let Some(msgs) = info.get("DeviceMessages").and_then(|m| m.as_object()) {
            // ScalarCmd actuators
            if let Some(scalars) = msgs.get("ScalarCmd").and_then(|s| s.as_array()) {
                for (i, attr) in scalars.iter().enumerate() {
                    let atype = attr
                        .get("ActuatorType")
                        .and_then(|t| t.as_str())
                        .unwrap_or("Vibrate");
                    let step_count = attr
                        .get("StepCount")
                        .and_then(serde_json::Value::as_u64)
                        .unwrap_or(100) as u32;
                    actuators.push(Actuator {
                        index: i as u32,
                        actuator_type: ActuatorType::parse(atype),
                        description: format!("{atype} #{i}"),
                        step_count,
                    });
                }
            }
            // LinearCmd actuators
            if let Some(linears) = msgs.get("LinearCmd").and_then(|s| s.as_array()) {
                for (i, attr) in linears.iter().enumerate() {
                    let step_count = attr
                        .get("StepCount")
                        .and_then(serde_json::Value::as_u64)
                        .unwrap_or(100) as u32;
                    actuators.push(Actuator {
                        index: i as u32,
                        actuator_type: ActuatorType::Linear,
                        description: format!("Linear #{i}"),
                        step_count,
                    });
                }
            }
            // RotateCmd actuators
            if let Some(rotates) = msgs.get("RotateCmd").and_then(|s| s.as_array()) {
                for (i, attr) in rotates.iter().enumerate() {
                    let step_count = attr
                        .get("StepCount")
                        .and_then(serde_json::Value::as_u64)
                        .unwrap_or(100) as u32;
                    actuators.push(Actuator {
                        index: i as u32,
                        actuator_type: ActuatorType::Rotate,
                        description: format!("Rotate #{i}"),
                        step_count,
                    });
                }
            }
        }

        Some(Device {
            id: device_id,
            name,
            protocol: "buttplug".into(),
            actuators,
            metadata: HashMap::new(),
        })
    }
}

impl Protocol for ButtplugAdapter {
    fn connect(&mut self) -> Pin<Box<dyn Future<Output = Result<(), ProtocolError>> + Send + '_>> {
        Box::pin(async move {
            let (ws, _) = tokio_tungstenite::connect_async(&self.config.endpoint)
                .await
                .map_err(|e| ProtocolError::Connection(e.to_string()))?;

            *self.ws.lock().await = Some(ws);

            let resp = self
                .send_and_recv(
                    "RequestServerInfo",
                    json!({"ClientName": self.config.client_name, "MessageVersion": 3}),
                )
                .await?;

            let server_name = resp
                .get("ServerInfo")
                .and_then(|s| s.get("ServerName"))
                .and_then(|n| n.as_str())
                .unwrap_or("unknown");
            info!(server = server_name, "Buttplug connected");
            self.connected = true;
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
            self.devices.lock().await.clear();
            info!("Buttplug disconnected");
            Ok(())
        })
    }

    fn scan(
        &mut self,
        duration_ms: u64,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<Device>, ProtocolError>> + Send + '_>> {
        Box::pin(async move {
            self.send_msg("StartScanning", json!({})).await?;
            info!(duration_ms, "Scanning for devices");

            let deadline =
                tokio::time::Instant::now() + tokio::time::Duration::from_millis(duration_ms);

            while tokio::time::Instant::now() < deadline {
                let msg = {
                    let mut ws_guard = self.ws.lock().await;
                    let ws = ws_guard.as_mut().ok_or(ProtocolError::Disconnected)?;
                    let result =
                        tokio::time::timeout(tokio::time::Duration::from_millis(500), ws.next())
                            .await;
                    drop(ws_guard);
                    result
                };

                match msg {
                    Ok(Some(Ok(Message::Text(text)))) => {
                        if let Ok(msgs) = serde_json::from_str::<Vec<Value>>(&text) {
                            for m in msgs {
                                if let Some(info) = m.get("DeviceAdded") {
                                    if let Some(dev) = Self::parse_device(info) {
                                        info!(name = dev.name, id = %dev.id.0, "Device found");
                                        self.devices.lock().await.insert(dev.id.0.clone(), dev);
                                    }
                                }
                            }
                        }
                    }
                    Ok(Some(Err(e))) => {
                        warn!(error = %e, "WebSocket error during scan");
                    }
                    _ => {} // Timeout or stream end
                }
            }

            let _ = self.send_msg("StopScanning", json!({})).await;
            let devices: Vec<Device> = self.devices.lock().await.values().cloned().collect();
            info!(count = devices.len(), "Scan complete");
            Ok(devices)
        })
    }

    fn scalar_cmd(
        &self,
        device_id: &str,
        intensity: f64,
        actuator_type: ActuatorType,
        actuator_index: u32,
    ) -> Pin<Box<dyn Future<Output = Result<(), ProtocolError>> + Send + '_>> {
        let device_id = device_id.to_string();
        Box::pin(async move {
            let idx = parse_buttplug_index(&device_id)?;
            let intensity = intensity.clamp(0.0, 1.0);
            self.send_msg(
                "ScalarCmd",
                json!({
                    "DeviceIndex": idx,
                    "Scalars": [{
                        "Index": actuator_index,
                        "Scalar": intensity,
                        "ActuatorType": actuator_type.as_str(),
                    }]
                }),
            )
            .await
        })
    }

    fn linear_cmd(
        &self,
        device_id: &str,
        position: f64,
        duration_ms: u32,
    ) -> Pin<Box<dyn Future<Output = Result<(), ProtocolError>> + Send + '_>> {
        let device_id = device_id.to_string();
        Box::pin(async move {
            let idx = parse_buttplug_index(&device_id)?;
            let position = position.clamp(0.0, 1.0);
            self.send_msg(
                "LinearCmd",
                json!({
                    "DeviceIndex": idx,
                    "Vectors": [{
                        "Index": 0,
                        "Duration": duration_ms.max(1),
                        "Position": position,
                    }]
                }),
            )
            .await
        })
    }

    fn rotate_cmd(
        &self,
        device_id: &str,
        speed: f64,
        clockwise: bool,
    ) -> Pin<Box<dyn Future<Output = Result<(), ProtocolError>> + Send + '_>> {
        let device_id = device_id.to_string();
        Box::pin(async move {
            let idx = parse_buttplug_index(&device_id)?;
            let speed = speed.clamp(0.0, 1.0);
            self.send_msg(
                "RotateCmd",
                json!({
                    "DeviceIndex": idx,
                    "Rotations": [{
                        "Index": 0,
                        "Speed": speed,
                        "Clockwise": clockwise,
                    }]
                }),
            )
            .await
        })
    }

    fn stop_device(
        &self,
        device_id: &str,
    ) -> Pin<Box<dyn Future<Output = Result<(), ProtocolError>> + Send + '_>> {
        let device_id = device_id.to_string();
        Box::pin(async move {
            let idx = parse_buttplug_index(&device_id)?;
            self.send_msg("StopDeviceCmd", json!({"DeviceIndex": idx}))
                .await
        })
    }

    fn stop_all(&self) -> Pin<Box<dyn Future<Output = Result<(), ProtocolError>> + Send + '_>> {
        Box::pin(async move { self.send_msg("StopAllDevices", json!({})).await })
    }

    fn is_connected(&self) -> bool {
        self.connected
    }

    fn name(&self) -> &'static str {
        "buttplug"
    }
}

fn parse_buttplug_index(device_id: &str) -> Result<u32, ProtocolError> {
    // "buttplug:3" -> 3
    device_id
        .strip_prefix("buttplug:")
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| ProtocolError::DeviceNotFound(device_id.to_string()))
}
