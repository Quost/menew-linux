#!/usr/bin/env bash
# Run a command inside the Linux dev image (build it first, see Dockerfile), from macOS or Windows.
# bash scripts/dev/run.sh cargo test --workspace
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
exec docker run --rm -i -v "$ROOT:/src" -v menew-target:/target -v menew-cargo:/root/.cargo/registry \
  -e CARGO_TERM_COLOR=never menew-dev bash -lc "$*"
