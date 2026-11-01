use super::*;
use merman_core::diagrams::state::{StateDiagramRenderLinks, StateDiagramRenderModel};

pub(super) fn validate_source(source: &str) -> Result<()> {
    let mut note = false;
    for line in source.lines().map(str::trim) {
        if line.starts_with("%%") {
            continue;
        }
        if line == "end note" {
            note = false;
        } else if line.starts_with("note ") {
            note = !line.contains(':');
        } else if !note
            && let Some((_, transition)) = line.split_once("-->")
            && let Some((_, label)) = transition.split_once(':')
        {
            ensure!(
                !label.contains(';'),
                "Semicolons in state transition labels are ambiguous; use a comma"
            );
        }
    }
    ensure!(
        source.matches('{').count() <= MAX_DEPTH,
        "Mermaid state nesting exceeds parser budget"
    );
    Ok(())
}

pub(super) fn render(model: StateDiagramRenderModel, columns: u16) -> Result<MermaidPreview> {
    ensure!(
        model.nodes.len() <= MAX_NODES && model.edges.len() <= MAX_EDGES,
        "Mermaid state diagram exceeds preview limit (128 nodes, 256 edges)"
    );
    let mut labels = BTreeMap::new();
    for node in &model.nodes {
        ensure!(
            matches!(
                node.shape.as_str(),
                "rect" | "stateStart" | "stateEnd" | "note" | "noteGroup"
            ),
            "Unsupported state shape: {}",
            plain_label(&node.shape)
        );
        ensure!(
            !node.is_group || node.shape == "noteGroup",
            "Composite states are not supported in previews"
        );
        let label = match node.shape.as_str() {
            "stateStart" => "● Start".to_owned(),
            "stateEnd" => "◎ End".to_owned(),
            _ => {
                let mut label = match &node.label {
                    Some(serde_json::Value::String(value)) => value.clone(),
                    Some(serde_json::Value::Array(values)) => values
                        .iter()
                        .map(|value| {
                            value
                                .as_str()
                                .ok_or_else(|| anyhow::anyhow!("Unsupported state label"))
                        })
                        .collect::<Result<Vec<_>>>()?
                        .join("\n"),
                    None => node.id.clone(),
                    _ => anyhow::bail!("Unsupported state label"),
                };
                if node.shape == "rect" && label != node.id {
                    label.push_str(&format!(" ({})", node.id));
                }
                if let Some(descriptions) = &node.description {
                    for description in descriptions {
                        label.push('\n');
                        label.push_str(description);
                    }
                }
                label
            }
        };
        ensure!(
            labels.insert(node.id.as_str(), label).is_none(),
            "Duplicate state identifier"
        );
    }
    for edge in &model.edges {
        ensure!(
            labels.contains_key(edge.start.as_str()) && labels.contains_key(edge.end.as_str()),
            "Unknown state transition endpoint"
        );
    }
    let mut lines = Vec::new();
    for node in &model.nodes {
        if matches!(node.shape.as_str(), "note" | "noteGroup") {
            continue;
        }
        let mut label = labels[node.id.as_str()].clone();
        for edge in &model.edges {
            if edge
                .classes
                .split_whitespace()
                .any(|class| class == "note-edge")
            {
                let note_id = if edge.start == node.id {
                    &edge.end
                } else if edge.end == node.id {
                    &edge.start
                } else {
                    continue;
                };
                let note = model
                    .nodes
                    .iter()
                    .find(|note| note.id == *note_id && note.shape == "note")
                    .ok_or_else(|| anyhow::anyhow!("Unknown attached state note"))?;
                label.push_str(&format!(
                    "\nNote ({}):\n{}",
                    note.position.as_deref().unwrap_or("attached"),
                    labels[note_id.as_str()]
                ));
            } else if edge.start == node.id {
                label.push_str(&format!("\n  └─▶ {}", labels[edge.end.as_str()]));
                if !edge.label.is_empty() {
                    label.push_str(&format!(": {}", edge.label));
                }
            }
        }
        if let Some(links) = model.links.get(&node.id) {
            let links = match links {
                StateDiagramRenderLinks::One(link) => std::slice::from_ref(link),
                StateDiagramRenderLinks::Many(links) => links.as_slice(),
            };
            for link in links {
                label.push_str(&format!("\nLink: {} {}", link.url, link.tooltip));
            }
        }
        lines.extend(card(&label, columns, Style::default())?);
        ensure!(
            lines.len() <= usize::from(MAX_ROWS),
            "Mermaid state diagram exceeds 512-row preview limit"
        );
    }
    Ok(MermaidPreview {
        lines,
        notices: vec!["States use cards with complete outgoing transitions and attached notes; CSS, note placement and browser geometry use terminal equivalents; links are text".to_owned()],
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_transitions_self_loops_and_notes_remain_complete() {
        let source = "stateDiagram-v2\n[*] --> Pending: Create\nPending --> Pending: Edit fields\nPending --> Pending: Retry after conflict\nPending --> Approved: Approve, final validation\nApproved --> [*]\nnote right of Pending\nImmutable owner.\nNo decision metadata.\nend note\nnote left of Approved\nDecision recorded.\nend note";
        for columns in [30, 60, 100, 160] {
            let preview = render_mermaid(source, columns).unwrap();
            let output: String = preview.lines.iter().map(ToString::to_string).collect();
            let compact = |text: &str| {
                text.chars()
                    .filter(|ch| !ch.is_whitespace() && !"┌┐└┘─│".contains(*ch))
                    .collect::<String>()
            };
            for expected in [
                "● Start",
                "◎ End",
                "Edit fields",
                "Retry after conflict",
                "Approve, final validation",
                "Immutable owner.",
                "No decision metadata.",
                "Decision recorded.",
                "right of",
                "left of",
            ] {
                assert!(
                    compact(&output).contains(&compact(expected)),
                    "missing {expected}: {output}"
                );
            }
            assert!(
                preview
                    .lines
                    .iter()
                    .all(|line| line.width() <= usize::from(columns))
            );
        }
    }

    #[test]
    fn ambiguous_state_labels_retain_source_instead_of_inventing_states() {
        let error = render_mermaid("stateDiagram-v2\nA --> B: approved; final state valid", 80)
            .unwrap_err();
        assert!(error.to_string().contains("use a comma"));
        assert!(
            render_mermaid(
                "stateDiagram-v2\nA --> B\nnote right of A\nx --> y: note; text\nend note",
                80
            )
            .is_ok()
        );
    }
}
