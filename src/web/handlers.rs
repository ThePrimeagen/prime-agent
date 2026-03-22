//! HTTP handlers + WebSocket mutations.

use anyhow::{anyhow, Result};
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    http::StatusCode,
    response::{Html, IntoResponse, Response},
    routing::{any, get},
    Router,
};
use serde_json::json;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::broadcast;
use tower_http::services::ServeDir;

use crate::counter;
use crate::generation::GenerationRegistry;
use crate::idle_commit::SkillActivity;
use crate::live_reload::FsSuppress;
use crate::pipeline_store::PipelineStore;
use crate::skills_store::SkillsStore;
use crate::web::render::{
    join_skill_names, pipeline_vm, render_page, skill_vm, step_skill_vm, step_vm, PageInput,
    PipelineStepVm, PipelineVm, SkillVm,
};
use crate::web::ws_protocol::{AckMsg, ClientOp, UiBroadcast};

type PageContextVms = (
    Vec<SkillVm>,
    Vec<PipelineVm>,
    Vec<PipelineStepVm>,
    Option<SkillVm>,
    Option<PipelineVm>,
);

#[derive(Clone)]
pub struct AppState {
    pub data_dir: PathBuf,
    pub skills: SkillsStore,
    pub pipelines: PipelineStore,
    pub counter_path: PathBuf,
    pub skill_activity: Arc<SkillActivity>,
    pub live_reload_enabled: bool,
    pub live_tx: broadcast::Sender<String>,
    pub fs_suppress: FsSuppress,
    pub generations: Arc<Mutex<GenerationRegistry>>,
}

fn not_found() -> Response {
    StatusCode::NOT_FOUND.into_response()
}

fn internal(e: &anyhow::Error) -> Response {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        format!("internal error: {e:#}"),
    )
        .into_response()
}

/// Gather VM state for rendering (shared by full page + WS broadcast).
fn page_context(
    state: &AppState,
    selected_skill_name: Option<&str>,
    selected_pipeline_name: Option<&str>,
) -> Result<PageContextVms> {
    let skill_names = state.skills.list_skill_names()?;
    let mut skills: Vec<SkillVm> = Vec::new();
    for name in &skill_names {
        let prompt = state.skills.load_skill(name)?;
        skills.push(skill_vm(name, &prompt));
    }

    let selected_skill = if let Some(name) = selected_skill_name {
        if state.skills.skill_exists(name) {
            let prompt = state.skills.load_skill(name)?;
            Some(skill_vm(name, &prompt))
        } else {
            return Err(anyhow!("not found"));
        }
    } else {
        None
    };

    let pipeline_names = state.pipelines.list_pipeline_names()?;
    let pipelines: Vec<PipelineVm> = pipeline_names.iter().map(|n| pipeline_vm(n)).collect();

    let selected_pipeline = if let Some(name) = selected_pipeline_name {
        state.pipelines.get_pipeline_meta(name).ok().map(|m| pipeline_vm(&m.name))
    } else {
        None
    };

    let mut pipeline_steps: Vec<PipelineStepVm> = Vec::new();
    if let Some(name) = selected_pipeline_name {
        let steps = state.pipelines.list_steps(name)?;
        for s in steps {
            let summary = join_skill_names(&s.skills);
            let sks: Vec<_> = s.skills.iter().map(|x| step_skill_vm(&x.name)).collect();
            pipeline_steps.push(step_vm(
                s.id,
                &s.title,
                &s.prompt,
                s.skill_count,
                sks,
                summary,
            ));
        }
    }

    Ok((
        skills,
        pipelines,
        pipeline_steps,
        selected_skill,
        selected_pipeline,
    ))
}

#[allow(clippy::too_many_arguments)]
fn page_input<'a>(
    active: &'a str,
    skills: &'a [SkillVm],
    pipelines: &'a [PipelineVm],
    pipeline_steps: &'a [PipelineStepVm],
    selected_skill: Option<&'a SkillVm>,
    selected_pipeline: Option<&'a PipelineVm>,
    live_reload: bool,
    generations_json: &'a str,
) -> PageInput<'a> {
    PageInput {
        active_section: active,
        skills,
        selected_skill,
        pipelines,
        selected_pipeline,
        pipeline_steps,
        live_reload,
        generations_json,
    }
}

fn push_url(active: &str, sel_skill: Option<&str>, sel_pipe: Option<&str>) -> String {
    match active {
        "skills" => sel_skill.map_or_else(
            || "/skills".to_string(),
            |s| format!("/skills/{}", urlencoding::encode(s)),
        ),
        "pipelines" => sel_pipe.map_or_else(
            || "/pipelines".to_string(),
            |p| format!("/pipelines/{}", urlencoding::encode(p)),
        ),
        _ => "/".to_string(),
    }
}

fn broadcast_ui(
    state: &AppState,
    active: &str,
    sel_skill: Option<&str>,
    sel_pipe: Option<&str>,
) -> Result<()> {
    let (skills, pipelines, pipeline_steps, selected_skill, selected_pipeline) =
        page_context(state, sel_skill, sel_pipe)?;
    let generations = state
        .generations
        .lock()
        .expect("generations lock")
        .snapshot();
    let b = UiBroadcast {
        r#type: "ui",
        active_section: active.to_string(),
        push_url: Some(push_url(active, sel_skill, sel_pipe)),
        skills: skills.clone(),
        pipelines: pipelines.clone(),
        pipeline_steps: pipeline_steps.clone(),
        selected_skill: selected_skill.clone(),
        selected_pipeline: selected_pipeline.clone(),
        live_reload: state.live_reload_enabled,
        generations,
    };
    let json = serde_json::to_string(&b).map_err(|e| anyhow!("{e}"))?;
    let _ = state.live_tx.send(json);
    Ok(())
}

fn suppress_skill_file(state: &AppState, name: &str) {
    state
        .fs_suppress
        .mark_path(&state.skills.skill_path(name), Duration::from_millis(900));
}

fn suppress_pipeline_file(state: &AppState, name: &str) {
    let path = state.data_dir.join("pipelines").join(name).join("pipeline.json");
    state.fs_suppress.mark_path(&path, Duration::from_millis(900));
}

fn suppress_skill_dir(state: &AppState, name: &str) {
    state
        .fs_suppress
        .mark_path(&state.skills.root().join(name), Duration::from_millis(900));
}

fn ack_ok(id: &str, location: Option<String>) -> String {
    serde_json::to_string(&AckMsg {
        id: id.to_string(),
        ok: true,
        location,
        error: None,
    })
    .expect("ack json")
}

fn ack_err(id: &str, msg: &str) -> String {
    serde_json::to_string(&AckMsg {
        id: id.to_string(),
        ok: false,
        location: None,
        error: Some(msg.to_string()),
    })
    .expect("ack json")
}

fn record_pipeline_mutation(state: &AppState, pipeline: &str) -> Result<()> {
    let path = state.pipelines.pipeline_json_path(pipeline);
    state
        .generations
        .lock()
        .expect("generations lock")
        .record_pipeline_write_from_path(pipeline, &path)?;
    Ok(())
}

fn record_pipelines_after_skill_reference_changes(state: &AppState) -> Result<()> {
    let mut g = state.generations.lock().expect("generations lock");
    for name in state.pipelines.list_pipeline_names()? {
        let raw = std::fs::read_to_string(state.pipelines.pipeline_json_path(&name))?;
        g.reconcile_pipeline_file_content(&name, &raw);
    }
    Ok(())
}

/// After filesystem debounce: reconcile disk with generation hashes and broadcast `fs_changed`
/// only if reconciliation actually changed generations (avoids noisy duplicate `fs_changed` when
/// the watcher fires repeatedly while disk content is unchanged).
pub fn broadcast_fs_changed(state: &AppState) -> Result<()> {
    let mut g = state.generations.lock().expect("generations lock");
    let changed = g.reconcile_from_disk(&state.skills, &state.pipelines)?;
    let snapshot = g.snapshot();
    drop(g);
    if !changed {
        return Ok(());
    }
    let msg = json!({
        "type": "fs_changed",
        "generations": snapshot,
    });
    let _ = state.live_tx.send(msg.to_string());
    Ok(())
}

#[allow(clippy::too_many_lines)]
fn handle_client_op(state: &AppState, op: ClientOp) -> String {
    match op {
        ClientOp::CreateSkill { id, name, prompt } => {
            let name = name.trim().to_string();
            let prompt = prompt.trim().to_string();
            if prompt.is_empty() {
                return ack_err(&id, "prompt is required");
            }
            if let Err(e) = SkillsStore::validate_write_name(&name) {
                return ack_err(&id, &e.to_string());
            }
            if state.skills.skill_exists(&name) {
                return ack_err(&id, "skill already exists");
            }
            suppress_skill_file(state, &name);
            if let Err(e) = state.skills.save_skill(&name, &prompt) {
                return ack_err(&id, &format!("{e:#}"));
            }
            state.skill_activity.mark_mutation();
            state
                .generations
                .lock()
                .expect("generations lock")
                .record_skill_created(&name, &prompt);
            if broadcast_ui(state, "skills", Some(&name), None).is_err() {
                return ack_err(&id, "broadcast failed");
            }
            let loc = format!("/skills/{}", urlencoding::encode(&name));
            ack_ok(&id, Some(loc))
        }
        ClientOp::UpdateSkill {
            id,
            old_name,
            name,
            prompt,
        } => {
            let new_name = name.trim().to_string();
            let prompt = prompt.trim().to_string();
            if prompt.is_empty() {
                return ack_err(&id, "prompt is required");
            }
            if let Err(e) = SkillsStore::validate_write_name(&new_name) {
                return ack_err(&id, &e.to_string());
            }
            if !state.skills.skill_exists(&old_name) {
                return ack_err(&id, "not found");
            }
            if old_name != new_name {
                if state.skills.skill_exists(&new_name) {
                    return ack_err(&id, "skill already exists");
                }
                suppress_skill_dir(state, &old_name);
                suppress_skill_dir(state, &new_name);
                if let Err(e) = state.skills.rename_skill_directory(&old_name, &new_name) {
                    return ack_err(&id, &format!("{e:#}"));
                }
                if let Err(e) = state
                    .pipelines
                    .rename_skill_reference(&old_name, &new_name)
                {
                    return ack_err(&id, &format!("{e:#}"));
                }
            }
            suppress_skill_file(state, &new_name);
            if let Err(e) = state.skills.save_skill(&new_name, &prompt) {
                return ack_err(&id, &format!("{e:#}"));
            }
            state.skill_activity.mark_mutation();
            if old_name == new_name {
                state
                    .generations
                    .lock()
                    .expect("generations lock")
                    .record_skill_write(&new_name, &prompt);
            } else {
                state.generations.lock().expect("generations lock").record_skill_rename(
                    &old_name,
                    &new_name,
                    &prompt,
                );
                if let Err(e) = record_pipelines_after_skill_reference_changes(state) {
                    return ack_err(&id, &format!("{e:#}"));
                }
            }
            if broadcast_ui(state, "skills", Some(&new_name), None).is_err() {
                return ack_err(&id, "broadcast failed");
            }
            let loc = format!("/skills/{}", urlencoding::encode(&new_name));
            ack_ok(&id, Some(loc))
        }
        ClientOp::DeleteSkill { id, name } => {
            if !state.skills.skill_exists(&name) {
                return ack_err(&id, "not found");
            }
            suppress_skill_dir(state, &name);
            if let Err(e) = state.skills.delete_skill(&name) {
                return ack_err(&id, &format!("{e:#}"));
            }
            if let Err(e) = state.pipelines.remove_skill_everywhere(&name) {
                return ack_err(&id, &format!("{e:#}"));
            }
            state.skill_activity.mark_mutation();
            state.generations.lock().expect("generations lock").record_skill_delete(&name);
            if let Err(e) = record_pipelines_after_skill_reference_changes(state) {
                return ack_err(&id, &format!("{e:#}"));
            }
            if broadcast_ui(state, "skills", None, None).is_err() {
                return ack_err(&id, "broadcast failed");
            }
            ack_ok(&id, Some("/skills".to_string()))
        }
        ClientOp::CreatePipeline { id, name } => {
            let name = name.trim().to_string();
            if let Err(e) = PipelineStore::validate_kebab_name(&name) {
                return ack_err(&id, &e.to_string());
            }
            suppress_pipeline_file(state, &name);
            if let Err(e) = state.pipelines.create_pipeline(&name) {
                let msg = e.to_string();
                if msg.contains("already exists") {
                    return ack_err(&id, "pipeline exists");
                }
                return ack_err(&id, &format!("{e:#}"));
            }
            let raw = match std::fs::read_to_string(state.pipelines.pipeline_json_path(&name)) {
                Ok(r) => r,
                Err(e) => return ack_err(&id, &format!("{e:#}")),
            };
            state
                .generations
                .lock()
                .expect("generations lock")
                .record_pipeline_created(&name, &raw);
            if broadcast_ui(state, "pipelines", None, Some(&name)).is_err() {
                return ack_err(&id, "broadcast failed");
            }
            let loc = format!("/pipelines/{}", urlencoding::encode(&name));
            ack_ok(&id, Some(loc))
        }
        ClientOp::CreateStep {
            id,
            pipeline,
            title,
            prompt,
        } => {
            if state.pipelines.get_pipeline_meta(&pipeline).is_err() {
                return ack_err(&id, "not found");
            }
            suppress_pipeline_file(state, &pipeline);
            let step_id = match state.pipelines.create_step(&pipeline, &title, &prompt) {
                Ok(id) => id,
                Err(e) => {
                    let msg = e.to_string();
                    if msg.contains("required") {
                        return ack_err(&id, &msg);
                    }
                    return ack_err(&id, &format!("{e:#}"));
                }
            };
            if let Err(e) = record_pipeline_mutation(state, &pipeline) {
                return ack_err(&id, &format!("{e:#}"));
            }
            if broadcast_ui(state, "pipelines", None, Some(&pipeline)).is_err() {
                return ack_err(&id, "broadcast failed");
            }
            let loc = format!(
                "/pipelines/{}/steps/{}",
                urlencoding::encode(&pipeline),
                step_id
            );
            ack_ok(&id, Some(loc))
        }
        ClientOp::UpdateStep {
            id,
            pipeline,
            step_id,
            title,
            prompt,
        } => {
            suppress_pipeline_file(state, &pipeline);
            if let Err(e) = state
                .pipelines
                .update_step(&pipeline, step_id, &title, &prompt)
            {
                let msg = e.to_string();
                if msg.contains("required") {
                    return ack_err(&id, &msg);
                }
                if msg.contains("not found") {
                    return ack_err(&id, "not found");
                }
                return ack_err(&id, &format!("{e:#}"));
            }
            if let Err(e) = record_pipeline_mutation(state, &pipeline) {
                return ack_err(&id, &format!("{e:#}"));
            }
            if broadcast_ui(state, "pipelines", None, Some(&pipeline)).is_err() {
                return ack_err(&id, "broadcast failed");
            }
            let loc = format!("/pipelines/{}", urlencoding::encode(&pipeline));
            ack_ok(&id, Some(loc))
        }
        ClientOp::DeleteStep {
            id,
            pipeline,
            step_id,
        } => {
            suppress_pipeline_file(state, &pipeline);
            if let Err(e) = state.pipelines.delete_step(&pipeline, step_id) {
                let msg = e.to_string();
                if msg.contains("not found") {
                    return ack_err(&id, "not found");
                }
                return ack_err(&id, &format!("{e:#}"));
            }
            if let Err(e) = record_pipeline_mutation(state, &pipeline) {
                return ack_err(&id, &format!("{e:#}"));
            }
            if broadcast_ui(state, "pipelines", None, Some(&pipeline)).is_err() {
                return ack_err(&id, "broadcast failed");
            }
            let loc = format!("/pipelines/{}", urlencoding::encode(&pipeline));
            ack_ok(&id, Some(loc))
        }
        ClientOp::ReorderStep {
            id,
            pipeline,
            step_id,
            target_step_id,
        } => {
            suppress_pipeline_file(state, &pipeline);
            if let Err(e) = state
                .pipelines
                .reorder_step(&pipeline, step_id, target_step_id)
            {
                let msg = e.to_string();
                if msg.contains("not found") {
                    return ack_err(&id, "not found");
                }
                return ack_err(&id, &format!("{e:#}"));
            }
            if let Err(e) = record_pipeline_mutation(state, &pipeline) {
                return ack_err(&id, &format!("{e:#}"));
            }
            if broadcast_ui(state, "pipelines", None, Some(&pipeline)).is_err() {
                return ack_err(&id, "broadcast failed");
            }
            let loc = format!("/pipelines/{}", urlencoding::encode(&pipeline));
            ack_ok(&id, Some(loc))
        }
        ClientOp::AddStepSkill {
            id,
            pipeline,
            step_id,
            skill_id,
        } => {
            let skill_name = skill_id.trim().to_string();
            if skill_name.is_empty() {
                return ack_err(&id, "skill_id is required");
            }
            let exists = state.skills.skill_exists(&skill_name);
            suppress_pipeline_file(state, &pipeline);
            match state.pipelines.add_step_skill(&pipeline, step_id, &skill_name, || exists) {
                Ok(()) => {
                    if let Err(e) = record_pipeline_mutation(state, &pipeline) {
                        return ack_err(&id, &format!("{e:#}"));
                    }
                    if broadcast_ui(state, "pipelines", None, Some(&pipeline)).is_err() {
                        return ack_err(&id, "broadcast failed");
                    }
                    let loc = format!("/pipelines/{}", urlencoding::encode(&pipeline));
                    ack_ok(&id, Some(loc))
                }
                Err(e) => {
                    let msg = e.to_string();
                    if msg.contains("not found") {
                        ack_err(&id, "not found")
                    } else if msg.contains("already attached") {
                        ack_err(&id, &msg)
                    } else {
                        ack_err(&id, &format!("{e:#}"))
                    }
                }
            }
        }
        ClientOp::DeleteStepSkill {
            id,
            pipeline,
            step_id,
            skill_name,
        } => {
            suppress_pipeline_file(state, &pipeline);
            if let Err(e) = state
                .pipelines
                .delete_step_skill(&pipeline, step_id, &skill_name)
            {
                if e.to_string().contains("not found") {
                    return ack_err(&id, "not found");
                }
                return ack_err(&id, &format!("{e:#}"));
            }
            if let Err(e) = record_pipeline_mutation(state, &pipeline) {
                return ack_err(&id, &format!("{e:#}"));
            }
            if broadcast_ui(state, "pipelines", None, Some(&pipeline)).is_err() {
                return ack_err(&id, "broadcast failed");
            }
            let loc = format!("/pipelines/{}", urlencoding::encode(&pipeline));
            ack_ok(&id, Some(loc))
        }
    }
}

fn build_page(
    state: &AppState,
    active: &str,
    selected_skill_name: Option<&str>,
    selected_pipeline_name: Option<&str>,
) -> Result<String> {
    let (skills, pipelines, pipeline_steps, selected_skill, selected_pipeline) =
        page_context(state, selected_skill_name, selected_pipeline_name)?;
    let selected_skill_ref = selected_skill.as_ref();
    let selected_pipeline_ref = selected_pipeline.as_ref();
    let generations_json = serde_json::to_string(
        &state
            .generations
            .lock()
            .expect("generations lock")
            .snapshot(),
    )
    .map_err(|e| anyhow!("{e}"))?;
    let page = page_input(
        active,
        &skills,
        &pipelines,
        &pipeline_steps,
        selected_skill_ref,
        selected_pipeline_ref,
        state.live_reload_enabled,
        &generations_json,
    );
    Ok(render_page(&page))
}

async fn get_root(State(state): State<AppState>) -> Result<Html<String>, Response> {
    let html = build_page(&state, "pipelines", None, None).map_err(|e| internal(&e))?;
    Ok(Html(html))
}

async fn get_skills(State(state): State<AppState>) -> Result<Html<String>, Response> {
    let html = build_page(&state, "skills", None, None).map_err(|e| internal(&e))?;
    Ok(Html(html))
}

async fn get_skill_detail(
    State(state): State<AppState>,
    axum::extract::Path(name): axum::extract::Path<String>,
) -> Result<Html<String>, Response> {
    if !state.skills.skill_exists(&name) {
        return Err(not_found());
    }
    let html = build_page(&state, "skills", Some(&name), None).map_err(|e| internal(&e))?;
    Ok(Html(html))
}

async fn get_pipelines(State(state): State<AppState>) -> Result<Html<String>, Response> {
    let html = build_page(&state, "pipelines", None, None).map_err(|e| internal(&e))?;
    Ok(Html(html))
}

async fn get_pipeline_detail(
    State(state): State<AppState>,
    axum::extract::Path(name): axum::extract::Path<String>,
) -> Result<Html<String>, Response> {
    if state.pipelines.get_pipeline_meta(&name).is_err() {
        return Err(not_found());
    }
    let html = build_page(&state, "pipelines", None, Some(&name)).map_err(|e| internal(&e))?;
    Ok(Html(html))
}

async fn get_pipeline_step_path(
    State(state): State<AppState>,
    axum::extract::Path((name, step_id)): axum::extract::Path<(String, i64)>,
) -> Result<Html<String>, Response> {
    if state.pipelines.get_pipeline_meta(&name).is_err() {
        return Err(not_found());
    }
    let steps = state
        .pipelines
        .list_steps(&name)
        .map_err(|e| internal(&e))?;
    if !steps.iter().any(|s| s.id == step_id) {
        return Err(not_found());
    }
    let html = build_page(&state, "pipelines", None, Some(&name)).map_err(|e| internal(&e))?;
    Ok(Html(html))
}

async fn get_counter(State(state): State<AppState>) -> Result<Html<String>, Response> {
    let n = counter::increment_and_get(&state.counter_path).map_err(|e| internal(&e))?;
    Ok(Html(format!(
        "<div id=\"counter\">hello world {n}</div>"
    )))
}

async fn ws_live(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> Result<Response, Response> {
    Ok(ws.on_upgrade(move |socket| handle_ws_live(socket, state)))
}

async fn handle_ws_live(mut socket: WebSocket, state: AppState) {
    let mut rx = state.live_tx.subscribe();
    loop {
        tokio::select! {
            msg = rx.recv() => {
                match msg {
                    Ok(payload) => {
                        if socket.send(Message::Text(payload.into())).await.is_err() {
                            break;
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(_)) => {}
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
            incoming = socket.recv() => {
                match incoming {
                    Some(Ok(Message::Ping(p))) => {
                        let _ = socket.send(Message::Pong(p)).await;
                    }
                    Some(Ok(Message::Text(t))) => {
                        let corr_id = serde_json::from_str::<serde_json::Value>(&t)
                            .ok()
                            .and_then(|v| {
                                v.get("id")
                                    .and_then(|x| x.as_str())
                                    .map(std::string::ToString::to_string)
                            })
                            .unwrap_or_default();
                        match serde_json::from_str::<ClientOp>(&t) {
                            Ok(op) => {
                                let ack = handle_client_op(&state, op);
                                if socket.send(Message::Text(ack.into())).await.is_err() {
                                    break;
                                }
                            }
                            Err(_) => {
                                let _ = socket.send(Message::Text(
                                    json!({"id": corr_id, "ok": false, "error": "invalid message"})
                                        .to_string()
                                        .into(),
                                ))
                                .await;
                            }
                        }
                    }
                    None | Some(Ok(Message::Close(_)) | Err(_)) => break,
                    Some(Ok(_)) => {}
                }
            }
        }
    }
}

pub fn build_router(state: AppState, static_root: PathBuf) -> Router {
    let serve = ServeDir::new(static_root);
    Router::new()
        .route("/", get(get_root))
        .route("/ws", get(ws_live))
        .route("/fragments/counter", get(get_counter))
        .route("/skills", get(get_skills))
        .route("/skills/{name}", get(get_skill_detail))
        .route(
            "/skills/{name}/{*rest}",
            any(|| async { StatusCode::NOT_FOUND }),
        )
        .route("/pipelines", get(get_pipelines))
        .route("/pipelines/{name}", get(get_pipeline_detail))
        .route(
            "/pipelines/{name}/steps/{step_id}",
            get(get_pipeline_step_path),
        )
        .fallback_service(serve)
        .with_state(state)
}
