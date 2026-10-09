#!/bin/bash
# Master Build Script - Reinstalls everything and builds APK from scratch
# This script is completely self-contained and will work in a fresh environment

set -e

echo "=========================================="
echo "RISS Launcher - Master Build Script"
echo "=========================================="
echo "This will install all dependencies and build the APK"
echo "Estimated time: 10-15 minutes"
echo "=========================================="
echo ""

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

log() { echo -e "${BLUE}[$(date +%H:%M:%S)]${NC} $1"; }
success() { echo -e "${GREEN}[$(date +%H:%M:%S)] ✓${NC} $1"; }
warn() { echo -e "${YELLOW}[$(date +%H:%M:%S)] !${NC} $1"; }
error() { echo -e "${RED}[$(date +%H:%M:%S)] ✗${NC} $1"; exit 1; }

cd "$(dirname "$0")"
PROJECT_DIR="$(pwd)"
START_TIME=$(date +%s)

log "Starting master build..."
log "Project directory: $PROJECT_DIR"
echo ""

# ============================================================================
# STEP 1: Install Rust
# ============================================================================
log "Step 1/10: Installing Rust..."
if [ ! -f "$HOME/.cargo/bin/cargo" ]; then
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y > /dev/null 2>&1
fi
export PATH="$HOME/.cargo/bin:$PATH"
success "Rust $(rustc --version | cut -d' ' -f2)"

# ============================================================================
# STEP 2: Add Android targets
# ============================================================================
log "Step 2/10: Adding Android targets..."
for target in aarch64-linux-android armv7-linux-androideabi x86_64-linux-android; do
    if ! rustup target list --installed 2>/dev/null | grep -q "$target"; then
        rustup target add "$target" > /dev/null 2>&1
    fi
done
success "Android targets installed"

# ============================================================================
# STEP 3: Install cargo-ndk
# ============================================================================
log "Step 3/10: Installing cargo-ndk..."
if [ ! -f "$HOME/.cargo/bin/cargo-ndk" ]; then
    cargo install cargo-ndk > /dev/null 2>&1
fi
success "cargo-ndk installed"

# ============================================================================
# STEP 4: Install Java 17
# ============================================================================
log "Step 4/10: Installing Java 17..."
export JAVA_HOME="$HOME/jdk-17"

if [ ! -f "$JAVA_HOME/bin/java" ]; then
    cd /tmp
    if [ ! -f "OpenJDK17U-jdk_x64_linux_hotspot_17.0.11_9.tar.gz" ]; then
        wget -q https://github.com/adoptium/temurin17-binaries/releases/download/jdk-17.0.11%2B9/OpenJDK17U-jdk_x64_linux_hotspot_17.0.11_9.tar.gz
    fi
    tar xzf OpenJDK17U-jdk_x64_linux_hotspot_17.0.11_9.tar.gz
    mv jdk-17.0.11+9 "$JAVA_HOME"
    cd "$PROJECT_DIR"
fi

# Fix permissions
chmod +x "$JAVA_HOME/bin/"* 2>/dev/null || true

# Verify Java works
if ! "$JAVA_HOME/bin/java" -version > /dev/null 2>&1; then
    warn "Java 17 installation incomplete, downloading again..."
    rm -rf "$JAVA_HOME"
    cd /tmp
    rm -f OpenJDK17U-jdk_x64_linux_hotspot_17.0.11_9.tar.gz
    wget -q https://github.com/adoptium/temurin17-binaries/releases/download/jdk-17.0.11%2B9/OpenJDK17U-jdk_x64_linux_hotspot_17.0.11_9.tar.gz
    tar xzf OpenJDK17U-jdk_x64_linux_hotspot_17.0.11_9.tar.gz
    mv jdk-17.0.11+9 "$JAVA_HOME"
    chmod +x "$JAVA_HOME/bin/"*
    cd "$PROJECT_DIR"
fi

export PATH="$JAVA_HOME/bin:$PATH"
success "Java 17 installed"

# ============================================================================
# STEP 5: Install Android SDK
# ============================================================================
log "Step 5/10: Installing Android SDK..."
export ANDROID_HOME="$HOME/android-sdk"

if [ ! -d "$ANDROID_HOME/cmdline-tools/latest/bin" ]; then
    mkdir -p "$ANDROID_HOME"
    cd "$ANDROID_HOME"
    
    if [ ! -f "commandlinetools-linux-11076708_latest.zip" ]; then
        wget -q https://dl.google.com/android/repository/commandlinetools-linux-11076708_latest.zip
    fi
    unzip -q -o commandlinetools-linux-11076708_latest.zip
    mkdir -p cmdline-tools/latest
    mv cmdline-tools/bin cmdline-tools/lib cmdline-tools/NOTICE.txt cmdline-tools/source.properties cmdline-tools/latest/ 2>/dev/null || true
    rm -f commandlinetools-linux-11076708_latest.zip
    cd "$PROJECT_DIR"
fi

export PATH="$ANDROID_HOME/cmdline-tools/latest/bin:$ANDROID_HOME/platform-tools:$PATH"
success "Android SDK command-line tools installed"

# ============================================================================
# STEP 6: Install SDK components
# ============================================================================
log "Step 6/10: Installing SDK components (NDK, build-tools, platforms)..."
log "  This may take several minutes..."

# Accept licenses
yes | sdkmanager --licenses > /dev/null 2>&1 || true

# Install components
sdkmanager "platform-tools" > /dev/null 2>&1
sdkmanager "platforms;android-33" > /dev/null 2>&1
sdkmanager "build-tools;33.0.2" > /dev/null 2>&1
sdkmanager "ndk;25.2.9519653" > /dev/null 2>&1

export NDK_HOME="$ANDROID_HOME/ndk/25.2.9519653"
success "SDK components installed"

# ============================================================================
# STEP 7: Clean and prepare build
# ============================================================================
log "Step 7/10: Preparing build environment..."
cd "$PROJECT_DIR"

# Clean previous builds
rm -rf target/android target/android-build
mkdir -p target/android

success "Build environment ready"

# ============================================================================
# STEP 8: Build native libraries
# ============================================================================
log "Step 8/10: Building native libraries..."
log "  Building for arm64-v8a..."
cargo ndk -t arm64-v8a -o target/android/lib/arm64-v8a build --release 2>&1 | grep -E "(Compiling riss_launcher|Finished)" || true

# Fix nested directory
if [ -d "target/android/lib/arm64-v8a/arm64-v8a" ]; then
    mv target/android/lib/arm64-v8a/arm64-v8a/*.so target/android/lib/arm64-v8a/ 2>/dev/null || true
    rmdir target/android/lib/arm64-v8a/arm64-v8a 2>/dev/null || true
fi

if [ ! -f "target/android/lib/arm64-v8a/libriss_launcher.so" ]; then
    error "Build failed for arm64-v8a"
fi
SIZE=$(du -h target/android/lib/arm64-v8a/libriss_launcher.so | cut -f1)
success "arm64-v8a: libriss_launcher.so ($SIZE)"

log "  Building for armeabi-v7a..."
cargo ndk -t armeabi-v7a -o target/android/lib/armeabi-v7a build --release 2>&1 | grep -E "(Compiling riss_launcher|Finished)" || true

if [ -d "target/android/lib/armeabi-v7a/armeabi-v7a" ]; then
    mv target/android/lib/armeabi-v7a/armeabi-v7a/*.so target/android/lib/armeabi-v7a/ 2>/dev/null || true
    rmdir target/android/lib/armeabi-v7a/armeabi-v7a 2>/dev/null || true
fi

if [ ! -f "target/android/lib/armeabi-v7a/libriss_launcher.so" ]; then
    error "Build failed for armeabi-v7a"
fi
SIZE=$(du -h target/android/lib/armeabi-v7a/libriss_launcher.so | cut -f1)
success "armeabi-v7a: libriss_launcher.so ($SIZE)"

log "  Building for x86_64..."
cargo ndk -t x86_64 -o target/android/lib/x86_64 build --release 2>&1 | grep -E "(Compiling riss_launcher|Finished)" || true

if [ -d "target/android/lib/x86_64/x86_64" ]; then
    mv target/android/lib/x86_64/x86_64/*.so target/android/lib/x86_64/ 2>/dev/null || true
    rmdir target/android/lib/x86_64/x86_64 2>/dev/null || true
fi

if [ ! -f "target/android/lib/x86_64/libriss_launcher.so" ]; then
    error "Build failed for x86_64"
fi
SIZE=$(du -h target/android/lib/x86_64/libriss_launcher.so | cut -f1)
success "x86_64: libriss_launcher.so ($SIZE)"

# ============================================================================
# STEP 9: Package APK
# ============================================================================
log "Step 9/10: Packaging APK..."
cd target
rm -rf apk-build
mkdir -p apk-build/lib
cd apk-build

# Copy manifest
cp "$PROJECT_DIR/android/AndroidManifest.xml" .

# Copy libraries into lib/ directory
cp -r ../android/lib/* lib/

# Package with aapt - create APK with manifest only
AAPT="$ANDROID_HOME/build-tools/33.0.2/aapt"
"$AAPT" package -f \
    -M AndroidManifest.xml \
    -I "$ANDROID_HOME/platforms/android-33/android.jar" \
    -F app-base.apk > /dev/null 2>&1

# Add lib directory to APK using zip
zip -r app-base.apk lib/ > /dev/null 2>&1
mv app-base.apk app-unsigned.apk

if [ ! -f "app-unsigned.apk" ]; then
    error "APK packaging failed"
fi
success "APK packaged"

# Align
ZIPALIGN="$ANDROID_HOME/build-tools/33.0.2/zipalign"
"$ZIPALIGN" -f 4 app-unsigned.apk app-aligned.apk
success "APK aligned"

# ============================================================================
# STEP 10: Sign APK
# ============================================================================
log "Step 10/10: Signing APK..."
KEYSTORE="$HOME/.android/debug.keystore"

if [ ! -f "$KEYSTORE" ]; then
    mkdir -p "$HOME/.android"
    "$JAVA_HOME/bin/keytool" -genkey -v \
        -keystore "$KEYSTORE" \
        -storepass android \
        -alias androiddebugkey \
        -keypass android \
        -keyalg RSA \
        -keysize 2048 \
        -validity 10000 \
        -dname "CN=Android Debug,O=Android,C=US" > /dev/null 2>&1
fi

APKSIGNER="$ANDROID_HOME/build-tools/33.0.2/apksigner"
"$APKSIGNER" sign \
    --ks "$KEYSTORE" \
    --ks-pass pass:android \
    --ks-key-alias androiddebugkey \
    --key-pass pass:android \
    --out "$PROJECT_DIR/riss-launcher-FINAL.apk" \
    app-aligned.apk > /dev/null 2>&1

success "APK signed"

# Verify
"$APKSIGNER" verify "$PROJECT_DIR/riss-launcher-FINAL.apk" > /dev/null 2>&1 || error "APK verification failed"
success "APK verified"

# ============================================================================
# DONE!
# ============================================================================
END_TIME=$(date +%s)
DURATION=$((END_TIME - START_TIME))
MINUTES=$((DURATION / 60))
SECONDS=$((DURATION % 60))

cd "$PROJECT_DIR"

echo ""
echo "=========================================="
echo -e "${GREEN}✓ BUILD COMPLETE!${NC}"
echo "=========================================="
echo ""
echo "APK Location: $PROJECT_DIR/riss-launcher-FINAL.apk"
echo "APK Size:     $(du -h riss-launcher-FINAL.apk | cut -f1)"
echo "Build Time:   ${MINUTES}m ${SECONDS}s"
echo ""
echo "APK Contents:"
unzip -l riss-launcher-FINAL.apk | grep -E "(AndroidManifest|\.so)" | awk '{print "  " $4 " (" $1 " bytes)"}'
echo ""
echo "To install:"
echo "  adb install riss-launcher-FINAL.apk"
echo ""
echo "To view logs:"
echo "  adb logcat | grep RissLauncher"
echo ""
echo "=========================================="
