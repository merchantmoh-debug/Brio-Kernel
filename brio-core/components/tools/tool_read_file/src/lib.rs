use wit_bindgen::generate;

generate!({
    world: "tool-runner",
    path: "../../../wit",
    exports: {
        "brio:tools/tool-read-file": ReadFileTool,
    },
});

struct ReadFileTool;

impl exports::brio::tools::tool_read_file::Guest for ReadFileTool {
    fn read_file(_path: String) -> Result<String, String> {
        Err("read_file not yet implemented".to_string())
    }

    fn read_file_range(_path: String, _start_line: u32, _end_line: u32) -> Result<String, String> {
        Err("read_file_range not yet implemented".to_string())
    }
}
