use crate::data::{CanvasData, CanvasEdge, CanvasNode, TextNode};
use anyhow::{Result, ensure};
use regex::Regex;
use std::collections::BTreeMap;

pub fn parse(content: &str) -> Result<CanvasData> {
    let mut nodes_map = BTreeMap::new();
    let mut edges = Vec::new();
    let mut positions = BTreeMap::new();
    let mut shapes_map = BTreeMap::new();
    let mut orientation = crate::data::DiagramOrientation::TopDown;

    let meta_re =
        Regex::new(r"^'\s*pinstar_layout:\s+(\S+)\s+(\S+)\s+(\S+)\s+(\S+)\s+(\S+)$").unwrap();

    // Captures declarations like: object my_node, database cache
    let decl_re = Regex::new(r"(?i)^\s*(object|usecase|database)\s+([a-zA-Z0-9_\-]+)$").unwrap();

    // Captures label assignments like: my_node : "This is the text"
    let label_assign_re = Regex::new(r"^\s*([a-zA-Z0-9_\-]+)\s*:\s*(.*?)\s*$").unwrap();

    // Captures connections: A --> B or A ..> B
    let edge_re = Regex::new(r"^([a-zA-Z0-9_\-]+)\s*(?:-->|\.\.>)\s*([a-zA-Z0-9_\-]+)$").unwrap();
    let edge_label_re =
        Regex::new(r"^([a-zA-Z0-9_\-]+)\s*(?:-->|\.\.>)\s*([a-zA-Z0-9_\-]+)\s*:\s*(.*?)\s*$")
            .unwrap();
    let id_pattern = Regex::new(r"^[a-zA-Z0-9_\-]+$").unwrap();

    let mut opened = false;
    let mut closed = false;
    for line in content.lines() {
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

        // Check for PlantUML layout orientation
        if trimmed.eq_ignore_ascii_case("left to right direction") {
            orientation = crate::data::DiagramOrientation::LeftRight;
            continue;
        } else if trimmed.eq_ignore_ascii_case("down to top direction")
            || trimmed.eq_ignore_ascii_case("bottom to top direction")
        {
            orientation = crate::data::DiagramOrientation::DownTop;
            continue;
        }

        // Skip PlantUML framing markers
        if trimmed == "@startuml" {
            ensure!(!opened, "Multiple PlantUML diagrams are unsupported");
            opened = true;
            continue;
        }
        if trimmed == "@enduml" {
            ensure!(opened && !closed, "Unexpected PlantUML end marker");
            closed = true;
            continue;
        }
        if trimmed == "' pinstar_orientation: RL" {
            orientation = crate::data::DiagramOrientation::RightLeft;
            continue;
        }
        if trimmed.starts_with("'") {
            ensure!(
                !trimmed.contains("pinstar_layout:"),
                "Invalid PlantUML layout metadata"
            );
            continue;
        }
        ensure!(opened && !closed, "Expected a PlantUML diagram body");

        // 2. Parse Edge Connections (Match labeled edges first)
        if let Some(caps) = edge_label_re.captures(trimmed) {
            let from = caps[1].to_string();
            let to = caps[2].to_string();
            let mut label = caps[3].to_string();

            if label.starts_with('"') && label.ends_with('"') && label.len() >= 2 {
                label = serde_json::from_str(&label)?;
            }

            let style = if trimmed.contains("..") {
                crate::data::EdgeStyle::Dotted
            } else {
                crate::data::EdgeStyle::Solid
            };

            nodes_map
                .entry(from.clone())
                .or_insert_with(|| from.clone());
            nodes_map.entry(to.clone()).or_insert_with(|| to.clone());

            edges.push(CanvasEdge {
                id: format!("edge_{}_{}_{}", from, to, edges.len()),
                from_node: from,
                to_node: to,
                from_side: Some("right".to_string()),
                to_side: Some("left".to_string()),
                label: Some(label),
                color: None,
                style,
            });
            continue;
        }

        if let Some(caps) = edge_re.captures(trimmed) {
            let from = caps[1].to_string();
            let to = caps[2].to_string();

            let style = if trimmed.contains("..") {
                crate::data::EdgeStyle::Dotted
            } else {
                crate::data::EdgeStyle::Solid
            };

            nodes_map
                .entry(from.clone())
                .or_insert_with(|| from.clone());
            nodes_map.entry(to.clone()).or_insert_with(|| to.clone());

            edges.push(CanvasEdge {
                id: format!("edge_{}_{}_{}", from, to, edges.len()),
                from_node: from,
                to_node: to,
                from_side: Some("right".to_string()),
                to_side: Some("left".to_string()),
                label: None,
                color: None,
                style,
            });
            continue;
        }

        // 3. Parse Explicit Node Declarations
        if let Some(caps) = decl_re.captures(trimmed) {
            let kind = caps[1].to_lowercase();
            let id = caps[2].to_string();
            let shape = match kind.as_str() {
                "database" => crate::data::NodeShape::Cylinder,
                "usecase" => crate::data::NodeShape::Circle,
                _ => crate::data::NodeShape::Rectangle,
            };
            shapes_map.insert(id.clone(), shape);
            nodes_map.entry(id.clone()).or_insert(id);
            continue;
        }

        // 4. Parse Attribute/Label Assignments: ID : Text
        if let Some(caps) = label_assign_re.captures(trimmed) {
            let id = caps[1].to_string();
            let mut text = caps[2].to_string();

            if text.starts_with('"') && text.ends_with('"') && text.len() >= 2 {
                text = serde_json::from_str(&text)?;
            }

            nodes_map.insert(id, text);
            continue;
        }

        // 5. Standalone Identifier
        let clean_id = trimmed.trim_matches(|c| c == ';' || c == ':').trim();
        if id_pattern.is_match(clean_id) {
            ensure!(
                !["start", "stop", "end", "kill", "detach"].contains(&clean_id),
                "PlantUML activity syntax is unsupported"
            );
            nodes_map
                .entry(clean_id.to_string())
                .or_insert_with(|| clean_id.to_string());
            continue;
        }
        anyhow::bail!(
            "Unsupported PlantUML syntax (only object/usecase/database diagrams are editable): {trimmed}"
        );
    }
    ensure!(opened && closed, "Incomplete PlantUML diagram");

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
        canvas_nodes.push(CanvasNode::Text(TextNode {
            id,
            x,
            y,
            width: w,
            height: h,
            text: label,
            title: None,
            color: None,
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
    super::validate_flowchart(data, super::SupportedFormat::PlantUml)?;
    let mut buf = String::new();
    buf.push_str("@startuml\n");

    if data.orientation == crate::data::DiagramOrientation::RightLeft {
        buf.push_str("' pinstar_orientation: RL\n");
    } else if data.orientation == crate::data::DiagramOrientation::LeftRight {
        buf.push_str("left to right direction\n");
    } else if data.orientation == crate::data::DiagramOrientation::DownTop {
        buf.push_str("down to top direction\n");
    }

    if write_layout {
        // Layout metadata in PlantUML single quote comments
        for node in &data.nodes {
            let (x, y) = node.pos();
            let (w, h) = node.size();
            buf.push_str(&format!(
                "' pinstar_layout: {} {} {} {} {}\n",
                node.id(),
                x,
                y,
                w,
                h
            ));
        }
    }

    // Explicit object declarations
    for node in &data.nodes {
        let keyword = match node.shape() {
            crate::data::NodeShape::Cylinder => "database",
            crate::data::NodeShape::Circle => "usecase",
            _ => "object",
        };
        buf.push_str(&format!("{} {}\n", keyword, node.id()));
    }

    // Text Labels
    for node in &data.nodes {
        buf.push_str(&format!(
            "{} : {}\n",
            node.id(),
            serde_json::to_string(node.text())?
        ));
    }

    // Connections
    for edge in &data.edges {
        let arrow = match edge.style {
            crate::data::EdgeStyle::Dotted | crate::data::EdgeStyle::Dashed => "..>",
            _ => "-->",
        };
        if let Some(ref lbl) = edge.label {
            let escaped_lbl = serde_json::to_string(lbl)?;
            buf.push_str(&format!(
                "{} {} {} : {}\n",
                edge.from_node, arrow, edge.to_node, escaped_lbl
            ));
        } else {
            buf.push_str(&format!("{} {} {}\n", edge.from_node, arrow, edge.to_node));
        }
    }

    buf.push_str("@enduml\n");
    Ok(buf)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unsupported_activity_and_incomplete_diagrams_are_rejected() {
        for source in [
            "@startuml\nstart\n:Do work;\nstop\n@enduml",
            "@startuml\nstart\nstop\n@enduml",
            "@startuml\nclass A {\nfield\n}\n@enduml",
            "@startuml\nA --> B",
            "@startuml\nA --> B --> C\n@enduml",
        ] {
            assert!(parse(source).is_err(), "{source}");
        }
    }

    #[test]
    fn escaped_labels_and_right_left_orientation_round_trip() {
        let mut data = parse("@startuml\nA --> B\n@enduml").unwrap();
        data.nodes[0].set_text("quote \" and \\ path\nnext".into());
        data.edges[0].label = Some("label \" and \\ path\nnext".into());
        data.orientation = crate::data::DiagramOrientation::RightLeft;
        assert_eq!(parse(&serialize(&data, true).unwrap()).unwrap(), data);
    }

    #[test]
    fn test_plantuml_basic() {
        let content = "@startuml\nA --> B\n@enduml";
        let data = parse(content).unwrap();
        assert_eq!(data.nodes.len(), 2);
        assert_eq!(data.edges.len(), 1);

        let output = serialize(&data, false).unwrap();
        assert!(output.contains("A --> B"));
    }
}
