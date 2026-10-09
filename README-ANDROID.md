# KISS Launcher - Android Version

A KISS-style launcher for Android built with Rust and eframe.

## ✅ What Was Fixed

### 1. App Now Launches Successfully
- **Problem:** Previous builds crashed with `NO_INPUT_CHANNEL` errors
- **Solution:** Upgraded to eframe 0.32+ with proper Android support
- **Key Changes:**
  - Use `winit::platform::android::activity::AndroidApp` (re-export)
  - Pass `AndroidApp` via `NativeOptions.android_app`
  - Use `OnceLock` for thread-safe global state
  - Proper entry point signature

### 2. All Apps Now Display by Default
- **Problem:** Only favorites and frequently used apps were shown when search was empty
- **Solution:** Modified `update_results()` to show ALL installed apps
- **Sorting:** Favorites first → Most used → Alphabetical

### 3. Icon Display Improved
- **Current:** Shows emoji icons based on app category or first letter of app name
- **Examples:**
  - 🎮 Games
  - 🌐 Browsers
  - 📁 File managers
  - 📝 Text editors
  - "F" for Firefox (first letter fallback)

## 📱 Features

- ✅ **Real app discovery** - Reads all installed apps via JNI
- ✅ **App launching** - Launches apps via Android Intent system
- ✅ **Fuzzy search** - Find apps quickly with intelligent matching
- ✅ **Calculator** - Type math expressions (e.g., "2+2", "sqrt(16)")
- ✅ **Favorites** - Star your most-used apps
- ✅ **History** - Tracks usage frequency
- ✅ **Tags** - Auto-generated from package names
- ✅ **Dark theme** - Easy on the eyes
- ✅ **All apps visible** - Shows complete app list by default

## 🔧 Building the APK

Run the master build script:

```bash
cd /home/user/kiss-launcher
./MASTER-BUILD.sh
```

The script will:
1. Install Rust, Java 17, Android SDK, NDK, build-tools (if not present)
2. Build native libraries for arm64-v8a, armeabi-v7a, x86_64
3. Package, align, and sign the APK
4. Output: `kiss-launcher-FINAL.apk`

**Build time:** ~10-15 minutes (first run), ~5 minutes (subsequent runs)

## 📲 Installation

```bash
adb install kiss-launcher-FINAL.apk
```

Or transfer to your device and install manually (enable "Install from unknown sources").

## 🔍 Debugging

View logs in real-time:

```bash
adb logcat | grep KissLauncher
```

Expected output on successful launch:
```
KissLauncher: android_main called
KissLauncher: Storing AndroidApp for JNI calls
KissLauncher: Starting eframe with Android support
KissLauncher: eframe creation callback called
KissLauncher: Found X installed applications
KissLauncher: Processed Y launchable applications
```

## 📂 Project Structure

```
kiss-launcher/
├── MASTER-BUILD.sh          # Complete build script
├── Cargo.toml               # eframe 0.32+ configuration
├── android/
│   └── AndroidManifest.xml # NativeActivity config
├── src/
│   ├── lib.rs              # Android entry point (android_main)
│   ├── main.rs             # Desktop entry point
│   ├── ui.rs               # Cross-platform UI
│   ├── search.rs           # Fuzzy search engine
│   ├── history.rs          # Usage tracking
│   ├── app_entry.rs        # Linux app discovery
│   └── android_app_entry/
│       ├── mod.rs          # Android app entry
│       └── android_jni.rs  # JNI bridge to PackageManager
└── README-ANDROID.md       # This file
```

## 🎯 Technical Details

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
KissApp::new() called
    ↓
discover_apps_jni() reads installed apps via PackageManager
    ↓
UI renders with ALL apps (sorted by favorites → usage → name)
```

### JNI Bridge
- `discover_apps_jni()` - Calls `PackageManager.getInstalledApplications()`
- `launch_app_jni()` - Calls `startActivity()` with launch intent
- Uses `vm_as_ptr()` and `activity_as_ptr()` from winit's AndroidApp
- Properly manages JNI local references to avoid memory leaks

### Permissions
- `QUERY_ALL_PACKAGES` - Required on Android 11+ to see all apps
- `INTERNET` - For potential future features

## 🔧 Configuration

### Cargo.toml
```toml
eframe = { version = "0.32", features = [
    "glow",
    "default_fonts",
    "android-native-activity"  # ← Critical!
] }

winit = { version = "0.30", features = ["android-native-activity"] }
```

### AndroidManifest.xml
```xml
<application android:hasCode="false">
    <activity android:name="android.app.NativeActivity">
        <!-- CRITICAL: must match [lib] name exactly -->
        <meta-data android:name="android.app.lib_name" 
                   android:value="kiss_launcher" />
    </activity>
</application>
```

## 🐛 Troubleshooting

### App crashes immediately
1. Check logs: `adb logcat | grep KissLauncher`
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

## 📊 Comparison with Previous Versions

| Issue | v1 (eframe 0.27) | v2 (eframe 0.32+) |
|-------|------------------|-------------------|
| Android support | Pre-official | Official (stable) |
| Entry point | Conflicting | Proper integration |
| android-activity | Direct dependency | Via winit re-export |
| Global state | Re-initialized | OnceLock protected |
| App list | Favorites only | ALL apps |
| Crash on launch | ❌ YES | ✅ Fixed |
| Reads installed apps | ✅ YES | ✅ YES |
| Launches apps | ✅ YES | ✅ YES |

## 🚀 Future Improvements

- [ ] Load actual app icons via JNI (currently using emoji/letters)
- [ ] Add app icon caching for better performance
- [ ] Implement contact search
- [ ] Add settings search
- [ ] Support for app widgets
- [ ] Custom themes
- [ ] Gesture support

## 📚 References

- [eframe Android PR #5318](https://github.com/emilk/egui/pull/5318)
- [winit Android documentation](https://docs.rs/winit/latest/winit/platform/android/)
- [android-activity crate](https://crates.io/crates/android-activity)
- [Android NativeActivity](https://developer.android.com/ndk/guides/stable_apis)

## 📄 License

MIT

---

**Built with ❤️ using Rust, eframe, and Android NDK**
