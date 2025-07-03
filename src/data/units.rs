use crate::scaled_number::{ScaledNumber, ScaledNumberExt, SiScale};
use serde_with::DeserializeFromStr;
use std::{
    fmt::{Display, Write},
    str::FromStr,
};

#[derive(Debug, Clone, Copy, Default, PartialEq, DeserializeFromStr)]
pub struct Frequency(pub f64);

impl FromStr for Frequency {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.strip_suffix("Hz").ok_or("not a sampling rate")?;
        Ok(Self(f64::parse_scaled(s).ok_or("invalid sampling rate")?))
    }
}

impl Display for Frequency {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ScaledNumber::from(self.0).fmt(f)?;
        f.write_str("Hz")
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, DeserializeFromStr)]
pub struct SamplingRate(pub f64);

impl FromStr for SamplingRate {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.strip_suffix("Sa/s").ok_or("not a sampling rate")?;
        Ok(Self(f64::parse_scaled(s).ok_or("invalid sampling rate")?))
    }
}

impl Display for SamplingRate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ScaledNumber::from(self.0).fmt(f)?;
        f.write_str("Sa/s")
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, DeserializeFromStr)]
pub struct Time(pub f64);

impl FromStr for Time {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.strip_suffix('s').ok_or("not a time value")?;
        Ok(Self(f64::parse_scaled(s).ok_or("invalid time value")?))
    }
}

impl Display for Time {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let (val, scale) = if self.0.abs() >= 1.0 {
            (self.0, SiScale::None)
        } else {
            self.0.unscale()
        };

        f.write_fmt(format_args!("{val:.0}"))?;
        if val.abs() < 10.0 {
            f.write_str(".0")?;
        }

        // respects alternate formatting
        scale.fmt(f)?;
        f.write_char('s')
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, DeserializeFromStr)]
pub struct Voltage(pub f64);

impl FromStr for Voltage {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.strip_suffix(['v', 'V']).ok_or("not a voltage")?;

        // necessary _sometimes_ (e.g HDS272S, see issue #6)
        let s = s.trim_ascii();

        Ok(Self(f64::parse_scaled(s).ok_or("invalid voltage")?))
    }
}

impl Display for Voltage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ScaledNumber::from(self.0).fmt(f)?;
        f.write_char('V')
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, DeserializeFromStr)]
pub struct ProbeAttenuation(pub u32);

impl FromStr for ProbeAttenuation {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s
            .strip_suffix(['x', 'X'])
            .ok_or("not an attenuation factor")?;

        Ok(Self(s.parse().map_err(|_| "invalid attenuation factor")?))
    }
}

impl Display for ProbeAttenuation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)?;
        f.write_char('X')
    }
}

impl Default for ProbeAttenuation {
    fn default() -> Self {
        Self(10)
    }
}
