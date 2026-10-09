# KISS Launcher - Android APK Built Successfully! 🎉

## APK Details

✅ **APK Size:** 5.9 MB  
✅ **Architectures:** arm64-v8a, armeabi-v7a, x86_64  
✅ **Min SDK:** 21 (Android 5.0 Lollipop)  
✅ **Target SDK:** 33 (Android 13)  
✅ **Signed:** Yes (debug key with v1, v2, v3 signatures)

## What's New in This Version

This Android version now **actually reads installed apps** from your device using JNI (Java Native Interface) to call Android's PackageManager API. Here's what it does:

### Real App Discovery
- Scans all installed applications on your device
- Filters to show only launchable apps (those with launch intents)
- Extracts app names, package names, and categories
- Automatically generates search tags from package names

### Real App Launching
- Launches apps using Android's Intent system
- Uses `PackageManager.getLaunchIntentForPackage()` to get the correct intent
- Starts activities with `FLAG_ACTIVITY_NEW_TASK` for proper behavior

### Smart Features
- **Automatic categorization** based on Android's app categories (Game, Audio, Video, Social, etc.)
- **Smart tag generation** from package names (e.g., "com.spotify.music" → tags: spotify, music, audio)
- **Favorites system** - star your most-used apps
- **Usage tracking** - shows frequently used apps at the top
- **Fuzzy search** - find apps even with typos
- **Built-in calculator** - type math expressions like "2+2" or "sqrt(16)"

## How It Works

### JNI Bridge Architecture

The app uses a JNI bridge to communicate between Rust and Android's Java APIs:

1. **App Discovery** (`discover_apps_jni()`):
   ```rust
   // Get PackageManager
   let pm = context.getPackageManager();
   
   // Get all installed applications
   let apps = pm.getInstalledApplications(GET_META_DATA);
   
   // For each app:
   //   - Get package name
   //   - Check if it has a launch intent
   //   - Get human-readable app label
   //   - Extract category and generate tags
   ```

2. **App Launching** (`launch_app_jni()`):
   ```rust
   // Get launch intent for package
   let intent = pm.getLaunchIntentForPackage(package_name);
   
   // Add FLAG_ACTIVITY_NEW_TASK
   intent.addFlags(0x10000000);
   
   // Start the activity
   context.startActivity(intent);
   ```

### Permissions

The app requests `QUERY_ALL_PACKAGES` permission to see all installed apps on Android 11+ (API 30+). This permission is automatically granted to launcher apps.

## Installation

### Option 1: ADB Install (Recommended)
```bash
cd /home/user/kiss-launcher
adb install target/kiss-launcher.apk
```

### Option 2: Direct Install
1. Copy `target/kiss-launcher.apk` to your Android device
2. Open the file and install
3. Grant permissions when prompted

### Option 3: Set as Default Launcher
After installation, you can set KISS Launcher as your default home screen:
1. Go to Settings → Apps → Default apps → Home app
2. Select "KISS Launcher"

## Building from Source

### Prerequisites
- Rust with Android targets: `aarch64-linux-android`, `armv7-linux-androideabi`, `x86_64-linux-android`
- Android SDK with NDK 25.2.9519653
- Android build-tools 33.0.2
- Android platform API 33
- JDK 17

### Build Steps
```bash
# Set environment variables
export ANDROID_HOME=/path/to/android-sdk
export NDK_HOME=$ANDROID_HOME/ndk/25.2.9519653
export PATH="/path/to/.cargo/bin:$PATH"

# Build APK
cd /home/user/kiss-launcher
./build-apk.sh
```

The script will:
1. Build native libraries for all three architectures using `cargo ndk`
2. Package them into an APK using Android's `aapt` tool
3. Sign the APK with a debug key
4. Align the APK for optimal performance

## APK Structure

```
kiss-launcher.apk (5.9 MB)
├── AndroidManifest.xml (3.3 KB)
│   - Package: com.kisslauncher.app
│   - Min SDK: 21, Target SDK: 33
│   - Permissions: QUERY_ALL_PACKAGES, INTERNET
│   - NativeActivity configuration
│
├── lib/
│   ├── arm64-v8a/libkiss_launcher.so (4.1 MB)
│   ├── armeabi-v7a/libkiss_launcher.so (3.3 MB)
│   └── x86_64/libkiss_launcher.so (4.4 MB)
│
└── META-INF/
    ├── MANIFEST.MF
    ├── ANDROIDD.SF
    └── ANDROIDD.RSA
```

## Technical Implementation

### Key Components

1. **`android_jni.rs`** - JNI bridge for Android API calls
   - `discover_apps_jni()` - Reads installed apps via PackageManager
   - `launch_app_jni()` - Launches apps via Intent system
   - `build_tags_from_package()` - Generates smart search tags

2. **`android_app_entry.rs`** - Android-specific app entry point
   - Stores AndroidApp reference for JNI calls
   - Delegates to JNI bridge for app operations

3. **`lib.rs`** - Main library entry point
   - Initializes Android logger
   - Stores AndroidApp reference globally
   - Launches egui application

4. **`ui.rs`** - Cross-platform UI (shared with Linux version)
   - KISS-style search interface
   - Fuzzy search implementation
   - Favorites and history management

### JNI Safety

The code carefully manages JNI references:
- Uses `JObject::from_raw()` for activity references (global references, not deleted)
- Properly handles `JNIEnv` attachment for native threads
- Uses `Mutex<AndroidApp>` for thread-safe access to the app context

## Troubleshooting

### App doesn't show any apps
- Make sure you granted the QUERY_ALL_PACKAGES permission
- On Android 11+, this permission is required to see all installed apps
- Go to Settings → Apps → KISS Launcher → Permissions → Allow "Display over other apps"

### App crashes on launch
- Check logcat for errors: `adb logcat | grep kiss_launcher`
- Ensure your device supports OpenGL ES 2.0+
- Try on a different device or emulator

### Build fails
- Ensure all Android targets are installed: `rustup target list --installed`
- Verify NDK version is 25.2.9519653
- Check that JAVA_HOME points to JDK 17

## Comparison with Original KISS Launcher

| Feature | Original KISS (Android) | This Implementation |
|---------|------------------------|---------------------|
| App Discovery | ✅ PackageManager | ✅ PackageManager via JNI |
| App Launching | ✅ Intent system | ✅ Intent system via JNI |
| Search | ✅ Fuzzy search | ✅ Fuzzy search |
| Calculator | ✅ Basic math | ✅ Advanced (sqrt, trig, etc.) |
| History | ✅ Usage tracking | ✅ Usage tracking |
| Favorites | ✅ Star apps | ✅ Star apps |
| Tags | ❌ | ✅ Auto-generated from package names |
| UI Framework | Java/Android Views | Rust/egui (OpenGL) |
| Performance | Fast | Very fast (native code) |
| APK Size | ~1 MB | ~6 MB (includes 3 architectures) |

## Future Enhancements

Potential improvements for future versions:

1. **App Icons** - Load and display app icons using JNI
2. **Contact Search** - Search contacts via ContentResolver
3. **Settings Search** - Search system settings
4. **Widgets** - Support for app widgets
5. **Custom Tags** - Allow users to add custom tags
6. **Themes** - Multiple color themes
7. **Gesture Support** - Swipe gestures for navigation

## License

This is a demonstration project showing how to build Android apps with Rust and egui. The KISS Launcher concept is inspired by the original KISS Launcher (https://github.com/Neamar/KISS).

## Credits

- **egui** - Immediate mode GUI library for Rust
- **eframe** - egui framework for native apps
- **android-activity** - Android NativeActivity bindings
- **jni** - JNI bindings for Rust
- **fuzzy-matcher** - Fuzzy string matching

---

**Built with ❤️ using Rust, egui, and Android NDK**
