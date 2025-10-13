use super::units::{ProbeAttenuation, SamplingRate, Time, Voltage};
use crate::consts::GRID_DIV_SIZE;
use serde::Deserialize;
use strum::{Display, EnumString};

#[derive(Debug, Deserialize, Clone, Copy)]
#[serde(rename_all(deserialize = "UPPERCASE"))]
pub struct DataHeader {
    // pub idn: &'a str,
    // pub model: &'a str,
    #[serde(rename = "TIMEBASE")]
    pub time_base: TimeBase,
    pub sample: Sample,
    #[serde(rename = "CHANNEL")]
    pub channels: [ChannelInfo; 2],
    pub datatype: DataType,
    #[serde(rename = "RUNSTATUS")]
    pub run_status: RunStatus,
    #[serde(rename = "Trig")] // yes, exactly this spelling
    pub trigger: Trigger,
}

impl DataHeader {
    pub fn channel(&self, ch: Channel) -> &ChannelInfo {
        &self.channels[ch as usize]
    }

    pub fn channel_enabled(&self, ch: Channel) -> bool {
        self.channel(ch).display == ChannelDisplay::On
    }
}

impl Default for DataHeader {
    fn default() -> Self {
        Self {
            time_base: Default::default(),
            sample: Default::default(),
            channels: [
                ChannelInfo::default(),
                ChannelInfo {
                    channel: Channel::Ch2,
                    ..Default::default()
                },
            ],
            datatype: Default::default(),
            run_status: Default::default(),
            trigger: Default::default(),
        }
    }
}

#[derive(Debug, Deserialize, Default, Clone, Copy)]
#[serde(rename_all(deserialize = "UPPERCASE"))]
pub struct TimeBase {
    pub scale: Time,
    /// Horizontal offset, in samples
    #[serde(rename = "HOFFSET")]
    pub h_offset: i64,
}

impl TimeBase {
    pub fn h_offset_grid_divs(&self) -> f64 {
        self.h_offset as f64 / GRID_DIV_SIZE
    }
}

#[derive(Debug, Deserialize, Default, Clone, Copy)]
#[serde(rename_all(deserialize = "UPPERCASE"))]
pub struct Sample {
    pub fullscreen: i32,
    #[serde(rename = "SLOWMOVE")]
    pub slow_move: i32,
    #[serde(rename = "DATALEN")]
    pub data_len: i32,
    #[serde(rename = "SAMPLERATE")]
    pub sampling_rate: SamplingRate,
    #[serde(rename = "TYPE")]
    pub sample_type: SampleType,
    pub depmem: MemoryDepth,
}

#[derive(Debug, Deserialize, Default, Clone, Copy)]
#[serde(rename_all(deserialize = "UPPERCASE"))]
pub struct ChannelInfo {
    #[serde(rename = "NAME")]
    pub channel: Channel,
    pub display: ChannelDisplay,
    pub coupling: ChannelCoupling,
    pub probe: ProbeAttenuation,
    /// Channel scale, in volts per grid square (for some reason), unattenuated.
    pub scale: Voltage,
    /// Offset in units
    pub offset: i64,
    #[serde(rename = "FREQUENCE")]
    pub frequency: f64,
}

impl ChannelInfo {
    pub fn scale_per_unit(&self) -> f64 {
        self.scale.0 * self.probe.0 as f64 / GRID_DIV_SIZE
    }

    /// Calculate the scale in volts per grid square, with probe attenuation applied.
    pub fn scale_attenuated(&self) -> Voltage {
        Voltage(self.scale.0 * self.probe.0 as f64)
    }

    pub fn offset_grid_divs(&self) -> f64 {
        self.offset as f64 / GRID_DIV_SIZE
    }
}

#[derive(Debug, Deserialize, Default, Clone, Copy)]
#[serde(rename_all(deserialize = "UPPERCASE"))]
pub enum DataType {
    #[default]
    Screen,
}

#[derive(Debug, Deserialize, Default, Clone, Copy)]
#[serde(rename_all(deserialize = "PascalCase"))]
pub struct Trigger {
    pub mode: TriggerMode,
    pub r#type: TriggerType,
    pub items: TriggerItems,
}

#[derive(Debug, Deserialize, Default, Clone, Copy)]
#[serde(rename_all(deserialize = "PascalCase"))]
pub struct TriggerItems {
    pub channel: Channel,
    pub level: Voltage,
    pub edge: TriggerEdge,
    pub coupling: TriggerCoupling,
    pub sweep: TriggerSweep,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Display)]
pub enum TriggerEdge {
    #[default]
    #[serde(rename = "RISE", alias = "RISe")]
    #[strum(serialize = "RISE", serialize = "RISe")]
    Rising,
    #[serde(rename = "FALL", alias = "FALl")]
    #[strum(serialize = "FALL", serialize = "FALl")]
    Falling,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
pub enum TriggerMode {
    #[default]
    #[serde(rename = "SINGle")]
    Single,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
pub enum TriggerType {
    #[default]
    Edge,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Display)]
pub enum SampleType {
    #[default]
    #[serde(rename = "SAMPle")]
    Sample,
    #[serde(rename = "PEAK")]
    Peak,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Display)]
pub enum MemoryDepth {
    #[serde(rename = "4K")]
    #[strum(serialize = "4K")]
    FourK,
    #[default]
    #[serde(rename = "8K")]
    #[strum(serialize = "8K")]
    EightK,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Display)]
#[serde(rename_all(deserialize = "UPPERCASE"))]
pub enum Channel {
    #[default]
    #[strum(serialize = "CH1")]
    Ch1 = 0,
    #[strum(serialize = "CH2")]
    Ch2 = 1,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Display, EnumString)]
#[serde(rename_all(deserialize = "UPPERCASE"))]
pub enum ChannelDisplay {
    #[default]
    #[strum(serialize = "ON")]
    On,
    #[strum(serialize = "OFF")]
    Off,
}

impl From<ChannelDisplay> for bool {
    fn from(c: ChannelDisplay) -> Self {
        c == ChannelDisplay::On
    }
}

impl From<bool> for ChannelDisplay {
    fn from(b: bool) -> Self {
        if b {
            ChannelDisplay::On
        } else {
            ChannelDisplay::Off
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Display)]
#[serde(rename_all(deserialize = "UPPERCASE"))]
pub enum ChannelCoupling {
    #[default]
    #[strum(serialize = "DC")]
    Dc,
    #[strum(serialize = "AC")]
    Ac,
    #[strum(serialize = "GND")]
    Gnd,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Display)]
#[serde(rename_all(deserialize = "UPPERCASE"))]
pub enum TriggerCoupling {
    #[default]
    #[strum(serialize = "DC")]
    Dc,
    #[strum(serialize = "AC")]
    Ac,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Display)]
pub enum TriggerSweep {
    #[default]
    #[serde(rename = "AUTo", alias = "AUTO")]
    Auto,
    #[serde(rename = "NORMal")]
    Normal,
    #[serde(rename = "SINGlE")]
    Single,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Display)]
pub enum RunStatus {
    /// Scanning, trigger disabled (time 100ms)
    #[serde(rename = "SCAN")]
    Scanning,
    /// Trigger armed/ready (in normal/single mode)
    #[default]
    #[serde(rename = "end")]
    #[strum(serialize = "Ready")]
    NotStarted,
    /// Trigger armed/ready (in normal/single mode)
    #[serde(rename = "READy")]
    Ready,
    /// Triggered in normal/single mode
    #[serde(rename = "STOP")]
    Stopped,

    /// Armed/scanning in auto mode
    #[serde(rename = "AUTo")]
    Auto,
    /// Triggered in auto mode
    #[serde(rename = "TRIG")]
    Triggering,
}
