mod app;
mod data;
mod helpers;
mod input;
mod render;
mod state;

use std::path::PathBuf;

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();

    if args.len() != 2 {
        eprintln!("Usage: pinstar <FILE_NAME>.canvas");
        std::process::exit(1);
    }

    let path = PathBuf::from(&args[1]);

    let ext = path
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();

    if ext != "canvas" {
        eprintln!(
            "Error: '{}' does not have .canvas extension",
            path.display()
        );
        std::process::exit(1);
    }

    if !path.exists() {
        let empty = serde_json::json!({
            "nodes": [],
            "edges": []
        });
        std::fs::write(&path, serde_json::to_string_pretty(&empty)?)?;
        eprintln!("Created empty canvas: {}", path.display());
    }

    app::run_pinstar(path)
}
