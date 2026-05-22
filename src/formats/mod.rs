pub mod mermaid;
pub mod dot;
pub mod plantuml;

use std::path::Path;
use crate::data::{CanvasData, CanvasNode};
use anyhow::Result;

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum SupportedFormat {
    Canvas,
    Mermaid,
    Dot,
    PlantUml,
}

impl SupportedFormat {
    pub fn is_flowchart(self) -> bool {
        matches!(self, SupportedFormat::Mermaid | SupportedFormat::Dot | SupportedFormat::PlantUml)
    }
}

pub fn detect_format(path: &Path) -> SupportedFormat {
    match path.extension().and_then(|s| s.to_str()).unwrap_or("").to_lowercase().as_str() {
        "canvas" => SupportedFormat::Canvas,
        "md" | "markdown" | "mermaid" | "mmd" => SupportedFormat::Mermaid,
        "dot" | "gv" => SupportedFormat::Dot,
        "puml" | "plantuml" | "iuml" => SupportedFormat::PlantUml,
        _ => SupportedFormat::Canvas,
    }
}

pub fn load_from_format(_path: &Path, content: &str, format: SupportedFormat) -> Result<CanvasData> {
    let mut data = match format {
        SupportedFormat::Canvas => serde_json::from_str(content)?,
        SupportedFormat::Mermaid => mermaid::parse(content)?,
        SupportedFormat::Dot => dot::parse(content)?,
        SupportedFormat::PlantUml => plantuml::parse(content)?,
    };

    // Determine if the layout needs initialization.
    // If any nodes have non-zero positions, we keep the existing layout.
    let has_any_layout = data.nodes.iter().any(|n| {
        let (x, y) = n.pos();
        x.abs() > 0.01 || y.abs() > 0.01
    });

    if !has_any_layout && !data.nodes.is_empty() {
        match format {
            SupportedFormat::Canvas => apply_force_directed_layout(&mut data),
            _ => apply_hierarchical_layout(&mut data),
        }
    }

    Ok(data)
}

pub fn save_to_format(data: &CanvasData, original_content: &str, format: SupportedFormat, write_layout: bool) -> Result<String> {
    match format {
        SupportedFormat::Canvas => serde_json::to_string_pretty(data).map_err(Into::into),
        SupportedFormat::Mermaid => mermaid::serialize(data, original_content, write_layout),
        SupportedFormat::Dot => dot::serialize(data, write_layout),
        SupportedFormat::PlantUml => plantuml::serialize(data, write_layout),
    }
}

pub fn apply_force_directed_layout(data: &mut CanvasData) {
    let n = data.nodes.len();
    if n == 0 {
        return;
    }

    let mut positions = vec![(0.0, 0.0); n];

    // Initial layout in a neat circle to prevent node collapse
    let radius = 150.0 * (n as f64).sqrt();
    for i in 0..n {
        let angle = 2.0 * std::f64::consts::PI * (i as f64) / (n as f64);
        positions[i] = (angle.cos() * radius, angle.sin() * radius);
    }

    let iterations = 80;
    let area = 1200.0 * 1200.0;
    let k = (area / n as f64).sqrt() * 0.6; // Optimal node distance
    let mut temp = 150.0;

    let node_ids: Vec<String> = data.nodes.iter().map(|node| node.id().to_string()).collect();

    for _ in 0..iterations {
        let mut displacements = vec![(0.0, 0.0); n];

        // Repulsive forces between all node pairs
        for i in 0..n {
            for j in 0..n {
                if i == j { continue; }
                let dx = positions[i].0 - positions[j].0;
                let dy = positions[i].1 - positions[j].1;
                let dist = (dx * dx + dy * dy).sqrt().max(1.0);
                
                // Force decreases with distance
                let force = (k * k) / dist;
                displacements[i].0 += (dx / dist) * force;
                displacements[i].1 += (dy / dist) * force;
            }
        }

        // Attractive forces along edges
        for edge in &data.edges {
            let from_idx = node_ids.iter().position(|id| id == &edge.from_node);
            let to_idx = node_ids.iter().position(|id| id == &edge.to_node);
            if let (Some(i), Some(j)) = (from_idx, to_idx) {
                let dx = positions[i].0 - positions[j].0;
                let dy = positions[i].1 - positions[j].1;
                let dist = (dx * dx + dy * dy).sqrt().max(1.0);
                
                // Force increases with distance
                let force = (dist * dist) / k;
                let ux = dx / dist;
                let uy = dy / dist;

                displacements[i].0 -= ux * force;
                displacements[i].1 -= uy * force;
                displacements[j].0 += ux * force;
                displacements[j].1 += uy * force;
            }
        }

        // Apply and bound displacements
        for i in 0..n {
            let disp_x = displacements[i].0;
            let disp_y = displacements[i].1;
            let disp_len = (disp_x * disp_x + disp_y * disp_y).sqrt().max(1.0);
            
            let capped_disp = disp_len.min(temp);
            positions[i].0 += (disp_x / disp_len) * capped_disp;
            positions[i].1 += (disp_y / disp_len) * capped_disp;
        }

        // Cooldown temperature linearly
        temp *= 0.92;
    }

    // Copy positions and update dimensions back to nodes
    for (i, node) in data.nodes.iter_mut().enumerate() {
        let (x, y) = positions[i];
        
        // Calculate intuitive default dimensions based on label text
        let label = node.text();
        let width = (label.len() * 9).max(120).min(400) as f64;
        let height = (2 + label.lines().count() * 20).max(60).min(200) as f64;

        match node {
            CanvasNode::Text(n) => { n.x = x - width/2.0; n.y = y - height/2.0; n.width = width; n.height = height; },
            CanvasNode::File(n) => { n.x = x - width/2.0; n.y = y - height/2.0; n.width = width; n.height = height; },
            CanvasNode::Link(n) => { n.x = x - width/2.0; n.y = y - height/2.0; n.width = width; n.height = height; },
            CanvasNode::Group(n) => { n.x = x - width/2.0; n.y = y - height/2.0; n.width = width; n.height = height; },
        }
    }
}

pub fn apply_hierarchical_layout(data: &mut CanvasData) {
    let n = data.nodes.len();
    if n == 0 {
        return;
    }

    use std::collections::{HashMap, VecDeque};
    let node_ids: Vec<String> = data.nodes.iter().map(|node| node.id().to_string()).collect();
    let mut in_degree: HashMap<String, usize> = HashMap::new();
    let mut adj: HashMap<String, Vec<String>> = HashMap::new();

    for id in &node_ids {
        in_degree.insert(id.clone(), 0);
        adj.insert(id.clone(), Vec::new());
    }

    for edge in &data.edges {
        if in_degree.contains_key(&edge.from_node) && in_degree.contains_key(&edge.to_node) {
            adj.get_mut(&edge.from_node).unwrap().push(edge.to_node.clone());
            *in_degree.get_mut(&edge.to_node).unwrap() += 1;
        }
    }

    let mut ranks: HashMap<String, usize> = HashMap::new();
    let mut queue = VecDeque::new();

    for id in &node_ids {
        if *in_degree.get(id).unwrap_or(&0) == 0 {
            ranks.insert(id.clone(), 0);
            queue.push_back((id.clone(), 0));
        }
    }

    if queue.is_empty() {
        for id in &node_ids {
            ranks.insert(id.clone(), 0);
            queue.push_back((id.clone(), 0));
        }
    }

    while let Some((current, rank)) = queue.pop_front() {
        if let Some(neighbors) = adj.get(&current) {
            for neighbor in neighbors {
                let current_rank = ranks.get(neighbor).copied().unwrap_or(0);
                if rank + 1 < node_ids.len() && (!ranks.contains_key(neighbor) || rank + 1 > current_rank) {
                    ranks.insert(neighbor.clone(), rank + 1);
                    queue.push_back((neighbor.clone(), rank + 1));
                }
            }
        }
    }

    let mut rank_groups: HashMap<usize, Vec<String>> = HashMap::new();
    for (id, rank) in &ranks {
        rank_groups.entry(*rank).or_default().push(id.clone());
    }

    for id in &node_ids {
        if !ranks.contains_key(id) {
            rank_groups.entry(0).or_default().push(id.clone());
        }
    }

    for siblings in rank_groups.values_mut() {
        siblings.sort();
    }

    for node in &mut data.nodes {
        let id = node.id().to_string();
        let rank = ranks.get(&id).copied().unwrap_or(0);
        
        let idx_in_rank = if let Some(siblings) = rank_groups.get(&rank) {
            siblings.iter().position(|x| x == &id).unwrap_or(0)
        } else {
            0
        };

        let (px, py) = match data.orientation {
            crate::data::DiagramOrientation::LeftRight => {
                let stack_spacing = 350.0;
                let sibling_spacing = 150.0;
                let num_siblings = rank_groups.get(&rank).map(|s| s.len()).unwrap_or(1);
                let y_offset = -(num_siblings as f64 - 1.0) * sibling_spacing / 2.0;
                let px = (rank as f64) * stack_spacing;
                let py = y_offset + (idx_in_rank as f64) * sibling_spacing;
                (px, py)
            }
            crate::data::DiagramOrientation::RightLeft => {
                let stack_spacing = 350.0;
                let sibling_spacing = 150.0;
                let num_siblings = rank_groups.get(&rank).map(|s| s.len()).unwrap_or(1);
                let y_offset = -(num_siblings as f64 - 1.0) * sibling_spacing / 2.0;
                let px = -(rank as f64) * stack_spacing;
                let py = y_offset + (idx_in_rank as f64) * sibling_spacing;
                (px, py)
            }
            crate::data::DiagramOrientation::DownTop => {
                let vertical_spacing = 200.0;
                let horizontal_spacing = 250.0;
                let num_siblings = rank_groups.get(&rank).map(|s| s.len()).unwrap_or(1);
                let x_offset = -(num_siblings as f64 - 1.0) * horizontal_spacing / 2.0;
                let px = x_offset + (idx_in_rank as f64) * horizontal_spacing;
                let py = -(rank as f64) * vertical_spacing;
                (px, py)
            }
            _ => {
                let vertical_spacing = 200.0;
                let horizontal_spacing = 250.0;
                let num_siblings = rank_groups.get(&rank).map(|s| s.len()).unwrap_or(1);
                let x_offset = -(num_siblings as f64 - 1.0) * horizontal_spacing / 2.0;
                let px = x_offset + (idx_in_rank as f64) * horizontal_spacing;
                let py = (rank as f64) * vertical_spacing;
                (px, py)
            }
        };

        let label = node.text();
        let width = (label.len() * 9).max(120).min(400) as f64;
        let height = (2 + label.lines().count() * 20).max(60).min(200) as f64;

        let x = px - width / 2.0;
        let y = py - height / 2.0;

        match node {
            CanvasNode::Text(n) => { n.x = x; n.y = y; n.width = width; n.height = height; }
            CanvasNode::File(n) => { n.x = x; n.y = y; n.width = width; n.height = height; }
            CanvasNode::Link(n) => { n.x = x; n.y = y; n.width = width; n.height = height; }
            CanvasNode::Group(n) => { n.x = x; n.y = y; n.width = width; n.height = height; }
        }
    }
}
