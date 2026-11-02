use super::*;
use merman_core::diagrams::er::ErDiagramRenderModel;

pub(super) fn render(model: ErDiagramRenderModel, columns: u16) -> Result<MermaidPreview> {
    ensure!(
        model.entities.len() <= MAX_NODES && model.relationships.len() <= MAX_EDGES,
        "Mermaid ER diagram exceeds preview limit (128 entities, 256 relationships)"
    );
    let mut lines = Vec::new();
    let mut names = BTreeMap::new();
    for (name, entity) in &model.entities {
        ensure!(
            names.insert(entity.id.as_str(), name.as_str()).is_none(),
            "Duplicate ER entity"
        );
        let mut label = name.clone();
        if !entity.alias.is_empty() && entity.alias != *name {
            label.push_str(&format!(" ({})", entity.alias));
        }
        for attribute in &entity.attributes {
            label.push_str(&format!("\n{}: {}", attribute.name, attribute.ty));
            if !attribute.keys.is_empty() {
                label.push_str(&format!(" [{}]", attribute.keys.join(",")));
            }
            if !attribute.comment.is_empty() {
                label.push_str(&format!(" — {}", attribute.comment));
            }
        }
        lines.extend(card(&label, columns, Style::default())?);
        ensure!(
            lines.len() <= usize::from(MAX_ROWS),
            "Mermaid ER diagram exceeds 512-row preview limit"
        );
    }
    lines.extend(text_lines(
        "Relationships",
        columns,
        Style::default().add_modifier(Modifier::BOLD),
    ));
    for relationship in &model.relationships {
        let from = names
            .get(relationship.entity_a.as_str())
            .ok_or_else(|| anyhow::anyhow!("Unknown ER relationship source"))?;
        let to = names
            .get(relationship.entity_b.as_str())
            .ok_or_else(|| anyhow::anyhow!("Unknown ER relationship target"))?;
        // Mermaid names cardA for the right endpoint and cardB for the left.
        let left = cardinality(&relationship.rel_spec.card_b, true)?;
        let right = cardinality(&relationship.rel_spec.card_a, false)?;
        let stroke = match relationship.rel_spec.rel_type.as_str() {
            "IDENTIFYING" => "──",
            "NON_IDENTIFYING" => "┄┄",
            _ => anyhow::bail!("Unsupported ER relationship type"),
        };
        lines.extend(text_lines(
            &format!("{from} {left}{stroke}{right} {to}: {}", relationship.role_a),
            columns,
            Style::default(),
        ));
    }
    lines.extend(text_lines(
        "|| exactly one; o| zero or one; |{ one or more; o{ zero or more. Solid: identifying; dotted: non-identifying.",
        columns, Style::default(),
    ));
    Ok(MermaidPreview {
        lines,
        notices: vec!["ER diagrams use entity cards and complete cardinality-labelled relationships; CSS and browser geometry use terminal equivalents".to_owned()],
    })
}

fn cardinality(value: &str, left: bool) -> Result<&'static str> {
    Ok(match (value, left) {
        ("ONLY_ONE", _) => "||",
        ("ZERO_OR_ONE", true) => "|o",
        ("ZERO_OR_ONE", false) => "o|",
        ("ONE_OR_MORE", true) => "}|",
        ("ONE_OR_MORE", false) => "|{",
        ("ZERO_OR_MORE", true) => "}o",
        ("ZERO_OR_MORE", false) => "o{",
        _ => anyhow::bail!("Unsupported ER cardinality"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entities_attributes_comments_and_cardinality_remain_complete() {
        let source = "erDiagram\npeople ||--o{ tasks : owns\npeople |o..o| accounts : links\npeople {\nbigint id PK,FK \"shared identifier\"\n}\ntasks {\ntext name UK\n}\naudit {\njsonb before \"nullable\"\n}";
        for columns in [30, 60, 100, 160] {
            let preview = render_mermaid(source, columns).unwrap();
            let output: String = preview.lines.iter().map(ToString::to_string).collect();
            let compact = |text: &str| {
                text.chars()
                    .filter(|ch| !ch.is_whitespace() && !"┌┐└┘─│".contains(*ch))
                    .collect::<String>()
            };
            for expected in [
                "people",
                "tasks",
                "audit",
                "id: bigint [PK,FK]",
                "shared identifier",
                "name: text [UK]",
                "before: jsonb",
                "nullable",
                "people ||──o{ tasks: owns",
                "people |o┄┄o| accounts: links",
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
}
