use futures_util::{SinkExt, StreamExt};
use std::net::SocketAddr;
use tokio::net::TcpListener;
use tokio::sync::{broadcast, mpsc};
use tokio_tungstenite::tungstenite::Message;

use crate::gateway::protocol::{IncomingCommand, OutgoingEvent};

pub struct WsGatewayServer {
    port: u16,
    cmd_sender: mpsc::UnboundedSender<IncomingCommand>,
    event_sender: broadcast::Sender<OutgoingEvent>,
}

impl WsGatewayServer {
    pub fn new(
        port: u16,
        cmd_sender: mpsc::UnboundedSender<IncomingCommand>,
        event_sender: broadcast::Sender<OutgoingEvent>,
    ) -> Self {
        Self {
            port,
            cmd_sender,
            event_sender,
        }
    }

    pub async fn run(self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let addr: SocketAddr = format!("127.0.0.1:{}", self.port).parse()?;
        let listener = TcpListener::bind(&addr).await?;
        println!("[GATEWAY] Servidor WebSocket activo en ws://{}", addr);

        while let Ok((stream, client_addr)) = listener.accept().await {
            let cmd_tx = self.cmd_sender.clone();
            let mut event_rx = self.event_sender.subscribe();

            tokio::spawn(async move {
                if let Ok(mut ws_stream) = tokio_tungstenite::accept_async(stream).await {
                    println!("[GATEWAY] Nuevo cliente conectado desde {}", client_addr);

                    loop {
                        tokio::select! {
                            msg = ws_stream.next() => {
                                match msg {
                                    Some(Ok(Message::Text(text))) => {
                                        if let Ok(cmd) = serde_json::from_str::<IncomingCommand>(&text) {
                                            let _ = cmd_tx.send(cmd);
                                        }
                                    }
                                    Some(Ok(Message::Close(_))) | None => break,
                                    _ => {}
                                }
                            }
                            Ok(event) = event_rx.recv() => {
                                if let Ok(json) = serde_json::to_string(&event)
                                    && ws_stream.send(Message::Text(json)).await.is_err()
                                {
                                    break;
                                }
                            }
                        }
                    }

                    println!("[GATEWAY] Cliente desconectado: {}", client_addr);
                }
            });
        }

        Ok(())
    }
}
