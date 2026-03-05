//! MQTT protocol adapter for IoT/ESP32/Arduino haptic devices.
//!
//! Publishes JSON commands to configurable topics, enabling custom hardware
//! (ESP32, Arduino, Raspberry Pi) to receive control signals over MQTT.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use rumqttc::{AsyncClient, EventLoop, MqttOptions, QoS};
use serde_json::json;
use tokio::sync::Mutex;
use tracing::info;

use crate::device::{ActuatorType, Device};
use crate::protocol::{Protocol, ProtocolConfig, ProtocolError};

/// MQTT protocol adapter.
pub struct MqttAdapter {
    config: ProtocolConfig,
    base_topic: String,
    qos: QoS,
    client: Arc<Mutex<Option<AsyncClient>>>,
    event_loop: Arc<Mutex<Option<EventLoop>>>,
    connected: bool,
}

impl MqttAdapter {
    pub fn new(config: ProtocolConfig, base_topic: String, qos: u8) -> Self {
        let qos = match qos {
            0 => QoS::AtMostOnce,
            1 => QoS::AtLeastOnce,
            _ => QoS::ExactlyOnce,
        };
        Self {
            config,
            base_topic,
            qos,
            client: Arc::new(Mutex::new(None)),
            event_loop: Arc::new(Mutex::new(None)),
            connected: false,
        }
    }

    async fn publish(&self, subtopic: &str, payload: &str) -> Result<(), ProtocolError> {
        let client_guard = self.client.lock().await;
        let client = client_guard.as_ref().ok_or(ProtocolError::Disconnected)?;
        let topic = format!("{}/{subtopic}", self.base_topic);
        client
            .publish(topic, self.qos, false, payload.as_bytes())
            .await
            .map_err(|e| ProtocolError::Protocol(e.to_string()))?;
        drop(client_guard);
        Ok(())
    }
}

impl Protocol for MqttAdapter {
    fn connect(&mut self) -> Pin<Box<dyn Future<Output = Result<(), ProtocolError>> + Send + '_>> {
        Box::pin(async move {
            let mut opts = MqttOptions::new(&self.config.client_name, &self.config.endpoint, 1883);
            opts.set_keep_alive(std::time::Duration::from_secs(60));

            if let Some(auth) = &self.config.auth {
                opts.set_credentials(&auth.username, &auth.password);
            }

            let (client, event_loop) = AsyncClient::new(opts, 64);
            *self.client.lock().await = Some(client);
            *self.event_loop.lock().await = Some(event_loop);
            self.connected = true;

            info!(
                broker = self.config.endpoint,
                topic = self.base_topic,
                "MQTT connected"
            );
            Ok(())
        })
    }

    fn disconnect(
        &mut self,
    ) -> Pin<Box<dyn Future<Output = Result<(), ProtocolError>> + Send + '_>> {
        Box::pin(async move {
            // Send stop before disconnect
            let _ = self
                .publish("emergency", &json!({"emergency_stop": true}).to_string())
                .await;

            let taken_client = self.client.lock().await.take();
            if let Some(client) = taken_client {
                let _ = client.disconnect().await;
            }
            *self.event_loop.lock().await = None;
            self.connected = false;
            info!("MQTT disconnected");
            Ok(())
        })
    }

    fn scan(
        &mut self,
        _duration_ms: u64,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<Device>, ProtocolError>> + Send + '_>> {
        // MQTT devices are configured, not discovered
        Box::pin(async move { Ok(Vec::new()) })
    }

    fn scalar_cmd(
        &self,
        device_id: &str,
        intensity: f64,
        _actuator_type: ActuatorType,
        _actuator_index: u32,
    ) -> Pin<Box<dyn Future<Output = Result<(), ProtocolError>> + Send + '_>> {
        let device_id = device_id.to_string();
        Box::pin(async move {
            let payload = json!({
                "intensity": (intensity.clamp(0.0, 1.0) * 10000.0).round() / 10000.0,
                "device": device_id,
            });
            self.publish("vibration", &payload.to_string()).await
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
            let payload = json!({
                "position": (position.clamp(0.0, 1.0) * 10000.0).round() / 10000.0,
                "duration_ms": duration_ms,
                "device": device_id,
            });
            self.publish("position", &payload.to_string()).await
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
            let payload = json!({
                "speed": (speed.clamp(0.0, 1.0) * 10000.0).round() / 10000.0,
                "clockwise": clockwise,
                "device": device_id,
            });
            self.publish("rotate", &payload.to_string()).await
        })
    }

    fn stop_device(
        &self,
        device_id: &str,
    ) -> Pin<Box<dyn Future<Output = Result<(), ProtocolError>> + Send + '_>> {
        let device_id = device_id.to_string();
        Box::pin(async move {
            let payload = json!({
                "position": 0.0,
                "intensity": 0.0,
                "device": device_id,
            });
            self.publish("stop", &payload.to_string()).await
        })
    }

    fn stop_all(&self) -> Pin<Box<dyn Future<Output = Result<(), ProtocolError>> + Send + '_>> {
        Box::pin(async move {
            let payload = json!({"emergency_stop": true});
            self.publish("emergency", &payload.to_string()).await
        })
    }

    fn is_connected(&self) -> bool {
        self.connected
    }

    fn name(&self) -> &'static str {
        "mqtt"
    }
}
