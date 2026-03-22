//! Interactive pipeline picker for `prime-agent pipelines` (ratatui + crossterm).

use anyhow::{bail, Context, Result};
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen};
use ratatui::prelude::*;
use ratatui::style::Stylize;
use ratatui::widgets::{Block, Borders, List, ListItem, ListState};
use std::io::stdout;

/// When `names` has one entry, returns it immediately. When multiple, shows a list (TTY).
/// `names` must be non-empty (caller checks).
pub fn pick_pipeline_interactive(names: &[String]) -> Result<String> {
    if names.is_empty() {
        bail!("pick_pipeline_interactive: empty names");
    }
    if names.len() == 1 {
        return Ok(names[0].clone());
    }

    enable_raw_mode().context("enable terminal raw mode")?;
    let mut stdout_h = stdout();
    execute!(
        stdout_h,
        EnterAlternateScreen,
        crossterm::cursor::Hide
    )
    .context("enter alternate screen")?;
    let backend = CrosstermBackend::new(stdout_h);
    let mut terminal = Terminal::new(backend).context("terminal")?;

    let mut state = ListState::default();
    state.select(Some(0));

    let result = run_picker_loop(&mut terminal, names, &mut state);

    let _ = disable_raw_mode();
    let mut restore = stdout();
    let _ = execute!(restore, LeaveAlternateScreen, crossterm::cursor::Show);

    result
}

fn run_picker_loop(
    terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
    names: &[String],
    state: &mut ListState,
) -> Result<String> {
    loop {
        terminal.draw(|f| {
            let block = Block::default()
                .borders(Borders::ALL)
                .title("Select pipeline (↑/↓, Enter, q/Esc to cancel)");
            let items: Vec<ListItem> = names
                .iter()
                .map(|n| ListItem::new(n.clone()))
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
                let next = (i + 1).min(names.len().saturating_sub(1));
                state.select(Some(next));
            }
            KeyCode::Up => {
                let i = state.selected().unwrap_or(0);
                let next = i.saturating_sub(1);
                state.select(Some(next));
            }
            KeyCode::Enter => {
                if let Some(i) = state.selected() {
                    return Ok(names[i].clone());
                }
            }
            _ => {}
        }
    }
}
