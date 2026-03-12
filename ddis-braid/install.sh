#!/bin/sh
# Braid installer — download pre-built binary or build from source.
#
# Usage:
#   curl -sSL https://raw.githubusercontent.com/wvandaal/ddis/main/ddis-braid/install.sh | sh
#   curl -sSL https://raw.githubusercontent.com/wvandaal/ddis/main/ddis-braid/install.sh | sh -s -- --from-source
#
# Options:
#   --from-source    Build from source (requires Rust toolchain)
#   --prefix DIR     Install to DIR/bin/ (default: ~/.local)
#
# Traces to: WP8 (Install Script + Fallback)

set -eu

REPO="wvandaal/ddis"
INSTALL_PREFIX="${HOME}/.local"
FROM_SOURCE=0

# Parse arguments
while [ $# -gt 0 ]; do
    case "$1" in
        --from-source) FROM_SOURCE=1; shift ;;
        --prefix) INSTALL_PREFIX="$2"; shift 2 ;;
        *) echo "Unknown option: $1"; exit 1 ;;
    esac
done

BIN_DIR="${INSTALL_PREFIX}/bin"

# Detect OS and architecture
detect_platform() {
    OS="$(uname -s)"
    ARCH="$(uname -m)"

    case "$OS" in
        Linux)  OS_NAME="linux" ;;
        Darwin) OS_NAME="macos" ;;
        *)      echo "Unsupported OS: $OS"; exit 1 ;;
    esac

    case "$ARCH" in
        x86_64|amd64)  ARCH_NAME="x86_64" ;;
        aarch64|arm64) ARCH_NAME="aarch64" ;;
        *)             echo "Unsupported architecture: $ARCH"; exit 1 ;;
    esac

    PLATFORM="${OS_NAME}-${ARCH_NAME}"
    echo "Detected platform: ${PLATFORM}"
}

# Install from source using cargo
install_from_source() {
    if ! command -v cargo >/dev/null 2>&1; then
        echo "Error: Rust toolchain not found. Install from https://rustup.rs/"
        exit 1
    fi

    echo "Building braid from source..."

    # Check if we're in the repo
    if [ -f "crates/braid/Cargo.toml" ]; then
        cargo install --path crates/braid --root "${INSTALL_PREFIX}"
    else
        # Clone and build
        TMPDIR="$(mktemp -d)"
        trap 'rm -rf "$TMPDIR"' EXIT

        echo "Cloning repository..."
        git clone --depth 1 "https://github.com/${REPO}.git" "$TMPDIR/ddis"
        cd "$TMPDIR/ddis/ddis-braid"
        cargo install --path crates/braid --root "${INSTALL_PREFIX}"
    fi
}

# Install pre-built binary from GitHub Releases
install_binary() {
    detect_platform

    # Get latest release tag
    LATEST=$(curl -sSL "https://api.github.com/repos/${REPO}/releases/latest" \
        | grep '"tag_name"' | head -1 | cut -d'"' -f4)

    if [ -z "$LATEST" ]; then
        echo "No releases found. Falling back to source install."
        install_from_source
        return
    fi

    ASSET_NAME="braid-${LATEST}-${PLATFORM}.tar.gz"
    DOWNLOAD_URL="https://github.com/${REPO}/releases/download/${LATEST}/${ASSET_NAME}"

    echo "Downloading braid ${LATEST} for ${PLATFORM}..."

    TMPDIR="$(mktemp -d)"
    trap 'rm -rf "$TMPDIR"' EXIT

    if ! curl -sSL -o "$TMPDIR/$ASSET_NAME" "$DOWNLOAD_URL" 2>/dev/null; then
        echo "Binary not available for ${PLATFORM}. Falling back to source install."
        install_from_source
        return
    fi

    mkdir -p "$BIN_DIR"
    tar -xzf "$TMPDIR/$ASSET_NAME" -C "$TMPDIR"

    if [ -f "$TMPDIR/braid" ]; then
        cp "$TMPDIR/braid" "$BIN_DIR/braid"
        chmod +x "$BIN_DIR/braid"
    else
        echo "Archive does not contain braid binary. Falling back to source install."
        install_from_source
        return
    fi
}

# Ensure BIN_DIR is in PATH
ensure_path() {
    case ":$PATH:" in
        *":${BIN_DIR}:"*) ;; # Already in PATH
        *)
            echo ""
            echo "Add to your shell profile:"
            echo "  export PATH=\"${BIN_DIR}:\$PATH\""
            ;;
    esac
}

# Main
echo "Installing braid..."
echo ""

mkdir -p "$BIN_DIR"

if [ "$FROM_SOURCE" = 1 ]; then
    install_from_source
else
    install_binary
fi

# Verify installation
if command -v braid >/dev/null 2>&1 || [ -x "${BIN_DIR}/braid" ]; then
    echo ""
    echo "Braid installed successfully!"
    echo "  Binary: ${BIN_DIR}/braid"
    echo ""
    echo "Quick start:"
    echo "  cd your-project"
    echo "  braid init              # detect environment, create store"
    echo "  braid status            # see where you are"
    echo ""
    echo "Tell your AI agent: 'Use braid'"
    ensure_path
else
    echo ""
    echo "Installation failed."
    exit 1
fi
