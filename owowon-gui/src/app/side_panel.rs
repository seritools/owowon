use super::{
    utils::{
        calc_new_trigger_level, calc_new_vertical_offset, selected_time_base, selected_voltage,
    },
    AwgState, OwowonApp,
};
use crate::{app::utils::calc_new_horizontal_offset, optional_sender::OptionalSender};
use egui::{Button, CollapsingHeader, ComboBox, Modifiers, TextEdit, TextStyle, Ui};
use owowon::{
    consts::{PROBE_ATTENUATIONS, TIME_BASES, VERTICAL_SCALES},
    data::{
        awg::AWG_MODES,
        head::{
            Channel, ChannelCoupling, ChannelInfo, DataHeader, MemoryDepth, SampleType,
            TriggerCoupling, TriggerEdge, TriggerSweep,
        },
        units::{Frequency, Time, Voltage},
    },
    scaled_number::ScaledNumber,
    OscilloscopeCommand, OscilloscopeRunCommand,
};
use std::fmt::Write;

const ARROW_DOWN: &str = "⬇";
const ARROW_UP: &str = "⬆";
const ARROW_LEFT: &str = " < ";
const ARROW_RIGHT: &str = " > ";

pub(crate) fn ui(
    app: &mut OwowonApp,
    ui: &mut Ui,
    command_tx: &OptionalSender<OscilloscopeRunCommand>,
) {
    let head = &app.osc_ui_state.head;

    ui.add_space(ui.style().spacing.item_spacing.y);
    ui.group(|ui| {
        ui.heading("General");

        if ui.button("Auto").clicked() {
            command_tx.run_auto();
        }

        ui.label("Display");

        let ch1_disp = head.channels[0].display.into();
        let ch2_disp = head.channels[1].display.into();
        let both_disp = ch1_disp && ch2_disp;

        ui.columns(3, |cols| {
            if cols[0]
                .selectable_label(ch1_disp && !ch2_disp, "CH1")
                .clicked()
            {
                if !ch1_disp {
                    command_tx.set_channel_display(Channel::Ch1, true);
                }
                if ch2_disp {
                    command_tx.set_channel_display(Channel::Ch2, false);
                }
            }
            if cols[1]
                .selectable_label(!ch1_disp && ch2_disp, "CH2")
                .clicked()
            {
                if ch1_disp {
                    command_tx.set_channel_display(Channel::Ch1, false);
                }
                if !ch2_disp {
                    command_tx.set_channel_display(Channel::Ch2, true);
                }
            }
            if cols[2].selectable_label(both_disp, "Both").clicked() {
                if !ch1_disp {
                    command_tx.set_channel_display(Channel::Ch1, true);
                }
                if !ch2_disp {
                    command_tx.set_channel_display(Channel::Ch2, true);
                }
            }
        });

        time_base_ui(ui, head, &mut app.horizontal_offset_string, command_tx);

        ui.collapsing("Acquisition", |ui| {
            ui.label("Mode");
            ui.columns(2, |cols| {
                if cols[0]
                    .selectable_label(head.sample.sample_type == SampleType::Sample, "Sample")
                    .clicked()
                {
                    command_tx.set_acquisition_mode(SampleType::Sample);
                }

                if cols[1]
                    .selectable_label(head.sample.sample_type == SampleType::Peak, "Peak detect")
                    .clicked()
                {
                    command_tx.set_acquisition_mode(SampleType::Peak);
                }
            });

            ui.label("Depth");
            ui.columns(2, |cols| {
                if cols[0]
                    .selectable_label(head.sample.depmem == MemoryDepth::FourK, "4K")
                    .clicked()
                {
                    command_tx.set_acquisition_depth(MemoryDepth::FourK);
                }

                if cols[1]
                    .selectable_label(head.sample.depmem == MemoryDepth::EightK, "8K")
                    .clicked()
                {
                    command_tx.set_acquisition_depth(MemoryDepth::EightK);
                }
            });
        })
    });

    ui.group(|ui| {
        let ch1 = head.channel(Channel::Ch1);
        channel_ui("Channel 1", ui, ch1, &mut app.ch1_offset_string, command_tx);
    });
    ui.group(|ui| {
        let ch2 = head.channel(Channel::Ch2);
        channel_ui("Channel 2", ui, ch2, &mut app.ch2_offset_string, command_tx);
    });

    ui.group(|ui| {
        trigger_ui(ui, head, command_tx, &mut app.trigger_level_string);
    });
    ui.group(|ui| {
        ui.with_layout(egui::Layout::top_down_justified(egui::Align::LEFT), |ui| {
            let mut measurements_enabled = app.osc_ui_state.measurements.is_some();
            if ui
                .checkbox(&mut measurements_enabled, "Enable measurements")
                .changed()
            {
                command_tx.set_measurements_enabled(measurements_enabled);
            }
        });
    });

    ui.group(|ui| {
        ui.collapsing("Waveform generator", |ui| {
            awg(ui, &mut app.awg_state, command_tx)
        })
    });
}

fn time_base_ui(
    ui: &mut Ui,
    head: &DataHeader,
    horizontal_offset_string: &mut String,
    command_tx: &OptionalSender<OscilloscopeRunCommand>,
) {
    ui.label("Time base (per div)");
    let (mut selected, selected_timebase, smaller, bigger) = selected_time_base(head);

    ui.horizontal_top(|ui| {
        if ui
            .add_enabled(bigger.is_some(), Button::new("out"))
            .clicked()
        {
            command_tx.set_time_scale(bigger.unwrap());
        }
        if ui
            .add_enabled(smaller.is_some(), Button::new("in"))
            .clicked()
        {
            command_tx.set_time_scale(smaller.unwrap());
        }
        if ComboBox::from_id_source("time_base")
            .width(150.0)
            .show_index(ui, &mut selected, TIME_BASES.len(), |i| {
                TIME_BASES[i].to_string()
            })
            .changed()
        {
            command_tx.set_time_scale(TIME_BASES[selected]);
        }
    });

    ui.label(format!(
        "Horizontal offset ({:+.2} divs)",
        head.time_base.h_offset_grid_divs()
    ));

    value_changer_box(
        ui,
        horizontal_offset_string,
        |mods| command_tx.set_horizontal_offset(calc_new_horizontal_offset(head, mods, true)),
        |mods| command_tx.set_horizontal_offset(calc_new_horizontal_offset(head, mods, false)),
        |horizontal_offset_string| {
            let trimmed = horizontal_offset_string.trim();
            if let Some(o) = trimmed
                .parse::<Time>()
                .map(|time| time.0 / selected_timebase.0)
                .ok()
                .or_else(|| trimmed.parse::<f64>().ok())
            {
                command_tx.set_horizontal_offset(o);
            }
        },
        |horizontal_offset_string| {
            horizontal_offset_string.clear();
            let _ = write!(
                horizontal_offset_string,
                "{}s",
                ScaledNumber(head.time_base.h_offset_grid_divs() * selected_timebase.0)
            );
        },
        ARROW_LEFT,
        ARROW_RIGHT,
    );
}

fn channel_ui(
    name: &'static str,
    ui: &mut Ui,
    ch: &ChannelInfo,
    offset_string: &mut String,
    command_tx: &OptionalSender<OscilloscopeRunCommand>,
) {
    ui.heading(name);

    channel_voffset(ui, ch, offset_string, command_tx);
    channel_vscale(ui, ch, command_tx);
    CollapsingHeader::new("Configuration")
        .id_source(format!("collapsing_{}_config", ch.channel))
        .show(ui, |ui| {
            ui.label("Probe attenuation");
            let mut current_pos = PROBE_ATTENUATIONS
                .iter()
                .position(|&v| v == ch.probe)
                .unwrap_or(0);

            if ComboBox::from_id_source(format!("combobox_{}_attenuation", ch.channel))
                .width(150.0)
                .show_index(ui, &mut current_pos, PROBE_ATTENUATIONS.len(), |i| {
                    format!("{}", PROBE_ATTENUATIONS[i])
                })
                .changed()
            {
                let _ = command_tx.blocking_send(OscilloscopeCommand::SetChannelAttenuation(
                    ch.channel,
                    PROBE_ATTENUATIONS[current_pos],
                ));
            }

            ui.label("Coupling");
            ui.columns(3, |cols| {
                let mut new_coupling = None;
                if cols[0]
                    .selectable_label(ch.coupling == ChannelCoupling::Dc, "DC")
                    .clicked()
                {
                    new_coupling = Some(ChannelCoupling::Dc);
                }
                if cols[1]
                    .selectable_label(ch.coupling == ChannelCoupling::Ac, "AC")
                    .clicked()
                {
                    new_coupling = Some(ChannelCoupling::Ac);
                }
                if cols[2]
                    .selectable_label(ch.coupling == ChannelCoupling::Gnd, "GND")
                    .clicked()
                {
                    new_coupling = Some(ChannelCoupling::Gnd);
                }

                if let Some(coupling) = new_coupling {
                    command_tx.set_channel_coupling(ch.channel, coupling);
                }
            });
        });
}

fn channel_vscale(
    ui: &mut Ui,
    channel_info: &ChannelInfo,
    command_tx: &OptionalSender<OscilloscopeRunCommand>,
) {
    let channel = channel_info.channel;
    ui.label("Scale per div");
    ui.horizontal_top(|ui| {
        let attenuation = channel_info.probe;

        let (mut selected_index, _, smaller, bigger) = selected_voltage(channel_info);
        if ui
            .add_enabled(bigger.is_some(), Button::new("out"))
            .clicked()
        {
            command_tx.set_vertical_scale(channel, bigger.unwrap(), attenuation);
        }
        if ui
            .add_enabled(smaller.is_some(), Button::new("in"))
            .clicked()
        {
            command_tx.set_vertical_scale(channel, smaller.unwrap(), attenuation);
        }

        if ComboBox::from_id_source(format!("combobox_{channel}_vscale"))
            .width(150.0)
            .show_index(ui, &mut selected_index, VERTICAL_SCALES.len(), |i| {
                format!(
                    "{:.2}",
                    Voltage(VERTICAL_SCALES[i].0 * (attenuation.0 as f64))
                )
            })
            .changed()
        {
            command_tx.set_vertical_scale(channel, VERTICAL_SCALES[selected_index], attenuation);
        }
    });
}

fn channel_voffset(
    ui: &mut Ui,
    channel_info: &ChannelInfo,
    channel_string: &mut String,
    command_tx: &OptionalSender<OscilloscopeRunCommand>,
) {
    let channel = channel_info.channel;

    ui.label(format!(
        "Offset ({:+.2} divs)",
        channel_info.offset_grid_divs()
    ));
    value_changer_box(
        ui,
        channel_string,
        |mods| {
            let new_offset = calc_new_vertical_offset(channel_info, mods, false);
            command_tx.set_vertical_offset(channel, new_offset);
        },
        |mods| {
            let new_offset = calc_new_vertical_offset(channel_info, mods, true);
            command_tx.set_vertical_offset(channel, new_offset);
        },
        |channel_string| {
            let trimmed = channel_string.trim();
            if let Some(vertical_offset) = trimmed
                .parse::<Voltage>()
                .map(|voltage| voltage.0 / channel_info.scale_attenuated().0)
                .ok()
                .or_else(|| trimmed.parse::<f64>().ok())
            {
                command_tx.set_vertical_offset(channel, vertical_offset);
            }
        },
        |channel_string| {
            channel_string.clear();
            let _ = write!(
                channel_string,
                "{}",
                Voltage(channel_info.offset_grid_divs() * channel_info.scale_attenuated().0)
            );
        },
        ARROW_DOWN,
        ARROW_UP,
    );
}

fn trigger_ui(
    ui: &mut Ui,
    head: &DataHeader,
    command_tx: &OptionalSender<OscilloscopeRunCommand>,
    trigger_level_string: &mut String,
) {
    ui.heading("Trigger");
    ui.label("Source");
    ui.columns(2, |cols| {
        if cols[0]
            .selectable_label(head.trigger.items.channel == Channel::Ch1, "CH1")
            .clicked()
        {
            command_tx.set_trigger_source(Channel::Ch1)
        }
        if cols[1]
            .selectable_label(head.trigger.items.channel == Channel::Ch2, "CH2")
            .clicked()
        {
            command_tx.set_trigger_source(Channel::Ch2)
        }
    });

    trigger_level(ui, head, trigger_level_string, command_tx);

    let configuration = |ui: &mut Ui| {
        ui.label("Mode/Sweep (see tooltip)").on_hover_text(
            "\
'Normal' mode is broken with USB transfer.

There is no way to restart the acquisition in 'Single' trigger mode.
You'll have to press the play/pause button on the device itself.",
        );
        ui.columns(2, |cols| {
            if cols[0]
                .selectable_label(head.trigger.items.sweep == TriggerSweep::Auto, "Auto")
                .clicked()
            {
                command_tx.set_trigger_sweep(TriggerSweep::Auto);
            }

            if cols[1]
                .selectable_label(head.trigger.items.sweep == TriggerSweep::Single, "Single")
                .clicked()
            {
                command_tx.set_trigger_sweep(TriggerSweep::Single);
            }
        });

        ui.label("Edge");
        ui.columns(2, |cols| {
            if cols[0]
                .selectable_label(head.trigger.items.edge == TriggerEdge::Rising, "Rising")
                .clicked()
            {
                command_tx.set_trigger_edge(TriggerEdge::Rising);
            }
            if cols[1]
                .selectable_label(head.trigger.items.edge == TriggerEdge::Falling, "Falling")
                .clicked()
            {
                command_tx.set_trigger_edge(TriggerEdge::Falling);
            }
        });

        ui.label("Coupling");
        ui.columns(2, |cols| {
            if cols[0]
                .selectable_label(head.trigger.items.coupling == TriggerCoupling::Dc, "DC")
                .clicked()
            {
                command_tx.set_trigger_coupling(TriggerCoupling::Dc);
            }
            if cols[1]
                .selectable_label(head.trigger.items.coupling == TriggerCoupling::Ac, "AC")
                .clicked()
            {
                command_tx.set_trigger_coupling(TriggerCoupling::Ac);
            }
        });
    };

    CollapsingHeader::new("Configuration")
        .id_source("collapsing_trigger_config")
        .show(ui, configuration);
}

fn trigger_level(
    ui: &mut Ui,
    head: &DataHeader,
    trigger_level_string: &mut String,
    command_tx: &OptionalSender<OscilloscopeRunCommand>,
) {
    ui.label("Level");
    value_changer_box(
        ui,
        trigger_level_string,
        |mods| command_tx.set_trigger_level(calc_new_trigger_level(head, mods, false)),
        |mods| command_tx.set_trigger_level(calc_new_trigger_level(head, mods, true)),
        |trigger_level_string| {
            let s = trigger_level_string.trim();
            if let Some(voltage) = s
                .parse::<Voltage>()
                .ok()
                .or_else(|| s.parse::<ScaledNumber>().map(|f| Voltage(f.0)).ok())
                .or_else(|| s.parse::<f64>().map(Voltage).ok())
            {
                command_tx.set_trigger_level(voltage);
            }
        },
        |trigger_level_string| {
            trigger_level_string.clear();
            let _ = write!(trigger_level_string, "{}", head.trigger.items.level);
        },
        ARROW_DOWN,
        ARROW_UP,
    )
}

#[allow(clippy::too_many_arguments)]
fn value_changer_box(
    ui: &mut Ui,
    string: &mut String,
    on_left: impl FnOnce(Modifiers),
    on_right: impl FnOnce(Modifiers),
    on_lost_focus: impl FnOnce(&mut String),
    update_without_focus: impl FnOnce(&mut String),
    left_str: &str,
    right_str: &str,
) {
    ui.horizontal_top(|ui| {
        if ui.button(left_str).clicked() {
            let modifiers = ui.input(|input| input.modifiers);
            on_left(modifiers);
        }
        if ui.button(right_str).clicked() {
            let modifiers = ui.input(|input| input.modifiers);
            on_right(modifiers);
        }

        let ch_textbox = ui.add(
            TextEdit::singleline(string)
                .font(TextStyle::Button)
                .desired_width(f32::INFINITY),
        );
        if ch_textbox.lost_focus() {
            on_lost_focus(string);
        } else if !ch_textbox.has_focus() {
            update_without_focus(string);
        }
    });
}

fn awg(ui: &mut Ui, awg_state: &mut AwgState, command_tx: &OptionalSender<OscilloscopeRunCommand>) {
    ui.horizontal(|ui| {
        if ui.button("Read config").clicked() {
            command_tx.read_awg_config();
        }
        if ui.button("Set config").clicked() {
            // parse here to work around egui not having an easy way for regular focus loss update
            // (only works on enter/tab with textboxes for some reason)
            parse_awg_freq(awg_state);
            parse_awg_voltage(
                &mut awg_state.config.amplitude,
                &mut awg_state.amplitude,
                Voltage(1.0),
            );
            parse_awg_voltage(
                &mut awg_state.config.offset,
                &mut awg_state.offset,
                Voltage(0.0),
            );
            command_tx.set_awg_config(awg_state.config);
        }
    });

    ui.checkbox(&mut awg_state.config.enabled, "Enabled");

    ui.label("Function");
    if ComboBox::from_id_source("awg_mode")
        .width(200.0)
        .show_index(ui, &mut awg_state.mode_index, AWG_MODES.len(), |i| {
            AWG_MODES[i].to_string()
        })
        .changed()
    {
        awg_state.config.mode = AWG_MODES[awg_state.mode_index];
    }

    ui.label("Frequency");
    if TextEdit::singleline(&mut awg_state.frequency)
        .font(TextStyle::Button)
        .desired_width(f32::INFINITY)
        .show(ui)
        .response
        .lost_focus()
    {
        parse_awg_freq(awg_state);
    };

    ui.label("Amplitude");
    if TextEdit::singleline(&mut awg_state.amplitude)
        .font(TextStyle::Button)
        .desired_width(f32::INFINITY)
        .show(ui)
        .response
        .lost_focus()
    {
        parse_awg_voltage(
            &mut awg_state.config.amplitude,
            &mut awg_state.amplitude,
            Voltage(1.0),
        );
    };

    ui.label("Offset");
    if TextEdit::singleline(&mut awg_state.offset)
        .font(TextStyle::Button)
        .desired_width(f32::INFINITY)
        .show(ui)
        .response
        .lost_focus()
    {
        parse_awg_voltage(
            &mut awg_state.config.offset,
            &mut awg_state.offset,
            Voltage(0.0),
        );
    };
}

fn parse_awg_freq(awg_state: &mut AwgState) {
    let s = awg_state.frequency.trim();
    let freq = if let Some(freq) = s
        .parse::<Frequency>()
        .ok()
        .or_else(|| s.parse::<ScaledNumber>().map(|f| Frequency(f.0)).ok())
        .or_else(|| s.parse::<f64>().map(Frequency).ok())
    {
        awg_state.config.frequency = freq;
        freq
    } else {
        Frequency(1_000_000.0)
    };
    awg_state.frequency.clear();
    let _ = write!(&mut awg_state.frequency, "{freq}");
}

fn parse_awg_voltage(config: &mut Voltage, string: &mut String, default: Voltage) {
    let s = string.trim();
    let val = if let Some(voltage) = s
        .parse::<Voltage>()
        .ok()
        .or_else(|| s.parse::<ScaledNumber>().map(|f| Voltage(f.0)).ok())
        .or_else(|| s.parse::<f64>().map(Voltage).ok())
    {
        *config = voltage;
        voltage
    } else {
        *config = default;
        default
    };
    string.clear();
    let _ = write!(string, "{val}");
}
