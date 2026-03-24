//! JSON protocol for bidirectional `/ws` (mutations + UI broadcasts).

use serde::{Deserialize, Serialize};

use crate::generation::GenerationSnapshot;
use crate::web::render::{PipelineStepVm, PipelineVm, SkillVm};

#[derive(Debug, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum ClientOp {
    CreateSkill {
        id: String,
        name: String,
        prompt: String,
    },
    UpdateSkill {
        id: String,
        old_name: String,
        name: String,
        prompt: String,
    },
    DeleteSkill {
        id: String,
        name: String,
    },
    CreatePipeline {
        id: String,
        name: String,
    },
    RenamePipeline {
        id: String,
        old_name: String,
        new_name: String,
    },
    CreateStep {
        id: String,
        pipeline: String,
        title: String,
        prompt: String,
    },
    UpdateStep {
        id: String,
        pipeline: String,
        step_id: i64,
        title: String,
        prompt: String,
    },
    DeleteStep {
        id: String,
        pipeline: String,
        step_id: i64,
    },
    ReorderStep {
        id: String,
        pipeline: String,
        step_id: i64,
        target_step_id: i64,
    },
    AddStepSkill {
        id: String,
        pipeline: String,
        step_id: i64,
        skill_id: String,
    },
    DeleteStepSkill {
        id: String,
        pipeline: String,
        step_id: i64,
        skill_id: String,
    },
}

#[derive(Debug, Serialize)]
pub struct AckMsg {
    pub id: String,
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// WebSocket `type: "ui"` payload: view state only (no HTML strings).
#[derive(Debug, Serialize)]
pub struct UiBroadcast {
    pub r#type: &'static str,
    pub active_section: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub push_url: Option<String>,
    pub skills: Vec<SkillVm>,
    pub pipelines: Vec<PipelineVm>,
    pub pipeline_steps: Vec<PipelineStepVm>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selected_skill: Option<SkillVm>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selected_pipeline: Option<PipelineVm>,
    pub live_reload: bool,
    pub generations: GenerationSnapshot,
}
