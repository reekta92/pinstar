//! Bounded, read-only Mermaid flowchart previews. No files, callbacks or network I/O.
//!
//! The grammar and semantic model come from `merman-core`; the interactive canvas
//! schema is deliberately not used as an intermediate representation, since it
//! cannot retain all Mermaid shapes, markers, classes and subgraph relationships.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::sync::OnceLock;

use anyhow::{Result, ensure};
use merman_core::diagrams::flowchart::{FlowEdge, FlowchartV2Model};
use merman_core::{Engine, ParseOptions};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use regex::Regex;
use unicode_segmentation::UnicodeSegmentation;

const MAX_SOURCE_BYTES: usize = 64 * 1024;
const MAX_NODES: usize = 128;
const MAX_EDGES: usize = 256;
const MAX_GROUPS: usize = 32;
const MAX_DEPTH: usize = 16;
const MAX_AMPERSANDS: usize = 32;
const MAX_OPENERS: usize = 256;
const MAX_ROWS: u16 = 512;
const MAX_COLS: u16 = 240;

/// Styled terminal rows plus explicit explanations of visual substitutions.
/// Callers own caching, source positions and scrolling. Rendering never edits source.
#[derive(Debug)]
pub struct MermaidPreview {
    pub lines: Vec<Line<'static>>,
    pub notices: Vec<String>,
}

/// Parse a complete Mermaid flowchart literal and render it at terminal-cell width.
///
/// Non-flowcharts, malformed input and diagrams exceeding the bounded preview area
/// return an error. The caller should retain the fenced source with a diagnostic.
/// Browser-only features are represented as text, never executed or fetched.
pub fn render_mermaid(source: &str, columns: u16) -> Result<MermaidPreview> {
    ensure!(
        source.len() <= MAX_SOURCE_BYTES,
        "Mermaid source exceeds 64 KiB preview limit"
    );
    ensure!(columns >= 8, "Mermaid preview needs at least 8 columns");
    validate_source_complexity(source)?;
    static ENGINE: OnceLock<Engine> = OnceLock::new();
    let engine = ENGINE.get_or_init(Engine::default);
    let metadata = engine
        .parse_metadata_sync(source, ParseOptions::strict())?
        .ok_or_else(|| anyhow::anyhow!("Empty Mermaid diagram"))?;
    ensure!(
        metadata.diagram_type.starts_with("flowchart"),
        "Only Mermaid flowcharts are supported in previews"
    );
    let parsed = engine
        .parse_diagram_sync(source, ParseOptions::strict())?
        .ok_or_else(|| anyhow::anyhow!("Empty Mermaid diagram"))?;
    ensure!(
        parsed.meta.diagram_type.starts_with("flowchart"),
        "Only Mermaid flowcharts are supported in previews"
    );
    let title = parsed.meta.title.as_deref().map(plain_label);
    let configured = parsed
        .meta
        .config
        .as_value()
        .as_object()
        .is_some_and(|config| !config.is_empty());
    let model: FlowchartV2Model = serde_json::from_value(parsed.model)?;
    ensure!(
        model.nodes.len() <= MAX_NODES
            && model.edges.len() <= MAX_EDGES
            && model.subgraphs.len() <= MAX_GROUPS,
        "Mermaid diagram exceeds preview limit (128 nodes, 256 edges, 32 subgraphs)"
    );
    let columns = columns.min(MAX_COLS);
    let mut renderer = Renderer::new(&model, columns)?;
    let scene = renderer.container(None, model.direction.as_deref().unwrap_or("TB"), 0)?;
    ensure!(
        scene.width <= columns && scene.height <= MAX_ROWS,
        "Mermaid diagram exceeds available preview area"
    );
    let mut buffer = Buffer::empty(Rect::new(0, 0, columns, scene.height.max(1)));
    for item in &scene.boxes {
        if let Kind::Group(index) = item.kind {
            let group = &model.subgraphs[index];
            draw_box(
                &mut buffer,
                item.rect,
                &plain_label(&group.title),
                Style::default()
                    .fg(Color::DarkGray)
                    .patch(renderer.css(&group.classes, &group.styles)),
                "group",
            );
        }
    }
    let bounds: BTreeMap<_, _> = scene
        .boxes
        .iter()
        .map(|item| (item.id.as_str(), item.rect))
        .collect();
    for edge in &model.edges {
        let from = bounds
            .get(edge.from.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing edge source {}", edge.from))?;
        let to = bounds
            .get(edge.to.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing edge target {}", edge.to))?;
        let properties: Vec<_> = model
            .edge_defaults
            .as_ref()
            .into_iter()
            .flat_map(|defaults| &defaults.style)
            .chain(&edge.style)
            .cloned()
            .collect();
        let style = renderer.css(&edge.classes, &properties);
        let horizontal = from.x.abs_diff(to.x) > from.y.abs_diff(to.y);
        draw_edge(&mut buffer, *from, *to, edge, horizontal, style);
        if edge.interpolate.is_some() || edge.length > 1 {
            renderer.notice("Curves and link lengths use compact orthogonal routing");
        }
        if edge.animate == Some(true) || edge.animation.is_some() {
            renderer.notice("Animated edges are shown as static connections");
        }
    }
    for item in &scene.boxes {
        if let Kind::Node(index) = item.kind {
            let node = &model.nodes[index];
            let label = renderer.label(index);
            let style = renderer.css(&node.classes, &node.styles);
            draw_box(
                &mut buffer,
                item.rect,
                &label,
                style,
                node.layout_shape.as_deref().unwrap_or("squareRect"),
            );
        }
    }
    let mut lines = buffer_lines(&buffer);
    if let Some(title) = title {
        let mut heading: Vec<_> = wrap_text(&title, columns as usize)
            .into_iter()
            .map(|line| Line::styled(line, Style::default().add_modifier(Modifier::BOLD)))
            .collect();
        heading.append(&mut lines);
        lines = heading;
    }
    if configured {
        renderer.notice(
            "Mermaid configuration is parsed; browser themes and geometry use terminal equivalents",
        );
    }
    // Edge labels stay complete even when a routing lane is too narrow. This also
    // makes parallel edges, self-loops, and their endpoint types unambiguous.
    for edge in &model.edges {
        if edge.stroke.as_deref() == Some("invisible") {
            continue;
        }
        let label = edge.label.as_deref().map(plain_label).unwrap_or_default();
        if !label.is_empty()
            || edge.from == edge.to
            || model
                .edges
                .iter()
                .filter(|other| {
                    (other.from == edge.from && other.to == edge.to)
                        || (other.from == edge.to && other.to == edge.from)
                })
                .count()
                > 1
        {
            let text = format!(
                "{} {} {}{}",
                plain_label(&edge.from),
                edge_symbol(edge),
                plain_label(&edge.to),
                if label.is_empty() {
                    String::new()
                } else {
                    format!(": {label}")
                }
            );
            lines.extend(
                wrap_text(&text, columns as usize)
                    .into_iter()
                    .map(Line::raw),
            );
        }
    }
    for node in &model.nodes {
        if let Some(link) = &node.link {
            lines.extend(
                wrap_text(
                    &format!("{}: link {}", plain_label(&node.id), plain_label(link)),
                    columns as usize,
                )
                .into_iter()
                .map(Line::raw),
            );
            renderer.notice("Links are shown as text; callbacks never execute");
        }
        if node.have_callback {
            renderer.notice("Links are shown as text; callbacks never execute");
        }
        if let Some(tooltip) = model.tooltips.get(&node.id) {
            lines.extend(
                wrap_text(
                    &format!("{}: {}", plain_label(&node.id), plain_label(tooltip)),
                    columns as usize,
                )
                .into_iter()
                .map(Line::raw),
            );
        }
    }
    // Group headers have one line of space. Keep complete long/multiline titles
    // in a legend instead of clipping semantic text or putting LF into a cell.
    for group in &model.subgraphs {
        let title = plain_label(&group.title);
        if let Some(rect) = bounds.get(group.id.as_str()) {
            if title.contains('\n')
                || Line::raw(title.clone()).width() > usize::from(rect.width.saturating_sub(4))
            {
                lines.extend(
                    wrap_text(
                        &format!("{}: {title}", plain_label(&group.id)),
                        usize::from(columns),
                    )
                    .into_iter()
                    .map(Line::raw),
                );
            }
        }
    }
    for text in [&model.acc_title, &model.acc_descr].into_iter().flatten() {
        lines.extend(
            wrap_text(&plain_label(text), columns as usize)
                .into_iter()
                .map(Line::raw),
        );
    }
    if model.edge_defaults.is_some() {
        renderer.notice("Edge defaults and CSS are approximated by terminal styles");
    }
    ensure!(
        lines.len() <= MAX_ROWS as usize,
        "Mermaid output exceeds 512-row preview limit"
    );
    Ok(MermaidPreview {
        lines,
        notices: renderer.notices.into_iter().collect(),
    })
}

// The semantic parser expands grouped links before returning its model. This
// preflight is not a replacement grammar. Raw-marker budgets are conservative:
// labels/comments also count, so quote-state tricks cannot hide parser work.
fn validate_source_complexity(source: &str) -> Result<()> {
    static FRONTMATTER: OnceLock<Regex> = OnceLock::new();
    static GROUP: OnceLock<Regex> = OnceLock::new();
    // Match the pinned parser's CR normalization and frontmatter extraction.
    let normalized = source.replace("\r\n", "\n").replace('\r', "\n");
    let source = normalized.trim_start();
    let body = if source.starts_with("---") {
        let captures = FRONTMATTER
            .get_or_init(|| {
                Regex::new(r"(?s)^-{3}\s*[\n\r](.*?)[\n\r]-{3}\s*[\n\r]+")
                    .expect("Mermaid frontmatter")
            })
            .captures(source)
            .ok_or_else(|| anyhow::anyhow!("Unterminated Mermaid frontmatter"))?;
        validate_yaml_configuration(captures.get(1).expect("YAML capture").as_str())?;
        &source[captures.get(0).expect("frontmatter capture").end()..]
    } else {
        source
    };
    ensure!(
        body.chars().filter(|ch| *ch == '&').count() <= MAX_AMPERSANDS,
        "Mermaid source exceeds 32-ampersand parser budget (labels/comments also count)"
    );
    ensure!(
        body.chars()
            .filter(|ch| matches!(ch, '[' | '(' | '{'))
            .count()
            <= MAX_OPENERS,
        "Mermaid source exceeds 256-opening-delimiter parser budget (labels/comments also count)"
    );
    ensure!(
        GROUP
            .get_or_init(|| Regex::new(r"(?i)\bsubgraph\b").expect("subgraph keyword"))
            .find_iter(body)
            .count()
            <= MAX_GROUPS,
        "Mermaid source exceeds 32-subgraph-keyword parser budget (labels/comments also count)"
    );
    Ok(())
}

fn validate_yaml_configuration(source: &str) -> Result<()> {
    use yaml_rust2::scanner::{Scanner, TokenType};
    static TAG: OnceLock<Regex> = OnceLock::new();
    static ATTRIBUTE: OnceLock<Regex> = OnceLock::new();
    static ENTITY: OnceLock<Regex> = OnceLock::new();
    // The pinned parser rewrites HTML attribute quotes and encodes #word;
    // entities before YAML loading. Scan that spelling too: raw YAML comments
    // or quoted scalars must not hide anchors exposed by preprocessing.
    let transformed = TAG
        .get_or_init(|| Regex::new(r"<(\w+)([^>]*)>").expect("HTML tag"))
        .replace_all(source, |captures: &regex::Captures| {
            let attributes = ATTRIBUTE
                .get_or_init(|| Regex::new("=\"([^\"]*)\"").expect("HTML attribute"))
                .replace_all(&captures[2], "='$1'");
            format!("<{}{attributes}>", &captures[1])
        });
    let transformed = ENTITY
        .get_or_init(|| Regex::new(r"#\w+;").expect("Mermaid entity"))
        .replace_all(&transformed, "MERMAID_ENTITY");
    for spelling in [source, transformed.as_ref()] {
        let mut scanner = Scanner::new(spelling.chars());
        let mut depth = 0_usize;
        for token in scanner.by_ref() {
            match token.1 {
                TokenType::Alias(_) | TokenType::Anchor(_) => {
                    anyhow::bail!("YAML aliases/anchors are disabled in bounded previews")
                }
                TokenType::BlockSequenceStart
                | TokenType::BlockMappingStart
                | TokenType::FlowSequenceStart
                | TokenType::FlowMappingStart => {
                    depth += 1;
                    ensure!(
                        depth <= MAX_DEPTH,
                        "Mermaid YAML nesting exceeds 16-level parser budget"
                    );
                }
                TokenType::BlockEnd | TokenType::FlowSequenceEnd | TokenType::FlowMappingEnd => {
                    depth = depth.saturating_sub(1);
                }
                _ => {}
            }
        }
        if let Some(error) = scanner.get_error() {
            anyhow::bail!("Invalid Mermaid YAML: {error}");
        }
    }
    Ok(())
}

#[derive(Clone, Copy)]
enum Kind {
    Node(usize),
    Group(usize),
}
struct Item {
    id: String,
    kind: Kind,
    rect: Rect,
}
struct Scene {
    width: u16,
    height: u16,
    boxes: Vec<Item>,
}

struct Renderer<'a> {
    model: &'a FlowchartV2Model,
    columns: u16,
    kinds: BTreeMap<String, Kind>,
    parents: BTreeMap<String, String>,
    notices: BTreeSet<String>,
}

impl<'a> Renderer<'a> {
    fn new(model: &'a FlowchartV2Model, columns: u16) -> Result<Self> {
        let mut kinds = BTreeMap::new();
        for (index, node) in model.nodes.iter().enumerate() {
            kinds.insert(node.id.clone(), Kind::Node(index));
        }
        for (index, group) in model.subgraphs.iter().enumerate() {
            kinds.insert(group.id.clone(), Kind::Group(index));
        }
        let mut parents = BTreeMap::new();
        for group in &model.subgraphs {
            for child in &group.nodes {
                ensure!(kinds.contains_key(child), "Unknown subgraph member {child}");
                ensure!(child != &group.id, "Recursive Mermaid subgraph");
                parents.insert(child.clone(), group.id.clone());
            }
        }
        // Validate every chain, including components disconnected from the root.
        for id in kinds.keys() {
            let mut seen = BTreeSet::new();
            let mut current = id;
            while let Some(parent) = parents.get(current) {
                ensure!(seen.insert(parent), "Recursive Mermaid subgraph");
                ensure!(
                    seen.len() <= MAX_DEPTH,
                    "Mermaid subgraph nesting exceeds 16 levels"
                );
                current = parent;
            }
        }
        Ok(Self {
            model,
            columns,
            kinds,
            parents,
            notices: BTreeSet::new(),
        })
    }

    fn notice(&mut self, text: &str) {
        self.notices.insert(text.to_owned());
    }

    fn label(&mut self, index: usize) -> String {
        let node = &self.model.nodes[index];
        let raw_label = node.label.as_deref().unwrap_or(&node.id);
        let mut label = plain_label(raw_label);
        let shape = node.layout_shape.as_deref().unwrap_or("squareRect");
        if !matches!(
            shape,
            "squareRect"
                | "rect"
                | "roundedRect"
                | "round"
                | "stadium"
                | "circle"
                | "doublecircle"
                | "diamond"
                | "cylinder"
                | "subroutine"
        ) {
            label = format!("[{}] {label}", plain_label(shape));
            self.notice("Specialized shapes use labelled terminal outlines");
        }
        if let Some(icon) = &node.icon {
            label.push_str(&format!(" [icon: {}]", plain_label(icon)));
        }
        if node.img.is_some() {
            label.push_str(" [image]");
        }
        if node.icon.is_some()
            || node.img.is_some()
            || raw_label.to_ascii_lowercase().contains("<img")
        {
            self.notice("Icons and images use text placeholders; no assets are fetched");
        }
        if node
            .label_type
            .as_deref()
            .is_some_and(|kind| kind != "text")
            || label.contains("fa:")
            || (raw_label.contains('<') && raw_label.contains('>'))
        {
            self.notice("HTML, Markdown and icon labels use safe text equivalents");
        }
        label
    }

    fn direct_child(&self, id: &str, parent: Option<&str>) -> Option<String> {
        let mut child = id;
        while self.parents.get(child).map(String::as_str) != parent {
            child = self.parents.get(child)?.as_str();
        }
        Some(child.to_owned())
    }

    fn container(&mut self, parent: Option<&str>, direction: &str, depth: usize) -> Result<Scene> {
        ensure!(
            depth <= MAX_DEPTH,
            "Mermaid subgraph nesting exceeds 16 levels"
        );
        let ids: Vec<_> = self
            .kinds
            .keys()
            .filter(|id| self.parents.get(*id).map(String::as_str) == parent)
            .cloned()
            .collect();
        if ids.is_empty() {
            return Ok(Scene {
                width: 6,
                height: 3,
                boxes: Vec::new(),
            });
        }
        let mut adjacency: BTreeMap<String, BTreeSet<String>> =
            ids.iter().map(|id| (id.clone(), BTreeSet::new())).collect();
        let mut indegree: BTreeMap<String, usize> = ids.iter().map(|id| (id.clone(), 0)).collect();
        for edge in &self.model.edges {
            if let (Some(from), Some(to)) = (
                self.direct_child(&edge.from, parent),
                self.direct_child(&edge.to, parent),
            ) {
                if from != to
                    && adjacency
                        .get_mut(&from)
                        .is_some_and(|set| set.insert(to.clone()))
                {
                    *indegree
                        .get_mut(&to)
                        .ok_or_else(|| anyhow::anyhow!("Unknown flowchart endpoint"))? += 1;
                }
            }
        }
        let mut ranks: BTreeMap<String, usize> = ids.iter().map(|id| (id.clone(), 0)).collect();
        let mut queue: VecDeque<_> = ids
            .iter()
            .filter(|id| indegree[*id] == 0)
            .cloned()
            .collect();
        let mut visited = BTreeSet::new();
        while let Some(id) = queue.pop_front() {
            visited.insert(id.clone());
            let rank = ranks[&id];
            for target in &adjacency[&id] {
                let next = ranks.get_mut(target).expect("validated endpoint");
                *next = (*next).max(rank + 1);
                let count = indegree.get_mut(target).expect("validated endpoint");
                *count -= 1;
                if *count == 0 {
                    queue.push_back(target.clone());
                }
            }
        }
        // Cycles are valid flowcharts: place remaining components deterministically,
        // without iterating ranks indefinitely or dropping any connection.
        let mut cycle_rank = ranks.values().copied().max().unwrap_or(0) + 1;
        for id in &ids {
            if !visited.contains(id) {
                ranks.insert(id.clone(), cycle_rank);
                cycle_rank += 1;
            }
        }
        let mut units = BTreeMap::new();
        for id in &ids {
            let kind = self.kinds[id];
            let scene = match kind {
                Kind::Node(index) => {
                    let label = self.label(index);
                    let width = (Line::raw(label.clone()).width() + 4)
                        .clamp(8, self.columns as usize) as u16;
                    let rows = wrap_text(&label, width.saturating_sub(4).max(1) as usize).len();
                    ensure!(
                        rows <= MAX_ROWS as usize - 2,
                        "Mermaid node label exceeds preview height"
                    );
                    let height = rows as u16 + 2;
                    Scene {
                        width,
                        height,
                        boxes: vec![Item {
                            id: id.clone(),
                            kind,
                            rect: Rect::new(0, 0, width, height),
                        }],
                    }
                }
                Kind::Group(index) => {
                    let group = &self.model.subgraphs[index];
                    let dir = group.dir.as_deref().unwrap_or(direction).to_owned();
                    let mut child = self.container(Some(id), &dir, depth + 1)?;
                    let title_width = Line::raw(plain_label(&group.title)).width() + 4;
                    child.width = child
                        .width
                        .saturating_add(4)
                        .max(title_width.min(self.columns as usize) as u16);
                    child.height = child.height.saturating_add(4);
                    for item in &mut child.boxes {
                        item.rect.x += 2;
                        item.rect.y += 2;
                    }
                    child.boxes.insert(
                        0,
                        Item {
                            id: id.clone(),
                            kind,
                            rect: Rect::new(0, 0, child.width, child.height),
                        },
                    );
                    child
                }
            };
            ensure!(
                scene.height <= MAX_ROWS && scene.width <= self.columns,
                "Mermaid diagram exceeds available preview area"
            );
            units.insert(id.clone(), scene);
        }
        let horizontal = matches!(direction, "LR" | "RL");
        let mut bands: BTreeMap<usize, Vec<String>> = BTreeMap::new();
        for id in &ids {
            bands.entry(ranks[id]).or_default().push(id.clone());
        }
        let mut scene = Scene {
            width: 0,
            height: 0,
            boxes: Vec::new(),
        };
        let mut major = 0;
        for band in bands.values() {
            let mut minor = 0;
            let mut thickness = 0;
            for id in band {
                let mut child = units.remove(id).expect("layout unit");
                let (x, y) = if horizontal {
                    (major, minor)
                } else {
                    (minor, major)
                };
                ensure!(
                    x <= self.columns.saturating_sub(child.width)
                        && y <= MAX_ROWS.saturating_sub(child.height),
                    "Mermaid diagram exceeds available preview area"
                );
                for item in &mut child.boxes {
                    item.rect.x += x;
                    item.rect.y += y;
                }
                scene.width = scene.width.max(x + child.width);
                scene.height = scene.height.max(y + child.height);
                minor += if horizontal {
                    child.height + 3
                } else {
                    child.width + 3
                };
                thickness = thickness.max(if horizontal {
                    child.width
                } else {
                    child.height
                });
                scene.boxes.extend(child.boxes);
            }
            major += thickness + 3;
            ensure!(
                scene.width <= self.columns && scene.height <= MAX_ROWS,
                "Mermaid diagram exceeds available preview area"
            );
        }
        if matches!(direction, "RL" | "BT") {
            // Reverse sibling units, not their contents: a nested group's local
            // direction must not be mirrored by the orientation of its parent.
            let units: BTreeMap<_, _> = scene
                .boxes
                .iter()
                .filter(|item| self.parents.get(&item.id).map(String::as_str) == parent)
                .map(|item| (item.id.clone(), item.rect))
                .collect();
            for item in &mut scene.boxes {
                let unit = units[&self
                    .direct_child(&item.id, parent)
                    .expect("validated hierarchy")];
                if horizontal {
                    item.rect.x = scene.width - unit.x - unit.width + (item.rect.x - unit.x);
                } else {
                    item.rect.y = scene.height - unit.y - unit.height + (item.rect.y - unit.y);
                }
            }
        }
        Ok(scene)
    }

    fn css(&mut self, classes: &[String], inline: &[String]) -> Style {
        let mut style = Style::default();
        let properties: Vec<_> = std::iter::once("default")
            .chain(classes.iter().map(String::as_str))
            .filter_map(|class| self.model.class_defs.get(class))
            .flatten()
            .chain(inline)
            .cloned()
            .collect();
        for property in properties {
            for declaration in property.split(';') {
                let Some((key, value)) = declaration.split_once(':') else {
                    continue;
                };
                match key.trim() {
                    "color" | "stroke" => {
                        if let Some(color) = parse_color(value.trim()) {
                            style = style.fg(color);
                        } else {
                            self.notice("Unsupported CSS colors use terminal defaults");
                        }
                    }
                    "fill" => {
                        if let Some(color) = parse_color(value.trim()) {
                            style = style.bg(color);
                        } else {
                            self.notice("Unsupported CSS colors use terminal defaults");
                        }
                    }
                    "font-weight" if matches!(value.trim(), "bold" | "700") => {
                        style = style.add_modifier(Modifier::BOLD)
                    }
                    "font-style" if value.trim() == "italic" => {
                        style = style.add_modifier(Modifier::ITALIC)
                    }
                    "text-decoration" if value.trim() == "underline" => {
                        style = style.add_modifier(Modifier::UNDERLINED)
                    }
                    _ => self.notice(
                        "CSS geometry, curves and unsupported properties use terminal defaults",
                    ),
                }
            }
        }
        style
    }
}

fn parse_color(value: &str) -> Option<Color> {
    let Some(hex) = value.strip_prefix('#') else {
        return value.parse().ok();
    };
    let hex = if hex.len() == 3 {
        hex.chars().flat_map(|ch| [ch, ch]).collect::<String>()
    } else {
        hex.to_owned()
    };
    let rgb = u32::from_str_radix(&hex, 16).ok()?;
    (hex.len() == 6).then_some(Color::Rgb((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8))
}

fn plain_label(text: &str) -> String {
    static BREAKS: OnceLock<Regex> = OnceLock::new();
    static TAGS: OnceLock<Regex> = OnceLock::new();
    let text = BREAKS
        .get_or_init(|| Regex::new(r"(?i)<br\s*/?>").expect("label breaks"))
        .replace_all(text, "\n");
    static IMAGES: OnceLock<Regex> = OnceLock::new();
    let text = IMAGES
        .get_or_init(|| Regex::new(r"(?i)<img\b[^>]*>").expect("HTML image labels"))
        .replace_all(&text, "[image]");
    let text = TAGS
        .get_or_init(|| Regex::new(r"</?[A-Za-z][^>]*>|<!--(?s:.*?)-->").expect("label tags"))
        .replace_all(&text, "");
    static ENTITIES: OnceLock<Regex> = OnceLock::new();
    let text = ENTITIES
        .get_or_init(|| Regex::new(r"#([0-9]+|[A-Za-z][A-Za-z0-9]+);").expect("Mermaid entities"))
        .replace_all(&text, |captures: &regex::Captures<'_>| {
            let entity = &captures[1];
            if let Ok(number) = entity.parse::<u32>() {
                return char::from_u32(number)
                    .map_or_else(|| captures[0].to_owned(), |ch| ch.to_string());
            }
            let encoded = format!("&{entity};");
            let decoded = htmlize::unescape_attribute(encoded.as_str());
            if decoded == encoded {
                captures[0].to_owned()
            } else {
                decoded.into_owned()
            }
        });
    htmlize::unescape(text.as_ref())
        .chars()
        .filter(|ch| !ch.is_control() || matches!(*ch, '\n' | '\t'))
        .map(|ch| if ch == '\t' { ' ' } else { ch })
        .collect()
}

fn wrap_text(text: &str, width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    for physical in text.split('\n') {
        let mut row = String::new();
        let mut used = 0;
        for grapheme in physical.graphemes(true) {
            let cells = Span::raw(grapheme).width();
            if used + cells > width && !row.is_empty() {
                lines.push(std::mem::take(&mut row));
                used = 0;
            }
            row.push_str(grapheme);
            used += cells;
        }
        lines.push(row);
    }
    lines
}

fn put(buffer: &mut Buffer, x: u16, y: u16, ch: char, style: Style) {
    if x < buffer.area.width && y < buffer.area.height {
        buffer[(x, y)].set_char(ch).set_style(style);
    }
}

fn draw_box(buffer: &mut Buffer, rect: Rect, label: &str, style: Style, shape: &str) {
    if rect.width < 2 || rect.height < 2 {
        return;
    }
    if shape != "group" {
        for y in rect.y..rect.bottom() {
            for x in rect.x..rect.right() {
                put(buffer, x, y, ' ', style);
            }
        }
    }
    let (tl, tr, bl, br) = match shape {
        "roundedRect" | "round" | "stadium" | "circle" | "doublecircle" | "cylinder" => {
            ('╭', '╮', '╰', '╯')
        }
        "diamond" => ('╱', '╲', '╲', '╱'),
        "subroutine" => ('╔', '╗', '╚', '╝'),
        _ => ('┌', '┐', '└', '┘'),
    };
    for x in rect.x + 1..rect.right() - 1 {
        put(buffer, x, rect.y, '─', style);
        put(buffer, x, rect.bottom() - 1, '─', style);
    }
    for y in rect.y + 1..rect.bottom() - 1 {
        put(buffer, rect.x, y, '│', style);
        put(buffer, rect.right() - 1, y, '│', style);
    }
    for (x, y, ch) in [
        (rect.x, rect.y, tl),
        (rect.right() - 1, rect.y, tr),
        (rect.x, rect.bottom() - 1, bl),
        (rect.right() - 1, rect.bottom() - 1, br),
    ] {
        put(buffer, x, y, ch, style);
    }
    if shape == "group" {
        buffer.set_stringn(
            rect.x + 2,
            rect.y,
            label.lines().next().unwrap_or_default(),
            rect.width.saturating_sub(4) as usize,
            style,
        );
    } else {
        for (index, row) in wrap_text(label, rect.width.saturating_sub(4).max(1) as usize)
            .iter()
            .enumerate()
        {
            buffer.set_stringn(
                rect.x + 2,
                rect.y + 1 + index as u16,
                row,
                rect.width.saturating_sub(4) as usize,
                style,
            );
        }
    }
}

fn edge_symbol(edge: &FlowEdge) -> &'static str {
    match edge.edge_type.as_deref() {
        Some("double_arrow_point") => "<-->",
        Some("double_arrow_circle") => "●──●",
        Some("double_arrow_cross") => "×──×",
        Some("arrow_circle") => "──●",
        Some("arrow_cross") => "──×",
        Some("arrow_open") => "---",
        _ => "-->",
    }
}

fn draw_edge(
    buffer: &mut Buffer,
    from: Rect,
    to: Rect,
    edge: &FlowEdge,
    horizontal: bool,
    style: Style,
) {
    if edge.stroke.as_deref() == Some("invisible") {
        return;
    }
    let forward = if horizontal {
        from.x < to.x
    } else {
        from.y < to.y
    };
    let (start, end) = if horizontal {
        (
            (
                if forward {
                    from.right()
                } else {
                    from.x.saturating_sub(1)
                },
                from.y + from.height / 2,
            ),
            (
                if forward {
                    to.x.saturating_sub(1)
                } else {
                    to.right()
                },
                to.y + to.height / 2,
            ),
        )
    } else {
        (
            (
                from.x + from.width / 2,
                if forward {
                    from.bottom()
                } else {
                    from.y.saturating_sub(1)
                },
            ),
            (
                to.x + to.width / 2,
                if forward {
                    to.y.saturating_sub(1)
                } else {
                    to.bottom()
                },
            ),
        )
    };
    let middle = if horizontal {
        ((start.0 as u32 + end.0 as u32) / 2) as u16
    } else {
        ((start.1 as u32 + end.1 as u32) / 2) as u16
    };
    let path = if horizontal {
        [start, (middle, start.1), (middle, end.1), end]
    } else {
        [start, (start.0, middle), (end.0, middle), end]
    };
    let (h, v) = match edge.stroke.as_deref() {
        Some("thick") => ('━', '┃'),
        Some("dotted") => ('┄', '┆'),
        _ => ('─', '│'),
    };
    for segment in path.windows(2) {
        let (a, b) = (segment[0], segment[1]);
        if a.1 == b.1 {
            for x in a.0.min(b.0)..=a.0.max(b.0) {
                put(buffer, x, a.1, h, style);
            }
        } else {
            for y in a.1.min(b.1)..=a.1.max(b.1) {
                put(buffer, a.0, y, v, style);
            }
        }
    }
    for point in &path[1..3] {
        put(buffer, point.0, point.1, '┼', style);
    }
    let kind = edge.edge_type.as_deref().unwrap_or("arrow_point");
    let marker = if kind.contains("circle") {
        '●'
    } else if kind.contains("cross") {
        '×'
    } else if horizontal {
        if forward { '▶' } else { '◀' }
    } else if forward {
        '▼'
    } else {
        '▲'
    };
    if !kind.contains("open") {
        put(buffer, end.0, end.1, marker, style);
    }
    if kind.starts_with("double_") {
        let marker = if kind.contains("circle") {
            '●'
        } else if kind.contains("cross") {
            '×'
        } else if horizontal {
            if forward { '◀' } else { '▶' }
        } else if forward {
            '▲'
        } else {
            '▼'
        };
        put(buffer, start.0, start.1, marker, style);
    }
}

fn buffer_lines(buffer: &Buffer) -> Vec<Line<'static>> {
    (0..buffer.area.height)
        .map(|y| {
            let mut spans: Vec<Span<'static>> = Vec::new();
            let mut text = String::new();
            let mut style = Style::default();
            let mut x = 0;
            while x < buffer.area.width {
                let cell = &buffer[(x, y)];
                let mut next = cell.style();
                // An off-screen buffer's reset colors mean "unspecified" for a
                // preview host; preserve its theme instead of resetting to the
                // terminal palette when these rows are embedded in Markdown.
                if next.fg == Some(Color::Reset) {
                    next.fg = None;
                }
                if next.bg == Some(Color::Reset) {
                    next.bg = None;
                }
                if next != style && !text.is_empty() {
                    spans.push(Span::styled(std::mem::take(&mut text), style));
                }
                style = next;
                text.push_str(cell.symbol());
                x += Span::raw(cell.symbol()).width().max(1) as u16;
            }
            spans.push(Span::styled(text, style));
            Line::from(spans)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    fn text(preview: &MermaidPreview) -> String {
        preview
            .lines
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn renders_chains_branches_classes_and_special_shapes() {
        let preview = render_mermaid("flowchart TB\nA[Start] -->|yes| B{Question}\nB --> C@{shape: doc, label: \"Document\"}\nclassDef red fill:#f00,stroke:#0f0\nclass A red", 80).unwrap();
        let output = text(&preview);
        for label in ["Start", "Question", "Document", "yes", "doc"] {
            assert!(output.contains(label), "missing {label}: {output}");
        }
        assert!(
            preview
                .lines
                .iter()
                .flat_map(|line| &line.spans)
                .any(|span| span.style.bg == Some(Color::Rgb(255, 0, 0)))
        );
    }

    #[test]
    fn nested_subgraphs_and_local_directions() {
        let preview = render_mermaid("flowchart TB\nsubgraph outer[Outer]\nsubgraph inner[Inner]\ndirection LR\nA --> B\nend\nend\nB --> C", 100).unwrap();
        let output = text(&preview);
        for label in ["Outer", "Inner", "A", "B", "C"] {
            assert!(output.contains(label), "{output}");
        }
    }

    #[test]
    fn cycles_parallel_edges_self_loops_and_markers() {
        let preview = render_mermaid(
            "graph TD\nA o--o B\nB x--x A\nA -->|again| A\nA -->|second| B",
            100,
        )
        .unwrap();
        let output = text(&preview);
        assert!(output.contains("again") && output.contains("second") && output.contains("●──●"));
        assert!(output.contains('●') && output.contains('×'));
    }

    #[test]
    fn all_directions_and_unicode_are_bounded() {
        for direction in ["TD", "TB", "BT", "LR", "RL"] {
            let preview = render_mermaid(
                &format!("flowchart {direction}\nA[\"日本語 🦀\"] --> B[Done]"),
                80,
            )
            .unwrap();
            assert!(text(&preview).contains("日本語 🦀"));
            assert!(preview.lines.iter().all(|line| line.width() <= 80));
        }
    }

    #[test]
    fn assets_and_callbacks_are_never_executed() {
        let preview = render_mermaid("flowchart TD\nA@{ img: \"https://invalid.example/image.png\", label: \"Picture\" }\nclick A call dangerous()\n", 80).unwrap();
        assert!(text(&preview).contains("[image]"));
        assert!(
            preview
                .notices
                .iter()
                .any(|notice| notice.contains("no assets"))
        );
    }

    #[test]
    fn invalid_unsupported_and_oversized_input_returns_error() {
        for source in [
            "flowchart TD\nA[broken",
            "sequenceDiagram\nA->>B: hello",
            "",
        ] {
            assert!(render_mermaid(source, 80).is_err());
        }
        assert!(render_mermaid(&"a".repeat(MAX_SOURCE_BYTES + 1), 80).is_err());
        assert!(render_mermaid("graph TD\nA-->B", 3).is_err());
    }

    #[test]
    fn syntax_feature_families() {
        let fixtures = [
            "flowchart TB\nA --> B & C\nB & C --> D",
            "flowchart LR\nA -. text .-> B\nB == thick ==> C\nC ~~~ D",
            "flowchart TD\nA e1@--> B\ne1@{ animate: true }",
            "%%{init: {\"theme\": \"dark\"}}%%\nflowchart TD\nA-->B",
            "---\ntitle: Chart title\n---\nflowchart TD\nA-->B",
            "flowchart TD\nA[\"`**Bold** and _italic_`\"] --> B[\"line<br/>two\"]",
            "flowchart TD\nA-->B\nlinkStyle default stroke:#f00,stroke-width:4px",
            "flowchart TD\nsubgraph g[Group]\nA-->B\nend\ng-->C",
        ];
        for source in fixtures {
            let preview =
                render_mermaid(source, 240).unwrap_or_else(|error| panic!("{source}: {error}"));
            assert!(!preview.lines.is_empty());
            if source.contains("Chart title") {
                assert!(text(&preview).contains("Chart title"));
            }
        }
        for shape in [
            "rect",
            "rounded",
            "stadium",
            "diamond",
            "hex",
            "lean-r",
            "lean-l",
            "trap-b",
            "trap-t",
            "circle",
            "dbl-circ",
            "fr-rect",
            "cyl",
            "doc",
            "docs",
            "tag-doc",
            "lin-doc",
            "delay",
            "h-cyl",
            "cloud",
            "bang",
            "brace",
            "brace-r",
            "braces",
            "bolt",
            "tri",
            "f-circ",
            "sm-circ",
            "cross-circ",
            "win-pane",
            "notch-rect",
            "hourglass",
            "div-rect",
            "lin-rect",
            "sl-rect",
            "flag",
            "text",
            "odd",
        ] {
            let source = format!("flowchart TD\nA@{{ shape: {shape}, label: \"Content\" }}");
            assert!(
                text(&render_mermaid(&source, 80).unwrap()).contains("Content"),
                "{shape}"
            );
        }
    }

    #[test]
    fn expansion_and_nested_data_are_bounded_before_semantic_parsing() {
        let group = (0..1000)
            .map(|index| format!("A{index}"))
            .collect::<Vec<_>>()
            .join(" & ");
        assert!(
            render_mermaid(&format!("flowchart TD\n{group} --> {group}"), 80)
                .unwrap_err()
                .to_string()
                .contains("parser budget")
        );
        let source = format!(
            "flowchart TD\n{}A{}",
            "subgraph g\n".repeat(40),
            "\nend".repeat(40)
        );
        assert!(validate_source_complexity(&source).is_err());
        let nested = format!(
            "flowchart TD\nA@{{ unused: {}0{} }}",
            "[".repeat(300),
            "]".repeat(300)
        );
        assert!(validate_source_complexity(&nested).is_err());
        assert!(validate_source_complexity("---\ntitle: &a [*a]\n---\nflowchart TD\nA").is_err());
        let label = "& subgraph *".repeat(100);
        assert!(
            validate_source_complexity(&format!("flowchart TD\nA[\"{label}\"] --> B")).is_err()
        );
        let hidden = format!("flowchart TD\nclassDef c font-family:broken'\n{source}");
        assert!(validate_source_complexity(&hidden).is_err());
        assert!(validate_source_complexity("flowchart TD\nA[unfinished ( text] --> B & C").is_ok());
    }

    #[test]
    fn yaml_alias_scan_handles_plain_quotes_blocks_and_preprocessing() {
        for yaml in [
            "title: foo'bar\nx: &a [*a]",
            "title: 'x\\'\nx: &a [*a]",
            "title: |-\n  '\nx: &a [*a]",
            "#quot;: &a [*a]",
            "unused: [ '<img alt=\": &a [x] #\">'\n]",
        ] {
            assert!(
                validate_source_complexity(&format!("---\n{yaml}\n---\nflowchart TD\nA")).is_err(),
                "{yaml}"
            );
        }
        for yaml in [
            "title: A&B * literal",
            "title: 'foo\\'",
            "title: |\n  quotes ' & *",
            "title: \"quoted & *\"",
        ] {
            assert!(
                validate_source_complexity(&format!("---\n{yaml}\n---\nflowchart TD\nA")).is_ok(),
                "{yaml}"
            );
        }
    }

    #[test]
    fn default_colors_inherit_the_host_theme() {
        let preview = render_mermaid("flowchart TD\nA-->B", 80).unwrap();
        assert!(
            preview
                .lines
                .iter()
                .flat_map(|line| &line.spans)
                .all(|span| span.style.fg.is_none() && span.style.bg.is_none())
        );
    }

    #[test]
    fn reversed_parent_preserves_local_subgraph_direction() {
        let parsed = Engine::default()
            .parse_diagram_sync(
                "flowchart RL\nsubgraph g[Group]\ndirection LR\nA-->B\nend\nX-->g",
                ParseOptions::strict(),
            )
            .unwrap()
            .unwrap();
        let model: FlowchartV2Model = serde_json::from_value(parsed.model).unwrap();
        let scene = Renderer::new(&model, 100)
            .unwrap()
            .container(None, "RL", 0)
            .unwrap();
        let bounds = |id: &str| scene.boxes.iter().find(|item| item.id == id).unwrap().rect;
        assert!(bounds("A").x < bounds("B").x);
        assert!(bounds("g").x < bounds("X").x);
    }

    #[test]
    fn unknown_shape_names_cannot_inject_terminal_controls() {
        let model: FlowchartV2Model = serde_json::from_value(serde_json::json!({
            "nodes": [{"id": "A", "label": "Safe", "layoutShape": "bad\u{1b}[31m"}],
            "edges": []
        }))
        .unwrap();
        let label = Renderer::new(&model, 80).unwrap().label(0);
        assert!(!label.chars().any(char::is_control));
        assert!(label.contains("Safe"));
    }

    #[test]
    fn group_titles_remain_complete_and_cells_have_no_controls() {
        for title in ["a".repeat(200), "First<br/>Second".to_owned()] {
            let source = format!("flowchart TD\nsubgraph g[\"{title}\"]\nA-->B\nend");
            let preview = render_mermaid(&source, 30).unwrap();
            let flat: String = preview.lines.iter().map(ToString::to_string).collect();
            if title.starts_with('a') {
                assert!(flat.contains(&title));
            } else {
                assert!(flat.contains("First") && flat.contains("Second"));
            }
            assert!(
                preview
                    .lines
                    .iter()
                    .all(|line| !line.to_string().chars().any(char::is_control))
            );
        }
    }

    #[test]
    fn huge_label_is_rejected_before_integer_conversion() {
        let source = format!("flowchart TD\nA[\"{}\"]", "x".repeat(60_000));
        assert!(render_mermaid(&source, 8).is_err());
    }

    #[test]
    fn labels_strip_terminal_controls_and_html() {
        assert_eq!(plain_label("<b>hello</b><br/>world\u{1b}"), "hello\nworld");
        assert_eq!(wrap_text("日本語", 4), ["日本", "語"]);
        assert_eq!(wrap_text("👨‍👩‍👧‍👦e\u{301}", 2), ["👨‍👩‍👧‍👦", "e\u{301}"]);
        assert_eq!(plain_label("a\tb"), "a b");
        assert_eq!(plain_label("a < b > c"), "a < b > c");
        assert_eq!(
            plain_label("<img src=\"https://invalid.example/x\">"),
            "[image]"
        );
        assert_eq!(
            plain_label("#9829; #quot; &copy; #notanentity;"),
            "♥ \" © #notanentity;"
        );
    }
}
