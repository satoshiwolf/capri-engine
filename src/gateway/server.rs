use futures_util::{SinkExt, StreamExt};
use std::net::SocketAddr;
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::sync::{broadcast, mpsc};
use tokio_tungstenite::tungstenite::Message;

use crate::gateway::protocol::{IncomingCommand, OutgoingEvent};

const ACCEPT_RETRY_DELAY_MS: u64 = 100;

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
        let local_addr = listener.local_addr()?;
        println!("[GATEWAY] Servidor WebSocket activo en ws://{}", local_addr);
        self.serve(listener).await
    }

    async fn serve(
        self,
        listener: TcpListener,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        loop {
            let (stream, client_addr) = match listener.accept().await {
                Ok(connection) => connection,
                Err(error) => {
                    eprintln!("[GATEWAY ERROR] Error aceptando conexion: {error}");
                    tokio::time::sleep(Duration::from_millis(ACCEPT_RETRY_DELAY_MS)).await;
                    continue;
                }
            };
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
                                        match serde_json::from_str::<IncomingCommand>(&text) {
                                            Ok(command) => {
                                                if cmd_tx.send(command).is_err() {
                                                    break;
                                                }
                                            }
                                            Err(error) => {
                                                eprintln!("[GATEWAY ERROR] Comando JSON invalido: {error}");
                                            }
                                        }
                                    }
                                    Some(Ok(Message::Close(_))) | None => break,
                                    Some(Ok(Message::Ping(payload))) => {
                                        if ws_stream.send(Message::Pong(payload)).await.is_err() {
                                            break;
                                        }
                                    }
                                    Some(Err(error)) => {
                                        eprintln!("[GATEWAY ERROR] Cliente {client_addr}: {error}");
                                        break;
                                    }
                                    _ => {}
                                }
                            }
                            event = event_rx.recv() => {
                                match event {
                                    Ok(event) => {
                                        let json = match serde_json::to_string(&event) {
                                            Ok(json) => json,
                                            Err(error) => {
                                                eprintln!("[GATEWAY ERROR] Evento no serializable: {error}");
                                                continue;
                                            }
                                        };
                                        if ws_stream.send(Message::Text(json)).await.is_err() {
                                            break;
                                        }
                                    }
                                    Err(broadcast::error::RecvError::Lagged(skipped)) => {
                                        eprintln!("[GATEWAY WARNING] Cliente {client_addr} perdio {skipped} eventos");
                                    }
                                    Err(broadcast::error::RecvError::Closed) => break,
                                }
                            }
                        }
                    }

                    println!("[GATEWAY] Cliente desconectado: {}", client_addr);
                }
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures_util::{SinkExt, StreamExt};
    use tokio::sync::{broadcast, mpsc};
    use tokio_tungstenite::connect_async;
    use tokio_tungstenite::tungstenite::Message;

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn gateway_transports_commands_and_events_over_loopback() {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("test listener");
        let address = listener.local_addr().expect("listener address");
        let (cmd_tx, mut cmd_rx) = mpsc::unbounded_channel();
        let (event_tx, _) = broadcast::channel(8);
        let server = WsGatewayServer::new(0, cmd_tx, event_tx.clone());
        let server_task = tokio::spawn(server.serve(listener));

        let (mut socket, _) = connect_async(format!("ws://{address}"))
            .await
            .expect("connect gateway");
        socket
            .send(Message::Text(
                r#"{"type":"emotion","state":"happy"}"#.to_string(),
            ))
            .await
            .expect("send command");

        let command = tokio::time::timeout(Duration::from_secs(2), cmd_rx.recv())
            .await
            .expect("command timeout")
            .expect("command channel");
        assert!(matches!(command, IncomingCommand::Emotion { state } if state == "happy"));

        event_tx
            .send(OutgoingEvent::StateChanged {
                stance: "stand".to_string(),
            })
            .expect("send event");
        let event = tokio::time::timeout(Duration::from_secs(2), socket.next())
            .await
            .expect("event timeout")
            .expect("event stream")
            .expect("event message");
        assert!(matches!(event, Message::Text(text) if text.contains("state_changed")));

        server_task.abort();
    }
}
