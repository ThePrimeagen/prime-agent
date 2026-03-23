//! Plain stdout progress for `pipelines run` (no alternate screen, no agent stream).

use std::io::{self, Write};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::time::{Duration, Instant};

use crossterm::cursor::{MoveToColumn, MoveUp};
use crossterm::execute;

const SPINNER_FRAMES: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

const YELLOW: &str = "\x1b[33m";
const GREEN: &str = "\x1b[32m";
const RED: &str = "\x1b[31m";
const RESET: &str = "\x1b[0m";

fn pipeline_refresh_secs() -> u64 {
    std::env::var("PRIME_AGENT_PIPELINE_REFRESH_SECS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(30)
        .max(1)
}

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
        skills: Vec<String>,
        line_counters: Vec<(Arc<AtomicUsize>, Arc<AtomicUsize>)>,
        pipeline_stages_total: usize,
        /// Stages fully completed before this stage runs.
        pipeline_stages_completed_before: usize,
    },
    /// Emitted when a skill task finishes (any order).
    SkillDone {
        stage_display: usize,
        skill_idx: usize,
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
    skills: Vec<String>,
    skill_states: Vec<SkillState>,
    line_counters: Vec<(Arc<AtomicUsize>, Arc<AtomicUsize>)>,
    skills_total: usize,
    skills_done: usize,
    stage_failed: bool,
    pipeline_stages_total: usize,
    pipeline_stages_completed_before: usize,
    started: Instant,
    last_block_refresh: Instant,
}

/// Run on a dedicated thread; receives progress messages until [`ProgressMsg::Shutdown`].
#[allow(clippy::too_many_lines)]
#[allow(clippy::needless_pass_by_value)] // `Receiver` is moved into this thread
pub fn run_display_loop(rx: Receiver<ProgressMsg>, is_tty: bool) {
    let refresh = Duration::from_secs(pipeline_refresh_secs());
    let mut spinner_i = 0usize;
    let mut stage: Option<StageState> = None;
    let mut last_status_len = 0usize;

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
                println!("pipeline {pipeline} {run_name}");
                let _ = io::stdout().flush();
            }
            Ok(ProgressMsg::StageStart {
                stage_display,
                title,
                skills,
                line_counters,
                pipeline_stages_total,
                pipeline_stages_completed_before,
            }) => {
                spinner_i = 0;
                let n = skills.len();
                let skill_states = vec![SkillState::Running; n];
                let skills_total = n.max(1);
                stage = Some(StageState {
                    stage_display,
                    title,
                    skills: skills.clone(),
                    skill_states,
                    line_counters,
                    skills_total,
                    skills_done: 0,
                    stage_failed: false,
                    pipeline_stages_total,
                    pipeline_stages_completed_before,
                    started: Instant::now(),
                    last_block_refresh: Instant::now(),
                });
                if let Some(st) = stage.as_mut() {
                    paint_skill_block(st);
                    draw_status(is_tty, &mut last_status_len, st, &mut spinner_i);
                }
                let _ = io::stdout().flush();
            }
            Ok(ProgressMsg::SkillDone {
                stage_display,
                skill_idx,
                skill_name,
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

                    clear_status_line(is_tty, last_status_len);
                    last_status_len = 0;

                    if is_tty {
                        let _ = repaint_skill_block_tty(st);
                    }
                    // Non-tty: the completion line below is enough (no duplicate skill block).

                    let outcome: &str = if ok {
                        "\u{001b}[32msucceeded\u{001b}[0m"
                    } else {
                        "\u{001b}[31mfailed\u{001b}[0m"
                    };
                    println!(
                        "step {stage_display} skill {skill_name} {outcome}, {} / {} completed",
                        st.skills_done, st.skills_total
                    );

                    let pipeline_done = st.pipeline_stages_completed_before
                        + usize::from(st.skills_done >= st.skills_total);

                    draw_status_with_pipeline_done(
                        is_tty,
                        &mut last_status_len,
                        st,
                        pipeline_done,
                        &mut spinner_i,
                    );

                    let d = st.skills_done >= st.skills_total;
                    let _ = io::stdout().flush();
                    d
                };
                if done {
                    stage = None;
                }
            }
            Ok(ProgressMsg::Shutdown) | Err(RecvTimeoutError::Disconnected) => {
                clear_status_line(is_tty, last_status_len);
                break;
            }
            Err(RecvTimeoutError::Timeout) => {
                if let Some(ref mut st) = stage {
                    if st.last_block_refresh.elapsed() >= refresh {
                        clear_status_line(is_tty, last_status_len);
                        last_status_len = 0;
                        if is_tty {
                            let _ = repaint_skill_block_tty(st);
                        } else {
                            println!("---");
                            paint_skill_block(st);
                        }
                        st.last_block_refresh = Instant::now();
                        let pipeline_done = st.pipeline_stages_completed_before
                            + usize::from(st.skills_done >= st.skills_total);
                        draw_status_with_pipeline_done(
                            is_tty,
                            &mut last_status_len,
                            st,
                            pipeline_done,
                            &mut spinner_i,
                        );
                    } else if is_tty {
                        draw_status(is_tty, &mut last_status_len, st, &mut spinner_i);
                    }
                }
            }
        }
    }
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

fn skill_display_line(st: &StageState, i: usize) -> String {
    let name = &st.skills[i];
    match st.skill_states[i] {
        SkillState::Running => {
            let o = st.line_counters[i].0.load(Ordering::Relaxed);
            let e = st.line_counters[i].1.load(Ordering::Relaxed);
            format!("{YELLOW}  * running {name} ({o}, {e}){RESET}")
        }
        SkillState::DoneOk => {
            format!("{GREEN}  * {name}{RESET}")
        }
        SkillState::DoneErr => {
            format!("{RED}  * {name}{RESET}")
        }
    }
}

fn paint_skill_block(st: &StageState) {
    println!("{}", step_title_line(st));
    for i in 0..st.skills.len() {
        println!("{}", skill_display_line(st, i));
    }
}

fn repaint_skill_block_tty(st: &StageState) -> io::Result<()> {
    let n = u16::try_from(1 + st.skills.len()).unwrap_or(u16::MAX);
    let mut out = io::stdout();
    execute!(out, MoveUp(n), MoveToColumn(0))?;
    print!("{}", step_title_line(st));
    out.write_all(b"\n")?;
    for i in 0..st.skills.len() {
        print!("{}", skill_display_line(st, i));
        out.write_all(b"\n")?;
    }
    Ok(())
}

fn clear_status_line(is_tty: bool, last_len: usize) {
    if is_tty && last_len > 0 {
        print!("\r\x1b[K");
        let _ = io::stdout().flush();
    }
}

fn draw_status(is_tty: bool, last_status_len: &mut usize, st: &StageState, spinner_i: &mut usize) {
    let pipeline_done =
        st.pipeline_stages_completed_before + usize::from(st.skills_done >= st.skills_total);
    draw_status_with_pipeline_done(is_tty, last_status_len, st, pipeline_done, spinner_i);
}

fn draw_status_with_pipeline_done(
    is_tty: bool,
    last_status_len: &mut usize,
    st: &StageState,
    pipeline_done: usize,
    spinner_i: &mut usize,
) {
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
    let line = format!("{step_part} {pipe_part} {frame} {secs}s");

    if is_tty {
        print!("\r\x1b[K{line}");
        *last_status_len = line.chars().count();
    } else {
        println!("{line}");
    }
    let _ = io::stdout().flush();
}
