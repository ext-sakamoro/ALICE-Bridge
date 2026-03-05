//! 分散ブリッジ — 複数ノード管理、メッセージルーティング
//!
//! 複数のブリッジノードを統合して、デバイス管理を分散化する。

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// ノード状態。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeStatus {
    /// オンライン。
    Online,
    /// オフライン。
    Offline,
    /// ドレイン中 (新規接続拒否)。
    Draining,
}

/// ノード情報。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeInfo {
    /// ノードID。
    pub id: String,
    /// エンドポイント (host:port)。
    pub endpoint: String,
    /// ノード状態。
    pub status: NodeStatus,
    /// 接続デバイス数。
    pub device_count: u32,
    /// 最大デバイス数。
    pub max_devices: u32,
    /// 最終ハートビート時刻 (秒)。
    pub last_heartbeat: f64,
    /// メタデータ。
    pub metadata: HashMap<String, String>,
}

impl NodeInfo {
    /// 新しいノードを作成。
    #[must_use]
    pub fn new(id: &str, endpoint: &str, max_devices: u32) -> Self {
        Self {
            id: id.to_string(),
            endpoint: endpoint.to_string(),
            status: NodeStatus::Online,
            device_count: 0,
            max_devices,
            last_heartbeat: 0.0,
            metadata: HashMap::new(),
        }
    }

    /// デバイス受け入れ可能か。
    #[must_use]
    pub fn can_accept(&self) -> bool {
        self.status == NodeStatus::Online && self.device_count < self.max_devices
    }

    /// 負荷率 (0.0–1.0)。
    #[must_use]
    pub fn load(&self) -> f64 {
        if self.max_devices == 0 {
            return 1.0;
        }
        f64::from(self.device_count) / f64::from(self.max_devices)
    }
}

/// ルーティングメッセージ。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteMessage {
    /// 宛先ノードID。
    pub target_node: String,
    /// 宛先デバイスID。
    pub device_id: String,
    /// コマンドタイプ。
    pub command: String,
    /// ペイロード (JSON)。
    pub payload: String,
    /// タイムスタンプ。
    pub timestamp: f64,
}

/// ノードレジストリ — 分散ブリッジノード管理。
#[derive(Debug, Default)]
pub struct NodeRegistry {
    nodes: HashMap<String, NodeInfo>,
    /// デバイスID → ノードIDのマッピング。
    device_routes: HashMap<String, String>,
    /// ハートビートタイムアウト (秒)。
    heartbeat_timeout: f64,
}

impl NodeRegistry {
    /// 新しいレジストリを作成。
    #[must_use]
    pub fn new(heartbeat_timeout: f64) -> Self {
        Self {
            nodes: HashMap::new(),
            device_routes: HashMap::new(),
            heartbeat_timeout,
        }
    }

    /// ノードを登録。
    pub fn register(&mut self, node: NodeInfo) {
        self.nodes.insert(node.id.clone(), node);
    }

    /// ノードを削除。
    pub fn unregister(&mut self, node_id: &str) {
        self.nodes.remove(node_id);
        // このノードに紐づくデバイスルートを削除
        self.device_routes.retain(|_, nid| nid != node_id);
    }

    /// ハートビートを受信。
    pub fn heartbeat(&mut self, node_id: &str, now: f64) {
        if let Some(node) = self.nodes.get_mut(node_id) {
            node.last_heartbeat = now;
            if node.status == NodeStatus::Offline {
                node.status = NodeStatus::Online;
            }
        }
    }

    /// タイムアウトしたノードをオフラインに。
    pub fn check_timeouts(&mut self, now: f64) {
        for node in self.nodes.values_mut() {
            if node.status == NodeStatus::Online
                && now - node.last_heartbeat > self.heartbeat_timeout
            {
                node.status = NodeStatus::Offline;
            }
        }
    }

    /// デバイスをノードに割り当て。
    pub fn assign_device(&mut self, device_id: &str, node_id: &str) -> bool {
        if let Some(node) = self.nodes.get_mut(node_id) {
            if node.can_accept() {
                node.device_count += 1;
                self.device_routes
                    .insert(device_id.to_string(), node_id.to_string());
                return true;
            }
        }
        false
    }

    /// デバイスのルーティング先ノードを取得。
    #[must_use]
    pub fn route_for(&self, device_id: &str) -> Option<&str> {
        self.device_routes.get(device_id).map(String::as_str)
    }

    /// メッセージをルーティング。
    #[must_use]
    pub fn route_message(
        &self,
        device_id: &str,
        command: &str,
        payload: &str,
        now: f64,
    ) -> Option<RouteMessage> {
        let node_id = self.device_routes.get(device_id)?;
        let node = self.nodes.get(node_id)?;
        if node.status != NodeStatus::Online {
            return None;
        }
        Some(RouteMessage {
            target_node: node_id.clone(),
            device_id: device_id.to_string(),
            command: command.to_string(),
            payload: payload.to_string(),
            timestamp: now,
        })
    }

    /// 最も負荷の低いオンラインノードを選択。
    #[must_use]
    pub fn least_loaded(&self) -> Option<&str> {
        self.nodes
            .values()
            .filter(|n| n.can_accept())
            .min_by(|a, b| {
                a.load()
                    .partial_cmp(&b.load())
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|n| n.id.as_str())
    }

    /// オンラインノード数。
    #[must_use]
    pub fn online_count(&self) -> usize {
        self.nodes
            .values()
            .filter(|n| n.status == NodeStatus::Online)
            .count()
    }

    /// 全ノード数。
    #[must_use]
    pub fn total_count(&self) -> usize {
        self.nodes.len()
    }

    /// ノード情報を取得。
    #[must_use]
    pub fn get_node(&self, node_id: &str) -> Option<&NodeInfo> {
        self.nodes.get(node_id)
    }

    /// ノードをドレインモードに。
    pub fn drain(&mut self, node_id: &str) {
        if let Some(node) = self.nodes.get_mut(node_id) {
            node.status = NodeStatus::Draining;
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn node_new() {
        let node = NodeInfo::new("n1", "localhost:9000", 100);
        assert_eq!(node.id, "n1");
        assert_eq!(node.status, NodeStatus::Online);
        assert!(node.can_accept());
    }

    #[test]
    fn node_load() {
        let mut node = NodeInfo::new("n1", "localhost:9000", 100);
        assert!((node.load() - 0.0).abs() < 1e-10);
        node.device_count = 50;
        assert!((node.load() - 0.5).abs() < 1e-10);
    }

    #[test]
    fn node_load_zero_max() {
        let node = NodeInfo::new("n1", "localhost:9000", 0);
        assert!((node.load() - 1.0).abs() < 1e-10);
    }

    #[test]
    fn node_can_accept_full() {
        let mut node = NodeInfo::new("n1", "localhost:9000", 10);
        node.device_count = 10;
        assert!(!node.can_accept());
    }

    #[test]
    fn node_can_accept_offline() {
        let mut node = NodeInfo::new("n1", "localhost:9000", 10);
        node.status = NodeStatus::Offline;
        assert!(!node.can_accept());
    }

    #[test]
    fn registry_register_and_get() {
        let mut reg = NodeRegistry::new(10.0);
        reg.register(NodeInfo::new("n1", "localhost:9000", 100));
        assert_eq!(reg.total_count(), 1);
        assert_eq!(reg.online_count(), 1);
        assert!(reg.get_node("n1").is_some());
    }

    #[test]
    fn registry_unregister() {
        let mut reg = NodeRegistry::new(10.0);
        reg.register(NodeInfo::new("n1", "localhost:9000", 100));
        reg.unregister("n1");
        assert_eq!(reg.total_count(), 0);
    }

    #[test]
    fn registry_heartbeat_and_timeout() {
        let mut reg = NodeRegistry::new(5.0);
        let mut node = NodeInfo::new("n1", "localhost:9000", 100);
        node.last_heartbeat = 1.0;
        reg.register(node);

        // At t=7 (6 seconds since last heartbeat, timeout=5)
        reg.check_timeouts(7.0);
        assert_eq!(reg.get_node("n1").unwrap().status, NodeStatus::Offline);

        // Heartbeat brings it back
        reg.heartbeat("n1", 8.0);
        assert_eq!(reg.get_node("n1").unwrap().status, NodeStatus::Online);
    }

    #[test]
    fn registry_assign_device() {
        let mut reg = NodeRegistry::new(10.0);
        reg.register(NodeInfo::new("n1", "localhost:9000", 100));
        assert!(reg.assign_device("dev1", "n1"));
        assert_eq!(reg.route_for("dev1"), Some("n1"));
        assert_eq!(reg.get_node("n1").unwrap().device_count, 1);
    }

    #[test]
    fn registry_assign_to_full_node() {
        let mut reg = NodeRegistry::new(10.0);
        let mut node = NodeInfo::new("n1", "localhost:9000", 1);
        node.device_count = 1;
        reg.register(node);
        assert!(!reg.assign_device("dev2", "n1"));
    }

    #[test]
    fn registry_route_message() {
        let mut reg = NodeRegistry::new(10.0);
        reg.register(NodeInfo::new("n1", "localhost:9000", 100));
        reg.assign_device("dev1", "n1");
        let msg = reg.route_message("dev1", "vibrate", "{\"intensity\":0.5}", 1.0);
        assert!(msg.is_some());
        let msg = msg.unwrap();
        assert_eq!(msg.target_node, "n1");
        assert_eq!(msg.device_id, "dev1");
    }

    #[test]
    fn registry_route_offline_node() {
        let mut reg = NodeRegistry::new(10.0);
        let mut node = NodeInfo::new("n1", "localhost:9000", 100);
        node.status = NodeStatus::Offline;
        reg.register(node);
        reg.device_routes
            .insert("dev1".to_string(), "n1".to_string());
        let msg = reg.route_message("dev1", "vibrate", "{}", 1.0);
        assert!(msg.is_none());
    }

    #[test]
    fn registry_least_loaded() {
        let mut reg = NodeRegistry::new(10.0);
        let mut n1 = NodeInfo::new("n1", "localhost:9000", 100);
        n1.device_count = 80;
        let mut n2 = NodeInfo::new("n2", "localhost:9001", 100);
        n2.device_count = 20;
        reg.register(n1);
        reg.register(n2);
        assert_eq!(reg.least_loaded(), Some("n2"));
    }

    #[test]
    fn registry_drain() {
        let mut reg = NodeRegistry::new(10.0);
        reg.register(NodeInfo::new("n1", "localhost:9000", 100));
        reg.drain("n1");
        assert_eq!(reg.get_node("n1").unwrap().status, NodeStatus::Draining);
        assert!(!reg.get_node("n1").unwrap().can_accept());
    }

    #[test]
    fn registry_unregister_clears_routes() {
        let mut reg = NodeRegistry::new(10.0);
        reg.register(NodeInfo::new("n1", "localhost:9000", 100));
        reg.assign_device("dev1", "n1");
        reg.unregister("n1");
        assert!(reg.route_for("dev1").is_none());
    }
}
