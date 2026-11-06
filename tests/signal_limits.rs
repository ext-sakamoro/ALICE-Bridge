//! `SignalBridge` always outputs a position in its range, and refuses what would break that.
//!
//! oracle: the range is `min_position..=max_position`, so `min <= max` and both finite, with a width
//! `f64` can hold, or there is no range; `f64::clamp` panics on `min > max` or a `NaN` bound, which is
//! the fuzz crash. A value that is not a number has no place in a weighted average, and one `NaN` kept
//! there would be the output from then on. So the door refuses these, and for every sequence of
//! accepted calls the position is finite and in the range.

use alice_bridge::{SignalBridge, SignalError};
use SignalError::{InvalidRange, InvalidSpeedLimit, InvalidWeight, NotFinite};

const NAN: f64 = f64::NAN;
const INF: f64 = f64::INFINITY;

fn bridge() -> SignalBridge {
    let mut b = SignalBridge::new(4, 1.0, 0.0, 10.0, 20).unwrap();
    b.add_source("s", 1.0).unwrap();
    b
}

#[test]
fn a_range_that_is_no_range_is_refused() {
    for (min, max) in [
        (5.0, 1.0),
        // the fuzz crash, as logged: min 2.41e64, max 1.64e-312
        (2.410_825_498_970_963_4e64, 1.640_344_354_175e-312),
        (NAN, 1.0),
        (0.0, NAN),
        (NAN, NAN),
        (-INF, 1.0),
        (0.0, INF),
        (INF, INF),
        // finite, but the width is not
        (-f64::MAX, f64::MAX),
    ] {
        assert_eq!(
            SignalBridge::new(4, 1.0, min, max, 20).err(),
            Some(InvalidRange),
            "{min} {max}"
        );
    }
    for (min, max) in [
        (0.0, 1.0),
        (1.0, 1.0),
        (-5.0, 5.0),
        (-0.0, 0.0),
        (0.0, f64::MAX),
    ] {
        assert!(
            SignalBridge::new(4, 1.0, min, max, 20).is_ok(),
            "{min} {max}"
        );
    }
}

#[test]
fn a_speed_limit_is_a_number_at_least_zero() {
    for v in [NAN, -1.0, -INF] {
        assert_eq!(
            SignalBridge::new(4, v, 0.0, 1.0, 20).err(),
            Some(InvalidSpeedLimit),
            "{v}"
        );
    }
    // no limit, and a position that holds
    for v in [0.0, 1.0, INF] {
        assert!(SignalBridge::new(4, v, 0.0, 1.0, 20).is_ok(), "{v}");
    }
}

#[test]
fn a_weight_is_finite_and_at_least_zero() {
    let mut b = bridge();
    for w in [NAN, INF, -INF, -1.0] {
        assert_eq!(b.add_source("t", w), Err(InvalidWeight), "{w}");
    }
    assert_eq!(b.add_source("t", 0.0), Ok(()));
}

#[test]
fn values_and_times_that_are_not_numbers_are_refused_and_change_nothing() {
    let mut b = bridge();
    b.update("s", 0.5, 1.0).unwrap();
    let before = b.tick(1.0).unwrap().position;
    for (v, t) in [(NAN, 1.0), (INF, 1.0), (-INF, 1.0), (0.5, NAN), (0.5, INF)] {
        assert_eq!(b.update("s", v, t), Err(NotFinite), "{v} {t}");
    }
    for t in [NAN, INF, -INF] {
        assert_eq!(b.tick(t).err(), Some(NotFinite), "{t}");
    }
    assert_eq!(b.tick_count(), 1);
    assert_eq!(b.tick(1.0).unwrap().position.to_bits(), before.to_bits());
}

#[test]
fn weights_that_overflow_their_sum_still_give_the_weighted_mean() {
    let mut b = SignalBridge::new(1, INF, 0.0, 8.0, 20).unwrap();
    b.add_source("a", f64::MAX).unwrap();
    b.add_source("b", f64::MAX).unwrap();
    b.add_source("c", f64::MAX / 3.0 * 2.0).unwrap();
    b.update("a", 1.0, 1.0).unwrap();
    b.update("b", 0.0, 1.0).unwrap();
    b.update("c", 1.0, 1.0).unwrap();
    // (1 + 0 + 2/3) / (1 + 1 + 2/3) = 0.625
    let got = b.tick(1.0).unwrap().position;
    assert!((got - 5.0).abs() < 1e-12, "got {got}");
}

#[test]
fn weights_of_far_apart_sizes_with_an_overflowing_sum_still_give_the_weighted_mean() {
    // oracle: 1.0 next to MAX adds nothing a double can hold: (1 + 1 + 0) / (1 + 1 + 0) = 1
    let mut b = SignalBridge::new(1, INF, 0.0, 8.0, 20).unwrap();
    b.add_source("a", f64::MAX).unwrap();
    b.add_source("b", f64::MAX).unwrap();
    b.add_source("c", 1.0).unwrap();
    b.update("a", 1.0, 1.0).unwrap();
    b.update("b", 1.0, 1.0).unwrap();
    b.update("c", 0.0, 1.0).unwrap();
    assert_eq!(b.tick(1.0).unwrap().position, 8.0);
}

/// A fixed sequence (splitmix64), so every run compares the same inputs.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    /// Any `f64`, half of them from the bit patterns that break things.
    fn any(&mut self) -> f64 {
        const EDGES: [f64; 10] = [
            f64::NAN,
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::MAX,
            f64::MIN,
            f64::MIN_POSITIVE,
            0.0,
            -0.0,
            1.0,
            0.5,
        ];
        if self.next().is_multiple_of(2) {
            EDGES[usize::try_from(self.next() % 10).unwrap()]
        } else {
            f64::from_bits(self.next())
        }
    }
}

#[test]
fn every_accepted_sequence_stays_in_the_range() {
    let mut rng = Rng(0x5349_4e47);
    let (mut bridges, mut ticks, mut sources) = (0usize, 0usize, 0usize);
    for _ in 0..200_000 {
        let (a, b, v) = (rng.any(), rng.any(), rng.any());
        let Ok(mut bridge) = SignalBridge::new(3, v, a, b, 20) else {
            continue;
        };
        bridges += 1;
        for i in 0..24 {
            let name = if rng.next().is_multiple_of(2) {
                "x"
            } else {
                "y"
            };
            match rng.next() % 3 {
                0 => {
                    if bridge.add_source(name, rng.any()).is_ok() {
                        sources += 1;
                    }
                }
                1 => {
                    let _ = bridge.update(name, rng.any(), rng.any());
                }
                _ => {
                    if let Ok(action) = bridge.tick(rng.any()) {
                        ticks += 1;
                        let p = action.position;
                        assert!(p.is_finite() && a <= p && p <= b, "{p} in {a}..={b} at {i}");
                    }
                }
            }
        }
    }
    assert!(bridges > 10_000, "bridges {bridges}");
    assert!(ticks > 10_000, "ticks {ticks}");
    assert!(sources > 10_000, "sources {sources}");
}

#[test]
fn ordinary_use_is_as_it_was() {
    let mut b = SignalBridge::new(1, 1000.0, 0.0, 100.0, 20).unwrap();
    b.add_source("s", 1.0).unwrap();
    b.update("s", 0.25, 1.0).unwrap();
    let action = b.tick(1.0).unwrap();
    assert_eq!(action.position, 25.0);
    assert_eq!(action.duration_ms, 20);
    assert_eq!(b.active_sources(), vec!["s"]);
}
