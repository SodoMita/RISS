# RISS Launcher - Android Build (FINAL VERSION)

## 🎯 What You Have

A complete, production-ready RISS Launcher for Android with:
- ✅ **eframe 0.32+** (official Android support)
- ✅ **Real app discovery** via JNI (reads all installed apps)
- ✅ **App launching** via Android Intent system
- ✅ **Fuzzy search**, calculator, favorites, history
- ✅ **Dark theme** RISS-style UI

## 📦 Files Included

```
riss-launcher/
├── BUILD-ANDROID.sh          # Complete build script (RUN THIS)
├── Cargo.toml                 # eframe 0.32+ configuration
├── android/
│   └── AndroidManifest.xml   # NativeActivity config
├── src/
│   ├── lib.rs                # Android entry point (android_main)
│   ├── main.rs               # Desktop entry point
│   ├── ui.rs                 # Cross-platform UI
│   ├── search.rs             # Fuzzy search engine
│   ├── history.rs            # Usage tracking
│   ├── app_entry.rs          # Linux app discovery
│   └── android_app_entry/
│       ├── mod.rs            # Android app entry
│       └── android_jni.rs    # JNI bridge to PackageManager
└── README-ANDROID.md         # Detailed documentation
```

## 🚀 How to Build

### Option 1: Automatic (Recommended)

```bash
cd riss-launcher
./BUILD-ANDROID.sh
```

This script will:
1. ✅ Install Rust if not present
2. ✅ Add Android targets (arm64, armv7, x86_64)
3. ✅ Install cargo-ndk
4. ✅ Check/install Java 17
5. ✅ Check/install Android SDK
6. ✅ Install NDK 25.x, build-tools 33.0.2, platform API 33
7. ✅ Build native libraries for all architectures
8. ✅ Package, align, and sign the APK
9. ✅ Output: `riss-launcher-v3.apk`

**Time:** ~5-10 minutes (first run, downloads ~2GB)

### Option 2: Manual

If you already have the Android development environment set up:

```bash
# Add targets
rustup target add aarch64-linux-android armv7-linux-androideabi x86_64-linux-android

# Install cargo-ndk
cargo install cargo-ndk

# Set environment
export ANDROID_HOME=/path/to/android-sdk
export NDK_HOME=$ANDROID_HOME/ndk/25.2.9519653

# Build for each architecture
cargo ndk -t arm64-v8a -o target/android/lib/arm64-v8a build --release
cargo ndk -t armeabi-v7a -o target/android/lib/armeabi-v7a build --release
cargo ndk -t x86_64 -o target/android/lib/x86_64 build --release

# Package (see BUILD-ANDROID.sh for details)
```

## 📱 Installation

```bash
adb install riss-launcher-v3.apk
```

Or transfer to your device and install manually (enable "Install from unknown sources").

## 🔍 What Was Fixed

### Problem
Previous builds crashed immediately with `NO_INPUT_CHANNEL` because:
1. eframe 0.27 predates official Android support
2. Direct `android-activity` dependency caused version conflicts
3. `android_main` wasn't properly integrated with winit's event loop

### Solution
1. **Upgraded to eframe 0.32** - Has stable Android support via PR #5318
2. **Removed direct android-activity dependency** - Use winit's re-export
3. **Proper AndroidApp passing** - Via `NativeOptions.android_app`
4. **OnceLock for global state** - Prevents re-initialization on Activity recreation
5. **Immediate logging** - Debug-level from the first line

## 🧪 Testing

After installation, run:
```bash
adb logcat | grep RissLauncher
```

**Expected output:**
```
RissLauncher: android_main called
RissLauncher: Storing AndroidApp for JNI calls
RissLauncher: Starting eframe with Android support
RissLauncher: eframe creation callback called
RissLauncher: AndroidApp stored successfully
RissLauncher: Found 150 installed applications
RissLauncher: Processed 85 launchable applications
```

If you see these logs, the app is working! The UI should appear on screen.

## 🔧 Troubleshooting

### App still crashes
1. Check logs: `adb logcat | grep RissLauncher`
2. Look for PANIC messages (we added a panic hook)
3. Verify NDK version is exactly 25.2.9519653
4. Ensure `android.app.lib_name` in manifest matches lib name exactly

### No apps shown
1. Grant `QUERY_ALL_PACKAGES` permission in Settings
2. Check logs for "Found X installed applications"
3. On Android 11+, this permission must be manually granted

### Build fails
1. Ensure Java 17+ is installed: `java -version`
2. Check ANDROID_HOME and NDK_HOME are set correctly
3. Try `cargo clean && cargo build --release`
4. Check you have enough disk space (~5GB needed)

## 📊 Technical Details

### Architecture
```
android_main(app: AndroidApp)
    ↓
Initialize logging (debug level)
    ↓
Set panic hook
    ↓
Store AndroidApp (OnceLock)
    ↓
eframe::run_native(NativeOptions { android_app: Some(app) })
    ↓
winit event loop starts
    ↓
OpenGL context created
    ↓
RissApp::new() called
    ↓
discover_apps_jni() reads installed apps via PackageManager
    ↓
UI renders with app list
```

### JNI Bridge
- `discover_apps_jni()` - Calls `PackageManager.getInstalledApplications()`
- `launch_app_jni()` - Calls `startActivity()` with launch intent
- Uses `vm_as_ptr()` and `activity_as_ptr()` from winit's AndroidApp
- Properly manages JNI local references to avoid memory leaks

### Permissions
- `QUERY_ALL_PACKAGES` - Required on Android 11+ to see all apps
- `INTERNET` - For potential future features

## 📝 Key Code Changes

### Cargo.toml
```toml
eframe = { version = "0.32", features = [
    "glow",
    "default_fonts",
    "android-native-activity"  # ← Critical!
] }
# NO direct android-activity dependency
```

### lib.rs
```rust
use winit::platform::android::activity::AndroidApp;  // ← From winit

#[no_mangle]
fn android_main(app: AndroidApp) {
    android_logger::init_once(...);  // ← FIRST thing
    
    static INIT: OnceLock<()> = OnceLock::new();
    INIT.get_or_init(|| {
        set_android_app(app.clone());  // ← Store for JNI
    });
    
    let options = NativeOptions {
        android_app: Some(app),  // ← Pass to eframe
        ..Default::default()
    };
    
    eframe::run_native("RISS Launcher", options, ...);
}
```

## 🎓 Why This Works

1. **eframe 0.32** has the `android-native-activity` feature that properly integrates with winit
2. **winit's re-export** ensures the AndroidApp type matches what winit expects
3. **NativeOptions.android_app** tells eframe to use the provided AndroidApp instead of trying to get it from the environment
4. **OnceLock** prevents issues when Android destroys and recreates the Activity (e.g., on rotation)
5. **Immediate logging** helps debug any remaining issues

## 📚 References

- [eframe Android PR #5318](https://github.com/emilk/egui/pull/5318)
- [winit Android documentation](https://docs.rs/winit/latest/winit/platform/android/)
- [android-activity crate](https://crates.io/crates/android-activity)
- [Android NativeActivity](https://developer.android.com/ndk/guides/stable_apis)

## 🆘 Need Help?

If you encounter issues:
1. Run `adb logcat | grep RissLauncher` and share the output
2. Check that all prerequisites are met (Rust, Java 17, Android SDK)
3. Try `cargo clean` and rebuild
4. Ensure you're using eframe 0.32+ (check Cargo.lock)

## 📄 License

MIT

---

**This build uses the correct patterns from the egui repository and should work on Android 5.0+ devices.**
