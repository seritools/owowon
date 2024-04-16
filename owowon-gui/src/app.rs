use self::{
    shortcuts::*,
    utils::{
        calc_new_horizontal_offset, calc_new_trigger_level, calc_new_vertical_offset,
        selected_time_base, selected_voltage,
    },
};
use crate::{
    device_run::{DeviceRun, DeviceRunState},
    device_select::DeviceSelector,
    optional_sender::OptionalSender,
    selectable_label_full_width::SelectableLabelFullWidth,
};
use egui::{
    vec2, Align, Color32, Context, Direction, FontFamily, FontId, Label, Layout, RichText,
    ScrollArea, TextStyle, Ui,
};
use owowon::{
    data::{
        awg::{AwgConfig, AWG_MODES},
        head::{Channel, DataHeader, RunStatus},
        measurement::Measurements,
    },
    device::Device,
    InitialDeviceRunConfig, OscilloscopeMessage, OscilloscopeRunCommand,
};
use std::{collections::HashMap, fmt::Write, sync::Arc, time::Duration};
use tokio::sync::RwLock;
use windows::{core::HSTRING, Devices::Enumeration::DeviceInformation};

mod cmds;
mod plot;
mod shortcuts;
mod side_panel;
mod utils;

#[derive(Default)]
pub struct OscilloscopeUiState {
    head: DataHeader,
    ch1_data: Vec<u8>,
    ch2_data: Vec<u8>,
    measurements: Option<[Measurements; 2]>,
    acquisition_duration: Duration,
}

#[derive(Default, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct PersistentState {
    selected_device: Option<String>,
    measurements_enabled: bool,
}

#[derive(Default)]
pub struct AwgState {
    pub mode_index: usize,
    pub frequency: String,
    pub amplitude: String,
    pub offset: String,

    pub config: AwgConfig,
}

#[derive(Default)]
pub struct OwowonApp {
    persistent_state: PersistentState,

    ch1_offset_string: String,
    ch2_offset_string: String,
    horizontal_offset_string: String,
    trigger_level_string: String,

    osc_ui_state: OscilloscopeUiState,

    last_device_error: Option<String>,
    device_selector: Option<DeviceSelector>,
    device_run: DeviceRunState,

    awg_state: AwgState,
}

impl OwowonApp {
    /// Called once before the first frame.
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        cc.egui_ctx.set_visuals(egui::Visuals::dark());
        let mut style: egui::Style = (*cc.egui_ctx.style()).clone();
        style.text_styles = [
            (
                TextStyle::Small,
                FontId::new(12.0, FontFamily::Proportional),
            ),
            (TextStyle::Body, FontId::new(16.0, FontFamily::Proportional)),
            (
                TextStyle::Heading,
                FontId::new(26.0, FontFamily::Proportional),
            ),
            (
                TextStyle::Button,
                FontId::new(20.0, FontFamily::Proportional),
            ),
            (
                TextStyle::Monospace,
                FontId::new(18.0, FontFamily::Monospace),
            ),
        ]
        .into();

        style.spacing.item_spacing = vec2(8.0, 6.0);
        cc.egui_ctx.set_style(style);

        // Load previous app state (if any).
        // Note that you must enable the `persistence` feature for this to work.
        let persistent_state: PersistentState = cc
            .storage
            .and_then(|storage| eframe::get_value(storage, eframe::APP_KEY))
            .unwrap_or_default();

        let app = OwowonApp {
            persistent_state,
            ..Default::default()
        };

        let egui_ctx = cc.egui_ctx.clone();

        let device_selector = DeviceSelector::new(move || egui_ctx.request_repaint()).ok();

        let selected_device_id: Option<HSTRING> = app
            .persistent_state
            .selected_device
            .as_ref()
            .map(Into::into);

        let mut app = OwowonApp {
            device_selector,
            ..app
        };

        if let Some(selected_device_id) = selected_device_id {
            app.try_select_device(selected_device_id, &cc.egui_ctx);
        }

        app
    }
}

impl eframe::App for OwowonApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let device_list = match self.device_list_or_fail_ui(ctx) {
            Some(value) => value,
            None => return,
        };

        self.update_device_run();
        self.persistent_state.measurements_enabled = self.osc_ui_state.measurements.is_some();
        let command_tx = match &mut self.device_run {
            DeviceRunState::Running(run) => Some(run.command_channel().clone()),
            _ => None,
        };

        let command_tx = OptionalSender(command_tx);

        egui::SidePanel::right("side_panel")
            .resizable(true)
            .min_width(280.0)
            .show(ctx, |ui| {
                ui.set_enabled(self.device_run.is_running());
                ScrollArea::vertical().show(ui, |ui| side_panel::ui(self, ui, &command_tx));
            });

        egui::TopBottomPanel::top("top_bar").show(ctx, |ui| self.top_panel_ui(ui));

        if let Some(measurements) = &self.osc_ui_state.measurements {
            egui::TopBottomPanel::bottom("bottom_bar").show(ctx, |ui| {
                ui.set_enabled(self.device_run.is_running());
                bottom_panel_ui(ui, &self.osc_ui_state.head, measurements);
            });
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            if !self.device_run.is_running() {
                if let Some(error) = &self.last_device_error {
                    ui.heading("Last error");
                    ui.label(error);
                }
                ui.heading("Select device");
                ui.group(|ui| {
                    let devices = device_list.blocking_read();
                    if devices.is_empty() {
                        ui.label("No devices found");
                        return;
                    }

                    for device in device_list.blocking_read().values() {
                        let device_id = device.Id().unwrap();
                        if ui
                            .add(SelectableLabelFullWidth::new(false, device_id.to_string()))
                            .clicked()
                        {
                            self.persistent_state.selected_device = Some(device_id.to_string());
                            self.try_select_device(device_id, ctx);
                        }
                    }
                });

                if ui.button("Quit").clicked() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }

                return;
            }

            plot::ui(self, ui)
        });

        if self.device_run.is_running() && !ctx.wants_keyboard_input() {
            self.handle_shortcuts(ctx, &command_tx);
        }
    }

    fn persist_egui_memory(&self) -> bool {
        false
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, &self.persistent_state);
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.device_run.stop();
    }
}

impl OwowonApp {
    fn handle_shortcuts(
        &mut self,
        ctx: &egui::Context,
        cmd: &OptionalSender<OscilloscopeRunCommand>,
    ) {
        let head = &self.osc_ui_state.head;
        ctx.input_mut(|input| {
            if input.consume_shortcut(&TOGGLE_MEASUREMENT) {
                cmd.toggle_measurements(&self.osc_ui_state);
            }

            let ch = if !head.channel_enabled(Channel::Ch2) {
                Channel::Ch1
            } else if !head.channel_enabled(Channel::Ch1) || input.modifiers.alt {
                Channel::Ch2
            } else {
                Channel::Ch1
            };

            let channel_info = head.channel(ch);

            let try_zoom_out_vertical = || {
                if let (_, _, Some(smaller), _) = selected_voltage(channel_info) {
                    cmd.set_vertical_scale(ch, smaller, channel_info.probe)
                }
            };
            let try_zoom_in_vertical = || {
                if let (_, _, _, Some(larger)) = selected_voltage(channel_info) {
                    cmd.set_vertical_scale(ch, larger, channel_info.probe)
                }
            };

            if input.key_pressed(ZOOM_IN) || input.raw_scroll_delta.y > 0.0 {
                if input.modifiers.command {
                    try_zoom_out_vertical();
                } else if input.modifiers.alt {
                    cmd.set_trigger_level(calc_new_trigger_level(head, input.modifiers, true))
                } else if let (_, _, Some(smaller), _) = selected_time_base(head) {
                    cmd.set_time_scale(smaller);
                }
            }

            if input.zoom_delta() > 1.0 {
                try_zoom_out_vertical();
            }

            if input.key_pressed(ZOOM_OUT) || input.raw_scroll_delta.y < 0.0 {
                if input.modifiers.command {
                    try_zoom_in_vertical();
                } else if input.modifiers.alt {
                    cmd.set_trigger_level(calc_new_trigger_level(head, input.modifiers, false))
                } else if let (_, _, _, Some(larger)) = selected_time_base(head) {
                    cmd.set_time_scale(larger);
                }
            }

            if input.zoom_delta() < 1.0 {
                try_zoom_in_vertical();
            }

            if input.key_pressed(HORIZONTAL_OFFSET_LEFT) {
                cmd.set_horizontal_offset(calc_new_horizontal_offset(head, input.modifiers, true))
            }
            if input.key_pressed(HORIZONTAL_OFFSET_RIGHT) {
                cmd.set_horizontal_offset(calc_new_horizontal_offset(head, input.modifiers, false))
            }

            if input.key_pressed(VERTICAL_OFFSET_UP) {
                cmd.set_vertical_offset(
                    ch,
                    calc_new_vertical_offset(head.channel(ch), input.modifiers, true),
                )
            }
            if input.key_pressed(VERTICAL_OFFSET_DOWN) {
                cmd.set_vertical_offset(
                    ch,
                    calc_new_vertical_offset(head.channel(ch), input.modifiers, false),
                )
            }
        });
    }

    fn device_list_or_fail_ui(
        &mut self,
        ctx: &egui::Context,
    ) -> Option<Arc<RwLock<HashMap<String, DeviceInformation>>>> {
        let device_list = if let Some(s) = self.device_selector.as_ref() {
            s.list().clone()
        } else {
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.heading("Error");
                ui.label("Could not initialize device selector");

                if ui.button("Quit").clicked() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            });
            return None;
        };
        Some(device_list)
    }

    fn try_select_device(&mut self, device_id: impl Into<HSTRING>, ctx: &Context) {
        match Device::blocking_from_device_id(device_id).map(|d| {
            DeviceRun::new(
                d,
                ctx,
                InitialDeviceRunConfig {
                    measurements_enabled: self.persistent_state.measurements_enabled,
                },
            )
        }) {
            Ok(run) => {
                self.last_device_error = None;
                self.device_run = DeviceRunState::Running(run)
            }
            Err(e) => {
                self.last_device_error = Some(snafu::Report::from_error(e).to_string());
                self.device_run = DeviceRunState::Stopped;
            }
        }
    }

    fn update_device_run(&mut self) {
        self.device_run.update();

        match &mut self.device_run {
            DeviceRunState::Stopped => {}
            DeviceRunState::Error(e) => {
                self.last_device_error = Some(snafu::Report::from_error(&*e).to_string());
                self.device_run = DeviceRunState::Stopped;
            }
            DeviceRunState::Running(run) => {
                let channel = run.message_channel();

                match channel.try_recv() {
                    Ok(OscilloscopeMessage::Data(data)) => {
                        let state = &mut self.osc_ui_state;
                        update_osc_ui_state(state, data);
                    }
                    Ok(OscilloscopeMessage::Awg(awg_config)) => {
                        update_awg_state(&mut self.awg_state, awg_config)
                    }
                    Err(_) => {}
                }
            }
        }
    }

    fn top_panel_ui(&mut self, ui: &mut Ui) {
        if !self.device_run.is_running() {
            return;
        }

        let head = &self.osc_ui_state.head;

        ui.columns(3, |columns| {
            columns[0].with_layout(Layout::left_to_right(Align::Center), |ui| {
                if ui.button("disconnect").clicked() {
                    self.device_run.stop();
                }
            });

            columns[1].with_layout(Layout::centered_and_justified(Direction::TopDown), |ui| {
                ui.label({
                    let rt = RichText::new(head.run_status.to_string());
                    match head.run_status {
                        RunStatus::Stopped => rt.color(Color32::RED),
                        RunStatus::Triggering => rt.color(Color32::GREEN),
                        _ => rt,
                    }
                })
            });

            columns[2].columns(2, |columns| {
                columns[0].with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.label(self.osc_ui_state.head.sample.sampling_rate.to_string());
                });
                columns[1].with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.label(format!(
                        "Acq: {}ms",
                        self.osc_ui_state.acquisition_duration.as_millis()
                    ));
                });
            });
        });
    }
}

fn bottom_panel_ui(ui: &mut Ui, head: &DataHeader, measurements: &[Measurements; 2]) {
    if head.channel_enabled(Channel::Ch1) {
        ui.columns(Measurements::MEASUREMENT_COUNT, |cols| {
            for (index, measurement) in measurements[0].for_display().into_iter().enumerate() {
                cols[index].add(Label::new(measurement).wrap(false));
            }
        });
    }
    if head.channel_enabled(Channel::Ch2) {
        ui.columns(Measurements::MEASUREMENT_COUNT, |cols| {
            for (index, measurement) in measurements[1].for_display().into_iter().enumerate() {
                cols[index].add(Label::new(measurement).wrap(false));
            }
        });
    }
}

fn update_osc_ui_state(state: &mut OscilloscopeUiState, data: owowon::OscilloscopeData) {
    state.head = data.signal_data.header;
    state.ch1_data.clear();
    if let Some(ch) = data.signal_data.ch0_data {
        state.ch1_data.extend(ch);
    }
    state.ch2_data.clear();
    if let Some(ch) = data.signal_data.ch1_data {
        state.ch2_data.extend(ch);
    }
    state.measurements = data.measurements;
    state.acquisition_duration = data.acquisition_duration
}

fn update_awg_state(state: &mut AwgState, config: AwgConfig) {
    state.config = config;
    state.frequency.clear();
    let _ = write!(&mut state.frequency, "{}", config.frequency);
    state.amplitude.clear();
    let _ = write!(&mut state.amplitude, "{}", config.amplitude);
    state.offset.clear();
    let _ = write!(&mut state.offset, "{}", config.offset);

    state.mode_index = AWG_MODES
        .into_iter()
        .position(|m| m == config.mode)
        .unwrap();
}
