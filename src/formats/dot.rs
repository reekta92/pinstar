use crate::data::{CanvasData, CanvasEdge, CanvasNode, TextNode};
use anyhow::{Result, ensure};
use regex::Regex;
use std::collections::BTreeMap;

pub fn parse(content: &str) -> Result<CanvasData> {
    let mut nodes_map = BTreeMap::new();
    let mut edges = Vec::new();
    let mut positions = BTreeMap::new();
    let mut shapes_map = BTreeMap::new();
    let mut colors_map = BTreeMap::new();
    let mut orientation = crate::data::DiagramOrientation::TopDown;

    let meta_re =
        Regex::new(r"^//\s*pinstar_layout:\s+(\S+)\s+(\S+)\s+(\S+)\s+(\S+)\s+(\S+)$").unwrap();
    let node_decl_re = Regex::new(r#"(?s)^([a-zA-Z0-9_\-]+)\s*\[(.*)\]$"#).unwrap();
    let edge_re =
        Regex::new(r#"(?s)^([a-zA-Z0-9_\-]+)\s*->\s*([a-zA-Z0-9_\-]+)(?:\s*\[.*\])?$"#).unwrap();
    let id_regex = Regex::new(r"^[a-zA-Z0-9_\-]+$").unwrap();

    let attribute_re = Regex::new(r#"([a-zA-Z]+)\s*=\s*("(?:\\.|[^"\\])*"|[^\s,]+)"#)?;
    let header_re = Regex::new(r"^digraph(?:\s+[a-zA-Z0-9_]+)?\s*\{$")?;
    let direction_re = Regex::new(r#"(?i)^rankdir\s*=\s*"?(LR|RL|DT|BT|TB|TD)"?$"#)?;
    let normalized = statements(content)?;
    let mut opened = false;
    let mut closed = false;
    for line in normalized.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        // 1. Extract Layout Metadata
        if let Some(caps) = meta_re.captures(trimmed) {
            let id = caps[1].to_string();
            let (x, y, w, h) = super::parse_layout(&caps[2], &caps[3], &caps[4], &caps[5])?;
            positions.insert(id, (x, y, w, h));
            continue;
        }

        if trimmed.starts_with("//") {
            continue;
        }
        if header_re.is_match(trimmed) {
            ensure!(!opened, "Nested/multiple DOT graphs are unsupported");
            opened = true;
            continue;
        }
        if trimmed == "}" {
            ensure!(opened && !closed, "Unexpected DOT closing brace");
            closed = true;
            continue;
        }
        ensure!(opened && !closed, "Expected a DOT digraph body");
        if let Some(caps) = direction_re.captures(trimmed) {
            orientation = match caps[1].to_ascii_uppercase().as_str() {
                "LR" => crate::data::DiagramOrientation::LeftRight,
                "RL" => crate::data::DiagramOrientation::RightLeft,
                "BT" | "DT" => crate::data::DiagramOrientation::DownTop,
                _ => crate::data::DiagramOrientation::TopDown,
            };
            continue;
        }

        // Skip boilerplate or whole-line comments
        ensure!(
            !trimmed.starts_with("graph ") && !trimmed.starts_with("subgraph "),
            "Unsupported DOT graph attributes/subgraphs"
        );

        // 2. Parse Edges: A -> B [label="text"]
        if let Some(caps) = edge_re.captures(trimmed) {
            let attributes = parse_attributes(
                trimmed
                    .split_once('[')
                    .map(|(_, attrs)| attrs.strip_suffix(']').unwrap_or(attrs))
                    .unwrap_or(""),
                &attribute_re,
                &["label", "style", "color"],
            )?;
            {
                let from = caps[1].trim().to_string();
                let to = caps[2].trim().to_string();

                nodes_map
                    .entry(from.clone())
                    .or_insert_with(|| from.clone());
                nodes_map.entry(to.clone()).or_insert_with(|| to.clone());

                let label = attributes.get("label").cloned();
                let style = match attributes.get("style").map(String::as_str) {
                    Some("dashed") => crate::data::EdgeStyle::Dashed,
                    Some("dotted") => crate::data::EdgeStyle::Dotted,
                    Some("bold") => crate::data::EdgeStyle::Thick,
                    Some("solid") | None => crate::data::EdgeStyle::Solid,
                    Some(style) => anyhow::bail!("Unsupported DOT edge style: {style}"),
                };
                let color = attributes.get("color").cloned();

                edges.push(CanvasEdge {
                    id: format!("edge_{}_{}_{}", from, to, edges.len()),
                    from_node: from,
                    to_node: to,
                    from_side: Some("right".to_string()),
                    to_side: Some("left".to_string()),
                    label,
                    color,
                    style,
                });
            }
            continue;
        }

        // 3. Parse Node Declarations: ID [label="Text"]
        if let Some(caps) = node_decl_re.captures(trimmed) {
            let id = caps[1].to_string();
            let attributes = &caps[2];
            ensure!(
                !["node", "edge", "graph"].contains(&id.as_str()),
                "DOT defaults are unsupported"
            );
            let attributes =
                parse_attributes(attributes, &attribute_re, &["label", "shape", "color"])?;
            let label = attributes
                .get("label")
                .cloned()
                .unwrap_or_else(|| id.clone());
            let shape = match attributes.get("shape").map(String::as_str) {
                Some("diamond") => crate::data::NodeShape::Diamond,
                Some("cylinder") => crate::data::NodeShape::Cylinder,
                Some("circle") => crate::data::NodeShape::Circle,
                Some("ellipse") => crate::data::NodeShape::Stadium,
                Some("box" | "rect" | "rectangle") | None => crate::data::NodeShape::Rectangle,
                Some(shape) => anyhow::bail!("Unsupported DOT shape: {shape}"),
            };
            shapes_map.insert(id.clone(), shape);
            if let Some(color) = attributes.get("color") {
                colors_map.insert(id.clone(), color.clone());
            }

            nodes_map.insert(id, label);
            continue;
        }

        // 4. Fallback standalone identifier
        let clean_id = trimmed
            .trim_matches(|c| c == ';' || c == ',' || c == '{' || c == '}')
            .trim();
        if id_regex.is_match(clean_id) {
            nodes_map
                .entry(clean_id.to_string())
                .or_insert_with(|| clean_id.to_string());
            continue;
        }
        anyhow::bail!("Unsupported DOT syntax: {trimmed}");
    }
    ensure!(opened && closed, "Incomplete DOT digraph");

    let mut canvas_nodes = Vec::new();
    for (id, label) in nodes_map {
        let (x, y, w, h) = positions
            .get(&id)
            .copied()
            .unwrap_or((0.0, 0.0, 200.0, 100.0));
        let shape = shapes_map
            .get(&id)
            .copied()
            .unwrap_or(crate::data::NodeShape::Rectangle);
        let color = colors_map.get(&id).cloned();
        canvas_nodes.push(CanvasNode::Text(TextNode {
            id,
            x,
            y,
            width: w,
            height: h,
            text: label,
            title: None,
            color,
            shape,
        }));
    }

    Ok(CanvasData {
        nodes: canvas_nodes,
        edges,
        orientation,
    })
}

fn parse_attributes(
    source: &str,
    regex: &Regex,
    allowed: &[&str],
) -> Result<BTreeMap<String, String>> {
    let mut attributes = BTreeMap::new();
    let mut consumed = 0;
    for caps in regex.captures_iter(source) {
        let found = caps.get(0).unwrap();
        ensure!(
            source[consumed..found.start()]
                .chars()
                .all(|ch| ch.is_whitespace() || ch == ','),
            "Unsupported DOT attributes"
        );
        ensure!(
            allowed.contains(&&caps[1]),
            "Unsupported DOT attribute: {}",
            &caps[1]
        );
        let value = if caps[2].starts_with('"') {
            serde_json::from_str(&caps[2])?
        } else {
            caps[2].to_string()
        };
        attributes.insert(caps[1].to_string(), value);
        consumed = found.end();
    }
    ensure!(
        source[consumed..]
            .chars()
            .all(|ch| ch.is_whitespace() || ch == ','),
        "Unsupported DOT attributes"
    );
    Ok(attributes)
}

fn statements(content: &str) -> Result<String> {
    let mut output = String::new();
    let mut quoted = false;
    let mut escaped = false;
    let mut comment = false;
    let mut brackets = 0_usize;
    let mut chars = content.chars().peekable();
    while let Some(ch) = chars.next() {
        if comment {
            output.push(ch);
            if ch == '\n' {
                comment = false;
            }
        } else if quoted {
            output.push(ch);
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                quoted = false;
            }
        } else {
            match ch {
                '"' => {
                    quoted = true;
                    output.push(ch);
                }
                '/' if chars.peek() == Some(&'/') => {
                    comment = true;
                    output.push(ch);
                }
                '/' if chars.peek() == Some(&'*') => {
                    anyhow::bail!("DOT block comments are unsupported")
                }
                '[' => {
                    brackets += 1;
                    output.push(ch);
                }
                ']' => {
                    ensure!(brackets > 0, "Unexpected DOT bracket");
                    brackets -= 1;
                    output.push(ch);
                }
                ';' if brackets == 0 => output.push('\n'),
                '{' if brackets == 0 => {
                    output.push(ch);
                    output.push('\n');
                }
                '}' if brackets == 0 => {
                    output.push('\n');
                    output.push(ch);
                    output.push('\n');
                }
                _ => output.push(ch),
            }
        }
    }
    ensure!(
        !quoted && brackets == 0,
        "Unterminated DOT string/attributes"
    );
    Ok(output)
}

pub fn serialize(data: &CanvasData, write_layout: bool) -> Result<String> {
    super::validate_flowchart(data, super::SupportedFormat::Dot)?;
    let mut buf = String::new();
    buf.push_str("digraph G {\n");

    match data.orientation {
        crate::data::DiagramOrientation::LeftRight => buf.push_str("    rankdir=LR;\n"),
        crate::data::DiagramOrientation::RightLeft => buf.push_str("    rankdir=RL;\n"),
        crate::data::DiagramOrientation::DownTop => buf.push_str("    rankdir=BT;\n"),
        _ => buf.push_str("    rankdir=TB;\n"),
    }

    if write_layout {
        // Emit layouts first in comments
        for node in &data.nodes {
            let (x, y) = node.pos();
            let (w, h) = node.size();
            buf.push_str(&format!(
                "    // pinstar_layout: {} {} {} {} {}\n",
                node.id(),
                x,
                y,
                w,
                h
            ));
        }
    }

    // Emit node configurations
    for node in &data.nodes {
        let mut node_attrs = vec![format!("label={}", serde_json::to_string(node.text())?)];
        match node.shape() {
            crate::data::NodeShape::Diamond => node_attrs.push("shape=\"diamond\"".to_string()),
            crate::data::NodeShape::Cylinder => node_attrs.push("shape=\"cylinder\"".to_string()),
            crate::data::NodeShape::Circle => node_attrs.push("shape=\"circle\"".to_string()),
            crate::data::NodeShape::Stadium => node_attrs.push("shape=\"ellipse\"".to_string()),
            _ => {}
        }
        if let crate::data::CanvasNode::Text(tn) = node
            && let Some(ref c) = tn.color
        {
            node_attrs.push(format!("color={}", serde_json::to_string(c)?));
        }
        buf.push_str(&format!("    {} [{}];\n", node.id(), node_attrs.join(", ")));
    }

    // Emit connections
    for edge in &data.edges {
        let mut attrs = Vec::new();
        if let Some(ref l) = edge.label {
            attrs.push(format!("label={}", serde_json::to_string(l)?));
        }
        match edge.style {
            crate::data::EdgeStyle::Dashed => attrs.push("style=\"dashed\"".to_string()),
            crate::data::EdgeStyle::Dotted => attrs.push("style=\"dotted\"".to_string()),
            crate::data::EdgeStyle::Thick => attrs.push("style=\"bold\"".to_string()),
            _ => {}
        }
        if let Some(ref color) = edge.color {
            attrs.push(format!("color={}", serde_json::to_string(color)?));
        }

        if attrs.is_empty() {
            buf.push_str(&format!("    {} -> {};\n", edge.from_node, edge.to_node));
        } else {
            buf.push_str(&format!(
                "    {} -> {} [{}];\n",
                edge.from_node,
                edge.to_node,
                attrs.join(", ")
            ));
        }
    }

    buf.push_str("}\n");
    Ok(buf)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_line_and_escaped_labels_round_trip() {
        let data = parse(
            r#"digraph G { A [label="a; [b] \"quote\" \\ path\nnext"]; A -> B [label="yes"]; }"#,
        )
        .unwrap();
        assert_eq!(data.nodes.len(), 2);
        assert_eq!(data.edges.len(), 1);
        let restored = parse(&serialize(&data, true).unwrap()).unwrap();
        assert_eq!(data, restored);
    }

    #[test]
    fn unsupported_or_incomplete_input_is_rejected() {
        for source in [
            "digraph G { A -> B -> C; }",
            "digraph G { A [fontsize=20]; }",
            "digraph G { A -> B;",
            "digraph G { subgraph cluster { A; } }",
        ] {
            assert!(parse(source).is_err(), "{source}");
        }
    }

    #[test]
    fn test_dot_parse_and_serialize() {
        let content = "digraph G {\n    A [label=\"Root node\"];\n    A -> B;\n}";
        let data = parse(content).unwrap();
        assert_eq!(data.nodes.len(), 2);
        assert_eq!(data.edges.len(), 1);

        let output = serialize(&data, false).unwrap();
        assert!(output.contains("A [label=\"Root node\"]"));
        assert!(output.contains("A -> B"));
    }

    #[test]
    fn test_dot_dt_orientation() {
        let content = "digraph G {\n    rankdir=DT;\n    A -> B;\n}";
        let data = parse(content).unwrap();
        assert_eq!(data.orientation, crate::data::DiagramOrientation::DownTop);

        let output = serialize(&data, false).unwrap();
        assert!(
            output.contains("rankdir=BT"),
            "DOT serialization must preserve rankdir=BT but was: {}",
            output
        );
    }
}
