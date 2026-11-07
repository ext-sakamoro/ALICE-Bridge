#!/usr/bin/env python3
"""Breaks the refusals of `SignalBridge` one way at a time; tests/signal_limits.rs must go red.

usage: python3 scripts/signal_mutants.py
exit 0 if every mutant applied, compiled and made a test report FAILED; exit 1 otherwise.

Run by hand when src/bridge/signal.rs changes. A mutant that does not compile or no longer applies
counts against the result. A mutant past the time limit is killed with its process group and counts
as red. The file is restored after each mutant.
"""
import os
import re
import signal
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SOURCE = ROOT / "src" / "bridge" / "signal.rs"
COMMAND = ["cargo", "test", "--test", "signal_limits"]
FAILED = re.compile(r"^test .* \.\.\. .*FAILED|signal: \d+", re.M)
TIMEOUT_SECS = 300

RANGE = "if !(min_position <= max_position && (max_position - min_position).is_finite()) {"
# (what breaks, old text, new text)
MUTANTS = [
    ("no range check", RANGE, "if false {"),
    ("a reversed range allowed", RANGE, "if !(max_position - min_position).is_finite() {"),
    ("the width not checked", RANGE, "if !(min_position <= max_position) {"),
    ("NaN speed allowed", "if speed_limit.is_nan() || speed_limit < 0.0 {", "if speed_limit < 0.0 {"),
    ("negative speed allowed", "if speed_limit.is_nan() || speed_limit < 0.0 {", "if speed_limit.is_nan() {"),
    ("+inf speed refused", "if speed_limit.is_nan() || speed_limit < 0.0 {", "if !speed_limit.is_finite() || speed_limit < 0.0 {"),
    ("a weight that is not finite allowed", "if !weight.is_finite() || weight < 0.0 {", "if weight < 0.0 {"),
    ("a negative weight allowed", "if !weight.is_finite() || weight < 0.0 {", "if !weight.is_finite() {"),
    ("a NaN intensity kept", "if !intensity.is_finite() || !now_secs.is_finite() {", "if !now_secs.is_finite() {"),
    ("a NaN time kept in update", "if !intensity.is_finite() || !now_secs.is_finite() {", "if !intensity.is_finite() {"),
    ("a NaN time kept in tick", "        if !now_secs.is_finite() {\n            return Err(SignalError::NotFinite);\n        }\n        // no earlier", "        // no earlier"),
    ("weights not rescaled when their sum overflows", "if total_weight.is_normal() {", "if true {"),
    ("weights rescaled only when their sum overflows", "if total_weight.is_normal() {", "if total_weight.is_finite() {"),
    ("a small sum of weights reads as none", "if largest == 0.0 {", "if largest < 1e-10 {"),
    ("no check for weights of zero", "        if largest == 0.0 {\n            return 0.0;\n        }\n", ""),
    ("rescaled the wrong way", "let weight = src.weight / divisor;", "let weight = src.weight * divisor;"),
    ("the time goes back with the clock", "|last| last.max(now_secs)", "|_| now_secs"),
    ("time that goes back counts", ".clamp(0.0, f64::MAX)", ".clamp(f64::MIN, f64::MAX)"),
    ("time that goes back counts by its size", "(now_secs - last).clamp(0.0, f64::MAX)", "(now_secs - last).abs().min(f64::MAX)"),
    ("an infinite difference of times stays infinite", ".clamp(0.0, f64::MAX)", ".max(0.0)"),
    ("the first tick is limited", "self\n            .last_tick\n            .map(|last| (now_secs - last).clamp(0.0, f64::MAX));", "Some(self.last_tick.map_or(0.0, |last| (now_secs - last).clamp(0.0, f64::MAX)));"),
    ("rescaled by the wrong weight", ".fold(0.0, |m: f64, s| m.max(s.weight));", ".fold(f64::MAX, |m: f64, s| m.min(s.weight));"),
    ("the window preallocated", "buffer: VecDeque::new(),", "buffer: VecDeque::with_capacity(self.smooth_window),"),
    ("the speed limit direction flipped", "let sign = if delta > 0.0 { 1.0 } else { -1.0 };", "let sign = if delta > 0.0 { -1.0 } else { 1.0 };"),
    ("no speed limit", "if delta.abs() > max_delta {", "if false {"),
    ("the limit moves twice as far", "let max_delta = self.speed_limit * dt;", "let max_delta = self.speed_limit * dt * 2.0;"),
    ("a refused value marks the source active", "        if !intensity.is_finite() || !now_secs.is_finite() {\n            return Err(SignalError::NotFinite);\n        }\n        for src in &mut self.sources {", "        if !intensity.is_finite() || !now_secs.is_finite() {\n            for src in &mut self.sources {\n                if src.name == source_name {\n                    src.active = true;\n                }\n            }\n            return Err(SignalError::NotFinite);\n        }\n        for src in &mut self.sources {"),
    ("the output not clamped", "        target = target.clamp(self.min_position, self.max_position);\n", ""),
]


def run_tests() -> tuple[int | None, str]:
    p = subprocess.Popen(
        COMMAND, cwd=ROOT, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True,
        start_new_session=True,
    )
    try:
        out, _ = p.communicate(timeout=TIMEOUT_SECS)
        return p.returncode, out
    except subprocess.TimeoutExpired:
        os.killpg(p.pid, signal.SIGKILL)
        out, _ = p.communicate()
        return None, out


def main() -> int:
    if sys.argv[1:]:
        print(__doc__, file=sys.stderr)
        return 2
    original = SOURCE.read_text(encoding="utf-8")
    bad = []
    try:
        for name, old, new in MUTANTS:
            if original.count(old) != 1:
                bad.append(name)
                print(f"NOT APPLIED  {name}", flush=True)
                continue
            SOURCE.write_text(original.replace(old, new), encoding="utf-8")
            rc, output = run_tests()
            if "could not compile" in output:
                bad.append(name)
                print(f"BROKEN       {name}", flush=True)
            elif rc is None:
                print(f"red (killed) {name}", flush=True)
            elif rc != 0 and FAILED.search(output):
                print(f"red          {name}", flush=True)
            else:
                bad.append(name)
                print(f"SURVIVED     {name} (rc {rc})", flush=True)
    finally:
        SOURCE.write_text(original, encoding="utf-8")
    print(f"{len(MUTANTS) - len(bad)} of {len(MUTANTS)} red")
    return 1 if bad or not MUTANTS else 0


if __name__ == "__main__":
    sys.exit(main())
