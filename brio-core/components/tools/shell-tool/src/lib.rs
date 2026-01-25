use std::process::Command;
use wit_bindgen::generate;

generate!({
    world: "standard-tool",
    path: "../../../wit/tool.wit",
    export_macro_name: "export_shell_tool",
});

export_shell_tool!(ShellTool);

struct ShellTool;

impl exports::brio::core::tool::Guest for ShellTool {
    fn get_info() -> exports::brio::core::tool::ToolInfo {
        exports::brio::core::tool::ToolInfo {
            name: "shell".to_string(),
            description: "Executes shell commands. Use with caution.".to_string(),
            version: "0.1.0".to_string(),
        }
    }

    fn execute(params: String) -> Result<String, String> {
        let args: Vec<String> =
            serde_json::from_str(&params).map_err(|e| format!("Failed to parse params: {}", e))?;

        if args.is_empty() {
            return Err("No command provided".to_string());
        }

        let command = &args[0];
        let args = &args[1..];

        let output = Command::new(command)
            .args(args)
            .output()
            .map_err(|e| format!("Failed to execute command: {}", e))?;

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        } else {
            Err(String::from_utf8_lossy(&output.stderr).to_string())
        }
    }
}
