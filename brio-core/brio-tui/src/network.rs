use futures::{SinkExt, StreamExt};
use tokio::sync::mpsc;
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use url::Url;

pub enum NetworkEvent {
    MessageReceived(String),
    ConnectionEstablished,
    ConnectionError(String),
    ConnectionClosed,
}

pub struct Network {
    sender: mpsc::Sender<NetworkEvent>,
}

impl Network {
    pub fn new(sender: mpsc::Sender<NetworkEvent>) -> Self {
        Self { sender }
    }

    pub async fn connect(&self, url: &str, mut outgoing_rx: mpsc::Receiver<String>) {
        let url = match Url::parse(url) {
            Ok(u) => u,
            Err(e) => {
                let _ = self.sender.send(NetworkEvent::ConnectionError(e.to_string())).await;
                return;
            }
        };

        match connect_async(url.to_string()).await {
            Ok((ws_stream, _)) => {
                let _ = self.sender.send(NetworkEvent::ConnectionEstablished).await;
                let (mut write, mut read) = ws_stream.split();

                loop {
                    tokio::select! {
                        msg = read.next() => {
                            match msg {
                                Some(Ok(Message::Text(text))) => {
                                    let _ = self.sender.send(NetworkEvent::MessageReceived(text.to_string())).await;
                                }
                                Some(Ok(Message::Close(_))) => {
                                    let _ = self.sender.send(NetworkEvent::ConnectionClosed).await;
                                    break;
                                }
                                Some(Err(e)) => {
                                    let _ = self.sender.send(NetworkEvent::ConnectionError(e.to_string())).await;
                                    break;
                                }
                                None => {
                                    let _ = self.sender.send(NetworkEvent::ConnectionClosed).await;
                                    break;
                                }
                                _ => {}
                            }
                        }
                        outgoing = outgoing_rx.recv() => {
                            match outgoing {
                                Some(text) => {
                                    if let Err(e) = write.send(Message::Text(text.into())).await {
                                         let _ = self.sender.send(NetworkEvent::ConnectionError(e.to_string())).await;
                                         break;
                                    }
                                }
                                None => break, // Channel closed
                            }
                        }
                    }
                }
            }
            Err(e) => {
                let _ = self.sender.send(NetworkEvent::ConnectionError(e.to_string())).await;
            }
        }
    }
}
