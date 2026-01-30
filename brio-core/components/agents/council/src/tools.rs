use anyhow::Result;
use regex::{Captures, Regex};

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
