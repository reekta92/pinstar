use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub enum DiagramOrientation {
    #[default]
    TopDown,
    LeftRight,
    RightLeft,
    DownTop,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CanvasData {
    pub nodes: Vec<CanvasNode>,
    pub edges: Vec<CanvasEdge>,
    #[serde(default)]
    pub orientation: DiagramOrientation,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum CanvasNode {
    Text(TextNode),
    File(FileNode),
    Link(LinkNode),
    Group(GroupNode),
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub enum NodeShape {
    #[default]
    Rectangle,
    Diamond,
    Circle,
    Cylinder,
    Stadium,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TextNode {
    pub id: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    pub color: Option<String>,
    #[serde(default)]
    pub shape: NodeShape,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FileNode {
    pub id: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub file: String,
    pub subpath: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    pub color: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LinkNode {
    pub id: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    pub color: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GroupNode {
    pub id: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub label: Option<String>,
    pub color: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub enum EdgeStyle {
    #[default]
    Solid,
    Dashed,
    Dotted,
    Thick,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CanvasEdge {
    pub id: String,
    pub from_node: String,
    pub from_side: Option<String>,
    pub to_node: String,
    pub to_side: Option<String>,
    pub label: Option<String>,
    pub color: Option<String>,
    #[serde(default)]
    pub style: EdgeStyle,
}

impl CanvasData {
    /// Validate native Canvas identities, geometry and connections without changing layout.
    pub fn validate(&self) -> anyhow::Result<()> {
        let mut node_ids = std::collections::HashSet::new();
        for node in &self.nodes {
            anyhow::ensure!(
                !node.id().is_empty() && node_ids.insert(node.id()),
                "Empty or duplicate node ID: {}",
                node.id()
            );
            let (x, y) = node.pos();
            let (width, height) = node.size();
            anyhow::ensure!(
                [x, y, width, height].iter().all(|value| value.is_finite())
                    && width > 0.0
                    && height > 0.0,
                "Node {} requires finite coordinates and positive dimensions",
                node.id()
            );
        }
        let mut edge_ids = std::collections::HashSet::new();
        for edge in &self.edges {
            anyhow::ensure!(
                !edge.id.is_empty() && edge_ids.insert(edge.id.as_str()),
                "Empty or duplicate edge ID: {}",
                edge.id
            );
            anyhow::ensure!(
                node_ids.contains(edge.from_node.as_str())
                    && node_ids.contains(edge.to_node.as_str()),
                "Edge {} references a missing node",
                edge.id
            );
            for side in [&edge.from_side, &edge.to_side].into_iter().flatten() {
                anyhow::ensure!(
                    matches!(side.as_str(), "top" | "right" | "bottom" | "left"),
                    "Unsupported edge side: {side}"
                );
            }
        }
        Ok(())
    }

    pub fn node(&self, id: &str) -> Option<&CanvasNode> {
        self.nodes.iter().find(|node| node.id() == id)
    }

    pub fn edge(&self, id: &str) -> Option<&CanvasEdge> {
        self.edges.iter().find(|edge| edge.id == id)
    }

    fn edit(&mut self, edit: impl FnOnce(&mut Self) -> anyhow::Result<()>) -> anyhow::Result<()> {
        let mut next = self.clone();
        edit(&mut next)?;
        next.validate()?;
        *self = next;
        Ok(())
    }

    pub fn add_node(&mut self, node: CanvasNode) -> anyhow::Result<()> {
        self.edit(|data| {
            data.nodes.push(node);
            Ok(())
        })
    }

    pub fn update_node(&mut self, id: &str, node: CanvasNode) -> anyhow::Result<()> {
        anyhow::ensure!(node.id() == id, "Updating a node cannot change its ID");
        self.edit(|data| {
            let current = data
                .nodes
                .iter_mut()
                .find(|node| node.id() == id)
                .ok_or_else(|| anyhow::anyhow!("Node not found: {id}"))?;
            *current = node;
            Ok(())
        })
    }

    pub fn delete_node(&mut self, id: &str, cascade: bool) -> anyhow::Result<()> {
        self.edit(|data| {
            anyhow::ensure!(data.node(id).is_some(), "Node not found: {id}");
            anyhow::ensure!(
                cascade
                    || !data
                        .edges
                        .iter()
                        .any(|edge| edge.from_node == id || edge.to_node == id),
                "Node {id} has connections; cascade is required"
            );
            data.nodes.retain(|node| node.id() != id);
            data.edges
                .retain(|edge| edge.from_node != id && edge.to_node != id);
            Ok(())
        })
    }

    pub fn add_edge(&mut self, edge: CanvasEdge) -> anyhow::Result<()> {
        self.edit(|data| {
            data.edges.push(edge);
            Ok(())
        })
    }

    pub fn update_edge(&mut self, id: &str, edge: CanvasEdge) -> anyhow::Result<()> {
        anyhow::ensure!(edge.id == id, "Updating an edge cannot change its ID");
        self.edit(|data| {
            let current = data
                .edges
                .iter_mut()
                .find(|edge| edge.id == id)
                .ok_or_else(|| anyhow::anyhow!("Edge not found: {id}"))?;
            *current = edge;
            Ok(())
        })
    }

    pub fn delete_edge(&mut self, id: &str) -> anyhow::Result<()> {
        self.edit(|data| {
            anyhow::ensure!(data.edge(id).is_some(), "Edge not found: {id}");
            data.edges.retain(|edge| edge.id != id);
            Ok(())
        })
    }
}

impl CanvasNode {
    pub fn id(&self) -> &str {
        match self {
            CanvasNode::Text(n) => &n.id,
            CanvasNode::File(n) => &n.id,
            CanvasNode::Link(n) => &n.id,
            CanvasNode::Group(n) => &n.id,
        }
    }

    pub fn pos(&self) -> (f64, f64) {
        match self {
            CanvasNode::Text(n) => (n.x, n.y),
            CanvasNode::File(n) => (n.x, n.y),
            CanvasNode::Link(n) => (n.x, n.y),
            CanvasNode::Group(n) => (n.x, n.y),
        }
    }

    pub fn size(&self) -> (f64, f64) {
        match self {
            CanvasNode::Text(n) => (n.width, n.height),
            CanvasNode::File(n) => (n.width, n.height),
            CanvasNode::Link(n) => (n.width, n.height),
            CanvasNode::Group(n) => (n.width, n.height),
        }
    }

    pub fn text(&self) -> &str {
        match self {
            CanvasNode::Text(n) => &n.text,
            CanvasNode::File(n) => &n.file,
            CanvasNode::Link(n) => &n.url,
            CanvasNode::Group(n) => n.label.as_deref().unwrap_or(""),
        }
    }

    pub fn set_text(&mut self, text: String) {
        match self {
            CanvasNode::Text(n) => n.text = text,
            CanvasNode::File(n) => n.file = text,
            CanvasNode::Link(n) => n.url = text,
            CanvasNode::Group(n) => n.label = Some(text),
        }
    }

    pub fn title(&self) -> Option<&str> {
        match self {
            CanvasNode::Text(n) => n.title.as_deref(),
            CanvasNode::File(n) => n.title.as_deref(),
            CanvasNode::Link(n) => n.title.as_deref(),
            CanvasNode::Group(n) => n.label.as_deref(),
        }
    }

    pub fn set_title(&mut self, title: Option<String>) {
        match self {
            CanvasNode::Text(n) => n.title = title,
            CanvasNode::File(n) => n.title = title,
            CanvasNode::Link(n) => n.title = title,
            CanvasNode::Group(n) => n.label = title,
        }
    }

    pub fn shape(&self) -> NodeShape {
        match self {
            CanvasNode::Text(n) => n.shape,
            _ => NodeShape::Rectangle,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn canvas() -> CanvasData {
        serde_json::from_value(serde_json::json!({
            "nodes": [
                {"type":"text","id":"a","x":0,"y":0,"width":10,"height":20,"text":"a","color":"1"},
                {"type":"file","id":"b","x":0,"y":0,"width":10,"height":20,"file":"image.png","color":"#ffffff"},
                {"type":"link","id":"c","x":0,"y":0,"width":10,"height":20,"url":"https://example.com"},
                {"type":"group","id":"d","x":0,"y":0,"width":10,"height":20}
            ],
            "edges": [
                {"id":"ab","fromNode":"a","toNode":"b","fromSide":"right","toSide":"left"},
                {"id":"cd","fromNode":"c","toNode":"d","fromSide":"top","toSide":"bottom"},
                {"id":"ba","fromNode":"b","toNode":"a"}
            ]
        })).unwrap()
    }

    #[test]
    fn model_validation_rejects_invalid_ids_geometry_references_and_sides() {
        let valid = canvas();
        valid.validate().unwrap();
        let mut invalid = valid.clone();
        invalid.nodes.push(valid.nodes[0].clone());
        assert!(invalid.validate().is_err());
        let mut invalid = valid.clone();
        if let CanvasNode::Text(node) = &mut invalid.nodes[0] {
            node.id.clear();
        }
        assert!(invalid.validate().is_err());
        for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            for field in ["x", "y", "width", "height"] {
                let mut invalid = valid.clone();
                if let CanvasNode::Text(node) = &mut invalid.nodes[0] {
                    match field {
                        "x" => node.x = value,
                        "y" => node.y = value,
                        "width" => node.width = value,
                        _ => node.height = value,
                    }
                }
                assert!(invalid.validate().is_err(), "{field}={value}");
            }
        }
        for value in [0.0, -1.0] {
            let mut invalid = valid.clone();
            if let CanvasNode::Text(node) = &mut invalid.nodes[0] {
                node.width = value;
            }
            assert!(invalid.validate().is_err());
            if let CanvasNode::Text(node) = &mut invalid.nodes[0] {
                node.width = 10.0;
                node.height = value;
            }
            assert!(invalid.validate().is_err());
        }
        let mut invalid = valid.clone();
        invalid.edges.push(valid.edges[0].clone());
        assert!(invalid.validate().is_err());
        let mut invalid = valid.clone();
        invalid.edges[0].id.clear();
        assert!(invalid.validate().is_err());
        for endpoint in [true, false] {
            let mut invalid = valid.clone();
            if endpoint {
                invalid.edges[0].from_node = "missing".into();
            } else {
                invalid.edges[0].to_node = "missing".into();
            }
            assert!(invalid.validate().is_err());
            let mut invalid = valid.clone();
            if endpoint {
                invalid.edges[0].from_side = Some("center".into());
            } else {
                invalid.edges[0].to_side = Some("".into());
            }
            assert!(invalid.validate().is_err());
        }
    }

    #[test]
    fn model_edits_are_transactional_and_keep_ids_and_order() {
        let mut data = canvas();
        let original = data.clone();
        assert!(data.add_node(original.nodes[0].clone()).is_err());
        assert!(data.add_edge(original.edges[0].clone()).is_err());
        assert!(data.update_node("a", original.nodes[1].clone()).is_err());
        assert!(data.update_edge("ab", original.edges[1].clone()).is_err());
        assert!(data.delete_node("a", false).is_err());
        assert!(data.delete_node("missing", true).is_err());
        assert!(data.delete_edge("missing").is_err());
        assert_eq!(data, original);
        let mut invalid_node = data.node("a").unwrap().clone();
        if let CanvasNode::Text(node) = &mut invalid_node {
            node.width = 0.0;
        }
        assert!(data.update_node("a", invalid_node).is_err());
        let mut invalid_edge = data.edge("ab").unwrap().clone();
        invalid_edge.to_node = "missing".into();
        assert!(data.update_edge("ab", invalid_edge).is_err());
        assert_eq!(data, original);
        data.update_node("a", original.nodes[0].clone()).unwrap();
        data.update_edge("ab", original.edges[0].clone()).unwrap();
        assert_eq!(data, original);
        let mut node = data.node("a").unwrap().clone();
        node.set_text("updated".into());
        data.update_node("a", node).unwrap();
        let mut edge = data.edge("ab").unwrap().clone();
        edge.label = Some("updated".into());
        data.update_edge("ab", edge).unwrap();
        assert_eq!(data.nodes[0].text(), "updated");
        assert_eq!(data.edges[0].label.as_deref(), Some("updated"));
        data.delete_node("a", true).unwrap();
        assert_eq!(
            data.nodes.iter().map(CanvasNode::id).collect::<Vec<_>>(),
            ["b", "c", "d"]
        );
        assert_eq!(
            data.edges
                .iter()
                .map(|edge| edge.id.as_str())
                .collect::<Vec<_>>(),
            ["cd"]
        );
        data.add_node(original.nodes[0].clone()).unwrap();
        data.add_edge(original.edges[0].clone()).unwrap();
        assert_eq!(data.nodes.last().unwrap().id(), "a");
        assert_eq!(data.edges.last().unwrap().id, "ab");
        data.delete_edge("cd").unwrap();
        data.delete_node("c", false).unwrap();
        assert_eq!(data.edges[0].id, "ab");
    }

    #[test]
    fn parses_canvas_json_without_orientation() {
        // Pre-orientation canvas files keep parsing (serde default).
        let data: CanvasData = serde_json::from_str(r#"{"nodes":[],"edges":[]}"#).unwrap();
        assert_eq!(data.orientation, DiagramOrientation::TopDown);
    }

    #[test]
    fn parses_nodes_with_and_without_title() {
        let data: CanvasData = serde_json::from_str(
            r##"{"nodes":[{"type":"text","id":"a","x":0,"y":0,"width":10,"height":10,"text":"hi","title":"T","color":"#ff0000"},
                          {"type":"file","id":"b","x":0,"y":0,"width":10,"height":10,"file":"x.png","subpath":null,"color":null}],
               "edges":[]}"##,
        )
        .unwrap();
        assert_eq!(data.nodes.len(), 2);
        assert_eq!(data.nodes[0].title(), Some("T"));
        assert_eq!(data.nodes[1].title(), None);
        // Titles round-trip; absent titles stay absent.
        let out = serde_json::to_string(&data).unwrap();
        assert!(out.contains(r#""title":"T""#));
        assert!(!out.contains(
            r#""id":"b","x":0,"y":0,"width":10,"height":10,"file":"x.png","subpath":null,"title""#
        ));
    }
}
