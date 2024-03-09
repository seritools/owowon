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

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_min_inner_size(egui::vec2(800.0, 400.0))
            .with_maximized(true),
        // persist_window: true, // wait for egui to fix maximization not being persisted
        ..Default::default()
    };

    eframe::run_native(
        "owowon",
        native_options,
        Box::new(|cc| Box::new(app::OwowonApp::new(cc))),
    )?;

    Ok(())
}
