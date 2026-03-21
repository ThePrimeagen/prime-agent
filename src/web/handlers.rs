//! HTTP handlers (ported from `internal/web/handlers.go`).

use anyhow::{anyhow, Result};
use axum::{
    extract::State,
    http::{HeaderMap, HeaderName, HeaderValue, StatusCode},
    response::{Html, IntoResponse, Redirect, Response},
    routing::{any, get, post},
    Router,
};
use serde::Deserialize;
use std::path::PathBuf;
use tower_http::services::ServeDir;

use crate::counter;
use crate::pipeline_store::PipelineStore;
use crate::skills_store::SkillsStore;
use crate::web::render::{
    join_skill_names, pipeline_vm, render_page, skill_vm, step_skill_vm, step_vm, PageInput,
    PipelineStepVm, PipelineVm, SkillVm,
};

#[derive(Clone)]
pub struct AppState {
    pub skills: SkillsStore,
    pub pipelines: PipelineStore,
    pub counter_path: PathBuf,
}

#[derive(Deserialize)]
struct CreateSkillForm {
    name: String,
    prompt: String,
}

#[derive(Deserialize)]
struct UpdateSkillForm {
    name: String,
    prompt: String,
}

#[derive(Deserialize)]
struct CreatePipelineForm {
    name: String,
}

#[derive(Deserialize)]
struct CreateStepForm {
    title: String,
    prompt: String,
}

#[derive(Deserialize)]
struct UpdateStepForm {
    title: String,
    prompt: String,
}

#[derive(Deserialize)]
struct ReorderForm {
    target_step_id: String,
}

#[derive(Deserialize)]
struct AddStepSkillForm {
    skill_id: String,
}

fn bad_req(msg: impl Into<String>) -> Response {
    (StatusCode::BAD_REQUEST, msg.into()).into_response()
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

fn build_page(
    state: &AppState,
    active: &str,
    selected_skill_name: Option<&str>,
    selected_pipeline_name: Option<&str>,
) -> Result<String> {
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

    let selected_skill_ref = selected_skill.as_ref();
    let selected_pipeline_ref = selected_pipeline.as_ref();

    let page = PageInput {
        active_section: active,
        skills: &skills,
        selected_skill: selected_skill_ref,
        pipelines: &pipelines,
        selected_pipeline: selected_pipeline_ref,
        pipeline_steps: &pipeline_steps,
    };
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

async fn post_create_skill(
    State(state): State<AppState>,
    axum::Form(form): axum::Form<CreateSkillForm>,
) -> Result<Response, Response> {
    let name = form.name.trim().to_string();
    let prompt = form.prompt.trim().to_string();
    if prompt.is_empty() {
        return Err(bad_req("prompt is required"));
    }
    if let Err(e) = SkillsStore::validate_write_name(&name) {
        return Err(bad_req(e.to_string()));
    }
    if state.skills.skill_exists(&name) {
        return Err(bad_req("skill already exists"));
    }
    if let Err(e) = state.skills.save_skill(&name, &prompt) {
        return Err(internal(&e));
    }
    let loc = format!("/skills/{}", urlencoding::encode(&name));
    Ok(Redirect::to(&loc).into_response())
}

async fn post_update_skill(
    State(state): State<AppState>,
    axum::extract::Path(old_name): axum::extract::Path<String>,
    headers: HeaderMap,
    axum::Form(form): axum::Form<UpdateSkillForm>,
) -> Result<Response, Response> {
    let autosave = headers
        .get("X-Autosave")
        .and_then(|v| v.to_str().ok())
        == Some("1");

    let new_name = form.name.trim().to_string();
    let prompt = form.prompt.trim().to_string();
    if prompt.is_empty() {
        return Err(bad_req("prompt is required"));
    }
    if let Err(e) = SkillsStore::validate_write_name(&new_name) {
        return Err(bad_req(e.to_string()));
    }
    if !state.skills.skill_exists(&old_name) {
        return Err(not_found());
    }

    if old_name != new_name {
        if state.skills.skill_exists(&new_name) {
            return Err(bad_req("skill already exists"));
        }
        if let Err(e) = state.skills.rename_skill_directory(&old_name, &new_name) {
            return Err(internal(&e));
        }
        if let Err(e) = state
            .pipelines
            .rename_skill_reference(&old_name, &new_name)
        {
            return Err(internal(&e));
        }
    }
    if let Err(e) = state.skills.save_skill(&new_name, &prompt) {
        return Err(internal(&e));
    }

    if autosave {
        let mut res = StatusCode::NO_CONTENT.into_response();
        if old_name != new_name {
            let path = format!("/skills/{}", urlencoding::encode(&new_name));
            if let Ok(val) = HeaderValue::from_str(&path) {
                res.headers_mut()
                    .insert(HeaderName::from_static("x-skill-location"), val);
            }
        }
        return Ok(res);
    }
    let loc = format!("/skills/{}", urlencoding::encode(&new_name));
    Ok(Redirect::to(&loc).into_response())
}

async fn post_delete_skill(
    State(state): State<AppState>,
    axum::extract::Path(name): axum::extract::Path<String>,
) -> Result<Response, Response> {
    if !state.skills.skill_exists(&name) {
        return Err(not_found());
    }
    if let Err(e) = state.skills.delete_skill(&name) {
        return Err(internal(&e));
    }
    if let Err(e) = state.pipelines.remove_skill_everywhere(&name) {
        return Err(internal(&e));
    }
    Ok(Redirect::to("/skills").into_response())
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

async fn post_create_pipeline(
    State(state): State<AppState>,
    axum::Form(form): axum::Form<CreatePipelineForm>,
) -> Result<Response, Response> {
    let name = form.name.trim().to_string();
    if let Err(e) = PipelineStore::validate_kebab_name(&name) {
        return Err(bad_req(e.to_string()));
    }
    if let Err(e) = state.pipelines.create_pipeline(&name) {
        if e.to_string().contains("already exists") {
            return Err(bad_req("pipeline exists"));
        }
        return Err(internal(&e));
    }
    let loc = format!("/pipelines/{}", urlencoding::encode(&name));
    Ok(Redirect::to(&loc).into_response())
}

async fn post_create_step(
    State(state): State<AppState>,
    axum::extract::Path(pipeline_name): axum::extract::Path<String>,
    axum::Form(form): axum::Form<CreateStepForm>,
) -> Result<Response, Response> {
    if state.pipelines.get_pipeline_meta(&pipeline_name).is_err() {
        return Err(not_found());
    }
    let step_id = match state
        .pipelines
        .create_step(&pipeline_name, &form.title, &form.prompt)
    {
        Ok(id) => id,
        Err(e) => {
            let msg = e.to_string();
            if msg.contains("required") {
                return Err(bad_req(msg));
            }
            return Err(internal(&e));
        }
    };
    let loc = format!(
        "/pipelines/{}/steps/{}",
        urlencoding::encode(&pipeline_name),
        step_id
    );
    Ok(Redirect::to(&loc).into_response())
}

async fn post_update_step(
    State(state): State<AppState>,
    axum::extract::Path((pipeline_name, step_id)): axum::extract::Path<(String, i64)>,
    axum::Form(form): axum::Form<UpdateStepForm>,
) -> Result<Response, Response> {
    if let Err(e) = state
        .pipelines
        .update_step(&pipeline_name, step_id, &form.title, &form.prompt)
    {
        let msg = e.to_string();
        if msg.contains("required") {
            return Err(bad_req(msg));
        }
        if msg.contains("not found") {
            return Err(not_found());
        }
        return Err(internal(&e));
    }
    let loc = format!("/pipelines/{}", urlencoding::encode(&pipeline_name));
    Ok(Redirect::to(&loc).into_response())
}

async fn post_delete_step(
    State(state): State<AppState>,
    axum::extract::Path((pipeline_name, step_id)): axum::extract::Path<(String, i64)>,
) -> Result<Response, Response> {
    if let Err(e) = state.pipelines.delete_step(&pipeline_name, step_id) {
        if e.to_string().contains("not found") {
            return Err(not_found());
        }
        return Err(internal(&e));
    }
    let loc = format!("/pipelines/{}", urlencoding::encode(&pipeline_name));
    Ok(Redirect::to(&loc).into_response())
}

async fn post_reorder_step(
    State(state): State<AppState>,
    axum::extract::Path((pipeline_name, step_id)): axum::extract::Path<(String, i64)>,
    axum::Form(form): axum::Form<ReorderForm>,
) -> Result<Response, Response> {
    let target: i64 = form
        .target_step_id
        .trim()
        .parse()
        .map_err(|_| bad_req("target_step_id is required"))?;
    if let Err(e) = state
        .pipelines
        .reorder_step(&pipeline_name, step_id, target)
    {
        let msg = e.to_string();
        if msg.contains("not found") {
            return Err(not_found());
        }
        return Err(internal(&e));
    }
    let loc = format!("/pipelines/{}", urlencoding::encode(&pipeline_name));
    Ok(Redirect::to(&loc).into_response())
}

async fn post_add_step_skill(
    State(state): State<AppState>,
    axum::extract::Path((pipeline_name, step_id)): axum::extract::Path<(String, i64)>,
    axum::Form(form): axum::Form<AddStepSkillForm>,
) -> Result<Response, Response> {
    let skill_name = form.skill_id.trim().to_string();
    if skill_name.is_empty() {
        return Err(bad_req("skill_id is required"));
    }
    let exists = state.skills.skill_exists(&skill_name);
    match state.pipelines.add_step_skill(&pipeline_name, step_id, &skill_name, || exists) {
        Ok(()) => {
            let loc = format!("/pipelines/{}", urlencoding::encode(&pipeline_name));
            Ok(Redirect::to(&loc).into_response())
        }
        Err(e) => {
            let msg = e.to_string();
            if msg.contains("not found") {
                Err(not_found())
            } else if msg.contains("already attached") {
                Err(bad_req(msg))
            } else {
                Err(internal(&e))
            }
        }
    }
}

async fn post_delete_step_skill(
    State(state): State<AppState>,
    axum::extract::Path((pipeline_name, step_id, skill_name)): axum::extract::Path<(
        String,
        i64,
        String,
    )>,
) -> Result<Response, Response> {
    if let Err(e) = state
        .pipelines
        .delete_step_skill(&pipeline_name, step_id, &skill_name)
    {
        if e.to_string().contains("not found") {
            return Err(not_found());
        }
        return Err(internal(&e));
    }
    let loc = format!("/pipelines/{}", urlencoding::encode(&pipeline_name));
    Ok(Redirect::to(&loc).into_response())
}

async fn get_counter(State(state): State<AppState>) -> Result<Html<String>, Response> {
    let n = counter::increment_and_get(&state.counter_path).map_err(|e| internal(&e))?;
    Ok(Html(format!(
        "<div id=\"counter\">hello world {n}</div>"
    )))
}

pub fn build_router(state: AppState, static_root: PathBuf) -> Router {
    let serve = ServeDir::new(static_root);
    Router::new()
        .route("/", get(get_root))
        .route("/fragments/counter", get(get_counter))
        .route("/skills", get(get_skills).post(post_create_skill))
        .route("/skills/{name}", get(get_skill_detail))
        .route("/skills/{name}/update", post(post_update_skill))
        .route("/skills/{name}/delete", post(post_delete_skill))
        .route(
            "/skills/{name}/{*rest}",
            any(|| async { StatusCode::NOT_FOUND }),
        )
        .route("/pipelines", get(get_pipelines).post(post_create_pipeline))
        .route("/pipelines/{name}", get(get_pipeline_detail))
        .route(
            "/pipelines/{name}/steps",
            post(post_create_step),
        )
        .route(
            "/pipelines/{name}/steps/{step_id}",
            get(get_pipeline_step_path),
        )
        .route(
            "/pipelines/{name}/steps/{step_id}/update",
            post(post_update_step),
        )
        .route(
            "/pipelines/{name}/steps/{step_id}/delete",
            post(post_delete_step),
        )
        .route(
            "/pipelines/{name}/steps/{step_id}/reorder",
            post(post_reorder_step),
        )
        .route(
            "/pipelines/{name}/steps/{step_id}/skills",
            post(post_add_step_skill),
        )
        .route(
            "/pipelines/{name}/steps/{step_id}/skills/{skill_name}/delete",
            post(post_delete_step_skill),
        )
        .fallback_service(serve)
        .with_state(state)
}
