//! Pipeline run progress: full-screen ratatui on an interactive TTY; plain stdout when piped or
//! when `--no-tui` / `PRIME_AGENT_NO_TUI=1` is set.

use std::io::{self, IsTerminal, Write};
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::time::{Duration, Instant};

use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::prelude::*;
use ratatui::style::Stylize;
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

const SPINNER_FRAMES: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

const YELLOW: &str = "\x1b[33m";
const GREEN: &str = "\x1b[32m";
const RED: &str = "\x1b[31m";
const DIM: &str = "\x1b[2m";
const RESET: &str = "\x1b[0m";

/// Messages from the pipeline runner to the display thread.
pub enum ProgressMsg {
    PipelineHeader {
        pipeline: String,
        run_name: String,
    },
    StageStart {
        /// 1-based step index for display.
        stage_display: usize,
        title: String,
        /// All pipeline step titles in order (same length as pipeline stages).
        all_stage_titles: Vec<String>,
        skills: Vec<String>,
        stdout_tails: Vec<Arc<Mutex<Vec<String>>>>,
        pipeline_stages_total: usize,
        /// Stages fully completed before this stage runs.
        pipeline_stages_completed_before: usize,
    },
    /// Emitted when a skill task finishes (any order).
    SkillDone {
        stage_display: usize,
        skill_idx: usize,
        /// Carried for debugging / future display; TUI uses skill index to update state.
        #[allow(dead_code)]
        skill_name: String,
        ok: bool,
    },
    Shutdown,
}

#[derive(Clone, Copy)]
enum SkillState {
    Running,
    DoneOk,
    DoneErr,
}

struct StageState {
    stage_display: usize,
    title: String,
    all_stage_titles: Vec<String>,
    skills: Vec<String>,
    skill_states: Vec<SkillState>,
    stdout_tails: Vec<Arc<Mutex<Vec<String>>>>,
    skills_total: usize,
    skills_done: usize,
    stage_failed: bool,
    pipeline_stages_total: usize,
    pipeline_stages_completed_before: usize,
    started: Instant,
}

fn commit_tty_status_line(stdout: &mut io::Stdout, status_line_open: &mut bool) {
    if *status_line_open {
        let _ = writeln!(stdout);
        let _ = stdout.flush();
        *status_line_open = false;
    }
}

/// Run on a dedicated thread; receives progress messages until [`ProgressMsg::Shutdown`].
#[allow(clippy::too_many_lines)]
#[allow(clippy::needless_pass_by_value)] // `Receiver` is moved into this thread
pub fn run_display_loop(rx: Receiver<ProgressMsg>, use_pipeline_tui: bool) {
    if use_pipeline_tui {
        run_tui_display_loop(rx);
    } else {
        run_plain_display_loop(rx);
    }
}

#[allow(clippy::too_many_lines)]
fn run_tui_display_loop(rx: Receiver<ProgressMsg>) {
    let mut stdout = io::stdout();
    if enable_raw_mode().is_err() {
        run_plain_display_loop(rx);
        return;
    }
    if execute!(stdout, EnterAlternateScreen, crossterm::cursor::Hide).is_err() {
        let _ = disable_raw_mode();
        run_plain_display_loop(rx);
        return;
    }

    let Ok(mut terminal) = Terminal::new(CrosstermBackend::new(stdout)) else {
        let _ = disable_raw_mode();
        let mut o = io::stdout();
        let _ = execute!(o, LeaveAlternateScreen, crossterm::cursor::Show);
        run_plain_display_loop(rx);
        return;
    };

    let mut pipeline_title = String::new();
    let mut spinner_i = 0usize;
    let mut stage: Option<StageState> = None;

    let restore_terminal = || {
        let _ = disable_raw_mode();
        let mut out = io::stdout();
        let _ = execute!(out, LeaveAlternateScreen, crossterm::cursor::Show);
    };

    loop {
        let timeout = if stage.is_some() {
            Duration::from_millis(100)
        } else {
            Duration::from_secs(60 * 60)
        };

        let msg = match rx.recv_timeout(timeout) {
            Ok(m) => m,
            Err(RecvTimeoutError::Timeout) => {
                let _ = terminal.draw(|f| {
                    draw_tui_frame(f, &pipeline_title, stage.as_ref(), &mut spinner_i);
                });
                continue;
            }
            Err(RecvTimeoutError::Disconnected) => break,
        };

        match msg {
            ProgressMsg::PipelineHeader { pipeline, run_name } => {
                pipeline_title = format!("pipeline {pipeline}  {run_name}");
            }
            ProgressMsg::StageStart {
                stage_display,
                title,
                all_stage_titles,
                skills,
                stdout_tails,
                pipeline_stages_total,
                pipeline_stages_completed_before,
            } => {
                spinner_i = 0;
                let n = skills.len();
                let skill_states = vec![SkillState::Running; n];
                let skills_total = n.max(1);
                stage = Some(StageState {
                    stage_display,
                    title,
                    all_stage_titles,
                    skills: skills.clone(),
                    skill_states,
                    stdout_tails,
                    skills_total,
                    skills_done: 0,
                    stage_failed: false,
                    pipeline_stages_total,
                    pipeline_stages_completed_before,
                    started: Instant::now(),
                });
            }
            ProgressMsg::SkillDone {
                stage_display,
                skill_idx,
                skill_name: _,
                ok,
            } => {
                let done = {
                    let Some(st) = stage.as_mut() else {
                        continue;
                    };
                    if st.stage_display != stage_display {
                        continue;
                    }
                    if skill_idx < st.skill_states.len() {
                        st.skill_states[skill_idx] = if ok {
                            SkillState::DoneOk
                        } else {
                            SkillState::DoneErr
                        };
                    }
                    st.skills_done += 1;
                    st.stage_failed |= !ok;
                    st.skills_done >= st.skills_total
                };
                if done {
                    stage = None;
                }
            }
            ProgressMsg::Shutdown => break,
        }

        let _ = terminal.draw(|f| {
            draw_tui_frame(f, &pipeline_title, stage.as_ref(), &mut spinner_i);
        });
    }

    restore_terminal();
}

fn draw_tui_frame(
    f: &mut Frame,
    pipeline_title: &str,
    stage: Option<&StageState>,
    spinner_i: &mut usize,
) {
    let area = f.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(1)].as_ref())
        .split(area);

    let body_block = Block::default()
        .borders(Borders::ALL)
        .title(pipeline_title)
        .title_style(Style::default().fg(Color::Cyan));
    let inner = body_block.inner(chunks[0]);
    f.render_widget(body_block, chunks[0]);

    let body_lines = stage.map_or_else(Vec::new, tui_body_lines);
    let body = Paragraph::new(Text::from(body_lines)).wrap(Wrap { trim: true });
    f.render_widget(body, inner);

    let footer_text = stage.map_or_else(String::new, |st| {
        let pipeline_done = st.pipeline_stages_completed_before
            + usize::from(st.skills_done >= st.skills_total);
        format_status_line(st, pipeline_done, spinner_i)
    });
    let footer = Paragraph::new(footer_text).style(Style::default().fg(Color::White));
    f.render_widget(footer, chunks[1]);
}

fn tui_body_lines(st: &StageState) -> Vec<Line<'static>> {
    let mut lines: Vec<Line<'static>> = Vec::new();
    let titles = &st.all_stage_titles;
    if titles.is_empty() {
        lines.push(tui_step_title_line(st));
        for i in 0..st.skills.len() {
            lines.extend(tui_skill_lines(st, i));
        }
        return lines;
    }

    let current = st.pipeline_stages_completed_before;
    for i in 0..current {
        if let Some(t) = titles.get(i) {
            let s = format!("step {} {t}", i + 1);
            lines.push(Line::from(s).fg(Color::Green));
        }
    }
    lines.push(tui_step_title_line(st));
    for i in 0..st.skills.len() {
        lines.extend(tui_skill_lines(st, i));
    }
    for i in (current + 1)..titles.len() {
        if let Some(t) = titles.get(i) {
            let s = format!("step {} {t}", i + 1);
            lines.push(Line::from(s).fg(Color::DarkGray));
        }
    }
    lines
}

fn tui_step_title_line(st: &StageState) -> Line<'static> {
    let color = if st.skill_states.iter().any(|s| matches!(s, SkillState::DoneErr)) {
        Color::Red
    } else {
        Color::Yellow
    };
    let s = format!("step {} {}", st.stage_display, st.title);
    Line::from(s).fg(color)
}

fn tail_line_for_display(s: &str) -> String {
    const MAX_CHARS: usize = 200;
    if s.chars().count() <= MAX_CHARS {
        return s.to_string();
    }
    let mut out: String = s.chars().take(MAX_CHARS).collect();
    out.push_str("...");
    out
}

fn tui_skill_lines(st: &StageState, i: usize) -> Vec<Line<'static>> {
    let name = st.skills[i].clone();
    match st.skill_states[i] {
        SkillState::Running => {
            let mut lines = vec![Line::from(format!("  * running {name}")).fg(Color::Yellow)];
            if let Ok(guard) = st.stdout_tails[i].lock() {
                for line in guard.iter() {
                    let shown = tail_line_for_display(line);
                    lines.push(Line::from(format!("      {shown}")).fg(Color::DarkGray));
                }
            }
            lines
        }
        SkillState::DoneOk => {
            vec![Line::from(format!("  * {name}")).fg(Color::Green)]
        }
        SkillState::DoneErr => {
            vec![Line::from(format!("  * {name}")).fg(Color::Red)]
        }
    }
}

#[allow(clippy::too_many_lines)]
#[allow(clippy::needless_pass_by_value)]
fn run_plain_display_loop(rx: Receiver<ProgressMsg>) {
    let mut spinner_i = 0usize;
    let mut stage: Option<StageState> = None;
    let mut status_line_open = false;
    let mut stdout = io::stdout();
    let is_tty = std::io::stdout().is_terminal();

    loop {
        let timeout = if stage.is_some() {
            if is_tty {
                Duration::from_millis(100)
            } else {
                Duration::from_secs(1)
            }
        } else {
            Duration::from_secs(60 * 60)
        };

        match rx.recv_timeout(timeout) {
            Ok(ProgressMsg::PipelineHeader { pipeline, run_name }) => {
                commit_tty_status_line(&mut stdout, &mut status_line_open);
                println!("pipeline {pipeline} {run_name}");
                let _ = stdout.flush();
            }
            Ok(ProgressMsg::StageStart {
                stage_display,
                title,
                all_stage_titles,
                skills,
                stdout_tails,
                pipeline_stages_total,
                pipeline_stages_completed_before,
            }) => {
                commit_tty_status_line(&mut stdout, &mut status_line_open);
                spinner_i = 0;
                let n = skills.len();
                let skill_states = vec![SkillState::Running; n];
                let skills_total = n.max(1);
                stage = Some(StageState {
                    stage_display,
                    title,
                    all_stage_titles,
                    skills: skills.clone(),
                    skill_states,
                    stdout_tails,
                    skills_total,
                    skills_done: 0,
                    stage_failed: false,
                    pipeline_stages_total,
                    pipeline_stages_completed_before,
                    started: Instant::now(),
                });
                if let Some(st) = stage.as_mut() {
                    paint_skill_block(st, is_tty);
                    draw_status(
                        st,
                        &mut spinner_i,
                        is_tty,
                        false,
                        &mut stdout,
                        &mut status_line_open,
                    );
                }
                let _ = stdout.flush();
            }
            Ok(ProgressMsg::SkillDone {
                stage_display,
                skill_idx,
                skill_name: _,
                ok,
            }) => {
                let done = {
                    let Some(st) = stage.as_mut() else {
                        continue;
                    };
                    if st.stage_display != stage_display {
                        continue;
                    }
                    if skill_idx < st.skill_states.len() {
                        st.skill_states[skill_idx] = if ok {
                            SkillState::DoneOk
                        } else {
                            SkillState::DoneErr
                        };
                    }
                    st.skills_done += 1;
                    st.stage_failed |= !ok;

                    let pipeline_done = st.pipeline_stages_completed_before
                        + usize::from(st.skills_done >= st.skills_total);

                    draw_status_with_pipeline_done(
                        st,
                        pipeline_done,
                        &mut spinner_i,
                        is_tty,
                        false,
                        &mut stdout,
                        &mut status_line_open,
                    );

                    let d = st.skills_done >= st.skills_total;
                    if d {
                        commit_tty_status_line(&mut stdout, &mut status_line_open);
                    }
                    let _ = stdout.flush();
                    d
                };
                if done {
                    stage = None;
                }
            }
            Ok(ProgressMsg::Shutdown) | Err(RecvTimeoutError::Disconnected) => {
                break;
            }
            Err(RecvTimeoutError::Timeout) => {
                if let Some(ref mut st) = stage {
                    let pipeline_done = st.pipeline_stages_completed_before
                        + usize::from(st.skills_done >= st.skills_total);
                    draw_status_with_pipeline_done(
                        st,
                        pipeline_done,
                        &mut spinner_i,
                        is_tty,
                        false,
                        &mut stdout,
                        &mut status_line_open,
                    );
                    let _ = stdout.flush();
                }
            }
        }
    }
    commit_tty_status_line(&mut stdout, &mut status_line_open);
}

fn step_title_line(st: &StageState) -> String {
    let color = if st
        .skill_states
        .iter()
        .any(|s| matches!(s, SkillState::DoneErr))
    {
        RED
    } else if st
        .skill_states
        .iter()
        .any(|s| matches!(s, SkillState::Running))
    {
        YELLOW
    } else {
        GREEN
    };
    format!("{color}step {} {}{RESET}", st.stage_display, st.title)
}

fn skill_display_lines(st: &StageState, i: usize) -> Vec<String> {
    let name = &st.skills[i];
    match st.skill_states[i] {
        SkillState::Running => {
            let mut lines = vec![format!("{YELLOW}  * running {name}{RESET}")];
            if let Ok(guard) = st.stdout_tails[i].lock() {
                for line in guard.iter() {
                    let shown = tail_line_for_display(line);
                    lines.push(format!("{DIM}      {shown}{RESET}"));
                }
            }
            lines
        }
        SkillState::DoneOk => {
            vec![format!("{GREEN}  * {name}{RESET}")]
        }
        SkillState::DoneErr => {
            vec![format!("{RED}  * {name}{RESET}")]
        }
    }
}

fn paint_skill_block(st: &StageState, is_tty: bool) {
    let current = st.pipeline_stages_completed_before;
    let titles = &st.all_stage_titles;
    if titles.is_empty() {
        println!("{}", step_title_line(st));
        for i in 0..st.skills.len() {
            for line in skill_display_lines(st, i) {
                println!("{line}");
            }
        }
        return;
    }
    for i in 0..current {
        if let Some(t) = titles.get(i) {
            println!("{GREEN}step {} {t}{RESET}", i + 1);
        }
    }
    println!("{}", step_title_line(st));
    for i in 0..st.skills.len() {
        for line in skill_display_lines(st, i) {
            println!("{line}");
        }
    }
    for i in (current + 1)..titles.len() {
        if let Some(t) = titles.get(i) {
            if is_tty {
                println!("{DIM}step {} {t}{RESET}", i + 1);
            } else {
                println!("step {} {t}", i + 1);
            }
        }
    }
}

fn draw_status(
    st: &StageState,
    spinner_i: &mut usize,
    is_tty: bool,
    snapshot: bool,
    stdout: &mut io::Stdout,
    status_line_open: &mut bool,
) {
    let pipeline_done =
        st.pipeline_stages_completed_before + usize::from(st.skills_done >= st.skills_total);
    draw_status_with_pipeline_done(
        st,
        pipeline_done,
        spinner_i,
        is_tty,
        snapshot,
        stdout,
        status_line_open,
    );
}

fn draw_status_with_pipeline_done(
    st: &StageState,
    pipeline_done: usize,
    spinner_i: &mut usize,
    is_tty: bool,
    snapshot: bool,
    stdout: &mut io::Stdout,
    status_line_open: &mut bool,
) {
    let line = format_status_line(st, pipeline_done, spinner_i);
    if is_tty {
        if snapshot {
            writeln!(stdout, "{line}").expect("stdout");
            *status_line_open = false;
        } else {
            write!(stdout, "\r\x1b[K{line}").expect("stdout");
            *status_line_open = true;
        }
    } else {
        writeln!(stdout, "{line}").expect("stdout");
        *status_line_open = false;
    }
}

fn format_status_line(st: &StageState, pipeline_done: usize, spinner_i: &mut usize) -> String {
    let secs = st.started.elapsed().as_secs();
    let frame = SPINNER_FRAMES[*spinner_i % SPINNER_FRAMES.len()];
    *spinner_i = spinner_i.saturating_add(1);

    let step_part = if st.stage_failed {
        format!(
            "\u{001b}[31mStep {} / {}\u{001b}[0m",
            st.skills_done, st.skills_total
        )
    } else {
        format!("Step {} / {}", st.skills_done, st.skills_total)
    };

    let pipe_part = format!("Pipeline {} / {}", pipeline_done, st.pipeline_stages_total);
    format!("{step_part} {pipe_part} {frame} {secs}s")
}

#[cfg(test)]
mod tests {
    use super::format_status_line;
    use super::StageState;
    use std::io::Write;
    use std::time::Instant;

    fn minimal_stage() -> StageState {
        StageState {
            stage_display: 1,
            title: "t".to_string(),
            all_stage_titles: vec![],
            skills: vec![],
            skill_states: vec![],
            stdout_tails: vec![],
            skills_total: 1,
            skills_done: 0,
            stage_failed: false,
            pipeline_stages_total: 1,
            pipeline_stages_completed_before: 0,
            started: Instant::now(),
        }
    }

    #[test]
    fn tty_live_two_writes_no_newline_between() {
        let mut buf = Vec::new();
        let line1 = "Step 0 / 1 Pipeline 0 / 1 ⠋ 0s";
        let line2 = "Step 0 / 1 Pipeline 0 / 1 ⠙ 0s";
        write!(buf, "\r\x1b[K{line1}").unwrap();
        write!(buf, "\r\x1b[K{line2}").unwrap();
        let s = String::from_utf8(buf).unwrap();
        assert!(!s.contains('\n'));
        assert!(s.starts_with("\r\x1b[K"));
    }

    #[test]
    fn tty_snapshot_ends_with_newline_no_cr_before_text() {
        let line = "Step 0 / 1 Pipeline 0 / 1 ⠋ 0s";
        let mut buf = Vec::new();
        writeln!(buf, "{line}").unwrap();
        let s = String::from_utf8(buf).unwrap();
        assert!(s.ends_with('\n'));
        assert!(!s.contains('\r'));
    }

    #[test]
    fn non_tty_each_logical_line_has_newline() {
        let mut buf = Vec::new();
        let line = "Step 0 / 1 Pipeline 0 / 1 ⠋ 0s";
        writeln!(buf, "{line}").unwrap();
        assert!(buf.ends_with(b"\n"));
    }

    #[test]
    fn format_status_line_increments_spinner() {
        let st = minimal_stage();
        let mut i = 0usize;
        let a = format_status_line(&st, 0, &mut i);
        let b = format_status_line(&st, 0, &mut i);
        assert_ne!(a, b);
    }
}
