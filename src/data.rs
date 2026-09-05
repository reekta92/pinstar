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

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CanvasData {
    pub nodes: Vec<CanvasNode>,
    pub edges: Vec<CanvasEdge>,
    #[serde(default)]
    pub orientation: DiagramOrientation,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
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

#[derive(Debug, Serialize, Deserialize, Clone)]
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

#[derive(Debug, Serialize, Deserialize, Clone)]
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

#[derive(Debug, Serialize, Deserialize, Clone)]
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

#[derive(Debug, Serialize, Deserialize, Clone)]
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

#[derive(Debug, Serialize, Deserialize, Clone)]
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

    #[test]
    fn parses_canvas_json_without_orientation() {
        // Pre-orientation canvas files keep parsing (serde default).
        let data: CanvasData =
            serde_json::from_str(r#"{"nodes":[],"edges":[]}"#).unwrap();
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
        assert!(!out.contains(r#""id":"b","x":0,"y":0,"width":10,"height":10,"file":"x.png","subpath":null,"title""#));
    }
}
