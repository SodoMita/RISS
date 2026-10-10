// Desktop binary entry point
// On Android, the library is loaded as cdylib and android_main is called
#![allow(dead_code)]

#[cfg(target_os = "android")]
mod android_app_entry;
#[cfg(not(target_os = "android"))]
mod app_entry;

mod history;
mod search;
mod settings;
mod storage;
mod ui;

#[cfg(not(target_os = "android"))]
fn main() -> eframe::Result {
    // Settings are needed before the window exists to open it in the right
    // mode (`--fullscreen` / `--maximized` / `--windowed` override them).
    let (settings, notes) = settings::SettingsData::load();
    let viewport = egui::ViewportBuilder::default()
        .with_inner_size([400.0, 600.0])
        .with_min_inner_size([300.0, 400.0]);
    let native_options = eframe::NativeOptions {
        viewport: ui::window_mode::WindowMode::at_startup(&settings).apply_to_builder(viewport),
        ..Default::default()
    };

    eframe::run_native(
        "RISS Launcher",
        native_options,
        Box::new(move |cc| {
            ui::setup_fonts(&cc.egui_ctx);
            Ok(Box::new(ui::RissApp::with_settings(cc, settings, notes)))
        }),
    )
}

#[cfg(target_os = "android")]
fn main() {
    // On Android, this binary is not used
    // The library's android_main is called instead
    println!("Use the library on Android, not this binary");
}
