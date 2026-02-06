use anyhow::{bail, Result};

#[derive(Clone, Debug)]
pub struct AgentSection {
    pub name: String,
    pub content_lines: Vec<String>,
}

impl AgentSection {
    #[must_use]
    pub fn from_content(name: String, content: &str) -> Self {
        Self {
            name,
            content_lines: split_preserve_trailing_newline(content),
        }
    }

    #[must_use]
    pub fn content_string(&self) -> String {
        self.content_lines.join("\n")
    }
}

#[derive(Debug)]
pub struct AgentsDoc {
    segments: Vec<DocSegment>,
}

#[derive(Debug)]
enum DocSegment {
    Text(Vec<String>),
    Section(AgentSection),
}

impl AgentsDoc {
    #[must_use]
    #[allow(clippy::missing_const_for_fn)]
    pub fn empty() -> Self {
        Self {
            segments: Vec::new(),
        }
    }

    pub fn parse(contents: &str) -> Result<Self> {
        let mut segments = Vec::new();
        let mut text_lines: Vec<String> = Vec::new();
        let lines = split_preserve_trailing_newline(contents);
        let mut index = 0usize;
        while index < lines.len() {
            let line = &lines[index];
            if let Some(name) = parse_start_marker(line) {
                if !text_lines.is_empty() {
                    segments.push(DocSegment::Text(std::mem::take(&mut text_lines)));
                }
                index += 1;
                if index >= lines.len() {
                    bail!("missing section header after start marker for '{name}'");
                }
                let header_line = lines[index].trim_end();
                let expected_header = format!("## {name}");
                if header_line != expected_header {
                    bail!(
                        "expected header '{expected_header}', found '{header_line}'"
                    );
                }
                index += 1;
                let mut content_lines = Vec::new();
                while index < lines.len() {
                    let line = &lines[index];
                    if is_end_marker(line, &name) {
                        break;
                    }
                    content_lines.push(line.clone());
                    index += 1;
                }
                if index >= lines.len() {
                    bail!("missing end marker for '{name}'");
                }
                segments.push(DocSegment::Section(AgentSection { name, content_lines }));
            } else {
                text_lines.push(line.clone());
            }
            index += 1;
        }
        if !text_lines.is_empty() {
            segments.push(DocSegment::Text(text_lines));
        }
        Ok(Self { segments })
    }

    #[must_use]
    pub fn section_names(&self) -> Vec<String> {
        self.segments
            .iter()
            .filter_map(|segment| match segment {
                DocSegment::Section(section) => Some(section.name.clone()),
                DocSegment::Text(_) => None,
            })
            .collect()
    }

    pub fn get_section(&self, name: &str) -> Option<&AgentSection> {
        self.segments.iter().find_map(|segment| match segment {
            DocSegment::Section(section) if section.name == name => Some(section),
            _ => None,
        })
    }

    pub fn upsert_section(&mut self, section: AgentSection) {
        for segment in &mut self.segments {
            if let DocSegment::Section(existing) = segment
                && existing.name == section.name
            {
                *existing = section;
                return;
            }
        }
        if let Some(last_section) = self
            .segments
            .iter()
            .rposition(|segment| matches!(segment, DocSegment::Section(_)))
        {
            self.segments
                .insert(last_section + 1, DocSegment::Section(section));
        } else {
            self.segments.push(DocSegment::Section(section));
        }
    }

    pub fn remove_section(&mut self, name: &str) -> bool {
        let original_len = self.segments.len();
        self.segments.retain(|segment| match segment {
            DocSegment::Section(section) => section.name != name,
            DocSegment::Text(_) => true,
        });
        original_len != self.segments.len()
    }

    #[must_use]
    pub fn render(&self) -> String {
        let mut lines: Vec<String> = Vec::new();
        for segment in &self.segments {
            match segment {
                DocSegment::Text(text_lines) => {
                    lines.extend(text_lines.clone());
                }
                DocSegment::Section(section) => {
                    let name = &section.name;
                    lines.push(start_marker(&section.name));
                    lines.push(format!("## {name}"));
                    lines.extend(section.content_lines.clone());
                    lines.push(end_marker(&section.name));
                }
            }
        }
        lines.join("\n")
    }
}

#[must_use]
pub fn render_sections(sections: &[AgentSection]) -> String {
    let mut lines = Vec::new();
    for (index, section) in sections.iter().enumerate() {
        if index > 0 {
            lines.push(String::new());
        }
        let name = &section.name;
        lines.push(start_marker(name));
        lines.push(format!("## {name}"));
        lines.extend(section.content_lines.clone());
        lines.push(end_marker(name));
    }
    lines.join("\n")
}

fn parse_start_marker(line: &str) -> Option<String> {
    let prefix = "<!-- prime-agent(Start ";
    let suffix = ") -->";
    if line.starts_with(prefix) && line.ends_with(suffix) {
        let name = line[prefix.len()..line.len() - suffix.len()].trim();
        if !name.is_empty() {
            return Some(name.to_string());
        }
    }
    None
}

fn is_end_marker(line: &str, name: &str) -> bool {
    line.trim_end() == end_marker(name)
}

fn start_marker(name: &str) -> String {
    format!("<!-- prime-agent(Start {name}) -->")
}

fn end_marker(name: &str) -> String {
    format!("<!-- prime-agent(End {name}) -->")
}

fn split_preserve_trailing_newline(contents: &str) -> Vec<String> {
    if contents.is_empty() {
        return Vec::new();
    }
    contents.split('\n').map(str::to_string).collect()
}
