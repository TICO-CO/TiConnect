pub mod assets;
pub mod events;
pub mod protocol;
pub mod security;

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use axum::{
    Json, Router,
    body::Bytes,
    extract::{
        DefaultBodyLimit, Path, Query, State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::net::TcpListener;
use tokio::sync::{broadcast, mpsc, oneshot};

use crate::activation::ActivationEvent;
use crate::client::LinkHandle;
use ticonnect_core::ipc::events::DaemonEvent;

use self::events::WsEvent;
use self::protocol::{Reply, Request, RpcError};
use self::security::Origins;

const TICKET_TTL: Duration = Duration::from_secs(60);
const AUTH_TIMEOUT: Duration = Duration::from_secs(5);
const MAX_UPLOAD_BYTES: usize = 1024 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TicketKind {
    Attachment,
    Avatar,
}

#[derive(Debug, Clone)]
pub struct Ticket {
    pub id: String,
    pub kind: TicketKind,
    pub value: String,
    pub expires_at: u64,
}

pub struct BridgeState {
    pub(crate) link: LinkHandle,
    pub(crate) token: String,
    pub(crate) port: u16,
    pub(crate) origins: Origins,
    pub(crate) events: broadcast::Sender<WsEvent>,
    tickets: Mutex<HashMap<String, Ticket>>,
    online: AtomicBool,
}

impl BridgeState {
    pub fn online(&self) -> bool {
        self.online.load(Ordering::Relaxed)
    }

    pub fn mint_ticket(&self, kind: TicketKind, value: String) -> anyhow::Result<Ticket> {
        let id = security::generate_token()?;
        let expires_at = now_seconds() + TICKET_TTL.as_secs();
        let ticket = Ticket {
            id: id.clone(),
            kind,
            value,
            expires_at,
        };
        let mut tickets = self.tickets.lock().expect("ticket store");
        tickets.retain(|_, ticket| ticket.expires_at > now_seconds());
        tickets.insert(id, ticket.clone());
        Ok(ticket)
    }

    pub fn ticket_url(&self, ticket: &Ticket) -> String {
        let path = match ticket.kind {
            TicketKind::Attachment => format!("/attachment/{}", ticket.value),
            TicketKind::Avatar => format!("/avatar/{}", ticket.value),
        };
        format!(
            "http://127.0.0.1:{}{}?ticket={}",
            self.port, path, ticket.id
        )
    }

    fn take_ticket(&self, id: &str, kind: TicketKind, value: &str) -> Option<Ticket> {
        let mut tickets = self.tickets.lock().expect("ticket store");
        let ticket = tickets.remove(id)?;
        if ticket.kind != kind || ticket.value != value || ticket.expires_at < now_seconds() {
            return None;
        }
        Some(ticket)
    }

    fn authorized(&self, headers: &HeaderMap) -> bool {
        if self.origins.dev_mode() {
            return true;
        }
        security::bearer(headers) == Some(self.token.as_str())
    }

    fn request_allowed(&self, headers: &HeaderMap) -> bool {
        security::host_is_loopback(headers, self.port)
    }
}

pub struct Bridge {
    pub token: String,
    pub port: u16,
    shutdown: Option<oneshot::Sender<()>>,
    task: tokio::task::JoinHandle<()>,
}

impl Bridge {
    pub fn url(&self) -> String {
        format!("http://127.0.0.1:{}/#token={}", self.port, self.token)
    }

    pub fn dev_url() -> Option<String> {
        std::env::var("TICONNECT_DEV_URL")
            .ok()
            .map(|url| url.trim_end_matches('/').to_owned())
            .filter(|url| url == "http://127.0.0.1:5173")
    }
}

impl Drop for Bridge {
    fn drop(&mut self) {
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }
        self.task.abort();
    }
}

pub async fn start(
    link: LinkHandle,
    daemon_events: mpsc::UnboundedReceiver<DaemonEvent>,
    activation: broadcast::Receiver<ActivationEvent>,
) -> anyhow::Result<Bridge> {
    let token = security::generate_token()?;
    let listener = TcpListener::bind(("127.0.0.1", 0)).await?;
    let port = listener.local_addr()?.port();
    let (events, _) = broadcast::channel(256);
    let state = Arc::new(BridgeState {
        link,
        token: token.clone(),
        port,
        origins: Origins::new(port),
        events: events.clone(),
        tickets: Mutex::new(HashMap::new()),
        online: AtomicBool::new(false),
    });

    let app = router(state.clone());
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let server = axum::serve(listener, app).with_graceful_shutdown(async move {
        let _ = shutdown_rx.await;
    });
    let task = tokio::spawn(async move {
        if let Err(error) = server.await {
            tracing::warn!(%error, "bridge server stopped");
        }
    });

    spawn_fanout(state, daemon_events, activation);

    Ok(Bridge {
        token,
        port,
        shutdown: Some(shutdown_tx),
        task,
    })
}

fn router(state: Arc<BridgeState>) -> Router {
    Router::new()
        .route("/", get(index))
        .route("/assets/{*path}", get(asset))
        .route("/ws", get(websocket))
        .route("/attachment", post(upload_attachment))
        .route("/attachment/{hash}", get(download_attachment))
        .route("/avatar", post(upload_avatar))
        .route("/avatar/{fingerprint}", get(download_avatar))
        .fallback(index)
        .layer(DefaultBodyLimit::max(MAX_UPLOAD_BYTES))
        .with_state(state)
}

async fn index(State(state): State<Arc<BridgeState>>, headers: HeaderMap) -> Response {
    if !state.request_allowed(&headers) {
        return forbidden();
    }
    assets::index()
}

async fn asset(
    State(state): State<Arc<BridgeState>>,
    headers: HeaderMap,
    Path(path): Path<String>,
) -> Response {
    if !state.request_allowed(&headers) {
        return forbidden();
    }
    assets::asset(&format!("assets/{path}"))
}

async fn websocket(
    State(state): State<Arc<BridgeState>>,
    headers: HeaderMap,
    upgrade: WebSocketUpgrade,
) -> Response {
    if !state.request_allowed(&headers) {
        return forbidden();
    }
    if !state.origins.allows(security::origin(&headers)) {
        return forbidden();
    }
    upgrade.on_upgrade(move |socket| handle_socket(state, socket))
}

#[derive(Deserialize)]
struct AuthMessage {
    #[serde(default)]
    token: Option<String>,
}

async fn handle_socket(state: Arc<BridgeState>, socket: WebSocket) {
    let (mut sender, mut receiver) = socket.split();

    let auth = tokio::time::timeout(AUTH_TIMEOUT, receiver.next()).await;
    let token = match auth {
        Ok(Some(Ok(Message::Text(text)))) => serde_json::from_str::<AuthMessage>(text.as_str())
            .ok()
            .and_then(|auth| auth.token),
        _ => None,
    };
    if !state.origins.dev_mode() && token.as_deref() != Some(state.token.as_str()) {
        let _ = sender
            .send(Message::Text(
                WsEvent::Error {
                    code: "unauthorized".to_owned(),
                    message: "invalid bridge token".to_owned(),
                }
                .encode()
                .into(),
            ))
            .await;
        return;
    }

    let mut events = state.events.subscribe();
    let _ = sender
        .send(Message::Text(WsEvent::Ready.encode().into()))
        .await;

    loop {
        tokio::select! {
            incoming = receiver.next() => {
                let Some(Ok(message)) = incoming else { break };
                if let Message::Text(text) = message {
                    let reply = match serde_json::from_str::<Request>(text.as_str()) {
                        Ok(request) => protocol::dispatch(&state, request).await,
                        Err(error) => Reply::error(Value::Null, RpcError::invalid_params(error.to_string())),
                    };
                    if sender.send(Message::Text(reply.encode().into())).await.is_err() {
                        break;
                    }
                }
            }
            event = events.recv() => {
                match event {
                    Ok(event) => {
                        if sender.send(Message::Text(event.encode().into())).await.is_err() {
                            break;
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(_)) => {
                        let event = WsEvent::Error {
                            code: "lagged".to_owned(),
                            message: "client fell behind the event stream".to_owned(),
                        };
                        if sender.send(Message::Text(event.encode().into())).await.is_err() {
                            break;
                        }
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
        }
    }
}

#[derive(Deserialize)]
struct UploadQuery {
    user_id: String,
    #[serde(default)]
    name: Option<String>,
}

async fn upload_attachment(
    State(state): State<Arc<BridgeState>>,
    headers: HeaderMap,
    Query(query): Query<UploadQuery>,
    body: Bytes,
) -> Response {
    if !state.request_allowed(&headers) || !state.authorized(&headers) {
        return forbidden();
    }
    let path = temp_path(query.name.as_deref().unwrap_or("attachment"));
    if let Err(error) = write_private(&path, &body) {
        return http_error(StatusCode::INTERNAL_SERVER_ERROR, error.to_string());
    }
    let result = state
        .link
        .send_attachment(query.user_id, path.to_string_lossy().into_owned())
        .await;
    let _ = tokio::fs::remove_file(&path).await;
    match result {
        Ok(message_id) => Json(json!({ "message_id": message_id })).into_response(),
        Err(error) => http_error(StatusCode::BAD_GATEWAY, error.to_string()),
    }
}

async fn upload_avatar(
    State(state): State<Arc<BridgeState>>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    if !state.request_allowed(&headers) || !state.authorized(&headers) {
        return forbidden();
    }
    match state.link.set_avatar(body.to_vec()).await {
        Ok(()) => Json(json!({ "ok": true })).into_response(),
        Err(error) => http_error(StatusCode::BAD_GATEWAY, error.to_string()),
    }
}

#[derive(Deserialize)]
struct TicketQuery {
    ticket: String,
}

async fn download_attachment(
    State(state): State<Arc<BridgeState>>,
    headers: HeaderMap,
    Path(hash): Path<String>,
    Query(query): Query<TicketQuery>,
) -> Response {
    if !state.request_allowed(&headers) {
        return forbidden();
    }
    if state
        .take_ticket(&query.ticket, TicketKind::Attachment, &hash)
        .is_none()
    {
        return http_error(StatusCode::GONE, "attachment ticket expired");
    }
    let path = match state.link.attachment_path(hash).await {
        Ok(path) => path,
        Err(error) => return http_error(StatusCode::BAD_GATEWAY, error.to_string()),
    };
    let bytes = match tokio::fs::read(&path).await {
        Ok(bytes) => bytes,
        Err(error) => return http_error(StatusCode::NOT_FOUND, error.to_string()),
    };
    let name = std::path::Path::new(&path)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("attachment");
    let mime = mime_guess::from_path(&path).first_or_octet_stream();
    download_response(bytes, mime.as_ref(), name)
}

async fn download_avatar(
    State(state): State<Arc<BridgeState>>,
    headers: HeaderMap,
    Path(fingerprint): Path<String>,
    Query(query): Query<TicketQuery>,
) -> Response {
    if !state.request_allowed(&headers) {
        return forbidden();
    }
    if state
        .take_ticket(&query.ticket, TicketKind::Avatar, &fingerprint)
        .is_none()
    {
        return http_error(StatusCode::GONE, "avatar ticket expired");
    }
    match state.link.get_avatar(fingerprint).await {
        Ok(bytes) => (
            [
                (header::CONTENT_TYPE, "image/png".to_owned()),
                (header::CACHE_CONTROL, "no-store".to_owned()),
            ],
            bytes,
        )
            .into_response(),
        Err(error) => http_error(StatusCode::BAD_GATEWAY, error.to_string()),
    }
}

fn download_response(bytes: Vec<u8>, mime: &str, name: &str) -> Response {
    let disposition = format!(
        "attachment; filename=\"{}\"",
        name.replace(['"', '\\', '\n', '\r'], "_")
    );
    (
        [
            (header::CONTENT_TYPE, mime.to_owned()),
            (header::CONTENT_DISPOSITION, disposition),
            (header::CACHE_CONTROL, "no-store".to_owned()),
        ],
        bytes,
    )
        .into_response()
}

fn spawn_fanout(
    state: Arc<BridgeState>,
    mut daemon_events: mpsc::UnboundedReceiver<DaemonEvent>,
    mut activation: broadcast::Receiver<ActivationEvent>,
) {
    tokio::spawn(async move {
        loop {
            tokio::select! {
                event = daemon_events.recv() => {
                    let Some(event) = event else { break };
                    if matches!(event, DaemonEvent::Connected) {
                        state.online.store(true, Ordering::Relaxed);
                    } else if matches!(event, DaemonEvent::Disconnected(_)) {
                        state.online.store(false, Ordering::Relaxed);
                    }
                    let _ = state.events.send(WsEvent::from_daemon(&event));
                }
                event = activation.recv() => {
                    match event {
                        Ok(event) => {
                            let _ = state.events.send(WsEvent::from_activation(&event));
                        }
                        Err(broadcast::error::RecvError::Lagged(_)) => continue,
                        Err(broadcast::error::RecvError::Closed) => break,
                    }
                }
            }
        }
    });
}

fn temp_path(name: &str) -> std::path::PathBuf {
    let safe: String = name
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '.' | '-' | '_') {
                character
            } else {
                '_'
            }
        })
        .take(80)
        .collect();
    let id = security::generate_token().unwrap_or_else(|_| "fallback".to_owned());
    std::env::temp_dir().join(format!("ticonnect-upload-{id}-{safe}"))
}

fn write_private(path: &std::path::Path, bytes: &[u8]) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(path)?;
        file.write_all(bytes)?;
        Ok(())
    }
    #[cfg(not(unix))]
    {
        std::fs::write(path, bytes)
    }
}

fn now_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default()
}

fn forbidden() -> Response {
    http_error(StatusCode::FORBIDDEN, "forbidden")
}

fn http_error(status: StatusCode, message: impl AsRef<str>) -> Response {
    let mut headers = HeaderMap::new();
    headers.insert(header::CACHE_CONTROL, "no-store".parse().unwrap());
    (status, headers, message.as_ref().to_owned()).into_response()
}
