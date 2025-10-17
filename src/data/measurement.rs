use super::head::Channel;
use arrayvec::ArrayVec;

mod data;
use crate::scaled_number::ScaledNumber;
pub use data::*;

#[derive(Debug, Default)]
pub struct Measurements {
    pub peak_to_peak: PeakToPeak,
    pub amplitude: Amplitude,
    pub average: Average,
    pub frequency: Frequency,
    pub period: Period,
    pub rise_time: RiseTime,
    pub peak_width: PeakWidth,
    // frequency is derived from period
    // trough width is derived from peak width
}

impl Measurements {
    pub const MEASUREMENT_COUNT: usize = 8;

    pub fn with_parsed(&mut self, buf: &str) {
        if let Ok(peak_to_peak) = buf.parse() {
            self.peak_to_peak = peak_to_peak;
        }
        if let Ok(amplitude) = buf.parse() {
            self.amplitude = amplitude;
        }
        if let Ok(average) = buf.parse() {
            self.average = average;
        }
        if let Ok(period) = buf.parse() {
            self.period = period;
        }
        if let Ok(rise_time) = buf.parse() {
            self.rise_time = rise_time;
        }
        if let Ok(peak_width) = buf.parse() {
            self.peak_width = peak_width;
        }
    }

    pub fn for_display(&self) -> ArrayVec<String, { Self::MEASUREMENT_COUNT }> {
        let mut out = ArrayVec::new();
        out.push(self.peak_to_peak.to_string());
        out.push(self.amplitude.to_string());
        out.push(Frequency(self.period.0.map(|s| ScaledNumber(s.0.powi(-1)))).to_string());
        out.push(self.period.to_string());

        out.push(self.rise_time.to_string());
        out.push(self.peak_width.to_string());

        out.push(
            TroughWidth(
                self.period
                    .0
                    .zip(self.peak_width.0)
                    .map(|(period, peak)| ScaledNumber(period.0 - peak.0)),
            )
            .to_string(),
        );

        out.push(self.average.to_string());
        out
    }

    pub fn channel_to_measurement_commands(ch: Channel) -> &'static [&'static [u8]] {
        // :MEASuremen (older FW)
        // :MEASurement (newer FW)
        if ch == Channel::Ch1 {
            &[
                b":MEAS:CH1:PKPK?",
                b":MEAS:CH1:VAMP?",
                b":MEAS:CH1:AVER?",
                b":MEAS:CH1:PER?",
                b":MEAS:CH1:RT?",
                b":MEAS:CH1:PWID?",
            ]
        } else {
            &[
                b":MEAS:CH2:PKPK?",
                b":MEAS:CH2:VAMP?",
                b":MEAS:CH2:AVER?",
                b":MEAS:CH2:PER?",
                b":MEAS:CH2:RT?",
                b":MEAS:CH2:PWID?",
            ]
        }
    }
}
