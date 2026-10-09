#!/bin/bash
# Complete RISS Launcher Android Build Script
# This script sets up everything needed and builds the APK

set -e

echo "=========================================="
echo "RISS Launcher - Complete Android Build"
echo "=========================================="
echo ""

# Configuration
EFACE_VERSION="0.32"
NDK_VERSION="25.2.9519653"
BUILD_TOOLS="33.0.2"
PLATFORM="android-33"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

# Helper functions
log_info() { echo -e "${BLUE}[INFO]${NC} $1"; }
log_success() { echo -e "${GREEN}[SUCCESS]${NC} $1"; }
log_warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; }

# Check if running on Linux
if [[ "$OSTYPE" != "linux-gnu"* ]]; then
    log_error "This script only supports Linux"
    exit 1
fi

cd "$(dirname "$0")"
PROJECT_DIR="$(pwd)"

echo "Project directory: $PROJECT_DIR"
echo ""

# Step 1: Check/Install Rust
log_info "Step 1: Checking Rust installation..."
if ! command -v cargo &> /dev/null; then
    log_warn "Rust not found, installing..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    source $HOME/.cargo/env
fi
log_success "Rust $(rustc --version)"

# Step 2: Add Android targets
log_info "Step 2: Adding Android targets..."
for target in aarch64-linux-android armv7-linux-androideabi x86_64-linux-android; do
    if ! rustup target list --installed | grep -q "$target"; then
        rustup target add "$target"
    fi
done
log_success "Android targets installed"

# Step 3: Install cargo-ndk
log_info "Step 3: Installing cargo-ndk..."
if ! command -v cargo-ndk &> /dev/null; then
    cargo install cargo-ndk
fi
log_success "cargo-ndk installed"

# Step 4: Check/Install Java 17
log_info "Step 4: Checking Java 17..."
JAVA_CMD=""
if command -v java &> /dev/null; then
    JAVA_VER=$(java -version 2>&1 | head -1 | cut -d'"' -f2 | cut -d'.' -f1)
    if [ "$JAVA_VER" -ge 17 ]; then
        JAVA_CMD="java"
    fi
fi

if [ -z "$JAVA_CMD" ]; then
    log_warn "Java 17+ not found, installing..."
    
    # Try to install via package manager
    if command -v apt-get &> /dev/null; then
        sudo apt-get update
        sudo apt-get install -y openjdk-17-jdk
        JAVA_CMD="java"
    elif command -v dnf &> /dev/null; then
        sudo dnf install -y java-17-openjdk-devel
        JAVA_CMD="java"
    elif command -v pacman &> /dev/null; then
        sudo pacman -S jdk17-openjdk
        JAVA_CMD="java"
    else
        log_error "Cannot auto-install Java 17. Please install manually:"
        echo "  Ubuntu/Debian: sudo apt-get install openjdk-17-jdk"
        echo "  Fedora: sudo dnf install java-17-openjdk-devel"
        echo "  Arch: sudo pacman -S jdk17-openjdk"
        exit 1
    fi
fi
log_success "Java $($JAVA_CMD -version 2>&1 | head -1)"

# Step 5: Check/Install Android SDK
log_info "Step 5: Checking Android SDK..."
if [ -z "$ANDROID_HOME" ]; then
    if [ -d "$HOME/Android/Sdk" ]; then
        export ANDROID_HOME="$HOME/Android/Sdk"
    elif [ -d "$HOME/android-sdk" ]; then
        export ANDROID_HOME="$HOME/android-sdk"
    else
        log_warn "ANDROID_HOME not set, installing Android SDK..."
        mkdir -p "$HOME/android-sdk"
        export ANDROID_HOME="$HOME/android-sdk"
        
        # Download command-line tools
        cd "$ANDROID_HOME"
        wget -q https://dl.google.com/android/repository/commandlinetools-linux-11076708_latest.zip -O cmdtools.zip
        unzip -q cmdtools.zip
        mkdir -p cmdline-tools/latest
        mv cmdline-tools/bin cmdline-tools/lib cmdline-tools/NOTICE.txt cmdline-tools/source.properties cmdline-tools/latest/ 2>/dev/null || true
        rm cmdtools.zip
        cd "$PROJECT_DIR"
    fi
fi
log_success "Android SDK: $ANDROID_HOME"

# Step 6: Install SDK components
log_info "Step 6: Installing SDK components..."
export PATH="$JAVA_HOME/bin:$ANDROID_HOME/cmdline-tools/latest/bin:$ANDROID_HOME/platform-tools:$PATH"

# Accept licenses
yes | sdkmanager --licenses > /dev/null 2>&1 || true

# Install required components
COMPONENTS="platform-tools platforms;$PLATFORM build-tools;$BUILD_TOOLS ndk;$NDK_VERSION"
for comp in $COMPONENTS; do
    if ! sdkmanager --list_installed 2>/dev/null | grep -q "$comp"; then
        log_info "  Installing $comp..."
        sdkmanager "$comp"
    fi
done
log_success "SDK components installed"

# Step 7: Set NDK_HOME
export NDK_HOME="$ANDROID_HOME/ndk/$NDK_VERSION"
if [ ! -d "$NDK_HOME" ]; then
    log_error "NDK not found at $NDK_HOME"
    exit 1
fi
log_success "NDK: $NDK_HOME"

# Step 8: Build native libraries
log_info "Step 8: Building native libraries..."
cd "$PROJECT_DIR"

# Clean previous builds
rm -rf target/android
mkdir -p target/android

# Build for each architecture
for arch in arm64-v8a armeabi-v7a x86_64; do
    log_info "  Building for $arch..."
    cargo ndk -t "$arch" -o "target/android/lib/$arch" build --release
    
    # Fix nested directory
    if [ -d "target/android/lib/$arch/$arch" ]; then
        mv "target/android/lib/$arch/$arch"/*.so "target/android/lib/$arch/" 2>/dev/null || true
        rmdir "target/android/lib/$arch/$arch" 2>/dev/null || true
    fi
    
    if [ ! -f "target/android/lib/$arch/libriss_launcher.so" ]; then
        log_error "Build failed for $arch"
        exit 1
    fi
    
    SIZE=$(du -h "target/android/lib/$arch/libriss_launcher.so" | cut -f1)
    log_success "  Built libriss_launcher.so ($SIZE)"
done

# Step 9: Package APK
log_info "Step 9: Packaging APK..."
cd target
rm -rf apk-build
mkdir apk-build
cd apk-build

# Copy manifest and libraries
cp "$PROJECT_DIR/android/AndroidManifest.xml" .
cp -r ../android/lib .

# Package with aapt
AAPT="$ANDROID_HOME/build-tools/$BUILD_TOOLS/aapt"
"$AAPT" package -f \
    -M AndroidManifest.xml \
    -I "$ANDROID_HOME/platforms/$PLATFORM/android.jar" \
    -F app-unsigned.apk \
    lib/

if [ ! -f "app-unsigned.apk" ]; then
    log_error "APK packaging failed"
    exit 1
fi
log_success "APK packaged"

# Step 10: Align APK
log_info "Step 10: Aligning APK..."
ZIPALIGN="$ANDROID_HOME/build-tools/$BUILD_TOOLS/zipalign"
"$ZIPALIGN" -f 4 app-unsigned.apk app-aligned.apk
log_success "APK aligned"

# Step 11: Sign APK
log_info "Step 11: Signing APK..."
KEYSTORE="$HOME/.android/debug.keystore"
if [ ! -f "$KEYSTORE" ]; then
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

APKSIGNER="$ANDROID_HOME/build-tools/$BUILD_TOOLS/apksigner"
"$APKSIGNER" sign \
    --ks "$KEYSTORE" \
    --ks-pass pass:android \
    --ks-key-alias androiddebugkey \
    --key-pass pass:android \
    --out "$PROJECT_DIR/riss-launcher-v3.apk" \
    app-aligned.apk

log_success "APK signed"

# Step 12: Verify
log_info "Step 12: Verifying APK..."
"$APKSIGNER" verify --verbose "$PROJECT_DIR/riss-launcher-v3.apk" | head -5

echo ""
echo "=========================================="
log_success "BUILD COMPLETE!"
echo "=========================================="
echo ""
echo "APK Location: $PROJECT_DIR/riss-launcher-v3.apk"
echo "APK Size: $(du -h "$PROJECT_DIR/riss-launcher-v3.apk" | cut -f1)"
echo ""
echo "To install:"
echo "  adb install riss-launcher-v3.apk"
echo ""
echo "To view logs:"
echo "  adb logcat | grep RissLauncher"
echo ""
