use super::OscilloscopeUiState;
use crate::optional_sender::OptionalSender;
use owowon::{
    data::{
        awg::AwgConfig,
        head::{
            Channel, ChannelCoupling, MemoryDepth, SampleType, TriggerCoupling, TriggerEdge,
            TriggerSweep,
        },
        units::{ProbeAttenuation, Time, Voltage},
    },
    OscilloscopeCommand, OscilloscopeRunCommand, OscilloscopeRunSetting,
};

impl OptionalSender<OscilloscopeRunCommand> {
    pub fn run_auto(&self) {
        let _ = self.blocking_send(OscilloscopeCommand::Auto);
    }

    pub fn toggle_measurements(&self, osc_ui_state: &OscilloscopeUiState) {
        let measurements_enabled = osc_ui_state.measurements.is_some();
        self.set_measurements_enabled(!measurements_enabled)
    }

    pub fn set_measurements_enabled(&self, enabled: bool) {
        let _ = self.blocking_send(OscilloscopeRunSetting::SetMeasurementsEnabled(enabled));
    }

    pub fn set_channel_display(&self, channel: Channel, display: bool) {
        let _ = self.blocking_send(OscilloscopeCommand::SetChannelDisplay(
            channel,
            display.into(),
        ));
    }

    pub fn set_horizontal_offset(&self, offset_in_grid_units: f64) {
        let _ = self.blocking_send(OscilloscopeCommand::SetHorizontalOffset(
            offset_in_grid_units,
        ));
    }

    pub fn set_time_scale(&self, time_scale: Time) {
        let _ = self.blocking_send(OscilloscopeCommand::SetTimeScale(time_scale));
    }

    pub fn set_vertical_scale(
        &self,
        channel: Channel,
        scale: Voltage,
        attenuation: ProbeAttenuation,
    ) {
        let _ = self.blocking_send(OscilloscopeCommand::SetChannelVScale(
            channel,
            Voltage(scale.0 * (attenuation.0 as f64)),
        ));
    }

    pub fn set_vertical_offset(&self, channel: Channel, offset_in_grid_units: f64) {
        let _ = self.blocking_send(OscilloscopeCommand::SetChannelVOffset(
            channel,
            offset_in_grid_units,
        ));
    }

    pub fn set_channel_coupling(&self, channel: Channel, coupling: ChannelCoupling) {
        let _ = self.blocking_send(OscilloscopeCommand::SetChannelCoupling(channel, coupling));
    }

    pub fn set_acquisition_depth(&self, depth: MemoryDepth) {
        let _ = self.blocking_send(OscilloscopeCommand::SetAcquisitionDepth(depth));
    }

    pub fn set_acquisition_mode(&self, sample_type: SampleType) {
        let _ = self.blocking_send(OscilloscopeCommand::SetAcquisitionMode(sample_type));
    }

    pub fn set_trigger_level(&self, voltage: Voltage) {
        let _ = self.blocking_send(OscilloscopeCommand::SetTriggerLevel(voltage));
    }

    pub fn set_trigger_coupling(&self, trigger_coupling: TriggerCoupling) {
        let _ = self.blocking_send(OscilloscopeCommand::SetTriggerCoupling(trigger_coupling));
    }

    pub fn set_trigger_edge(&self, trigger_edge: TriggerEdge) {
        let _ = self.blocking_send(OscilloscopeCommand::SetTriggerEdge(trigger_edge));
    }

    pub fn set_trigger_sweep(&self, trigger_sweep: TriggerSweep) {
        let _ = self.blocking_send(OscilloscopeCommand::SetTriggerSweep(trigger_sweep));
    }

    pub fn set_trigger_source(&self, channel: Channel) {
        let _ = self.blocking_send(OscilloscopeCommand::SetTriggerSource(channel));
    }

    pub fn read_awg_config(&self) {
        let _ = self.blocking_send(OscilloscopeRunSetting::ReadAwgConfig);
    }

    pub fn set_awg_config(&self, config: AwgConfig) {
        let _ = self.blocking_send(OscilloscopeRunSetting::SetAwgConfig(config));
    }
}
