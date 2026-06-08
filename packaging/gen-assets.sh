#!/usr/bin/env bash
# Generate the man page and shell completions into target/assets/ so the deb/rpm
# packagers (which reference files by path) can pick them up. Run this BEFORE
# `cargo deb` / `cargo generate-rpm`.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

# Build the binary if it isn't there yet.
BIN="target/release/crux"
[ -x "$BIN" ] || cargo build --release --bin crux

OUT="target/assets"
mkdir -p "$OUT/completions"

"$BIN" man                         > "$OUT/crux.1"
"$BIN" completions bash            > "$OUT/completions/crux.bash"
"$BIN" completions zsh             > "$OUT/completions/_crux"
"$BIN" completions fish            > "$OUT/completions/crux.fish"

echo "Generated:"
echo "  $OUT/crux.1"
echo "  $OUT/completions/{crux.bash,_crux,crux.fish}"
