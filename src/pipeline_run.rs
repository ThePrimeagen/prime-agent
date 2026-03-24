//! Run pipeline stages via `cursor-agent`, writing task JSON under
//! `cwd/.prime-agent/pipelines/<adj-noun-slug>/` (never `pipelines/<pipeline-name>/`).

use anyhow::{Context, Result, bail};
use serde::Serialize;
use serde_json::Value;
use std::fmt::Write as FmtWrite;
use std::fs;
use std::io::{BufRead, BufReader, IsTerminal, Read, Write as IoWrite};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::Once;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use crate::dot_prime_agent_config::DotPrimeAgentConfig;
use crate::pipeline_progress::ProgressMsg;
use crate::pipeline_store::{PipelineStepView, PipelineStore};
use crate::skills_store::SkillsStore;
use rand::Rng;

const SUPPORTED_CLIRUNNER: &str = "cursor-agent";

/// Serialize debug echo lines for concurrent `cursor-agent` tasks.
static DEBUG_STREAM_STDERR_LOCK: Mutex<()> = Mutex::new(());

static INSTALL_CTRLC: Once = Once::new();

fn install_ctrlc_handler() {
    INSTALL_CTRLC.call_once(|| {
        let _ = ctrlc::set_handler(|| {
            eprintln!();
            let _ = IoWrite::write_all(&mut std::io::stdout(), b"\x1b[0m");
            let _ = crossterm::execute!(std::io::stdout(), crossterm::cursor::Show);
            std::process::exit(130);
        });
    });
}

/// Options for `pipeline_run::run`.
#[derive(Debug, Clone, Copy)]
pub struct PipelineRunOptions {
    pub debug: bool,
    /// When true, skip ratatui pipeline progress (plain stdout only).
    pub no_tui: bool,
}

/// When `--debug` is set, each subprocess stdout/stderr line is echoed to stderr with this context.
#[derive(Clone)]
pub(crate) struct DebugStreamCtx {
    pub(crate) step_title: String,
    /// 1-based index of this task within the pipeline step.
    pub(crate) task_pos: usize,
    pub(crate) task_total: usize,
}

fn debug_stream_line(ctx: &DebugStreamCtx, stream: &str, line: &str) {
    let _lock = DEBUG_STREAM_STDERR_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    eprintln!(
        "{}({} / {}):{}:: {}",
        ctx.step_title, ctx.task_pos, ctx.task_total, stream, line
    );
}

#[derive(Serialize)]
pub(crate) struct MetaFile {
    pub(crate) run_name: String,
    pub(crate) pipeline: String,
    pub(crate) model: String,
    pub(crate) clirunner: String,
}

/// One `cursor-agent` invocation (written to `{stage}_{task}.json`).
#[derive(Serialize)]
pub(crate) struct TaskRunFile {
    pub(crate) command: String,
    pub(crate) user_prompt: String,
    pub(crate) skill_prompt: String,
    pub(crate) pipeline_prompt: String,
    pub(crate) prompt: String,
    pub(crate) stdout: String,
    pub(crate) stderr: String,
    /// Process exit code; `-1` if the process did not exit normally (e.g. spawn error).
    pub(crate) code: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) error: Option<String>,
    /// Parsed agent output text when `code == 0`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) output: Option<String>,
}

/// Lowercase kebab slug for run artifacts (e.g. `"quiet-harbor"` → `"quiet-harbor"`; spaced words → hyphenated).
pub(crate) fn run_name_filesystem_slug(run_name: &str) -> String {
    run_name
        .to_lowercase()
        .split_whitespace()
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

fn resolve_pipeline_run_dir(cwd: &Path) -> Result<(PathBuf, String)> {
    let pipelines_root = cwd.join(".prime-agent").join("pipelines");
    fs::create_dir_all(&pipelines_root)
        .with_context(|| format!("create '{}'", pipelines_root.display()))?;

    let (preferred_dir, candidate_run_name) = allocate_new_run_dir(&pipelines_root)?;

    fs::create_dir_all(&preferred_dir)
        .with_context(|| format!("create '{}'", preferred_dir.display()))?;
    Ok((preferred_dir, candidate_run_name))
}

/// Remove all pipeline run artifacts under `cwd/.prime-agent/pipelines/`.
pub fn clear_pipeline_runs(cwd: &Path) -> Result<()> {
    let dir = cwd.join(".prime-agent").join("pipelines");
    if dir.is_dir() {
        fs::remove_dir_all(&dir).with_context(|| format!("remove '{}'", dir.display()))?;
    }
    Ok(())
}

/// Pick a fresh adj-noun run name whose directory has no `meta.json`, rolling until one is free.
fn allocate_new_run_dir(pipelines_root: &Path) -> Result<(PathBuf, String)> {
    let mut plain_tries: u32 = 0;
    loop {
        let name = if plain_tries < 100_000 {
            plain_tries += 1;
            generate_run_name()
        } else {
            format!("{}-{:04x}", generate_run_name(), rand::random::<u16>())
        };
        let slug = run_name_filesystem_slug(&name);
        let dir = pipelines_root.join(&slug);
        if !dir.join("meta.json").is_file() {
            return Ok((dir, name));
        }
    }
}

pub fn debug_log(debug: bool, msg: impl std::fmt::Display) {
    if debug {
        eprintln!("prime-agent(debug): {msg}");
    }
}

pub fn run(
    pipeline_name: &str,
    user_prompt: &str,
    data_dir: &Path,
    skills_store: &SkillsStore,
    dot_config: &DotPrimeAgentConfig,
    cwd: &Path,
    options: PipelineRunOptions,
) -> Result<()> {
    run_plain(
        pipeline_name,
        user_prompt,
        data_dir,
        skills_store,
        dot_config,
        cwd,
        options,
    )
}

fn run_plain(
    pipeline_name: &str,
    user_prompt: &str,
    data_dir: &Path,
    skills_store: &SkillsStore,
    dot_config: &DotPrimeAgentConfig,
    cwd: &Path,
    options: PipelineRunOptions,
) -> Result<()> {
    let debug = options.debug;
    install_ctrlc_handler();

    if dot_config.clirunner != SUPPORTED_CLIRUNNER {
        bail!(
            "unsupported clirunner '{}'; supported: {}",
            dot_config.clirunner,
            SUPPORTED_CLIRUNNER
        );
    }

    debug_log(
        debug,
        format!(
            "skills_dir={} cwd={}",
            skills_store.root().display(),
            cwd.display()
        ),
    );

    PipelineStore::validate_kebab_name(pipeline_name)?;
    let store = PipelineStore::new(data_dir);
    store.get_pipeline_meta(pipeline_name)?;
    let steps = store.list_steps(skills_store, pipeline_name)?;
    if steps.is_empty() {
        bail!("pipeline '{pipeline_name}' has no steps");
    }

    validate_attached_skills(pipeline_name, &steps)?;

    let (out_dir, run_name) = resolve_pipeline_run_dir(cwd)?;
    let meta_path = out_dir.join("meta.json");

    let meta = MetaFile {
        run_name: run_name.clone(),
        pipeline: pipeline_name.to_string(),
        model: dot_config.model.clone(),
        clirunner: dot_config.clirunner.clone(),
    };
    write_json_atomic(&meta_path, &meta)?;

    let workspace = cwd.canonicalize().unwrap_or_else(|_| cwd.to_path_buf());

    let use_pipeline_tui = std::io::stdout().is_terminal()
        && !options.no_tui
        && std::env::var("PRIME_AGENT_NO_TUI").ok().as_deref() != Some("1");
    let (tx, rx) = mpsc::channel::<ProgressMsg>();
    let display = thread::spawn(move || {
        crate::pipeline_progress::run_display_loop(rx, use_pipeline_tui);
    });

    let tx_result = (|| -> Result<()> {
        tx.send(ProgressMsg::PipelineHeader {
            pipeline: pipeline_name.to_string(),
            run_name: run_name.clone(),
        })
        .map_err(|_| anyhow::anyhow!("pipeline progress channel closed"))?;

        let total_stages = steps.len();
        let all_stage_titles: Vec<String> = steps.iter().map(|s| s.title.clone()).collect();
        for (stage_idx, step) in steps.iter().enumerate() {
            let stage_num = u32::try_from(stage_idx + 1).unwrap_or(u32::MAX);
            if stage_tasks_complete(&out_dir, stage_num, step, user_prompt)? {
                debug_log(
                    debug,
                    format!("stage {stage_num} already complete; skipping"),
                );
                continue;
            }
            let ctx = RunStageCtx {
                stage_num,
                step,
                user_prompt,
                skills_store,
                out_dir: &out_dir,
                prev_stages: stage_num.saturating_sub(1),
                workspace: &workspace,
                dot_config,
                debug,
            };
            run_stage(&ctx, &tx, total_stages, stage_idx, &all_stage_titles)?;
        }
        Ok(())
    })();

    let _ = tx.send(ProgressMsg::Shutdown);
    drop(tx);
    let _ = display.join();
    tx_result
}

/// Fail fast if any referenced skill UUID does not resolve on disk (`cursor-agent` is never started).
pub fn validate_attached_skills(pipeline_name: &str, steps: &[PipelineStepView]) -> Result<()> {
    for step in steps {
        for sk in &step.skills {
            if sk.resolved_name.is_some() {
                continue;
            }
            bail!(
                "pipeline '{pipeline_name}' is broken: step id {} ({}) references missing skill id {} (last known alias '{}'). \
                 cursor-agent was not started.",
                step.id,
                step.title,
                sk.id,
                sk.alias
            );
        }
    }
    Ok(())
}

/// Stage is only "complete" for resume if task JSON matches the **current** user prompt (`--prompt`
/// / `--file` body). Otherwise a prior run under the same run directory would skip every stage and
/// exit after printing only the pipeline header.
pub(crate) fn stage_tasks_complete(
    out_dir: &Path,
    stage_num: u32,
    step: &PipelineStepView,
    user_prompt: &str,
) -> Result<bool> {
    let n = expected_task_count(step);
    for task in 1..=n {
        let p = task_json_path(out_dir, stage_num, task);
        if !p.exists() {
            return Ok(false);
        }
        let raw = fs::read_to_string(&p).with_context(|| format!("read '{}'", p.display()))?;
        let v: Value = serde_json::from_str(&raw).context("parse task json")?;
        let stored_prompt = v.get("user_prompt").and_then(Value::as_str).unwrap_or("");
        if stored_prompt != user_prompt {
            return Ok(false);
        }
        let code = v.get("code").and_then(Value::as_i64).unwrap_or(-1);
        if code != 0 {
            return Ok(false);
        }
        if v.get("error")
            .and_then(|e| e.as_str())
            .filter(|s| !s.is_empty())
            .is_some()
        {
            return Ok(false);
        }
    }
    Ok(true)
}

pub(crate) fn expected_task_count(step: &PipelineStepView) -> usize {
    if step.skills.is_empty() {
        1
    } else {
        step.skills.len()
    }
}

pub(crate) fn task_json_path(out_dir: &Path, stage: u32, task: usize) -> PathBuf {
    out_dir.join(format!("{stage}_{task}.json"))
}

struct RunStageCtx<'a> {
    stage_num: u32,
    step: &'a PipelineStepView,
    user_prompt: &'a str,
    skills_store: &'a SkillsStore,
    out_dir: &'a Path,
    prev_stages: u32,
    workspace: &'a Path,
    dot_config: &'a DotPrimeAgentConfig,
    debug: bool,
}

#[allow(clippy::too_many_lines)]
fn run_stage(
    ctx: &RunStageCtx<'_>,
    tx: &mpsc::Sender<ProgressMsg>,
    pipeline_stages_total: usize,
    stage_idx: usize,
    all_stage_titles: &[String],
) -> Result<()> {
    let prior = read_prior_stage_json(ctx.out_dir, ctx.prev_stages)?;
    let specs = build_stage_task_builds(
        ctx.user_prompt,
        ctx.step,
        &prior,
        ctx.skills_store,
        ctx.debug,
    )?;

    let display_names: Vec<String> = specs
        .iter()
        .map(|s| {
            if s.skill_label.is_empty() {
                "(no skill)".to_string()
            } else {
                s.skill_label.clone()
            }
        })
        .collect();

    let stdout_tails: Vec<Arc<Mutex<Vec<String>>>> = (0..specs.len())
        .map(|_| Arc::new(Mutex::new(Vec::new())))
        .collect();
    let tail_max = usize::try_from(ctx.dot_config.stdout_lines).unwrap_or(3).max(1);

    let stage_display = usize::try_from(ctx.stage_num).unwrap_or(usize::MAX);
    tx.send(ProgressMsg::StageStart {
        stage_display,
        title: ctx.step.title.clone(),
        all_stage_titles: all_stage_titles.to_vec(),
        skills: display_names.clone(),
        stdout_tails: stdout_tails.clone(),
        pipeline_stages_total,
        pipeline_stages_completed_before: stage_idx,
    })
    .map_err(|_| anyhow::anyhow!("pipeline progress channel closed"))?;

    let command_line = format_cursor_agent_invocation(ctx.dot_config, ctx.workspace);

    let results: Vec<(String, String, i32, Result<String, String>)> = thread::scope(|scope| {
        let mut handles = Vec::new();
        for (ti, spec) in specs.iter().enumerate() {
            let prompt = spec.parts.combined.clone();
            let model = ctx.dot_config.model.clone();
            let workspace = ctx.workspace.to_path_buf();
            let binary = ctx.dot_config.clirunner.clone();
            let yolo = ctx.dot_config.yolo;
            let dbg = ctx.debug;
            let progress_tx = tx.clone();
            let skill_name = display_names[ti].clone();
            let sd = stage_display;
            let ti_idx = ti;
            let tail_buf = stdout_tails[ti].clone();
            let debug_stream = if dbg {
                Some(DebugStreamCtx {
                    step_title: ctx.step.title.clone(),
                    task_pos: ti + 1,
                    task_total: specs.len(),
                })
            } else {
                None
            };
            handles.push(scope.spawn(move || {
                debug_log(dbg, "spawning cursor-agent");
                let res = run_cursor_agent_streaming(
                    &binary,
                    &model,
                    &workspace,
                    &prompt,
                    None,
                    Some((tail_buf, tail_max)),
                    yolo,
                    debug_stream,
                );
                let ok = res.3.is_ok();
                let _ = progress_tx.send(ProgressMsg::SkillDone {
                    stage_display: sd,
                    skill_idx: ti_idx,
                    skill_name,
                    ok,
                });
                res
            }));
        }
        handles
            .into_iter()
            .map(|h| {
                h.join().unwrap_or_else(|_| {
                    (
                        String::new(),
                        String::new(),
                        -1,
                        Err("cursor-agent task panicked".to_string()),
                    )
                })
            })
            .collect()
    });

    let mut stage_err: Option<String> = None;

    for (ti, (spec, res)) in specs.iter().zip(results.iter()).enumerate() {
        let task_num = ti + 1;
        let path = task_json_path(ctx.out_dir, ctx.stage_num, task_num);
        let (stdout, stderr, code, parsed) = res;
        let (output, err_opt) = match parsed {
            Ok(p) => (Some(p.clone()), None),
            Err(e) => {
                stage_err = Some(match stage_err.take() {
                    None => e.clone(),
                    Some(prev) => format!("{prev} | {e}"),
                });
                (None, Some(e.clone()))
            }
        };
        let task_file = TaskRunFile {
            command: command_line.clone(),
            user_prompt: spec.parts.user_prompt.clone(),
            skill_prompt: spec.skill_prompt.clone(),
            pipeline_prompt: spec.parts.pipeline_prompt.clone(),
            prompt: spec.parts.combined.clone(),
            stdout: stdout.clone(),
            stderr: stderr.clone(),
            code: *code,
            error: err_opt,
            output,
        };
        write_json_atomic(&path, &task_file)?;
    }

    if stage_err.is_some() {
        bail!(
            "pipeline stage {} failed: {}",
            ctx.stage_num,
            stage_err.as_deref().unwrap_or("unknown")
        );
    }
    Ok(())
}

pub(crate) struct TaskBuild {
    #[allow(dead_code)] // reserved for UI / future structured output
    pub(crate) skill_label: String,
    pub(crate) skill_prompt: String,
    pub(crate) parts: PromptParts,
}

pub(crate) struct PromptParts {
    pub(crate) user_prompt: String,
    pub(crate) pipeline_prompt: String,
    pub(crate) combined: String,
}

/// Build one [`TaskBuild`] per `cursor-agent` invocation for `step` (same ordering as `run_stage`).
pub(crate) fn build_stage_task_builds(
    user_prompt: &str,
    step: &PipelineStepView,
    prior: &str,
    skills_store: &SkillsStore,
    debug: bool,
) -> Result<Vec<TaskBuild>> {
    let mut specs: Vec<TaskBuild> = Vec::new();

    if step.skills.is_empty() {
        let parts = prompt_parts(user_prompt, None, step, prior);
        specs.push(TaskBuild {
            skill_label: String::new(),
            skill_prompt: String::new(),
            parts,
        });
    } else {
        let mut names: Vec<String> = step
            .skills
            .iter()
            .filter_map(|s| s.resolved_name.clone())
            .collect();
        names.sort();
        for skill_name in names {
            debug_log(
                debug,
                format!("loading skill '{skill_name}' before cursor-agent"),
            );
            let skill_body = skills_store.load_skill(&skill_name).with_context(|| {
                format!(
                    "load skill '{skill_name}' (cursor-agent was not started; expected {})",
                    skills_store.skill_path(&skill_name).display()
                )
            })?;
            let parts = prompt_parts(user_prompt, Some((&skill_name, &skill_body)), step, prior);
            specs.push(TaskBuild {
                skill_label: skill_name.clone(),
                skill_prompt: skill_body,
                parts,
            });
        }
    }
    Ok(specs)
}

fn prompt_parts(
    user: &str,
    skill: Option<(&str, &str)>,
    step: &PipelineStepView,
    prior: &str,
) -> PromptParts {
    let user_prompt = user.to_string();
    let pipeline_prompt = step.prompt.clone();
    let combined = build_prompt(user, skill, step, prior);
    PromptParts {
        user_prompt,
        pipeline_prompt,
        combined,
    }
}

pub(crate) fn build_prompt(
    user: &str,
    skill: Option<(&str, &str)>,
    step: &PipelineStepView,
    prior: &str,
) -> String {
    let mut s = String::new();
    let prior_trim = prior.trim();
    if !prior_trim.is_empty() {
        s.push_str("<Context>\n");
        s.push_str(prior_trim);
        s.push_str("\n</Context>\n\n");
    }
    let pipe_trim = step.prompt.trim();
    if !pipe_trim.is_empty() {
        s.push_str("## Pipeline prompt\n\n");
        s.push_str("**Step:** ");
        s.push_str(&step.title);
        s.push_str("\n\n");
        s.push_str(pipe_trim);
        s.push_str("\n\n");
    }
    if let Some((name, body)) = skill {
        s.push_str("## Skill prompt\n\n");
        s.push_str("**Skill:** ");
        s.push_str(name);
        s.push_str("\n\n");
        s.push_str(body);
        s.push_str("\n\n");
    }
    s.push_str("## User prompt\n\n");
    s.push_str(user);
    s.push('\n');
    s
}

pub(crate) fn format_cursor_agent_invocation(
    dot: &DotPrimeAgentConfig,
    workspace: &Path,
) -> String {
    let force = if dot.yolo { " --force" } else { "" };
    format!(
        "{} --print{} --model {} --output-format json --workspace {}",
        dot.clirunner,
        force,
        dot.model,
        workspace.display()
    )
}

/// Extract display text from a task JSON object for `<Context>` (not the full record).
pub(crate) fn prior_task_body_for_context(v: &Value) -> String {
    if let Some(out) = v.get("output").and_then(|o| o.as_str())
        && !out.trim().is_empty()
    {
        return out.to_string();
    }
    let stdout = v.get("stdout").and_then(|s| s.as_str()).unwrap_or("");
    parse_agent_text(stdout)
}

pub(crate) fn read_prior_stage_json(out_dir: &Path, last_inclusive: u32) -> Result<String> {
    if last_inclusive == 0 {
        return Ok(String::new());
    }
    let mut acc = String::new();
    for stage in 1..=last_inclusive {
        let mut pairs: Vec<(u32, PathBuf)> = Vec::new();
        for entry in
            fs::read_dir(out_dir).with_context(|| format!("read_dir '{}'", out_dir.display()))?
        {
            let entry = entry?;
            let path = entry.path();
            let Some(name) = path.file_stem().and_then(|s| s.to_str()) else {
                continue;
            };
            let Some((s, tail)) = name.split_once('_') else {
                continue;
            };
            if s != stage.to_string() {
                continue;
            }
            let task_num = tail.parse::<u32>().unwrap_or(0);
            pairs.push((task_num, path));
        }
        pairs.sort_by_key(|(n, _)| *n);
        for (_, p) in pairs {
            let raw = fs::read_to_string(&p).with_context(|| format!("read '{}'", p.display()))?;
            let body = match serde_json::from_str::<Value>(&raw) {
                Ok(v) => prior_task_body_for_context(&v),
                Err(_) => raw.trim().to_string(),
            };
            let _ = writeln!(
                acc,
                "### Task file {}\n",
                p.file_name().and_then(|n| n.to_str()).unwrap_or("?")
            );
            acc.push_str(&body);
            acc.push_str("\n\n");
        }
    }
    Ok(acc)
}

/// Stream each stdout line to `line_tx` (if Some) while collecting full stdout/stderr.
/// When `stdout_tail` is Some, each stdout line is appended to the buffer, keeping at most the last N lines.
#[allow(clippy::too_many_lines)] // subprocess setup + thread join
#[allow(clippy::too_many_arguments)]
pub(crate) fn run_cursor_agent_streaming(
    binary: &str,
    model: &str,
    workspace: &Path,
    prompt: &str,
    line_tx: Option<mpsc::Sender<String>>,
    stdout_tail: Option<(Arc<Mutex<Vec<String>>>, usize)>,
    yolo: bool,
    debug_stream: Option<DebugStreamCtx>,
) -> (String, String, i32, Result<String, String>) {
    let mut child = None;
    for attempt in 0u32..8 {
        let mut cmd = Command::new(binary);
        cmd.arg("--print");
        if yolo {
            cmd.arg("--force");
        }
        match cmd
            .arg("--model")
            .arg(model)
            .arg("--output-format")
            .arg("json")
            .arg("--workspace")
            .arg(workspace)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
        {
            Ok(c) => {
                child = Some(c);
                break;
            }
            Err(e) => {
                let retry =
                    e.raw_os_error() == Some(26) || e.to_string().contains("Text file busy");
                if retry && attempt + 1 < 8 {
                    thread::sleep(Duration::from_millis(3u64 + u64::from(attempt) * 4));
                    continue;
                }
                return (
                    String::new(),
                    String::new(),
                    -1,
                    Err(format!("spawn {binary}: {e}")),
                );
            }
        }
    }
    let mut child = child.expect("spawn loop must set child or return");

    let Some(mut stdin) = child.stdin.take() else {
        let _ = child.wait();
        return (
            String::new(),
            String::new(),
            -1,
            Err("stdin unavailable".to_string()),
        );
    };
    if let Err(e) = std::io::Write::write_all(&mut stdin, prompt.as_bytes()) {
        drop(stdin);
        let _ = child.wait();
        return (
            String::new(),
            String::new(),
            -1,
            Err(format!("write stdin: {e}")),
        );
    }
    drop(stdin);

    let Some(stdout_pipe) = child.stdout.take() else {
        let _ = child.wait();
        return (
            String::new(),
            String::new(),
            -1,
            Err("stdout unavailable".to_string()),
        );
    };
    let Some(stderr_pipe) = child.stderr.take() else {
        let _ = child.wait();
        return (
            String::new(),
            String::new(),
            -1,
            Err("stderr unavailable".to_string()),
        );
    };

    let stdout_debug = debug_stream.clone();
    let stderr_debug = debug_stream;

    let stdout_handle = thread::spawn(move || {
        let mut acc = String::new();
        let reader = BufReader::new(stdout_pipe);
        for line in reader.lines() {
            let line = line.unwrap_or_default();
            if let Some(ref ctx) = stdout_debug {
                debug_stream_line(ctx, "stdout", &line);
            }
            acc.push_str(&line);
            acc.push('\n');
            if let Some((buf, max_lines)) = stdout_tail.as_ref() {
                let max_lines = (*max_lines).max(1);
                if let Ok(mut g) = buf.lock() {
                    g.push(line.clone());
                    while g.len() > max_lines {
                        g.remove(0);
                    }
                }
            }
            if let Some(ref tx) = line_tx {
                let _ = tx.send(line);
            }
        }
        acc
    });

    let stderr_handle = thread::spawn(move || {
        let mut acc = String::new();
        let mut reader = BufReader::new(stderr_pipe);
        let _ = reader.read_to_string(&mut acc);
        if let Some(ref ctx) = stderr_debug {
            for line in acc.lines() {
                debug_stream_line(ctx, "stderr", line);
            }
        }
        acc
    });

    let status = match child.wait() {
        Ok(s) => s,
        Err(e) => {
            return (String::new(), String::new(), -1, Err(format!("wait: {e}")));
        }
    };

    let stdout = stdout_handle.join().unwrap_or_default();
    let stderr = stderr_handle.join().unwrap_or_default();
    let code = status.code().unwrap_or(-1);

    if !status.success() {
        return (
            stdout.clone(),
            stderr.clone(),
            code,
            Err(format!("exit {code}: {}", stderr.trim())),
        );
    }
    let parsed = parse_agent_text(&stdout);
    (stdout, stderr, code, Ok(parsed))
}

fn try_extract_from_json_value(v: &Value) -> Option<String> {
    if let Some(s) = v.get("text").and_then(|t| t.as_str()) {
        return Some(s.to_string());
    }
    if let Some(s) = v.get("message").and_then(|t| t.as_str()) {
        return Some(s.to_string());
    }
    if let Some(s) = v.get("response").and_then(|t| t.as_str()) {
        return Some(s.to_string());
    }
    if let Some(s) = v.get("result").and_then(|t| t.as_str()) {
        return Some(s.to_string());
    }
    if let Some(choices) = v.get("choices").and_then(|c| c.as_array())
        && let Some(first) = choices.first()
        && let Some(s) = first.get("text").and_then(|t| t.as_str())
    {
        return Some(s.to_string());
    }
    None
}

pub(crate) fn parse_agent_text(stdout: &str) -> String {
    let trimmed = stdout.trim();
    if let Ok(v) = serde_json::from_str::<Value>(trimmed)
        && let Some(s) = try_extract_from_json_value(&v)
    {
        return s;
    }
    let mut last_line_extract: Option<String> = None;
    for line in stdout.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Ok(v) = serde_json::from_str::<Value>(line)
            && let Some(s) = try_extract_from_json_value(&v)
        {
            last_line_extract = Some(s);
        }
    }
    if let Some(s) = last_line_extract {
        return s;
    }
    trimmed.to_string()
}

pub(crate) fn write_json_atomic<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).with_context(|| format!("create '{}'", parent.display()))?;
    }
    let tmp = path.with_extension("json.tmp");
    let serialized = serde_json::to_string_pretty(value).context("serialize json")?;
    fs::write(&tmp, format!("{serialized}\n"))
        .with_context(|| format!("write '{}'", tmp.display()))?;
    fs::rename(&tmp, path).with_context(|| format!("rename to '{}'", path.display()))?;
    Ok(())
}

pub(crate) fn generate_run_name_with_rng(rng: &mut impl Rng) -> String {
    const ADJ: &[&str] = &[
        "quiet", "brave", "calm", "swift", "gentle", "bright", "clever", "noble", "wild", "keen",
        "brisk", "crisp", "steady", "nimble", "subtle", "rugged", "solemn", "merry", "vivid",
        "lucid", "hardy", "stoic", "rustic", "cosmic", "floral", "sonic", "timely", "latent",
        "docile", "fierce", "agile", "ample", "ardent", "hollow", "brittle", "lofty", "narrow",
        "patient", "radiant", "dapper", "sunny", "misty", "stormy", "frosty", "balmy", "dusty",
        "smoky", "musky", "tangy", "zesty", "humble", "proud", "sober", "jolly", "weary", "cozy",
        "grimy", "glossy", "velvety", "silken", "azure", "crimson", "scarlet", "violet", "indigo",
        "saffron", "ochre", "umber", "sepia", "teal", "plucky", "sturdy", "pliant", "curvy",
        "linear", "planar",
    ];
    const NOUN: &[&str] = &[
        "harbor", "meadow", "cipher", "compass", "beacon", "atlas", "vertex", "summit", "delta",
        "quartz", "brook", "canyon", "crest", "drift", "ember", "fjord", "glacier", "inlet",
        "island", "lagoon", "marsh", "node", "oasis", "peak", "prism", "quarry", "reef", "ridge",
        "river", "spire", "tundra", "upland", "vale", "widget", "nova", "orbit", "pixel", "quanta",
        "raster", "signal", "vector", "tensor", "matrix", "bundle", "packet", "stream", "window",
        "bridge", "tunnel", "gateway", "portal", "pillar", "cavern", "grotto", "alcove", "niche",
        "vault", "dome", "arch", "span", "shelter", "gazebo", "pavilion", "terrace", "veranda",
        "atrium", "cloister", "corridor", "gallery", "foyer", "stairway", "doorway", "pathway",
        "rocket", "comet",
    ];
    let ai = rng.gen_range(0..ADJ.len());
    let ni = rng.gen_range(0..NOUN.len());
    format!("{}-{}", ADJ[ai], NOUN[ni])
}

pub(crate) fn generate_run_name() -> String {
    generate_run_name_with_rng(&mut rand::thread_rng())
}

#[cfg(test)]
mod tests {
    use super::{
        build_prompt, generate_run_name_with_rng, parse_agent_text, prior_task_body_for_context,
        read_prior_stage_json,
    };
    use crate::pipeline_store::PipelineStepView;
    use rand::rngs::StdRng;
    use rand::SeedableRng;
    use serde_json::json;
    use std::fs;
    use tempfile::TempDir;

    fn step_view(title: &str, prompt: &str) -> PipelineStepView {
        PipelineStepView {
            id: 1,
            title: title.to_string(),
            prompt: prompt.to_string(),
            skill_count: 0,
            skills: vec![],
        }
    }

    #[test]
    fn generate_run_name_seeded_is_deterministic() {
        let mut a = StdRng::seed_from_u64(42);
        let mut b = StdRng::seed_from_u64(42);
        assert_eq!(
            generate_run_name_with_rng(&mut a),
            generate_run_name_with_rng(&mut b)
        );
    }

    #[test]
    fn generate_run_name_seeded_sequence_differs() {
        let mut rng = StdRng::seed_from_u64(7);
        let x = generate_run_name_with_rng(&mut rng);
        let y = generate_run_name_with_rng(&mut rng);
        assert_ne!(x, y);
    }

    #[test]
    fn parse_agent_text_extracts_cursor_result_field() {
        let j = r#"{"type":"result","subtype":"success","result":"hello"}"#;
        assert_eq!(parse_agent_text(j), "hello");
    }

    #[test]
    fn parse_agent_text_ndjson_last_line_wins() {
        let s = "{\"noise\":1}\n{\"type\":\"result\",\"result\":\"last\"}\n";
        assert_eq!(parse_agent_text(s), "last");
    }

    #[test]
    fn parse_agent_text_nonjson_returns_trimmed_raw() {
        assert_eq!(parse_agent_text("  plain  "), "plain");
    }

    #[test]
    fn prior_task_body_prefers_output_over_stdout() {
        let v = json!({
            "output": "out-field",
            "stdout": "{\"text\":\"from-stdout\"}"
        });
        assert_eq!(prior_task_body_for_context(&v), "out-field");
    }

    #[test]
    fn prior_task_body_parses_stdout_when_output_empty() {
        let v = json!({
            "output": "",
            "stdout": "{\"result\":\"from-out\"}"
        });
        assert_eq!(prior_task_body_for_context(&v), "from-out");
    }

    #[test]
    fn read_prior_stage_json_uses_extracted_body_not_raw_json() {
        let dir = TempDir::new().expect("temp");
        let task = json!({
            "stdout": "{\"text\":\"agent-said\"}",
            "output": null,
            "code": 0
        });
        fs::write(
            dir.path().join("1_1.json"),
            serde_json::to_string_pretty(&task).expect("json"),
        )
        .expect("write");
        let got = read_prior_stage_json(dir.path(), 1).expect("read");
        assert!(got.contains("agent-said"));
        assert!(!got.contains("\"stdout\":"));
    }

    #[test]
    fn build_prompt_omits_context_when_prior_empty() {
        let step = step_view("s", "pipe");
        let p = build_prompt("u", None, &step, "");
        assert!(!p.contains("<Context>"));
        assert!(p.contains("## Pipeline prompt"));
    }

    #[test]
    fn build_prompt_order_context_pipeline_skill_user() {
        let step = step_view("my-step", "pipe-body");
        let prior = "### Task file 1_1.json\n\nprior-text\n\n";
        let p = build_prompt("user-body", Some(("sk", "skill-body")), &step, prior);
        let ctx = p.find("<Context>").expect("context");
        let pipe = p.find("## Pipeline prompt").expect("pipe");
        let sk = p.find("## Skill prompt").expect("skill");
        let us = p.find("## User prompt").expect("user");
        assert!(ctx < pipe && pipe < sk && sk < us);
        assert!(p.contains("prior-text"));
        assert!(p.contains("pipe-body"));
        assert!(p.contains("skill-body"));
        assert!(p.contains("user-body"));
    }

    #[test]
    fn build_prompt_skips_pipeline_when_step_prompt_empty() {
        let step = step_view("t", "");
        let p = build_prompt("u", None, &step, "");
        assert!(!p.contains("## Pipeline prompt"));
        assert!(p.contains("## User prompt"));
    }

    #[test]
    fn build_prompt_skips_skill_when_none() {
        let step = step_view("t", "p");
        let p = build_prompt("u", None, &step, "");
        assert!(!p.contains("## Skill prompt"));
    }

    #[test]
    fn build_prompt_includes_skill_with_metadata() {
        let step = step_view("st", "pv");
        let p = build_prompt("uu", Some(("alpha", "body")), &step, "");
        assert!(p.contains("## Skill prompt"));
        assert!(p.contains("**Skill:** alpha"));
        assert!(p.contains("body"));
    }
}
