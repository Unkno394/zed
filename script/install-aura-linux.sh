#!/usr/bin/env sh
set -eu

# Downloads a Zed Aura release tarball from GitHub and unpacks it into
# ~/.local/. Linux only.
#
# Usage:
#   sh script/install-aura-linux.sh
#
# Env vars:
#   ZED_AURA_VERSION   Release tag to install, e.g. "v0.2.0-aura" (default: latest)
#   ZED_BUNDLE_PATH    Path to a local tarball to install instead of downloading

REPO="Unkno394/zed"

main() {
    platform="$(uname -s)"
    arch="$(uname -m)"
    version="${ZED_AURA_VERSION:-latest}"

    if [ "$platform" != "Linux" ]; then
        echo "This script only supports Linux. Unsupported platform: $platform"
        exit 1
    fi

    case "$arch" in
        aarch64 | arm64)
            arch="aarch64"
            ;;
        x86_64 | amd64)
            arch="x86_64"
            ;;
        *)
            echo "Unsupported architecture: $arch"
            exit 1
            ;;
    esac

    if [ -n "${TMPDIR:-}" ] && [ -d "${TMPDIR}" ]; then
        temp="$(mktemp -d "$TMPDIR/zed-aura-XXXXXX")"
    else
        temp="$(mktemp -d "/tmp/zed-aura-XXXXXX")"
    fi

    if command -v curl >/dev/null 2>&1; then
        curl () {
            command curl -fL "$@"
        }
    elif command -v wget >/dev/null 2>&1; then
        curl () {
            wget -O- "$@"
        }
    else
        echo "Could not find 'curl' or 'wget' in your path"
        exit 1
    fi

    asset="zed-linux-${arch}.tar.gz"
    archive="$temp/$asset"

    if [ -n "${ZED_BUNDLE_PATH:-}" ]; then
        cp "$ZED_BUNDLE_PATH" "$archive"
    elif [ "$version" = "latest" ]; then
        echo "Downloading Zed Aura (latest release) for linux-$arch"
        curl "https://github.com/$REPO/releases/latest/download/$asset" > "$archive"
    else
        echo "Downloading Zed Aura $version for linux-$arch"
        curl "https://github.com/$REPO/releases/download/$version/$asset" > "$archive"
    fi

    # Unpack. The archive contains a single top-level directory (e.g. zed-dev.app/).
    install_dir="$HOME/.local/zed-aura.app"
    rm -rf "$install_dir"
    mkdir -p "$install_dir"
    tar -xzf "$archive" --strip-components=1 -C "$install_dir"

    mkdir -p "$HOME/.local/bin" "$HOME/.local/share/applications" "$HOME/.local/share/icons"

    ln -sf "$install_dir/bin/zed" "$HOME/.local/bin/zed-aura"

    # Copy whichever .desktop file shipped in the bundle, rename it, and
    # point Exec/Icon at the installed location instead of relying on PATH.
    src_desktop="$(find "$install_dir/share/applications" -maxdepth 1 -name '*.desktop' | head -n1)"
    if [ -n "$src_desktop" ]; then
        desktop_file_path="$HOME/.local/share/applications/zed-aura.desktop"
        cp "$src_desktop" "$desktop_file_path"
        sed -i "s|Icon=zed|Icon=$install_dir/share/icons/hicolor/512x512/apps/zed.png|g" "$desktop_file_path"
        sed -i "s|Exec=zed|Exec=$install_dir/bin/zed|g" "$desktop_file_path"
        sed -i "s|^Name=.*|Name=Zed Aura|" "$desktop_file_path"
    fi

    if [ "$(command -v zed-aura)" = "$HOME/.local/bin/zed-aura" ]; then
        echo "Zed Aura has been installed. Run with 'zed-aura'"
    else
        echo "To run Zed Aura from your terminal, you must add ~/.local/bin to your PATH"
        echo "Run:"

        case "${SHELL:-}" in
            *zsh)
                echo "   echo 'export PATH=\$HOME/.local/bin:\$PATH' >> ~/.zshrc"
                echo "   source ~/.zshrc"
                ;;
            *fish)
                echo "   fish_add_path -U $HOME/.local/bin"
                ;;
            *)
                echo "   echo 'export PATH=\$HOME/.local/bin:\$PATH' >> ~/.bashrc"
                echo "   source ~/.bashrc"
                ;;
        esac

        echo "To run Zed Aura now, '~/.local/bin/zed-aura'"
    fi
}

main "$@"
