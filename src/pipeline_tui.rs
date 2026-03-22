//! Full-screen TUI for `pipelines run` (ratatui + crossterm).
#![allow(clippy::too_many_lines)] // Terminal loop + worker orchestration

use anyhow::{bail, Context, Result};
use crossterm::execute;
use crossterm::terminal::{EnterAlternateScreen, LeaveAlternateScreen};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph};
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::io::{self, stdout};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use crate::dot_prime_agent_config::DotPrimeAgentConfig;
use crate::pipeline_run::{
    build_prompt, generate_run_name, read_prior_stage_json, run_cursor_agent_streaming,
    stage_file_is_complete, write_json_atomic, MetaFile, StageFile, TaskSpec,
};
use crate::pipeline_store::{PipelineStepView, PipelineStore};
use crate::skills_store::SkillsStore;
use crate::stdout_tail::StdoutTail;

const SUPPORTED_CLIRUNNER: &str = "cursor-agent";

enum UiMsg {
    Line {
        stage_idx: usize,
        skill_idx: usize,
        line: String,
    },
}

pub fn run_tui(
    pipeline_name: &str,
    user_prompt: &str,
    data_dir: &Path,
    skills_store: &SkillsStore,
    dot_config: &DotPrimeAgentConfig,
    cwd: &Path,
    stdout_lines: u32,
) -> Result<()> {
    if dot_config.clirunner != SUPPORTED_CLIRUNNER {
        bail!(
            "unsupported clirunner '{}'; supported: {}",
            dot_config.clirunner,
            SUPPORTED_CLIRUNNER
        );
    }

    PipelineStore::validate_kebab_name(pipeline_name)?;
    let store = PipelineStore::new(data_dir);
    store.get_pipeline_meta(pipeline_name)?;
    let steps = store.list_steps(pipeline_name)?;
    if steps.is_empty() {
        bail!("pipeline '{pipeline_name}' has no steps");
    }

    let out_dir = cwd.join(".prime-agent").join(format!("pipeline-{pipeline_name}"));
    fs::create_dir_all(&out_dir)
        .with_context(|| format!("create '{}'", out_dir.display()))?;

    let meta_path = out_dir.join("meta.json");
    let run_name = if meta_path.exists() {
        let raw = fs::read_to_string(&meta_path).context("read meta.json")?;
        let v: Value = serde_json::from_str(&raw).context("parse meta.json")?;
        v.get("run_name")
            .and_then(|s| s.as_str())
            .map_or_else(|| generate_run_name(pipeline_name), str::to_string)
    } else {
        generate_run_name(pipeline_name)
    };

    let meta = MetaFile {
        run_name: run_name.clone(),
        pipeline: pipeline_name.to_string(),
        model: dot_config.model.clone(),
        clirunner: dot_config.clirunner.clone(),
    };
    write_json_atomic(&meta_path, &meta)?;

    let workspace = cwd
        .canonicalize()
        .unwrap_or_else(|_| cwd.to_path_buf());

    let stage_titles: Vec<String> = steps.iter().map(|s| s.title.clone()).collect();
    let skills_per_stage: Vec<Vec<String>> = steps
        .iter()
        .map(|s| {
            if s.skills.is_empty() {
                vec!["(no skill)".to_string()]
            } else {
                s.skills.iter().map(|sk| sk.name.clone()).collect()
            }
        })
        .collect();

    let out_dir_display = out_dir
        .canonicalize()
        .unwrap_or_else(|_| out_dir.clone());

    let (ui_tx, ui_rx) = mpsc::channel::<UiMsg>();

    let worker_data = WorkerCtx {
        steps,
        user_prompt: user_prompt.to_string(),
        skills_store: skills_store.root().to_path_buf(),
        out_dir: out_dir.clone(),
        workspace,
        model: dot_config.model.clone(),
        binary: dot_config.clirunner.clone(),
        ui_tx,
    };

    let worker = thread::spawn(move || worker_data.run());

    let mut stdout_h = stdout();
    execute!(stdout_h, EnterAlternateScreen, crossterm::cursor::Hide)?;
    let backend = CrosstermBackend::new(stdout_h);
    let mut terminal = Terminal::new(backend).context("terminal")?;

    let cap = usize::try_from(stdout_lines).unwrap_or(3).max(1);
    let mut tails: HashMap<(usize, usize), StdoutTail> = HashMap::new();

    let result = loop {
        while let Ok(msg) = ui_rx.try_recv() {
            match msg {
                UiMsg::Line {
                    stage_idx,
                    skill_idx,
                    line,
                } => {
                    tails
                        .entry((stage_idx, skill_idx))
                        .or_insert_with(|| StdoutTail::new(cap))
                        .push_line(line);
                }
            }
        }

        terminal.draw(|f| {
            let block = Block::default()
                .borders(Borders::ALL)
                .title("prime-agent pipeline");
            let inner = block.inner(f.area());
            f.render_widget(block, f.area());

            let header = format!(
                "Running Pipeline {} ({})\nRun name: {}",
                pipeline_name,
                out_dir_display.display(),
                run_name
            );
            let p = Paragraph::new(header).style(Style::default().fg(Color::Yellow));
            f.render_widget(p, inner);

            let mut y = 3_u16;
            for (si, title) in stage_titles.iter().enumerate() {
                let stage_style = Style::default().fg(Color::DarkGray);
                let t = Paragraph::new(format!("  • {title}")).style(stage_style);
                let area = Rect::new(inner.x + 1, inner.y + y, inner.width.saturating_sub(2), 1);
                f.render_widget(t, area);
                y = y.saturating_add(1);

                let skill_names = &skills_per_stage[si];
                for (ki, sk) in skill_names.iter().enumerate() {
                    let line = format!("      - {sk}");
                    let st = Style::default().fg(Color::DarkGray);
                    let pa = Paragraph::new(line).style(st);
                    let area = Rect::new(inner.x + 1, inner.y + y, inner.width.saturating_sub(2), 1);
                    f.render_widget(pa, area);
                    y = y.saturating_add(1);

                    if let Some(tail) = tails.get(&(si, ki)) {
                        for ln in tail.lines() {
                            let tl = Paragraph::new(format!("          {ln}"))
                                .style(Style::default().fg(Color::Gray));
                            let area = Rect::new(
                                inner.x + 1,
                                inner.y + y,
                                inner.width.saturating_sub(2),
                                1,
                            );
                            f.render_widget(tl, area);
                            y = y.saturating_add(1);
                        }
                    }
                }
            }
        })?;

        if worker.is_finished() {
            break worker
                .join()
                .map_err(|_| "pipeline worker panicked".to_string())
                .and_then(|r| r.map_err(|e| e.to_string()));
        }

        thread::sleep(Duration::from_millis(50));
    };

    let mut stdout_restore = io::stdout();
    execute!(
        stdout_restore,
        LeaveAlternateScreen,
        crossterm::cursor::Show
    )?;

    match result {
        Ok(()) => Ok(()),
        Err(e) => bail!("{e}"),
    }
}

struct WorkerCtx {
    steps: Vec<PipelineStepView>,
    user_prompt: String,
    skills_store: PathBuf,
    out_dir: PathBuf,
    workspace: PathBuf,
    model: String,
    binary: String,
    ui_tx: mpsc::Sender<UiMsg>,
}

impl WorkerCtx {
    fn run(self) -> Result<()> {
        let skills = SkillsStore::new(self.skills_store);
        for (idx, step) in self.steps.iter().enumerate() {
            let stage_num = u32::try_from(idx + 1).unwrap_or(u32::MAX);
            let stage_path = self.out_dir.join(format!("{stage_num}.json"));
            if stage_file_is_complete(&stage_path, step)? {
                continue;
            }

            let prior = read_prior_stage_json(&self.out_dir, stage_num.saturating_sub(1))?;
            let mut task_specs: Vec<TaskSpec> = Vec::new();

            if step.skills.is_empty() {
                let prompt = build_prompt(&self.user_prompt, None, step, &prior);
                task_specs.push(TaskSpec {
                    skill_label: String::new(),
                    prompt,
                });
            } else {
                let mut names: Vec<String> = step.skills.iter().map(|s| s.name.clone()).collect();
                names.sort();
                for skill_name in names {
                    let skill_body = skills
                        .load_skill(&skill_name)
                        .with_context(|| format!("load skill '{skill_name}'"))?;
                    let prompt = build_prompt(
                        &self.user_prompt,
                        Some((&skill_name, &skill_body)),
                        step,
                        &prior,
                    );
                    task_specs.push(TaskSpec {
                        skill_label: skill_name,
                        prompt,
                    });
                }
            }

            let results: Vec<(String, String, Result<String, String>)> = thread::scope(|scope| {
                let mut handles = Vec::new();
                for (skill_idx, spec) in task_specs.iter().enumerate() {
                    let prompt = spec.prompt.clone();
                    let model = self.model.clone();
                    let workspace = self.workspace.clone();
                    let binary = self.binary.clone();
                    let ui_tx = self.ui_tx.clone();
                    let stage_idx = idx;
                    handles.push(scope.spawn(move || {
                        let (tx_line, rx_line) = mpsc::channel::<String>();
                        let ui_tx2 = ui_tx.clone();
                        let _forward = thread::spawn(move || {
                            for line in rx_line {
                                let _ = ui_tx2.send(UiMsg::Line {
                                    stage_idx,
                                    skill_idx,
                                    line,
                                });
                            }
                        });
                        run_cursor_agent_streaming(
                            &binary,
                            &model,
                            &workspace,
                            &prompt,
                            Some(tx_line),
                        )
                    }));
                }
                handles
                    .into_iter()
                    .map(|h| {
                        h.join().unwrap_or_else(|_| {
                            (
                                String::new(),
                                String::new(),
                                Err("task panicked".to_string()),
                            )
                        })
                    })
                    .collect()
            });

            let mut name: Vec<String> = Vec::new();
            let mut input_prompt: Vec<String> = Vec::new();
            let mut output: Vec<String> = Vec::new();
            let mut stdout: Vec<String> = Vec::new();
            let mut stderr: Vec<String> = Vec::new();
            let mut stage_err: Option<String> = None;

            for (spec, res) in task_specs.iter().zip(results.iter()) {
                name.push(spec.skill_label.clone());
                input_prompt.push(spec.prompt.clone());
                let (out, err, parsed) = res;
                stdout.push(out.clone());
                stderr.push(err.clone());
                match parsed {
                    Ok(p) => output.push(p.clone()),
                    Err(e) => {
                        output.push(String::new());
                        stage_err = Some(match stage_err.take() {
                            None => e.clone(),
                            Some(prev) => format!("{prev} | {e}"),
                        });
                    }
                }
            }

            if let Some(ref e) = stage_err
                && e.is_empty()
            {
                stage_err = Some("one or more tasks failed".to_string());
            }

            let stage_file = StageFile {
                stage: stage_num,
                step_id: step.id,
                title: step.title.clone(),
                name,
                input_prompt,
                output,
                stdout,
                stderr,
                error: stage_err.clone(),
            };

            let path = self.out_dir.join(format!("{stage_num}.json"));
            write_json_atomic(&path, &stage_file)?;
            if stage_file.error.is_some() {
                bail!(
                    "pipeline stage {} failed: {}\nSee {}",
                    stage_num,
                    stage_file.error.as_deref().unwrap_or("unknown"),
                    path.display()
                );
            }
        }
        Ok(())
    }
}
