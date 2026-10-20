# pinstar

<img width="1888" height="1012" alt="image" src="https://github.com/user-attachments/assets/85c0c26c-f7f6-4e5a-9538-dc8f603e0659" />

A terminal-based diagram editor for **Obsidian Canvas**, **Mermaid**, **Graphviz DOT**, and **PlantUML** files. Built with Rust and the Ratatui framework.

## Disclaimer
`pinstar` is a sub-project of [clin-rs](https://github.com/reekta92/clin-rs). Separated for standalone use/testing. The project evolves as a module inside the `clin-rs` project **for now** in the future once it's matured enough both versions will be merged and unified together so any future updates inside one or other will be available on each other. Until then the project is in semi-maintanence mode. Reported bugs or features will be looked into for implementation.

**If you want to have newest features/changes about this project check out clin-rs. Note that flowchart support does not exists in clin-rs though, for now.**

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
- **External Editor**: Edit node content or entire diagrams in your `$EDITOR`
- **Image Rendering**: Terminal-native image previews for Canvas file nodes (requires Kitty/Sixel/iTerm2 support)
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
| `r` | Rename selected node (ID) |
| `s` | Resize selected node |
| `a` | Open context menu |
| `x` | Delete selected node |
| `o` | Set node color |
| `p` | Set node shape (flowchart only) |

### Connections
| Key | Action |
|-----|--------|
| `c` | Create connection from selected node |
| `d` | Delete connection from selected node |
| `b` | Delete all connections on selected node |
### Global
| Key | Action |
|-----|--------|
| `Ctrl+S` | Save diagram |
| `Ctrl+Z` | Undo last action |
| `Ctrl+Y` / `Ctrl+Shift+Z` | Redo undone action |
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
Releases publish the `pinstar` crate to crates.io. Install the standalone binary with `cargo install pinstar`; historical binary assets remain on the GitHub Releases page.

### Via Cargo
```bash
cargo install pinstar
```

## Building from Source
Requires Rust toolchain (MSRV 1.90.0).

```bash
cargo build --release
```

## Library Usage

pinstar can be embedded as a library in other Rust projects (e.g. [clin-rs](https://github.com/reekta92/clin-rs)). See **[API.md](API.md)** for the full integration guide covering state management, input handling, rendering, data model, and format support.

### Mermaid dependency compatibility

The optional `mermaid-preview` feature uses the published `merman-core-clin
0.6.0` compatibility package. It preserves the upstream 0.6 parser API and
Rust 1.90 support while using maintained `yaml_serde 0.10.7`. Consumers need no
root Cargo patch. Upstream MIT/Apache-2.0 licensing is retained.

Retire the compatibility package when a maintained upstream release fits the
host's supported compiler and parser API. See the shared development process in
[Clin](https://github.com/reekta92/clin-rs/blob/main/docs/DEVELOPMENT.md).
