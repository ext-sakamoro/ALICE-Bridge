# ALICE-Bridge

Universal hardware bridge — protocol-agnostic device communication layer.

## Overview

ALICE-Bridge provides a unified abstraction for communicating with any hardware device,
regardless of the underlying protocol. Connect to 750+ devices via Buttplug.io, IoT
devices via MQTT, REST API devices, OSC-compatible applications, or raw WebSocket endpoints
— all through a single, type-safe Rust API.

## Architecture

```
Application (FunForge, VRChat, IoT controller, ...)
    |
    v
+-------------------------------------+
|  ALICE-Bridge                        |
|  +-------------+ +-----------------+|
|  | Device Mgr  | | Safety Layer    ||
|  +------+------+ +--------+--------+|
|         |                  |         |
|  +------v------------------v--------+|
|  |     Signal Bridge                ||
|  +------+---------------------------+|
|         |                            |
|  +------v---------------------------+|
|  | Protocol Adapters                ||
|  | Buttplug | MQTT | REST | OSC | WS||
|  +----------------------------------+|
+--------------------------------------+
    |
    v
Hardware (750+ devices)
```

## Protocol Adapters

| Protocol | Transport | Use Case |
|----------|-----------|----------|
| **Buttplug.io** | WebSocket | 750+ consumer haptic devices via Intiface Central |
| **MQTT** | TCP | IoT devices (ESP32, Arduino, Raspberry Pi) |
| **REST** | HTTP | Devices with REST APIs (The Handy v2, etc.) |
| **OSC** | UDP | VRChat, TouchDesigner, audio/visual apps |
| **WebSocket** | WebSocket | Custom firmware with WS servers |

## Features

- `websocket` — Buttplug.io + raw WebSocket adapters (default)
- `mqtt` — MQTT adapter (default)
- `rest` — REST API adapter (default)
- `full` — All features

## Safety Layer

- **IntensityLimiter** — Hard cap + soft-knee compression
- **Per-type safety** — Automatic limits for Heat (0.7), E-Stim (0.5), etc.
- **Ramp rate limiting** — Prevents sudden intensity spikes
- **Auto-shutoff** — Heat: 5min, E-Stim: 1min
- **EmergencyStop** — Global panic button, lock-free, broadcast to all subscribers
- **GradualRamp** — Smooth intensity increase at session start

## Usage

```rust
use alice_bridge::{
    DeviceManager, SignalBridge, IntensityLimiter, EmergencyStop,
    device::DeviceMapping,
};

// Create safety pipeline
let mut limiter = IntensityLimiter::new(0.9, Some(0.7), 0.3);
let estop = EmergencyStop::new();
estop.arm();

// Create signal bridge
let mut bridge = SignalBridge::new(5, 500.0, 0.0, 1.0, 50);
bridge.add_source("osc", 0.6);
bridge.add_source("capture", 0.4);

// Process input
bridge.update("osc", 0.8, now);
let action = bridge.tick(now);
let safe_intensity = limiter.apply(action.position);
```

## Quality

| Metric | Value |
|--------|-------|
| Tests | 108 (102 unit + 6 integration) |
| clippy (pedantic+nursery) | 0 warnings |
| fmt | clean |

## Part of ALICE Ecosystem

ALICE-Bridge is a component of the [ALICE Ecosystem](https://github.com/ext-sakamoro/ALICE-Eco-System) — 59 components covering compression, networking, compute, and more.

## License

**Dual License: AGPL-3.0 + Commercial**

- **Open Source (AGPL-3.0)**: Free for personal use, hobby projects, VRChat communities, and any project that complies with AGPL-3.0 (full source disclosure).
- **Commercial License**: If your organization cannot comply with AGPL-3.0 (e.g., proprietary embedded systems, closed-source cloud services, robotics), contact us for a commercial license.

See [LICENSE](LICENSE) for the full AGPL-3.0 text.
