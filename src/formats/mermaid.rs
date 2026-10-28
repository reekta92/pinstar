use crate::data::{CanvasData, CanvasEdge, CanvasNode, TextNode};
use anyhow::{Result, ensure};
use regex::Regex;
use std::collections::BTreeMap;

pub fn parse(content: &str) -> Result<CanvasData> {
    // Check if the content contains a markdown-fenced mermaid block
    let range = block_range(content)?;
    let mermaid_block = range.map(|range| &content[range]).unwrap_or(content);

    let mut nodes_map = BTreeMap::new();
    let mut edges = Vec::new();
    let mut positions = BTreeMap::new();
    let mut shapes_map = BTreeMap::new();
    let mut orientation = crate::data::DiagramOrientation::TopDown;

    let meta_re =
        Regex::new(r"^%%?\s*pinstar_layout:\s+(\S+)\s+(\S+)\s+(\S+)\s+(\S+)\s+(\S+)$").unwrap();
    let node_decl_re = Regex::new(r#"([a-zA-Z0-9_\-]+)\s*(\(\[|\[\(|\(\(|\[|\(|\{)("(?:\\.|[^"\\])*"|[^\[\](){}"]*)(\]\)|\)\]|\)\)|\]|\)|\})"#).unwrap();
    let edge_re = Regex::new(r"^([a-zA-Z0-9_\-]+)\s*(-->|==>|-\.->)\s*([a-zA-Z0-9_\-]+)$").unwrap();
    let edge_label_re = Regex::new(
        r#"^([a-zA-Z0-9_\-]+)\s*(--|==|-\.)\s+("(?:\\.|[^"\\])*"|[^<>"|]*?)\s*(?:-->|==>|\.->)\s*([a-zA-Z0-9_\-]+)$"#,
    )
    .unwrap();
    let lone_id_re = Regex::new(r"^[a-zA-Z0-9_\-]+$").unwrap();

    let pipe_edge_re = Regex::new(
        r#"^([a-zA-Z0-9_\-]+)\s*(-->|==>|-\.->)\s*\|("(?:\\.|[^"\\])*"|[^|]*)\|\s*([a-zA-Z0-9_\-]+)$"#,
    )?;
    let header_re = Regex::new(r"(?i)^(graph|flowchart)\s+(TD|TB|LR|RL|BT|DT)$")?;
    let mut header = false;
    for line in mermaid_block.lines() {
        let mut line_str = line.trim().trim_end_matches(';').trim().to_string();
        if line_str.is_empty() {
            continue;
        }

        // 1. Parse layout metadata
        if let Some(caps) = meta_re.captures(&line_str) {
            let id = caps[1].to_string();
            let (x, y, w, h) = super::parse_layout(&caps[2], &caps[3], &caps[4], &caps[5])?;
            positions.insert(id, (x, y, w, h));
            continue;
        }

        // Skip chart definition headers or basic comments
        if line_str.starts_with("%%") {
            ensure!(
                !line_str.starts_with("%%{") && !line_str.contains("pinstar_layout:"),
                "Unsupported Mermaid configuration/layout comment"
            );
            continue;
        }

        if let Some(caps) = header_re.captures(&line_str) {
            ensure!(!header, "Multiple Mermaid chart headers are unsupported");
            header = true;
            orientation = match caps[2].to_ascii_uppercase().as_str() {
                "LR" => crate::data::DiagramOrientation::LeftRight,
                "RL" => crate::data::DiagramOrientation::RightLeft,
                "BT" | "DT" => crate::data::DiagramOrientation::DownTop,
                _ => crate::data::DiagramOrientation::TopDown,
            };
            continue;
        }
        ensure!(header, "Expected a Mermaid graph/flowchart header");
        ensure!(
            ![
                "subgraph",
                "end",
                "direction",
                "class",
                "classDef",
                "style",
                "linkStyle",
                "click"
            ]
            .iter()
            .any(|word| line_str == *word || line_str.starts_with(&format!("{word} "))),
            "Unsupported Mermaid syntax: {line_str}"
        );

        // 2. Pre-process inline node shapes to build our label map
        // We iterate and pull out shapes to avoid parsing collisions with arrows.
        let mut keep_processing = true;
        while keep_processing {
            keep_processing = false;
            if let Some(caps) = node_decl_re.captures(&line_str) {
                let full_match = caps.get(0).unwrap().as_str().to_string();
                let id = caps[1].to_string();
                let left_bracket = caps[2].trim();
                let label = decode_label(caps[3].trim())?;
                let expected = match left_bracket {
                    "([" => "])",
                    "[(" => ")]",
                    "((" => "))",
                    "[" => "]",
                    "(" => ")",
                    "{" => "}",
                    _ => unreachable!(),
                };
                ensure!(&caps[4] == expected, "Mismatched Mermaid shape delimiters");

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
        if let Some(caps) = pipe_edge_re
            .captures(&line_str)
            .or_else(|| edge_label_re.captures(&line_str))
        {
            let from = caps[1].trim().to_string();
            let label = decode_label(caps[3].trim())?;
            let to = caps[4].trim().to_string();

            let style = match &caps[2] {
                "==" | "==>" => crate::data::EdgeStyle::Thick,
                "-." | "-.->" => crate::data::EdgeStyle::Dashed,
                _ => crate::data::EdgeStyle::Solid,
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
                id: format!("edge_{}_{}_{}", from, to, edges.len()),
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
            } else {
                anyhow::bail!("Unsupported Mermaid syntax: {line_str}");
            }
        }
    }
    ensure!(header, "Expected a Mermaid graph/flowchart header");

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

fn block_range(content: &str) -> Result<Option<std::ops::Range<usize>>> {
    let fenced = Regex::new(r"(?ms)^ {0,3}```mermaid[ \t]*\r?\n(.*?)^ {0,3}```[ \t]*(?:\r?\n|$)")?;
    let body = fenced
        .captures(content)
        .and_then(|caps| caps.get(1))
        .map(|body| body.range());
    ensure!(
        body.is_some()
            || !content
                .lines()
                .any(|line| line.trim_start().starts_with("```mermaid")),
        "Unterminated Mermaid code block"
    );
    Ok(body)
}

fn decode_label(label: &str) -> Result<String> {
    let text = if label.starts_with('"') {
        serde_json::from_str::<String>(label)?
    } else {
        label.to_string()
    };
    let entities = Regex::new(r"#(quot|[0-9]+);")?;
    Ok(entities
        .replace_all(&text, |caps: &regex::Captures| {
            if &caps[1] == "quot" {
                return "\"".to_string();
            }
            caps[1]
                .parse::<u32>()
                .ok()
                .and_then(char::from_u32)
                .map(|ch| ch.to_string())
                .unwrap_or_else(|| caps[0].to_string())
        })
        .into_owned())
}

fn encode_label(label: &str) -> String {
    label
        .chars()
        .map(|ch| match ch {
            '"' => "#quot;".into(),
            '#' | '\\' | '\n' | '\r' | '[' | ']' | '(' | ')' | '{' | '}' | '|' => {
                format!("#{};", ch as u32)
            }
            _ => ch.to_string(),
        })
        .collect()
}

pub fn serialize(data: &CanvasData, original_content: &str, write_layout: bool) -> Result<String> {
    super::validate_flowchart(data, super::SupportedFormat::Mermaid)?;
    let mut block_buf = String::new();
    match data.orientation {
        crate::data::DiagramOrientation::LeftRight => block_buf.push_str("graph LR\n"),
        crate::data::DiagramOrientation::RightLeft => block_buf.push_str("graph RL\n"),
        crate::data::DiagramOrientation::DownTop => block_buf.push_str("graph BT\n"),
        _ => block_buf.push_str("graph TD\n"),
    }

    if write_layout {
        // Emit the layout metadata
        for node in &data.nodes {
            let (x, y) = node.pos();
            let (w, h) = node.size();
            block_buf.push_str(&format!(
                "    %% pinstar_layout: {} {} {} {} {}\n",
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
        let escaped_txt = encode_label(node.text());
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
            let escaped_lbl = encode_label(l);
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
    if let Some(range) = block_range(original_content)? {
        let mut output = original_content.to_string();
        output.replace_range(range, &block_buf);
        return Ok(output);
    }

    Ok(block_buf)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_shape_layout_and_escaped_label_round_trips() {
        let mut data = parse("flowchart TD\nA[initial]\n").unwrap();
        for shape in [
            crate::data::NodeShape::Rectangle,
            crate::data::NodeShape::Diamond,
            crate::data::NodeShape::Circle,
            crate::data::NodeShape::Cylinder,
            crate::data::NodeShape::Stadium,
        ] {
            if let CanvasNode::Text(node) = &mut data.nodes[0] {
                node.shape = shape;
                node.text = "quote \" [bracket] (paren) {brace} \\ slash\nline #10;".into();
                node.x = 123.456;
                node.y = -77.789;
            }
            let saved = serialize(&data, "", true).unwrap();
            assert_eq!(parse(&saved).unwrap(), data, "{saved}");
            #[cfg(feature = "mermaid-preview")]
            assert!(
                crate::preview::render_mermaid(&saved, 120).is_ok(),
                "{saved}"
            );
        }
    }

    #[test]
    fn pipe_labels_and_parallel_edges_keep_their_identity() {
        let data = parse("graph TD\nA -->|yes| B\nA -->|no| B\n").unwrap();
        assert_eq!(data.edges[0].label.as_deref(), Some("yes"));
        assert_ne!(data.edges[0].id, data.edges[1].id);
        assert_eq!(parse(&serialize(&data, "", false).unwrap()).unwrap(), data);
    }

    #[test]
    fn markdown_keeps_other_diagrams_and_accepts_crlf_fences() {
        let source = "# Note\r\n```mermaid\r\ngraph TD\r\nA --> B\r\n```\r\nBetween\n```mermaid\ngraph LR\nC --> D\n```\nFooter";
        let data = parse(source).unwrap();
        let saved = serialize(&data, source, false).unwrap();
        assert!(saved.starts_with("# Note\r\n```mermaid\r\n"));
        assert!(saved.contains("Between\n```mermaid\ngraph LR\nC --> D\n```\nFooter"));
        for source in [
            "# prose without diagram",
            "```mermaid\ngraph TD\nA --> B",
            "graph TD\nA --> B --> C",
            "graph TD\nsubgraph g\nA\nend",
            "graph TD\nA[broken)",
            "graph TD\n%%{init: {}}%%",
        ] {
            assert!(parse(source).is_err(), "{source}");
        }
    }

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
            serial.contains("graph BT"),
            "Serialized string should contain graph BT but was: {}",
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
            serial.contains("graph BT"),
            "Markdown output should preserve bottom-up orientation but was: {}",
            serial
        );
        assert!(serial.contains("# Doc"), "Markdown header lost!");
        assert!(serial.contains("Footer"), "Markdown footer lost!");
    }
}
