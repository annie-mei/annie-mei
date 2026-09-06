#!/usr/bin/env bash

set -euo pipefail

cargo fmt --check
cargo test --all-features
cargo clippy --all-targets --all-features -- -D warnings
git diff --check
