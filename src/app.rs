use crate::helpers::PinstarTheme;
use crate::input::{handle_pinstar_event, handle_pinstar_mouse};
use crate::render::draw_pinstar_view;
use crate::state::PinstarState;
use crossterm::event::{self, Event};
use ratatui::prelude::*;
use ratatui::Terminal;
use std::io;
use std::io::Write;
use std::path::PathBuf;
use std::time::Duration;

pub struct TermGuard {
    terminal: Terminal<CrosstermBackend<io::Stdout>>,
}

impl TermGuard {
    pub fn new() -> anyhow::Result<Self> {
        crossterm::terminal::enable_raw_mode()?;
        let mut stdout = io::stdout();
        crossterm::execute!(
            stdout,
            crossterm::terminal::EnterAlternateScreen,
            crossterm::event::EnableMouseCapture
        )?;
        let backend = CrosstermBackend::new(stdout);
        let terminal = Terminal::new(backend)?;
        Ok(Self { terminal })
    }

    pub fn as_mut(&mut self) -> &mut Terminal<CrosstermBackend<io::Stdout>> {
        &mut self.terminal
    }

    fn suspend(&mut self) -> anyhow::Result<()> {
        crossterm::terminal::disable_raw_mode()?;
        crossterm::execute!(
            self.terminal.backend_mut(),
            crossterm::terminal::LeaveAlternateScreen,
            crossterm::event::DisableMouseCapture
        )?;
        self.terminal.show_cursor()?;
        io::stdout().flush()?;
        Ok(())
    }

    fn resume(&mut self) -> anyhow::Result<()> {
        crossterm::terminal::enable_raw_mode()?;
        crossterm::execute!(
            self.terminal.backend_mut(),
            crossterm::terminal::EnterAlternateScreen,
            crossterm::event::EnableMouseCapture
        )?;
        self.terminal.clear()?;
        Ok(())
    }
}

impl Drop for TermGuard {
    fn drop(&mut self) {
        let _ = crossterm::terminal::disable_raw_mode();
        let _ = crossterm::execute!(
            self.terminal.backend_mut(),
            crossterm::terminal::LeaveAlternateScreen,
            crossterm::event::DisableMouseCapture
        );
        let _ = self.terminal.show_cursor();
        let _ = io::stdout().flush();
    }
}

pub fn run_pinstar(path: PathBuf) -> anyhow::Result<()> {
    let mut guard = TermGuard::new()?;
    let mut state = PinstarState::load(&path)?;
    let theme = PinstarTheme::default();
    let mut running = true;

    let external_editor = std::env::var("VISUAL")
        .or_else(|_| std::env::var("EDITOR"))
        .unwrap_or_else(|_| "vi".to_string());

    while running {
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
                // If generic x-terminal-emulator isn't present, gracefully degrade to suspended inline terminal edit
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
            if let Some(node_id) = &state.selected_node_id {
                let node_text = state
                    .data
                    .nodes
                    .iter()
                    .find(|n| n.id() == node_id)
                    .map(|n| n.text().to_string())
                    .unwrap_or_default();

                let temp_dir = std::env::temp_dir();
                let temp_id = uuid::Uuid::new_v4().to_string();
                let temp_file_path = temp_dir.join(format!("clin_pinstar_{}.md", temp_id));
                std::fs::write(&temp_file_path, &node_text)?;

                guard.suspend()?;

                let parts: Vec<&str> = external_editor.split_whitespace().collect();
                let (program, editor_args) = parts
                    .split_first()
                    .map(|(p, a)| (*p, a.to_vec()))
                    .unwrap_or(("vi", vec![]));

                let mut command = std::process::Command::new(program);
                for arg in editor_args {
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

        if let Ok(metadata) = std::fs::metadata(&path) {
            if let Ok(modified) = metadata.modified() {
                if modified > state.last_modified {
                    let _ = state.reload();
                }
            }
        }

        guard.as_mut().draw(|frame| {
            draw_pinstar_view(frame, &mut state, &theme);
        })?;

        if event::poll(Duration::from_millis(100))? {
            loop {
                let area = guard.as_mut().size()?;
                match event::read()? {
                    Event::Key(key) => {
                        handle_pinstar_event(&mut state, key, &mut running, area.into());
                    }
                    Event::Mouse(mouse) => {
                        handle_pinstar_mouse(&mut state, mouse, area.into());
                    }
                    _ => {}
                }
                if !event::poll(Duration::ZERO)? {
                    break;
                }
            }
        }
    }

    let _ = state.save();
    Ok(())
}
