#!/bin/bash
# Quick start script for KISS Launcher

set -e

echo "⚡ KISS Launcher - Quick Start"
echo "=============================="
echo ""

# Check if binary exists
if [ -f "target/release/kiss_launcher" ]; then
    echo "✅ Release build found"
    BINARY="target/release/kiss_launcher"
elif [ -f "target/debug/kiss_launcher" ]; then
    echo "⚠️  Debug build found (consider running 'cargo build --release' for better performance)"
    BINARY="target/debug/kiss_launcher"
else
    echo "📦 Building project..."
    cargo build --release
    BINARY="target/release/kiss_launcher"
fi

echo ""
echo "🚀 Launching KISS Launcher..."
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
