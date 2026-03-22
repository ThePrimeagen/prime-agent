//! Run pipeline stages via `cursor-agent`, writing `.prime-agent/pipeline-<name>/{N}.json`.

use anyhow::{bail, Context, Result};
use serde::Serialize;
use serde_json::Value;
use std::fmt::Write as FmtWrite;
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;

use crate::dot_prime_agent_config::DotPrimeAgentConfig;
use crate::pipeline_store::{PipelineStepView, PipelineStore};
use crate::skills_store::SkillsStore;

const SUPPORTED_CLIRUNNER: &str = "cursor-agent";

/// How to run the pipeline (plain stdout vs full-screen TUI).
#[derive(Debug, Clone, Copy)]
pub struct PipelineRunOptions {
    pub use_tui: bool,
    pub stdout_lines: u32,
}

#[derive(Serialize)]
pub(crate) struct MetaFile {
    pub(crate) run_name: String,
    pub(crate) pipeline: String,
    pub(crate) model: String,
    pub(crate) clirunner: String,
}

#[derive(Serialize)]
pub(crate) struct StageFile {
    pub(crate) stage: u32,
    pub(crate) step_id: i64,
    pub(crate) title: String,
    pub(crate) name: Vec<String>,
    pub(crate) input_prompt: Vec<String>,
    pub(crate) output: Vec<String>,
    pub(crate) stdout: Vec<String>,
    pub(crate) stderr: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) error: Option<String>,
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
    if options.use_tui {
        return crate::pipeline_tui::run_tui(
            pipeline_name,
            user_prompt,
            data_dir,
            skills_store,
            dot_config,
            cwd,
            options.stdout_lines,
        );
    }
    run_plain(
        pipeline_name,
        user_prompt,
        data_dir,
        skills_store,
        dot_config,
        cwd,
    )
}

fn run_plain(
    pipeline_name: &str,
    user_prompt: &str,
    data_dir: &Path,
    skills_store: &SkillsStore,
    dot_config: &DotPrimeAgentConfig,
    cwd: &Path,
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

    println!("{run_name}");

    let workspace = cwd
        .canonicalize()
        .unwrap_or_else(|_| cwd.to_path_buf());

    for (idx, step) in steps.iter().enumerate() {
        let stage_num = u32::try_from(idx + 1).unwrap_or(u32::MAX);
        let stage_path = out_dir.join(format!("{stage_num}.json"));
        if stage_file_is_complete(&stage_path, step)? {
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
        };
        run_stage(&ctx)?;
    }

    Ok(())
}

pub(crate) fn stage_file_is_complete(path: &Path, step: &PipelineStepView) -> Result<bool> {
    if !path.exists() {
        return Ok(false);
    }
    let raw = fs::read_to_string(path).context("read stage file")?;
    let v: Value = serde_json::from_str(&raw).context("parse stage file")?;
    if v.get("error").and_then(|e| e.as_str()).filter(|s| !s.is_empty()).is_some() {
        return Ok(false);
    }
    let expected = expected_task_count(step);
    let out_len = v
        .get("output")
        .and_then(|o| o.as_array())
        .map_or(0, Vec::len);
    Ok(out_len == expected)
}

pub(crate) fn expected_task_count(step: &PipelineStepView) -> usize {
    if step.skills.is_empty() {
        1
    } else {
        step.skills.len()
    }
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
}

fn run_stage(ctx: &RunStageCtx<'_>) -> Result<()> {
    let prior = read_prior_stage_json(ctx.out_dir, ctx.prev_stages)?;
    let mut task_specs: Vec<TaskSpec> = Vec::new();

    if ctx.step.skills.is_empty() {
        let prompt = build_prompt(ctx.user_prompt, None, ctx.step, &prior);
        task_specs.push(TaskSpec {
            skill_label: String::new(),
            prompt,
        });
    } else {
        let mut names: Vec<String> = ctx.step.skills.iter().map(|s| s.name.clone()).collect();
        names.sort();
        for skill_name in names {
            let skill_body = ctx
                .skills_store
                .load_skill(&skill_name)
                .with_context(|| format!("load skill '{skill_name}'"))?;
            let prompt = build_prompt(
                ctx.user_prompt,
                Some((&skill_name, &skill_body)),
                ctx.step,
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
        for spec in &task_specs {
            let prompt = spec.prompt.clone();
            let model = ctx.dot_config.model.clone();
            let workspace = ctx.workspace.to_path_buf();
            let binary = ctx.dot_config.clirunner.clone();
            handles.push(scope.spawn(move || {
                run_cursor_agent(&binary, &model, &workspace, &prompt)
            }));
        }
        handles
            .into_iter()
            .map(|h| h.join().unwrap_or_else(|_| {
                (
                    String::new(),
                    String::new(),
                    Err("cursor-agent task panicked".to_string()),
                )
            }))
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

    let stage_file = StageFile {
        stage: ctx.stage_num,
        step_id: ctx.step.id,
        title: ctx.step.title.clone(),
        name,
        input_prompt,
        output,
        stdout,
        stderr,
        error: stage_err,
    };

    let path = ctx.out_dir.join(format!("{}.json", ctx.stage_num));
    write_json_atomic(&path, &stage_file)?;
    if stage_file.error.is_some() {
        bail!(
            "pipeline stage {} failed: {}",
            ctx.stage_num,
            stage_file.error.as_deref().unwrap_or("unknown")
        );
    }
    Ok(())
}

pub(crate) struct TaskSpec {
    pub(crate) skill_label: String,
    pub(crate) prompt: String,
}

pub(crate) fn build_prompt(
    user: &str,
    skill: Option<(&str, &str)>,
    step: &PipelineStepView,
    prior: &str,
) -> String {
    let mut s = String::new();
    s.push_str("## User prompt\n\n");
    s.push_str(user);
    s.push_str("\n\n");
    if let Some((name, body)) = skill {
        s.push_str("## Skill (");
        s.push_str(name);
        s.push_str(")\n\n");
        s.push_str(body);
        s.push_str("\n\n");
    }
    s.push_str("## Pipeline step (");
    s.push_str(&step.title);
    s.push_str(")\n\n");
    s.push_str(&step.prompt);
    s.push_str("\n\n");
    if !prior.is_empty() {
        s.push_str("## Previous stage outputs (JSON files 1..N-1)\n\n");
        s.push_str(prior);
        s.push('\n');
    }
    s
}

pub(crate) fn read_prior_stage_json(out_dir: &Path, last_inclusive: u32) -> Result<String> {
    if last_inclusive == 0 {
        return Ok(String::new());
    }
    let mut acc = String::new();
    for n in 1..=last_inclusive {
        let p = out_dir.join(format!("{n}.json"));
        if !p.exists() {
            continue;
        }
        let raw = fs::read_to_string(&p).with_context(|| format!("read '{}'", p.display()))?;
        let _ = writeln!(acc, "### Stage file {n}.json\n");
        acc.push_str(&raw);
        acc.push_str("\n\n");
    }
    Ok(acc)
}

/// Stream each stdout line to `line_tx` (if Some) while collecting full stdout/stderr.
#[allow(clippy::too_many_lines)] // subprocess setup + thread join
pub(crate) fn run_cursor_agent_streaming(
    binary: &str,
    model: &str,
    workspace: &Path,
    prompt: &str,
    line_tx: Option<mpsc::Sender<String>>,
) -> (String, String, Result<String, String>) {
    let mut child = match Command::new(binary)
        .arg("--print")
        .arg("--yolo")
        .arg("--model")
        .arg(model)
        .arg("--output-format")
        .arg("json")
        .arg("--trust")
        .arg("--workspace")
        .arg(workspace)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => {
            return (
                String::new(),
                String::new(),
                Err(format!("spawn {binary}: {e}")),
            );
        }
    };

    let Some(mut stdin) = child.stdin.take() else {
        return (
            String::new(),
            String::new(),
            Err("stdin unavailable".to_string()),
        );
    };
    if let Err(e) = std::io::Write::write_all(&mut stdin, prompt.as_bytes()) {
        return (
            String::new(),
            String::new(),
            Err(format!("write stdin: {e}")),
        );
    }
    drop(stdin);

    let Some(stdout_pipe) = child.stdout.take() else {
        return (
            String::new(),
            String::new(),
            Err("stdout unavailable".to_string()),
        );
    };
    let Some(mut stderr_pipe) = child.stderr.take() else {
        return (
            String::new(),
            String::new(),
            Err("stderr unavailable".to_string()),
        );
    };

    let stdout_handle = thread::spawn(move || {
        let mut acc = String::new();
        let reader = BufReader::new(stdout_pipe);
        for line in reader.lines() {
            let line = line.unwrap_or_default();
            acc.push_str(&line);
            acc.push('\n');
            if let Some(ref tx) = line_tx {
                let _ = tx.send(line);
            }
        }
        acc
    });

    let stderr_handle = thread::spawn(move || {
        let mut s = String::new();
        let _ = std::io::Read::read_to_string(&mut stderr_pipe, &mut s);
        s
    });

    let status = match child.wait() {
        Ok(s) => s,
        Err(e) => {
            return (
                String::new(),
                String::new(),
                Err(format!("wait: {e}")),
            );
        }
    };

    let stdout = stdout_handle.join().unwrap_or_default();
    let stderr = stderr_handle.join().unwrap_or_default();

    if !status.success() {
        return (
            stdout.clone(),
            stderr.clone(),
            Err(format!(
                "exit {}: {}",
                status.code().unwrap_or(-1),
                stderr.trim()
            )),
        );
    }
    let parsed = parse_agent_text(&stdout);
    (stdout, stderr, Ok(parsed))
}

fn run_cursor_agent(
    binary: &str,
    model: &str,
    workspace: &Path,
    prompt: &str,
) -> (String, String, Result<String, String>) {
    let mut child = match Command::new(binary)
        .arg("--print")
        .arg("--yolo")
        .arg("--model")
        .arg(model)
        .arg("--output-format")
        .arg("json")
        .arg("--trust")
        .arg("--workspace")
        .arg(workspace)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => {
            return (
                String::new(),
                String::new(),
                Err(format!("spawn {binary}: {e}")),
            );
        }
    };

    let Some(mut stdin) = child.stdin.take() else {
        return (
            String::new(),
            String::new(),
            Err("stdin unavailable".to_string()),
        );
    };
    if let Err(e) = std::io::Write::write_all(&mut stdin, prompt.as_bytes()) {
        return (
            String::new(),
            String::new(),
            Err(format!("write stdin: {e}")),
        );
    }
    drop(stdin);

    let out = match child.wait_with_output() {
        Ok(o) => o,
        Err(e) => {
            return (
                String::new(),
                String::new(),
                Err(format!("wait: {e}")),
            );
        }
    };
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
    if !out.status.success() {
        return (
            stdout.clone(),
            stderr.clone(),
            Err(format!(
                "exit {}: {}",
                out.status.code().unwrap_or(-1),
                stderr.trim()
            )),
        );
    }
    let parsed = parse_agent_text(&stdout);
    (stdout, stderr, Ok(parsed))
}

pub(crate) fn parse_agent_text(stdout: &str) -> String {
    let trimmed = stdout.trim();
    if let Ok(v) = serde_json::from_str::<Value>(trimmed) {
        if let Some(s) = v.get("text").and_then(|t| t.as_str()) {
            return s.to_string();
        }
        if let Some(s) = v.get("message").and_then(|t| t.as_str()) {
            return s.to_string();
        }
        if let Some(s) = v.get("response").and_then(|t| t.as_str()) {
            return s.to_string();
        }
        if let Some(choices) = v.get("choices").and_then(|c| c.as_array())
            && let Some(first) = choices.first()
            && let Some(s) = first.get("text").and_then(|t| t.as_str())
        {
            return s.to_string();
        }
    }
    trimmed.to_string()
}

pub(crate) fn write_json_atomic<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("create '{}'", parent.display()))?;
    }
    let tmp = path.with_extension("json.tmp");
    let serialized =
        serde_json::to_string_pretty(value).context("serialize json")?;
    fs::write(&tmp, format!("{serialized}\n")).with_context(|| format!("write '{}'", tmp.display()))?;
    fs::rename(&tmp, path).with_context(|| format!("rename to '{}'", path.display()))?;
    Ok(())
}

pub(crate) fn generate_run_name(pipeline: &str) -> String {
    const ADJ: &[&str] = &[
        "quiet", "brave", "calm", "swift", "gentle", "bright", "clever", "noble", "wild", "keen",
    ];
    const NOUN: &[&str] = &[
        "harbor", "meadow", "cipher", "compass", "beacon", "atlas", "vertex", "summit", "delta",
        "quartz",
    ];
    let mut h: u64 = 14_695_981_039_346_656_037;
    for b in pipeline.as_bytes() {
        h ^= u64::from(*b);
        h = h.wrapping_mul(1_099_511_628_211);
    }
    let ai = usize::try_from(h % ADJ.len() as u64).unwrap_or(0);
    let ni = usize::try_from((h >> 32) % NOUN.len() as u64).unwrap_or(0);
    format!(
        "{} {}",
        capitalize(ADJ[ai]),
        capitalize(NOUN[ni])
    )
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        None => String::new(),
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
    }
}
