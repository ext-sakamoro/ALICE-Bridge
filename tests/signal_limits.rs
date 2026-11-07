//! `SignalBridge` always outputs a position in its range, and refuses what would break that.
//!
//! oracle: the range is `min_position..=max_position`, so `min <= max` and both finite, with a width
//! `f64` can hold, or there is no range; `f64::clamp` panics on `min > max` or a `NaN` bound, which is
//! the fuzz crash. A value that is not a number has no place in a weighted average, and one `NaN` kept
//! there would be the output from then on. So the door refuses these, and for every sequence of
//! accepted calls the position is finite and in the range.

#![allow(
    clippy::float_cmp,
    reason = "the positions are exact: the limit times whole seconds, and powers of two"
)]

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
    assert_eq!(b.tick(1.0).unwrap().position.to_bits(), 8.0_f64.to_bits());
}

#[test]
#[cfg(target_pointer_width = "64")]
fn a_smoothing_window_of_any_size_costs_nothing_until_it_fills() {
    // oracle: a window larger than the values given averages the values given
    for window in [1usize << 44, 1usize << 61, usize::MAX] {
        let mut b = SignalBridge::new(window, INF, 0.0, 8.0, 20).unwrap();
        b.add_source("s", 1.0).unwrap();
        b.update("s", 0.5, 1.0).unwrap();
        assert_eq!(
            b.tick(1.0).unwrap().position.to_bits(),
            4.0_f64.to_bits(),
            "{window}"
        );
    }
}

#[test]
fn the_speed_limit_holds_every_tick_in_both_directions() {
    // oracle: at 2 per second, a quarter of a second moves 0.5 whichever way the target is
    let mut b = SignalBridge::new(1, 2.0, 0.0, 10.0, 20).unwrap();
    b.add_source("s", 1.0).unwrap();
    let mut step = |value: f64, t: f64| {
        b.update("s", value, t).unwrap();
        b.tick(t).unwrap().position
    };
    assert_eq!(step(1.0, 1.0).to_bits(), 10.0_f64.to_bits());
    assert_eq!(step(0.0, 1.25).to_bits(), 9.5_f64.to_bits());
    assert_eq!(step(0.0, 1.5).to_bits(), 9.0_f64.to_bits());
    assert_eq!(step(1.0, 1.75).to_bits(), 9.5_f64.to_bits());
}

#[test]
fn a_refused_value_does_not_make_a_source_active() {
    let mut b = SignalBridge::new(1, INF, 0.0, 10.0, 20).unwrap();
    b.add_source("s", 1.0).unwrap();
    assert_eq!(b.update("s", NAN, 1.0), Err(NotFinite));
    assert!(b.active_sources().is_empty());
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
    assert_eq!(action.position.to_bits(), 25.0_f64.to_bits());
    assert_eq!(action.duration_ms, 20);
    assert_eq!(b.active_sources(), vec!["s"]);
}

/// A bridge with one source and no smoothing, ticked once at `start` with the source at 0, so it
/// stands at `min`.
fn standing_at_zero(speed_limit: f64, start: f64) -> SignalBridge {
    let mut b = SignalBridge::new(1, speed_limit, 0.0, 100.0, 1).unwrap();
    b.add_source("s", 1.0).unwrap();
    b.update("s", 0.0, start).unwrap();
    b.tick(start).unwrap();
    assert_eq!(b.last_position(), 0.0);
    b
}

/// The position after the source says `value` at `now`, so it is not stale at that tick
fn tick_towards(b: &mut SignalBridge, value: f64, now: f64) -> f64 {
    b.update("s", value, now).unwrap();
    b.tick(now).unwrap().position
}

// oracle for the three below: a limit of L per second moves at most L * (time that has passed), and
// time that has not passed moves nothing
#[test]
fn time_that_goes_back_or_stands_still_moves_nothing() {
    for start in [10.0, 0.0, -5.0] {
        let mut b = standing_at_zero(1.0, start);
        // the same time, and earlier times, whether or not the clock reads above zero
        for now in [start, start - 1.0, start - 3.0] {
            assert_eq!(
                tick_towards(&mut b, 1.0, now),
                0.0,
                "start {start} now {now}"
            );
        }
        // from a position inside the range too, where a step back would show
        let mut mid = standing_at_zero(1.0, start);
        assert_eq!(
            tick_towards(&mut mid, 1.0, start + 50.0),
            50.0,
            "start {start}"
        );
        assert_eq!(
            tick_towards(&mut mid, 1.0, start + 47.0),
            50.0,
            "start {start}"
        );
        // the tick after a step back counts from the time stepped back to
        assert_eq!(
            tick_towards(&mut mid, 1.0, start + 48.0),
            51.0,
            "start {start}"
        );
        assert_eq!(
            tick_towards(&mut mid, 1.0, start + 50.0),
            53.0,
            "start {start}"
        );
    }
}

// A clock that goes back does not hold the output until it catches up: limit 10 per second at
// position 100, the clock falls from 3600 to 5, and the output comes down at 10 per second from
// there, whether the source sends 0 or stops and goes stale.
#[test]
fn a_clock_stepped_back_does_not_freeze_the_output() {
    let at_the_top = |top: f64| {
        let mut b = standing_at_zero(10.0, 0.0);
        assert_eq!(tick_towards(&mut b, 1.0, top), 100.0);
        b
    };
    let mut b = at_the_top(3600.0);
    assert_eq!(tick_towards(&mut b, 0.0, 5.0), 100.0);
    assert_eq!(tick_towards(&mut b, 0.0, 6.0), 90.0);
    assert_eq!(tick_towards(&mut b, 0.0, 8.0), 70.0);

    // the source stops: it goes stale after half a second, and the fused value reads 0
    let mut b = at_the_top(3600.0);
    b.update("s", 1.0, 5.0).unwrap();
    assert_eq!(b.tick(5.0).unwrap().position, 100.0);
    assert_eq!(b.tick(6.0).unwrap().position, 90.0);

    // a source still fed on the stepped-back clock stays active: it holds the output where it is
    let mut b = at_the_top(3600.0);
    for now in [5.0, 6.0, 7.0] {
        assert_eq!(tick_towards(&mut b, 1.0, now), 100.0, "{now}");
    }
    assert_eq!(b.active_sources(), ["s"]);

    // one tick far ahead by mistake freezes nothing either
    let mut b = at_the_top(100.0);
    assert_eq!(tick_towards(&mut b, 1.0, 12_000.0), 100.0);
    assert_eq!(tick_towards(&mut b, 0.0, 101.0), 100.0);
    assert_eq!(tick_towards(&mut b, 0.0, 102.0), 90.0);
}

#[test]
fn the_limit_holds_in_both_directions() {
    let mut b = standing_at_zero(1.0, 1.0);
    assert_eq!(tick_towards(&mut b, 1.0, 51.0), 50.0);
    assert_eq!(tick_towards(&mut b, 0.0, 53.0), 48.0);
}

#[test]
fn a_limit_of_zero_never_moves_and_an_infinite_one_always_may() {
    let mut b = standing_at_zero(0.0, 1.0);
    // the difference of these times is infinite, and 0 * inf must not read as no limit
    for now in [2.0, f64::MAX, -f64::MAX, f64::MAX] {
        assert_eq!(tick_towards(&mut b, 1.0, now), 0.0, "{now}");
    }
    // the first time is far enough back that the difference is infinite
    let mut b = standing_at_zero(0.0, -f64::MAX);
    assert_eq!(tick_towards(&mut b, 1.0, f64::MAX), 0.0);
    let mut b = standing_at_zero(INF, 1.0);
    // no time has passed, and inf * 0 must not read as a limit
    assert_eq!(tick_towards(&mut b, 1.0, 1.0), 100.0);
    assert_eq!(tick_towards(&mut b, 0.0, 0.5), 0.0);
}

#[test]
fn the_first_tick_has_no_time_to_measure_and_goes_to_the_target() {
    // no earlier tick means no known speed, whatever the clock reads
    for start in [1.0, 0.0, -1.0] {
        let mut b = SignalBridge::new(1, 0.0, 0.0, 100.0, 1).unwrap();
        b.add_source("s", 1.0).unwrap();
        assert_eq!(tick_towards(&mut b, 1.0, start), 100.0, "{start}");
    }
}

#[test]
fn the_scale_of_the_weights_does_not_change_the_mean() {
    // oracle: a weighted mean is the same when every weight is multiplied by a constant. Weights
    // [1, 3] with values [1, 0] give 1/4 at every scale that holds the ratio exactly: powers of two
    let mean = |scale: f64| {
        let mut b = SignalBridge::new(4, INF, 0.0, 1.0, 1).unwrap();
        // in two steps, since 2^1030 on its way to 2^-1030 would overflow
        let scale = scale * 2f64.powi(-1000);
        b.add_source("a", scale).unwrap();
        b.add_source("b", 3.0 * scale).unwrap();
        b.update("a", 1.0, 1.0).unwrap();
        b.update("b", 0.0, 1.0).unwrap();
        b.tick(1.0).unwrap().position
    };
    for exp in [2000, 1967, 1960, 1000, 978, 970, 928, 1999] {
        assert_eq!(mean(2f64.powi(exp - 1000)), 0.25, "2^{}", exp - 2000);
    }
    // one source alone, at the values the old cut-off turned to 0, and the smallest there is
    for w in [1e-9, 1e-10, 1e-11, 1e-300, f64::MIN_POSITIVE, 5e-324] {
        let mut b = SignalBridge::new(4, INF, 0.0, 1.0, 1).unwrap();
        b.add_source("a", w).unwrap();
        b.update("a", 0.75, 1.0).unwrap();
        assert_eq!(b.tick(1.0).unwrap().position, 0.75, "{w:e}");
    }
}

#[test]
fn only_weights_of_exactly_zero_leave_nothing_to_average() {
    let mut b = SignalBridge::new(4, INF, 2.0, 3.0, 1).unwrap();
    b.add_source("a", 0.0).unwrap();
    b.update("a", 1.0, 1.0).unwrap();
    assert_eq!(b.tick(1.0).unwrap().position, 2.0);
}

#[test]
fn a_refused_update_changes_nothing() {
    let mut b = SignalBridge::new(4, INF, 0.0, 1.0, 2).unwrap();
    b.add_source("s", 1.0).unwrap();
    b.update("s", 0.5, 1.0).unwrap();
    for (value, time) in [(NAN, 1.0), (INF, 1.0), (0.9, NAN), (0.9, INF)] {
        assert_eq!(b.update("s", value, time), Err(NotFinite), "{value} {time}");
    }
    // neither the buffer nor the time of the source moved
    assert_eq!(b.tick(1.0).unwrap().position, 0.5);
    assert_eq!(b.active_sources(), ["s"]);
}
