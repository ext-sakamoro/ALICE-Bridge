//! 録画/再生スクリプティング — コマンド録画、タイムスタンプ付き再生
//!
//! デバイスコマンドのシーケンスを記録・再生する。

use serde::{Deserialize, Serialize};

/// スクリプトコマンド。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScriptCommand {
    /// コマンド発行からの相対時刻 (秒)。
    pub time: f64,
    /// 対象デバイスID。
    pub device_id: String,
    /// コマンドタイプ ("scalar", "linear", "rotate", "stop")。
    pub command_type: String,
    /// 強度/位置 (0.0–1.0)。
    pub value: f64,
    /// コマンド固有パラメータ (`duration_ms等`)。
    pub params: String,
}

/// 記録済みスクリプト。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Script {
    /// スクリプト名。
    pub name: String,
    /// コマンドリスト (時刻順)。
    pub commands: Vec<ScriptCommand>,
    /// 合計時間 (秒)。
    pub duration: f64,
}

impl Script {
    /// 新しい空スクリプトを作成。
    #[must_use]
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            commands: Vec::new(),
            duration: 0.0,
        }
    }

    /// コマンド数。
    #[must_use]
    pub const fn len(&self) -> usize {
        self.commands.len()
    }

    /// 空かどうか。
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.commands.is_empty()
    }

    /// コマンドを時刻順に挿入。
    pub fn add_command(&mut self, cmd: ScriptCommand) {
        if cmd.time > self.duration {
            self.duration = cmd.time;
        }
        // 挿入位置をバイナリサーチ
        let pos = self.commands.partition_point(|c| c.time <= cmd.time);
        self.commands.insert(pos, cmd);
    }
}

/// スクリプトレコーダー。
#[derive(Debug)]
pub struct ScriptRecorder {
    script: Script,
    recording: bool,
    start_time: f64,
}

impl ScriptRecorder {
    /// 新しいレコーダーを作成。
    #[must_use]
    pub fn new(name: &str) -> Self {
        Self {
            script: Script::new(name),
            recording: false,
            start_time: 0.0,
        }
    }

    /// 録画開始。
    pub fn start(&mut self, now: f64) {
        self.recording = true;
        self.start_time = now;
        self.script.commands.clear();
        self.script.duration = 0.0;
    }

    /// 録画停止、完成したスクリプトを返す。
    #[must_use]
    pub fn stop(&mut self) -> Script {
        self.recording = false;
        self.script.clone()
    }

    /// 録画中かどうか。
    #[must_use]
    pub const fn is_recording(&self) -> bool {
        self.recording
    }

    /// コマンドを記録。
    pub fn record(
        &mut self,
        device_id: &str,
        command_type: &str,
        value: f64,
        params: &str,
        now: f64,
    ) {
        if !self.recording {
            return;
        }
        let time = now - self.start_time;
        self.script.add_command(ScriptCommand {
            time,
            device_id: device_id.to_string(),
            command_type: command_type.to_string(),
            value,
            params: params.to_string(),
        });
    }

    /// 現在の録画時間 (秒)。
    #[must_use]
    pub fn elapsed(&self, now: f64) -> f64 {
        if self.recording {
            now - self.start_time
        } else {
            self.script.duration
        }
    }
}

/// スクリプトプレイヤー状態。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayState {
    Stopped,
    Playing,
    Paused,
}

/// スクリプトプレイヤー。
#[derive(Debug)]
pub struct ScriptPlayer {
    script: Script,
    state: PlayState,
    /// 再生開始時刻。
    start_time: f64,
    /// 一時停止時の経過時刻。
    pause_elapsed: f64,
    /// 次に実行するコマンドインデックス。
    cursor: usize,
    /// ループ再生。
    looping: bool,
    /// 再生速度倍率。
    speed: f64,
}

impl ScriptPlayer {
    /// 新しいプレイヤーを作成。
    #[must_use]
    pub const fn new(script: Script) -> Self {
        Self {
            script,
            state: PlayState::Stopped,
            start_time: 0.0,
            pause_elapsed: 0.0,
            cursor: 0,
            looping: false,
            speed: 1.0,
        }
    }

    /// ループ設定。
    pub const fn set_looping(&mut self, looping: bool) {
        self.looping = looping;
    }

    /// 再生速度設定。
    pub const fn set_speed(&mut self, speed: f64) {
        self.speed = speed.max(0.01);
    }

    /// 再生開始。
    pub fn play(&mut self, now: f64) {
        match self.state {
            PlayState::Stopped => {
                self.start_time = now;
                self.cursor = 0;
                self.pause_elapsed = 0.0;
            }
            PlayState::Paused => {
                // 一時停止からの再開
                self.start_time = now - self.pause_elapsed;
            }
            PlayState::Playing => return,
        }
        self.state = PlayState::Playing;
    }

    /// 一時停止。
    pub fn pause(&mut self, now: f64) {
        if self.state == PlayState::Playing {
            self.pause_elapsed = (now - self.start_time) * self.speed;
            self.state = PlayState::Paused;
        }
    }

    /// 停止。
    pub const fn stop(&mut self) {
        self.state = PlayState::Stopped;
        self.cursor = 0;
        self.pause_elapsed = 0.0;
    }

    /// 現在の状態。
    #[must_use]
    pub const fn state(&self) -> PlayState {
        self.state
    }

    /// ティック: 現時刻に基づいて実行すべきコマンドを返す。
    pub fn tick(&mut self, now: f64) -> Vec<ScriptCommand> {
        if self.state != PlayState::Playing {
            return Vec::new();
        }

        let elapsed = (now - self.start_time) * self.speed;
        let mut commands = Vec::new();

        while self.cursor < self.script.commands.len() {
            let cmd = &self.script.commands[self.cursor];
            if cmd.time <= elapsed {
                commands.push(cmd.clone());
                self.cursor += 1;
            } else {
                break;
            }
        }

        // ループ処理
        if self.cursor >= self.script.commands.len() && self.looping && !self.script.is_empty() {
            self.cursor = 0;
            self.start_time = now;
        } else if self.cursor >= self.script.commands.len() {
            self.state = PlayState::Stopped;
        }

        commands
    }

    /// スクリプトの合計時間。
    #[must_use]
    pub const fn duration(&self) -> f64 {
        self.script.duration
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_command(time: f64, value: f64) -> ScriptCommand {
        ScriptCommand {
            time,
            device_id: "dev1".to_string(),
            command_type: "scalar".to_string(),
            value,
            params: String::new(),
        }
    }

    #[test]
    fn script_new_empty() {
        let script = Script::new("test");
        assert_eq!(script.name, "test");
        assert!(script.is_empty());
        assert_eq!(script.len(), 0);
    }

    #[test]
    fn script_add_command_sorted() {
        let mut script = Script::new("test");
        script.add_command(sample_command(2.0, 0.5));
        script.add_command(sample_command(1.0, 0.3));
        script.add_command(sample_command(3.0, 0.7));
        assert_eq!(script.len(), 3);
        assert!((script.commands[0].time - 1.0).abs() < 1e-10);
        assert!((script.commands[1].time - 2.0).abs() < 1e-10);
        assert!((script.commands[2].time - 3.0).abs() < 1e-10);
    }

    #[test]
    fn script_duration() {
        let mut script = Script::new("test");
        script.add_command(sample_command(5.0, 0.5));
        assert!((script.duration - 5.0).abs() < 1e-10);
    }

    #[test]
    fn recorder_basic_flow() {
        let mut rec = ScriptRecorder::new("rec1");
        assert!(!rec.is_recording());

        rec.start(1.0);
        assert!(rec.is_recording());

        rec.record("dev1", "scalar", 0.5, "", 1.5);
        rec.record("dev1", "scalar", 0.8, "", 2.0);

        let script = rec.stop();
        assert!(!rec.is_recording());
        assert_eq!(script.len(), 2);
        assert!((script.commands[0].time - 0.5).abs() < 1e-10);
        assert!((script.commands[1].time - 1.0).abs() < 1e-10);
    }

    #[test]
    fn recorder_ignores_when_not_recording() {
        let mut rec = ScriptRecorder::new("rec1");
        rec.record("dev1", "scalar", 0.5, "", 1.0);
        let script = rec.stop();
        assert!(script.is_empty());
    }

    #[test]
    fn recorder_elapsed() {
        let mut rec = ScriptRecorder::new("rec1");
        rec.start(1.0);
        assert!((rec.elapsed(3.0) - 2.0).abs() < 1e-10);
    }

    #[test]
    fn player_basic_playback() {
        let mut script = Script::new("test");
        script.add_command(sample_command(0.0, 0.3));
        script.add_command(sample_command(0.5, 0.5));
        script.add_command(sample_command(1.0, 0.8));

        let mut player = ScriptPlayer::new(script);
        player.play(0.0);
        assert_eq!(player.state(), PlayState::Playing);

        // At t=0.0 → first command
        let cmds = player.tick(0.0);
        assert_eq!(cmds.len(), 1);
        assert!((cmds[0].value - 0.3).abs() < 1e-10);

        // At t=0.6 → second command
        let cmds = player.tick(0.6);
        assert_eq!(cmds.len(), 1);
        assert!((cmds[0].value - 0.5).abs() < 1e-10);

        // At t=1.5 → third command + auto-stop
        let cmds = player.tick(1.5);
        assert_eq!(cmds.len(), 1);
        assert_eq!(player.state(), PlayState::Stopped);
    }

    #[test]
    fn player_loop() {
        let mut script = Script::new("test");
        script.add_command(sample_command(0.0, 0.5));
        script.add_command(sample_command(0.1, 0.8));

        let mut player = ScriptPlayer::new(script);
        player.set_looping(true);
        player.play(0.0);

        let cmds = player.tick(0.2);
        assert_eq!(cmds.len(), 2);
        // Should loop, not stop
        assert_eq!(player.state(), PlayState::Playing);
    }

    #[test]
    fn player_pause_resume() {
        let mut script = Script::new("test");
        script.add_command(sample_command(0.0, 0.3));
        script.add_command(sample_command(1.0, 0.8));

        let mut player = ScriptPlayer::new(script);
        player.play(0.0);
        player.tick(0.0); // consume first command

        player.pause(0.5);
        assert_eq!(player.state(), PlayState::Paused);

        let cmds = player.tick(1.0);
        assert!(cmds.is_empty()); // paused

        player.play(1.0); // resume
        assert_eq!(player.state(), PlayState::Playing);
    }

    #[test]
    fn player_stop() {
        let mut script = Script::new("test");
        script.add_command(sample_command(0.0, 0.5));

        let mut player = ScriptPlayer::new(script);
        player.play(0.0);
        player.stop();
        assert_eq!(player.state(), PlayState::Stopped);
    }

    #[test]
    fn player_speed() {
        let mut script = Script::new("test");
        script.add_command(sample_command(1.0, 0.5));

        let mut player = ScriptPlayer::new(script);
        player.set_speed(2.0); // 2x speed
        player.play(0.0);

        // At t=0.5 real time → 1.0 script time
        let cmds = player.tick(0.5);
        assert_eq!(cmds.len(), 1);
    }

    #[test]
    fn player_tick_when_stopped() {
        let script = Script::new("test");
        let mut player = ScriptPlayer::new(script);
        let cmds = player.tick(1.0);
        assert!(cmds.is_empty());
    }

    #[test]
    fn player_duration() {
        let mut script = Script::new("test");
        script.add_command(sample_command(5.0, 0.5));
        let player = ScriptPlayer::new(script);
        assert!((player.duration() - 5.0).abs() < 1e-10);
    }

    #[test]
    fn script_serde_roundtrip() {
        let mut script = Script::new("test");
        script.add_command(sample_command(1.0, 0.5));
        let json = serde_json::to_string(&script).unwrap();
        let parsed: Script = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.name, "test");
        assert_eq!(parsed.len(), 1);
    }
}
