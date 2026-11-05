// Hide the console window on Windows in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod cheatsheet;
mod editor;
mod fileformat;
mod theme;
mod widgets;

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("Varan ARM Emulator")
            .with_inner_size([1280.0, 820.0])
            .with_min_inner_size([800.0, 500.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Varan ARM Emulator",
        options,
        Box::new(|cc| Ok(Box::new(app::VaranApp::new(cc)))),
    )
}
