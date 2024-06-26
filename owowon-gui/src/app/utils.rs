use egui::Modifiers;
use float_cmp::ApproxEqUlps;
use owowon::{
    consts::{GRID_DIV_SIZE, GRID_DIV_SIZE_INT, TIME_BASES, VERTICAL_SCALES},
    data::{
        head::{ChannelInfo, DataHeader},
        units::{Time, Voltage},
    },
};

pub fn selected_time_base(head: &DataHeader) -> (usize, Time, Option<Time>, Option<Time>) {
    let selected_index = TIME_BASES
        .iter()
        .position(|&t| t.0.approx_eq_ulps(&head.time_base.scale.0, 2))
        .unwrap_or(0);
    let selected_timebase = TIME_BASES[selected_index];

    (
        selected_index,
        selected_timebase,
        selected_index.checked_sub(1).map(|i| TIME_BASES[i]),
        TIME_BASES.get(selected_index + 1).copied(),
    )
}

pub fn selected_voltage(
    channel_info: &ChannelInfo,
) -> (usize, Voltage, Option<Voltage>, Option<Voltage>) {
    let selected_index = VERTICAL_SCALES
        .iter()
        .position(|&v| v.0.approx_eq_ulps(&channel_info.scale.0, 2))
        .unwrap_or(0);
    let selected_voltage = VERTICAL_SCALES[selected_index];

    (
        selected_index,
        selected_voltage,
        selected_index.checked_sub(1).map(|i| VERTICAL_SCALES[i]),
        VERTICAL_SCALES.get(selected_index + 1).copied(),
    )
}

pub fn calc_new_vertical_offset(
    channel_info: &ChannelInfo,
    mods: Modifiers,
    positive: bool,
) -> f64 {
    (channel_info.offset + grid_offset_change(mods, positive)) as f64 / GRID_DIV_SIZE
}

pub fn calc_new_horizontal_offset(head: &DataHeader, mods: Modifiers, positive: bool) -> f64 {
    (head.time_base.h_offset + grid_offset_change(mods, positive)) as f64 / GRID_DIV_SIZE
}

fn grid_offset_change(mods: Modifiers, positive: bool) -> i64 {
    let magnitude = if mods.command {
        1
    } else if mods.shift {
        GRID_DIV_SIZE_INT
    } else {
        GRID_DIV_SIZE_INT / 5
    };

    if positive {
        magnitude
    } else {
        -magnitude
    }
}

pub fn calc_new_trigger_level(head: &DataHeader, mods: Modifiers, positive: bool) -> Voltage {
    let scale_per_unit = head.channel(head.trigger.items.channel).scale_per_unit();

    Voltage(head.trigger.items.level.0 + trigger_level_change(mods, positive, scale_per_unit))
}

fn trigger_level_change(mods: Modifiers, positive: bool, scale_per_unit: f64) -> f64 {
    let magnitude = if mods.shift {
        scale_per_unit * 5.0
    } else {
        scale_per_unit
    };

    if positive {
        magnitude
    } else {
        -magnitude
    }
}
