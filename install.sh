#!/bin/sh
# Luno installer — builds ln from source and installs to ~/.luno/bin
set -e

RED='\033[0;31m'
GREEN='\033[0;32m'
CYAN='\033[0;36m'
NC='\033[0m'

info()  { printf "${CYAN}%s${NC}\n" "$*"; }
ok()    { printf "${GREEN}%s${NC}\n" "$*"; }
err()   { printf "${RED}%s${NC}\n" "$*"; }

# Check requirements
command -v rustc >/dev/null 2>&1 || { err "error: Rust is not installed"; err "  install it from https://rustup.rs"; exit 1; }
command -v cargo >/dev/null 2>&1 || { err "error: Cargo is not installed"; err "  install it from https://rustup.rs"; exit 1; }
command -v gcc >/dev/null 2>&1 || command -v clang >/dev/null 2>&1 || {
    err "error: no C compiler found (install gcc or clang)"; exit 1; }

# Get the directory where this script lives
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

info "building ln..."
cargo build --release --manifest-path "$SCRIPT_DIR/Cargo.toml"

INSTALL_DIR="$HOME/.luno/bin"
mkdir -p "$INSTALL_DIR"
cp "$SCRIPT_DIR/target/release/ln" "$INSTALL_DIR/ln"

ok "installed ln to $INSTALL_DIR/ln"

# Add to PATH in shell config files
PATH_LINE='export PATH="$HOME/.luno/bin:$PATH"'
UPDATED=""

for rc in ".bashrc" ".zshrc" ".profile" ".bash_profile" ".zshenv"; do
    rc_path="$HOME/$rc"
    if [ -f "$rc_path" ]; then
        if grep -q '\.luno/bin' "$rc_path" 2>/dev/null; then
            continue
        fi
        printf "\n# Luno\n%s\n" "$PATH_LINE" >> "$rc_path"
        UPDATED="$UPDATED $rc"
    fi
done

# Fall back to .profile if nothing else was updated
if [ -z "$UPDATED" ]; then
    profile="$HOME/.profile"
    if [ ! -f "$profile" ] || ! grep -q '\.luno/bin' "$profile" 2>/dev/null; then
        printf "\n# Luno\n%s\n" "$PATH_LINE" >> "$profile"
        UPDATED=".profile"
    fi
fi

if [ -n "$UPDATED" ]; then
    info "added to PATH in:$UPDATED"
fi

echo ""
ok "Luno is ready!"
echo ""
echo "  Restart your shell, or run:"
echo "    export PATH=\"\$HOME/.luno/bin:\$PATH\""
echo ""
echo "  Then verify with:"
echo "    ln version"
