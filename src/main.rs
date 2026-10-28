mod app;

use std::path::PathBuf;

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();

    if args.len() == 2 && (args[1] == "-V" || args[1] == "--version") {
        println!("pinstar {}", env!("CARGO_PKG_VERSION"));
        std::process::exit(0);
    }

    if args.len() == 2 && (args[1] == "-h" || args[1] == "--help") {
        println!("Usage: pinstar <FILE>.canvas|.md|.dot|.puml");
        std::process::exit(0);
    }

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
        let created = create_diagram(&path, &initial_content)?;
        let format_name = match format {
            pinstar::formats::SupportedFormat::Canvas => "Canvas",
            pinstar::formats::SupportedFormat::Mermaid => "Mermaid",
            pinstar::formats::SupportedFormat::Dot => "DOT",
            pinstar::formats::SupportedFormat::PlantUml => "PlantUML",
        };
        if created {
            eprintln!("Created empty {format_name} diagram: {}", path.display());
        }
    }

    app::run_pinstar(path)
}

fn create_diagram(path: &std::path::Path, content: &str) -> anyhow::Result<bool> {
    use std::io::Write;
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = match options.open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => return Ok(false),
        Err(error) => return Err(error.into()),
    };
    file.write_all(content.as_bytes())?;
    file.sync_all()?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    #[test]
    fn creation_never_overwrites_an_existing_diagram() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("new.canvas");
        assert!(super::create_diagram(&path, "original").unwrap());
        assert!(!super::create_diagram(&path, "replacement").unwrap());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "original");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
    }
}
