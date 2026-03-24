mod common;

use assert_cmd::cargo::cargo_bin_cmd;
use common::{
    chmod_x, pipeline_artifact_dir_for, pipelines_cmd, write_dot_prime_agent_config,
    write_dot_prime_agent_config_yolo, write_pipeline, write_skill_with_id,
};
use serde_json::json;
use std::collections::HashSet;
use std::fs;
use tempfile::TempDir;
use uuid::Uuid;

#[test]
fn pipelines_run_resolves_skills_under_explicit_data_dir() {
    let temp = TempDir::new().expect("temp");
    let data_dir = temp.path().join("skill-issues");
    let skills_under_data = data_dir.join("skills");
    let sid = Uuid::parse_str("00000000-0000-4000-8000-00000000a111").expect("uuid");
    write_skill_with_id(&skills_under_data, "attached-skill", &sid, "x\n");

    let config_home = temp.path().join("xdg_config");
    fs::create_dir_all(config_home.join("prime-agent")).expect("prime cfg dir");

    write_dot_prime_agent_config(&temp, "m", "cursor-agent");
    let pj = format!(
        r#"{{"steps":[{{"id":1,"title":"a","prompt":"p","skills":[{{"id":"{sid}","alias":"attached-skill"}}]}}]}}"#
    );
    write_pipeline(&data_dir, "pl", &pj);
    let bin = temp.path().join("bin");
    fs::create_dir_all(&bin).expect("bin");
    let mock = bin.join("cursor-agent");
    fs::write(
        &mock,
        "#!/bin/sh\ncat >/dev/null\necho '{\"text\":\"ok\"}'\n",
    )
    .expect("mock");
    chmod_x(&mock);

    let path_var = format!(
        "{}:{}",
        bin.display(),
        std::env::var("PATH").unwrap_or_default()
    );
    let mut cmd = cargo_bin_cmd!("prime-agent");
    cmd.current_dir(temp.path())
        .env("PATH", &path_var)
        .env("XDG_CONFIG_HOME", &config_home)
        .env("PRIME_AGENT_NO_TUI", "1")
        .args([
            "--data-dir",
            data_dir.to_str().expect("utf8 path"),
            "run",
            "pl",
            "--prompt",
            "u",
        ]);
    cmd.assert().success();
}

#[test]
fn pipelines_run_writes_stage_files_under_pipeline_dir() {
    let temp = TempDir::new().expect("temp");
    let data_dir = temp.path().join("data");
    let skills_dir = temp.path().join("skills");
    fs::create_dir_all(&skills_dir).expect("skills");
    write_dot_prime_agent_config(&temp, "composer-2-fast", "cursor-agent");
    write_pipeline(
        &data_dir,
        "demo-pipe",
        r#"{"steps":[{"id":1,"title":"stepone","prompt":"doprompt","skills":[]}]}"#,
    );
    let bin = temp.path().join("bin");
    fs::create_dir_all(&bin).expect("bin");
    let mock = bin.join("cursor-agent");
    fs::write(
        &mock,
        "#!/bin/sh\ncat >/dev/null\necho '{\"text\":\"out1\"}'\n",
    )
    .expect("mock");
    chmod_x(&mock);

    let mut cmd = pipelines_cmd(&temp, &data_dir, &skills_dir, &bin);
    cmd.args(["run", "demo-pipe", "--prompt", "userhi"]);
    cmd.assert().success();

    let one = pipeline_artifact_dir_for(temp.path(), "demo-pipe").join("1_1.json");
    let raw = fs::read_to_string(&one).expect("1_1.json");
    let v: serde_json::Value = serde_json::from_str(&raw).expect("parse");
    assert_eq!(v["code"], 0);
    assert!(v["stdout"].as_str().is_some());
    assert!(v["stderr"].as_str().is_some());
    assert_eq!(v["output"], json!("out1"));
}

/// A second run with the same prompt allocates a new run directory and invokes the agent again.
#[test]
fn pipelines_second_run_same_prompt_invokes_agent_again() {
    let temp = TempDir::new().expect("temp");
    let data_dir = temp.path().join("data");
    let skills_dir = temp.path().join("skills");
    fs::create_dir_all(&skills_dir).expect("skills");
    write_dot_prime_agent_config(&temp, "composer-2-fast", "cursor-agent");
    write_pipeline(
        &data_dir,
        "demo-pipe",
        r#"{"steps":[{"id":1,"title":"stepone","prompt":"doprompt","skills":[]}]}"#,
    );
    let bin = temp.path().join("bin");
    fs::create_dir_all(&bin).expect("bin");
    let calls_log = temp.path().join("cursor_invocations");
    let mock = bin.join("cursor-agent");
    let body = format!(
        "#!/bin/sh\necho x >> \"{}\"\ncat >/dev/null\necho '{{\"text\":\"out1\"}}'\n",
        calls_log.display()
    );
    fs::write(&mock, body).expect("mock");
    chmod_x(&mock);

    let mut cmd = pipelines_cmd(&temp, &data_dir, &skills_dir, &bin);
    cmd.args(["run", "demo-pipe", "--prompt", "userhi"]);
    cmd.assert().success();

    let mut cmd2 = pipelines_cmd(&temp, &data_dir, &skills_dir, &bin);
    cmd2.args(["run", "demo-pipe", "--prompt", "userhi"]);
    cmd2.assert().success();

    let n = fs::read_to_string(&calls_log).expect("calls log");
    assert_eq!(n.lines().filter(|l| !l.is_empty()).count(), 2);
}

#[test]
fn pipelines_two_runs_create_distinct_artifact_directories() {
    let temp = TempDir::new().expect("temp");
    let data_dir = temp.path().join("data");
    let skills_dir = temp.path().join("skills");
    fs::create_dir_all(&skills_dir).expect("skills");
    write_dot_prime_agent_config(&temp, "composer-2-fast", "cursor-agent");
    write_pipeline(
        &data_dir,
        "demo-pipe",
        r#"{"steps":[{"id":1,"title":"stepone","prompt":"doprompt","skills":[]}]}"#,
    );
    let bin = temp.path().join("bin");
    fs::create_dir_all(&bin).expect("bin");
    let mock = bin.join("cursor-agent");
    fs::write(
        &mock,
        "#!/bin/sh\ncat >/dev/null\necho '{\"text\":\"out1\"}'\n",
    )
    .expect("mock");
    chmod_x(&mock);

    for _ in 0..2 {
        let mut cmd = pipelines_cmd(&temp, &data_dir, &skills_dir, &bin);
        cmd.args(["run", "demo-pipe", "--prompt", "userhi"]);
        cmd.assert().success();
    }

    let root = temp.path().join(".prime-agent/pipelines");
    let mut slugs = HashSet::new();
    for entry in fs::read_dir(&root).expect("read pipelines") {
        let p = entry.expect("entry").path();
        if !p.is_dir() || !p.join("meta.json").is_file() {
            continue;
        }
        let raw = fs::read_to_string(p.join("meta.json")).expect("meta");
        let v: serde_json::Value = serde_json::from_str(&raw).expect("parse");
        if v.get("pipeline").and_then(|x| x.as_str()) == Some("demo-pipe") {
            slugs.insert(
                p.file_name()
                    .expect("file_name")
                    .to_str()
                    .expect("utf8")
                    .to_string(),
            );
        }
    }
    assert_eq!(
        slugs.len(),
        2,
        "expected two distinct run directories for demo-pipe, got {slugs:?}"
    );
}

#[test]
fn pipelines_run_parallel_skills_two_outputs() {
    let temp = TempDir::new().expect("temp");
    let data_dir = temp.path().join("data");
    let skills_dir = temp.path().join("skills");
    let id_a = Uuid::parse_str("00000000-0000-4000-8000-00000000aa01").expect("a");
    let id_b = Uuid::parse_str("00000000-0000-4000-8000-00000000bb02").expect("b");
    write_skill_with_id(&skills_dir, "alpha", &id_a, "A\n");
    write_skill_with_id(&skills_dir, "beta", &id_b, "B\n");
    write_dot_prime_agent_config(&temp, "m", "cursor-agent");
    let pj = format!(
        r#"{{"steps":[{{"id":1,"title":"t","prompt":"p","skills":[{{"id":"{id_b}","alias":"beta"}},{{"id":"{id_a}","alias":"alpha"}}]}}]}}"#
    );
    write_pipeline(&data_dir, "pl", &pj);
    let bin = temp.path().join("bin");
    fs::create_dir_all(&bin).expect("bin");
    let mock = bin.join("cursor-agent");
    fs::write(
        &mock,
        "#!/bin/sh\ncat >/dev/null\necho '{\"text\":\"parallel\"}'\n",
    )
    .expect("mock");
    chmod_x(&mock);

    let mut cmd = pipelines_cmd(&temp, &data_dir, &skills_dir, &bin);
    cmd.args(["run", "pl", "--prompt", "u"]);
    cmd.assert().success();

    let out_dir = pipeline_artifact_dir_for(temp.path(), "pl");
    for (task, sk) in [("1_1", "alpha"), ("1_2", "beta")] {
        let p = out_dir.join(format!("{task}.json"));
        let raw = fs::read_to_string(&p).expect("task json");
        let v: serde_json::Value = serde_json::from_str(&raw).expect("parse");
        assert_eq!(v["code"], 0);
        assert_eq!(v["output"], json!("parallel"));
        assert!(
            v["prompt"].as_str().is_some_and(|s| s.contains(sk)),
            "expected skill {sk} in prompt: {:?}",
            v["prompt"]
        );
    }
}

#[test]
fn pipelines_run_second_stage_receives_prior_stage_files_in_prompt() {
    let temp = TempDir::new().expect("temp");
    let data_dir = temp.path().join("data");
    let skills_dir = temp.path().join("skills");
    fs::create_dir_all(&skills_dir).expect("skills");
    write_dot_prime_agent_config(&temp, "m", "cursor-agent");
    write_pipeline(
        &data_dir,
        "pl",
        r#"{"steps":[{"id":1,"title":"s1","prompt":"p1","skills":[]},{"id":2,"title":"s2","prompt":"p2","skills":[]}]}"#,
    );
    let bin = temp.path().join("bin");
    fs::create_dir_all(&bin).expect("bin");
    let stdin_log = temp.path().join("stdin.log");
    let mock = bin.join("cursor-agent");
    let body = format!(
        "#!/bin/sh\ncat > \"{}\"\necho '{{\"text\":\"ok\"}}'\n",
        stdin_log.display()
    );
    fs::write(&mock, body).expect("mock");
    chmod_x(&mock);

    let mut cmd = pipelines_cmd(&temp, &data_dir, &skills_dir, &bin);
    cmd.args(["run", "pl", "--prompt", "u"]);
    cmd.assert().success();

    let stage2 = fs::read_to_string(&stdin_log).expect("stdin");
    assert!(
        stage2.contains("<Context>") && stage2.contains("</Context>"),
        "expected Context wrapper in stage-2 prompt: {stage2}"
    );
    assert!(
        stage2.contains("ok"),
        "expected extracted prior output text in stage-2 prompt: {stage2}"
    );
    assert!(
        !stage2.contains("\"stdout\""),
        "should not embed raw task JSON in Context: {stage2}"
    );
}

#[test]
fn pipelines_run_empty_step_prompt_omits_pipeline_section_in_stdin() {
    let temp = TempDir::new().expect("temp");
    let data_dir = temp.path().join("data");
    let skills_dir = temp.path().join("skills");
    fs::create_dir_all(&skills_dir).expect("skills");
    write_dot_prime_agent_config(&temp, "m", "cursor-agent");
    write_pipeline(
        &data_dir,
        "pl",
        r#"{"steps":[{"id":1,"title":"only-title","prompt":"","skills":[]}]}"#,
    );
    let bin = temp.path().join("bin");
    fs::create_dir_all(&bin).expect("bin");
    let stdin_log = temp.path().join("stdin.log");
    let mock = bin.join("cursor-agent");
    let body = format!(
        "#!/bin/sh\ncat > \"{}\"\necho '{{\"text\":\"x\"}}'\n",
        stdin_log.display()
    );
    fs::write(&mock, body).expect("mock");
    chmod_x(&mock);

    let mut cmd = pipelines_cmd(&temp, &data_dir, &skills_dir, &bin);
    cmd.args(["run", "pl", "--prompt", "userhi"]);
    cmd.assert().success();

    let logged = fs::read_to_string(&stdin_log).expect("stdin");
    assert!(
        !logged.contains("## Pipeline prompt"),
        "empty step prompt should omit pipeline section: {logged}"
    );
    assert!(logged.contains("## User prompt"));
}

#[test]
fn pipelines_run_does_not_resume_preexisting_run_directory() {
    let temp = TempDir::new().expect("temp");
    let data_dir = temp.path().join("data");
    let skills_dir = temp.path().join("skills");
    fs::create_dir_all(&skills_dir).expect("skills");
    write_dot_prime_agent_config(&temp, "m", "cursor-agent");
    write_pipeline(
        &data_dir,
        "pl",
        r#"{"steps":[{"id":1,"title":"s1","prompt":"p1","skills":[]},{"id":2,"title":"s2","prompt":"p2","skills":[]}]}"#,
    );
    let out_dir = temp.path().join(".prime-agent/pipelines/quiet-harbor");
    fs::create_dir_all(&out_dir).expect("out");
    fs::write(
        out_dir.join("meta.json"),
        r#"{"run_name":"quiet-harbor","pipeline":"pl","model":"m","clirunner":"cursor-agent"}
"#,
    )
    .expect("meta");
    fs::write(
        out_dir.join("1_1.json"),
        r#"{"command":"c","user_prompt":"u","skill_prompt":"","pipeline_prompt":"p1","prompt":"x","stdout":"so","stderr":"","code":0,"output":"done"}
"#,
    )
    .expect("1_1.json");

    let bin = temp.path().join("bin");
    fs::create_dir_all(&bin).expect("bin");
    let argv_log = temp.path().join("argv.log");
    let mock = bin.join("cursor-agent");
    let body = format!(
        "#!/bin/sh\necho \"$@\" >> \"{}\"\ncat >/dev/null\necho '{{\"text\":\"two\"}}'\n",
        argv_log.display()
    );
    fs::write(&mock, body).expect("mock");
    chmod_x(&mock);

    let mut cmd = pipelines_cmd(&temp, &data_dir, &skills_dir, &bin);
    cmd.args(["run", "pl", "--prompt", "u"]);
    cmd.assert().success();

    let logged = fs::read_to_string(&argv_log).expect("argv log");
    assert_eq!(
        logged.lines().filter(|l| !l.is_empty()).count(),
        2,
        "each invocation uses a fresh run dir; both stages should run: {logged}"
    );
}

#[test]
fn pipelines_run_ignores_stale_run_artifacts_on_disk() {
    let temp = TempDir::new().expect("temp");
    let data_dir = temp.path().join("data");
    let skills_dir = temp.path().join("skills");
    fs::create_dir_all(&skills_dir).expect("skills");
    write_dot_prime_agent_config(&temp, "m", "cursor-agent");
    write_pipeline(
        &data_dir,
        "pl",
        r#"{"steps":[{"id":1,"title":"s1","prompt":"p1","skills":[]}]}"#,
    );
    let out_dir = temp.path().join(".prime-agent/pipelines/calm-meadow");
    fs::create_dir_all(&out_dir).expect("out");
    fs::write(
        out_dir.join("meta.json"),
        r#"{"run_name":"calm-meadow","pipeline":"pl","model":"m","clirunner":"cursor-agent"}
"#,
    )
    .expect("meta");
    fs::write(
        out_dir.join("1_1.json"),
        r#"{"command":"c","user_prompt":"old-prompt","skill_prompt":"","pipeline_prompt":"p1","prompt":"x","stdout":"so","stderr":"","code":0,"output":"done"}
"#,
    )
    .expect("1_1.json");

    let bin = temp.path().join("bin");
    fs::create_dir_all(&bin).expect("bin");
    let argv_log = temp.path().join("argv.log");
    let mock = bin.join("cursor-agent");
    let body = format!(
        "#!/bin/sh\necho \"$@\" >> \"{}\"\ncat >/dev/null\necho '{{\"text\":\"new\"}}'\n",
        argv_log.display()
    );
    fs::write(&mock, body).expect("mock");
    chmod_x(&mock);

    let mut cmd = pipelines_cmd(&temp, &data_dir, &skills_dir, &bin);
    cmd.args(["run", "pl", "--prompt", "new-prompt"]);
    cmd.assert().success();

    let logged = fs::read_to_string(&argv_log).expect("argv log");
    assert_eq!(
        logged.lines().filter(|l| !l.is_empty()).count(),
        1,
        "fresh run dir; single stage runs once: {logged}"
    );
}

/// Braille spinner frames (must match [`prime_agent::pipeline_progress`]).
const SPINNER_CHARS: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

fn strip_ansi_progress(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut it = s.chars().peekable();
    while let Some(c) = it.next() {
        if c == '\r' {
            continue;
        }
        if c == '\x1b' && it.peek() == Some(&'[') {
            it.next();
            for ch in it.by_ref() {
                if ch.is_ascii() && (ch as u32) >= 0x40 && (ch as u32) <= 0x7e {
                    break;
                }
            }
            continue;
        }
        out.push(c);
    }
    out
}

fn normalize_pipeline_header_line(line: &str) -> String {
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() >= 3 && parts[0] == "pipeline" {
        return format!("pipeline {} <run_name>", parts[1]);
    }
    line.to_string()
}

fn normalize_spinner_and_secs_line(line: &str) -> String {
    let mut s = line.to_string();
    for sp in SPINNER_CHARS {
        s = s.replace(sp, "<spinner>");
    }
    if let Some(sp) = s.rfind(' ') {
        let tail = &s[sp + 1..];
        if let Some(num) = tail.strip_suffix('s')
            && !num.is_empty()
            && num.chars().all(|c| c.is_ascii_digit())
        {
            return format!("{} <secs>s", &s[..sp]);
        }
    }
    s
}

fn normalize_pipeline_run_stdout(raw: &str) -> String {
    let raw = strip_ansi_progress(raw);
    let mut out = String::new();
    let mut first = true;
    for line in raw.lines() {
        if !first {
            out.push('\n');
        }
        first = false;
        let line = if line.starts_with("prime-agent(") {
            "prime-agent(<version>)".to_string()
        } else if line.starts_with("effective:") {
            "<effective-config>".to_string()
        } else if line.starts_with("pipeline ") {
            normalize_pipeline_header_line(line)
        } else {
            normalize_spinner_and_secs_line(line)
        };
        out.push_str(&line);
    }
    out.push('\n');
    out
}

const PIPELINE_RUN_STDOUT_GOLDEN: &str = r#"prime-agent(<version>)
<effective-config>
pipeline demo-pipe <run_name>
step 1 stepone
  * running (no skill)
Step 0 / 1 Pipeline 0 / 1 <spinner> <secs>s
Step 1 / 1 Pipeline 1 / 1 <spinner> <secs>s
"#;

#[test]
fn pipelines_run_stdout_matches_golden_after_normalization() {
    let temp = TempDir::new().expect("temp");
    let data_dir = temp.path().join("data");
    let skills_dir = temp.path().join("skills");
    fs::create_dir_all(&skills_dir).expect("skills");
    write_dot_prime_agent_config_yolo(&temp, "m", "cursor-agent", true);
    write_pipeline(
        &data_dir,
        "demo-pipe",
        r#"{"steps":[{"id":1,"title":"stepone","prompt":"doprompt","skills":[]}]}"#,
    );
    let bin = temp.path().join("bin");
    fs::create_dir_all(&bin).expect("bin");
    let mock = bin.join("cursor-agent");
    fs::write(
        &mock,
        "#!/bin/sh\ncat >/dev/null\necho '{\"text\":\"out1\"}'\n",
    )
    .expect("mock");
    chmod_x(&mock);

    let mut cmd = pipelines_cmd(&temp, &data_dir, &skills_dir, &bin);
    cmd.args(["run", "demo-pipe", "--prompt", "userhi"]);
    let out = cmd.assert().success().get_output().stdout.clone();
    let got = normalize_pipeline_run_stdout(&String::from_utf8_lossy(&out));
    assert_eq!(got, PIPELINE_RUN_STDOUT_GOLDEN);
}

/// Two-step pipeline: while stage 1 runs, roadmap lists stage 2 as a following line (dim in TTY; plain in tests).
#[test]
fn pipelines_run_two_step_stdout_lists_pending_step_after_current_in_roadmap() {
    let temp = TempDir::new().expect("temp");
    let data_dir = temp.path().join("data");
    let skills_dir = temp.path().join("skills");
    fs::create_dir_all(&skills_dir).expect("skills");
    write_dot_prime_agent_config_yolo(&temp, "m", "cursor-agent", true);
    write_pipeline(
        &data_dir,
        "two-step",
        r#"{"steps":[{"id":1,"title":"alpha","prompt":"p1","skills":[]},{"id":2,"title":"beta","prompt":"p2","skills":[]}]}"#,
    );
    let bin = temp.path().join("bin");
    fs::create_dir_all(&bin).expect("bin");
    let mock = bin.join("cursor-agent");
    fs::write(
        &mock,
        "#!/bin/sh\ncat >/dev/null\necho '{\"text\":\"out1\"}'\n",
    )
    .expect("mock");
    chmod_x(&mock);

    let mut cmd = pipelines_cmd(&temp, &data_dir, &skills_dir, &bin);
    cmd.args(["run", "two-step", "--prompt", "userhi"]);
    let out = cmd.assert().success().get_output().stdout.clone();
    let got = normalize_pipeline_run_stdout(&String::from_utf8_lossy(&out));

    let pos_current = got.find("step 1 alpha").expect("roadmap should include step 1");
    let pos_pending = got.find("step 2 beta").expect("roadmap should include pending step 2");
    assert!(
        pos_current < pos_pending,
        "current step line should appear before pending step line: {got:?}"
    );
}
