#!/usr/bin/env bash
set -euo pipefail
cargo build --release
mkdir -p bin
cp target/release/orchid bin/
