use crate::data::{CanvasData, CanvasEdge, CanvasNode, TextNode};
use anyhow::Result;
use regex::Regex;
use std::collections::HashMap;

pub fn parse(content: &str) -> Result<CanvasData> {
    // Check if the content contains a markdown-fenced mermaid block
    let mermaid_block = if content.contains("```mermaid") {
        let re = Regex::new(r"(?s)```mermaid\s*\n(.*?)```").unwrap();
        if let Some(caps) = re.captures(content) {
            caps.get(1).unwrap().as_str()
        } else {
            content
        }
    } else {
        content
    };

    let mut nodes_map = HashMap::new();
    let mut edges = Vec::new();
    let mut positions = HashMap::new();
    let mut shapes_map = HashMap::new();
    let mut orientation = crate::data::DiagramOrientation::TopDown;

    let meta_re = Regex::new(
        r"^%\s*pinstar_layout:\s+(\S+)\s+([\d\.-]+)\s+([\d\.-]+)\s+([\d\.-]+)\s+([\d\.-]+)",
    )
    .unwrap();
    let node_decl_re = Regex::new(r"([a-zA-Z0-9_\-]+)\s*(\[+|\(+|\{+)(.*?)(\]+|\)+|\}+)").unwrap();
    let edge_re =
        Regex::new(r"([a-zA-Z0-9_\-]+)\s*(-{2,}>|={2,}>|-\.-\->)\s*([a-zA-Z0-9_\-]+)").unwrap();
    let edge_label_re = Regex::new(
        r"([a-zA-Z0-9_\-]+)\s*(-{2,}|={2,}|-\.-)\s*(.*?)\s*(?:-->|==>|\.->)\s*([a-zA-Z0-9_\-]+)",
    )
    .unwrap();
    let lone_id_re = Regex::new(r"^[a-zA-Z0-9_\-]+$").unwrap();

    for line in mermaid_block.lines() {
        let mut line_str = line.trim().to_string();
        if line_str.is_empty() {
            continue;
        }

        // 1. Parse layout metadata
        if let Some(caps) = meta_re.captures(&line_str) {
            let id = caps[1].to_string();
            let x: f64 = caps[2].parse().unwrap_or(0.0);
            let y: f64 = caps[3].parse().unwrap_or(0.0);
            let w: f64 = caps[4].parse().unwrap_or(200.0);
            let h: f64 = caps[5].parse().unwrap_or(100.0);
            positions.insert(id, (x, y, w, h));
            continue;
        }

        // Skip chart definition headers or basic comments
        if line_str.starts_with("%%") {
            continue;
        }

        let upper_line = line_str.to_uppercase();
        if upper_line.starts_with("GRAPH") || upper_line.starts_with("FLOWCHART") {
            if upper_line.contains(" LR") || upper_line.contains("\tLR") {
                orientation = crate::data::DiagramOrientation::LeftRight;
            } else if upper_line.contains(" RL") || upper_line.contains("\tRL") {
                orientation = crate::data::DiagramOrientation::RightLeft;
            } else if upper_line.contains(" DT")
                || upper_line.contains("\tDT")
                || upper_line.contains(" BT")
                || upper_line.contains("\tBT")
            {
                orientation = crate::data::DiagramOrientation::DownTop;
            } else if upper_line.contains(" TD")
                || upper_line.contains("\tTD")
                || upper_line.contains(" TB")
                || upper_line.contains("\tTB")
            {
                orientation = crate::data::DiagramOrientation::TopDown;
            }
            continue;
        }

        // 2. Pre-process inline node shapes to build our label map
        // We iterate and pull out shapes to avoid parsing collisions with arrows.
        let mut keep_processing = true;
        while keep_processing {
            keep_processing = false;
            if let Some(caps) = node_decl_re.captures(&line_str) {
                let full_match = caps.get(0).unwrap().as_str().to_string();
                let id = caps[1].to_string();
                let left_bracket = caps[2].trim();
                let mut label = caps[3].trim().to_string();

                if label.starts_with('"') && label.ends_with('"') && label.len() >= 2 {
                    label = label[1..label.len() - 1].to_string();
                }

                let shape = if left_bracket.contains('{') {
                    crate::data::NodeShape::Diamond
                } else if left_bracket.contains("([") {
                    crate::data::NodeShape::Stadium
                } else if left_bracket.contains("[(") {
                    crate::data::NodeShape::Cylinder
                } else if left_bracket.contains("((") {
                    crate::data::NodeShape::Circle
                } else {
                    crate::data::NodeShape::Rectangle
                };

                nodes_map.insert(id.clone(), label);
                shapes_map.insert(id.clone(), shape);
                line_str = line_str.replace(&full_match, &id);
                keep_processing = true;
            }
        }

        // 3. Identify graph edges on the simplified line
        if let Some(caps) = edge_label_re.captures(&line_str) {
            let from = caps[1].trim().to_string();
            let label = caps[3].trim().to_string();
            let to = caps[4].trim().to_string();

            let style = if line_str.contains("==") {
                crate::data::EdgeStyle::Thick
            } else if line_str.contains("-.-") || line_str.contains("-.") {
                crate::data::EdgeStyle::Dashed
            } else {
                crate::data::EdgeStyle::Solid
            };

            nodes_map
                .entry(from.clone())
                .or_insert_with(|| from.clone());
            nodes_map.entry(to.clone()).or_insert_with(|| to.clone());

            edges.push(CanvasEdge {
                id: format!("edge_{}_{}", from, to),
                from_node: from,
                to_node: to,
                from_side: Some("right".to_string()),
                to_side: Some("left".to_string()),
                label: Some(label),
                color: None,
                style,
            });
        } else if let Some(caps) = edge_re.captures(&line_str) {
            let from = caps[1].trim().to_string();
            let to = caps[3].trim().to_string();

            let style = if line_str.contains("==>") {
                crate::data::EdgeStyle::Thick
            } else if line_str.contains("-.->") {
                crate::data::EdgeStyle::Dashed
            } else {
                crate::data::EdgeStyle::Solid
            };

            nodes_map
                .entry(from.clone())
                .or_insert_with(|| from.clone());
            nodes_map.entry(to.clone()).or_insert_with(|| to.clone());

            edges.push(CanvasEdge {
                id: format!("edge_{}_{}", from, to),
                from_node: from,
                to_node: to,
                from_side: Some("right".to_string()),
                to_side: Some("left".to_string()),
                label: None,
                color: None,
                style,
            });
        } else {
            // Lone single node ID with no shapes
            if lone_id_re.is_match(&line_str) {
                nodes_map.entry(line_str.clone()).or_insert(line_str);
            }
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

pub fn serialize(data: &CanvasData, original_content: &str, write_layout: bool) -> Result<String> {
    let mut block_buf = String::new();
    match data.orientation {
        crate::data::DiagramOrientation::LeftRight => block_buf.push_str("graph LR\n"),
        crate::data::DiagramOrientation::RightLeft => block_buf.push_str("graph RL\n"),
        crate::data::DiagramOrientation::DownTop => block_buf.push_str("graph DT\n"),
        _ => block_buf.push_str("graph TD\n"),
    }

    if write_layout {
        // Emit the layout metadata
        for node in &data.nodes {
            let (x, y) = node.pos();
            let (w, h) = node.size();
            block_buf.push_str(&format!(
                "    %% pinstar_layout: {} {:.1} {:.1} {:.1} {:.1}\n",
                node.id(),
                x,
                y,
                w,
                h
            ));
        }
    }

    // Emit definitions
    for node in &data.nodes {
        let escaped_txt = node.text().replace('"', "\\\"");
        let (l_bracket, r_bracket) = match node.shape() {
            crate::data::NodeShape::Diamond => ("{\"", "\"}"),
            crate::data::NodeShape::Circle => ("((\"", "\"))"),
            crate::data::NodeShape::Cylinder => ("[(\"", "\")]"),
            crate::data::NodeShape::Stadium => ("([\"", "\"])"),
            _ => ("[\"", "\"]"),
        };
        block_buf.push_str(&format!(
            "    {}{}{}{}\n",
            node.id(),
            l_bracket,
            escaped_txt,
            r_bracket
        ));
    }

    // Emit connections
    for edge in &data.edges {
        if let Some(ref l) = edge.label {
            let escaped_lbl = l.replace('"', "\\\"");
            match edge.style {
                crate::data::EdgeStyle::Thick => block_buf.push_str(&format!(
                    "    {} == \"{}\" ==> {}\n",
                    edge.from_node, escaped_lbl, edge.to_node
                )),
                crate::data::EdgeStyle::Dashed | crate::data::EdgeStyle::Dotted => block_buf
                    .push_str(&format!(
                        "    {} -. \"{}\" .-> {}\n",
                        edge.from_node, escaped_lbl, edge.to_node
                    )),
                _ => block_buf.push_str(&format!(
                    "    {} -- \"{}\" --> {}\n",
                    edge.from_node, escaped_lbl, edge.to_node
                )),
            }
        } else {
            let arrow = match edge.style {
                crate::data::EdgeStyle::Thick => "==>",
                crate::data::EdgeStyle::Dashed | crate::data::EdgeStyle::Dotted => "-.->",
                _ => "-->",
            };
            block_buf.push_str(&format!(
                "    {} {} {}\n",
                edge.from_node, arrow, edge.to_node
            ));
        }
    }

    // If original content has a markdown mermaid codeblock, overwrite it in place.
    if original_content.contains("```mermaid") {
        let re = Regex::new(r"(?s)(```mermaid\s*\n).*?(\n```)").unwrap();
        if re.is_match(original_content) {
            let output = re
                .replace(original_content, |caps: &regex::Captures| {
                    format!("{}{}{}", &caps[1], block_buf, &caps[2])
                })
                .to_string();
            return Ok(output);
        }
    }

    Ok(block_buf)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_parse_and_serialize() {
        let content = "graph TD\nA[First Node] --> B[Second Node]";
        let data = parse(content).unwrap();
        assert_eq!(data.nodes.len(), 2);
        assert_eq!(data.edges.len(), 1);

        let serial = serialize(&data, content, false).unwrap();
        assert!(serial.contains("A --> B") || serial.contains("A[\"First Node\"]"));
    }

    #[test]
    fn test_dt_parse_and_serialize() {
        let content = "graph DT\nA --> B";
        let data = parse(content).unwrap();
        assert_eq!(data.orientation, crate::data::DiagramOrientation::DownTop);

        let serial = serialize(&data, content, false).unwrap();
        assert!(
            serial.contains("graph DT"),
            "Serialized string should contain graph DT but was: {}",
            serial
        );
    }

    #[test]
    fn test_dt_markdown_parse_and_serialize() {
        let content = "# Doc\n```mermaid\ngraph DT\nA --> B\n```\nFooter";
        let data = parse(content).unwrap();
        assert_eq!(data.orientation, crate::data::DiagramOrientation::DownTop);

        let serial = serialize(&data, content, false).unwrap();
        assert!(
            serial.contains("graph DT"),
            "Markdown output should preserve graph DT but was: {}",
            serial
        );
        assert!(serial.contains("# Doc"), "Markdown header lost!");
        assert!(serial.contains("Footer"), "Markdown footer lost!");
    }
}
