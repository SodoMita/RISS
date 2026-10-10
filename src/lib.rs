// RISS Launcher - Android entry point
// Uses eframe 0.32+ official Android support
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

#[cfg(target_os = "android")]
use winit::platform::android::activity::AndroidApp;

#[cfg(target_os = "android")]
#[no_mangle]
fn android_main(app: AndroidApp) {
    use log::LevelFilter;
    use std::sync::OnceLock;

    // Initialize logger FIRST so we can see crashes
    android_logger::init_once(
        android_logger::Config::default()
            .with_max_level(LevelFilter::Debug)
            .with_tag("RissLauncher"),
    );

    log::info!("RISS Launcher: android_main called");

    // Set panic hook to log panics
    std::panic::set_hook(Box::new(|info| {
        log::error!("PANIC: {}", info);
    }));

    // Store AndroidApp for JNI calls (use OnceLock to avoid re-initialization)
    static ANDROID_APP_INITIALIZED: OnceLock<()> = OnceLock::new();
    ANDROID_APP_INITIALIZED.get_or_init(|| {
        log::info!("Storing AndroidApp for JNI calls");
        android_app_entry::android_jni::set_android_app(app.clone());
    });

    // NativeActivity gives the window an RGB_565 format, which has no alpha
    // bits, so the home-screen wallpaper can never be seen behind the launcher
    // (issue #32). Ask for a format with alpha before the surface is created.
    android_app_entry::android_jni::make_window_translucent_jni();

    log::info!("Starting eframe with Android support");

    // Configure eframe with Android app handle. The viewport asks for a
    // transparency capable framebuffer so the window's alpha is not thrown
    // away; whether anything is actually see-through is decided per frame by
    // `RissApp::clear_color` and the palette.
    let options = eframe::NativeOptions {
        android_app: Some(app),
        viewport: egui::ViewportBuilder::default().with_transparent(true),
        ..Default::default()
    };

    // Run the app
    if let Err(e) = eframe::run_native(
        "RISS Launcher",
        options,
        Box::new(|cc| {
            log::info!("eframe creation callback called");
            ui::setup_fonts(&cc.egui_ctx);
            Ok(Box::new(ui::RissApp::new(cc)))
        }),
    ) {
        log::error!("eframe failed: {:?}", e);
    }
}

// Desktop entry point
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
