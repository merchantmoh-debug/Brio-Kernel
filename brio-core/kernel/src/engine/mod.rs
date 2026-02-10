//! WebAssembly engine components for the Brio kernel.
//!
//! This module provides the core WASM runtime functionality including
//! the component linker, runner, and runtime management.

pub mod linker;
pub mod runner;
pub mod runtime;

pub use linker::{create_engine_config, create_linker};
pub use runtime::WasmEngine;

// WIT bindings module - generated code allows missing docs
#[allow(missing_docs)]
mod wit_bindings {
    wasmtime::component::bindgen!({
        inline: r#"
            package brio:core;

            interface service-mesh {
                variant payload {
                    json(string),
                    binary(list<u8>)
                }
                call: func(target: string, method: string, args: payload) -> result<payload, string>;
            }

            interface sql-state {
                record row {
                    columns: list<string>,
                    values: list<string>
                }
                query: func(sql: string, params: list<string>) -> result<list<row>, string>;
                execute: func(sql: string, params: list<string>) -> result<u32, string>;
            }

            interface session-fs {
                begin-session: func(base-path: string) -> result<string, string>;
                commit-session: func(session-id: string) -> result<tuple<>, string>;
            }

            interface inference {
                 variant role { system, user, assistant }
                 record message { role: role, content: string }
                 record usage { prompt-tokens: u32, completion-tokens: u32, total-tokens: u32 }
                 record completion-response { content: string, usage: option<usage> }
                 variant inference-error { provider-error(string), rate-limit, context-length-exceeded }
                 chat: func(model: string, messages: list<message>) -> result<completion-response, inference-error>;
            }

            interface logging {
                enum level { trace, debug, info, warn, error }
                log: func(level: level, context: string, message: string);
            }

            interface pub-sub {
                 use service-mesh.{payload};
                 subscribe: func(topic: string) -> result<tuple<>, string>;
                 publish: func(topic: string, data: payload) -> result<tuple<>, string>;
            }

            world brio-host {
                import service-mesh;
                import sql-state;
                import session-fs;
                import inference;
                import logging;
                import pub-sub;
            }
        "#,
    });
}

pub use wit_bindings::*;
