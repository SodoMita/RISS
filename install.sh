#!/bin/bash
# RISS Launcher Install Script
# Installs the RISS Launcher binary and desktop file

set -e

INSTALL_DIR="${HOME}/.local/bin"
APP_DIR="${HOME}/.local/share/applications"
BINARY="target/release/riss_launcher"
DESKTOP="riss-launcher.desktop"

echo "⚡ RISS Launcher Installer"
echo "========================="

# Build if needed
if [ ! -f "$BINARY" ]; then
    echo "📦 Building release version..."
    cargo build --release
fi

# Create directories
mkdir -p "$INSTALL_DIR"
mkdir -p "$APP_DIR"

# Copy binary
echo "📁 Installing binary to $INSTALL_DIR..."
cp "$BINARY" "$INSTALL_DIR/riss_launcher"
chmod +x "$INSTALL_DIR/riss_launcher"

# Copy desktop file
echo "📁 Installing desktop entry..."
cp "$DESKTOP" "$APP_DIR/"

# Update desktop database
if command -v update-desktop-database &> /dev/null; then
    update-desktop-database "$APP_DIR" 2>/dev/null || true
fi

# Add to PATH if needed
if [[ ":$PATH:" != *":$INSTALL_DIR:"* ]]; then
    echo ""
    echo "⚠️  $INSTALL_DIR is not in your PATH."
    echo "   Add this to your ~/.bashrc or ~/.zshrc:"
    echo "   export PATH=\"\$PATH:$INSTALL_DIR\""
fi

echo ""
echo "✅ RISS Launcher installed successfully!"
echo ""
echo "You can now:"
echo "  • Run 'riss_launcher' from terminal"
echo "  • Find 'RISS Launcher' in your application menu"
echo "  • Set it as your default launcher (if your DE supports it)"
echo ""
echo "To uninstall:"
echo "  rm $INSTALL_DIR/riss_launcher"
echo "  rm $APP_DIR/riss-launcher.desktop"
