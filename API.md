# Pinstar Library API

Host-agnostic canvas/diagram engine for Obsidian-compatible `.canvas` files,
Mermaid, DOT, and PlantUML flowcharts. Hosts own keybinds, status lines,
dialogs, and the system clipboard; pinstar owns everything canvas-related.

## Dependency

```toml
# Cargo.toml
[dependencies]
pinstar = { git = "https://github.com/reekta92/pinstar", tag = "v1.0.0", features = ["images"] }
```

The `images` feature enables image-node support (`pinstar::image` module,
`ratatui-image`, `lru`). Omit it for a lighter build if your host doesn't need
inline images.

## Architecture Overview

```
┌─────────────────────────────────────────────────────────┐
│  Host application (e.g. clin-rs)                        │
│  ┌───────────┐  ┌──────────┐  ┌──────────────────────┐  │
│  │ Keybinds  │  │ Clipboard│  │ Dialogs / Status bar │  │
│  └─────┬─────┘  └────┬─────┘  └──────────┬───────────┘  │
│        │              │                   │              │
│        ▼              ▼                   ▼              │
│  ┌─────────────────────────────────────────────────┐     │
│  │            pinstar library                      │     │
│  │  PinstarState ◄─── apply_action(PinstarAction)  │     │
│  │       │       ◄─── handle_pinstar_mouse()       │     │
│  │       ▼                                         │     │
│  │  draw_pinstar_view(frame, state, theme, area)   │     │
│  └─────────────────────────────────────────────────┘     │
└─────────────────────────────────────────────────────────┘
```

Hosts resolve their own key events to `PinstarAction` variants and feed them
through `apply_action`. Mouse events go through `handle_pinstar_mouse`. The
renderer `draw_pinstar_view` paints the canvas into any ratatui `Rect`,
reserving one bottom row for the host's footer.

## Quick Start

```rust
use pinstar::{
    PinstarState, Settings, ThemeColors,
    PinstarAction, ActionCtx, ActionOutcome,
    apply_action, draw_pinstar_view, handle_pinstar_mouse,
    execute_menu_action,
};

// 1. Load a canvas file
let mut state = PinstarState::load(Path::new("my_diagram.canvas"))?;

// 2. Configure host-specific settings
state.settings = Settings {
    enable_image_nodes: true,  // show "Add Image Node" in context menus
    image_cache_size: 64,      // LRU slots for decoded images
    rename_uses_id: false,     // false = rename title (clin), true = rename id (standalone)
};

// 3. In your render loop
fn render(frame: &mut Frame, state: &mut PinstarState, area: Rect) {
    let theme = ThemeColors::default(); // or map from your app's theme
    let mouse_pos = state.mouse_pos;    // updated by your mouse handler
    draw_pinstar_view(frame, state, &theme, area, mouse_pos);
}

// 4. Dispatch actions from your keybind system
let outcome = apply_action(
    &mut state,
    PinstarAction::ZoomIn,
    &ActionCtx { area, count: 1 },
);

// 5. Handle host-level requests from the outcome
if let Some(host_action) = outcome.host_action {
    match host_action {
        PinstarAction::Save => { let _ = state.save(); }
        PinstarAction::Quit => { /* exit canvas view */ }
        PinstarAction::Help => { /* open your help screen */ }
        PinstarAction::AddImageNode => { /* open file picker, then: */ }
        PinstarAction::ToggleOrthogonal => { /* persist preference */ }
        _ => {}
    }
}
if let Some(notice) = outcome.notice {
    // display transient status message
}
```

## Core Types

### `PinstarState`

Central mutable state for one canvas session. Created via `PinstarState::load(path)`.

```rust
let mut state = PinstarState::load(Path::new("diagram.canvas"))?;
```

Key fields accessible to hosts:

| Field | Type | Description |
|---|---|---|
| `path` | `PathBuf` | File being edited |
| `format` | `SupportedFormat` | Detected file format |
| `data` | `CanvasData` | Live node/edge data |
| `settings` | `Settings` | Host behavior switches |
| `selection` | `Selection<String>` | Multi-select state (primary + extras) |
| `selected_edge_id` | `Option<String>` | Currently selected edge |
| `floating_editor` | `Option<TextArea>` | Inline node text editor |
| `raw_editor` | `TextArea` | Source-view text editor |
| `editor_focus` | `bool` | Whether raw editor has focus |
| `context_menu` | `Option<PinstarContextMenu>` | Active context menu |
| `mouse_pos` | `Option<(u16, u16)>` | Current mouse position (host must set) |
| `locked` | `bool` | Editing locked (auto-true for flowcharts) |
| `show_grid` | `bool` | Grid dots visible |
| `show_help` | `bool` | Help overlay active |
| `orthogonal_connections` | `bool` | Right-angle edges |
| `rename_popup` | `Option<TextArea>` | Active rename popup |
| `trigger_ext_editor` | `bool` | Flag: host should open external editor |
| `trigger_image_picker` | `bool` | Flag: host should open image file dialog |
| `footer_hint` | `String` | Context-sensitive hint text for host footer |
| `last_area` | `Rect` | Last rendered area |

Key methods:

```rust
state.save()?;                           // serialize + atomic write
state.sync_to_raw_editor();              // data → raw editor text
state.sync_from_raw_editor()?;           // raw editor text → data
state.rename_node_title(new_title);      // rename selected node
state.add_image_node_with(path, x, y);   // add image node at position
state.toggle_editor();                   // toggle raw source editor
state.finish_connection(&target_id);     // complete edge creation
state.finish_delete_connection(&target_id); // complete edge deletion
state.active_mode_message();             // status text for connection/resize modes
state.select_edge_of_selected_node(idx); // select Nth edge of current node
state.open_edge_menu_centered(area);     // open edge context menu
```

### `Settings`

```rust
pub struct Settings {
    pub enable_image_nodes: bool,  // default: false
    pub image_cache_size: usize,   // default: 32
    pub rename_uses_id: bool,      // default: false
}
```

### `PinstarAction`

All possible canvas actions. The host maps its own keybinds onto these.

**Host-level actions** (returned via `ActionOutcome::host_action` for the host to handle):

| Variant | Host responsibility |
|---|---|
| `Quit` | Exit canvas view |
| `Save` | Call `state.save()` |
| `Help` | Open help screen |
| `AddImageNode` | Open file dialog → `state.add_image_node_with(path, x, y)` |
| `ToggleOrthogonal` | Persist the preference per vault/project |

**Stateful actions** (consumed by `apply_action`, no host handling needed):

`Undo`, `Redo`, `ZoomIn`, `ZoomOut`, `ZoomFineIn`, `ZoomFineOut`,
`MoveLeft`, `MoveRight`, `MoveUp`, `MoveDown`, `EditOrConnect`,
`OpenContextMenu`, `MenuUp`, `MenuDown`, `MenuSelect`, `MenuClose`,
`CreateConnection`, `DeleteConnection`, `RenameNode`, `ResizeMode`,
`SetColor`, `DeleteNode`, `DeleteAllConnections`, `AddTextNode`, `AddGroup`,
`ToggleGrid`, `ToggleEditorPane`, `CycleFocus`, `RenameConfirm`,
`RenameCancel`, `ConfirmResize`, `CancelResize`, `EditorUnfocus`,
`CloseEditor`, `CloseEditorAlt`

**Standalone-only** (used by the pinstar binary's default keymap, ignored by hosted use):

`PickShape`, `PickOrientation`, `ToggleLock`, `OpenExternalEditor`,
`OpenWholeFileEditor`, `ShowHelp`

### `ActionCtx`

```rust
pub struct ActionCtx {
    pub area: Rect,    // terminal area the canvas occupies
    pub count: usize,  // repeat count from host's prefix resolver (1-based)
}
```

### `ActionOutcome`

```rust
pub struct ActionOutcome {
    pub host_action: Option<PinstarAction>,  // host-level request
    pub notice: Option<&'static str>,        // transient status message
}
```

### `MouseOutcome`

```rust
pub struct MouseOutcome {
    pub consumed: bool,                // event was handled
    pub notice: Option<&'static str>,  // transient status message
    pub clipboard: Option<String>,     // text to write to system clipboard
}
```

## Input Handling

### Keyboard: `apply_action`

```rust
pub fn apply_action(
    state: &mut PinstarState,
    action: PinstarAction,
    ctx: &ActionCtx,
) -> ActionOutcome;
```

Host resolves key events → `PinstarAction`, calls `apply_action`. Stateful
actions are consumed. Host-level actions are echoed back via
`outcome.host_action` for the host to fulfill.

### Mouse: `handle_pinstar_mouse`

```rust
pub fn handle_pinstar_mouse(
    state: &mut PinstarState,
    mouse: crossterm::event::MouseEvent,
    area: Rect,
) -> MouseOutcome;
```

Handles click, drag, scroll, marquee selection, context menus, resize
handles, and text selection. The host must:

1. Set `state.mouse_pos` before calling
2. Handle `outcome.clipboard` (write to system clipboard)
3. Display `outcome.notice` if present
4. Check `state.trigger_image_picker` after the call

### Context Menus: `execute_menu_action`

```rust
pub fn execute_menu_action(
    state: &mut PinstarState,
    label: &str,
    menu_type: PinstarMenuType,
    menu_x: u16,
    menu_y: u16,
) -> ActionOutcome;
```

Execute a context menu item by its label string. Used when the host intercepts
menu selection through its own keybind system rather than letting the engine
handle `MenuSelect`.

### Standalone Keymap: `handle_pinstar_event`

```rust
pub fn handle_pinstar_event(
    state: &mut PinstarState,
    key: KeyEvent,
    running: &mut bool,
    area: Rect,
) -> bool;
```

Default hardcoded keymap used by the standalone `pinstar` binary. Hosts with
their own keybind system (like clin-rs) should use `apply_action` instead.

## Rendering

```rust
pub fn draw_pinstar_view(
    frame: &mut Frame,
    state: &mut PinstarState,
    theme: &ThemeColors,
    area: Rect,
    mouse_pos: Option<(u16, u16)>,
);
```

Renders the full canvas view into `area`. Reserves the bottom row for the
host's footer/statusline. Handles both canvas-format rendering (node blocks,
edges, marquee) and flowchart-format rendering (shapes, braille borders,
box-drawing orthogonal edges).

### `ThemeColors`

Map your app's theme to pinstar's theme struct:

```rust
pub struct ThemeColors {
    pub accent: Color,
    pub heading: Color,
    pub success: Color,
    pub warning: Color,
    pub destructive: Color,
    pub muted: Color,
    pub text: Color,
    pub fg: Color,
    pub bg: Color,
    pub border: Color,
    pub tag: Color,
    pub folder: Color,
    pub highlight_fg: Color,
    pub highlight_bg: Color,
}
```

All fields have sensible defaults via `ThemeColors::default()`.

## Data Model (`pinstar::data`)

Obsidian-compatible `.canvas` JSON format. All types derive `Serialize` +
`Deserialize`.

### `CanvasData`

```rust
pub struct CanvasData {
    pub nodes: Vec<CanvasNode>,
    pub edges: Vec<CanvasEdge>,
    pub orientation: DiagramOrientation,  // default: TopDown
}
```

### `CanvasNode`

```rust
pub enum CanvasNode {
    Text(TextNode),
    File(FileNode),
    Link(LinkNode),
    Group(GroupNode),
}
```

Common accessors on `CanvasNode`: `id()`, `pos()`, `size()`, `set_pos()`,
`set_size()`, `color()`, `title()`.

### Node Types

```rust
pub struct TextNode {
    pub id: String,
    pub x: f64, pub y: f64,
    pub width: f64, pub height: f64,
    pub text: String,
    pub title: Option<String>,
    pub color: Option<String>,
    pub shape: NodeShape,  // Rectangle, Diamond, Circle, Cylinder, Stadium
}

pub struct FileNode {
    pub id: String,
    pub x: f64, pub y: f64,
    pub width: f64, pub height: f64,
    pub file: String,
    pub subpath: Option<String>,
    pub title: Option<String>,
    pub color: Option<String>,
}

pub struct LinkNode {
    pub id: String,
    pub x: f64, pub y: f64,
    pub width: f64, pub height: f64,
    pub url: String,
    pub title: Option<String>,
    pub color: Option<String>,
}

pub struct GroupNode {
    pub id: String,
    pub x: f64, pub y: f64,
    pub width: f64, pub height: f64,
    pub label: Option<String>,
    pub color: Option<String>,
}
```

### `CanvasEdge`

```rust
pub struct CanvasEdge {
    pub id: String,
    pub from_node: String,
    pub from_side: Option<String>,
    pub to_node: String,
    pub to_side: Option<String>,
    pub label: Option<String>,
    pub color: Option<String>,
    pub style: EdgeStyle,  // Solid, Dashed, Dotted, Thick
}
```

### Creating Canvas Data Programmatically

```rust
use pinstar::data::*;

let data = CanvasData {
    nodes: vec![
        CanvasNode::Text(TextNode {
            id: "node1".into(),
            x: 0.0, y: 0.0,
            width: 250.0, height: 60.0,
            text: "Hello".into(),
            title: None,
            color: Some("1".into()),  // palette index or "#rrggbb"
            shape: NodeShape::default(),
        }),
    ],
    edges: vec![],
    orientation: DiagramOrientation::default(),
};

// Serialize to .canvas JSON
let json = serde_json::to_string_pretty(&data)?;
```

## Format Support (`pinstar::formats`)

```rust
pub enum SupportedFormat {
    Canvas,    // .canvas (Obsidian JSON)
    Mermaid,   // .md, .mmd, .mermaid
    Dot,       // .dot, .gv
    PlantUml,  // .puml, .plantuml, .iuml
}
```

```rust
// Auto-detect from file extension
let format = pinstar::formats::detect_format(path);

// Load any supported format into CanvasData
let data = pinstar::formats::load_from_format(path, &content, format)?;

// Save back (round-trips flowchart source, serializes canvas as JSON)
let output = pinstar::formats::save_to_format(&data, &original, format, write_layout)?;
```

Flowchart formats auto-apply hierarchical layout when no existing positions
are found. Canvas format uses force-directed layout for unpositioned nodes.

## Image Support (`pinstar::image`, feature = `"images"`)

Background image decode worker + LRU cache. Wire it up during init:

```rust
use pinstar::image::{spawn_worker, ImageCache, DecodedImage};

// Spawn the decode worker thread
let (tx, rx) = spawn_worker();
state.image_decode_tx = Some(tx);
state.image_cache = ImageCache::new(state.settings.image_cache_size);

// If using ratatui-image, provide a picker
state.image_picker = Some(ratatui_image::picker::Picker::from_query_stdio()?);

// In your event loop, poll for completed decodes
while let Ok(result) = rx.try_recv() {
    if let (Ok(img), Some(picker)) = (result, state.image_picker.as_ref()) {
        state.image_cache.install_decoded(img, picker);
    }
}
```

## Additional Public Modules

| Module | Key exports | Purpose |
|---|---|---|
| `camera` | `zoom_step`, `pan_centered`, `clamp_world`, `nearest_in_dir` | Viewport math |
| `grid` | `CanvasGridProjection`, `draw_canvas_grid` | Adaptive dot grid |
| `selection` | `Selection<Id>` | Multi-select with primary + extras |
| `overlay` | `MarqueeState`, `draw_canvas_rect_filled`, `muted_canvas_selection_fill` | Selection visuals |
| `menu` | `PinstarContextMenu`, `PinstarMenuType`, `MenuItemSpec`, `menu_specs` | Context menu definitions |
| `help` | Help text content | Keyboard/mouse/format help tabs |
| `textsel` | `MouseTextSelection` | Mouse-driven text selection |
| `theme` | `ThemeColors`, `parse_hex_color`, `get_node_color`, `get_edge_color` | Theme + color utilities |

## Constants

```rust
// Color picker palette shared between canvas and draw views
pub const COLOR_PICKER_PALETTE: &[(&str, &str, Color)] = &[...];

// Delay before re-rendering pixel images after zoom/pan/resize
pub const TRANSFORM_SETTLE: Duration = Duration::from_millis(150);
```

## Utilities

```rust
// Atomic file write (temp + rename) to prevent corruption
pub fn atomic_write(path: &Path, content: &str) -> Result<()>;
```

## Integration Pattern (clin-rs reference)

clin-rs wraps pinstar in a `PinstarPlugin` struct that owns:
- `PinstarState` — the engine state
- Host keybinds + sequence matcher
- Image decode receiver

The integration flow:

1. **Init**: `PinstarState::load(path)` → configure `settings` → `spawn_worker()` → wire `image_decode_tx` and `image_picker`
2. **Render**: call `draw_pinstar_view` → paint host footer over the reserved bottom row
3. **Key events**: map host `CanvasAction` → `PinstarAction` via a 1:1 mapping function → `apply_action` → handle `ActionOutcome`
4. **Mouse events**: set `state.mouse_pos` → `handle_pinstar_mouse` → handle `MouseOutcome` (clipboard, notices, image picker trigger)
5. **Save**: on `ActionOutcome::host_action == Save`, call `state.save()`
6. **Host dialogs**: when `AddImageNode` returned or `trigger_image_picker` set, open file picker → `state.add_image_node_with(path, x, y)` → `state.sync_to_raw_editor()`

```rust
// Minimal host action → lib action mapping (shared subset)
fn to_lib_action(action: CanvasAction) -> PinstarAction {
    match action {
        CanvasAction::Quit       => PinstarAction::Quit,
        CanvasAction::Save       => PinstarAction::Save,
        CanvasAction::Help       => PinstarAction::Help,
        CanvasAction::Undo       => PinstarAction::Undo,
        CanvasAction::Redo       => PinstarAction::Redo,
        CanvasAction::ZoomIn     => PinstarAction::ZoomIn,
        CanvasAction::ZoomOut    => PinstarAction::ZoomOut,
        CanvasAction::MoveUp     => PinstarAction::MoveUp,
        CanvasAction::MoveDown   => PinstarAction::MoveDown,
        CanvasAction::MoveLeft   => PinstarAction::MoveLeft,
        CanvasAction::MoveRight  => PinstarAction::MoveRight,
        CanvasAction::AddTextNode => PinstarAction::AddTextNode,
        CanvasAction::DeleteNode => PinstarAction::DeleteNode,
        // ... remaining 1:1 mappings
    }
}
```
