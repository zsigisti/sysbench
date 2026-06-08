#!/usr/bin/env bash
set -euo pipefail

# CRUCIBLE installer — builds natively on THIS host.
#
#   ./install.sh            # install the CLI  (crux + sysinfo alias)   [default]
#   ./install.sh --gui      # install the GUI  (crux-gui + .desktop)    only
#   ./install.sh --all      # install both
#
# Over curl:  curl -sSf <url>/install.sh | bash            (CLI)
#             curl -sSf <url>/install.sh | bash -s -- --gui (GUI)

BIN="crux"
ALIAS="sysinfo"
GUI_BIN="crux-gui"
INSTALL_DIR="/usr/local/bin"

WANT_CLI=1
WANT_GUI=0
case "${1:-}" in
    ""|--cli)        WANT_CLI=1; WANT_GUI=0 ;;
    --gui|--gui-only) WANT_CLI=0; WANT_GUI=1 ;;
    --all)           WANT_CLI=1; WANT_GUI=1 ;;
    -h|--help)
        cat <<'USAGE'
CRUCIBLE installer — builds natively on THIS host.

  ./install.sh            install the CLI  (crux + sysinfo alias)   [default]
  ./install.sh --gui      install the GUI  (crux-gui + .desktop)    only
  ./install.sh --all      install both

Over curl:
  curl -sSf <url>/install.sh | bash             # CLI
  curl -sSf <url>/install.sh | bash -s -- --gui # GUI
USAGE
        exit 0 ;;
    *) echo "unknown option: $1 (try --cli | --gui | --all)" >&2; exit 1 ;;
esac

# ── colours ────────────────────────────────────────────────────────────────
RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; NC='\033[0m'
info()  { echo -e "${GREEN}==>${NC} $*"; }
warn()  { echo -e "${YELLOW}warn:${NC} $*"; }
die()   { echo -e "${RED}error:${NC} $*" >&2; exit 1; }

# ── ensure a C toolchain is available ──────────────────────────────────────
ensure_cc() {
    if echo 'int main(){}' | cc -x c - -o /tmp/_cc_test 2>/dev/null; then
        rm -f /tmp/_cc_test; return
    fi
    warn "No working C toolchain found — installing build tools..."
    if   command -v apt-get >/dev/null 2>&1; then apt-get install -y build-essential
    elif command -v dnf     >/dev/null 2>&1; then dnf install -y gcc gcc-c++
    elif command -v yum     >/dev/null 2>&1; then yum install -y gcc gcc-c++
    elif command -v pacman  >/dev/null 2>&1; then pacman -Sy --noconfirm base-devel
    elif command -v apk     >/dev/null 2>&1; then apk add --no-cache build-base
    else die "Install gcc/build-essential and re-run."; fi
    echo 'int main(){}' | cc -x c - -o /tmp/_cc_test 2>/dev/null \
        || die "C toolchain still not working after install attempt."
    rm -f /tmp/_cc_test; info "C toolchain ready."
}

# ── ensure Rust is available ───────────────────────────────────────────────
ensure_rust() {
    if command -v cargo >/dev/null 2>&1; then
        info "Rust $(rustc --version) found."; return
    fi
    warn "Rust not found."
    read -r -p "Install Rust via rustup now? [Y/n] " answer </dev/tty || true
    case "${answer,,}" in n|no) die "Rust is required: https://rustup.rs";; esac
    command -v curl >/dev/null 2>&1 || die "curl is required to install rustup."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --no-modify-path
    # shellcheck source=/dev/null
    source "$HOME/.cargo/env"
    command -v cargo >/dev/null 2>&1 || die "cargo still not found after rustup."
    info "Rust installed: $(rustc --version)"
}

# ── ensure Qt 6 (GUI only) ─────────────────────────────────────────────────
ensure_qt() {
    if command -v qmake6 >/dev/null 2>&1 || command -v qmake >/dev/null 2>&1; then
        info "Qt $( (qmake6 -query QT_VERSION 2>/dev/null || qmake -query QT_VERSION) ) found."
        return
    fi
    warn "Qt 6 not found — installing Qt 6 (Quick/QML) dev packages..."
    if   command -v pacman  >/dev/null 2>&1; then pacman -Sy --noconfirm qt6-base qt6-declarative
    elif command -v apt-get >/dev/null 2>&1; then apt-get install -y qt6-base-dev qt6-declarative-dev
    elif command -v dnf     >/dev/null 2>&1; then dnf install -y qt6-qtbase-devel qt6-qtdeclarative-devel
    else die "Install Qt 6 (qt6-base + qt6-declarative) and re-run."; fi
    command -v qmake6 >/dev/null 2>&1 || command -v qmake >/dev/null 2>&1 \
        || die "Qt 6 still not found after install attempt."
}

ensure_cc
ensure_rust
[ "$WANT_GUI" -eq 1 ] && ensure_qt

# ── locate source (clone if running via curl) ──────────────────────────────
CLONED_DIR=""; CRATE_DIR=""
_self="${BASH_SOURCE[0]:-}"
if [ -n "$_self" ]; then
    CRATE_DIR="$(cd "$(dirname "$_self")" && pwd)"
fi
if [ ! -f "${CRATE_DIR}/Cargo.toml" ]; then
    command -v git >/dev/null 2>&1 || die "git is required."
    CLONED_DIR="$(mktemp -d)"
    info "Cloning repository into $CLONED_DIR ..."
    git clone --depth 1 https://github.com/zsigisti/crucible.git "$CLONED_DIR"
    CRATE_DIR="$CLONED_DIR"
fi
cleanup() { [ -n "$CLONED_DIR" ] && rm -rf "$CLONED_DIR"; }
trap cleanup EXIT

# ── resolve install dir (prefer ~/.local when not root) ────────────────────
if [ "$EUID" -ne 0 ] && [ "$INSTALL_DIR" = "/usr/local/bin" ]; then
    INSTALL_DIR="$HOME/.local/bin"; mkdir -p "$INSTALL_DIR"
    warn "Not root — installing to $INSTALL_DIR"
fi

# ── CLI install ────────────────────────────────────────────────────────────
install_cli() {
    info "Building $BIN (release + -C target-cpu=native)..."
    RUSTFLAGS="-C target-cpu=native" \
        cargo build --release --bin "$BIN" --manifest-path "$CRATE_DIR/Cargo.toml"
    local p="$CRATE_DIR/target/release/$BIN"
    [ -f "$p" ] || die "binary not found at $p"
    info "Installing $BIN -> $INSTALL_DIR/$BIN"
    install -m 755 "$p" "$INSTALL_DIR/$BIN"
    info "Linking $ALIAS -> $BIN"
    ln -sf "$BIN" "$INSTALL_DIR/$ALIAS"

    # man page + shell completions
    local CRUX="$INSTALL_DIR/$BIN" MAN BASH ZSH FISH
    if [ "$EUID" -eq 0 ]; then
        MAN="/usr/local/share/man/man1"; BASH="/usr/share/bash-completion/completions"
        ZSH="/usr/share/zsh/site-functions"; FISH="/usr/share/fish/vendor_completions.d"
    else
        MAN="$HOME/.local/share/man/man1"; BASH="$HOME/.local/share/bash-completion/completions"
        ZSH="$HOME/.local/share/zsh/site-functions"; FISH="$HOME/.config/fish/completions"
    fi
    gen() { local d="$1" f="$2"; shift 2; mkdir -p "$d" 2>/dev/null || return 0
        "$CRUX" "$@" > "$d/$f" 2>/dev/null && info "Installed $d/$f" || true; }
    gen "$MAN" "crux.1" man
    gen "$BASH" "crux" completions bash
    gen "$ZSH" "_crux" completions zsh
    gen "$FISH" "crux.fish" completions fish
}

# ── GUI install ────────────────────────────────────────────────────────────
install_gui() {
    info "Building $GUI_BIN (release + -C target-cpu=native, needs Qt 6)..."
    RUSTFLAGS="-C target-cpu=native" \
        cargo build --release -p crucible-gui --manifest-path "$CRATE_DIR/Cargo.toml"
    local p="$CRATE_DIR/target/release/$GUI_BIN"
    [ -f "$p" ] || die "binary not found at $p"
    info "Installing $GUI_BIN -> $INSTALL_DIR/$GUI_BIN"
    install -m 755 "$p" "$INSTALL_DIR/$GUI_BIN"

    # desktop entry + icon
    local APPS ICONS
    if [ "$EUID" -eq 0 ]; then
        APPS="/usr/share/applications"; ICONS="/usr/share/icons/hicolor/scalable/apps"
    else
        APPS="$HOME/.local/share/applications"; ICONS="$HOME/.local/share/icons/hicolor/scalable/apps"
    fi
    mkdir -p "$APPS" "$ICONS"
    install -m 644 "$CRATE_DIR/packaging/crux-gui.desktop" "$APPS/crux-gui.desktop"
    install -m 644 "$CRATE_DIR/assets/logo.svg" "$ICONS/crucible.svg"
    command -v update-desktop-database >/dev/null 2>&1 && update-desktop-database "$APPS" 2>/dev/null || true
    info "Installed desktop entry + icon."
}

[ "$WANT_CLI" -eq 1 ] && install_cli
[ "$WANT_GUI" -eq 1 ] && install_gui

# ── PATH hint ──────────────────────────────────────────────────────────────
if ! echo ":$PATH:" | grep -q ":$INSTALL_DIR:"; then
    warn "$INSTALL_DIR is not in your PATH — add: export PATH=\"$INSTALL_DIR:\$PATH\""
fi

echo
[ "$WANT_CLI" -eq 1 ] && { info "CLI ready:  $BIN   ·   $BIN info   ·   $ALIAS"; }
[ "$WANT_GUI" -eq 1 ] && { info "GUI ready:  $GUI_BIN   (also in your application menu)"; }
