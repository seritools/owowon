use super::{OscilloscopeUiState, OwowonApp};
use egui::{Color32, Ui, Vec2};
use egui_plot::{GridInput, GridMark, HLine, Line, LineStyle, Plot, PlotPoints, VLine};
use owowon::{
    consts::{GRID_DIV_COUNT_HORIZONTAL, GRID_DIV_SIZE, SAMPLES},
    data::{head::Channel, units::Voltage},
};
use std::ops::Deref;

pub(crate) fn ui(app: &OwowonApp, ui: &mut Ui) {
    let OscilloscopeUiState {
        head,
        ch1_data,
        ch2_data,
        ..
    } = &app.osc_ui_state;

    let ch1_data = (!ch1_data.is_empty()).then_some(ch1_data.deref());
    let ch2_data = (!ch2_data.is_empty()).then_some(ch2_data.deref());

    let (line1, line2) = {
        let line1 = ch1_data.map(prep_channel_data);
        let line2 = ch2_data.map(prep_channel_data);
        (line1, line2)
    };

    let head = *head;
    Plot::new("osc")
        .include_y(-128.25)
        .include_y(127.25)
        .include_x(-150.0f32)
        .include_x(150.0f32)
        .set_margin_fraction(Vec2::ZERO)
        .x_grid_spacer(const_grid_lines)
        .y_grid_spacer(const_grid_lines)
        .allow_boxed_zoom(false)
        .allow_drag(false)
        .allow_scroll(false)
        .allow_zoom(false)
        .allow_double_click_reset(false)
        .show_axes(false)
        .show_x(true)
        .show_y(true)
        .label_formatter(move |name, point| {
            let channel = match name {
                "CH1" => Channel::Ch1,
                "CH2" => Channel::Ch2,
                _ => return String::new(),
            };

            let channel = head.channel(channel);
            let vscale_per_unit = channel.scale_per_unit();
            let offset_scaled = channel.offset as f64 * vscale_per_unit;
            format!(
                "{}: {}",
                channel.channel,
                Voltage(point.y * vscale_per_unit - offset_scaled)
            )
        })
        .show(ui, |plot_ui| {
            if let Some(line) = line1 {
                plot_ui.line(Line::new(line).name("CH1").color(Color32::YELLOW));
            }
            if let Some(line) = line2 {
                plot_ui.line(Line::new(line).name("CH2").color(Color32::LIGHT_BLUE));
            }

            let trigger_channel = head.channel(head.trigger.items.channel);
            let voltage_per_unit = trigger_channel.scale_per_unit();
            let offset = trigger_channel.offset;

            let trigger_level = head.trigger.items.level.0 / voltage_per_unit + offset as f64;

            plot_ui.hline(
                HLine::new(trigger_level)
                    .width(2.0)
                    .color(Color32::from_rgb(160, 80, 80))
                    .style(LineStyle::Dotted { spacing: 20.0 }),
            );

            let hoffset = -head.time_base.h_offset as f64;
            plot_ui.vline(
                VLine::new(hoffset)
                    .color(Color32::from_rgb(160, 80, 80))
                    .style(LineStyle::Solid),
            );
        });
}

fn prep_channel_data(data: &[u8]) -> PlotPoints {
    const WEIRD_OFFSET: f64 = GRID_DIV_SIZE * GRID_DIV_COUNT_HORIZONTAL / 2.0 - 1.0;

    fn two((i, [val1, val2]): (usize, [u8; 2])) -> [f64; 2] {
        [
            i as f64 - WEIRD_OFFSET,
            ((val1 as i8 as f64) + (val2 as i8 as f64)) / 2.0,
        ]
    }

    fn one((i, val): (usize, u8)) -> [f64; 2] {
        [i as f64 - WEIRD_OFFSET, val as i8 as f64]
    }

    let vec = if data.len() != SAMPLES {
        data.array_chunks::<2>()
            .copied()
            .enumerate()
            .map(two)
            .map(Into::into)
            .collect()
    } else {
        data.iter()
            .copied()
            .enumerate()
            .map(one)
            .map(Into::into)
            .collect()
    };

    PlotPoints::Owned(vec)
}

fn const_grid_lines(grid_input: GridInput) -> Vec<GridMark> {
    const GRID_LINES: [f64; 11] = [
        GRID_DIV_SIZE * -5.0,
        GRID_DIV_SIZE * -4.0,
        GRID_DIV_SIZE * -3.0,
        GRID_DIV_SIZE * -2.0,
        GRID_DIV_SIZE * -1.0,
        GRID_DIV_SIZE * 0.0,
        GRID_DIV_SIZE * 1.0,
        GRID_DIV_SIZE * 2.0,
        GRID_DIV_SIZE * 3.0,
        GRID_DIV_SIZE * 4.0,
        GRID_DIV_SIZE * 5.0,
    ];

    let step_size = grid_input.base_step_size * GRID_DIV_SIZE;

    GRID_LINES
        .into_iter()
        .filter(|&p| grid_input.bounds.0 <= p && p <= grid_input.bounds.1)
        .map(|p| GridMark {
            value: p,
            step_size,
        })
        .collect()
}
