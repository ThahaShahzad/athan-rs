#!/bin/sh
# athan-rs installer — downloads the latest release binary to ~/.local/bin/athan.
# Usage: curl -fsSL https://raw.githubusercontent.com/ThahaShahzad/athan-rs/main/install.sh | sh
set -eu

REPO="ThahaShahzad/athan-rs"
INSTALL_DIR="$HOME/.local/bin"

main() {
    os="$(uname -s)"
    arch="$(uname -m)"

    case "$os" in
        Linux) os="linux" ;;
        Darwin) os="macos" ;;
        *)
            echo "error: unsupported OS: $os (this installer supports Linux and macOS)" >&2
            exit 1
            ;;
    esac

    case "$arch" in
        x86_64 | amd64) arch="x86_64" ;;
        aarch64 | arm64) arch="aarch64" ;;
        *)
            echo "error: unsupported architecture: $arch" >&2
            exit 1
            ;;
    esac

    url="https://github.com/$REPO/releases/latest/download/athan-$os-$arch"
    tmp="$INSTALL_DIR/athan.tmp"

    echo "Downloading athan ($os/$arch) from:"
    echo "  $url"
    mkdir -p "$INSTALL_DIR"

    if command -v curl >/dev/null 2>&1; then
        curl -fsSL "$url" -o "$tmp"
    elif command -v wget >/dev/null 2>&1; then
        wget -qO "$tmp" "$url"
    else
        echo "error: neither curl nor wget found" >&2
        exit 1
    fi

    chmod +x "$tmp"
    mv -f "$tmp" "$INSTALL_DIR/athan"

    echo ""
    echo "Installed athan to $INSTALL_DIR/athan"
    case ":$PATH:" in
        *":$INSTALL_DIR:"*) ;;
        *)
            echo "NOTE: $INSTALL_DIR is not in your PATH — add it, e.g.:"
            echo "  export PATH=\"$INSTALL_DIR:\$PATH\""
            ;;
    esac
    echo ""
    echo "Next steps:"
    echo "  athan                    # configure location & open the dashboard"
    echo "  athan autostart enable   # play athan at prayer times, starting on login"
}

main "$@"
