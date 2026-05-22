mod app;
mod data;
mod formats;
mod helpers;
mod input;
mod render;
mod state;

use std::path::PathBuf;

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();

    if args.len() != 2 {
        eprintln!("Usage: pinstar <FILE_NAME>");
        std::process::exit(1);
    }

    let path = PathBuf::from(&args[1]);
    let format = formats::detect_format(&path);

    if !path.exists() {
        let initial_content = match format {
            formats::SupportedFormat::Canvas => {
                let empty = serde_json::json!({
                    "nodes": [],
                    "edges": []
                });
                serde_json::to_string_pretty(&empty)?
            }
            formats::SupportedFormat::Mermaid => {
                if path.extension().and_then(|s| s.to_str()) == Some("md") {
                    "# Diagram\n\n```mermaid\ngraph TD\n```\n".to_string()
                } else {
                    "graph TD\n".to_string()
                }
            }
            formats::SupportedFormat::Dot => {
                "digraph G {\n}\n".to_string()
            }
            formats::SupportedFormat::PlantUml => {
                "@startuml\n@enduml\n".to_string()
            }
        };
        std::fs::write(&path, initial_content)?;
        eprintln!("Created empty diagram: {}", path.display());
    }

    app::run_pinstar(path)
}
