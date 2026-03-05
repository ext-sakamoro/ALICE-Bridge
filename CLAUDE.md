# ALICE-Bridge

## Overview
Universal hardware bridge — protocol-agnostic device communication layer.
Part of the ALICE ecosystem.

## Quick Reference
- **Language**: Rust (edition 2021)
- **Version**: 0.1.0
- **Tests**: `cargo test`
- **License**: AGPL-3.0 (+ Commercial dual license)
- **Features**: websocket, mqtt, rest, full

## Architecture
- `src/protocol/` — Protocol adapters (Buttplug.io, MQTT, REST, OSC, WebSocket)
- `src/device/` — Device abstraction (actuator types, manager, mapping)
- `src/safety/` — Safety layer (limiter, emergency stop, ramp)
- `src/bridge/` — Real-time signal processing bridge
- `src/sensor.rs` — Sensor feedback input (pressure, temperature, acceleration, etc.)
- `src/feedback.rs` — Closed-loop PID control with anti-windup, deadband
- `src/distributed.rs` — Distributed bridge node management, message routing
- `src/script.rs` — Recording/playback scripting with loop, speed control
- `src/ble.rs` — BLE GATT protocol adapter, device scanning

## Key Traits
- `Protocol` — Pluggable transport backend (connect, scan, scalar/linear/rotate_cmd, stop)
- `ActuatorType` — Universal actuator enum (Vibrate, Linear, Heat, E-Stim, etc.)

## Safety Defaults
| Type | Max Intensity | Ramp Rate | Auto-Shutoff |
|------|--------------|-----------|--------------|
| Vibrate | 1.0 | 10.0/s | — |
| Heat | 0.7 | 0.5/s | 5 min |
| E-Stim | 0.5 | 0.3/s | 1 min |
| Constrict | 0.8 | 1.0/s | — |

## Quality
| Metric | Value |
|--------|-------|
| clippy (pedantic+nursery) | 0 warnings |
| Tests | 262 |
| fmt | clean |

## Dependencies
- tokio (async runtime)
- serde/serde_json (serialization)
- thiserror (error types)
- tracing (logging)
- tokio-tungstenite (WebSocket, optional)
- rumqttc (MQTT, optional)
- reqwest (HTTP, optional)
