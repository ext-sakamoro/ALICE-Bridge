# Changelog

## [Unreleased]

### Security
- CI: 何もしない composite action `.github/actions/alice-stubs` (sibling の path 依存が 0 なので echo するだけ) と、それを呼ぶ 6 個の step を削除した
- tokio を 1.53.3 に上げる (RUSTSEC-2026-0386、`AsyncFd` の unsound、1.53.3 で修正) Cargo.lock の変更は tokio の版と checksum だけ
- rustls を 0.23.46、rustls-webpki を 0.103.16 に上げる (RUSTSEC-2026-0346 / 0347 / 0348 / 0349、RUSTSEC-2026-0382) Cargo.lock の変更は 2 package の版と checksum だけ `deny.toml` から rustls 系の ignore 5 件 (RUSTSEC-2025-0134 / RUSTSEC-2026-0049 / 0098 / 0099 / 0104) を外す

### Changed
- Licence: `AGPL-3.0-or-later` → `LicenseRef-Proprietary`
- CI: `ci.yml` と `security-audit.yml` を `ci/**` の branch でも走らせ (main に入れる前に同じ検査を通す)、`ci.yml` は手動実行もでき、main 以外の古い run は打ち切る

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
