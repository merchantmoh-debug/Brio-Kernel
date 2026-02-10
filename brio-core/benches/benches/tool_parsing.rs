//! Benchmarks for tool parsing in agent-sdk/src/tools.rs
//!
//! Performance-critical paths:
//! - `ToolParser::parse`: Regex-based tool invocation extraction
//! - `ToolRegistry::execute_all`: Multi-tool execution pipeline
//! - `validate_path`: Path traversal prevention
//! - `validate_shell_command`: Command allowlist validation

#![allow(missing_docs)]

use criterion::{BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main};
use regex::{Captures, Regex};
use std::collections::HashMap;

/// Simulated tool parser - mirrors agent-sdk/src/tools.rs
pub struct ToolParser {
    regex: Regex,
}

impl ToolParser {
    /// Creates a new `ToolParser` with the given regex pattern.
    ///
    /// # Errors
    /// Returns an error if the regex pattern is invalid.
    pub fn new(pattern: &str) -> Result<Self, regex::Error> {
        let regex = Regex::new(pattern)?;
        Ok(Self { regex })
    }

    #[must_use]
    pub fn parse(&self, input: &str) -> Vec<ParsedInvocation> {
        let mut results = Vec::new();

        for mat in self.regex.find_iter(input) {
            if let Some(caps) = self.regex.captures(mat.as_str()) {
                let args = Self::extract_args(&caps);
                results.push(ParsedInvocation {
                    name: caps
                        .get(1)
                        .map(|m| m.as_str().to_string())
                        .unwrap_or_default(),
                    args,
                    position: mat.start(),
                });
            }
        }

        results.sort_by_key(|inv| inv.position);
        results
    }

    fn extract_args(caps: &Captures) -> HashMap<String, String> {
        let mut args = HashMap::new();
        for (name, value) in caps.iter().skip(2).zip(caps.iter().skip(3)) {
            if let (Some(n), Some(v)) = (name, value) {
                args.insert(n.as_str().to_string(), v.as_str().to_string());
            }
        }
        args
    }
}

#[derive(Debug, Clone)]
pub struct ParsedInvocation {
    pub name: String,
    pub args: HashMap<String, String>,
    pub position: usize,
}

fn bench_parse_simple(c: &mut Criterion) {
    let mut group = c.benchmark_group("tool_parsing/parse_simple");

    let parser = ToolParser::new(r"<(\w+)(?:\s+(\w+)='([^']*)')*\s*/?>").unwrap();

    let test_inputs = [
        ("single_tool", "<tool name='test' />"),
        ("multiple_tools", "<tool1 /> <tool2 /> <tool3 />"),
        ("with_args", "<read_file path='/test/file.txt' />"),
        ("nested_text", "Some text here <tool /> more text"),
    ];

    for (name, input) in &test_inputs {
        group.bench_with_input(BenchmarkId::from_parameter(*name), *input, |b, i| {
            b.iter(|| parser.parse(black_box(i)));
        });
    }

    group.finish();
}

fn bench_parse_complex(c: &mut Criterion) {
    let mut group = c.benchmark_group("tool_parsing/parse_complex");

    // More complex patterns simulating real agent outputs
    let parser = ToolParser::new(r"<(\w+)(?:\s+(\w+)=(?:'([^']*)'|([^\s>]+)))*\s*/?>").unwrap();

    let large_input = "I'll help you with that. <read_file path='/src/main.rs' /> <search pattern='fn main' /> <write_file path='/src/lib.rs' content='pub fn add(a: i32, b: i32) -> i32 { a + b }' /> <done />";

    let very_large = large_input.repeat(10).clone();

    group.throughput(Throughput::Bytes(large_input.len() as u64));
    group.bench_with_input(
        BenchmarkId::from_parameter("complex_response"),
        large_input,
        |b, i| b.iter(|| parser.parse(black_box(i))),
    );

    group.throughput(Throughput::Bytes(very_large.len() as u64));
    group.bench_with_input(
        BenchmarkId::from_parameter("repeated_10x"),
        &very_large,
        |b, i| b.iter(|| parser.parse(black_box(i))),
    );

    group.finish();
}

fn bench_parse_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("tool_parsing/throughput");

    let parser = ToolParser::new(r"<(\w+)(?:\s+(\w+)='([^']*)')*\s*/?>").unwrap();

    // Varying sizes of input
    let sizes = [100usize, 1000, 10000];

    for size in sizes {
        // Generate input with tools scattered throughout
        let input = format!("Start {}<tool />{}", "x".repeat(size), " End");

        group.throughput(Throughput::Bytes(input.len() as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{size}_bytes")),
            &input,
            |b, i| b.iter(|| parser.parse(black_box(i))),
        );
    }

    group.finish();
}

fn validate_path(path: &str) -> Result<(), &str> {
    if path.contains("..") {
        return Err("path_traversal");
    }
    if path.starts_with('/') || path.starts_with("\\\\") {
        return Err("absolute_path");
    }
    Ok(())
}

fn bench_validate_path(c: &mut Criterion) {
    let mut group = c.benchmark_group("tool_parsing/validate_path");

    let test_paths = [
        ("valid_simple", "src/main.rs", true),
        ("valid_nested", "src/deep/nested/file.txt", true),
        ("path_traversal", "../../../etc/passwd", false),
        ("absolute_unix", "/etc/passwd", false),
        ("absolute_win", "C:\\Windows\\System32", false),
    ];

    for (name, path, _should_pass) in &test_paths {
        group.bench_with_input(BenchmarkId::from_parameter(*name), *path, |b, p| {
            b.iter(|| validate_path(black_box(p)));
        });
    }

    group.finish();
}

fn validate_shell_command<'a>(command: &'a str, allowlist: &[&'a str]) -> Result<(), &'a str> {
    let cmd_trimmed = command.trim();
    let first_word = cmd_trimmed.split_whitespace().next().unwrap_or(cmd_trimmed);

    let is_allowed = allowlist.contains(&first_word);
    if !is_allowed {
        return Err("not_allowed");
    }

    let dangerous_chars = [b';', b'&', b'|', b'>', b'<', b'`', b'$', b'('];
    if command.bytes().any(|c| dangerous_chars.contains(&c)) {
        return Err("dangerous_chars");
    }

    Ok(())
}

fn bench_validate_shell_command(c: &mut Criterion) {
    let mut group = c.benchmark_group("tool_parsing/validate_shell");

    let allowlist = &["ls", "cat", "echo", "grep", "pwd"];

    let test_commands = [
        ("allowed_simple", "ls -la", true),
        ("allowed_with_args", "grep -r pattern /path", true),
        ("not_allowed", "rm -rf /", false),
        ("dangerous_semicolon", "ls; rm -rf /", false),
        ("dangerous_pipe", "cat /etc/passwd | grep root", false),
    ];

    for (name, command, _should_pass) in &test_commands {
        group.bench_with_input(BenchmarkId::from_parameter(*name), *command, |b, cmd| {
            b.iter(|| validate_shell_command(black_box(cmd), black_box(allowlist)));
        });
    }

    group.finish();
}

fn bench_regex_compilation(c: &mut Criterion) {
    let mut group = c.benchmark_group("tool_parsing/regex_compile");

    let patterns = [
        ("simple_tool", r"\<(\w+)\s*/?\>"),
        ("tool_with_args", r"\<(\w+)(?:\s+(\w+)='([^']*)')*\s*/?\>"),
        ("complex_attrs", r"\<(\w+)(?:\s+\w+=[^\s\>]+)*\s*/?\>"),
    ];

    for (name, pattern) in &patterns {
        group.bench_with_input(BenchmarkId::from_parameter(*name), *pattern, |b, p| {
            b.iter(|| Regex::new(black_box(p)));
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_parse_simple,
    bench_parse_complex,
    bench_parse_throughput,
    bench_validate_path,
    bench_validate_shell_command,
    bench_regex_compilation
);
criterion_main!(benches);
