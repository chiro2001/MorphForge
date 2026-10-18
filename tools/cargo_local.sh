#!/bin/sh
set -eu
export CARGO_HOME="${MORPHFORGE_CARGO_HOME:-/home/chiro/tmp/morphforge/cargo}"
export RUSTUP_HOME="${MORPHFORGE_RUSTUP_HOME:-/home/chiro/tmp/morphforge/rustup}"
export PATH="$CARGO_HOME/bin:$PATH"
exec cargo "$@"
