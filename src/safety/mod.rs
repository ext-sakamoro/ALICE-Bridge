//! Safety layer — intensity limiting, emergency stop, gradual ramp.
//!
//! Protocol-agnostic safety enforcement that applies to any hardware.

mod emergency;
mod limiter;
pub mod ramp;

pub use emergency::EmergencyStop;
pub use limiter::{IntensityLimiter, SafetyLimits};
pub use ramp::GradualRamp;
