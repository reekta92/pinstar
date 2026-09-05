//! Standalone bin host: terminal lifecycle, external editor integration and
//! the event loop. All canvas logic comes from the `pinstar` library.

use pinstar::theme::ThemeColors;
use pinstar::{
    PinstarState, Settings, draw_pinstar_view, handle_pinstar_event, handle_pinstar_mouse,
};
use ratatui::Terminal;
use ratatui::prelude::*;
use std::io;
use std::path::PathBuf;
#[cfg(feature = "images")]
use std::sync::mpsc::Receiver;
use std::time::Duration;

pub struct TermGuard {
    terminal: Terminal<CrosstermBackend<io::Stdout>>,
}

impl TermGuard {
    pub fn new() -> anyhow::Result<Self> {
        let mut stdout = io::stdout();
        crossterm::terminal::enable_raw_mode()?;
        crossterm::execute!(stdout, crossterm::event::EnableMouseCapture)?;
        let backend = CrosstermBackend::new(stdout);
        let terminal = Terminal::new(backend)?;
        Ok(Self { terminal })
    }

    pub fn as_mut(&mut self) -> &mut Terminal<CrosstermBackend<io::Stdout>> {
        &mut self.terminal
    }

    fn suspend(&mut self) -> anyhow::Result<()> {
        crossterm::terminal::disable_raw_mode()?;
        crossterm::execute!(io::stdout(), crossterm::event::DisableMouseCapture)?;
        crossterm::execute!(io::stdout(), crossterm::terminal::LeaveAlternateScreen)?;
        Ok(())
    }

    fn resume(&mut self) -> anyhow::Result<()> {
        crossterm::execute!(io::stdout(), crossterm::terminal::EnterAlternateScreen)?;
        crossterm::terminal::enable_raw_mode()?;
        crossterm::execute!(io::stdout(), crossterm::event::EnableMouseCapture)?;
        self.terminal.clear()?;
        Ok(())
    }
}

impl Drop for TermGuard {
    fn drop(&mut self) {
        let _ = crossterm::terminal::disable_raw_mode();
        let _ = crossterm::execute!(io::stdout(), crossterm::event::DisableMouseCapture);
        let _ = crossterm::execute!(io::stdout(), crossterm::terminal::LeaveAlternateScreen);
        let _ = self.terminal.show_cursor();
    }
}

#[cfg(feature = "images")]
struct ImageWorker {
    rx: Receiver<anyhow::Result<pinstar::image::DecodedImage>>,
}

pub fn run_pinstar(path: PathBuf) -> anyhow::Result<()> {
    let mut guard = TermGuard::new()?;
    let mut state = PinstarState::load(&path)?;
    state.settings = Settings {
        enable_image_nodes: cfg!(feature = "images"),
        image_cache_size: 32,
        rename_uses_id: true,
    };
    #[cfg(feature = "images")]
    {
        let (tx, rx) = pinstar::image::spawn_worker();
        state.image_decode_tx = Some(tx);
        run_loop(&mut guard, &mut state, Some(ImageWorker { rx }), &path)
    }
    #[cfg(not(feature = "images"))]
    run_loop(&mut guard, &mut state, None, &path)
}

fn run_loop(
    guard: &mut TermGuard,
    state: &mut PinstarState,
    #[cfg(feature = "images")] worker: Option<ImageWorker>,
    #[cfg(not(feature = "images"))] _worker: Option<()>,
    path: &PathBuf,
) -> anyhow::Result<()> {
    let theme = ThemeColors::default();
    let mut running = true;

    let external_editor = std::env::var("VISUAL")
        .or_else(|_| std::env::var("EDITOR"))
        .unwrap_or_else(|_| "vi".to_string());

    while running {
        #[cfg(feature = "images")]
        if let Some(worker) = &worker {
            while let Ok(result) = worker.rx.try_recv() {
                if let (Ok(img), Some(picker)) = (result, state.image_picker.as_ref()) {
                    state.image_cache.install_decoded(img, picker);
                }
            }
        }

        if state.trigger_whole_file_editor {
            state.trigger_whole_file_editor = false;

            let parts: Vec<&str> = external_editor.split_whitespace().collect();
            let (program, editor_args) = parts
                .split_first()
                .map(|(p, a)| (*p, a.to_vec()))
                .unwrap_or(("vi", vec![]));

            let mut command = std::process::Command::new("x-terminal-emulator");
            command.arg("-e").arg(program);
            for arg in &editor_args {
                command.arg(arg);
            }
            command.arg(&state.path);

            if command.spawn().is_err() {
                // If generic x-terminal-emulator isn't present, gracefully degrade
                // to suspended inline terminal edit
                let _ = guard.suspend();
                let mut fallback = std::process::Command::new(program);
                for arg in &editor_args {
                    fallback.arg(arg);
                }
                fallback.arg(&state.path);
                let _ = fallback.status();
                let _ = guard.resume();
                let _ = state.reload();
            }
        }

        if state.trigger_ext_editor {
            state.trigger_ext_editor = false;
            if let Some(node_id) = &state.selection.primary {
                let node_text = state
                    .data
                    .nodes
                    .iter()
                    .find(|n| n.id() == node_id)
                    .map(|n| n.text().to_string())
                    .unwrap_or_default();

                let temp_dir = std::env::temp_dir();
                let temp_id = uuid::Uuid::new_v4().to_string();
                let temp_file_path = temp_dir.join(format!("clin_pinstar_{temp_id}.md"));
                std::fs::write(&temp_file_path, &node_text)?;

                guard.suspend()?;

                let parts: Vec<&str> = external_editor.split_whitespace().collect();
                let (program, editor_args) = parts
                    .split_first()
                    .map(|(p, a)| (*p, a.to_vec()))
                    .unwrap_or(("vi", vec![]));

                let mut command = std::process::Command::new(program);
                for arg in &editor_args {
                    command.arg(arg);
                }
                command.arg(&temp_file_path);
                let _ = command.status();

                guard.resume()?;

                if let Ok(new_text) = std::fs::read_to_string(&temp_file_path)
                    && new_text != node_text
                {
                    for node in &mut state.data.nodes {
                        if node.id() == node_id {
                            node.set_text(new_text);
                            break;
                        }
                    }
                    let _ = state.save();
                }
                let _ = std::fs::remove_file(&temp_file_path);
            }
        }

        if let Ok(metadata) = std::fs::metadata(path) {
            if let Ok(modified) = metadata.modified() {
                if modified > state.last_modified {
                    let _ = state.reload();
                }
            }
        }

        guard.as_mut().draw(|frame| {
            let area = frame.area();
            draw_pinstar_view(frame, state, &theme, area, state.mouse_pos);
        })?;

        if crossterm::event::poll(Duration::from_millis(100))? {
            loop {
                let area = guard.as_mut().size()?;
                match crossterm::event::read()? {
                    crossterm::event::Event::Key(key) => {
                        handle_pinstar_event(state, key, &mut running, area.into());
                    }
                    crossterm::event::Event::Mouse(mouse) => {
                        state.mouse_pos = Some((mouse.column, mouse.row));
                        let _ = handle_pinstar_mouse(state, mouse, area.into());
                    }
                    _ => {}
                }
                if !crossterm::event::poll(Duration::ZERO)? {
                    break;
                }
            }
        }
    }

    let _ = state.save();
    Ok(())
}
