//! Pure UI state for pipeline run (tests + TUI).
#![allow(dead_code)] // Model for future TUI polish (stage colors, failure path).

use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineState {
    Pending,
    Running,
    Passed,
    Failed,
}

#[derive(Debug, Clone)]
pub struct SkillLine {
    pub name: String,
    pub state: LineState,
}

#[derive(Debug, Clone)]
pub struct StageLine {
    pub title: String,
    pub state: LineState,
    pub skills: Vec<SkillLine>,
}

#[derive(Debug, Clone)]
pub struct PipelineHeaderState {
    pub pipeline_name: String,
    pub run_dir: PathBuf,
    pub state: LineState,
}

#[derive(Debug, Clone)]
pub struct PipelineUiModel {
    pub header: PipelineHeaderState,
    pub stages: Vec<StageLine>,
    pub failure_json: Option<PathBuf>,
}

impl PipelineUiModel {
    #[must_use]
    pub fn new(pipeline_name: &str, run_dir: &Path, stage_titles: &[String], skills_per_stage: &[Vec<String>]) -> Self {
        let stages: Vec<StageLine> = stage_titles
            .iter()
            .enumerate()
            .map(|(i, title)| {
                let skills = skills_per_stage
                    .get(i)
                    .map(|names| {
                        names
                            .iter()
                            .map(|n| SkillLine {
                                name: n.clone(),
                                state: LineState::Pending,
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                StageLine {
                    title: title.clone(),
                    state: LineState::Pending,
                    skills,
                }
            })
            .collect();
        Self {
            header: PipelineHeaderState {
                pipeline_name: pipeline_name.to_string(),
                run_dir: run_dir.to_path_buf(),
                state: LineState::Running,
            },
            stages,
            failure_json: None,
        }
    }

    pub fn set_stage_running(&mut self, idx: usize) {
        if let Some(s) = self.stages.get_mut(idx) {
            s.state = LineState::Running;
            for sk in &mut s.skills {
                sk.state = LineState::Running;
            }
        }
    }

    pub fn set_stage_passed(&mut self, idx: usize) {
        if let Some(s) = self.stages.get_mut(idx) {
            s.state = LineState::Passed;
            for sk in &mut s.skills {
                sk.state = LineState::Passed;
            }
        }
    }

    pub fn set_failed(&mut self, stage_idx: usize, json_path: PathBuf) {
        self.header.state = LineState::Failed;
        if let Some(s) = self.stages.get_mut(stage_idx) {
            s.state = LineState::Failed;
            for sk in &mut s.skills {
                if sk.state == LineState::Running || sk.state == LineState::Pending {
                    sk.state = LineState::Failed;
                }
            }
        }
        self.failure_json = Some(json_path);
    }

    pub fn mark_all_passed(&mut self) {
        self.header.state = LineState::Passed;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linear_stage_passes() {
        let mut m = PipelineUiModel::new("p", Path::new("/tmp/out"), &["s1".to_string()], &[vec![]]);
        m.set_stage_running(0);
        m.set_stage_passed(0);
        m.mark_all_passed();
        assert_eq!(m.header.state, LineState::Passed);
        assert_eq!(m.stages[0].state, LineState::Passed);
    }

    #[test]
    fn parallel_skills_then_pass() {
        let mut m = PipelineUiModel::new(
            "p",
            Path::new("/x"),
            &["s1".to_string()],
            &[vec!["a".to_string(), "b".to_string()]],
        );
        m.set_stage_running(0);
        assert_eq!(m.stages[0].skills[0].state, LineState::Running);
        m.set_stage_passed(0);
        assert_eq!(m.stages[0].skills[0].state, LineState::Passed);
        assert_eq!(m.stages[0].skills[1].state, LineState::Passed);
    }

    #[test]
    fn failure_sets_path() {
        let mut m = PipelineUiModel::new("p", Path::new("/x"), &["s1".to_string()], &[vec![]]);
        let p = PathBuf::from("/x/.prime-agent/pipeline-p/1.json");
        m.set_failed(0, p.clone());
        assert_eq!(m.failure_json, Some(p));
        assert_eq!(m.header.state, LineState::Failed);
    }
}
