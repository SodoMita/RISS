// RISS Launcher - Android entry point
// Uses eframe 0.32+ official Android support
#![allow(dead_code)]

#[cfg(target_os = "android")]
mod android_app_entry;
#[cfg(not(target_os = "android"))]
mod app_entry;

mod history;
#[cfg(not(target_os = "android"))]
mod package_managers;
mod providers;
mod search;
mod settings;
mod storage;
#[cfg(test)]
mod tests;

mod ui;

#[cfg(target_os = "android")]
use winit::platform::android::activity::AndroidApp;

#[cfg(target_os = "android")]
#[no_mangle]
fn android_main(app: AndroidApp) {
    use log::LevelFilter;
    use std::sync::atomic::{AtomicBool, Ordering};

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

    // `android_main` runs once per Activity *instance*, not once per process
    // (see the android-activity crate docs). When Android destroys and
    // recreates the Activity while the process lives on, this second run
    // can never work:
    //  - winit 0.30 builds one EventLoop per process; a second build fails
    //    with `EventLoopError::RecreationAttempt` (winit src/event_loop.rs),
    //    so `eframe::run_native` below would return an error at once;
    //  - winit's Android backend does not leave the first loop on
    //    `MainEvent::Destroy` (a TODO in platform_impl/android), so the old
    //    loop keeps the old, destroyed Activity;
    //  - the JNI helpers would keep using that destroyed Activity, so its
    //    internal data path is gone and settings/history fall back to the
    //    unwritable working directory (issue #19).
    // Returning makes android-activity finish the new Activity, and Android
    // relaunches the home screen into the same dead process: the launcher
    // never comes back until the process is killed. Start over instead: a
    // fresh process gets fresh statics and Android relaunches the pending
    // Activity into it. The manifest's configChanges keep this path rare.
    static STARTED: AtomicBool = AtomicBool::new(false);
    if STARTED.swap(true, Ordering::SeqCst) {
        log::warn!(
            "Activity recreated inside a live process; restarting the process \
             because winit cannot build a second event loop"
        );
        std::process::exit(0);
    }
    android_app_entry::android_jni::set_android_app(app.clone());

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
    // Settings are needed before the window exists to open it in the right
    // mode (`--fullscreen` / `--maximized` / `--windowed` override them).
    let (settings, notes) = settings::SettingsData::load();
    // Ask for a transparency capable framebuffer, like the Android path does:
    // a plain X11/OpenGL window is RGB-only, so  can
    // return alpha 0 and the compositor still shows an opaque slab. With an
    // RGBA surface, Sway (and friends) composite the desktop wallpaper
    // through whenever the transparent theme is on; opaque themes clear with
    // full alpha, so nothing changes for them.
    let viewport = egui::ViewportBuilder::default()
        .with_inner_size(ui::window_mode::WindowMode::DEFAULT_INNER_SIZE)
        .with_min_inner_size([300.0, 400.0])
        .with_transparent(true);
    let native_options = eframe::NativeOptions {
        viewport: ui::window_mode::WindowMode::apply_startup(&settings, viewport),
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
