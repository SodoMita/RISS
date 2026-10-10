// Desktop binary entry point
// On Android, the library is loaded as cdylib and android_main is called
#![allow(dead_code)]

#[cfg(target_os = "android")]
mod android_app_entry;
#[cfg(not(target_os = "android"))]
mod app_entry;

mod history;
mod providers;
mod search;
mod settings;
mod storage;
mod ui;

#[cfg(not(target_os = "android"))]
fn main() -> eframe::Result {
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([400.0, 600.0])
            .with_min_inner_size([300.0, 400.0]),
        ..Default::default()
    };

    eframe::run_native(
        "RISS Launcher",
        native_options,
        Box::new(|cc| {
            ui::setup_fonts(&cc.egui_ctx);
            Ok(Box::new(ui::RissApp::new(cc)))
        }),
    )
}

#[cfg(target_os = "android")]
fn main() {
    // On Android, this binary is not used
    // The library's android_main is called instead
    println!("Use the library on Android, not this binary");
}
