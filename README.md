# pinstar

A terminal-based canvas editor compatible with the Obsidian `.canvas` JSON specification. Built with Rust and the Ratatui framework.

## DISCLAIMER
`pinstar` is a sub project of [clin-rs](https://github.com/reekta92/clin-rs) project, i separated it as a different project for those who only want to use/test this feature.

## Features
- **Spec Compliance**: Read, write, and manipulate `CanvasData` serializable schemas (text, file, link, and group nodes).
- **Interactive TUI**: Pan, zoom, and navigate node relations inside the terminal using `ratatui`.
- **Inline Editing**: Built-in node content modification via `ratatui-textarea`.
- **UUID Graph**: Resilient node identification and edge resolution logic.

## Technical Stack
- **Core Framework**: [Ratatui](https://github.com/ratatui/ratatui) for terminal rendering.
- **Serialization**: [Serde JSON](https://github.com/serde-rs/json) for compliant parsing of the `.canvas` spec.
- **Identifiers**: `uuid` crate for RFC 4122 compliant node tracking.

## Installation
### Pre-built Packages
Pre-compiled binaries and system packages (`.deb`, `.rpm`, `PKGBUILD`) for Debian/Ubuntu, Fedora/RHEL, and Arch Linux distributions are available on the GitHub **[Releases](https://github.com/reekta92/clin/releases)** page.

### Via Cargo
You can install the crate directly from crates.io:
```bash
cargo install pinstar
```

## Building from Source
Requires a standard Rust toolchain (MSRV 1.85 as specified in `Cargo.toml`).

```bash
cargo build --release
```
