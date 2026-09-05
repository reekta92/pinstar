use crate::data::{CanvasData, CanvasEdge, CanvasNode, TextNode};
use anyhow::Result;
use regex::Regex;
use std::collections::HashMap;

pub fn parse(content: &str) -> Result<CanvasData> {
    let mut nodes_map = HashMap::new();
    let mut edges = Vec::new();
    let mut positions = HashMap::new();
    let mut shapes_map = HashMap::new();
    let mut colors_map = HashMap::new();
    let mut orientation = crate::data::DiagramOrientation::TopDown;

    let meta_re = Regex::new(
        r"^//\s*pinstar_layout:\s+(\S+)\s+([\d\.-]+)\s+([\d\.-]+)\s+([\d\.-]+)\s+([\d\.-]+)",
    )
    .unwrap();
    let label_re = Regex::new(r#"label\s*=\s*"([^"]*)""#).unwrap();
    let node_decl_re = Regex::new(r#"([a-zA-Z0-9_\-]+)\s*\[([^\]]*)\]"#).unwrap();
    let edge_re = Regex::new(r#"([a-zA-Z0-9_\-]+)\s*->\s*([a-zA-Z0-9_\-]+)"#).unwrap();
    let style_re = Regex::new(r#"style\s*=\s*"?([^",\s\]]+)"?"#).unwrap();
    let color_re = Regex::new(r#"color\s*=\s*"?([^",\s\]]+)"?"#).unwrap();
    let node_shape_re = Regex::new(r#"shape\s*=\s*"?([^",\s\]]+)"?"#).unwrap();
    let node_color_re = Regex::new(r#"color\s*=\s*"?([^",\s\]]+)"?"#).unwrap();
    let id_regex = Regex::new(r"^[a-zA-Z0-9_\-]+$").unwrap();

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        // 1. Extract Layout Metadata
        if let Some(caps) = meta_re.captures(trimmed) {
            let id = caps[1].to_string();
            let x: f64 = caps[2].parse().unwrap_or(0.0);
            let y: f64 = caps[3].parse().unwrap_or(0.0);
            let w: f64 = caps[4].parse().unwrap_or(200.0);
            let h: f64 = caps[5].parse().unwrap_or(100.0);
            positions.insert(id, (x, y, w, h));
            continue;
        }

        let upper_trimmed = trimmed.to_uppercase();
        if upper_trimmed.contains("RANKDIR") {
            if upper_trimmed.contains("LR") {
                orientation = crate::data::DiagramOrientation::LeftRight;
            } else if upper_trimmed.contains("RL") {
                orientation = crate::data::DiagramOrientation::RightLeft;
            } else if upper_trimmed.contains("DT") || upper_trimmed.contains("BT") {
                orientation = crate::data::DiagramOrientation::DownTop;
            } else if upper_trimmed.contains("TB") || upper_trimmed.contains("TD") {
                orientation = crate::data::DiagramOrientation::TopDown;
            }
            continue;
        }

        // Skip boilerplate or whole-line comments
        if trimmed.starts_with("//")
            || trimmed.starts_with("/*")
            || trimmed.starts_with("digraph")
            || trimmed.starts_with("graph")
            || trimmed == "}"
        {
            continue;
        }

        // 2. Parse Edges: A -> B [label="text"]
        if trimmed.contains("->") {
            if let Some(caps) = edge_re.captures(trimmed) {
                let from = caps[1].trim().to_string();
                let to = caps[2].trim().to_string();

                nodes_map
                    .entry(from.clone())
                    .or_insert_with(|| from.clone());
                nodes_map.entry(to.clone()).or_insert_with(|| to.clone());

                let label = label_re.captures(trimmed).map(|c| c[1].to_string());
                let style = if let Some(scaps) = style_re.captures(trimmed) {
                    match &scaps[1] {
                        "dashed" => crate::data::EdgeStyle::Dashed,
                        "dotted" => crate::data::EdgeStyle::Dotted,
                        "bold" => crate::data::EdgeStyle::Thick,
                        _ => crate::data::EdgeStyle::Solid,
                    }
                } else {
                    crate::data::EdgeStyle::Solid
                };
                let color = color_re.captures(trimmed).map(|c| c[1].to_string());

                edges.push(CanvasEdge {
                    id: format!("edge_{}_{}", from, to),
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

            let label = if let Some(lbl_caps) = label_re.captures(attributes) {
                lbl_caps[1].to_string()
            } else {
                id.clone()
            };
            let shape = if let Some(scaps) = node_shape_re.captures(attributes) {
                match &scaps[1] {
                    "diamond" => crate::data::NodeShape::Diamond,
                    "cylinder" => crate::data::NodeShape::Cylinder,
                    "circle" => crate::data::NodeShape::Circle,
                    "ellipse" => crate::data::NodeShape::Stadium,
                    _ => crate::data::NodeShape::Rectangle,
                }
            } else {
                crate::data::NodeShape::Rectangle
            };
            shapes_map.insert(id.clone(), shape);

            if let Some(ccaps) = node_color_re.captures(attributes) {
                colors_map.insert(id.clone(), ccaps[1].to_string());
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
        }
    }

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

pub fn serialize(data: &CanvasData, write_layout: bool) -> Result<String> {
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
                "    // pinstar_layout: {} {:.1} {:.1} {:.1} {:.1}\n",
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
        let escaped_lbl = node.text().replace('"', "\\\"");
        let mut node_attrs = vec![format!("label=\"{}\"", escaped_lbl)];
        match node.shape() {
            crate::data::NodeShape::Diamond => node_attrs.push("shape=\"diamond\"".to_string()),
            crate::data::NodeShape::Cylinder => node_attrs.push("shape=\"cylinder\"".to_string()),
            crate::data::NodeShape::Circle => node_attrs.push("shape=\"circle\"".to_string()),
            crate::data::NodeShape::Stadium => node_attrs.push("shape=\"ellipse\"".to_string()),
            _ => {}
        }
        if let crate::data::CanvasNode::Text(tn) = node {
            if let Some(ref c) = tn.color {
                node_attrs.push(format!("color=\"{}\"", c));
            }
        }
        buf.push_str(&format!("    {} [{}];\n", node.id(), node_attrs.join(", ")));
    }

    // Emit connections
    for edge in &data.edges {
        let mut attrs = Vec::new();
        if let Some(ref l) = edge.label {
            let escaped_lbl = l.replace('"', "\\\"");
            attrs.push(format!("label=\"{}\"", escaped_lbl));
        }
        match edge.style {
            crate::data::EdgeStyle::Dashed => attrs.push("style=\"dashed\"".to_string()),
            crate::data::EdgeStyle::Dotted => attrs.push("style=\"dotted\"".to_string()),
            crate::data::EdgeStyle::Thick => attrs.push("style=\"bold\"".to_string()),
            _ => {}
        }
        if let Some(ref color) = edge.color {
            attrs.push(format!("color=\"{}\"", color));
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
