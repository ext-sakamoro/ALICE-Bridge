//! Actuator types — universal actuator abstraction.

use serde::{Deserialize, Serialize};

/// Actuator type enumeration (superset of Buttplug.io v3 `ActuatorType`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ActuatorType {
    Vibrate,
    Rotate,
    Oscillate,
    Constrict,
    Inflate,
    Heat,
    Electrostimulate,
    Linear,
    Position,
    /// Custom type for protocol-specific actuators.
    Custom,
}

impl ActuatorType {
    #[must_use]
    pub fn parse(s: &str) -> Self {
        match s {
            "Vibrate" => Self::Vibrate,
            "Rotate" => Self::Rotate,
            "Oscillate" => Self::Oscillate,
            "Constrict" => Self::Constrict,
            "Inflate" => Self::Inflate,
            "Heat" => Self::Heat,
            "Electrostimulate" => Self::Electrostimulate,
            "Linear" => Self::Linear,
            "Position" => Self::Position,
            _ => Self::Custom,
        }
    }

    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Vibrate => "Vibrate",
            Self::Rotate => "Rotate",
            Self::Oscillate => "Oscillate",
            Self::Constrict => "Constrict",
            Self::Inflate => "Inflate",
            Self::Heat => "Heat",
            Self::Electrostimulate => "Electrostimulate",
            Self::Linear => "Linear",
            Self::Position => "Position",
            Self::Custom => "Custom",
        }
    }

    /// Whether this actuator type is potentially dangerous and needs safety limits.
    #[must_use]
    pub const fn is_safety_critical(&self) -> bool {
        matches!(
            self,
            Self::Heat | Self::Electrostimulate | Self::Constrict | Self::Inflate
        )
    }
}

/// A single actuator on a device.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Actuator {
    /// Actuator index on the device.
    pub index: u32,
    /// Type of actuation.
    pub actuator_type: ActuatorType,
    /// Human-readable description.
    pub description: String,
    /// Number of discrete intensity steps.
    pub step_count: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn actuator_type_roundtrip() {
        for atype in [
            ActuatorType::Vibrate,
            ActuatorType::Rotate,
            ActuatorType::Heat,
            ActuatorType::Electrostimulate,
            ActuatorType::Linear,
            ActuatorType::Custom,
        ] {
            assert_eq!(ActuatorType::parse(atype.as_str()), atype);
        }
    }

    #[test]
    fn safety_critical() {
        assert!(ActuatorType::Heat.is_safety_critical());
        assert!(ActuatorType::Electrostimulate.is_safety_critical());
        assert!(ActuatorType::Constrict.is_safety_critical());
        assert!(!ActuatorType::Vibrate.is_safety_critical());
        assert!(!ActuatorType::Linear.is_safety_critical());
    }

    #[test]
    fn parse_all_variants() {
        assert_eq!(ActuatorType::parse("Vibrate"), ActuatorType::Vibrate);
        assert_eq!(ActuatorType::parse("Rotate"), ActuatorType::Rotate);
        assert_eq!(ActuatorType::parse("Oscillate"), ActuatorType::Oscillate);
        assert_eq!(ActuatorType::parse("Constrict"), ActuatorType::Constrict);
        assert_eq!(ActuatorType::parse("Inflate"), ActuatorType::Inflate);
        assert_eq!(ActuatorType::parse("Heat"), ActuatorType::Heat);
        assert_eq!(
            ActuatorType::parse("Electrostimulate"),
            ActuatorType::Electrostimulate
        );
        assert_eq!(ActuatorType::parse("Linear"), ActuatorType::Linear);
        assert_eq!(ActuatorType::parse("Position"), ActuatorType::Position);
        assert_eq!(ActuatorType::parse("Custom"), ActuatorType::Custom);
    }

    #[test]
    fn parse_unknown_is_custom() {
        assert_eq!(ActuatorType::parse(""), ActuatorType::Custom);
        assert_eq!(ActuatorType::parse("vibrate"), ActuatorType::Custom);
        assert_eq!(ActuatorType::parse("FooBar"), ActuatorType::Custom);
    }

    #[test]
    fn as_str_all_variants() {
        assert_eq!(ActuatorType::Vibrate.as_str(), "Vibrate");
        assert_eq!(ActuatorType::Rotate.as_str(), "Rotate");
        assert_eq!(ActuatorType::Oscillate.as_str(), "Oscillate");
        assert_eq!(ActuatorType::Constrict.as_str(), "Constrict");
        assert_eq!(ActuatorType::Inflate.as_str(), "Inflate");
        assert_eq!(ActuatorType::Heat.as_str(), "Heat");
        assert_eq!(ActuatorType::Electrostimulate.as_str(), "Electrostimulate");
        assert_eq!(ActuatorType::Linear.as_str(), "Linear");
        assert_eq!(ActuatorType::Position.as_str(), "Position");
        assert_eq!(ActuatorType::Custom.as_str(), "Custom");
    }

    #[test]
    fn safety_critical_inflate() {
        assert!(ActuatorType::Inflate.is_safety_critical());
    }

    #[test]
    fn safety_non_critical_all() {
        assert!(!ActuatorType::Rotate.is_safety_critical());
        assert!(!ActuatorType::Oscillate.is_safety_critical());
        assert!(!ActuatorType::Position.is_safety_critical());
        assert!(!ActuatorType::Custom.is_safety_critical());
    }

    #[test]
    fn roundtrip_full_set() {
        let all = [
            ActuatorType::Vibrate,
            ActuatorType::Rotate,
            ActuatorType::Oscillate,
            ActuatorType::Constrict,
            ActuatorType::Inflate,
            ActuatorType::Heat,
            ActuatorType::Electrostimulate,
            ActuatorType::Linear,
            ActuatorType::Position,
            ActuatorType::Custom,
        ];
        for atype in all {
            assert_eq!(ActuatorType::parse(atype.as_str()), atype);
        }
    }

    #[test]
    fn actuator_struct_fields() {
        let a = Actuator {
            index: 3,
            actuator_type: ActuatorType::Heat,
            description: "Heater #3".into(),
            step_count: 50,
        };
        assert_eq!(a.index, 3);
        assert_eq!(a.actuator_type, ActuatorType::Heat);
        assert_eq!(a.description, "Heater #3");
        assert_eq!(a.step_count, 50);
    }

    #[test]
    fn actuator_type_clone_eq() {
        let a = ActuatorType::Vibrate;
        let b = a;
        assert_eq!(a, b);
        let c = ActuatorType::Heat;
        assert_ne!(a, c);
    }

    #[test]
    fn serde_roundtrip() {
        let atype = ActuatorType::Electrostimulate;
        let json = serde_json::to_string(&atype).unwrap();
        let parsed: ActuatorType = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, atype);
    }
}
