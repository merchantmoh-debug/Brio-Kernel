use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use std::{error::Error, io, time::Duration};
use ratatui::{
    backend::{Backend, CrosstermBackend},
    Terminal,
};
use tokio::sync::mpsc;
use serde_json::to_string;

mod app;
mod network;
mod messages;
mod ui;

use app::{App, ConnectionStatus, InputMode};
use network::{Network, NetworkEvent};
use messages::{ClientMessage, SessionAction, SessionParams};
use ui::ui;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // create app and run it
    let mut app = App::new();
    
    // Setup Network
    let (tx, mut rx) = mpsc::channel(100);
    // Channel for outgoing messages (UI -> WS)
    let (tx_outgoing, rx_outgoing) = mpsc::channel(100);

    let network = Network::new(tx);
    let network_handle = tokio::spawn(async move {
        network.connect("ws://127.0.0.1:9090/ws", rx_outgoing).await;
    });

    app.connection_status = ConnectionStatus::Connecting;

    let res = run_app(&mut terminal, app, &mut rx, tx_outgoing).await;

    // restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;
    
    // cleanup
    network_handle.abort();

    if let Err(err) = res {
        println!("{:?}", err)
    }

    Ok(())
}

async fn run_app<B: Backend>(
    terminal: &mut Terminal<B>, 
    mut app: App, 
    rx: &mut mpsc::Receiver<NetworkEvent>,
    tx_outgoing: mpsc::Sender<String>,
) -> io::Result<()> {
    loop {
        terminal.draw(|f| ui(f, &mut app))?;

        // Manual event loop with timeout to allow checking the network channel
        if event::poll(Duration::from_millis(16))? { // ~60fps
             if let Event::Key(key) = event::read()? {
                match app.input_mode {
                    InputMode::Normal => match key.code {
                        KeyCode::Char('e') => {
                            app.input_mode = InputMode::Editing;
                        }
                        KeyCode::Char('q') => {
                            return Ok(());
                        }
                        _ => {}
                    },
                    InputMode::Editing => match key.code {
                        KeyCode::Enter => {
                            let input_text = app.input.clone();
                            app.add_message(format!("You: {}", input_text));
                            app.input.clear();
                            
                            // Parse and Serialize
                            let msg = if let Some(sql) = input_text.strip_prefix("/sql ") {
                                ClientMessage::Query { sql: sql.to_string() }
                            } else if input_text.starts_with("/begin") {
                                // Simple Session Begin example
                                ClientMessage::Session { 
                                    action: SessionAction::Begin, 
                                    params: SessionParams { base_path: Some("./src".into()), session_id: None } 
                                }
                            } else {
                                ClientMessage::Task { content: input_text.to_string() }
                            };

                            if let Ok(json_str) = to_string(&msg) {
                                let _ = tx_outgoing.send(json_str).await;
                            } else {
                                app.add_message("Error: Failed to serialize message.".into());
                            }
                        }
                        KeyCode::Char(c) => {
                            app.input.push(c);
                        }
                        KeyCode::Backspace => {
                            app.input.pop();
                        }
                        KeyCode::Esc => {
                            app.input_mode = InputMode::Normal;
                        }
                        _ => {}
                    },
                }
            }
        }

        // Process network events
        while let Ok(net_event) = rx.try_recv() {
            match net_event {
                NetworkEvent::MessageReceived(msg) => {
                    app.add_message(format!("Kernel: {}", msg));
                }
                NetworkEvent::ConnectionEstablished => {
                    app.connection_status = ConnectionStatus::Connected;
                    app.add_message("Connected to Brio Kernel.".into());
                }
                NetworkEvent::ConnectionError(err) => {
                    app.connection_status = ConnectionStatus::Error(err.clone());
                    app.add_message(format!("Connection Error: {}", err));
                }
                NetworkEvent::ConnectionClosed => {
                    app.connection_status = ConnectionStatus::Disconnected;
                    app.add_message("Connection Closed.".into());
                }
            }
        }
    }
}
