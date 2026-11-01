use super::*;
use merman_core::diagrams::sequence::SequenceDiagramRenderModel;

pub(super) fn render(model: SequenceDiagramRenderModel, columns: u16) -> Result<MermaidPreview> {
    ensure!(
        model.actors.len() <= MAX_NODES && model.messages.len() <= MAX_EDGES,
        "Mermaid sequence exceeds preview limit (128 participants, 256 events)"
    );
    ensure!(
        !model.actor_order.is_empty() && model.actor_order.len() == model.actors.len(),
        "Invalid sequence participant order"
    );
    ensure!(
        model.boxes.is_empty()
            && model.created_actors.is_empty()
            && model.destroyed_actors.is_empty(),
        "Participant boxes, creation and destruction are not supported in sequence previews"
    );
    let mut depth = 0usize;
    let mut max_depth = 0usize;
    for event in &model.messages {
        match event.message_type {
            10 | 12 | 15 => {
                depth += 1;
                max_depth = max_depth.max(depth);
            }
            11 | 14 | 16 => {
                ensure!(depth > 0, "Unmatched sequence block end");
                depth -= 1;
            }
            0 | 1 | 13 => {}
            other => anyhow::bail!("Unsupported sequence event type {other}"),
        }
    }
    ensure!(
        depth == 0 && max_depth <= MAX_DEPTH,
        "Sequence block nesting exceeds preview limit"
    );
    let margin = (max_depth * 2 + 2) as u16;
    ensure!(
        usize::from(columns) >= usize::from(margin) + 4 * model.actors.len(),
        "Sequence participants need a wider preview"
    );
    let mut positions = BTreeMap::new();
    let mut lines = Vec::new();
    for (index, id) in model.actor_order.iter().enumerate() {
        let actor = model
            .actors
            .get(id)
            .ok_or_else(|| anyhow::anyhow!("Unknown sequence participant"))?;
        let x = margin
            + (usize::from(columns - margin - 2) * index
                / model.actors.len().saturating_sub(1).max(1)) as u16;
        ensure!(
            positions.insert(id.as_str(), x).is_none(),
            "Duplicate sequence participant"
        );
        let kind = if actor.actor_type == "actor" {
            "actor"
        } else {
            "participant"
        };
        lines.extend(text_lines(
            &format!("{}: {} ({kind}; {id})", index + 1, actor.description),
            columns,
            Style::default(),
        ));
        ensure!(
            actor.links.is_empty() && actor.properties.is_empty(),
            "Participant links and properties are not supported in sequence previews"
        );
    }
    let mut header = Buffer::empty(Rect::new(0, 0, columns, 1));
    for (index, id) in model.actor_order.iter().enumerate() {
        header.set_string(
            positions[id.as_str()],
            0,
            (index + 1).to_string(),
            Style::default().add_modifier(Modifier::BOLD),
        );
    }
    lines.extend(buffer_lines(&header));
    let mut groups = Vec::new();
    for event in &model.messages {
        match event.message_type {
            10 | 12 | 15 => {
                let group = match event.message_type {
                    10 => "loop",
                    12 => "alt",
                    _ => "opt",
                };
                group_text(
                    &mut lines,
                    &groups,
                    &format!("┌ {group}: {}", event.message_text()),
                    columns,
                )?;
                groups.push(group);
            }
            13 => {
                ensure!(groups.last() == Some(&"alt"), "Unmatched sequence else");
                group_text(
                    &mut lines,
                    &groups[..groups.len() - 1],
                    &format!("├ else: {}", event.message_text()),
                    columns,
                )?;
            }
            11 | 14 | 16 => {
                let expected = match event.message_type {
                    11 => "loop",
                    14 => "alt",
                    _ => "opt",
                };
                ensure!(
                    groups.pop() == Some(expected),
                    "Mismatched sequence block end"
                );
                group_text(&mut lines, &groups, &format!("└ end {expected}"), columns)?;
            }
            0 | 1 => {
                let from = event
                    .from
                    .as_deref()
                    .ok_or_else(|| anyhow::anyhow!("Missing sequence sender"))?;
                let to = event
                    .to
                    .as_deref()
                    .ok_or_else(|| anyhow::anyhow!("Missing sequence receiver"))?;
                let start = *positions
                    .get(from)
                    .ok_or_else(|| anyhow::anyhow!("Unknown sequence sender"))?;
                let end = *positions
                    .get(to)
                    .ok_or_else(|| anyhow::anyhow!("Unknown sequence receiver"))?;
                let arrow = if event.message_type == 1 {
                    "-->>"
                } else {
                    "->>"
                };
                group_text(
                    &mut lines,
                    &groups,
                    &format!("{from} {arrow} {to}: {}", event.message_text()),
                    columns,
                )?;
                let mut row = lifelines(&positions, &groups, columns);
                let stroke = if event.message_type == 1 {
                    '┄'
                } else {
                    '─'
                };
                if start == end {
                    let right = start + 4 < columns;
                    let side = if right { start + 3 } else { start - 3 };
                    for x in start.min(side)..=start.max(side) {
                        put(&mut row, x, 0, stroke, Style::default());
                    }
                    put(
                        &mut row,
                        side,
                        0,
                        if right { '┐' } else { '┌' },
                        Style::default(),
                    );
                    lines.extend(buffer_lines(&row));
                    row = lifelines(&positions, &groups, columns);
                    for x in start.min(side)..=start.max(side) {
                        put(&mut row, x, 0, stroke, Style::default());
                    }
                    put(
                        &mut row,
                        side,
                        0,
                        if right { '┘' } else { '└' },
                        Style::default(),
                    );
                    put(
                        &mut row,
                        start,
                        0,
                        if right { '◀' } else { '▶' },
                        Style::default(),
                    );
                } else {
                    for x in start.min(end)..=start.max(end) {
                        put(&mut row, x, 0, stroke, Style::default());
                    }
                    put(
                        &mut row,
                        start,
                        0,
                        if start < end { '├' } else { '┤' },
                        Style::default(),
                    );
                    put(
                        &mut row,
                        end,
                        0,
                        if start < end { '▶' } else { '◀' },
                        Style::default(),
                    );
                }
                lines.extend(buffer_lines(&row));
                ensure!(
                    !event.activate,
                    "Sequence activations are not supported in previews"
                );
            }
            _ => unreachable!("event types validated before drawing"),
        }
        ensure!(
            lines.len() <= usize::from(MAX_ROWS),
            "Mermaid sequence exceeds 512-row preview limit"
        );
    }
    ensure!(groups.is_empty(), "Unclosed sequence block");
    Ok(MermaidPreview {
        lines,
        notices: vec!["Sequence lifelines use numbered participant headers; full messages and nested conditions wrap above their arrows".to_owned()],
    })
}

fn group_text(
    lines: &mut Vec<Line<'static>>,
    groups: &[&str],
    text: &str,
    columns: u16,
) -> Result<()> {
    let prefix = "│ ".repeat(groups.len());
    let width = columns.saturating_sub((groups.len() * 2) as u16);
    ensure!(width >= 8, "Sequence nesting needs a wider preview");
    for row in wrap_text(&plain_label(text), usize::from(width)) {
        lines.push(Line::raw(format!("{prefix}{row}")));
    }
    Ok(())
}

fn lifelines(positions: &BTreeMap<&str, u16>, groups: &[&str], columns: u16) -> Buffer {
    let mut buffer = Buffer::empty(Rect::new(0, 0, columns, 1));
    for x in positions.values() {
        put(&mut buffer, *x, 0, '│', Style::default());
    }
    for index in 0..groups.len() {
        put(&mut buffer, (index * 2) as u16, 0, '│', Style::default());
    }
    buffer
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sequence_preserves_aliases_order_replies_self_calls_and_nested_conditions() {
        let source = "sequenceDiagram\nactor User\nparticipant UI as Browser / Application\nparticipant API as Service\nparticipant IdP as Identity provider\nparticipant DB as Database\nUser->>UI: Sign in\nopt Unknown key\nAPI->>IdP: Fetch keys\nIdP-->>API: Key response\nend\nAPI->>API: Validate token\nalt Accepted\nAPI->>DB: Lookup account\nalt Active\nDB-->>API: Account\nelse Inactive\nAPI-->>UI: Denied\nend\nelse Invalid\nAPI-->>UI: Unauthorized\nend";
        for columns in [60, 100, 160] {
            let preview = render_mermaid(source, columns).unwrap();
            let output = preview
                .lines
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("\n");
            for expected in [
                "Browser / Application",
                "Identity provider",
                "(actor; User)",
                "Sign in",
                "Fetch keys",
                "Key response",
                "Validate token",
                "Lookup account",
                "Account",
                "Denied",
                "Unauthorized",
                "┌ opt",
                "┌ alt",
                "├ else",
                "└ end alt",
                "┄",
                "▶",
                "◀",
            ] {
                assert!(output.contains(expected), "missing {expected}: {output}");
            }
            assert!(
                output.find("Validate token").unwrap() < output.find("Lookup account").unwrap()
            );
            assert!(
                preview
                    .lines
                    .iter()
                    .all(|line| line.width() <= usize::from(columns))
            );
        }
    }

    #[test]
    fn unsupported_sequence_events_never_produce_partial_diagrams() {
        for source in [
            "sequenceDiagram\nA->>B: Hi\nactivate B",
            "sequenceDiagram\nA->>B: Hi\npar first\nB->>A: One\nand second\nB->>A: Two\nend",
        ] {
            assert!(render_mermaid(source, 80).is_err());
        }
    }
}
