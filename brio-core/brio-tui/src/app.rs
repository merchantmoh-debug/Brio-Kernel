use ratatui::widgets::ListState;

pub struct App {
    pub messages: Vec<String>,
    pub input: String,
    pub input_mode: InputMode,
    pub scroll_state: ListState,
    pub is_running: bool,
    pub connection_status: ConnectionStatus,
}

#[derive(PartialEq)]
pub enum InputMode {
    Normal,
    Editing,
}

#[derive(PartialEq)]
pub enum ConnectionStatus {
    Disconnected,
    Connecting,
    Connected,
    Error(String),
}

impl App {
    pub fn new() -> App {
        App {
            messages: vec![
                "Welcome to Brio - The Iron Interface".to_string(),
                "Type /help for commands.".to_string(),
            ],
            input: String::new(),
            input_mode: InputMode::Normal,
            scroll_state: ListState::default(),
            is_running: true,
            connection_status: ConnectionStatus::Disconnected,
        }
    }

    pub fn add_message(&mut self, msg: String) {
        self.messages.push(msg);
    }
}
