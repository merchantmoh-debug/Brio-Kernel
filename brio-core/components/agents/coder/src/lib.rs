use anyhow::Result;
use wit_bindgen::generate;

generate!({
    world: "smart-agent",
    path: "../../../wit",
    skip: ["tool"],
    generate_all,
});

mod engine;
mod prompt;
mod tools;

use brio::ai::inference::{Message, Role};
use engine::AgentEngine;

struct CoderAgent;

impl exports::brio::core::agent_runner::Guest for CoderAgent {
    fn run(context: exports::brio::core::agent_runner::TaskContext) -> Result<String, String> {
        let mut engine = AgentEngine::new(context);
        engine.run().map_err(|e| e.to_string())
    }
}

impl exports::brio::core::event_handler::Guest for CoderAgent {
    fn handle_event(_topic: String, _data: exports::brio::core::event_handler::Payload) {
        // Event handling can be another component if needed
        todo!("Implement asynchronous event handling when the Message Bus specs are finalized");
    }
}

pub struct AgentState {
    history: Vec<Message>,
}

impl AgentState {
    fn new(system_prompt: String) -> Self {
        Self {
            history: vec![Message {
                role: Role::System,
                content: system_prompt,
            }],
        }
    }

    fn add_user_message(&mut self, content: String) {
        self.history.push(Message {
            role: Role::User,
            content,
        });
    }

    fn add_assistant_message(&mut self, content: String) {
        self.history.push(Message {
            role: Role::Assistant,
            content,
        });
    }
}

export!(CoderAgent);
