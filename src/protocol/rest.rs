//! REST API protocol adapter for HTTP-controlled devices.
//!
//! Connects to devices with REST APIs (e.g., The Handy v2).
//! Supports time synchronization for synchronized playback.

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use reqwest::Client;
use serde_json::json;
use tokio::sync::Mutex;
use tracing::info;

use crate::device::{Actuator, ActuatorType, Device, DeviceId};
use crate::protocol::{Protocol, ProtocolConfig, ProtocolError};

/// REST API protocol adapter.
pub struct RestAdapter {
    config: ProtocolConfig,
    client: Client,
    /// Custom headers sent with every request (e.g., connection keys).
    headers: HashMap<String, String>,
    connected: bool,
    /// Server time offset for synchronized playback (ms).
    server_time_offset_ms: Arc<Mutex<f64>>,
}

impl RestAdapter {
    /// # Panics
    ///
    /// Panics if the HTTP client builder fails (should never happen with default TLS).
    #[must_use]
    pub fn new(config: ProtocolConfig, headers: HashMap<String, String>) -> Self {
        let client = Client::builder()
            .timeout(std::time::Duration::from_millis(config.timeout_ms))
            .build()
            .expect("HTTP client build");
        Self {
            config,
            client,
            headers,
            connected: false,
            server_time_offset_ms: Arc::new(Mutex::new(0.0)),
        }
    }

    async fn get(&self, path: &str) -> Result<serde_json::Value, ProtocolError> {
        let url = format!("{}{path}", self.config.endpoint);
        let mut req = self.client.get(&url);
        for (k, v) in &self.headers {
            req = req.header(k.as_str(), v.as_str());
        }
        let resp = req
            .send()
            .await
            .map_err(|e| ProtocolError::Connection(e.to_string()))?;
        if !resp.status().is_success() {
            return Err(ProtocolError::CommandRejected(format!(
                "GET {path}: {}",
                resp.status()
            )));
        }
        resp.json()
            .await
            .map_err(|e| ProtocolError::Protocol(e.to_string()))
    }

    async fn put(&self, path: &str, body: &serde_json::Value) -> Result<(), ProtocolError> {
        let url = format!("{}{path}", self.config.endpoint);
        let mut req = self.client.put(&url).json(body);
        for (k, v) in &self.headers {
            req = req.header(k.as_str(), v.as_str());
        }
        let resp = req
            .send()
            .await
            .map_err(|e| ProtocolError::Connection(e.to_string()))?;
        if !resp.status().is_success() {
            return Err(ProtocolError::CommandRejected(format!(
                "PUT {path}: {}",
                resp.status()
            )));
        }
        Ok(())
    }

    /// Estimate server time offset via multiple round-trips.
    #[allow(clippy::cast_precision_loss)]
    pub async fn sync_server_time(&self, endpoint: &str, samples: u32) -> f64 {
        let mut offsets = Vec::new();
        for _ in 0..samples {
            let t_send = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as f64;

            if let Ok(resp) = self.get(endpoint).await {
                let t_recv = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as f64;
                if let Some(server_time) =
                    resp.get("serverTime").and_then(serde_json::Value::as_f64)
                {
                    let rtt = t_recv - t_send;
                    let offset = server_time - (t_send + rtt / 2.0);
                    offsets.push(offset);
                }
            }
        }
        if offsets.is_empty() {
            return 0.0;
        }
        let avg = offsets.iter().sum::<f64>() / offsets.len() as f64;
        *self.server_time_offset_ms.lock().await = avg;
        info!(offset_ms = avg, "Server time synchronized");
        avg
    }
}

impl Protocol for RestAdapter {
    fn connect(&mut self) -> Pin<Box<dyn Future<Output = Result<(), ProtocolError>> + Send + '_>> {
        Box::pin(async move {
            let resp = self.get("/connected").await?;
            let is_connected = resp
                .get("connected")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false);
            if !is_connected {
                return Err(ProtocolError::Connection("device not connected".into()));
            }
            self.connected = true;
            info!(endpoint = self.config.endpoint, "REST device connected");
            Ok(())
        })
    }

    fn disconnect(
        &mut self,
    ) -> Pin<Box<dyn Future<Output = Result<(), ProtocolError>> + Send + '_>> {
        Box::pin(async move {
            self.connected = false;
            info!("REST disconnected");
            Ok(())
        })
    }

    fn scan(
        &mut self,
        _duration_ms: u64,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<Device>, ProtocolError>> + Send + '_>> {
        Box::pin(async move {
            // REST devices are pre-configured; return info if connected
            if !self.connected {
                return Ok(Vec::new());
            }
            let info = self.get("/info").await.unwrap_or_default();
            let model = info
                .get("model")
                .and_then(|m| m.as_str())
                .unwrap_or("REST Device");
            let device = Device {
                id: DeviceId("rest:0".into()),
                name: model.to_string(),
                protocol: "rest".into(),
                actuators: vec![Actuator {
                    index: 0,
                    actuator_type: ActuatorType::Linear,
                    description: "Primary axis".into(),
                    step_count: 100,
                }],
                metadata: HashMap::new(),
            };
            Ok(vec![device])
        })
    }

    fn scalar_cmd(
        &self,
        _device_id: &str,
        intensity: f64,
        _actuator_type: ActuatorType,
        _actuator_index: u32,
    ) -> Pin<Box<dyn Future<Output = Result<(), ProtocolError>> + Send + '_>> {
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        Box::pin(async move {
            let velocity = (intensity.clamp(0.0, 1.0) * 100.0) as u32;
            self.put("/hamp/velocity", &json!({"velocity": velocity}))
                .await
        })
    }

    fn linear_cmd(
        &self,
        _device_id: &str,
        position: f64,
        _duration_ms: u32,
    ) -> Pin<Box<dyn Future<Output = Result<(), ProtocolError>> + Send + '_>> {
        Box::pin(async move {
            let pos_pct = position.clamp(0.0, 1.0) * 100.0;
            self.put(
                "/hdsp/xpva",
                &json!({"position": pos_pct, "stopOnTarget": true}),
            )
            .await
        })
    }

    fn rotate_cmd(
        &self,
        _device_id: &str,
        _speed: f64,
        _clockwise: bool,
    ) -> Pin<Box<dyn Future<Output = Result<(), ProtocolError>> + Send + '_>> {
        Box::pin(async move {
            Err(ProtocolError::CommandRejected(
                "REST adapter: rotate not supported".into(),
            ))
        })
    }

    fn stop_device(
        &self,
        _device_id: &str,
    ) -> Pin<Box<dyn Future<Output = Result<(), ProtocolError>> + Send + '_>> {
        Box::pin(async move { self.put("/hamp/stop", &json!({})).await })
    }

    fn stop_all(&self) -> Pin<Box<dyn Future<Output = Result<(), ProtocolError>> + Send + '_>> {
        Box::pin(async move { self.put("/hamp/stop", &json!({})).await })
    }

    fn is_connected(&self) -> bool {
        self.connected
    }

    fn name(&self) -> &'static str {
        "rest"
    }
}
