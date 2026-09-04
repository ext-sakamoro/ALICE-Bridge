#![no_main]
//! Fuzz target: NodeRegistry device routing + heartbeat lifecycle.
//!
//! 攻撃者制御の (node_id, endpoint, device_id, command, payload, now) 列で
//! `register` / `heartbeat` / `assign_device` / `route_for` / `route_message` /
//! `least_loaded` / `check_timeouts` / `drain` が panic せず終端することを検証。
//! NaN / Inf の now / max_devices=0 / 空文字列 / 未登録 ID lookup でも安全。

use alice_bridge::{NodeInfo, NodeRegistry};
use arbitrary::{Arbitrary, Unstructured};
use libfuzzer_sys::fuzz_target;

#[derive(Debug, Arbitrary)]
enum Op {
    Register {
        id: String,
        endpoint: String,
        max_devices: u32,
    },
    Unregister {
        node_id: String,
    },
    Heartbeat {
        node_id: String,
        now_bits: u64,
    },
    CheckTimeouts {
        now_bits: u64,
    },
    AssignDevice {
        device_id: String,
        node_id: String,
    },
    RouteFor {
        device_id: String,
    },
    RouteMessage {
        device_id: String,
        command: String,
        payload: String,
        now_bits: u64,
    },
    LeastLoaded,
    GetNode {
        node_id: String,
    },
    Drain {
        node_id: String,
    },
    Counts,
    CanAccept {
        id: String,
        endpoint: String,
        max_devices: u32,
    },
}

#[derive(Debug, Arbitrary)]
struct FuzzInput {
    heartbeat_timeout_bits: u64,
    ops: Vec<Op>,
}

fn bounded_str(s: &str, limit: usize) -> bool {
    s.len() <= limit
}

fuzz_target!(|data: &[u8]| {
    let mut u = Unstructured::new(data);
    let Ok(input) = FuzzInput::arbitrary(&mut u) else {
        return;
    };

    // OOM 予防
    if input.ops.len() > 512 {
        return;
    }

    let heartbeat_timeout = f64::from_bits(input.heartbeat_timeout_bits);
    let mut registry = NodeRegistry::new(heartbeat_timeout);

    for op in &input.ops {
        match op {
            Op::Register {
                id,
                endpoint,
                max_devices,
            } => {
                if !bounded_str(id, 256) || !bounded_str(endpoint, 512) {
                    continue;
                }
                let node = NodeInfo::new(id, endpoint, *max_devices);
                registry.register(node);
            }
            Op::Unregister { node_id } => {
                if !bounded_str(node_id, 256) {
                    continue;
                }
                registry.unregister(node_id);
            }
            Op::Heartbeat { node_id, now_bits } => {
                if !bounded_str(node_id, 256) {
                    continue;
                }
                registry.heartbeat(node_id, f64::from_bits(*now_bits));
            }
            Op::CheckTimeouts { now_bits } => {
                registry.check_timeouts(f64::from_bits(*now_bits));
            }
            Op::AssignDevice { device_id, node_id } => {
                if !bounded_str(device_id, 256) || !bounded_str(node_id, 256) {
                    continue;
                }
                let _ = registry.assign_device(device_id, node_id);
            }
            Op::RouteFor { device_id } => {
                if !bounded_str(device_id, 256) {
                    continue;
                }
                let _ = registry.route_for(device_id);
            }
            Op::RouteMessage {
                device_id,
                command,
                payload,
                now_bits,
            } => {
                if !bounded_str(device_id, 256)
                    || !bounded_str(command, 512)
                    || !bounded_str(payload, 4096)
                {
                    continue;
                }
                let msg =
                    registry.route_message(device_id, command, payload, f64::from_bits(*now_bits));
                if let Some(m) = msg {
                    let _ = m.target_node.len();
                    let _ = m.device_id.len();
                    let _ = m.command.len();
                    let _ = m.payload.len();
                    let _ = m.timestamp;
                }
            }
            Op::LeastLoaded => {
                let _ = registry.least_loaded();
            }
            Op::GetNode { node_id } => {
                if !bounded_str(node_id, 256) {
                    continue;
                }
                let _ = registry.get_node(node_id);
            }
            Op::Drain { node_id } => {
                if !bounded_str(node_id, 256) {
                    continue;
                }
                registry.drain(node_id);
            }
            Op::Counts => {
                let _ = registry.online_count();
                let _ = registry.total_count();
            }
            Op::CanAccept {
                id,
                endpoint,
                max_devices,
            } => {
                if !bounded_str(id, 256) || !bounded_str(endpoint, 512) {
                    continue;
                }
                let node = NodeInfo::new(id, endpoint, *max_devices);
                let _ = node.can_accept();
                let _ = node.load();
            }
        }
    }
});
