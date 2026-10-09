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
echo "  • Type to search for apps"
echo "  • Use ↑↓ to navigate results"
echo "  • Press Enter to launch selected app"
echo "  • Click ★ to add/remove favorites"
echo "  • Try typing '2+2' for calculator"
echo "  • Press Esc to clear search"
echo ""

# Launch the app
exec "$BINARY"
