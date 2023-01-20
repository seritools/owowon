use arrayvec::ArrayVec;
use data::{
    awg::AwgConfig,
    head::{
        Channel, ChannelCoupling, ChannelDisplay, DataHead, MemoryDepth, SampleType,
        TriggerCoupling, TriggerEdge, TriggerSweep,
    },
    measurement::Measurements,
    units::{ProbeAttenuation, Time, Voltage},
};
use std::time::Duration;

pub mod consts;
pub mod data;
pub mod device;
pub mod scaled_number;

#[derive(Debug)]
pub enum OscilloscopeRunCommand {
    Command(OscilloscopeCommand),
    RunSetting(OscilloscopeRunSetting),
}

impl From<OscilloscopeCommand> for OscilloscopeRunCommand {
    fn from(cmd: OscilloscopeCommand) -> Self {
        Self::Command(cmd)
    }
}
impl From<OscilloscopeRunSetting> for OscilloscopeRunCommand {
    fn from(cmd: OscilloscopeRunSetting) -> Self {
        Self::RunSetting(cmd)
    }
}

#[derive(Debug)]
pub enum OscilloscopeCommand {
    SetHorizontalOffset(f64),
    SetChannelDisplay(Channel, ChannelDisplay),
    SetChannelVOffset(Channel, f64),
    SetChannelVScale(Channel, Voltage),
    SetChannelCoupling(Channel, ChannelCoupling),
    SetChannelAttenuation(Channel, ProbeAttenuation),
    SetTimeScale(Time),
    SetTriggerSource(Channel),
    SetTriggerEdge(TriggerEdge),
    SetTriggerLevel(Voltage),
    SetTriggerSweep(TriggerSweep),
    SetTriggerCoupling(TriggerCoupling),
    SetAcquisitionMode(SampleType),
    SetAcquisitionDepth(MemoryDepth),
    Auto,
}

#[derive(Debug)]
pub enum OscilloscopeRunSetting {
    SetMeasurementsEnabled(bool),
    ReadAwgConfig,
    SetAwgConfig(AwgConfig),
}

#[allow(clippy::large_enum_variant)]
#[derive(Debug)]
pub enum OscilloscopeMessage {
    Data(OscilloscopeData),
    Awg(AwgConfig),
}

#[derive(Debug, Default)]
pub struct OscilloscopeData {
    pub measurements: Option<[Measurements; 2]>,
    pub signal_data: SignalData,
    pub acquisition_duration: Duration,
}

#[derive(Debug, Default)]
pub struct SignalData {
    pub head: DataHead,
    pub ch0_data: Option<ArrayVec<u8, 1024>>,
    pub ch1_data: Option<ArrayVec<u8, 1024>>,
}

#[derive(Debug, Default)]
pub struct InitialDeviceRunConfig {
    pub measurements_enabled: bool,
}
