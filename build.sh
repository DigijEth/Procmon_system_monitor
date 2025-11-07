#!/bin/bash

set -e

echo "================================"
echo "  Process Monitor Build Script  "
echo "================================"
echo

# Check if cargo is installed
if ! command -v cargo &> /dev/null; then
    echo "Error: Cargo is not installed."
    echo "Please install Rust from https://rustup.rs/"
    echo
    echo "Run: curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
    exit 1
fi

echo "Rust version:"
rustc --version
cargo --version
echo

# Build all workspace members
echo "Building all workspace members..."
echo

echo "[1/3] Building procmon-core..."
cargo build --release -p procmon-core

echo
echo "[2/3] Building procmon-tui..."
cargo build --release -p procmon-tui

echo
echo "[3/3] Building procmon-gui..."
cargo build --release -p procmon-gui

echo
echo "================================"
echo "  Build Completed Successfully! "
echo "================================"
echo
echo "Binaries are located in ./target/release/"
echo
echo "To run the Terminal UI:"
echo "  ./target/release/procmon-tui"
echo
echo "To run the Graphical UI:"
echo "  ./target/release/procmon-gui"
echo
echo "For more information, see README.md and QUICKSTART.md"
