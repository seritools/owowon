use derive_more::From;
use std::{
    fmt::{Display, Write},
    str::FromStr,
};
use strum::FromRepr;

#[derive(Debug, Clone, Copy, From)]
pub struct DynamicDecimals(pub f64, pub usize);

impl Display for DynamicDecimals {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let abs = self.0.abs();

        let prec = if abs == 0.0 {
            self.1
        } else {
            self.1 - abs.log10().floor().max(0.0) as usize
        };

        if f.sign_plus() {
            f.write_fmt(format_args!("{:+.prec$}", self.0))
        } else {
            f.write_fmt(format_args!("{:.prec$}", self.0))
        }
    }
}

#[derive(Debug, Clone, Copy, From, PartialEq)]
pub struct ScaledNumber(pub f64);

impl Display for ScaledNumber {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let (unscaled, scale) = self.0.unscale();

        DynamicDecimals::from((unscaled, f.precision().unwrap_or(3))).fmt(f)?;

        // respects alternate formatting
        scale.fmt(f)
    }
}

impl FromStr for ScaledNumber {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(ScaledNumber(
            f64::parse_scaled(s).ok_or("invalid scaled number")?,
        ))
    }
}

pub trait ScaledNumberExt: Sized {
    fn from_scale(unscaled: f64, scale: SiScale) -> Self;
    fn unscale(self) -> (f64, SiScale);
    fn parse_scaled(s: &str) -> Option<Self>;
}

impl ScaledNumberExt for f64 {
    fn from_scale(unscaled: f64, scale: SiScale) -> Self {
        scale.apply_to(unscaled)
    }

    fn unscale(mut self) -> (f64, SiScale) {
        if self == 0.0 {
            return (0.0, SiScale::None);
        }

        let sign = self.signum();
        self = self.abs();

        let exp = ((self.log10() / 3.0).floor() * 3.0) as i32;
        self *= 10.0f64.powi(-exp);
        (
            self * sign,
            SiScale::from_repr(exp as i8).expect("invalid SI scale"),
        )
    }

    fn parse_scaled(s: &str) -> Option<Self> {
        let mut scale = SiScale::None;
        let last = s.chars().rev().next()?;

        let mut val = if last.is_ascii_digit() {
            s.parse::<f64>().ok()?
        } else {
            assert!(last.len_utf8() == 1);
            scale = SiScale::try_from(last).ok()?;
            s[..s.len() - 1].trim_end().parse::<f64>().ok()?
        };

        val = scale.apply_to(val);

        Some(val)
    }
}

#[derive(Debug, Default, FromRepr, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(i8)]
pub enum SiScale {
    Pico = -12,
    Nano = -9,
    Micro = -6,
    Milli = -3,
    #[default]
    None = 0,
    Kilo = 3,
    Mega = 6,
    Giga = 9,
}

impl SiScale {
    pub const fn prev(self) -> Option<Self> {
        Self::from_repr((self as i8) - 3)
    }

    pub const fn next(self) -> Option<Self> {
        Self::from_repr((self as i8) + 3)
    }

    pub fn apply_to(self, unscaled: f64) -> f64 {
        unscaled * 10.0f64.powi(self as i8 as i32)
    }
}

impl TryFrom<char> for SiScale {
    type Error = ();
    fn try_from(c: char) -> Result<Self, Self::Error> {
        match c {
            'p' => Ok(SiScale::Pico),
            'n' => Ok(SiScale::Nano),
            'u' => Ok(SiScale::Micro),
            'm' => Ok(SiScale::Milli),
            'k' | 'K' => Ok(SiScale::Kilo),
            'M' => Ok(SiScale::Mega),
            'G' => Ok(SiScale::Giga),
            _ => Err(()),
        }
    }
}

impl Display for SiScale {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let c = match self {
            SiScale::Pico => 'p',
            SiScale::Nano => 'n',
            SiScale::Micro => {
                if f.alternate() {
                    'u'
                } else {
                    'µ'
                }
            }
            SiScale::Milli => 'm',
            SiScale::None => return Ok(()),
            SiScale::Kilo => 'k',
            SiScale::Mega => 'M',
            SiScale::Giga => 'G',
        };

        f.write_char(c)
    }
}
