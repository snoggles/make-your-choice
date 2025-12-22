#!/usr/bin/env zsh

set -e

echo "Building Make Your Choice for Linux…"
echo ""

# Check if Rust is installed
if ! command -v cargo &> /dev/null; then
    echo "❌ Error: Rust is not installed."
    echo "Please install Rust from https://rustup.rs/"
    exit 1
fi

# Check if GTK4 development libraries are installed
if ! pkg-config --exists gtk4 2>/dev/null; then
    echo "⚠️  Warning: GTK4 development libraries not found."
    echo "Please install them:"
    echo "  - Arch: sudo pacman -S gtk4 base-devel"
    echo "  - Debian: sudo apt install libgtk-4-dev build-essential"
    echo "  - Fedora: sudo dnf install gtk4-devel gcc"
    echo ""
    read "response?Continue anyway? (y/N) "
    if [[ ! "$response" =~ ^[Yy]$ ]]; then
        exit 1
    fi
fi

# Build in release mode
echo "📦 Building in release mode…"
cargo build --release

if [ $? -eq 0 ]; then
    echo ""
    echo "✅ Build successful!"
    echo ""
    echo "Binary location:"
    echo "   ./target/release/make-your-choice"
    echo ""
    echo "To run the program:"
    echo "   ./target/release/make-your-choice"
    echo ""
    echo "Note: Do NOT run the program using sudo, or as root."
else
    echo ""
    echo "❌ Build failed!"
    exit 1
fi
