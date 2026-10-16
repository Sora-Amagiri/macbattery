#!/bin/sh
# Install the latest macbattery release on macOS.
#
#   curl -fsSL https://raw.githubusercontent.com/Sora-Amagiri/macbattery/main/install.sh | sh
#
# Environment:
#   MACBATTERY_VERSION      release tag to install, e.g. v0.1.0 (default: latest)
#   MACBATTERY_INSTALL_DIR  directory to install into (default: /usr/local/bin)
set -eu

REPO="Sora-Amagiri/macbattery"
ASSET="macbattery-macos-universal.tar.gz"
VERSION="${MACBATTERY_VERSION:-latest}"

if [ "$(uname -s)" != "Darwin" ]; then
    echo "error: macbattery only runs on macOS" >&2
    exit 1
fi

if [ "$VERSION" = "latest" ]; then
    URL="https://github.com/$REPO/releases/latest/download"
else
    URL="https://github.com/$REPO/releases/download/$VERSION"
fi

# Pick an install location: explicit > /usr/local/bin > ~/.local/bin.
if [ -n "${MACBATTERY_INSTALL_DIR:-}" ]; then
    DIR="$MACBATTERY_INSTALL_DIR"
elif [ -w /usr/local/bin ]; then
    DIR="/usr/local/bin"
else
    DIR="$HOME/.local/bin"
fi
mkdir -p "$DIR"

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

echo "Downloading macbattery ($VERSION) for macOS..."
if ! curl -fsSL "$URL/$ASSET" -o "$TMP/$ASSET"; then
    echo "error: could not download $URL/$ASSET" >&2
    exit 1
fi
curl -fsSL "$URL/checksums.txt" -o "$TMP/checksums.txt"

# Verify the download against the published SHA-256 checksum.
expected="$(awk -v f="$ASSET" '$2 == f { print $1 }' "$TMP/checksums.txt")"
actual="$(shasum -a 256 "$TMP/$ASSET" | awk '{ print $1 }')"
if [ -z "$expected" ] || [ "$expected" != "$actual" ]; then
    echo "error: checksum verification failed for $ASSET" >&2
    exit 1
fi

tar -xzf "$TMP/$ASSET" -C "$TMP"
install -m 0755 "$TMP/macbattery" "$DIR/macbattery"

echo "Installed $DIR/macbattery"
case ":$PATH:" in
    *":$DIR:"*) ;;
    *) echo "note: $DIR is not on your PATH; add it so 'macbattery' resolves." ;;
esac
echo "Run 'macbattery' for the live dashboard, or 'macbattery --once' for a report."
