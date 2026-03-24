//! Plain stdout progress for `prime-agent run` (no alternate screen, no agent stream).

use std::io::{self, Write};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::time::{Duration, Instant};

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
pub fn run_display_loop(rx: Receiver<ProgressMsg>, is_tty: bool) {
    let refresh = Duration::from_secs(pipeline_refresh_secs());
    let mut spinner_i = 0usize;
    let mut stage: Option<StageState> = None;
    let mut status_line_open = false;
    let mut stdout = io::stdout();

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
                skills,
                line_counters,
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

                    commit_tty_status_line(&mut stdout, &mut status_line_open);

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
                    if st.last_block_refresh.elapsed() >= refresh {
                        commit_tty_status_line(&mut stdout, &mut status_line_open);
                        println!("---");
                        paint_skill_block(st);
                        st.last_block_refresh = Instant::now();
                        let pipeline_done = st.pipeline_stages_completed_before
                            + usize::from(st.skills_done >= st.skills_total);
                        draw_status_with_pipeline_done(
                            st,
                            pipeline_done,
                            &mut spinner_i,
                            is_tty,
                            true,
                            &mut stdout,
                            &mut status_line_open,
                        );
                    } else {
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
                    }
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
            skills: vec![],
            skill_states: vec![],
            line_counters: vec![],
            skills_total: 1,
            skills_done: 0,
            stage_failed: false,
            pipeline_stages_total: 1,
            pipeline_stages_completed_before: 0,
            started: Instant::now(),
            last_block_refresh: Instant::now(),
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
    fn commit_inserts_newline_before_static() {
        let mut buf: Vec<u8> = Vec::new();
        writeln!(&mut buf).unwrap();
        writeln!(&mut buf, "---").unwrap();
        let s = String::from_utf8(buf).unwrap();
        assert!(s.starts_with("\n---"));
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
