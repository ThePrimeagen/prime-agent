//! Interactive pipeline picker for default `prime-agent` (ratatui + crossterm).

use anyhow::{Context, Result, bail};
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::prelude::*;
use ratatui::style::Stylize;
use ratatui::widgets::{Block, Borders, List, ListItem, ListState};
use std::io::stdout;

/// Shows a list picker (TTY). `entries` must be non-empty (caller checks).
/// Each entry is `(pipeline_name, is_broken)`. Broken pipelines cannot be started from the picker.
pub fn pick_pipeline_interactive(entries: &[(String, bool)]) -> Result<String> {
    if entries.is_empty() {
        bail!("pick_pipeline_interactive: empty entries");
    }

    enable_raw_mode().context("enable terminal raw mode")?;
    let mut stdout_h = stdout();
    execute!(stdout_h, EnterAlternateScreen, crossterm::cursor::Hide)
        .context("enter alternate screen")?;
    let backend = CrosstermBackend::new(stdout_h);
    let mut terminal = Terminal::new(backend).context("terminal")?;

    let mut state = ListState::default();
    state.select(Some(0));

    let result = run_picker_loop(&mut terminal, entries, &mut state);

    let _ = disable_raw_mode();
    let mut restore = stdout();
    let _ = execute!(restore, LeaveAlternateScreen, crossterm::cursor::Show);

    result
}

fn run_picker_loop(
    terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
    entries: &[(String, bool)],
    state: &mut ListState,
) -> Result<String> {
    loop {
        terminal
            .draw(|f| {
                let block = Block::default()
                    .borders(Borders::ALL)
                    .title("Select pipeline (↑/↓, Enter, q/Esc to cancel)");
                let items: Vec<ListItem> = entries
                    .iter()
                    .map(|(name, broken)| {
                        let label = if *broken {
                            format!("{name} ! (broken skill)")
                        } else {
                            name.clone()
                        };
                        let style = if *broken {
                            Style::default().fg(Color::Red)
                        } else {
                            Style::default()
                        };
                        ListItem::new(label).style(style)
                    })
                    .collect();
                let list = List::new(items)
                    .block(block)
                    .highlight_style(Style::default().bold().fg(Color::Cyan));
                f.render_stateful_widget(list, f.area(), state);
            })
            .context("draw picker")?;

        let Event::Key(key) = event::read().context("read keyboard")? else {
            continue;
        };
        if key.kind != KeyEventKind::Press {
            continue;
        }
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => {
                bail!("cancelled");
            }
            KeyCode::Down => {
                let i = state.selected().unwrap_or(0);
                let next = (i + 1).min(entries.len().saturating_sub(1));
                state.select(Some(next));
            }
            KeyCode::Up => {
                let i = state.selected().unwrap_or(0);
                let next = i.saturating_sub(1);
                state.select(Some(next));
            }
            KeyCode::Enter => {
                if let Some(i) = state.selected() {
                    let (name, broken) = &entries[i];
                    if *broken {
                        eprintln!(
                            "pipeline '{name}' is broken: fix missing skill attachments in pipeline.json or restore skills."
                        );
                        continue;
                    }
                    return Ok(name.clone());
                }
            }
            _ => {}
        }
    }
}
