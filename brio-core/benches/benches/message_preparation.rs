//! Benchmarks for message preparation in inference providers
//!
//! Performance-critical paths:
//! - `AnthropicProvider::prepare_messages`: Message format conversion
//! - Message serialization/deserialization
//! - Token counting estimation
//! - Role mapping and content extraction

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};

/// Mirrors brio-core/kernel/src/inference/types.rs
#[derive(Debug, Clone)]
pub enum Role {
    System,
    User,
    Assistant,
}

#[derive(Debug, Clone)]
pub struct Message {
    pub role: Role,
    pub content: String,
}

/// Simulates Anthropic message preparation
fn prepare_messages_anthropic(messages: &[Message]) -> (Option<String>, Vec<AnthropicMessage>) {
    let mut system_message = None;
    let mut anthropic_messages = Vec::with_capacity(messages.len());

    for msg in messages {
        match msg.role {
            Role::System => {
                system_message = Some(msg.content.clone());
            }
            Role::User => {
                anthropic_messages.push(AnthropicMessage {
                    role: "user".to_string(),
                    content: msg.content.clone(),
                });
            }
            Role::Assistant => {
                anthropic_messages.push(AnthropicMessage {
                    role: "assistant".to_string(),
                    content: msg.content.clone(),
                });
            }
        }
    }

    (system_message, anthropic_messages)
}

#[derive(Debug, Clone)]
pub struct AnthropicMessage {
    pub role: String,
    pub content: String,
}

/// Simulates `OpenAI` message preparation (mostly passthrough)
fn prepare_messages_openai(messages: &[Message]) -> Vec<OpenAIMessage> {
    messages
        .iter()
        .map(|msg| OpenAIMessage {
            role: match msg.role {
                Role::System => "system",
                Role::User => "user",
                Role::Assistant => "assistant",
            }
            .to_string(),
            content: msg.content.clone(),
        })
        .collect()
}

#[derive(Debug, Clone)]
pub struct OpenAIMessage {
    pub role: String,
    pub content: String,
}

fn bench_prepare_messages(c: &mut Criterion) {
    let mut group = c.benchmark_group("message_preparation/anthropic");

    // Different conversation sizes
    let sizes = [1usize, 5, 10, 50];

    for size in sizes {
        let messages: Vec<Message> = (0..size)
            .map(|i| {
                let role = match i % 3 {
                    0 => Role::System,
                    1 => Role::User,
                    _ => Role::Assistant,
                };
                Message {
                    role,
                    content: format!("Message content for message {i} with some text here"),
                }
            })
            .collect();

        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{size}_msgs")),
            &messages,
            |b, msgs| b.iter(|| prepare_messages_anthropic(black_box(msgs))),
        );
    }

    group.finish();
}

fn bench_prepare_messages_openai(c: &mut Criterion) {
    let mut group = c.benchmark_group("message_preparation/openai");

    let sizes = [1usize, 5, 10, 50];

    for size in sizes {
        let messages: Vec<Message> = (0..size)
            .map(|i| {
                let role = match i % 3 {
                    0 => Role::System,
                    1 => Role::User,
                    _ => Role::Assistant,
                };
                Message {
                    role,
                    content: format!("Message content for message {i} with some text here"),
                }
            })
            .collect();

        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{size}_msgs")),
            &messages,
            |b, msgs| b.iter(|| prepare_messages_openai(black_box(msgs))),
        );
    }

    group.finish();
}

fn bench_message_sizes(c: &mut Criterion) {
    let mut group = c.benchmark_group("message_preparation/content_sizes");

    let content_sizes = [
        ("short", 100usize),
        ("medium", 1000),
        ("long", 10000),
        ("very_long", 100_000),
    ];

    for (name, size) in &content_sizes {
        let content = "x".repeat(*size);
        let messages = vec![
            Message {
                role: Role::System,
                content: "System prompt".to_string(),
            },
            Message {
                role: Role::User,
                content: content.clone(),
            },
        ];

        group.bench_with_input(BenchmarkId::from_parameter(*name), &messages, |b, msgs| {
            b.iter(|| prepare_messages_anthropic(black_box(msgs)));
        });
    }

    group.finish();
}

/// Naive token estimation (roughly 4 chars per token on average)
fn estimate_tokens(content: &str) -> usize {
    content.len() / 4
}

/// More accurate token estimation
fn estimate_tokens_precise(content: &str) -> usize {
    // Split by whitespace and punctuation
    let tokens: Vec<&str> = content
        .split(|c: char| c.is_whitespace() || c.is_ascii_punctuation())
        .filter(|s| !s.is_empty())
        .collect();
    tokens.len()
}

fn bench_token_counting(c: &mut Criterion) {
    let mut group = c.benchmark_group("message_preparation/token_count");

    let content =
        "This is a sample message with multiple words for token counting estimation. ".repeat(100);

    group.bench_function("naive_estimate", |b| {
        b.iter(|| estimate_tokens(black_box(&content)));
    });

    group.bench_function("precise_estimate", |b| {
        b.iter(|| estimate_tokens_precise(black_box(&content)));
    });

    group.finish();
}

fn bench_role_mapping(c: &mut Criterion) {
    let mut group = c.benchmark_group("message_preparation/role_mapping");

    // Test different role distribution patterns
    let patterns = [
        (
            "system_first",
            vec![
                (Role::System, "You are helpful."),
                (Role::User, "Hello!"),
                (Role::Assistant, "Hi there!"),
            ],
        ),
        (
            "alternating",
            vec![
                (Role::User, "Q1"),
                (Role::Assistant, "A1"),
                (Role::User, "Q2"),
                (Role::Assistant, "A2"),
            ],
        ),
        (
            "multi_system",
            vec![
                (Role::System, "Sys1"),
                (Role::System, "Sys2"),
                (Role::User, "Hello"),
            ],
        ),
    ];

    for (name, pattern) in patterns {
        let messages: Vec<Message> = pattern
            .into_iter()
            .map(|(role, content)| Message {
                role,
                content: content.to_string(),
            })
            .collect();

        group.bench_with_input(BenchmarkId::from_parameter(name), &messages, |b, msgs| {
            b.iter(|| prepare_messages_anthropic(black_box(msgs)));
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_prepare_messages,
    bench_prepare_messages_openai,
    bench_message_sizes,
    bench_token_counting,
    bench_role_mapping
);
criterion_main!(benches);
