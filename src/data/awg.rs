use super::units::{Frequency, Voltage};
use strum::{Display, EnumCount, EnumIter, EnumString};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, EnumString, EnumIter, EnumCount, Display)]
pub enum AwgMode {
    #[default]
    #[strum(serialize = "SINE")]
    Sine,
    #[strum(serialize = "SQUare")]
    Square,
    #[strum(serialize = "RAMP")]
    Ramp,
    #[strum(serialize = "PULSe")]
    Pulse,
    #[strum(serialize = "AmpALT")]
    AmpAlt,

    // BUG: these aren't ever returned from the API because yes. FW always returns `AmpALT` for these
    #[strum(serialize = "AttALT")]
    AttAlt,
    #[strum(serialize = "StairDn")]
    StairDown,
    #[strum(serialize = "StairUD")]
    StairUpDown,
    #[strum(serialize = "StairUp")]
    StairUp,
    #[strum(serialize = "Besselj")]
    BesselJ,
    #[strum(serialize = "Bessely")]
    BesselY,
    Sinc,
}

pub const AWG_MODES: [AwgMode; AwgMode::COUNT] = [
    AwgMode::Sine,
    AwgMode::Square,
    AwgMode::Ramp,
    AwgMode::Pulse,
    AwgMode::AmpAlt,
    AwgMode::AttAlt,
    AwgMode::StairDown,
    AwgMode::StairUpDown,
    AwgMode::StairUp,
    AwgMode::BesselJ,
    AwgMode::BesselY,
    AwgMode::Sinc,
];

#[derive(Debug, Clone, Copy)]
pub struct AwgConfig {
    pub enabled: bool,
    pub mode: AwgMode,
    pub frequency: Frequency,
    pub amplitude: Voltage,
    pub offset: Voltage,
}

impl Default for AwgConfig {
    fn default() -> Self {
        Self {
            enabled: Default::default(),
            mode: Default::default(),
            frequency: Frequency(1_000_000.0),
            amplitude: Voltage(1.0),
            offset: Default::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Display, EnumString)]
#[strum(serialize_all = "UPPERCASE")]
pub enum AwgChannelDisplay {
    On,
    #[default]
    Off,
}

impl From<AwgChannelDisplay> for bool {
    fn from(c: AwgChannelDisplay) -> Self {
        c == AwgChannelDisplay::On
    }
}

impl From<bool> for AwgChannelDisplay {
    fn from(b: bool) -> Self {
        if b {
            AwgChannelDisplay::On
        } else {
            AwgChannelDisplay::Off
        }
    }
}
