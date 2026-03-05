//! クローズドループ制御 — フィードバック→振幅調整、PID制御ループ
//!
//! センサーフィードバックに基づいてアクチュエーター出力を自動調整する。

/// PID コントローラー設定。
#[derive(Debug, Clone, Copy)]
pub struct PidConfig {
    /// 比例ゲイン。
    pub kp: f64,
    /// 積分ゲイン。
    pub ki: f64,
    /// 微分ゲイン。
    pub kd: f64,
    /// 出力下限。
    pub output_min: f64,
    /// 出力上限。
    pub output_max: f64,
    /// 積分巻き上げ防止リミット。
    pub integral_limit: f64,
}

impl Default for PidConfig {
    fn default() -> Self {
        Self {
            kp: 1.0,
            ki: 0.1,
            kd: 0.05,
            output_min: 0.0,
            output_max: 1.0,
            integral_limit: 10.0,
        }
    }
}

/// PID コントローラー。
#[derive(Debug, Clone)]
pub struct PidController {
    config: PidConfig,
    /// 目標値 (setpoint)。
    setpoint: f64,
    /// 積分項の累積。
    integral: f64,
    /// 前回の誤差。
    prev_error: f64,
    /// 前回の出力。
    output: f64,
    /// 更新回数。
    update_count: u64,
}

impl PidController {
    /// 新しいPIDコントローラーを作成。
    #[must_use]
    pub const fn new(setpoint: f64, config: PidConfig) -> Self {
        Self {
            config,
            setpoint,
            integral: 0.0,
            prev_error: 0.0,
            output: 0.0,
            update_count: 0,
        }
    }

    /// 目標値を設定。
    pub const fn set_setpoint(&mut self, setpoint: f64) {
        self.setpoint = setpoint;
    }

    /// 目標値を取得。
    #[must_use]
    pub const fn setpoint(&self) -> f64 {
        self.setpoint
    }

    /// PID更新: フィードバック値を受け取り、制御出力を返す。
    ///
    /// `measured` はセンサーからの測定値、`dt` は前回更新からの経過時間(秒)。
    pub fn update(&mut self, measured: f64, dt: f64) -> f64 {
        let error = self.setpoint - measured;

        // 積分項 (anti-windup)
        self.integral += error * dt;
        self.integral = self
            .integral
            .clamp(-self.config.integral_limit, self.config.integral_limit);

        // 微分項
        let derivative = if dt > 1e-15 {
            (error - self.prev_error) / dt
        } else {
            0.0
        };

        // PID出力
        let raw_output = self.config.kd.mul_add(
            derivative,
            self.config
                .kp
                .mul_add(error, self.config.ki * self.integral),
        );

        self.output = raw_output.clamp(self.config.output_min, self.config.output_max);
        self.prev_error = error;
        self.update_count += 1;

        self.output
    }

    /// 現在の出力値。
    #[must_use]
    pub const fn output(&self) -> f64 {
        self.output
    }

    /// コントローラーをリセット。
    pub const fn reset(&mut self) {
        self.integral = 0.0;
        self.prev_error = 0.0;
        self.output = 0.0;
        self.update_count = 0;
    }

    /// 更新回数。
    #[must_use]
    pub const fn update_count(&self) -> u64 {
        self.update_count
    }
}

/// フィードバックコントローラー — センサー→PID→アクチュエーター。
#[derive(Debug)]
pub struct FeedbackController {
    /// チャンネル名。
    name: String,
    /// PIDコントローラー。
    pid: PidController,
    /// フィードバック有効/無効。
    enabled: bool,
    /// デッドバンド (この範囲内は調整しない)。
    deadband: f64,
}

impl FeedbackController {
    /// 新しいフィードバックコントローラーを作成。
    #[must_use]
    pub fn new(name: &str, setpoint: f64, config: PidConfig) -> Self {
        Self {
            name: name.to_string(),
            pid: PidController::new(setpoint, config),
            enabled: true,
            deadband: 0.01,
        }
    }

    /// デッドバンドを設定。
    pub const fn set_deadband(&mut self, deadband: f64) {
        self.deadband = deadband.abs();
    }

    /// 有効/無効を切り替え。
    pub const fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// フィードバック値を処理し、制御出力を返す。
    ///
    /// 無効時は `None` を返す。デッドバンド内は前回値を維持。
    pub fn process(&mut self, measured: f64, dt: f64) -> Option<f64> {
        if !self.enabled {
            return None;
        }

        let error = (self.pid.setpoint() - measured).abs();
        if error < self.deadband {
            return Some(self.pid.output());
        }

        Some(self.pid.update(measured, dt))
    }

    /// チャンネル名。
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// 目標値を設定。
    pub const fn set_setpoint(&mut self, setpoint: f64) {
        self.pid.set_setpoint(setpoint);
    }

    /// リセット。
    pub const fn reset(&mut self) {
        self.pid.reset();
    }

    /// 有効かどうか。
    #[must_use]
    pub const fn is_enabled(&self) -> bool {
        self.enabled
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pid_converges_to_setpoint() {
        let config = PidConfig {
            kp: 2.0,
            ki: 0.5,
            kd: 0.1,
            ..Default::default()
        };
        let mut pid = PidController::new(0.5, config);
        let mut measured = 0.0;
        let dt = 0.1;
        for _ in 0..100 {
            let output = pid.update(measured, dt);
            // 簡易シミュレーション: 出力を測定値に反映
            measured += (output - measured) * 0.3;
        }
        assert!(
            (measured - 0.5).abs() < 0.05,
            "Should converge to setpoint, got {measured}"
        );
    }

    #[test]
    fn pid_output_clamped() {
        let config = PidConfig {
            kp: 100.0,
            output_min: 0.0,
            output_max: 1.0,
            ..Default::default()
        };
        let mut pid = PidController::new(1.0, config);
        let output = pid.update(0.0, 0.1);
        assert!(output <= 1.0);
        assert!(output >= 0.0);
    }

    #[test]
    fn pid_reset() {
        let mut pid = PidController::new(0.5, PidConfig::default());
        pid.update(0.0, 0.1);
        pid.update(0.1, 0.1);
        assert!(pid.update_count() == 2);
        pid.reset();
        assert!(pid.update_count() == 0);
        assert!((pid.output() - 0.0).abs() < 1e-15);
    }

    #[test]
    fn pid_integral_anti_windup() {
        let config = PidConfig {
            kp: 0.0,
            ki: 100.0,
            kd: 0.0,
            integral_limit: 1.0,
            ..Default::default()
        };
        let mut pid = PidController::new(1.0, config);
        // 大きな誤差を長時間
        for _ in 0..1000 {
            pid.update(0.0, 0.1);
        }
        // 積分項は integral_limit でクランプされる
        let output = pid.output();
        assert!(
            output <= 1.0,
            "Anti-windup should limit output, got {output}"
        );
    }

    #[test]
    fn pid_zero_dt() {
        let mut pid = PidController::new(0.5, PidConfig::default());
        let output = pid.update(0.0, 0.0);
        assert!(output.is_finite());
    }

    #[test]
    fn pid_setpoint_change() {
        let mut pid = PidController::new(0.5, PidConfig::default());
        pid.update(0.5, 0.1); // error = 0
        pid.set_setpoint(1.0);
        let output = pid.update(0.5, 0.1); // error = 0.5
        assert!(output > 0.0);
    }

    #[test]
    fn feedback_controller_enabled() {
        let mut fc = FeedbackController::new("test", 0.5, PidConfig::default());
        assert!(fc.is_enabled());
        let result = fc.process(0.0, 0.1);
        assert!(result.is_some());
    }

    #[test]
    fn feedback_controller_disabled() {
        let mut fc = FeedbackController::new("test", 0.5, PidConfig::default());
        fc.set_enabled(false);
        assert!(!fc.is_enabled());
        let result = fc.process(0.0, 0.1);
        assert!(result.is_none());
    }

    #[test]
    fn feedback_controller_deadband() {
        let mut fc = FeedbackController::new("test", 0.5, PidConfig::default());
        fc.set_deadband(0.1);
        // error = 0.05 < deadband → 前回値維持
        let result = fc.process(0.45, 0.1);
        assert!(result.is_some());
    }

    #[test]
    fn feedback_controller_name() {
        let fc = FeedbackController::new("pressure_loop", 0.5, PidConfig::default());
        assert_eq!(fc.name(), "pressure_loop");
    }

    #[test]
    fn feedback_controller_reset() {
        let mut fc = FeedbackController::new("test", 0.5, PidConfig::default());
        fc.process(0.0, 0.1);
        fc.reset();
        assert!(fc.pid.update_count() == 0);
    }

    #[test]
    fn pid_config_default() {
        let cfg = PidConfig::default();
        assert!((cfg.kp - 1.0).abs() < 1e-10);
        assert!((cfg.ki - 0.1).abs() < 1e-10);
        assert!((cfg.kd - 0.05).abs() < 1e-10);
    }
}
