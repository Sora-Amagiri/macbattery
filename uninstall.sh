#!/bin/sh
# Uninstall macbattery.
#
#   curl -fsSL https://raw.githubusercontent.com/Sora-Amagiri/macbattery/main/uninstall.sh | sh
#
# Environment:
#   MACBATTERY_INSTALL_DIR  only remove from this directory (default: the
#                           standard install locations)
set -eu

found=0

remove_from() {
    dir="$1"
    if [ -f "$dir/macbattery" ]; then
        rm -f "$dir/macbattery"
        echo "Removed $dir/macbattery"
        found=1
    fi
}

if [ -n "${MACBATTERY_INSTALL_DIR:-}" ]; then
    remove_from "$MACBATTERY_INSTALL_DIR"
else
    remove_from /usr/local/bin
    remove_from "$HOME/.local/bin"
fi

if [ "$found" -eq 0 ]; then
    echo "macbattery was not found in the standard locations."
    echo "If you installed it elsewhere, re-run with MACBATTERY_INSTALL_DIR set."
fi
