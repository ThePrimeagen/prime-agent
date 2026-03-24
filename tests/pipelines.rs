mod common;

use assert_cmd::cargo::cargo_bin_cmd;
use common::{
    chmod_x, lone_pipeline_run_dir, pipeline_artifact_dir_for, pipelines_cmd,
    stdout_after_version_line, write_dot_prime_agent_config, write_dot_prime_agent_config_yolo,
    write_pipeline, write_skill_with_id,
};
use predicates::str::contains;
use predicates::str::contains as contains_text;
use serde_json::json;
use std::fs;
use tempfile::TempDir;
use uuid::Uuid;

#[test]
fn pipelines_default_no_subcommand_lists_two_sorted() {
    let temp = TempDir::new().expect("temp");
    let data_dir = temp.path().join("data");
    let skills_dir = temp.path().join("skills");
    fs::create_dir_all(&skills_dir).expect("skills");
    write_pipeline(
        &data_dir,
        "beta",
        r#"{"steps":[{"id":1,"title":"a","prompt":"p","skills":[]}]}"#,
    );
    write_pipeline(
        &data_dir,
        "alpha",
        r#"{"steps":[{"id":1,"title":"a","prompt":"p","skills":[]}]}"#,
    );
    let bin = temp.path().join("bin");
    fs::create_dir_all(&bin).expect("bin");

    let mut cmd = pipelines_cmd(&temp, &data_dir, &skills_dir, &bin);
    let out = cmd.assert().success().get_output().stdout.clone();
    assert_eq!(stdout_after_version_line(&out), "alpha\n\nbeta\n");
}

#[test]
fn pipelines_default_no_subcommand_lists_one() {
    let temp = TempDir::new().expect("temp");
    let data_dir = temp.path().join("data");
    let skills_dir = temp.path().join("skills");
    fs::create_dir_all(&skills_dir).expect("skills");
    write_pipeline(
        &data_dir,
        "solo",
        r#"{"steps":[{"id":1,"title":"a","prompt":"p","skills":[]}]}"#,
    );
    let bin = temp.path().join("bin");
    fs::create_dir_all(&bin).expect("bin");

    let mut cmd = pipelines_cmd(&temp, &data_dir, &skills_dir, &bin);
    let out = cmd.assert().success().get_output().stdout.clone();
    assert_eq!(stdout_after_version_line(&out), "solo\n");
}

#[test]
fn pipelines_default_no_subcommand_empty_stderr() {
    let temp = TempDir::new().expect("temp");
    let data_dir = temp.path().join("data");
    let skills_dir = temp.path().join("skills");
    fs::create_dir_all(&skills_dir).expect("skills");
    fs::create_dir_all(&data_dir).expect("data");
    let bin = temp.path().join("bin");
    fs::create_dir_all(&bin).expect("bin");

    let mut cmd = pipelines_cmd(&temp, &data_dir, &skills_dir, &bin);
    let out = cmd.assert().success().get_output().clone();
    assert_eq!(stdout_after_version_line(&out.stdout), "");
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("No pipelines found."));
}

#[test]
fn pipelines_default_no_subcommand_ignores_incomplete_dir() {
    let temp = TempDir::new().expect("temp");
    let data_dir = temp.path().join("data");
    let skills_dir = temp.path().join("skills");
    fs::create_dir_all(&skills_dir).expect("skills");
    fs::create_dir_all(data_dir.join("pipelines/incomplete")).expect("incomplete");
    write_pipeline(
        &data_dir,
        "good",
        r#"{"steps":[{"id":1,"title":"a","prompt":"p","skills":[]}]}"#,
    );
    let bin = temp.path().join("bin");
    fs::create_dir_all(&bin).expect("bin");

    let mut cmd = pipelines_cmd(&temp, &data_dir, &skills_dir, &bin);
    let out = cmd.assert().success().get_output().stdout.clone();
    assert_eq!(stdout_after_version_line(&out), "good\n");
}

#[test]
fn pipelines_default_no_subcommand_respects_data_dir_flag() {
    let temp = TempDir::new().expect("temp");
    let right = temp.path().join("right");
    let skills_dir = temp.path().join("skills");
    fs::create_dir_all(&skills_dir).expect("skills");
    write_pipeline(
        &right,
        "only",
        r#"{"steps":[{"id":1,"title":"a","prompt":"p","skills":[]}]}"#,
    );
    let bin = temp.path().join("bin");
    fs::create_dir_all(&bin).expect("bin");

    let mut cmd = pipelines_cmd(&temp, &right, &skills_dir, &bin);
    let out = cmd.assert().success().get_output().stdout.clone();
    assert_eq!(stdout_after_version_line(&out), "only\n");
}

#[test]
fn pipelines_help_mentions_default_behavior() {
    let temp = TempDir::new().expect("temp");
    let data_dir = temp.path().join("data");
    let skills_dir = temp.path().join("skills");
    fs::create_dir_all(&skills_dir).expect("skills");
    let bin = temp.path().join("bin");
    fs::create_dir_all(&bin).expect("bin");

    let mut cmd = pipelines_cmd(&temp, &data_dir, &skills_dir, &bin);
    cmd.args(["--help"]);
    cmd.assert()
        .success()
        .stdout(contains_text("run"))
        .stdout(contains_text("pipeline"));
}

#[test]
fn pipelines_run_errors_when_neither_prompt_nor_file() {
    let temp = TempDir::new().expect("temp");
    let data_dir = temp.path().join("data");
    let skills_dir = temp.path().join("skills");
    fs::create_dir_all(&skills_dir).expect("skills");
    write_dot_prime_agent_config(&temp, "m", "cursor-agent");
    write_pipeline(
        &data_dir,
        "pl",
        r#"{"steps":[{"id":1,"title":"a","prompt":"p","skills":[]}]}"#,
    );
    let bin = temp.path().join("bin");
    fs::create_dir_all(&bin).expect("bin");
    let mock = bin.join("cursor-agent");
    fs::write(&mock, "#!/bin/sh\necho '{\"text\":\"x\"}'\n").expect("mock");
    chmod_x(&mock);

    let mut cmd = pipelines_cmd(&temp, &data_dir, &skills_dir, &bin);
    cmd.args(["run", "pl"]);
    cmd.assert().failure();
}

#[test]
fn pipelines_run_errors_when_both_prompt_and_file() {
    let temp = TempDir::new().expect("temp");
    let data_dir = temp.path().join("data");
    let skills_dir = temp.path().join("skills");
    fs::create_dir_all(&skills_dir).expect("skills");
    write_dot_prime_agent_config(&temp, "m", "cursor-agent");
    write_pipeline(
        &data_dir,
        "pl",
        r#"{"steps":[{"id":1,"title":"a","prompt":"p","skills":[]}]}"#,
    );
    let bin = temp.path().join("bin");
    fs::create_dir_all(&bin).expect("bin");
    let mock = bin.join("cursor-agent");
    fs::write(&mock, "#!/bin/sh\necho '{\"text\":\"x\"}'\n").expect("mock");
    chmod_x(&mock);
    let f = temp.path().join("prompt.txt");
    fs::write(&f, "hi").expect("f");

    let mut cmd = pipelines_cmd(&temp, &data_dir, &skills_dir, &bin);
    cmd.args(["run", "pl", "--prompt", "a", "--file"])
        .arg(&f);
    cmd.assert().failure();
}

#[test]
fn pipelines_run_errors_when_dot_prime_agent_config_missing() {
    let temp = TempDir::new().expect("temp");
    let data_dir = temp.path().join("data");
    let skills_dir = temp.path().join("skills");
    fs::create_dir_all(&skills_dir).expect("skills");
    write_pipeline(
        &data_dir,
        "pl",
        r#"{"steps":[{"id":1,"title":"a","prompt":"p","skills":[]}]}"#,
    );
    let bin = temp.path().join("bin");
    fs::create_dir_all(&bin).expect("bin");
    let mock = bin.join("cursor-agent");
    fs::write(&mock, "#!/bin/sh\necho '{\"text\":\"x\"}'\n").expect("mock");
    chmod_x(&mock);

    let mut cmd = pipelines_cmd(&temp, &data_dir, &skills_dir, &bin);
    cmd.args(["run", "pl", "--prompt", "x"]);
    cmd.assert().failure();
}

#[test]
fn pipelines_run_uses_global_config_json_data_dir_without_cli_data_dir() {
    let temp = TempDir::new().expect("temp");
    let data_dir = temp.path().join("from-global");
    let skills_dir = data_dir.join("skills");
    fs::create_dir_all(&skills_dir).expect("skills");
    write_dot_prime_agent_config(&temp, "m", "cursor-agent");

    let config_home = temp.path().join("config");
    fs::create_dir_all(config_home.join("prime-agent")).expect("cfg");
    let global_cfg = config_home.join("prime-agent/config.json");
    let cfg = json!({
        "model": "m",
        "clirunner": "cursor-agent",
        "data-dir": data_dir.to_string_lossy(),
    });
    fs::write(
        &global_cfg,
        format!("{}\n", serde_json::to_string_pretty(&cfg).unwrap()),
    )
    .expect("write global");

    write_pipeline(
        &data_dir,
        "pl",
        r#"{"steps":[{"id":1,"title":"a","prompt":"p","skills":[]}]}"#,
    );
    let bin = temp.path().join("bin");
    fs::create_dir_all(&bin).expect("bin");
    let mock = bin.join("cursor-agent");
    fs::write(
        &mock,
        "#!/bin/sh\ncat >/dev/null\necho '{\"text\":\"x\"}'\n",
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
        .args(["run", "pl", "--prompt", "x"]);
    cmd.assert().success();
}

#[test]
fn pipelines_run_errors_on_unsupported_clirunner() {
    let temp = TempDir::new().expect("temp");
    let data_dir = temp.path().join("data");
    let skills_dir = temp.path().join("skills");
    fs::create_dir_all(&skills_dir).expect("skills");
    write_dot_prime_agent_config(&temp, "m", "other-cli");
    write_pipeline(
        &data_dir,
        "pl",
        r#"{"steps":[{"id":1,"title":"a","prompt":"p","skills":[]}]}"#,
    );
    let bin = temp.path().join("bin");
    fs::create_dir_all(&bin).expect("bin");
    let mock = bin.join("cursor-agent");
    fs::write(&mock, "#!/bin/sh\necho '{\"text\":\"x\"}'\n").expect("mock");
    chmod_x(&mock);

    let mut cmd = pipelines_cmd(&temp, &data_dir, &skills_dir, &bin);
    cmd.args(["run", "pl", "--prompt", "x"]);
    cmd.assert()
        .failure()
        .stderr(contains("unsupported clirunner"));
}

#[test]
fn pipelines_run_debug_echoes_subprocess_streams_to_stderr() {
    let temp = TempDir::new().expect("temp");
    let data_dir = temp.path().join("data");
    let skills_dir = temp.path().join("skills");
    fs::create_dir_all(&skills_dir).expect("skills");
    write_dot_prime_agent_config(&temp, "m", "cursor-agent");
    write_pipeline(
        &data_dir,
        "dbg-pipe",
        r#"{"steps":[{"id":1,"title":"MyStepTitle","prompt":"p","skills":[]}]}"#,
    );
    let bin = temp.path().join("bin");
    fs::create_dir_all(&bin).expect("bin");
    let mock = bin.join("cursor-agent");
    fs::write(
        &mock,
        "#!/bin/sh\ncat >/dev/null\necho stderrline >&2\necho '{\"text\":\"out\"}'\n",
    )
    .expect("mock");
    chmod_x(&mock);

    let mut cmd = pipelines_cmd(&temp, &data_dir, &skills_dir, &bin);
    cmd.args([
        "--debug",
        "run",
        "dbg-pipe",
        "--prompt",
        "u",
    ]);
    let out = cmd.output().expect("run");
    assert!(out.status.success(), "stderr={}", String::from_utf8_lossy(&out.stderr));
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(
        err.contains("MyStepTitle(1 / 1):stdout::"),
        "expected stdout debug line: {err}"
    );
    assert!(
        err.contains("MyStepTitle(1 / 1):stderr::"),
        "expected stderr debug line: {err}"
    );
    assert!(err.contains("stderrline"));
    assert!(err.contains("{\"text\":\"out\"}"));
}

#[test]
fn pipelines_run_debug_shows_task_position_for_multi_skill_step() {
    let temp = TempDir::new().expect("temp");
    let data_dir = temp.path().join("data");
    let skills_dir = temp.path().join("skills");
    let id_a = Uuid::parse_str("00000000-0000-4000-8000-0000000000a1").expect("uuid a");
    let id_b = Uuid::parse_str("00000000-0000-4000-8000-0000000000b2").expect("uuid b");
    write_skill_with_id(&skills_dir, "skill-a", &id_a, "a\n");
    write_skill_with_id(&skills_dir, "skill-b", &id_b, "b\n");
    write_dot_prime_agent_config(&temp, "m", "cursor-agent");
    let pipe = format!(
        r#"{{"steps":[{{"id":1,"title":"DualStep","prompt":"p","skills":[{{"id":"{id_a}","alias":"skill-a"}},{{"id":"{id_b}","alias":"skill-b"}}]}}]}}"#
    );
    write_pipeline(&data_dir, "dbg-multi", &pipe);
    let bin = temp.path().join("bin");
    fs::create_dir_all(&bin).expect("bin");
    let mock = bin.join("cursor-agent");
    fs::write(
        &mock,
        "#!/bin/sh\ncat >/dev/null\necho stderrline >&2\necho '{\"text\":\"x\"}'\n",
    )
    .expect("mock");
    chmod_x(&mock);

    let mut cmd = pipelines_cmd(&temp, &data_dir, &skills_dir, &bin);
    cmd.args([
        "--debug",
        "run",
        "dbg-multi",
        "--prompt",
        "u",
    ]);
    let out = cmd.output().expect("run");
    assert!(out.status.success(), "stderr={}", String::from_utf8_lossy(&out.stderr));
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(
        err.contains("DualStep(1 / 2):"),
        "expected (1 / 2) debug prefix: {err}"
    );
    assert!(
        err.contains("DualStep(2 / 2):"),
        "expected (2 / 2) debug prefix: {err}"
    );
}

#[test]
fn pipelines_shorthand_pipeline_and_prompt_matches_run_output() {
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
    cmd.args(["--pipeline", "demo-pipe", "--prompt", "userhi"]);
    cmd.assert().success();

    let one = pipeline_artifact_dir_for(temp.path(), "demo-pipe").join("1_1.json");
    let raw = fs::read_to_string(&one).expect("1_1.json");
    let v: serde_json::Value = serde_json::from_str(&raw).expect("parse");
    assert_eq!(v["code"], 0);
    assert_eq!(v["output"], json!("out1"));
}

#[test]
fn pipelines_shorthand_pipeline_and_file_matches_run_output() {
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

    let pf = temp.path().join("prompt.txt");
    fs::write(&pf, "userhi").expect("pf");

    let mut cmd = pipelines_cmd(&temp, &data_dir, &skills_dir, &bin);
    cmd.args(["--pipeline", "demo-pipe", "--file"])
        .arg(&pf);
    cmd.assert().success();

    let one = pipeline_artifact_dir_for(temp.path(), "demo-pipe").join("1_1.json");
    let raw = fs::read_to_string(&one).expect("1_1.json");
    let v: serde_json::Value = serde_json::from_str(&raw).expect("parse");
    assert_eq!(v["code"], 0);
    assert_eq!(v["output"], json!("out1"));
}

#[test]
fn pipelines_shorthand_file_flag_before_pipeline_flag_matches_run_output() {
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

    let pf = temp.path().join("user_prompt.txt");
    fs::write(&pf, "from-file-order-test").expect("pf");

    let mut cmd = pipelines_cmd(&temp, &data_dir, &skills_dir, &bin);
    cmd.args([
        "--file",
        pf.to_str().expect("utf8"),
        "--pipeline",
        "demo-pipe",
    ]);
    cmd.assert().success();

    let one = pipeline_artifact_dir_for(temp.path(), "demo-pipe").join("1_1.json");
    let raw = fs::read_to_string(&one).expect("1_1.json");
    let v: serde_json::Value = serde_json::from_str(&raw).expect("parse");
    assert_eq!(v["code"], 0);
    assert_eq!(v["output"], json!("out1"));
}

#[test]
fn pipelines_shorthand_pipeline_and_file_reads_file_into_agent_stdin() {
    let temp = TempDir::new().expect("temp");
    let data_dir = temp.path().join("data");
    let skills_dir = temp.path().join("skills");
    fs::create_dir_all(&skills_dir).expect("skills");
    write_dot_prime_agent_config(&temp, "m", "cursor-agent");
    write_pipeline(
        &data_dir,
        "pl",
        r#"{"steps":[{"id":1,"title":"a","prompt":"p","skills":[]}]}"#,
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
    let pf = temp.path().join("unique-prompt-file.txt");
    fs::write(&pf, "unique-file-content-shorthand-abc").expect("pf");

    let mut cmd = pipelines_cmd(&temp, &data_dir, &skills_dir, &bin);
    cmd.args(["--pipeline", "pl", "--file"])
        .arg(&pf);
    cmd.assert().success();

    let s = fs::read_to_string(&stdin_log).expect("stdin");
    assert!(s.contains("unique-file-content-shorthand-abc"));
}

#[test]
fn pipelines_shorthand_pipeline_and_file_with_data_dir_cli_flag_only() {
    let temp = TempDir::new().expect("temp");
    let data_dir = temp.path().join("skill-issues");
    let skills_dir = temp.path().join("skills");
    fs::create_dir_all(&skills_dir).expect("skills");
    write_dot_prime_agent_config(&temp, "composer-2-fast", "cursor-agent");
    write_pipeline(
        &data_dir,
        "prime-executor",
        r#"{"steps":[{"id":1,"title":"stepone","prompt":"doprompt","skills":[]}]}"#,
    );
    let bin = temp.path().join("bin");
    fs::create_dir_all(&bin).expect("bin");
    let mock = bin.join("cursor-agent");
    fs::write(
        &mock,
        "#!/bin/sh\ncat >/dev/null\necho '{\"text\":\"cli-flag\"}'\n",
    )
    .expect("mock");
    chmod_x(&mock);

    let pf = temp.path().join("title");
    fs::write(&pf, "issue body").expect("title");

    let path_var = format!(
        "{}:{}",
        bin.display(),
        std::env::var("PATH").unwrap_or_default()
    );
    let mut cmd = cargo_bin_cmd!("prime-agent");
    cmd.current_dir(temp.path())
        .env("PATH", &path_var)
        .env("PRIME_AGENT_NO_TUI", "1")
        .env("XDG_CONFIG_HOME", temp.path().join("config"))
        .args([
            "--data-dir",
            data_dir.to_str().expect("utf8"),
            "--skills-dir",
            skills_dir.to_str().expect("utf8"),
            "--pipeline",
            "prime-executor",
            "--file",
            pf.to_str().expect("utf8"),
        ]);
    cmd.assert().success();

    let one = pipeline_artifact_dir_for(temp.path(), "prime-executor").join("1_1.json");
    let raw = fs::read_to_string(&one).expect("1_1.json");
    let v: serde_json::Value = serde_json::from_str(&raw).expect("parse");
    assert_eq!(v["code"], 0);
    assert_eq!(v["output"], json!("cli-flag"));
}

#[test]
fn pipelines_shorthand_relative_title_file_in_cwd() {
    let temp = TempDir::new().expect("temp");
    let data_dir = temp.path().join("data");
    let skills_dir = temp.path().join("skills");
    fs::create_dir_all(&skills_dir).expect("skills");
    write_dot_prime_agent_config(&temp, "m", "cursor-agent");
    write_pipeline(
        &data_dir,
        "demo-pipe",
        r#"{"steps":[{"id":1,"title":"a","prompt":"p","skills":[]}]}"#,
    );
    fs::write(temp.path().join("title"), "relative-title-content\n").expect("title");
    let bin = temp.path().join("bin");
    fs::create_dir_all(&bin).expect("bin");
    let mock = bin.join("cursor-agent");
    fs::write(
        &mock,
        "#!/bin/sh\ncat >/dev/null\necho '{\"text\":\"rel\"}'\n",
    )
    .expect("mock");
    chmod_x(&mock);

    let mut cmd = pipelines_cmd(&temp, &data_dir, &skills_dir, &bin);
    cmd.args(["--pipeline", "demo-pipe", "--file", "title"]);
    cmd.assert().success();

    let one = pipeline_artifact_dir_for(temp.path(), "demo-pipe").join("1_1.json");
    let raw = fs::read_to_string(&one).expect("1_1.json");
    let v: serde_json::Value = serde_json::from_str(&raw).expect("parse");
    assert_eq!(v["code"], 0);
    assert_eq!(v["output"], json!("rel"));
}

#[test]
fn pipelines_shorthand_pipeline_without_prompt_or_file_errors() {
    let temp = TempDir::new().expect("temp");
    let data_dir = temp.path().join("data");
    let skills_dir = temp.path().join("skills");
    fs::create_dir_all(&skills_dir).expect("skills");
    write_dot_prime_agent_config(&temp, "m", "cursor-agent");
    write_pipeline(
        &data_dir,
        "pl",
        r#"{"steps":[{"id":1,"title":"a","prompt":"p","skills":[]}]}"#,
    );
    let bin = temp.path().join("bin");
    fs::create_dir_all(&bin).expect("bin");

    let mut cmd = pipelines_cmd(&temp, &data_dir, &skills_dir, &bin);
    cmd.args(["--pipeline", "pl"]);
    cmd.assert().failure().stderr(contains(
        "with --pipeline, provide exactly one of --prompt or --file",
    ));
}

#[test]
fn pipelines_shorthand_pipeline_both_prompt_and_file_errors() {
    let temp = TempDir::new().expect("temp");
    let data_dir = temp.path().join("data");
    let skills_dir = temp.path().join("skills");
    fs::create_dir_all(&skills_dir).expect("skills");
    write_dot_prime_agent_config(&temp, "m", "cursor-agent");
    write_pipeline(
        &data_dir,
        "pl",
        r#"{"steps":[{"id":1,"title":"a","prompt":"p","skills":[]}]}"#,
    );
    let bin = temp.path().join("bin");
    fs::create_dir_all(&bin).expect("bin");
    let f = temp.path().join("f.txt");
    fs::write(&f, "a").expect("f");

    let mut cmd = pipelines_cmd(&temp, &data_dir, &skills_dir, &bin);
    cmd.args([
        "--pipeline",
        "pl",
        "--prompt",
        "x",
        "--file",
        f.to_str().expect("utf8"),
    ]);
    cmd.assert()
        .failure()
        .stderr(contains("use only one of --prompt or --file"));
}

#[test]
fn pipelines_file_without_pipeline_non_tty_lists_does_not_run() {
    let temp = TempDir::new().expect("temp");
    let data_dir = temp.path().join("data");
    let skills_dir = temp.path().join("skills");
    fs::create_dir_all(&skills_dir).expect("skills");
    write_pipeline(
        &data_dir,
        "listed",
        r#"{"steps":[{"id":1,"title":"a","prompt":"p","skills":[]}]}"#,
    );
    write_dot_prime_agent_config(&temp, "m", "cursor-agent");
    let bin = temp.path().join("bin");
    fs::create_dir_all(&bin).expect("bin");
    let mock = bin.join("cursor-agent");
    fs::write(&mock, "#!/bin/sh\necho '{\"text\":\"x\"}'\n").expect("mock");
    chmod_x(&mock);

    let pf = temp.path().join("ignored.txt");
    fs::write(&pf, "should-not-run-pipeline").expect("pf");

    let mut cmd = pipelines_cmd(&temp, &data_dir, &skills_dir, &bin);
    cmd.args(["--file"]).arg(&pf);
    let out = cmd.assert().success().get_output().stdout.clone();
    assert_eq!(
        stdout_after_version_line(&out),
        "listed\n",
        "--file without --pipeline must not invoke pipeline run; stdout should list names only"
    );
    assert!(
        !temp.path().join(".prime-agent/pipelines").exists(),
        "no run artifacts when only --file is passed without --pipeline"
    );
}

#[test]
fn pipelines_run_rejects_combined_pipeline_flag() {
    let temp = TempDir::new().expect("temp");
    let data_dir = temp.path().join("data");
    let skills_dir = temp.path().join("skills");
    fs::create_dir_all(&skills_dir).expect("skills");
    write_dot_prime_agent_config(&temp, "m", "cursor-agent");
    write_pipeline(
        &data_dir,
        "pl",
        r#"{"steps":[{"id":1,"title":"a","prompt":"p","skills":[]}]}"#,
    );
    let bin = temp.path().join("bin");
    fs::create_dir_all(&bin).expect("bin");
    let mock = bin.join("cursor-agent");
    fs::write(
        &mock,
        "#!/bin/sh\ncat >/dev/null\necho '{\"text\":\"x\"}'\n",
    )
    .expect("mock");
    chmod_x(&mock);

    let mut cmd = pipelines_cmd(&temp, &data_dir, &skills_dir, &bin);
    cmd.args([
        "--pipeline",
        "pl",
        "run",
        "pl",
        "--prompt",
        "x",
    ]);
    cmd.assert()
        .failure()
        .stderr(contains("do not combine `run` with --pipeline"));
}

#[test]
fn pipelines_run_prints_kebab_run_name_first_line() {
    let temp = TempDir::new().expect("temp");
    let data_dir = temp.path().join("data");
    let skills_dir = temp.path().join("skills");
    fs::create_dir_all(&skills_dir).expect("skills");
    write_dot_prime_agent_config(&temp, "m", "cursor-agent");
    write_pipeline(
        &data_dir,
        "pl",
        r#"{"steps":[{"id":1,"title":"a","prompt":"p","skills":[]}]}"#,
    );
    let bin = temp.path().join("bin");
    fs::create_dir_all(&bin).expect("bin");
    let mock = bin.join("cursor-agent");
    fs::write(
        &mock,
        "#!/bin/sh\ncat >/dev/null\necho '{\"text\":\"x\"}'\n",
    )
    .expect("mock");
    chmod_x(&mock);

    let mut cmd = pipelines_cmd(&temp, &data_dir, &skills_dir, &bin);
    cmd.args(["run", "pl", "--prompt", "u"]);
    let out = cmd.assert().success().get_output().stdout.clone();
    let text = String::from_utf8_lossy(&out);
    let first = text
        .lines()
        .find(|l| !l.contains("prime-agent("))
        .expect("line");
    let parts: Vec<&str> = first.split_whitespace().collect();
    assert!(
        parts.len() >= 3 && parts[0] == "pipeline" && parts[1] == "pl",
        "expected `pipeline <kebab> <run name …>`, got: {first:?}"
    );
    let run_name = parts[2];
    assert!(
        run_name.contains('-')
            && run_name
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'),
        "expected lowercase kebab run name, got: {run_name:?}"
    );
}

#[test]
fn pipelines_run_passes_model_and_force_to_subprocess() {
    let temp = TempDir::new().expect("temp");
    let data_dir = temp.path().join("data");
    let skills_dir = temp.path().join("skills");
    fs::create_dir_all(&skills_dir).expect("skills");
    write_dot_prime_agent_config_yolo(&temp, "composer-2-fast", "cursor-agent", true);
    write_pipeline(
        &data_dir,
        "pl",
        r#"{"steps":[{"id":1,"title":"a","prompt":"p","skills":[]}]}"#,
    );
    let bin = temp.path().join("bin");
    fs::create_dir_all(&bin).expect("bin");
    let argv_log = temp.path().join("argv.log");
    let mock = bin.join("cursor-agent");
    let body = format!(
        "#!/bin/sh\necho \"$@\" >> \"{}\"\ncat >/dev/null\necho '{{\"text\":\"x\"}}'\n",
        argv_log.display()
    );
    fs::write(&mock, body).expect("mock");
    chmod_x(&mock);

    let mut cmd = pipelines_cmd(&temp, &data_dir, &skills_dir, &bin);
    cmd.args(["run", "pl", "--prompt", "u"]);
    cmd.assert().success();

    let logged = fs::read_to_string(&argv_log).expect("log");
    assert!(logged.contains("--model"));
    assert!(logged.contains("composer-2-fast"));
    assert!(logged.contains("--force"));
    assert!(!logged.contains("--trust"));
    assert!(!logged.contains("--yolo"));
}

#[test]
fn pipelines_run_omits_force_when_yolo_false() {
    let temp = TempDir::new().expect("temp");
    let data_dir = temp.path().join("data");
    let skills_dir = temp.path().join("skills");
    fs::create_dir_all(&skills_dir).expect("skills");
    write_dot_prime_agent_config_yolo(&temp, "m", "cursor-agent", false);
    write_pipeline(
        &data_dir,
        "pl",
        r#"{"steps":[{"id":1,"title":"a","prompt":"p","skills":[]}]}"#,
    );
    let bin = temp.path().join("bin");
    fs::create_dir_all(&bin).expect("bin");
    let argv_log = temp.path().join("argv.log");
    let mock = bin.join("cursor-agent");
    let body = format!(
        "#!/bin/sh\necho \"$@\" >> \"{}\"\ncat >/dev/null\necho '{{\"text\":\"x\"}}'\n",
        argv_log.display()
    );
    fs::write(&mock, body).expect("mock");
    chmod_x(&mock);

    let mut cmd = pipelines_cmd(&temp, &data_dir, &skills_dir, &bin);
    cmd.args(["run", "pl", "--prompt", "u"]);
    cmd.assert().success();

    let logged = fs::read_to_string(&argv_log).expect("log");
    assert!(!logged.contains("--force"));
}

#[test]
fn pipelines_run_includes_force_when_yolo_key_omitted() {
    let temp = TempDir::new().expect("temp");
    let data_dir = temp.path().join("data");
    let skills_dir = temp.path().join("skills");
    fs::create_dir_all(&skills_dir).expect("skills");
    let d = temp.path().join(".prime-agent");
    fs::create_dir_all(&d).expect("dot dir");
    fs::write(
        d.join("config.json"),
        "{\"model\":\"m\",\"clirunner\":\"cursor-agent\"}\n",
    )
    .expect("dot config");
    write_pipeline(
        &data_dir,
        "pl",
        r#"{"steps":[{"id":1,"title":"a","prompt":"p","skills":[]}]}"#,
    );
    let bin = temp.path().join("bin");
    fs::create_dir_all(&bin).expect("bin");
    let argv_log = temp.path().join("argv.log");
    let mock = bin.join("cursor-agent");
    let body = format!(
        "#!/bin/sh\necho \"$@\" >> \"{}\"\ncat >/dev/null\necho '{{\"text\":\"x\"}}'\n",
        argv_log.display()
    );
    fs::write(&mock, body).expect("mock");
    chmod_x(&mock);

    let mut cmd = pipelines_cmd(&temp, &data_dir, &skills_dir, &bin);
    cmd.args(["run", "pl", "--prompt", "u"]);
    cmd.assert().success();

    let logged = fs::read_to_string(&argv_log).expect("log");
    assert!(logged.contains("--force"));
}

#[test]
fn pipelines_run_reads_prompt_from_file() {
    let temp = TempDir::new().expect("temp");
    let data_dir = temp.path().join("data");
    let skills_dir = temp.path().join("skills");
    fs::create_dir_all(&skills_dir).expect("skills");
    write_dot_prime_agent_config(&temp, "m", "cursor-agent");
    write_pipeline(
        &data_dir,
        "pl",
        r#"{"steps":[{"id":1,"title":"a","prompt":"p","skills":[]}]}"#,
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
    let pf = temp.path().join("user.txt");
    fs::write(&pf, "unique-file-content-xyz").expect("pf");

    let mut cmd = pipelines_cmd(&temp, &data_dir, &skills_dir, &bin);
    cmd.args(["run", "pl", "--file"]).arg(&pf);
    cmd.assert().success();

    let s = fs::read_to_string(&stdin_log).expect("stdin");
    assert!(s.contains("unique-file-content-xyz"));
}

#[test]
fn pipelines_run_stage_json_includes_stdout_stderr_and_error_on_failure() {
    let temp = TempDir::new().expect("temp");
    let data_dir = temp.path().join("data");
    let skills_dir = temp.path().join("skills");
    fs::create_dir_all(&skills_dir).expect("skills");
    write_dot_prime_agent_config(&temp, "m", "cursor-agent");
    write_pipeline(
        &data_dir,
        "stage-json-fail",
        r#"{"steps":[{"id":1,"title":"a","prompt":"p","skills":[]}]}"#,
    );
    let bin = temp.path().join("bin");
    fs::create_dir_all(&bin).expect("bin");
    let mock = bin.join("cursor-agent");
    fs::write(&mock, "#!/bin/sh\necho boom >&2\nexit 7\n").expect("mock");
    chmod_x(&mock);

    let mut cmd = pipelines_cmd(&temp, &data_dir, &skills_dir, &bin);
    cmd.args(["run", "stage-json-fail", "--prompt", "u"]);
    cmd.assert().failure();

    let raw = fs::read_to_string(lone_pipeline_run_dir(temp.path()).join("1_1.json"))
        .expect("1_1.json");
    let v: serde_json::Value = serde_json::from_str(&raw).expect("parse");
    assert!(v.get("error").is_some());
    assert!(v["stdout"].as_str().is_some());
    assert!(
        v["stderr"].as_str().is_some_and(|s| s.contains("boom")),
        "unexpected task json stderr; raw={raw:?}"
    );
    assert_eq!(v["code"].as_i64(), Some(7));
}

#[test]
fn pipelines_run_stage1_failure_does_not_write_stage2_task_json() {
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
    let mock = bin.join("cursor-agent");
    fs::write(&mock, "#!/bin/sh\ncat >/dev/null\necho boom >&2\nexit 7\n").expect("mock");
    chmod_x(&mock);

    let mut cmd = pipelines_cmd(&temp, &data_dir, &skills_dir, &bin);
    cmd.args(["run", "pl", "--prompt", "u"]);
    cmd.assert().failure();

    let d = pipeline_artifact_dir_for(temp.path(), "pl");
    assert!(
        d.join("1_1.json").is_file(),
        "stage 1 task should be written"
    );
    assert!(
        !d.join("2_1.json").exists(),
        "stage 2 must not run after stage 1 failure"
    );
}

#[test]
fn skill_rename_updates_alias_in_two_pipelines_after_list() {
    let temp = TempDir::new().expect("temp");
    let data_dir = temp.path().join("data");
    let skills_dir = temp.path().join("skills");
    let sid = Uuid::parse_str("00000000-0000-4000-8000-00000000ab12").expect("uuid");
    write_skill_with_id(&skills_dir, "skill-a", &sid, "x\n");
    let step = format!(
        r#"{{"id":1,"title":"s","prompt":"p","skills":[{{"id":"{sid}","alias":"skill-a"}}]}}"#
    );
    write_pipeline(
        &data_dir,
        "pipe-a",
        &format!(r#"{{"steps":[{step}]}}"#),
    );
    write_pipeline(
        &data_dir,
        "pipe-b",
        &format!(r#"{{"steps":[{step}]}}"#),
    );
    fs::rename(skills_dir.join("skill-a"), skills_dir.join("zed")).expect("rename skill");

    let bin = temp.path().join("bin");
    fs::create_dir_all(&bin).expect("bin");
    let mut cmd = pipelines_cmd(&temp, &data_dir, &skills_dir, &bin);
    cmd.assert().success();

    for pl in ["pipe-a", "pipe-b"] {
        let raw = fs::read_to_string(
            data_dir
                .join("pipelines")
                .join(pl)
                .join("pipeline.json"),
        )
        .expect("read pipeline");
        let v: serde_json::Value = serde_json::from_str(&raw).expect("parse");
        assert_eq!(
            v["steps"][0]["skills"][0]["alias"].as_str(),
            Some("zed"),
            "pipeline {pl} should rewrite alias after skill dir rename"
        );
    }
}

#[test]
fn pipelines_run_reports_broken_pipeline_with_step_and_alias() {
    let temp = TempDir::new().expect("temp");
    let data_dir = temp.path().join("data");
    let skills_dir = temp.path().join("skills");
    fs::create_dir_all(&skills_dir).expect("skills");
    write_dot_prime_agent_config(&temp, "m", "cursor-agent");
    let missing = Uuid::nil();
    write_pipeline(
        &data_dir,
        "broken-pipe",
        &format!(
            r#"{{"steps":[{{"id":7,"title":"bad-step","prompt":"p","skills":[{{"id":"{missing}","alias":"gone-skill"}}]}}]}}"#
        ),
    );
    let bin = temp.path().join("bin");
    fs::create_dir_all(&bin).expect("bin");
    let mock = bin.join("cursor-agent");
    fs::write(&mock, "#!/bin/sh\necho x\n").expect("mock");
    chmod_x(&mock);

    let mut cmd = pipelines_cmd(&temp, &data_dir, &skills_dir, &bin);
    cmd.args(["run", "broken-pipe", "--prompt", "u"]);
    let out = cmd.output().expect("run");
    assert!(!out.status.success());
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("broken-pipe"), "{err}");
    assert!(err.contains("bad-step") || err.contains('7'), "{err}");
    assert!(err.contains("gone-skill"), "{err}");
    assert!(err.contains(&missing.to_string()), "{err}");
}

#[test]
fn pipelines_default_non_tty_marks_broken_pipeline() {
    let temp = TempDir::new().expect("temp");
    let data_dir = temp.path().join("data");
    let skills_dir = temp.path().join("skills");
    fs::create_dir_all(&skills_dir).expect("skills");
    let missing = Uuid::nil();
    write_pipeline(
        &data_dir,
        "broken-pipe",
        &format!(
            r#"{{"steps":[{{"id":1,"title":"s","prompt":"p","skills":[{{"id":"{missing}","alias":"orphan"}}]}}]}}"#
        ),
    );
    let bin = temp.path().join("bin");
    fs::create_dir_all(&bin).expect("bin");

    let mut cmd = pipelines_cmd(&temp, &data_dir, &skills_dir, &bin);
    let out = cmd.assert().success().get_output().stdout.clone();
    let text = stdout_after_version_line(&out);
    assert!(
        text.contains("broken-pipe !"),
        "expected broken suffix in list, got {text:?}"
    );
}

#[test]
fn pipelines_duplicate_skill_ids_on_disk_fail_run() {
    let temp = TempDir::new().expect("temp");
    let data_dir = temp.path().join("data");
    let skills_dir = temp.path().join("skills");
    let sid = Uuid::parse_str("00000000-0000-4000-8000-00000000d001").expect("uuid");
    write_skill_with_id(&skills_dir, "dup-a", &sid, "a\n");
    write_skill_with_id(&skills_dir, "dup-b", &sid, "b\n");
    write_dot_prime_agent_config(&temp, "m", "cursor-agent");
    write_pipeline(
        &data_dir,
        "pl",
        &format!(
            r#"{{"steps":[{{"id":1,"title":"t","prompt":"p","skills":[{{"id":"{sid}","alias":"dup-a"}}]}}]}}"#
        ),
    );
    let bin = temp.path().join("bin");
    fs::create_dir_all(&bin).expect("bin");
    let mock = bin.join("cursor-agent");
    fs::write(&mock, "#!/bin/sh\necho x\n").expect("mock");
    chmod_x(&mock);

    let mut cmd = pipelines_cmd(&temp, &data_dir, &skills_dir, &bin);
    cmd.args(["run", "pl", "--prompt", "u"]);
    let out = cmd.output().expect("run");
    assert!(!out.status.success());
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("duplicate skill id"), "{err}");
}
