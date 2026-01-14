pub mod linker;
pub mod runtime;

pub use linker::{create_engine_config, create_linker};
pub use runtime::WasmEngine;

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

        world brio-host {
            import service-mesh;
            import sql-state;
            import session-fs;
            import inference;
        }
    "#,
});
