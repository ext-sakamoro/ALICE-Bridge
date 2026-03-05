//! Device abstraction — unified model for any hardware device.

mod actuator;
mod manager;
pub mod mapping;

pub use actuator::{Actuator, ActuatorType};
pub use manager::{Device, DeviceId, DeviceManager};
pub use mapping::DeviceMapping;
