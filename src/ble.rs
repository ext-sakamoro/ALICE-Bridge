//! BLE プロトコルアダプター — GATT 特性読み書き、デバイススキャン
//!
//! Bluetooth Low Energy デバイスとの通信を抽象化する。

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// BLE デバイスの接続状態。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BleState {
    /// 未接続。
    Disconnected,
    /// 接続中。
    Connecting,
    /// 接続済み。
    Connected,
}

/// GATT 特性のプロパティ。
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CharProperties {
    /// 読み取り可能。
    pub read: bool,
    /// 書き込み可能。
    pub write: bool,
    /// 通知可能。
    pub notify: bool,
    /// 書き込み応答なし。
    pub write_without_response: bool,
}

impl Default for CharProperties {
    fn default() -> Self {
        Self {
            read: true,
            write: false,
            notify: false,
            write_without_response: false,
        }
    }
}

/// GATT 特性。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GattCharacteristic {
    /// 特性 UUID。
    pub uuid: String,
    /// サービス UUID。
    pub service_uuid: String,
    /// プロパティ。
    pub properties: CharProperties,
    /// 現在の値。
    pub value: Vec<u8>,
}

impl GattCharacteristic {
    /// 新しい特性を作成。
    #[must_use]
    pub fn new(uuid: &str, service_uuid: &str, properties: CharProperties) -> Self {
        Self {
            uuid: uuid.to_string(),
            service_uuid: service_uuid.to_string(),
            properties,
            value: Vec::new(),
        }
    }

    /// 値のサイズ (バイト)。
    #[must_use]
    pub const fn value_len(&self) -> usize {
        self.value.len()
    }
}

/// スキャン結果のエントリー。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanEntry {
    /// デバイス名。
    pub name: String,
    /// デバイスアドレス (MAC等)。
    pub address: String,
    /// RSSI (dBm)。
    pub rssi: i16,
    /// アドバタイズされたサービスUUID。
    pub service_uuids: Vec<String>,
    /// スキャン時刻。
    pub timestamp: f64,
}

/// BLE デバイスモデル。
#[derive(Debug, Clone)]
pub struct BleDevice {
    /// デバイス名。
    pub name: String,
    /// デバイスアドレス。
    pub address: String,
    /// 接続状態。
    pub state: BleState,
    /// GATT 特性マップ (UUID → 特性)。
    pub characteristics: HashMap<String, GattCharacteristic>,
    /// MTU サイズ。
    pub mtu: u16,
}

impl BleDevice {
    /// 新しい BLE デバイスを作成。
    #[must_use]
    pub fn new(name: &str, address: &str) -> Self {
        Self {
            name: name.to_string(),
            address: address.to_string(),
            state: BleState::Disconnected,
            characteristics: HashMap::new(),
            mtu: 23, // BLE デフォルト MTU
        }
    }

    /// 特性を登録。
    pub fn add_characteristic(&mut self, char: GattCharacteristic) {
        self.characteristics.insert(char.uuid.clone(), char);
    }

    /// 特性を読み取り。
    #[must_use]
    pub fn read_characteristic(&self, uuid: &str) -> Option<&[u8]> {
        let c = self.characteristics.get(uuid)?;
        if !c.properties.read {
            return None;
        }
        Some(&c.value)
    }

    /// 特性に書き込み。
    pub fn write_characteristic(&mut self, uuid: &str, data: &[u8]) -> bool {
        if let Some(c) = self.characteristics.get_mut(uuid) {
            if c.properties.write || c.properties.write_without_response {
                c.value = data.to_vec();
                return true;
            }
        }
        false
    }

    /// 接続済みか。
    #[must_use]
    pub fn is_connected(&self) -> bool {
        self.state == BleState::Connected
    }

    /// 特性数。
    #[must_use]
    pub fn characteristic_count(&self) -> usize {
        self.characteristics.len()
    }
}

/// BLE マネージャー — デバイスのスキャン・接続管理。
#[derive(Debug, Default)]
pub struct BleManager {
    /// 接続済み/既知デバイス (アドレス → デバイス)。
    devices: HashMap<String, BleDevice>,
    /// スキャン結果。
    scan_results: Vec<ScanEntry>,
    /// スキャン中か。
    scanning: bool,
}

impl BleManager {
    /// 新しいマネージャーを作成。
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// スキャン開始。
    pub fn start_scan(&mut self) {
        self.scanning = true;
        self.scan_results.clear();
    }

    /// スキャン停止。
    pub const fn stop_scan(&mut self) {
        self.scanning = false;
    }

    /// スキャン中か。
    #[must_use]
    pub const fn is_scanning(&self) -> bool {
        self.scanning
    }

    /// スキャン結果を追加 (BLE アダプターからのコールバック想定)。
    pub fn add_scan_result(&mut self, entry: ScanEntry) {
        if self.scanning {
            // 同一アドレスは最新で上書き
            if let Some(pos) = self
                .scan_results
                .iter()
                .position(|e| e.address == entry.address)
            {
                self.scan_results[pos] = entry;
            } else {
                self.scan_results.push(entry);
            }
        }
    }

    /// スキャン結果を取得。
    #[must_use]
    pub fn scan_results(&self) -> &[ScanEntry] {
        &self.scan_results
    }

    /// RSSI でソートしたスキャン結果 (強い順)。
    #[must_use]
    pub fn scan_results_sorted(&self) -> Vec<&ScanEntry> {
        let mut sorted: Vec<&ScanEntry> = self.scan_results.iter().collect();
        sorted.sort_by(|a, b| b.rssi.cmp(&a.rssi));
        sorted
    }

    /// デバイスを接続状態で登録。
    pub fn connect(&mut self, mut device: BleDevice) {
        device.state = BleState::Connected;
        self.devices.insert(device.address.clone(), device);
    }

    /// デバイスを切断。
    pub fn disconnect(&mut self, address: &str) {
        if let Some(dev) = self.devices.get_mut(address) {
            dev.state = BleState::Disconnected;
        }
    }

    /// デバイスを取得。
    #[must_use]
    pub fn get_device(&self, address: &str) -> Option<&BleDevice> {
        self.devices.get(address)
    }

    /// デバイスを可変で取得。
    pub fn get_device_mut(&mut self, address: &str) -> Option<&mut BleDevice> {
        self.devices.get_mut(address)
    }

    /// 接続済みデバイス数。
    #[must_use]
    pub fn connected_count(&self) -> usize {
        self.devices.values().filter(|d| d.is_connected()).count()
    }

    /// 全デバイス数。
    #[must_use]
    pub fn device_count(&self) -> usize {
        self.devices.len()
    }

    /// デバイスを削除。
    pub fn remove_device(&mut self, address: &str) {
        self.devices.remove(address);
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ble_device_new() {
        let dev = BleDevice::new("Sensor", "AA:BB:CC:DD:EE:FF");
        assert_eq!(dev.name, "Sensor");
        assert_eq!(dev.state, BleState::Disconnected);
        assert_eq!(dev.mtu, 23);
        assert_eq!(dev.characteristic_count(), 0);
    }

    #[test]
    fn ble_device_add_characteristic() {
        let mut dev = BleDevice::new("Sensor", "AA:BB:CC:DD:EE:FF");
        let props = CharProperties {
            read: true,
            write: true,
            ..Default::default()
        };
        dev.add_characteristic(GattCharacteristic::new("0x2A19", "0x180F", props));
        assert_eq!(dev.characteristic_count(), 1);
    }

    #[test]
    fn ble_device_read_write() {
        let mut dev = BleDevice::new("Sensor", "AA:BB:CC:DD:EE:FF");
        let props = CharProperties {
            read: true,
            write: true,
            ..Default::default()
        };
        dev.add_characteristic(GattCharacteristic::new("0x2A19", "0x180F", props));

        assert!(dev.write_characteristic("0x2A19", &[0x42]));
        let val = dev.read_characteristic("0x2A19").unwrap();
        assert_eq!(val, &[0x42]);
    }

    #[test]
    fn ble_device_read_only() {
        let mut dev = BleDevice::new("Sensor", "AA:BB:CC:DD:EE:FF");
        let props = CharProperties {
            read: true,
            write: false,
            ..Default::default()
        };
        dev.add_characteristic(GattCharacteristic::new("0x2A19", "0x180F", props));
        assert!(!dev.write_characteristic("0x2A19", &[0x42]));
    }

    #[test]
    fn ble_device_write_without_response() {
        let mut dev = BleDevice::new("Sensor", "AA:BB:CC:DD:EE:FF");
        let props = CharProperties {
            read: false,
            write: false,
            write_without_response: true,
            ..Default::default()
        };
        dev.add_characteristic(GattCharacteristic::new("0x2A19", "0x180F", props));
        assert!(dev.write_characteristic("0x2A19", &[0x01, 0x02]));
        // 読み取り不可
        assert!(dev.read_characteristic("0x2A19").is_none());
    }

    #[test]
    fn ble_device_nonexistent_char() {
        let dev = BleDevice::new("Sensor", "AA:BB:CC:DD:EE:FF");
        assert!(dev.read_characteristic("0xFFFF").is_none());
    }

    #[test]
    fn gatt_characteristic_value_len() {
        let mut c = GattCharacteristic::new("0x2A19", "0x180F", CharProperties::default());
        assert_eq!(c.value_len(), 0);
        c.value = vec![1, 2, 3];
        assert_eq!(c.value_len(), 3);
    }

    #[test]
    fn manager_scan_flow() {
        let mut mgr = BleManager::new();
        assert!(!mgr.is_scanning());

        mgr.start_scan();
        assert!(mgr.is_scanning());

        mgr.add_scan_result(ScanEntry {
            name: "Dev1".into(),
            address: "AA:BB:CC:DD:EE:01".into(),
            rssi: -60,
            service_uuids: vec!["0x180F".into()],
            timestamp: 1.0,
        });
        mgr.add_scan_result(ScanEntry {
            name: "Dev2".into(),
            address: "AA:BB:CC:DD:EE:02".into(),
            rssi: -40,
            service_uuids: vec![],
            timestamp: 1.1,
        });

        assert_eq!(mgr.scan_results().len(), 2);

        mgr.stop_scan();
        assert!(!mgr.is_scanning());
    }

    #[test]
    fn manager_scan_dedup() {
        let mut mgr = BleManager::new();
        mgr.start_scan();
        mgr.add_scan_result(ScanEntry {
            name: "Dev1".into(),
            address: "AA:BB:CC:DD:EE:01".into(),
            rssi: -60,
            service_uuids: vec![],
            timestamp: 1.0,
        });
        mgr.add_scan_result(ScanEntry {
            name: "Dev1".into(),
            address: "AA:BB:CC:DD:EE:01".into(),
            rssi: -50, // 更新
            service_uuids: vec![],
            timestamp: 2.0,
        });
        assert_eq!(mgr.scan_results().len(), 1);
        assert_eq!(mgr.scan_results()[0].rssi, -50);
    }

    #[test]
    fn manager_scan_sorted_by_rssi() {
        let mut mgr = BleManager::new();
        mgr.start_scan();
        mgr.add_scan_result(ScanEntry {
            name: "Far".into(),
            address: "01".into(),
            rssi: -80,
            service_uuids: vec![],
            timestamp: 1.0,
        });
        mgr.add_scan_result(ScanEntry {
            name: "Close".into(),
            address: "02".into(),
            rssi: -30,
            service_uuids: vec![],
            timestamp: 1.0,
        });
        let sorted = mgr.scan_results_sorted();
        assert_eq!(sorted[0].name, "Close");
        assert_eq!(sorted[1].name, "Far");
    }

    #[test]
    fn manager_scan_not_scanning() {
        let mut mgr = BleManager::new();
        mgr.add_scan_result(ScanEntry {
            name: "Dev1".into(),
            address: "01".into(),
            rssi: -60,
            service_uuids: vec![],
            timestamp: 1.0,
        });
        assert!(mgr.scan_results().is_empty());
    }

    #[test]
    fn manager_connect_disconnect() {
        let mut mgr = BleManager::new();
        let dev = BleDevice::new("Sensor", "AA:BB:CC:DD:EE:FF");
        mgr.connect(dev);
        assert_eq!(mgr.connected_count(), 1);
        assert_eq!(mgr.device_count(), 1);
        assert!(mgr.get_device("AA:BB:CC:DD:EE:FF").unwrap().is_connected());

        mgr.disconnect("AA:BB:CC:DD:EE:FF");
        assert_eq!(mgr.connected_count(), 0);
        assert_eq!(mgr.device_count(), 1); // まだ登録はある
    }

    #[test]
    fn manager_remove_device() {
        let mut mgr = BleManager::new();
        mgr.connect(BleDevice::new("Sensor", "AA:BB:CC:DD:EE:FF"));
        mgr.remove_device("AA:BB:CC:DD:EE:FF");
        assert_eq!(mgr.device_count(), 0);
    }

    #[test]
    fn manager_get_device_mut() {
        let mut mgr = BleManager::new();
        mgr.connect(BleDevice::new("Sensor", "AA:BB:CC:DD:EE:FF"));
        let dev = mgr.get_device_mut("AA:BB:CC:DD:EE:FF").unwrap();
        dev.mtu = 512;
        assert_eq!(mgr.get_device("AA:BB:CC:DD:EE:FF").unwrap().mtu, 512);
    }

    #[test]
    fn char_properties_default() {
        let props = CharProperties::default();
        assert!(props.read);
        assert!(!props.write);
        assert!(!props.notify);
        assert!(!props.write_without_response);
    }
}
