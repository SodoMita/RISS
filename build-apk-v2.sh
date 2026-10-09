#!/bin/bash
# Build APK for RISS Launcher using eframe 0.32+
# This script handles the complete Android build process

set -e

echo "⚡ Building RISS Launcher APK (eframe 0.32+)"
echo "=============================================="

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Check for required tools
check_tool() {
    if ! command -v $1 &> /dev/null; then
        echo -e "${RED}Error: $1 is not installed${NC}"
        return 1
    fi
    return 0
}

echo "Checking prerequisites..."

# Check Rust
if ! check_tool cargo; then
    echo -e "${YELLOW}Installing Rust...${NC}"
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    source $HOME/.cargo/env
fi

# Check Android targets
echo "Checking Android targets..."
for target in aarch64-linux-android armv7-linux-androideabi x86_64-linux-android; do
    if ! rustup target list --installed | grep -q $target; then
        echo -e "${YELLOW}Adding target: $target${NC}"
        rustup target add $target
    fi
done

# Check cargo-ndk
if ! check_tool cargo-ndk; then
    echo -e "${YELLOW}Installing cargo-ndk...${NC}"
    cargo install cargo-ndk
fi

# Check Android SDK
if [ -z "$ANDROID_HOME" ]; then
    if [ -d "$HOME/android-sdk" ]; then
        export ANDROID_HOME="$HOME/android-sdk"
    else
        echo -e "${RED}Error: ANDROID_HOME not set and no SDK found at ~/android-sdk${NC}"
        echo "Please install Android SDK or set ANDROID_HOME"
        exit 1
    fi
fi

# Check NDK
if [ -z "$NDK_HOME" ]; then
    NDK_PATH="$ANDROID_HOME/ndk/25.2.9519653"
    if [ -d "$NDK_PATH" ]; then
        export NDK_HOME="$NDK_PATH"
    else
        echo -e "${RED}Error: NDK not found at $NDK_PATH${NC}"
        echo "Please install NDK 25.x via sdkmanager"
        exit 1
    fi
fi

echo -e "${GREEN}✓ All prerequisites met${NC}"
echo ""

# Build for all architectures
cd "$(dirname "$0")"
BUILD_DIR="target/android-build"
APK_DIR="target/android"

rm -rf "$BUILD_DIR" "$APK_DIR"
mkdir -p "$BUILD_DIR" "$APK_DIR"

echo "📦 Building native libraries..."

for arch in arm64-v8a armeabi-v7a x86_64; do
    echo -e "${YELLOW}  Building for $arch...${NC}"
    
    cargo ndk -t $arch -o "$APK_DIR/lib/$arch" build --release 2>&1 | tail -5
    
    # Fix nested directory structure from cargo-ndk
    if [ -d "$APK_DIR/lib/$arch/$arch" ]; then
        mv "$APK_DIR/lib/$arch/$arch"/*.so "$APK_DIR/lib/$arch/" 2>/dev/null || true
        rmdir "$APK_DIR/lib/$arch/$arch" 2>/dev/null || true
    fi
    
    if [ -f "$APK_DIR/lib/$arch/libriss_launcher.so" ]; then
        SIZE=$(du -h "$APK_DIR/lib/$arch/libriss_launcher.so" | cut -f1)
        echo -e "  ${GREEN}✓ Built libriss_launcher.so ($SIZE)${NC}"
    else
        echo -e "  ${RED}✗ Build failed for $arch${NC}"
        exit 1
    fi
done

echo ""
echo "📦 Creating APK structure..."

# Copy manifest
cp android/AndroidManifest.xml "$BUILD_DIR/"

# Copy libraries
mkdir -p "$BUILD_DIR/lib"
cp -r "$APK_DIR/lib/"* "$BUILD_DIR/lib/"

echo "  Added AndroidManifest.xml"
for arch in arm64-v8a armeabi-v7a x86_64; do
    echo "  Added lib/$arch/libriss_launcher.so"
done

echo ""
echo "🔗 Packaging APK..."

# Check for aapt
AAPT_PATH="$ANDROID_HOME/build-tools/33.0.2/aapt"
if [ ! -f "$AAPT_PATH" ]; then
    echo -e "${RED}Error: aapt not found at $AAPT_PATH${NC}"
    echo "Please install build-tools via: sdkmanager 'build-tools;33.0.2'"
    exit 1
fi

cd "$BUILD_DIR"
"$AAPT_PATH" package -f \
    -M AndroidManifest.xml \
    -I "$ANDROID_HOME/platforms/android-33/android.jar" \
    -F app-unsigned.apk \
    lib/

if [ ! -f "app-unsigned.apk" ]; then
    echo -e "${RED}Error: APK packaging failed${NC}"
    exit 1
fi

echo -e "${GREEN}✓ APK packaged${NC}"

echo ""
echo "📐 Aligning APK..."

ZIPALIGN_PATH="$ANDROID_HOME/build-tools/33.0.2/zipalign"
"$ZIPALIGN_PATH" -f 4 app-unsigned.apk app-aligned.apk

echo -e "${GREEN}✓ APK aligned${NC}"

echo ""
echo "🔑 Signing APK..."

# Generate debug keystore if needed
KEYSTORE="$HOME/.android/debug.keystore"
if [ ! -f "$KEYSTORE" ]; then
    echo -e "${YELLOW}Generating debug keystore...${NC}"
    mkdir -p "$HOME/.android"
    
    # Check for keytool
    if command -v keytool &> /dev/null; then
        KEYTOOL_CMD="keytool"
    elif [ -n "$JAVA_HOME" ] && [ -f "$JAVA_HOME/bin/keytool" ]; then
        KEYTOOL_CMD="$JAVA_HOME/bin/keytool"
    else
        echo -e "${RED}Error: keytool not found. Please install JDK or set JAVA_HOME${NC}"
        exit 1
    fi
    
    "$KEYTOOL_CMD" -genkey -v \
        -keystore "$KEYSTORE" \
        -storepass android \
        -alias androiddebugkey \
        -keypass android \
        -keyalg RSA \
        -keysize 2048 \
        -validity 10000 \
        -dname "CN=Android Debug,O=Android,C=US"
fi

APKSIGNER_PATH="$ANDROID_HOME/build-tools/33.0.2/apksigner"
"$APKSIGNER_PATH" sign \
    --ks "$KEYSTORE" \
    --ks-pass pass:android \
    --ks-key-alias androiddebugkey \
    --key-pass pass:android \
    --out ../../riss-launcher.apk \
    app-aligned.apk

echo -e "${GREEN}✓ APK signed${NC}"

echo ""
echo "✅ Verifying APK..."

"$APKSIGNER_PATH" verify --verbose ../../riss-launcher.apk | head -5

echo ""
echo "📋 APK contents:"
unzip -l ../../riss-launcher.apk | head -15

echo ""
echo -e "${GREEN}============================================${NC}"
echo -e "${GREEN}✅ APK built successfully!${NC}"
echo -e "${GREEN}============================================${NC}"
echo ""
echo "📁 Location: $(pwd)/../../riss-launcher.apk"
echo "📏 Size: $(du -h ../../riss-launcher.apk | cut -f1)"
echo ""
echo "To install:"
echo "  adb install riss-launcher.apk"
echo ""
echo "To view logs:"
echo "  adb logcat | grep RissLauncher"
echo ""
