# RISS Launcher - Android Build

## APK Information

✅ **APK Built Successfully!**

- **File:** `riss-launcher.apk`
- **Size:** 5.9 MB
- **Package:** `com.risslauncher.app`
- **Version:** 0.1.0 (versionCode: 1)
- **Min SDK:** 21 (Android 5.0 Lollipop)
- **Target SDK:** 33 (Android 13)
- **Architectures:** arm64-v8a, armeabi-v7a, x86_64

## Installation

### On a Physical Device

1. **Enable USB Debugging** on your Android device:
   - Go to Settings → About Phone
   - Tap "Build Number" 7 times to enable Developer Options
   - Go to Settings → Developer Options → Enable USB Debugging

2. **Connect device via USB** and install:
   ```bash
   adb install riss-launcher.apk
   ```

3. **Launch the app** from your app drawer

### On an Emulator

1. Start an Android emulator (API 21+)
2. Install:
   ```bash
   adb install riss-launcher.apk
   ```

### Direct Install (if allowed)

1. Copy `riss-launcher.apk` to your device
2. Open the file manager and tap the APK
3. Allow installation from unknown sources if prompted
4. Install and launch

## Features on Android

The Android version includes:

- ✅ RISS-style search bar at the bottom
- ✅ Fuzzy search through apps
- ✅ Favorites system
- ✅ Usage tracking
- ✅ Custom tags
- ✅ Built-in calculator
- ✅ Dark theme
- ✅ Touch-friendly UI

**Note:** The Android version uses a demo set of popular apps (Chrome, Gmail, YouTube, etc.) since reading installed apps requires additional Android permissions and JNI integration.

## Rebuilding the APK

To rebuild the APK from source:

```bash
# Prerequisites
# - Rust with Android targets installed
# - Android SDK with NDK 25.x
# - JDK 17

# Build the APK
./build-apk.sh
```

The script will:
1. Build native libraries for all 3 architectures
2. Package them into an APK
3. Sign with debug key
4. Align the APK

## Technical Details

### Build Process

1. **Cross-compilation:** Uses `cargo-ndk` to build Rust code for Android
2. **Native Activity:** Uses `android-activity` crate for the activity lifecycle
3. **Graphics:** OpenGL ES via egui's glow backend
4. **Packaging:** Uses Android SDK tools (aapt, zipalign, apksigner)

### File Structure

```
riss-launcher.apk
├── AndroidManifest.xml (binary format)
├── lib/
│   ├── arm64-v8a/libriss_launcher.so (4.1 MB)
│   ├── armeabi-v7a/libriss_launcher.so (3.3 MB)
│   └── x86_64/libriss_launcher.so (4.4 MB)
└── META-INF/ (signatures)
```

### Signature

- Signed with debug keystore
- v1 (JAR signing): ✅
- v2 (APK Signature Scheme v2): ✅
- v3 (APK Signature Scheme v3): ✅

## Troubleshooting

### "App not installed" error
- Ensure your device supports one of: arm64-v8a, armeabi-v7a, or x86_64
- Check that you have enough storage space
- Try uninstalling any previous version first

### App crashes on launch
- Check logcat for errors: `adb logcat | grep RissLauncher`
- Ensure your device has OpenGL ES 2.0+ support
- Try on a different device or emulator

### Black screen or no UI
- The app requires OpenGL ES support
- Some emulators may not support hardware acceleration
- Try enabling "Use Host GPU" in emulator settings

## Release Build

For a production release, you would need to:

1. **Create a release keystore:**
   ```bash
   keytool -genkey -v -keystore release.keystore -alias riss -keyalg RSA -keysize 2048 -validity 10000
   ```

2. **Update build-apk.sh** to use your release keystore

3. **Add app icons** in `android/res/` directory

4. **Implement real app launching** via JNI/intents (currently logs only)

5. **Request QUERY_ALL_PACKAGES permission** to list installed apps (Android 11+)

## Comparison with Linux Version

| Feature | Linux | Android |
|---------|-------|---------|
| App Discovery | ✅ Reads .desktop files | ⚠️ Demo apps only |
| App Launching | ✅ Uses xdg-open | ⚠️ Logs only (needs JNI) |
| History Storage | ~/.config/riss-launcher/ | App internal storage |
| Fonts | DejaVu Sans | Roboto |
| Binary Size | 7.3 MB | 5.9 MB (multi-arch) |

## Next Steps for Full Android Support

To make this a fully functional Android launcher:

1. **Implement PackageManager integration** via JNI to list installed apps
2. **Add intent-based launching** to actually start other apps
3. **Request necessary permissions** (QUERY_ALL_PACKAGES for Android 11+)
4. **Add app icons** from package manager
5. **Implement as a proper launcher** with HOME category intent filter
6. **Add widget support** (optional)
7. **Implement wallpaper support** (optional)

## Support

For issues or questions:
- Check `adb logcat` for error messages
- Verify your device meets the minimum requirements (Android 5.0+)
- Try on an emulator first to isolate device-specific issues

---

**Built with Rust 🦀 + egui + Android NDK**
