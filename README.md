# macbattery

[![CI](https://github.com/Sora-Amagiri/macbattery/actions/workflows/ci.yml/badge.svg)](https://github.com/Sora-Amagiri/macbattery/actions/workflows/ci.yml)
[![Release](https://github.com/Sora-Amagiri/macbattery/actions/workflows/release.yml/badge.svg)](https://github.com/Sora-Amagiri/macbattery/actions/workflows/release.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)
![Platform: macOS](https://img.shields.io/badge/platform-macOS-black.svg)

Live MacBook battery health dashboard for the terminal: charge, health, cycle
count, power draw, and everything else the fuel gauge knows — refreshed every
two seconds.

It talks to IOKit's `AppleSmartBattery` service directly (no subprocesses, no
parsing `ioreg` text) and never needs `sudo`.

```
┌ MacBook Battery ─────────────────────────────────────────────┐
│ 42%  ·  3700 / 3842 mAh  ·  Charging                         │
│ 46 min until full  ·  AC power  ·  45 W adapter              │
├ Health ────────────────────┬ Power ──────────────────────────┤
│ Cycles    81 / 1000        │ Voltage   12.689 V              │
│ [███░░░░░░░░░░░░░░░░░░░░]  │ Current   +937 mA               │
│ Health    87.7 %           │ Power     11.9 W  (charging)    │
│ [██████████████████████░]  │ Adapter   45 W                  │
│ Condition Normal           │ Charge    3700 mAh  (42%)       │
│ Temperature 31.4 °C        │ Full charge 3842 mAh            │
├ Details ───────────────────┴─────────────────────────────────┤
│ Serial        F5D0...              Model          bq20z451   │
│ Manufacturer  DSY                  Manufactured   2023-04-28 │
├──────────────────────────────────────────────────────────────┤
│ q quit  r refresh  p pause                                   │
└──────────────────────────────────────────────────────────────┘
```

## Install

### Universal binary (no Rust toolchain needed)

```sh
curl -fsSL https://raw.githubusercontent.com/Sora-Amagiri/macbattery/main/install.sh | sh
```

Downloads the latest release — a universal binary that runs natively on both
Apple Silicon and Intel — verifies its SHA-256 checksum, and installs it to
`/usr/local/bin` (or `~/.local/bin` if that isn't writable).

```sh
# Pin a version, or choose the install directory:
curl -fsSL .../install.sh | MACBATTERY_VERSION=v0.1.0 sh
curl -fsSL .../install.sh | MACBATTERY_INSTALL_DIR="$HOME/bin" sh
```

### From source

```sh
cargo install --git https://github.com/Sora-Amagiri/macbattery
```

or clone and build (the binary lands in `target/release/macbattery`):

```sh
git clone https://github.com/Sora-Amagiri/macbattery
cd macbattery
cargo build --release
```

Building requires macOS and Rust 1.88+ (edition 2024).

### Uninstall

```sh
curl -fsSL https://raw.githubusercontent.com/Sora-Amagiri/macbattery/main/uninstall.sh | sh
```

Or remove the binary directly: `rm /usr/local/bin/macbattery` (or
`rm ~/.local/bin/macbattery`).

## Usage

```
macbattery             live TUI dashboard
macbattery --once      print a one-shot report and exit (-1)
macbattery --version   print the version and exit (-V)
macbattery --help      show this help (-h)
```

**TUI keys:** `q` / `Esc` / `Ctrl+C` quit · `r` refresh now · `p` pause/resume auto-refresh.

`--once` is handy in scripts, status bars, and dotfiles:

```sh
$ macbattery --once
MacBook Battery Report
  Charge         42%  (3700 / 3842 mAh)  Charging
  Time           46 min until full
  Health         87.7%  (3842 mAh of 4381 mAh design)
  Cycles         81 / 1000
  Power          12.689 V  +937 mA  11.9 W  31.4 °C
  Adapter        45 W
  Condition      Normal
  Serial         C0123456789ABCDEF
  Manufacturer   DSY (bq20z451)
  Manufactured   2023-04-28
```

## Requirements

- macOS with a battery (Intel or Apple Silicon).
- On desktop Macs and VMs the app reports “No battery detected” instead of
  showing empty panels.

## Development

```sh
cargo test          # unit tests for parsing, formatting, and rendering
cargo clippy        # lints
```

### Releasing

Pushing a `v*` tag triggers the release workflow, which runs the tests,
builds both architectures, combines them into one universal binary, ad-hoc
signs it, and publishes `macbattery-macos-universal.tar.gz` plus
`checksums.txt` to GitHub Releases:

```sh
git tag v0.1.0
git push origin v0.1.0
```

## License

MIT — see [LICENSE](LICENSE).
