//! `prime-agent serve` — HTTP UI and static file fallback from cwd.

use anyhow::{Context, Result};
use std::fs;
use std::net::SocketAddr;
use std::path::PathBuf;
use tokio::net::TcpListener;

use std::sync::Arc;

use crate::generation::new_registry_mutex;
use crate::idle_commit::{SkillActivity, spawn_idle_commit_task};
use crate::pipeline_store::PipelineStore;
use crate::skills_store::SkillsStore;
use crate::web::handlers::{AppState, broadcast_fs_changed, build_router};

pub fn run_blocking(data_dir: PathBuf, bind: String) -> Result<()> {
    let rt = tokio::runtime::Runtime::new().context("tokio runtime")?;
    rt.block_on(async { run(data_dir, bind).await })
}

async fn run(data_dir: PathBuf, bind: String) -> Result<()> {
    ensure_dot_prime_agent_config()?;
    fs::create_dir_all(&data_dir).with_context(|| format!("create '{}'", data_dir.display()))?;
    let skills_dir = data_dir.join("skills");
    fs::create_dir_all(&skills_dir)?;
    fs::create_dir_all(data_dir.join("pipelines"))?;
    let skills = SkillsStore::new(skills_dir);
    let pipelines = PipelineStore::new(&data_dir);
    let counter_path = data_dir.join("counter.json");

    let static_root = std::env::current_dir().context("current_dir for static files")?;

    let skill_activity = Arc::new(SkillActivity::new());
    let live_reload_enabled = crate::live_reload::live_reload_enabled_from_env();
    let live_tx = crate::live_reload::live_broadcast_channel();
    let fs_suppress = crate::live_reload::FsSuppress::default();
    let generations = new_registry_mutex(&skills, &pipelines)?;
    let state = AppState {
        data_dir: data_dir.clone(),
        skills,
        pipelines,
        counter_path,
        skill_activity: Arc::clone(&skill_activity),
        live_reload_enabled,
        live_tx,
        fs_suppress,
        generations,
    };
    if live_reload_enabled {
        let s = state.clone();
        let fs_suppress_for_watcher = s.fs_suppress.clone();
        crate::live_reload::spawn_fs_watcher(
            data_dir.clone(),
            &fs_suppress_for_watcher,
            move || {
                let _ = broadcast_fs_changed(&s);
            },
        );
    }

    spawn_idle_commit_task(data_dir.clone(), skill_activity);

    let app = build_router(state, static_root);

    let addr: SocketAddr = bind
        .parse()
        .with_context(|| format!("parse bind address {bind:?}"))?;
    let listener = TcpListener::bind(addr)
        .await
        .with_context(|| format!("bind {addr}"))?;
    let local = listener.local_addr().context("local_addr")?;
    println!("listening on http://{local}");
    axum::serve(listener, app).await.context("server")?;
    Ok(())
}

fn ensure_dot_prime_agent_config() -> Result<()> {
    let dir = PathBuf::from(".prime-agent");
    fs::create_dir_all(&dir).context("create .prime-agent")?;
    let path = dir.join("config.json");
    if !path.exists() {
        fs::write(
            &path,
            "{\n  \"model\": null,\n  \"clirunner\": null,\n  \"stdout_lines\": 3,\n  \"yolo\": true\n}\n",
        )
        .context("write .prime-agent/config.json")?;
    }
    Ok(())
}
