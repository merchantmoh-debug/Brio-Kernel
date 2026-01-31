use anyhow::Result;
use regex::{Captures, Regex};
use std::fs;

pub trait Tool {
    fn name(&self) -> &str;
    fn description_xml(&self) -> &str;
    fn regex(&self) -> Regex;
    fn execute(&self, caps: &Captures) -> Result<String>;
}

pub struct ToolRegistry {
    tools: Vec<Box<dyn Tool>>,
}

pub struct ExecutionResult {
    pub output: String,
    pub is_done: bool,
    pub final_output: Option<String>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self { tools: Vec::new() }
    }

    pub fn register(&mut self, tool: Box<dyn Tool>) {
        self.tools.push(tool);
    }

    pub fn help_text(&self) -> String {
        self.tools
            .iter()
            .map(|t| t.description_xml())
            .collect::<Vec<_>>()
            .join("\n")
    }

    pub fn execute_all(&self, input: &str) -> Result<ExecutionResult> {
        let mut matches: Vec<(usize, usize, Captures)> = Vec::new();

        for (i, tool) in self.tools.iter().enumerate() {
            let re = tool.regex();
            for cap in re.captures_iter(input) {
                let start = cap.get(0).unwrap().start();
                matches.push((start, i, cap));
            }
        }

        matches.sort_by_key(|(start, _, _)| *start);

        let mut output = String::new();
        let mut is_done = false;
        let mut final_output = None;

        for (_, tool_idx, caps) in matches {
            let tool = &self.tools[tool_idx];
            if tool.name() == "done" {
                is_done = true;
                if let Some(c) = caps.get(1) {
                    final_output = Some(c.as_str().trim().to_string());
                }
                break;
            }

            match tool.execute(&caps) {
                Ok(res) => {
                    output.push_str(&format!("Successfully executed {}: {}\n", tool.name(), res))
                }
                Err(e) => output.push_str(&format!("Error executing {}: {}\n", tool.name(), e)),
            }
        }

        Ok(ExecutionResult {
            output,
            is_done,
            final_output,
        })
    }
}

pub struct ReadFileTool;
impl Tool for ReadFileTool {
    fn name(&self) -> &str {
        "read_file"
    }
    fn description_xml(&self) -> &str {
        r#"<read_file path="path/to/file" />"#
    }
    fn regex(&self) -> Regex {
        Regex::new(r#"(?s)<read_file\s+path="([^"]+)"\s*(?:/>|>\s*</read_file>)"#).unwrap()
    }
    fn execute(&self, caps: &Captures) -> Result<String> {
        let path = caps.get(1).unwrap().as_str();
        let content = fs::read_to_string(path)?;
        Ok(content)
    }
}

pub struct LsTool;
impl Tool for LsTool {
    fn name(&self) -> &str {
        "ls"
    }
    fn description_xml(&self) -> &str {
        r#"<ls path="path/to/directory" />"#
    }
    fn regex(&self) -> Regex {
        Regex::new(r#"(?s)<ls\s+path="([^"]+)"\s*(?:/>|>\s*</ls>)"#).unwrap()
    }
    fn execute(&self, caps: &Captures) -> Result<String> {
        let path = caps.get(1).unwrap().as_str();
        let mut entries = Vec::new();
        for entry in fs::read_dir(path)? {
            let entry = entry?;
            let ty = if entry.file_type()?.is_dir() {
                "DIR"
            } else {
                "FILE"
            };
            entries.push(format!("{} {:?}", ty, entry.file_name()));
        }
        Ok(entries.join("\n"))
    }
}

pub struct DoneTool;
impl Tool for DoneTool {
    fn name(&self) -> &str {
        "done"
    }
    fn description_xml(&self) -> &str {
        r#"<done>summary</done>"#
    }
    fn regex(&self) -> Regex {
        Regex::new(r#"(?s)<done>\s*(.*?)\s*</done>"#).unwrap()
    }
    fn execute(&self, _caps: &Captures) -> Result<String> {
        Ok("Done".to_string())
    }
}
