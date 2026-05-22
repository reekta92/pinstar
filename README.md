# pinstar

<img width="1888" height="1012" alt="image" src="https://github.com/user-attachments/assets/85c0c26c-f7f6-4e5a-9538-dc8f603e0659" />

A terminal-based diagram editor for **Obsidian Canvas**, **Mermaid**, **Graphviz DOT**, and **PlantUML** files. Built with Rust and the Ratatui framework.

## Disclaimer
`pinstar` is a sub-project of [clin-rs](https://github.com/reekta92/clin-rs). Separated for standalone use/testing.

## Usage
```bash
pinstar <FILE>.canvas|.md|.dot|.puml
```

If the file doesn't exist, pinstar creates an empty diagram in the detected format.

### Supported Formats
| Extension | Format | Layout |
|-----------|--------|--------|
| `.canvas` | Obsidian Canvas JSON | Free-form, force-directed |
| `.md`, `.mermaid` | Mermaid flowchart syntax | Hierarchical (auto) |
| `.dot`, `.gv` | Graphviz DOT syntax | Hierarchical (auto) |
| `.puml`, `.plantuml` | PlantUML activity syntax | Hierarchical (auto) |

## Features
- **Multi-Format Support**: Read, write, and edit Canvas, Mermaid, DOT, and PlantUML diagrams
- **Spec Compliance**: Full Obsidian `.canvas` JSON support (text, file, link, and group nodes)
- **Interactive TUI**: Pan, zoom, and navigate node relations inside the terminal using `ratatui`
- **Inline Editing**: Built-in node content editing via `ratatui-textarea`
- **Node Shapes**: Rectangle, Diamond, Circle, Cylinder, Stadium (flowchart formats)
- **Edge Styles**: Solid, Dashed, Dotted
- **Color Presets**: 10 colors (Default, Red, Orange, Yellow, Green, Cyan, Blue, Purple, Magenta, White)
- **Hierarchical Layout**: Automatic layout for Mermaid, DOT, and PlantUML with 4 orientations
- **Undo/Redo**: Full undo/redo stack
- **Raw Editor**: Split-pane JSON/text editor with live sync

## Keybindings

### Navigation
| Key | Action |
|-----|--------|
| `Arrow keys` / `hjkl` | Select adjacent nodes |
| `Ctrl+J` / `+` | Zoom in |
| `Ctrl+K` / `−` | Zoom out |
| `Ctrl+F` | Fit all nodes into view |
| `Alt+Enter` | Cycle pane focus |

### Editing
| Key | Action |
|-----|--------|
| `i` / `Enter` | Edit selected node content |
| `r` | Rename node (ID) |
| `s` | Resize node |
| `a` | Open context menu |
| `x` | Delete selected node |
| `o` | Set node color |
| `p` | Set node shape (flowchart only) |

### Connections
| Key | Action |
|-----|--------|
| `c` | Create connection from selected node |
| `d` | Delete connection to target node |
| `b` | Delete all connections on selected node |

### Global
| Key | Action |
|-----|--------|
| `Ctrl+S` | Save diagram |
| `Ctrl+Z` | Undo |
| `Ctrl+Y` / `Ctrl+Shift+Z` | Redo |
| `Ctrl+R` | Cycle orientation (flowchart) / Reload from disk (canvas) |
| `Ctrl+G` | Toggle background grid |
| `Ctrl+E` | Toggle raw editor pane |
| `Ctrl+X` | Toggle external editor mode |
| `Ctrl+L` | Toggle spatial lock |
| `Ctrl+O` | Toggle orthogonal arrows (canvas only) |
| `Esc` / `q` | Dismiss / deselect / quit |
| `?` | Toggle help |

## Mouse Controls

| Action | Effect |
|--------|--------|
| Left-click | Select node |
| Left-double-click | Edit node content |
| Left-drag (on node) | Move node |
| Left-drag (empty) | Pan canvas |
| Right-click (on node) | Node context menu |
| Right-click (on edge) | Edge context menu |
| Right-click (empty) | Add node menu |
| Right-drag | Selection rectangle (multi-select) |
| Middle-click drag | Pan canvas |
| Scroll | Zoom in / out |

## Node & Edge Types

### Nodes
- **Text** — Standard text node with configurable shape and color
- **File** — Embedded file reference (Canvas only)
- **Link** — Embedded URL reference (Canvas only)
- **Group** — Container for grouping child nodes (Canvas only)

### Shapes (flowchart formats)
Rectangle · Diamond · Circle · Cylinder · Stadium

### Edge Styles
Solid · Dashed · Dotted

### Orientations (flowchart)
Top-Down · Left-Right · Right-Left · Bottom-Up

### Colors (10 presets)
Default · Red · Orange · Yellow · Green · Cyan · Blue · Purple · Magenta · White

## Known Issues / Compatibility
- Node titles may not be preserved when editing Obsidian Canvas files (Obsidian uses a different title format)
- No image rendering — images display as simple nodes
- Diamond and Stadium shapes not available in PlantUML
- Color editing not available in Mermaid or PlantUML syntaxes
- Groups only supported in Canvas format

## Technical Stack
- **Framework**: [Ratatui](https://github.com/ratatui/ratatui) v0.30 for terminal rendering
- **Terminal**: [Crossterm](https://github.com/crossterm-rs/crossterm) v0.29 for cross-platform input
- **Serialization**: [Serde JSON](https://github.com/serde-rs/json) for Canvas spec parsing
- **Text Editing**: [ratatui-textarea](https://github.com/rhysd/ratatui-textarea) for inline editing
- **Parsing**: `regex` for multi-format parsers
- **Identifiers**: `uuid` crate for RFC 4122 compliant node tracking

## Installation
### Pre-built Packages
Pre-compiled binaries and system packages (`.deb`, `.rpm`, `PKGBUILD`) for Debian/Ubuntu, Fedora/RHEL, and Arch Linux are available on the GitHub **[Releases](https://github.com/reekta92/pinstar/releases)** page.

### Via Cargo
```bash
cargo install pinstar
```

## Building from Source
Requires Rust toolchain (MSRV 1.85).

```bash
cargo build --release
```
