#!/usr/bin/env bash

set -euo pipefail

python3 .agents/skills/create-release/scripts/test_preflight.py -v
cargo fmt --check
cargo test --all-features
cargo clippy --all-targets --all-features -- -D warnings
git diff HEAD --check
