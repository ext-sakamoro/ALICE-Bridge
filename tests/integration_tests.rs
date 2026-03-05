use alice_bridge::bridge::SignalBridge;
use alice_bridge::device::mapping::MultiMapper;
use alice_bridge::device::{
    Actuator, ActuatorType, Device, DeviceId, DeviceManager, DeviceMapping,
};
use alice_bridge::safety::ramp::RampCurve;
use alice_bridge::safety::{EmergencyStop, GradualRamp, IntensityLimiter, SafetyLimits};
use std::collections::HashMap;

// ============================================================================
// Device Manager
// ============================================================================

#[test]
fn device_manager_full_lifecycle() {
    let mut mgr = DeviceManager::new();
    assert_eq!(mgr.count(), 0);

    // Register devices from different protocols
    mgr.register(Device {
        id: DeviceId("buttplug:0".into()),
        name: "The Handy".into(),
        protocol: "buttplug".into(),
        actuators: vec![Actuator {
            index: 0,
            actuator_type: ActuatorType::Linear,
            description: "Primary".into(),
            step_count: 100,
        }],
        metadata: HashMap::new(),
    });
    mgr.register(Device {
        id: DeviceId("mqtt:esp32".into()),
        name: "ESP32 Haptic".into(),
        protocol: "mqtt".into(),
        actuators: vec![
            Actuator {
                index: 0,
                actuator_type: ActuatorType::Vibrate,
                description: "Motor".into(),
                step_count: 256,
            },
            Actuator {
                index: 1,
                actuator_type: ActuatorType::Heat,
                description: "Heater".into(),
                step_count: 100,
            },
        ],
        metadata: HashMap::new(),
    });

    assert_eq!(mgr.count(), 2);
    assert_eq!(mgr.find_by_type(ActuatorType::Linear).len(), 1);
    assert_eq!(mgr.find_by_type(ActuatorType::Vibrate).len(), 1);
    assert_eq!(mgr.find_by_type(ActuatorType::Heat).len(), 1);
    assert_eq!(mgr.find_by_protocol("buttplug").len(), 1);
    assert_eq!(mgr.find_by_protocol("mqtt").len(), 1);

    // Unregister
    mgr.unregister("buttplug:0");
    assert_eq!(mgr.count(), 1);
    assert!(mgr.get("buttplug:0").is_none());
    assert!(mgr.get("mqtt:esp32").is_some());
}

// ============================================================================
// Multi-device Mapping
// ============================================================================

#[test]
fn multi_mapper_with_delay() {
    let mut mm = MultiMapper::new();
    mm.add(DeviceMapping {
        device_id: "dev:0".into(),
        label: "Primary".into(),
        delay_ms: 100,
        ..Default::default()
    });

    // Buffer values with timestamps
    let _ = mm.compute(0.5, "all", 0);
    let _ = mm.compute(0.7, "all", 50);
    let _ = mm.compute(0.9, "all", 100);

    // At t=200, we should get the value from t=100 (delay=100ms)
    let cmds = mm.compute(0.3, "all", 200);
    assert_eq!(cmds.len(), 1);
    // Delayed value should be from earlier
    assert!(cmds[0].position >= 0.0 && cmds[0].position <= 1.0);
}

// ============================================================================
// Signal Bridge
// ============================================================================

#[test]
fn signal_bridge_multi_source() {
    let mut bridge = SignalBridge::new(3, 500.0, 0.0, 1.0, 50);
    bridge.add_source("osc", 0.6);
    bridge.add_source("capture", 0.4);

    // Feed both sources
    for i in 0..5 {
        let t = f64::from(i) * 0.033;
        bridge.update("osc", 0.8, t);
        bridge.update("capture", 0.4, t);
    }

    let action = bridge.tick(0.2);
    assert!(action.position > 0.0);
    assert!(action.position <= 1.0);
    assert_eq!(bridge.active_sources().len(), 2);
}

// ============================================================================
// Safety: Limiter + Emergency Stop + Ramp
// ============================================================================

#[test]
fn safety_pipeline() {
    // 1. Gradual ramp (5 seconds, linear)
    let mut ramp = GradualRamp::new(5.0, RampCurve::Linear);
    ramp.start(0.0);

    // 2. Intensity limiter with soft compression
    let mut limiter = IntensityLimiter::new(0.9, Some(0.7), 0.3);

    // 3. Emergency stop
    let estop = EmergencyStop::new();
    estop.arm();

    // At t=2.5 (midpoint of ramp), input=1.0
    let ramped = ramp.apply(1.0, 2.5);
    assert!((ramped - 0.5).abs() < 1e-6); // Linear ramp at 50%

    let capped = limiter.apply(ramped);
    assert!(capped <= 0.9);

    // Trigger emergency stop
    estop.trigger("test");
    assert!(estop.is_triggered());
}

#[test]
fn heat_auto_shutoff() {
    let mut limiter = IntensityLimiter::new(1.0, None, 0.3);
    limiter.set_limits(
        ActuatorType::Heat,
        SafetyLimits {
            max_intensity: 0.7,
            ramp_rate: 10.0,
            cooldown_ms: 0,
            auto_shutoff_ms: 1000, // 1 second for test
        },
    );

    // Start heat
    let v = limiter.apply_typed(0.5, ActuatorType::Heat, 0, 0.0);
    assert!(v > 0.0);

    // After auto-shutoff
    let v = limiter.apply_typed(0.5, ActuatorType::Heat, 0, 2.0);
    assert!((v - 0.0).abs() < 1e-6);
}

// ============================================================================
// End-to-end: Input -> Safety -> Bridge -> Mapping
// ============================================================================

#[test]
fn end_to_end_pipeline() {
    // Setup
    let mut ramp = GradualRamp::new(2.0, RampCurve::EaseInOut);
    let mut limiter = IntensityLimiter::new(0.95, Some(0.8), 0.3);
    let mut bridge = SignalBridge::new(3, 500.0, 0.0, 1.0, 50);
    bridge.add_source("input", 1.0);
    let mut mapper = MultiMapper::new();
    mapper.add(DeviceMapping {
        device_id: "dev:0".into(),
        label: "Primary".into(),
        ..Default::default()
    });
    mapper.add(DeviceMapping {
        device_id: "dev:1".into(),
        label: "Secondary".into(),
        scale: 0.5,
        invert: true,
        ..Default::default()
    });

    ramp.start(0.0);

    // Simulate 1 second at 20Hz
    for i in 0..20 {
        let t = f64::from(i) * 0.05;
        let raw_intensity = 0.8;

        // Safety pipeline
        let ramped = ramp.apply(raw_intensity, t);
        let capped = limiter.apply(ramped);

        // Signal bridge
        bridge.update("input", capped, t);
        let action = bridge.tick(t);

        // Device mapping
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let ts_ms = (t * 1000.0) as u64;
        let commands = mapper.compute(action.position, "all", ts_ms);

        assert_eq!(commands.len(), 2);
        for cmd in &commands {
            assert!(cmd.position >= 0.0 && cmd.position <= 1.0);
        }
    }

    assert_eq!(bridge.tick_count(), 20);
}
