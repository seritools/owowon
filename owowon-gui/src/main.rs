#![feature(array_chunks)]
#![warn(clippy::all, rust_2018_idioms)]
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

mod app;
mod device_run;
mod device_select;
mod optional_sender;
mod selectable_label_full_width;

#[global_allocator]
static ALLOC: snmalloc_rs::SnMalloc = snmalloc_rs::SnMalloc;

fn main() {
    let native_options = eframe::NativeOptions {
        maximized: true,
        min_window_size: Some(egui::vec2(800.0, 400.0)),
        ..Default::default()
    };

    eframe::run_native(
        "owowon-gui",
        native_options,
        Box::new(|cc| Box::new(app::OwowonApp::new(cc))),
    );
}
