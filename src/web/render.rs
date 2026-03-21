//! Server-rendered HTML (ported from the Go `handlers.go` template).
#![allow(clippy::if_not_else)]
#![allow(clippy::too_many_lines)]
#![allow(clippy::write_with_newline)]

use std::fmt::Write;

use urlencoding::encode;

#[derive(Debug, Clone)]
pub struct SkillVm {
    pub name: String,
    pub name_encoded: String,
    pub prompt: String,
}

#[derive(Debug, Clone)]
pub struct PipelineVm {
    pub name: String,
    pub name_encoded: String,
}

#[derive(Debug, Clone)]
pub struct StepSkillVm {
    pub name: String,
    pub name_encoded: String,
}

#[derive(Debug, Clone)]
pub struct PipelineStepVm {
    pub id: i64,
    pub title: String,
    pub prompt: String,
    pub skill_count: i64,
    pub skills: Vec<StepSkillVm>,
    pub skill_summary: String,
}

pub struct PageInput<'a> {
    pub active_section: &'a str,
    pub skills: &'a [SkillVm],
    pub selected_skill: Option<&'a SkillVm>,
    pub pipelines: &'a [PipelineVm],
    pub selected_pipeline: Option<&'a PipelineVm>,
    pub pipeline_steps: &'a [PipelineStepVm],
}

fn esc_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn esc_attr(s: &str) -> String {
    s.replace('&', "&amp;").replace('"', "&quot;")
}

fn enc(s: &str) -> String {
    encode(s).into_owned()
}

fn aria_current(active: &str, tab: &str) -> &'static str {
    if active == tab {
        "page"
    } else {
        "false"
    }
}

fn hidden_attr(active: &str, tab: &str) -> &'static str {
    if active == tab {
        ""
    } else {
        " hidden"
    }
}

pub fn render_page(p: &PageInput<'_>) -> String {
    let mut o = String::new();
    o.push_str("<!doctype html>\n<html lang=\"en\">\n  <head>\n    <meta charset=\"utf-8\">\n    <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n    <title>Hello World</title>\n    <script src=\"https://unpkg.com/htmx.org@2.0.4\"></script>\n  </head>\n  <body style=\"margin:0;font-family:system-ui, sans-serif;\">\n    <div id=\"app-shell\" style=\"display:flex;min-height:100vh;\">\n      <nav id=\"left-nav\" style=\"width:20%;min-width:220px;border-right:1px solid #ddd;padding:1rem;box-sizing:border-box;\">\n        <div style=\"display:flex;gap:0.5rem;margin-bottom:1rem;\">\n          <a\n            id=\"tab-skills\"\n            data-testid=\"tab-skills\"\n            data-icon=\"sword\"\n            title=\"Skills\"\n            aria-label=\"Skills\"\n            href=\"/skills\"\n            aria-current=\"");
    let _ = write!(o, "{}", aria_current(p.active_section, "skills"));
    o.push_str("\"\n            style=\"display:inline-flex;align-items:center;justify-content:center;width:2rem;height:2rem;border:1px solid #bbb;border-radius:6px;background:#fff;cursor:pointer;text-decoration:none;color:inherit;\">&#9876;</a>\n          <a\n            id=\"tab-pipeline\"\n            data-testid=\"tab-pipeline\"\n            data-icon=\"pipe\"\n            title=\"Pipeline\"\n            aria-label=\"Pipeline\"\n            href=\"/pipelines\"\n            aria-current=\"");
    let _ = write!(o, "{}", aria_current(p.active_section, "pipelines"));
    o.push_str("\"\n            style=\"display:inline-flex;align-items:center;justify-content:center;width:2rem;height:2rem;border:1px solid #bbb;border-radius:6px;background:#fff;cursor:pointer;text-decoration:none;color:inherit;\">&#x2554;&#x2557;</a>\n        </div>\n\n        <section id=\"skills-nav-panel\" data-tab-panel=\"skills\"");
    o.push_str(hidden_attr(p.active_section, "skills"));
    o.push_str(">\n          <h2 style=\"margin-top:0;\">Skills</h2>\n          <button\n            type=\"button\"\n            data-testid=\"skill-create-open\"\n            style=\"background:#2563eb;color:#fff;border:none;border-radius:6px;padding:0.35rem 0.8rem;cursor:pointer;\"\n            onclick=\"document.getElementById('skill-modal').showModal();\">[ + ]</button>\n\n          <dialog id=\"skill-modal\">\n            <form method=\"post\" action=\"/skills\">\n              <h3>Create Skill</h3>\n              <label>Name<br><input name=\"name\" required></label><br><br>\n              <label>Prompt<br><textarea name=\"prompt\" rows=\"6\" cols=\"40\" required></textarea></label><br><br>\n              <button type=\"submit\">Save</button>\n              <button type=\"button\" onclick=\"document.getElementById('skill-modal').close();\">Cancel</button>\n            </form>\n          </dialog>\n\n");
    if !p.skills.is_empty() {
        o.push_str("          <ul id=\"skills-nav-list\" style=\"padding-left:0;margin-top:1rem;\">\n");
        for skill in p.skills {
            let sel = p
                .selected_skill
                .is_some_and(|s| s.name == skill.name);
            let _ = write!(
                o,
                "            <li style=\"list-style:none;margin-top:0.5rem;\">\n              <a\n                data-testid=\"skill-nav-link\"\n                data-selected=\"{}\"\n                href=\"/skills/{}\"\n                style=\"display:block;padding:0.4rem 0.5rem;border:1px solid #ddd;border-radius:6px;text-decoration:none;color:inherit;\">{}</a>\n            </li>\n",
                if sel { "true" } else { "false" },
                skill.name_encoded,
                esc_html(&skill.name)
            );
        }
        o.push_str("          </ul>\n");
    } else {
        o.push_str("          <p>No skills yet.</p>\n");
    }
    o.push_str("        </section>\n\n        <section id=\"pipeline-nav-panel\" data-tab-panel=\"pipeline\"");
    o.push_str(hidden_attr(p.active_section, "pipelines"));
    o.push_str(">\n          <h2 style=\"margin-top:0;\">Pipeline</h2>\n          <button\n            type=\"button\"\n            data-testid=\"pipeline-create-open\"\n            style=\"background:#2563eb;color:#fff;border:none;border-radius:6px;padding:0.35rem 0.8rem;cursor:pointer;\"\n            onclick=\"document.getElementById('pipeline-modal').showModal();\">[ + ]</button>\n\n          <dialog id=\"pipeline-modal\">\n            <form method=\"post\" action=\"/pipelines\">\n              <h3>Create Pipeline</h3>\n              <label>Name<br><input name=\"name\" data-lowercase-field=\"pipeline-name\" required></label><br><br>\n              <button type=\"submit\">Save</button>\n              <button type=\"button\" onclick=\"document.getElementById('pipeline-modal').close();\">Cancel</button>\n            </form>\n          </dialog>\n\n");
    if !p.pipelines.is_empty() {
        o.push_str("          <ul id=\"pipeline-nav-list\" style=\"padding-left:0;margin-top:1rem;\">\n");
        for pl in p.pipelines {
            let sel = p
                .selected_pipeline
                .is_some_and(|s| s.name == pl.name);
            let _ = write!(
                o,
                "            <li style=\"list-style:none;margin-top:0.5rem;\">\n              <a\n                data-testid=\"pipeline-nav-link\"\n                data-selected=\"{}\"\n                href=\"/pipelines/{}\"\n                style=\"display:block;padding:0.4rem 0.5rem;border:1px solid #ddd;border-radius:6px;text-decoration:none;color:inherit;\">{}</a>\n            </li>\n",
                if sel { "true" } else { "false" },
                pl.name_encoded,
                esc_html(&pl.name)
            );
        }
        o.push_str("          </ul>\n");
    } else {
        o.push_str("          <p>No pipelines yet.</p>\n");
    }

    if let Some(pl) = p.selected_pipeline {
        o.push_str("          <h3 style=\"margin-top:1.25rem;margin-bottom:0.25rem;\">Steps</h3>\n");
        if !p.pipeline_steps.is_empty() {
            o.push_str("          <ul id=\"pipeline-step-nav-list\" style=\"padding-left:0;margin-top:0.5rem;\">\n");
            for step in p.pipeline_steps {
                let _ = write!(
                    o,
                    "            <li\n              data-testid=\"pipeline-step-nav-item\"\n              data-step-id=\"{}\"\n              data-reorder-endpoint=\"/pipelines/{}/steps/{}/reorder\"\n              draggable=\"true\"\n              style=\"list-style:none;margin-top:0.4rem;padding:0.45rem 0.55rem;border:1px solid #ddd;border-radius:6px;background:#fff;display:flex;justify-content:space-between;align-items:center;gap:0.5rem;cursor:grab;\">\n              <span>{}</span>\n              <span style=\"display:flex;flex-direction:column;align-items:flex-end;gap:0.1rem;\">\n                <span data-testid=\"pipeline-step-skill-count\" style=\"font-size:0.8rem;color:#4b5563;\">{}</span>\n                <span data-testid=\"pipeline-step-skill-summary\" style=\"font-size:0.72rem;color:#6b7280;\">{}</span>\n              </span>\n            </li>\n",
                    step.id,
                    pl.name_encoded,
                    step.id,
                    esc_html(&step.title),
                    step.skill_count,
                    esc_html(&step.skill_summary)
                );
            }
            o.push_str("          </ul>\n");
        } else {
            o.push_str("          <p style=\"margin-top:0.5rem;\">No steps yet.</p>\n");
        }
    }
    o.push_str("        </section>\n      </nav>\n      <main id=\"main-content\" style=\"width:80%;padding:1rem;box-sizing:border-box;\">\n        <section id=\"skills-main-panel\" data-tab-main=\"skills\"");
    o.push_str(hidden_attr(p.active_section, "skills"));
    o.push_str(">\n          <h1>Skills</h1>\n");
    if let Some(sk) = p.selected_skill {
        let _ = write!(
            o,
            "          <article data-skill-editor data-skill-id=\"{}\" style=\"max-width:740px;border:1px solid #ddd;padding:0.8rem;margin-top:0.8rem;position:relative;\">\n            <form method=\"post\" action=\"/skills/{}/update\" data-skill-form>\n              <label>Name<br><input name=\"name\" value=\"{}\" required></label><br><br>\n              <label>Prompt<br><textarea name=\"prompt\" rows=\"8\" cols=\"70\" required>{}</textarea></label><br><br>\n              <div style=\"position:absolute;top:0.5rem;right:0.6rem;display:flex;align-items:center;gap:0.35rem;\">\n                <span\n                  data-testid=\"autosave-status\"\n                  data-save-state=\"saved\"\n                  aria-live=\"polite\"\n                  aria-label=\"Saved\"\n                  title=\"Saved\"\n                  style=\"font-size:0.85rem;line-height:1;color:#047857;\">✓</span>\n                <div data-delete-controls style=\"position:relative;\">\n                  <button\n                    type=\"button\"\n                    data-testid=\"delete-skill-trigger\"\n                    aria-label=\"Delete skill\"\n                    aria-haspopup=\"dialog\"\n                    aria-expanded=\"false\"\n                    title=\"Delete skill\"\n                    style=\"border:none;background:transparent;color:#dc2626;cursor:pointer;font-size:0.85rem;line-height:1;padding:0;\">&#128465;</button>\n                  <div\n                    data-testid=\"delete-skill-popover\"\n                    data-delete-popover\n                    hidden\n                    style=\"position:absolute;top:1.1rem;right:0;background:#fff;border:1px solid #d1d5db;border-radius:6px;padding:0.25rem;box-shadow:0 2px 8px rgba(0,0,0,0.12);z-index:10;white-space:nowrap;\">\n                    <button\n                      type=\"button\"\n                      data-delete-confirm\n                      style=\"border:1px solid #ef4444;background:#fff;color:#b91c1c;border-radius:4px;padding:0.1rem 0.35rem;font-size:0.72rem;cursor:pointer;\">are you sure</button>\n                  </div>\n                </div>\n              </div>\n            </form>\n            <form method=\"post\" action=\"/skills/{}/delete\" data-skill-delete-form></form>\n          </article>\n",
            esc_attr(&sk.name),
            sk.name_encoded,
            esc_attr(&sk.name),
            esc_html(&sk.prompt),
            sk.name_encoded
        );
    } else if !p.skills.is_empty() {
        o.push_str("            <p>Select a skill from the left nav.</p>\n");
    } else {
        o.push_str("            <p>No skills yet.</p>\n");
    }
    o.push_str("        </section>\n\n        <section id=\"pipeline-main-panel\" data-tab-main=\"pipeline\"");
    o.push_str(hidden_attr(p.active_section, "pipelines"));
    o.push_str(">\n");
    if let Some(pl) = p.selected_pipeline {
        let _ = write!(
            o,
            "          <h1 id=\"pipeline-title\">{}</h1>\n          <button\n            type=\"button\"\n            data-testid=\"pipeline-step-create-open\"\n            style=\"background:#2563eb;color:#fff;border:none;border-radius:6px;padding:0.35rem 0.8rem;cursor:pointer;\"\n            onclick=\"document.getElementById('pipeline-step-modal').showModal();\">Add Step</button>\n\n          <dialog id=\"pipeline-step-modal\">\n            <form method=\"post\" action=\"/pipelines/{}/steps\">\n              <h3>Create Step</h3>\n              <label>Title<br><input name=\"title\" data-lowercase-field=\"pipeline-step-title\" required></label><br><br>\n              <label>Prompt<br><textarea name=\"prompt\" rows=\"6\" cols=\"40\" required></textarea></label><br><br>\n              <button type=\"submit\">Save</button>\n              <button type=\"button\" onclick=\"document.getElementById('pipeline-step-modal').close();\">Cancel</button>\n            </form>\n          </dialog>\n\n",
            esc_html(&pl.name),
            pl.name_encoded
        );
        if !p.pipeline_steps.is_empty() {
            for step in p.pipeline_steps {
                let _ = write!(
                    o,
                    "            <article data-testid=\"pipeline-step-editor\" style=\"max-width:740px;border:1px solid #ddd;padding:0.8rem;margin-top:0.8rem;position:relative;\">\n              <form method=\"post\" action=\"/pipelines/{}/steps/{}/update\">\n                <label>Title<br><input name=\"title\" data-lowercase-field=\"pipeline-step-title\" value=\"{}\" required></label><br><br>\n                <label>Description<br><textarea name=\"prompt\" rows=\"6\" cols=\"70\" required>{}</textarea></label><br><br>\n                <button type=\"submit\" data-testid=\"pipeline-step-save\">Save</button>\n              </form>\n              <div style=\"display:flex;gap:0.5rem;align-items:center;flex-wrap:wrap;\">\n                <form method=\"post\" action=\"/pipelines/{}/steps/{}/skills\" style=\"display:flex;gap:0.4rem;align-items:center;\">\n                  <select name=\"skill_id\" required>\n                    <option value=\"\">Select skill</option>\n",
                    pl.name_encoded,
                    step.id,
                    esc_html(&step.title),
                    esc_html(&step.prompt),
                    pl.name_encoded,
                    step.id
                );
                for sk in p.skills {
                    let _ = write!(
                        o,
                        "                    <option value=\"{}\">{}</option>\n",
                        esc_attr(&sk.name),
                        esc_html(&sk.name)
                    );
                }
                let _ = write!(
                    o,
                    "                  </select>\n                  <button type=\"submit\">Add Skill</button>\n                </form>\n                <form method=\"post\" action=\"/pipelines/{}/steps/{}/delete\">\n                  <button type=\"submit\" data-testid=\"pipeline-step-delete\" style=\"border:1px solid #ef4444;background:#fff;color:#b91c1c;border-radius:4px;padding:0.2rem 0.45rem;font-size:0.8rem;cursor:pointer;\">Delete</button>\n                </form>\n              </div>\n",
                    pl.name_encoded,
                    step.id
                );
                if !step.skills.is_empty() {
                    o.push_str("              <ul style=\"padding-left:1.1rem;margin:0.6rem 0 0 0;\">\n");
                    for att in &step.skills {
                        let _ = write!(
                            o,
                            "                <li style=\"margin-top:0.3rem;\">\n                  <span data-testid=\"pipeline-step-attached-skill\">{}</span>\n                  <form method=\"post\" action=\"/pipelines/{}/steps/{}/skills/{}/delete\" style=\"display:inline;\">\n                    <button type=\"submit\" style=\"margin-left:0.4rem;border:1px solid #ef4444;background:#fff;color:#b91c1c;border-radius:4px;padding:0.1rem 0.35rem;font-size:0.72rem;cursor:pointer;\">Remove</button>\n                  </form>\n                </li>\n",
                            esc_html(&att.name),
                            pl.name_encoded,
                            step.id,
                            att.name_encoded
                        );
                    }
                    o.push_str("              </ul>\n");
                } else {
                    o.push_str("              <p style=\"margin-top:0.6rem;color:#6b7280;\">No skills attached.</p>\n");
                }
                o.push_str("            </article>\n");
            }
        } else {
            o.push_str("            <p>No steps yet.</p>\n");
        }
    } else {
        o.push_str("          <h1>Pipeline</h1>\n");
        if !p.pipelines.is_empty() {
            o.push_str("          <p>Select a pipeline from the left nav.</p>\n");
        } else {
            o.push_str("          <p>No pipelines yet.</p>\n");
        }
    }
    o.push_str("        </section>\n      </main>\n    </div>\n    <script>\n");
    o.push_str(include_str!("script.inc"));
    o.push_str("    </script>\n  </body>\n</html>\n");
    o
}

pub fn skill_vm(name: &str, prompt: &str) -> SkillVm {
    SkillVm {
        name: name.to_string(),
        name_encoded: enc(name),
        prompt: prompt.to_string(),
    }
}

pub fn pipeline_vm(name: &str) -> PipelineVm {
    PipelineVm {
        name: name.to_string(),
        name_encoded: enc(name),
    }
}

pub fn step_vm(
    id: i64,
    title: &str,
    prompt: &str,
    skill_count: i64,
    skills: Vec<StepSkillVm>,
    skill_summary: String,
) -> PipelineStepVm {
    PipelineStepVm {
        id,
        title: title.to_string(),
        prompt: prompt.to_string(),
        skill_count,
        skills,
        skill_summary,
    }
}

pub fn step_skill_vm(name: &str) -> StepSkillVm {
    StepSkillVm {
        name: name.to_string(),
        name_encoded: enc(name),
    }
}

pub fn join_skill_names(skills: &[crate::pipeline_store::StepSkillView]) -> String {
    let mut names: Vec<&str> = skills.iter().map(|s| s.name.as_str()).collect();
    names.sort_unstable();
    names.join(", ")
}
