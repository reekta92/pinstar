use ratatui::text::{Line, Span};
use ratatui::style::{Modifier, Style};
use crate::helpers::PinstarTheme;
use crate::state::PinstarHelpTab;

pub fn help_content(tab: PinstarHelpTab, theme: &PinstarTheme, max_width: u16) -> Vec<Line<'static>> {
    let key_style = Style::default().fg(theme.accent).add_modifier(Modifier::BOLD);
    let desc_style = Style::default().fg(theme.fg);
    let heading_style = Style::default().fg(theme.accent).add_modifier(Modifier::BOLD);
    let note_style = Style::default().fg(theme.muted);

    let separator = Line::from(vec![
        Span::styled("─".repeat(max_width as usize), note_style),
    ]);

    match tab {
        PinstarHelpTab::Keyboard => {
            let sections: Vec<(&str, Vec<(&str, &str)>)> = vec![
                ("Navigation", vec![
                    ("Arrows / hjkl", "Select adjacent nodes"),
                    ("Ctrl+J / +", "Zoom in"),
                    ("Ctrl+K / −", "Zoom out"),
                    ("Ctrl+F", "Fit all nodes into view"),
                ]),
                ("Editing", vec![
                    ("i / Enter", "Edit selected node content"),
                    ("r", "Rename selected node (ID)"),
                    ("s", "Resize selected node"),
                    ("a", "Open context menu"),
                    ("x", "Delete selected node"),
                    ("o", "Set node color"),
                    ("p", "Set node shape (flowchart only)"),
                ]),
                ("Connections", vec![
                    ("c", "Create connection from selected node"),
                    ("d", "Delete connection from selected node"),
                    ("b", "Delete all connections on selected node"),
                ]),
                ("Global", vec![
                    ("Ctrl+S", "Save diagram"),
                    ("Ctrl+Z", "Undo last action"),
                    ("Ctrl+Y / Ctrl+Shift+Z", "Redo undone action"),
                    ("Ctrl+R", "Cycle orientation (flowchart) / Reload (canvas)"),
                    ("Ctrl+G", "Toggle background grid"),
                    ("Ctrl+E", "Toggle raw editor pane"),
                    ("Ctrl+X", "Toggle external editor mode"),
                    ("Ctrl+L", "Toggle spatial lock"),
                    ("Ctrl+O", "Toggle orthogonal arrows (canvas only)"),
                    ("Alt+Enter", "Cycle pane focus"),
                ]),
                ("Other", vec![
                    ("?", "Toggle this help"),
                    ("Esc / q", "Dismiss / deselect / quit"),
                ]),
            ];

            let mut lines = Vec::new();
            for (i, (heading, items)) in sections.into_iter().enumerate() {
                if i > 0 {
                    lines.push(separator.clone());
                }
                lines.push(Line::from(vec![
                    Span::styled(format!("  {}", heading), heading_style),
                ]));
                lines.push(Line::from(""));
                for (key, desc) in items {
                    lines.push(Line::from(vec![
                        Span::styled(format!("  {:30}", key), key_style),
                        Span::raw(" "),
                        Span::styled(desc, desc_style),
                    ]));
                }
            }
            lines
        }

        PinstarHelpTab::Mouse => {
            let sections: Vec<(&str, Vec<(&str, &str)>)> = vec![
                ("Left Click", vec![
                    ("Click", "Select node"),
                    ("Double-click", "Edit node content"),
                    ("Drag (on node)", "Move node"),
                    ("Drag (empty)", "Pan canvas"),
                ]),
                ("Right Click", vec![
                    ("On node", "Node context menu"),
                    ("On edge", "Edge context menu"),
                    ("On empty space", "Add node menu"),
                    ("Drag", "Selection rectangle (multi-select)"),
                ]),
                ("Other", vec![
                    ("Middle-click drag", "Pan canvas"),
                    ("Scroll", "Zoom in / out"),
                    ("Scroll (editor)", "Scroll editor content"),
                ]),
            ];

            let mut lines = Vec::new();
            for (i, (heading, items)) in sections.into_iter().enumerate() {
                if i > 0 {
                    lines.push(separator.clone());
                }
                lines.push(Line::from(vec![
                    Span::styled(format!("  {}", heading), heading_style),
                ]));
                lines.push(Line::from(""));
                for (action, desc) in items {
                    lines.push(Line::from(vec![
                        Span::styled(format!("  {:30}", action), key_style),
                        Span::raw(" "),
                        Span::styled(desc, desc_style),
                    ]));
                }
            }
            lines
        }

        PinstarHelpTab::Menus => {
            let sections: Vec<(&str, Vec<(&str, &str)>)> = vec![
                ("Node Context Menu (right-click node)", vec![
                    ("Create Connection", "Start connection to another node"),
                    ("Delete Connection", "Remove connection to another node"),
                    ("Rename Node", "Change node ID / name"),
                    ("Resize Node", "Drag to resize"),
                    ("Set Shape", "Pick geometric shape"),
                    ("Set Color", "Pick color preset"),
                    ("Delete All Connections", "Remove all edges"),
                    ("Delete Node", "Remove node"),
                ]),
                ("Add Menu (right-click empty)", vec![
                    ("Add Text Node", "Create new empty text node"),
                    ("Add Group", "Create group container (canvas only)"),
                ]),
                ("Edge Context Menu (right-click edge)", vec![
                    ("Set Color", "Pick edge color preset"),
                    ("Set Style", "Solid / Dashed / Dotted"),
                ]),
                ("Editor Context Menu", vec![
                    ("Copy / Cut / Paste", "Clipboard operations"),
                    ("Select All", "Select all text"),
                ]),
                ("Shape Picker", vec![
                    ("Rectangle", "Default rectangular box"),
                    ("Diamond", "Diamond / decision shape"),
                    ("Circle", "Circular shape"),
                    ("Cylinder", "Database / cylinder shape"),
                    ("Stadium", "Rounded pill shape"),
                ]),
                ("Color Picker", vec![
                    ("10 presets", "Default, Red, Orange, Yellow, Green,\n                       Cyan, Blue, Purple, Magenta, White"),
                ]),
                ("Edge Style Picker", vec![
                    ("Solid", "Continuous line"),
                    ("Dashed", "Dashed line"),
                    ("Dotted", "Dotted line"),
                ]),
                ("Orientation Picker (flowchart)", vec![
                    ("Top-Down", "Default top→bottom"),
                    ("Left-Right", "Left→right flow"),
                    ("Right-Left", "Right→left flow"),
                    ("Bottom-Up", "Bottom→top flow"),
                ]),
            ];

            let mut lines = Vec::new();
            for (i, (heading, items)) in sections.into_iter().enumerate() {
                if i > 0 {
                    lines.push(separator.clone());
                }
                lines.push(Line::from(vec![
                    Span::styled(format!("  {}", heading), heading_style),
                ]));
                lines.push(Line::from(""));
                for (action, desc) in items {
                    lines.push(Line::from(vec![
                        Span::styled(format!("  {:30}", action), key_style),
                        Span::raw(" "),
                        Span::styled(desc, desc_style),
                    ]));
                }
            }
            lines
        }

        PinstarHelpTab::Formats => {
            let sections: Vec<(&str, Vec<(&str, &str)>)> = vec![
                ("Obsidian Canvas  (.canvas)", vec![
                    ("Layout", "Free-form, force-directed"),
                    ("Node types", "Text, File, Link, Group"),
                    ("Shapes", "Rectangle only"),
                    ("Colors", "10 color presets"),
                    ("Edges", "Solid (braille or orthogonal toggle)"),
                    ("Arrow style", "Ctrl+O toggles orthogonal routing"),
                ]),
                ("Mermaid Flowchart  (.md, .mermaid)", vec![
                    ("Layout", "Hierarchical (auto)"),
                    ("Node types", "Text only"),
                    ("Shapes", "Rectangle, Diamond, Circle, Cylinder, Stadium"),
                    ("Colors", "Not editable in Mermaid syntax"),
                    ("Edges", "Solid, Dashed, Dotted"),
                    ("Syntax", "graph TD / graph LR flowchart syntax"),
                    ("Note", "Groups not supported"),
                ]),
                ("Graphviz DOT  (.dot, .gv)", vec![
                    ("Layout", "Hierarchical (auto)"),
                    ("Node types", "Text only"),
                    ("Shapes", "Rectangle, Diamond, Circle, Cylinder, Stadium"),
                    ("Colors", "10 color presets"),
                    ("Edges", "Solid, Dashed, Dotted"),
                    ("Syntax", "digraph G { ... } DOT syntax"),
                ]),
                ("PlantUML  (.puml, .plantuml)", vec![
                    ("Layout", "Hierarchical (auto)"),
                    ("Node types", "Text only"),
                    ("Shapes", "Rectangle, Circle, Cylinder (no Diamond/Stadium)"),
                    ("Colors", "Not editable in PlantUML syntax"),
                    ("Edges", "Solid, Dashed, Dotted"),
                    ("Syntax", "@startuml / @enduml activity syntax"),
                ]),
            ];

            let mut lines = Vec::new();
            for (i, (heading, items)) in sections.into_iter().enumerate() {
                if i > 0 {
                    lines.push(separator.clone());
                }
                lines.push(Line::from(vec![
                    Span::styled(format!("  {}", heading), heading_style),
                ]));
                lines.push(Line::from(""));
                for (label, value) in items {
                    lines.push(Line::from(vec![
                        Span::styled(format!("  {:20}", label), key_style),
                        Span::raw(" "),
                        Span::styled(value, desc_style),
                    ]));
                }
            }

            lines.push(separator);
            lines.push(Line::from(""));
            lines.push(Line::from(vec![
                Span::styled("  Limitations:", heading_style),
            ]));
            lines.push(Line::from(""));
            lines.push(Line::from(vec![
                Span::styled("  • ", note_style),
                Span::styled("No image rendering in terminal", desc_style),
            ]));
            lines.push(Line::from(vec![
                Span::styled("  • ", note_style),
                Span::styled("Node titles may not be preserved across all formats", desc_style),
            ]));
            lines.push(Line::from(vec![
                Span::styled("  • ", note_style),
                Span::styled("Diamond/Stadium shapes not available in PlantUML", desc_style),
            ]));
            lines.push(Line::from(vec![
                Span::styled("  • ", note_style),
                Span::styled("Color editing not available in Mermaid or PlantUML", desc_style),
            ]));
            lines
        }
    }
}

/// Returns the total number of content lines for a tab (for scroll bounds).
pub fn help_content_height(tab: PinstarHelpTab) -> u16 {
    // Approximate: count entries. Accurate enough for scroll clamping.
    // The actual rendering will use the theme, so we provide generous estimates.
    match tab {
        PinstarHelpTab::Keyboard => 38,
        PinstarHelpTab::Mouse => 18,
        PinstarHelpTab::Menus => 44,
        PinstarHelpTab::Formats => 38,
    }
}
