#!/usr/bin/env bash
set -euo pipefail

BINARIES=(sysbench sysinfo)
INSTALL_DIR="/usr/local/bin"

# ── colours ────────────────────────────────────────────────────────────────
RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; NC='\033[0m'
info()  { echo -e "${GREEN}==>${NC} $*"; }
warn()  { echo -e "${YELLOW}warn:${NC} $*"; }
die()   { echo -e "${RED}error:${NC} $*" >&2; exit 1; }

# ── ensure C toolchain is available ───────────────────────────────────────
ensure_cc() {
    # Try to actually compile something — presence in PATH is not enough
    if echo 'int main(){}' | cc -x c - -o /tmp/_cc_test 2>/dev/null; then
        rm -f /tmp/_cc_test
        return
    fi
    warn "No working C toolchain found — installing build tools..."
    if command -v apt-get >/dev/null 2>&1; then
        apt-get install -y build-essential
    elif command -v dnf >/dev/null 2>&1; then
        dnf install -y gcc
    elif command -v yum >/dev/null 2>&1; then
        yum install -y gcc
    elif command -v pacman >/dev/null 2>&1; then
        pacman -Sy --noconfirm base-devel
    elif command -v apk >/dev/null 2>&1; then
        apk add --no-cache build-base
    else
        die "Could not install a C toolchain automatically. Install gcc/build-essential and re-run."
    fi
    echo 'int main(){}' | cc -x c - -o /tmp/_cc_test 2>/dev/null \
        || die "C toolchain still not working after install attempt."
    rm -f /tmp/_cc_test
    info "C toolchain ready."
}

# ── ensure Rust is available ───────────────────────────────────────────────
ensure_rust() {
    if command -v cargo >/dev/null 2>&1; then
        info "Rust $(rustc --version) found."
        return
    fi

    warn "Rust not found."
    # Read from /dev/tty so curl-pipe-bash doesn't consume the script stream
    read -r -p "Install Rust via rustup now? [Y/n] " answer </dev/tty || true
    case "${answer,,}" in
        n|no) die "Rust is required. Install from https://rustup.rs and re-run." ;;
    esac

    info "Installing Rust via rustup..."
    command -v curl >/dev/null 2>&1 || die "curl is required to install rustup."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --no-modify-path

    # shellcheck source=/dev/null
    source "$HOME/.cargo/env"

    command -v cargo >/dev/null 2>&1 || die "rustup install finished but cargo still not found."
    info "Rust installed: $(rustc --version)"
}

ensure_cc
ensure_rust

# ── locate source (clone if running via curl) ──────────────────────────────
CLONED_DIR=""
CRATE_DIR=""

_self="${BASH_SOURCE[0]:-}"
if [ -n "$_self" ]; then
    SCRIPT_DIR="$(cd "$(dirname "$_self")" && pwd)"
    CRATE_DIR="$SCRIPT_DIR/tester"
fi

if [ ! -f "${CRATE_DIR}/Cargo.toml" ]; then
    command -v git >/dev/null 2>&1 || die "git is required. Install git and re-run."
    CLONED_DIR="$(mktemp -d)"
    info "Cloning repository into $CLONED_DIR ..."
    git clone --depth 1 https://github.com/zsigisti/sysbench.git "$CLONED_DIR"
    CRATE_DIR="$CLONED_DIR/tester"
fi

cleanup() { [ -n "$CLONED_DIR" ] && rm -rf "$CLONED_DIR"; }
trap cleanup EXIT

# ── build ──────────────────────────────────────────────────────────────────
info "Building ${BINARIES[*]} (release + native CPU optimisations)..."
RUSTFLAGS="-C target-cpu=native" \
    cargo build --release --manifest-path "$CRATE_DIR/Cargo.toml"

for b in "${BINARIES[@]}"; do
    [ -f "$CRATE_DIR/target/release/$b" ] \
        || die "Build succeeded but binary not found at $CRATE_DIR/target/release/$b"
done

# ── install ────────────────────────────────────────────────────────────────
# Prefer ~/.local/bin when not root (no sudo needed)
if [ "$EUID" -ne 0 ] && [ "$INSTALL_DIR" = "/usr/local/bin" ]; then
    LOCAL_BIN="$HOME/.local/bin"
    mkdir -p "$LOCAL_BIN"
    INSTALL_DIR="$LOCAL_BIN"
    warn "Not root — installing to $INSTALL_DIR instead of /usr/local/bin"
    warn "Make sure $INSTALL_DIR is in your PATH."
fi

for b in "${BINARIES[@]}"; do
    info "Installing $b -> $INSTALL_DIR/$b"
    install -m 755 "$CRATE_DIR/target/release/$b" "$INSTALL_DIR/$b"
done

# ── PATH hint ──────────────────────────────────────────────────────────────
if ! echo ":$PATH:" | grep -q ":$INSTALL_DIR:"; then
    warn "$INSTALL_DIR is not in your PATH."
    warn "Add this to your shell rc file:"
    warn "  export PATH=\"$INSTALL_DIR:\$PATH\""
fi

info "Done. Run: ${BINARIES[0]}  (or: ${BINARIES[1]})"
