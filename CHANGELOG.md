# Changelog

## [0.1.0] - 2026-03-05

### Added
- Protocol trait with 5 adapters: Buttplug.io, MQTT, REST, OSC, WebSocket
- Device abstraction: ActuatorType, Device, DeviceManager, DeviceMapping
- Safety layer: IntensityLimiter (hard cap + soft compression), per-type limits
- EmergencyStop: lock-free, broadcast-based global panic button
- GradualRamp: configurable curve (linear, ease-in, ease-out, ease-in-out)
- SignalBridge: multi-source weighted fusion, smoothing, speed limiting
- MultiMapper: per-device scaling, inversion, delay, source filtering
- Integration tests covering full pipeline
