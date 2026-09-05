mod app;

use std::path::PathBuf;

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();

    if args.len() != 2 {
        eprintln!("Usage: pinstar <FILE>.canvas|.md|.dot|.puml");
        std::process::exit(1);
    }

    let path = PathBuf::from(&args[1]);
    let format = pinstar::formats::detect_format(&path);

    if !path.exists() {
        let initial_content = match format {
            pinstar::formats::SupportedFormat::Canvas => {
                let empty = serde_json::json!({
                    "nodes": [],
                    "edges": []
                });
                serde_json::to_string_pretty(&empty)?
            }
            pinstar::formats::SupportedFormat::Mermaid => {
                if path.extension().and_then(|s| s.to_str()) == Some("md") {
                    "# Diagram\n\n```mermaid\ngraph TD\n```\n".to_string()
                } else {
                    "graph TD\n".to_string()
                }
            }
            pinstar::formats::SupportedFormat::Dot => "digraph G {\n}\n".to_string(),
            pinstar::formats::SupportedFormat::PlantUml => "@startuml\n@enduml\n".to_string(),
        };
        std::fs::write(&path, initial_content)?;
        let format_name = match format {
            pinstar::formats::SupportedFormat::Canvas => "Canvas",
            pinstar::formats::SupportedFormat::Mermaid => "Mermaid",
            pinstar::formats::SupportedFormat::Dot => "DOT",
            pinstar::formats::SupportedFormat::PlantUml => "PlantUML",
        };
        eprintln!("Created empty {format_name} diagram: {}", path.display());
    }

    app::run_pinstar(path)
}
