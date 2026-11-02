use futures_util::{SinkExt, StreamExt};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::{Semaphore, broadcast, mpsc};
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::handshake::server::{ErrorResponse, Request, Response};
use tokio_tungstenite::tungstenite::http::StatusCode;
use tokio_tungstenite::tungstenite::http::header::{AUTHORIZATION, ORIGIN};
use tokio_tungstenite::tungstenite::protocol::WebSocketConfig;

use crate::core::config::{
    GATEWAY_ACCEPT_RETRY_DELAY_MS, GATEWAY_MAX_CONNECTIONS, GATEWAY_MAX_FRAME_BYTES,
    GATEWAY_MAX_MESSAGE_BYTES, GATEWAY_MAX_WRITE_BUFFER_BYTES,
};
use crate::gateway::protocol::{IncomingCommand, OutgoingEvent};

#[derive(Clone)]
enum CommandSender {
    Unbounded(mpsc::UnboundedSender<IncomingCommand>),
    Bounded(mpsc::Sender<IncomingCommand>),
}

impl CommandSender {
    fn try_send(&self, command: IncomingCommand) -> Result<(), GatewaySendError> {
        match self {
            Self::Unbounded(sender) => sender.send(command).map_err(|_| GatewaySendError::Closed),
            Self::Bounded(sender) => sender.try_send(command).map_err(|error| match error {
                mpsc::error::TrySendError::Full(_) => GatewaySendError::Full,
                mpsc::error::TrySendError::Closed(_) => GatewaySendError::Closed,
            }),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GatewaySendError {
    Full,
    Closed,
}

#[derive(Debug, Clone, Default)]
struct GatewaySecurity {
    token: Option<String>,
    allowed_origin: Option<String>,
}

impl GatewaySecurity {
    fn from_environment() -> Self {
        Self {
            token: std::env::var("CAPRI_GATEWAY_TOKEN")
                .ok()
                .filter(|token| !token.is_empty()),
            allowed_origin: std::env::var("CAPRI_GATEWAY_ALLOWED_ORIGIN")
                .ok()
                .filter(|origin| !origin.is_empty()),
        }
    }

    fn authorize(&self, request: &Request) -> bool {
        if let Some(origin) = request.headers().get(ORIGIN) {
            let Ok(origin) = origin.to_str() else {
                return false;
            };
            if self.allowed_origin.as_deref() != Some(origin) {
                return false;
            }
        }

        let Some(expected_token) = self.token.as_deref() else {
            return true;
        };
        request
            .headers()
            .get(AUTHORIZATION)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.strip_prefix("Bearer "))
            == Some(expected_token)
    }
}

// The tungstenite callback contract fixes the large HTTP error response type.
#[allow(clippy::result_large_err)]
fn authorize_handshake(
    security: &GatewaySecurity,
    request: &Request,
    response: Response,
) -> Result<Response, ErrorResponse> {
    if security.authorize(request) {
        Ok(response)
    } else {
        let mut error = ErrorResponse::new(Some("Unauthorized WebSocket client".to_string()));
        *error.status_mut() = StatusCode::UNAUTHORIZED;
        Err(error)
    }
}

pub struct WsGatewayServer {
    port: u16,
    cmd_sender: CommandSender,
    event_sender: broadcast::Sender<OutgoingEvent>,
    connection_slots: Arc<Semaphore>,
    security: GatewaySecurity,
}

impl WsGatewayServer {
    pub fn new(
        port: u16,
        cmd_sender: mpsc::UnboundedSender<IncomingCommand>,
        event_sender: broadcast::Sender<OutgoingEvent>,
    ) -> Self {
        Self {
            port,
            cmd_sender: CommandSender::Unbounded(cmd_sender),
            event_sender,
            connection_slots: Arc::new(Semaphore::new(GATEWAY_MAX_CONNECTIONS)),
            security: GatewaySecurity::from_environment(),
        }
    }

    pub fn with_bounded_channel(
        port: u16,
        cmd_sender: mpsc::Sender<IncomingCommand>,
        event_sender: broadcast::Sender<OutgoingEvent>,
    ) -> Self {
        Self {
            port,
            cmd_sender: CommandSender::Bounded(cmd_sender),
            event_sender,
            connection_slots: Arc::new(Semaphore::new(GATEWAY_MAX_CONNECTIONS)),
            security: GatewaySecurity::from_environment(),
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
                    tokio::time::sleep(std::time::Duration::from_millis(
                        GATEWAY_ACCEPT_RETRY_DELAY_MS,
                    ))
                    .await;
                    continue;
                }
            };
            let connection_permit = match self.connection_slots.clone().try_acquire_owned() {
                Ok(permit) => permit,
                Err(_) => {
                    eprintln!(
                        "[GATEWAY WARNING] Conexion rechazada: limite de {} clientes",
                        GATEWAY_MAX_CONNECTIONS
                    );
                    continue;
                }
            };
            let cmd_tx = self.cmd_sender.clone();
            let security = self.security.clone();
            let mut event_rx = self.event_sender.subscribe();

            tokio::spawn(async move {
                let _connection_permit = connection_permit;
                let websocket_config = WebSocketConfig {
                    max_message_size: Some(GATEWAY_MAX_MESSAGE_BYTES),
                    max_frame_size: Some(GATEWAY_MAX_FRAME_BYTES),
                    max_write_buffer_size: GATEWAY_MAX_WRITE_BUFFER_BYTES,
                    ..Default::default()
                };
                #[allow(clippy::result_large_err)]
                let handshake = tokio_tungstenite::accept_hdr_async_with_config(
                    stream,
                    move |request: &Request, response: Response| {
                        authorize_handshake(&security, request, response)
                    },
                    Some(websocket_config),
                )
                .await;
                if let Ok(mut ws_stream) = handshake {
                    println!("[GATEWAY] Nuevo cliente conectado desde {}", client_addr);

                    loop {
                        tokio::select! {
                            msg = ws_stream.next() => {
                                match msg {
                                    Some(Ok(Message::Text(text))) => {
                                        match serde_json::from_str::<IncomingCommand>(&text) {
                                            Ok(command) => {
                                                match cmd_tx.try_send(command) {
                                                    Ok(()) => {}
                                                    Err(GatewaySendError::Full) => {
                                                        eprintln!("[GATEWAY WARNING] Canal de comandos lleno; comando descartado");
                                                    }
                                                    Err(GatewaySendError::Closed) => break,
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
    use std::time::Duration;
    use tokio::sync::{broadcast, mpsc};
    use tokio_tungstenite::connect_async;
    use tokio_tungstenite::tungstenite::Message;

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn gateway_transports_commands_and_events_over_loopback() {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("test listener");
        let address = listener.local_addr().expect("listener address");
        let (cmd_tx, mut cmd_rx) = mpsc::channel(8);
        let (event_tx, _) = broadcast::channel(8);
        let server = WsGatewayServer::with_bounded_channel(0, cmd_tx, event_tx.clone());
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

    #[test]
    fn gateway_security_rejects_unapproved_browser_origins() {
        let request = Request::builder()
            .header(ORIGIN, "https://untrusted.example")
            .body(())
            .expect("test request");
        assert!(!GatewaySecurity::default().authorize(&request));
    }

    #[test]
    fn gateway_security_accepts_configured_origin_and_token() {
        let security = GatewaySecurity {
            token: Some("secret".to_string()),
            allowed_origin: Some("http://127.0.0.1:3000".to_string()),
        };
        let request = Request::builder()
            .header(ORIGIN, "http://127.0.0.1:3000")
            .header(AUTHORIZATION, "Bearer secret")
            .body(())
            .expect("test request");
        assert!(security.authorize(&request));
    }

    #[tokio::test]
    async fn bounded_command_sender_reports_full_and_closed() {
        let (sender, mut receiver) = mpsc::channel(1);
        let sender = CommandSender::Bounded(sender);
        sender
            .try_send(IncomingCommand::LipSync { viseme: 1 })
            .expect("first command");
        assert_eq!(
            sender.try_send(IncomingCommand::LipSync { viseme: 2 }),
            Err(GatewaySendError::Full)
        );

        receiver.recv().await.expect("queued command");
        drop(receiver);
        assert_eq!(
            sender.try_send(IncomingCommand::LipSync { viseme: 3 }),
            Err(GatewaySendError::Closed)
        );
    }
}
