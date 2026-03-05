//! センサーフィードバック入力 — 圧力、温度、加速度センサーの読み取り
//!
//! デバイスからのセンサーデータを統一モデルで管理する。

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// センサータイプ。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SensorType {
    /// 圧力 (N/cm²)。
    Pressure,
    /// 温度 (°C)。
    Temperature,
    /// 加速度 (m/s²)。
    Acceleration,
    /// 角速度 (rad/s)。
    Gyroscope,
    /// 心拍数 (bpm)。
    HeartRate,
    /// 近接 (0.0–1.0)。
    Proximity,
    /// カスタム。
    Custom,
}

impl SensorType {
    /// 文字列からパース。
    #[must_use]
    pub fn parse(s: &str) -> Self {
        match s {
            "Pressure" => Self::Pressure,
            "Temperature" => Self::Temperature,
            "Acceleration" => Self::Acceleration,
            "Gyroscope" => Self::Gyroscope,
            "HeartRate" => Self::HeartRate,
            "Proximity" => Self::Proximity,
            _ => Self::Custom,
        }
    }

    /// 文字列表現。
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Pressure => "Pressure",
            Self::Temperature => "Temperature",
            Self::Acceleration => "Acceleration",
            Self::Gyroscope => "Gyroscope",
            Self::HeartRate => "HeartRate",
            Self::Proximity => "Proximity",
            Self::Custom => "Custom",
        }
    }
}

/// センサー読み取り値。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SensorReading {
    /// センサーID。
    pub sensor_id: String,
    /// センサータイプ。
    pub sensor_type: SensorType,
    /// 読み取り値。
    pub value: f64,
    /// タイムスタンプ (秒、モノトニック)。
    pub timestamp: f64,
    /// 有効範囲下限。
    pub min_value: f64,
    /// 有効範囲上限。
    pub max_value: f64,
}

impl SensorReading {
    /// 正規化された値 (0.0–1.0)。
    #[must_use]
    pub fn normalized(&self) -> f64 {
        let range = self.max_value - self.min_value;
        if range.abs() < 1e-15 {
            return 0.0;
        }
        ((self.value - self.min_value) / range).clamp(0.0, 1.0)
    }
}

/// センサーレジストリ — 複数センサーを管理。
#[derive(Debug, Default)]
pub struct SensorRegistry {
    /// センサーID → 最新の読み取り値。
    readings: HashMap<String, SensorReading>,
    /// センサーID → 読み取り値の履歴 (リングバッファ)。
    history: HashMap<String, Vec<SensorReading>>,
    /// 履歴保持数。
    history_capacity: usize,
}

impl SensorRegistry {
    /// 新しいレジストリを作成。
    #[must_use]
    pub fn new(history_capacity: usize) -> Self {
        Self {
            readings: HashMap::new(),
            history: HashMap::new(),
            history_capacity: history_capacity.max(1),
        }
    }

    /// センサー読み取り値を更新。
    pub fn update(&mut self, reading: SensorReading) {
        let id = reading.sensor_id.clone();

        // 履歴に追加
        let hist = self.history.entry(id.clone()).or_default();
        if hist.len() >= self.history_capacity {
            hist.remove(0);
        }
        hist.push(reading.clone());

        // 最新値を更新
        self.readings.insert(id, reading);
    }

    /// センサーの最新値を取得。
    #[must_use]
    pub fn get(&self, sensor_id: &str) -> Option<&SensorReading> {
        self.readings.get(sensor_id)
    }

    /// センサーの最新正規化値を取得。
    #[must_use]
    pub fn get_normalized(&self, sensor_id: &str) -> Option<f64> {
        self.readings.get(sensor_id).map(SensorReading::normalized)
    }

    /// センサーの履歴を取得。
    #[must_use]
    pub fn history(&self, sensor_id: &str) -> Option<&[SensorReading]> {
        self.history.get(sensor_id).map(Vec::as_slice)
    }

    /// 登録済みセンサー数。
    #[must_use]
    pub fn count(&self) -> usize {
        self.readings.len()
    }

    /// 指定タイプのセンサーIDリスト。
    #[must_use]
    pub fn sensors_by_type(&self, sensor_type: SensorType) -> Vec<&str> {
        self.readings
            .iter()
            .filter(|(_, r)| r.sensor_type == sensor_type)
            .map(|(id, _)| id.as_str())
            .collect()
    }

    /// 指定時刻以降に更新されたセンサーのみ。
    #[must_use]
    pub fn active_since(&self, since: f64) -> Vec<&str> {
        self.readings
            .iter()
            .filter(|(_, r)| r.timestamp >= since)
            .map(|(id, _)| id.as_str())
            .collect()
    }

    /// センサーを削除。
    pub fn remove(&mut self, sensor_id: &str) {
        self.readings.remove(sensor_id);
        self.history.remove(sensor_id);
    }

    /// 全センサーをクリア。
    pub fn clear(&mut self) {
        self.readings.clear();
        self.history.clear();
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn make_reading(
        id: &str,
        sensor_type: SensorType,
        value: f64,
        timestamp: f64,
    ) -> SensorReading {
        SensorReading {
            sensor_id: id.to_string(),
            sensor_type,
            value,
            timestamp,
            min_value: 0.0,
            max_value: 100.0,
        }
    }

    #[test]
    fn sensor_type_roundtrip() {
        for st in [
            SensorType::Pressure,
            SensorType::Temperature,
            SensorType::Acceleration,
            SensorType::Gyroscope,
            SensorType::HeartRate,
            SensorType::Proximity,
            SensorType::Custom,
        ] {
            assert_eq!(SensorType::parse(st.as_str()), st);
        }
    }

    #[test]
    fn sensor_type_unknown() {
        assert_eq!(SensorType::parse("unknown"), SensorType::Custom);
    }

    #[test]
    fn reading_normalized_mid() {
        let r = make_reading("s1", SensorType::Pressure, 50.0, 1.0);
        assert!((r.normalized() - 0.5).abs() < 1e-10);
    }

    #[test]
    fn reading_normalized_min() {
        let r = make_reading("s1", SensorType::Pressure, 0.0, 1.0);
        assert!((r.normalized() - 0.0).abs() < 1e-10);
    }

    #[test]
    fn reading_normalized_max() {
        let r = make_reading("s1", SensorType::Pressure, 100.0, 1.0);
        assert!((r.normalized() - 1.0).abs() < 1e-10);
    }

    #[test]
    fn reading_normalized_clamped() {
        let r = make_reading("s1", SensorType::Pressure, 200.0, 1.0);
        assert!((r.normalized() - 1.0).abs() < 1e-10);
    }

    #[test]
    fn reading_normalized_zero_range() {
        let r = SensorReading {
            sensor_id: "s1".into(),
            sensor_type: SensorType::Pressure,
            value: 50.0,
            timestamp: 1.0,
            min_value: 50.0,
            max_value: 50.0,
        };
        assert!((r.normalized() - 0.0).abs() < 1e-10);
    }

    #[test]
    fn registry_update_and_get() {
        let mut reg = SensorRegistry::new(10);
        let r = make_reading("pressure1", SensorType::Pressure, 42.0, 1.0);
        reg.update(r);
        assert_eq!(reg.count(), 1);
        let got = reg.get("pressure1").unwrap();
        assert!((got.value - 42.0).abs() < 1e-10);
    }

    #[test]
    fn registry_get_normalized() {
        let mut reg = SensorRegistry::new(10);
        reg.update(make_reading("s1", SensorType::Temperature, 75.0, 1.0));
        let norm = reg.get_normalized("s1").unwrap();
        assert!((norm - 0.75).abs() < 1e-10);
    }

    #[test]
    fn registry_get_nonexistent() {
        let reg = SensorRegistry::new(10);
        assert!(reg.get("nope").is_none());
        assert!(reg.get_normalized("nope").is_none());
    }

    #[test]
    fn registry_history() {
        let mut reg = SensorRegistry::new(3);
        for i in 0..5 {
            reg.update(make_reading(
                "s1",
                SensorType::Pressure,
                f64::from(i),
                f64::from(i),
            ));
        }
        let hist = reg.history("s1").unwrap();
        assert_eq!(hist.len(), 3); // capacity = 3
        assert!((hist[0].value - 2.0).abs() < 1e-10); // oldest surviving
    }

    #[test]
    fn registry_sensors_by_type() {
        let mut reg = SensorRegistry::new(10);
        reg.update(make_reading("p1", SensorType::Pressure, 10.0, 1.0));
        reg.update(make_reading("t1", SensorType::Temperature, 20.0, 1.0));
        reg.update(make_reading("p2", SensorType::Pressure, 30.0, 1.0));
        let pressure = reg.sensors_by_type(SensorType::Pressure);
        assert_eq!(pressure.len(), 2);
    }

    #[test]
    fn registry_active_since() {
        let mut reg = SensorRegistry::new(10);
        reg.update(make_reading("old", SensorType::Pressure, 10.0, 1.0));
        reg.update(make_reading("new", SensorType::Pressure, 20.0, 5.0));
        let active = reg.active_since(3.0);
        assert_eq!(active.len(), 1);
        assert_eq!(active[0], "new");
    }

    #[test]
    fn registry_remove() {
        let mut reg = SensorRegistry::new(10);
        reg.update(make_reading("s1", SensorType::Pressure, 10.0, 1.0));
        assert_eq!(reg.count(), 1);
        reg.remove("s1");
        assert_eq!(reg.count(), 0);
        assert!(reg.history("s1").is_none());
    }

    #[test]
    fn registry_clear() {
        let mut reg = SensorRegistry::new(10);
        reg.update(make_reading("s1", SensorType::Pressure, 10.0, 1.0));
        reg.update(make_reading("s2", SensorType::Temperature, 20.0, 1.0));
        reg.clear();
        assert_eq!(reg.count(), 0);
    }
}
