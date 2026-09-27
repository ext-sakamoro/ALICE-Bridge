# Changelog

## [Unreleased]

### Changed
- **License: `AGPL-3.0-or-later` → `AGPL-3.0-or-later OR LicenseRef-Commercial` (dual-licensed、2026-09-27)** AGPL 側の条件は変更なし (既存 AGPL 利用者への影響ゼロ)、商用という選択肢が追加されただけ SPDX が AGPL 単独だと cargo-deny / FOSSA / SBOM に「商用オプションなし」と見えるため宣言を dual に 変更点: SPDX / `LICENSE` → `LICENSE-AGPL` / `LICENSE-COMMERCIAL.md` (商用トリガー 6 条件 = クローズド製品・商用 SaaS・エッジ / ファームウェア配布・plugin 再配布・プラットフォーム NDA・保証、社内利用は AGPL 側で無償と明記) / README の選択肢表 商用窓口は法人 `contact@extoria.co.jp`

### Added
- `rust-version = "1.88"` (reqwest → icu 2.3 の要求、`cargo +1.88 check` green / 1.87 は deps で拒否)
- `ci.yml`: fmt + actionlint のみ → test (default + no-default) / clippy `--all-targets` pedantic `-D warnings` 2 variant / `msrv` 1.88 / `feature-powerset` (depth 2) / doc `-D warnings`、rust-cache

### Fixed
- clippy pedantic 3 件 (test の `assert!(a == b)` → `assert_eq!`)

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
