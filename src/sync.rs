use crate::agents_md::{AgentSection, AgentsDoc};
use crate::skills_store::SkillsStore;
use anyhow::{bail, Context, Result};
use similar::{ChangeTag, TextDiff};
use std::collections::BTreeSet;
use std::fs;
use std::io::{self, Write};
use std::path::Path;

pub fn run_sync(skills_store: &SkillsStore, agents_path: &Path) -> Result<()> {
    let (mut agents_doc, original_agents) = read_agents_doc(agents_path)?;
    let mut all_names = BTreeSet::new();

    for name in agents_doc.section_names() {
        all_names.insert(name);
    }
    for name in skills_store.list_skill_names()? {
        all_names.insert(name);
    }

    let mut updated = false;
    for name in all_names {
        SkillsStore::validate_name(&name)?;
        let skill_exists = skills_store.skill_exists(&name);
        let section = agents_doc.get_section(&name).cloned();

        match (skill_exists, section) {
            (false, Some(section)) => {
                skills_store.save_skill(&name, &section.content_string())?;
            }
            (true, None) => {
                let content = skills_store.load_skill(&name)?;
                agents_doc.upsert_section(AgentSection::from_content(name, &content));
                updated = true;
            }
            (true, Some(section)) => {
                let skill_content = skills_store.load_skill(&name)?;
                let agents_content = section.content_string();
                if normalize_content(&skill_content) != normalize_content(&agents_content) {
                    let resolved = resolve_conflicts_interactive(&name, &skill_content, &agents_content)?;
                    skills_store.save_skill(&name, &resolved)?;
                    agents_doc.upsert_section(AgentSection::from_content(name, &resolved));
                    updated = true;
                }
            }
            (false, None) => {}
        }
    }

    let rendered = agents_doc.render();
    if updated || original_agents.as_deref() != Some(rendered.as_str()) {
        fs::write(agents_path, rendered)
            .with_context(|| format!("failed to write '{}'", agents_path.display()))?;
    }

    Ok(())
}

fn read_agents_doc(path: &Path) -> Result<(AgentsDoc, Option<String>)> {
    if path.exists() {
        let contents = fs::read_to_string(path)
            .with_context(|| format!("failed to read '{}'", path.display()))?;
        let doc = AgentsDoc::parse(&contents)?;
        Ok((doc, Some(contents)))
    } else {
        Ok((AgentsDoc::empty(), None))
    }
}

fn resolve_conflicts_interactive(
    name: &str,
    skill_content: &str,
    agents_content: &str,
) -> Result<String> {
    let diff = TextDiff::from_lines(skill_content, agents_content);
    if diff.ops().is_empty() {
        return Ok(skill_content.to_string());
    }

    let mut resolved = String::new();
    for group in diff.grouped_ops(3) {
        let hunk = render_hunk(&diff, &group);
        println!("\nConflict in skill '{name}':\n{hunk}");
        let choice = prompt_choice()?;
        for op in &group {
            for change in diff.iter_changes(op) {
                match change.tag() {
                    ChangeTag::Equal => resolved.push_str(change.value()),
                    ChangeTag::Delete => {
                        if choice == Choice::Skill {
                            resolved.push_str(change.value());
                        }
                    }
                    ChangeTag::Insert => {
                        if choice == Choice::Agents {
                            resolved.push_str(change.value());
                        }
                    }
                }
            }
        }
    }

    Ok(resolved)
}

fn render_hunk(diff: &TextDiff<'_, '_, '_, str>, group: &[similar::DiffOp]) -> String {
    let mut out = String::new();
    for op in group {
        for change in diff.iter_changes(op) {
            let sign = match change.tag() {
                ChangeTag::Delete => "-",
                ChangeTag::Insert => "+",
                ChangeTag::Equal => " ",
            };
            out.push_str(sign);
            out.push_str(change.value());
        }
    }
    out
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Choice {
    Skill,
    Agents,
}

fn prompt_choice() -> Result<Choice> {
    loop {
        print!("Choose [s]kill or [a]gents for this hunk: ");
        io::stdout().flush().ok();
        let mut input = String::new();
        let read = io::stdin().read_line(&mut input)?;
        if read == 0 {
            bail!("stdin closed during conflict resolution");
        }
        match input.trim().to_ascii_lowercase().as_str() {
            "s" | "skill" => return Ok(Choice::Skill),
            "a" | "agents" => return Ok(Choice::Agents),
            _ => {
                println!("Enter 's' or 'a'.");
            }
        }
    }
}

fn normalize_content(content: &str) -> String {
    content.replace("\r\n", "\n").trim_end_matches('\n').to_string()
}
