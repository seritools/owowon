use crate::scaled_number::{ScaledNumber, ScaledNumberExt};
use serde_with::DeserializeFromStr;
use std::str::FromStr;

// Measurements:
// PKPK peak-to-peak voltage Vpp=3.720V
// VAMP voltage amplitude Va=3.640V
// AVERage average voltage V=1.822V
// PERiod: cycle time T=1000.0us
// FREQuency: frequency F=1.000kHz
// RTime: Rise time: RT=32.00ns
// FTime: Fall time: FT=32.00ns
// PWIDth: Peak width: PW=1000.0us
// NWIDth: Trough width: NW=1.500ms
// SQUAresum: RMS: RMS=50.21mV

macro_rules! decl_measurement {
    ($name:ident, $prefix:expr, $unit:literal) => {
        decl_measurement!($name, $prefix, $unit, $prefix);
    };
    ($name:ident, $prefix:expr, $unit:literal, $disp_prefix:expr) => {
        #[derive(Debug, Default, PartialEq, DeserializeFromStr)]
        pub struct $name(pub Option<ScaledNumber>);

        impl FromStr for $name {
            type Err = &'static str;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                let s = s
                    .strip_prefix(concat!($prefix, "="))
                    .ok_or("not a measurement")?;

                if s.ends_with("?\n") || s.ends_with("OFF\n") {
                    Ok(Self(None))
                } else {
                    let s = s
                        .strip_suffix(concat!($unit, "\n"))
                        .expect("not a measurement");
                    Ok(Self(Some(ScaledNumber(
                        f64::parse_scaled(s).expect("invalid measurement"),
                    ))))
                }
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str(concat!($disp_prefix, "="))?;
                if let Some(val) = &self.0 {
                    val.fmt(f)?;
                    f.write_str($unit)
                } else {
                    Ok(())
                }
            }
        }
    };
}

decl_measurement!(PeakToPeak, "Vpp", "V");
decl_measurement!(Amplitude, "Va", "V");
decl_measurement!(Average, "V", "V", "Vavg");
decl_measurement!(Period, "T", "s");
decl_measurement!(Frequency, "F", "Hz");
decl_measurement!(RiseTime, "RT", "s");
// decl_measurement!(FallTime, "FT", "s"); // slightly buggy
decl_measurement!(PeakWidth, "PW", "s");
decl_measurement!(TroughWidth, "NW", "s");
decl_measurement!(Rms, "RMS", "V");
