use wit_bindgen::generate;

generate!({
    world: "tool-runner",
    path: "../../../wit",
    exports: {
        "brio:tools/tool-grep": GrepTool,
    },
});

struct GrepTool;

impl exports::brio::tools::tool_grep::Guest for GrepTool {
    fn grep(
        _pattern: String,
        _path: String,
    ) -> Result<Vec<exports::brio::tools::tool_grep::GrepResult>, String> {
        Ok(Vec::new())
    }
}
