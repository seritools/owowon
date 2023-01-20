use crate::data::units::Voltage;

/// Supported vertical scales (per division) at 1X probe attenuation.
pub const VERTICAL_SCALES: [Voltage; 10] = [
    Voltage(0.01),
    Voltage(0.02),
    Voltage(0.05),
    Voltage(0.1),
    Voltage(0.2),
    Voltage(0.5),
    Voltage(1.0),
    Voltage(2.0),
    Voltage(5.0),
    Voltage(10.0),
];
