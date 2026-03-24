//! Live `cursor-agent` integration test. Not run by default (`#[ignore]`).
//!
//! ```text
//! PRIME_AGENT_LIVE_CURSOR=1 cargo test cursor_pipeline_live -- --ignored --nocapture
//! ```
//!
//! Requires `cursor-agent` on `PATH`, auth, and network.

use assert_cmd::cargo::cargo_bin_cmd;
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;
use uuid::Uuid;

const PIPELINE_NAME: &str = "live-e2e-words";

fn output_dir_for_pipeline(root: &Path, pipeline_name: &str) -> PathBuf {
    let base = root.join(".prime-agent").join("pipelines");
    for entry in fs::read_dir(&base).expect("read pipelines") {
        let p = entry.expect("entry").path();
        if !p.is_dir() || !p.join("meta.json").is_file() {
            continue;
        }
        let raw = fs::read_to_string(p.join("meta.json")).expect("meta");
        let v: Value = serde_json::from_str(&raw).expect("parse meta");
        if v.get("pipeline").and_then(|x| x.as_str()) == Some(pipeline_name) {
            return p;
        }
    }
    panic!("no run dir for pipeline '{pipeline_name}' under {base:?}");
}

fn write_skill(skills_root: &Path, name: &str, id: Uuid, body: &str) {
    let dir = skills_root.join(name);
    fs::create_dir_all(&dir).expect("skill dir");
    fs::write(dir.join("SKILL.md"), body).expect("SKILL.md");
    fs::write(dir.join(".prime-agent-skill-id"), format!("{id}\n")).expect("skill id");
}

#[test]
#[ignore = "live cursor-agent; run: PRIME_AGENT_LIVE_CURSOR=1 cargo test cursor_pipeline_live -- --ignored --nocapture"]
fn live_cursor_pipeline_three_skills_then_aggregate() {
    if std::env::var("PRIME_AGENT_LIVE_CURSOR").ok().as_deref() != Some("1") {
        panic!("set PRIME_AGENT_LIVE_CURSOR=1 to run this ignored test");
    }

    let temp = TempDir::new().expect("temp");
    let root = temp.path();
    let data_dir = root.join("data");
    let skills_dir = data_dir.join("skills");
    fs::create_dir_all(&skills_dir).expect("skills");

    let id_a = Uuid::parse_str("00000000-0000-4000-8000-00000000c0a1").expect("uuid");
    let id_b = Uuid::parse_str("00000000-0000-4000-8000-00000000c0b2").expect("uuid");
    let id_c = Uuid::parse_str("00000000-0000-4000-8000-00000000c0c3").expect("uuid");
    let id_d = Uuid::parse_str("00000000-0000-4000-8000-00000000c0d4").expect("uuid");
    write_skill(
        &skills_dir,
        "skill-a",
        id_a,
        "Respond with a single word, nothing else, just say 'alfa'\n",
    );
    write_skill(
        &skills_dir,
        "skill-b",
        id_b,
        "Respond with a single word, nothing else, just say 'bravo'\n",
    );
    write_skill(
        &skills_dir,
        "skill-c",
        id_c,
        "Respond with a single word, nothing else, just say 'charlie'\n",
    );
    write_skill(
        &skills_dir,
        "skill-d",
        id_d,
        "## Aggregate\n\n\
         The prompt includes prior stage outputs inside a `<Context>` block. Read the three single-word \
         outputs from stage 1 and respond with a JSON array of those words sorted \
         alphabetically, e.g. [\"alfa\",\"bravo\",\"charlie\"]. Nothing else.\n",
    );

    let pipeline_dir = data_dir.join("pipelines").join(PIPELINE_NAME);
    fs::create_dir_all(&pipeline_dir).expect("pipeline dir");
    let pipeline_body = format!(
        r#"{{
  "steps": [
    {{
      "id": 1,
      "title": "words",
      "prompt": "Execute each attached skill.",
      "skills": [
        {{"id":"{id_a}","alias":"skill-a"}},
        {{"id":"{id_b}","alias":"skill-b"}},
        {{"id":"{id_c}","alias":"skill-c"}}
      ]
    }},
    {{
      "id": 2,
      "title": "aggregate",
      "prompt": "Use skill-d to combine prior outputs.",
      "skills": [{{"id":"{id_d}","alias":"skill-d"}}]
    }}
  ]
}}
"#
    );
    fs::write(pipeline_dir.join("pipeline.json"), pipeline_body).expect("pipeline.json");

    let dot = root.join(".prime-agent");
    fs::create_dir_all(&dot).expect(".prime-agent");
    fs::write(
        dot.join("config.json"),
        r#"{
  "model": "composer-2-fast",
  "clirunner": "cursor-agent",
  "yolo": true
}
"#,
    )
    .expect("config.json");

    // Real `cursor-agent` from PATH (no mock).
    let path_var = std::env::var("PATH").unwrap_or_default();

    let mut cmd = cargo_bin_cmd!("prime-agent");
    cmd.current_dir(root)
        .env("PATH", &path_var)
        .env("PRIME_AGENT_NO_TUI", "1")
        .env("XDG_CONFIG_HOME", root.join("xdg_config"))
        .args([
            "--data-dir",
            data_dir.to_str().expect("utf8"),
            "run",
            PIPELINE_NAME,
            "--prompt",
            "please respond with 'hi'",
        ]);
    cmd.assert().success();

    let out_dir = output_dir_for_pipeline(root, PIPELINE_NAME);

    let mut task_files: Vec<PathBuf> = fs::read_dir(&out_dir)
        .expect("read out_dir")
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.file_name().and_then(|n| n.to_str()).is_some_and(|n| {
                let stem = n.strip_suffix(".json").unwrap_or(n);
                stem.contains('_')
                    && stem
                        .split_once('_')
                        .is_some_and(|(a, b)| a.parse::<u32>().is_ok() && b.parse::<u32>().is_ok())
            })
        })
        .collect();
    task_files.sort();
    let names: Vec<String> = task_files
        .iter()
        .filter_map(|p| p.file_name().and_then(|n| n.to_str().map(String::from)))
        .collect();
    assert_eq!(
        names,
        vec![
            "1_1.json".to_string(),
            "1_2.json".to_string(),
            "1_3.json".to_string(),
            "2_1.json".to_string(),
        ],
        "expected exactly four task JSON files, got {names:?}"
    );

    let w1 = parse_task(&out_dir.join("1_1.json"));
    let w2 = parse_task(&out_dir.join("1_2.json"));
    let w3 = parse_task(&out_dir.join("1_3.json"));
    let w4 = parse_task(&out_dir.join("2_1.json"));

    for (label, t) in [("1_1", &w1), ("1_2", &w2), ("1_3", &w3), ("2_1", &w4)] {
        assert!(
            !t.stderr.to_lowercase().contains("unknown option"),
            "{label}: stderr should not contain CLI unknown-option errors: {}",
            t.stderr
        );
        assert_eq!(t.code, 0, "{label}: code {}", t.code);
        assert!(t.error.is_none(), "{label}: error={:?}", t.error);
    }

    assert!(
        contains_ci(&w1.stdout, "alfa") || contains_ci(&w1.output, "alfa"),
        "1_1: expected alfa in stdout or output: {:?} {:?}",
        w1.stdout,
        w1.output
    );
    assert!(
        contains_ci(&w2.stdout, "bravo") || contains_ci(&w2.output, "bravo"),
        "1_2: expected bravo: {:?}",
        w2.stdout
    );
    assert!(
        contains_ci(&w3.stdout, "charlie") || contains_ci(&w3.output, "charlie"),
        "1_3: expected charlie: {:?}",
        w3.stdout
    );

    let p2 = w4.prompt.as_str();
    assert!(
        contains_ci(p2, "alfa") && contains_ci(p2, "bravo") && contains_ci(p2, "charlie"),
        "2_1 prompt must include prior stage evidence for all three words (A,B,C). prompt len {}",
        p2.len()
    );

    assert!(
        (contains_ci(&w4.stdout, "alfa") || contains_ci(&w4.output, "alfa"))
            && (contains_ci(&w4.stdout, "bravo") || contains_ci(&w4.output, "bravo"))
            && (contains_ci(&w4.stdout, "charlie") || contains_ci(&w4.output, "charlie")),
        "2_1: aggregate output should reference all three words: stdout={:?} output={:?}",
        w4.stdout,
        w4.output
    );
}

fn contains_ci(hay: &str, needle: &str) -> bool {
    hay.to_lowercase().contains(&needle.to_lowercase())
}

struct TaskJson {
    stdout: String,
    stderr: String,
    code: i32,
    error: Option<String>,
    output: String,
    prompt: String,
}

fn parse_task(path: &Path) -> TaskJson {
    let raw = fs::read_to_string(path).expect("read task json");
    let v: Value = serde_json::from_str(&raw).expect("parse task json");
    let stdout = v
        .get("stdout")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let stderr = v
        .get("stderr")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let code = v.get("code").and_then(Value::as_i64).unwrap_or(-1) as i32;
    let error = v.get("error").and_then(|e| e.as_str()).map(String::from);
    let output = v
        .get("output")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let prompt = v
        .get("prompt")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    TaskJson {
        stdout,
        stderr,
        code,
        error,
        output,
        prompt,
    }
}
