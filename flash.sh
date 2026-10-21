#!/bin/sh
# Build the firmware and flash it to the Pico.
set -e
cd "$(dirname "$0")/firmware"
cargo build --release
../bootsel.sh
cargo run --release
