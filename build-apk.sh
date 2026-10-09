#!/bin/bash
# Build APK for KISS Launcher

set -e

export ANDROID_HOME=/home/user/android-sdk
export NDK_HOME=$ANDROID_HOME/ndk/25.2.9519653
export JAVA_HOME=/home/user/jdk-17
export PATH=$JAVA_HOME/bin:$ANDROID_HOME/build-tools/33.0.2:$ANDROID_HOME/platform-tools:$PATH

PROJECT_DIR="$(cd "$(dirname "$0")" && pwd)"
BUILD_DIR="$PROJECT_DIR/target/android-build"
APK_DIR="$PROJECT_DIR/target/android"
MANIFEST="$PROJECT_DIR/android/AndroidManifest.xml"

echo "⚡ Building KISS Launcher APK"
echo "=============================="

# Clean previous build
rm -rf "$BUILD_DIR"
mkdir -p "$BUILD_DIR"
mkdir -p "$APK_DIR"

# Step 1: Build native libraries for all architectures
echo "📦 Building native libraries..."
source /home/user/.cargo/env

echo "  Building for arm64-v8a..."
cargo ndk -t arm64-v8a -o "$APK_DIR/lib/arm64-v8a" build --release 2>&1 | tail -2

echo "  Building for armeabi-v7a..."
cargo ndk -t armeabi-v7a -o "$APK_DIR/lib/armeabi-v7a" build --release 2>&1 | tail -2

echo "  Building for x86_64..."
cargo ndk -t x86_64 -o "$APK_DIR/lib/x86_64" build --release 2>&1 | tail -2

# Fix library paths (cargo-ndk creates nested directories)
for arch in arm64-v8a armeabi-v7a x86_64; do
    LIB_DIR="$APK_DIR/lib/$arch"
    if [ -d "$LIB_DIR/$arch" ]; then
        mv "$LIB_DIR/$arch"/*.so "$LIB_DIR/" 2>/dev/null || true
        rmdir "$LIB_DIR/$arch" 2>/dev/null || true
    fi
done

# Step 2: Create APK structure (without manifest - it's added by aapt)
echo "📦 Creating APK structure..."
rm -rf "$BUILD_DIR/apk-contents"
mkdir -p "$BUILD_DIR/apk-contents/lib"

# Copy native libraries only (manifest is added by aapt -M flag)
for arch in arm64-v8a armeabi-v7a x86_64; do
    if [ -f "$APK_DIR/lib/$arch/libkiss_launcher.so" ]; then
        mkdir -p "$BUILD_DIR/apk-contents/lib/$arch"
        cp "$APK_DIR/lib/$arch/libkiss_launcher.so" "$BUILD_DIR/apk-contents/lib/$arch/"
        echo "  Added lib/$arch/libkiss_launcher.so ($(du -h "$APK_DIR/lib/$arch/libkiss_launcher.so" | cut -f1))"
    fi
done

# Step 3: Use aapt to package the APK with binary XML manifest
echo "🔗 Packaging APK with aapt..."
cd "$BUILD_DIR"
rm -f app-unsigned.apk

# aapt package: -M for manifest (converts to binary XML), -F for output, last arg is resource dir
aapt package -v -f \
    -M "$MANIFEST" \
    -I "$ANDROID_HOME/platforms/android-33/android.jar" \
    -F app-unsigned.apk \
    "$BUILD_DIR/apk-contents" 2>&1 | tail -10

# Verify the APK has binary XML manifest
echo "  Checking manifest format..."
aapt dump badging app-unsigned.apk 2>&1 | head -3

# Step 4: Align APK
echo "📐 Aligning APK..."
zipalign -f -v 4 app-unsigned.apk app-aligned.apk 2>&1 | tail -3

# Step 5: Generate debug keystore if it doesn't exist
KEYSTORE="$HOME/.android/debug.keystore"
if [ ! -f "$KEYSTORE" ]; then
    echo "🔑 Generating debug keystore..."
    mkdir -p "$HOME/.android"
    keytool -genkey -v \
        -keystore "$KEYSTORE" \
        -storepass android \
        -alias androiddebugkey \
        -keypass android \
        -keyalg RSA \
        -keysize 2048 \
        -validity 10000 \
        -dname "CN=Android Debug,O=Android,C=US"
fi

# Step 6: Sign APK
echo "✍️  Signing APK..."
apksigner sign \
    --ks "$KEYSTORE" \
    --ks-pass pass:android \
    --ks-key-alias androiddebugkey \
    --key-pass pass:android \
    --out "$PROJECT_DIR/target/kiss-launcher.apk" \
    app-aligned.apk

# Step 7: Verify APK
echo "✅ Verifying APK..."
apksigner verify --verbose "$PROJECT_DIR/target/kiss-launcher.apk" 2>&1 | head -5

# Show APK contents
echo ""
echo "📋 APK contents:"
unzip -l "$PROJECT_DIR/target/kiss-launcher.apk" | head -20

echo ""
echo "🎉 APK built successfully!"
echo "   Location: $PROJECT_DIR/target/kiss-launcher.apk"
echo "   Size: $(du -h "$PROJECT_DIR/target/kiss-launcher.apk" | cut -f1)"
echo ""
echo "To install on a device:"
echo "  adb install target/kiss-launcher.apk"
echo ""
