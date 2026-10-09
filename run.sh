#!/bin/bash
# Quick start script for RISS Launcher

set -e

echo "⚡ RISS Launcher - Quick Start"
echo "=============================="
echo ""

# Check if binary exists
if [ -f "target/release/riss_launcher" ]; then
    echo "✅ Release build found"
    BINARY="target/release/riss_launcher"
elif [ -f "target/debug/riss_launcher" ]; then
    echo "⚠️  Debug build found (consider running 'cargo build --release' for better performance)"
    BINARY="target/debug/riss_launcher"
else
    echo "📦 Building project..."
    cargo build --release
    BINARY="target/release/riss_launcher"
fi

echo ""
echo "🚀 Launching RISS Launcher..."
echo ""
echo "Tips:"
echo "  • Type to search apps, settings, the web… (2+2 is a calculator)"
echo "  • ↑↓ to navigate, Enter to launch, Esc to go back"
echo "  • Tap or click the star of a row to favorite it"
echo "  • Long press (or right click) a row for the context menu"
echo "  • The ⚙ button opens the KISS style settings screen"
echo ""

# Launch the app
exec "$BINARY"
