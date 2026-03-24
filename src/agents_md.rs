use anyhow::{Result, bail};

#[derive(Clone, Debug)]
pub struct AgentSection {
    pub name: String,
    pub content_lines: Vec<String>,
}

impl AgentSection {
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
    // Preserves non-section lines between markers; only `Section` is queried after parse.
    #[allow(dead_code)]
    Text(Vec<String>),
    Section(AgentSection),
}

impl AgentsDoc {
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
                    bail!("expected header '{expected_header}', found '{header_line}'");
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
                segments.push(DocSegment::Section(AgentSection {
                    name,
                    content_lines,
                }));
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

fn end_marker(name: &str) -> String {
    format!("<!-- prime-agent(End {name}) -->")
}

fn split_preserve_trailing_newline(contents: &str) -> Vec<String> {
    if contents.is_empty() {
        return Vec::new();
    }
    contents.split('\n').map(str::to_string).collect()
}
