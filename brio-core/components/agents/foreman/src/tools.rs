use anyhow::Result;

pub trait Tool {
    fn name(&self) -> &str;
    fn execute(&self, input: &str) -> Result<String>;
}

pub struct CreateTaskTool;

impl Tool for CreateTaskTool {
    fn name(&self) -> &str {
        "create_task"
    }

    fn execute(&self, input: &str) -> Result<String> {
        let milestone = input;
        // Insert into tasks table
        // Schema: content, priority, status, ... (from host schema)
        let sql = "INSERT INTO tasks (content, priority, status) VALUES (?, 10, 'pending')";
        let params = vec![milestone.to_string()];

        crate::brio::core::sql_state::execute(sql, &params)
            .map(|_| format!("Created task: {}", milestone))
            .map_err(|e| anyhow::anyhow!("DB Error: {}", e))
    }
}
