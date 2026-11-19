use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use ticonnect_ui::activation::ActivationEvent;
use ticonnect_ui::bridge;
use ticonnect_ui::client::LinkHandle;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::{broadcast, mpsc};
use tokio_tungstenite::{connect_async, tungstenite::client::IntoClientRequest};

async fn start_bridge() -> bridge::Bridge {
    let (_daemon_tx, daemon_rx) = mpsc::unbounded_channel();
    let (activation_tx, _) = broadcast::channel::<ActivationEvent>(8);
    bridge::start(LinkHandle::offline(), daemon_rx, activation_tx.subscribe())
        .await
        .expect("bridge starts")
}

async fn http_get(port: u16, host: &str) -> String {
    http_get_path(port, host, "/").await
}

async fn http_get_path(port: u16, host: &str, path: &str) -> String {
    let mut stream = tokio::net::TcpStream::connect(("127.0.0.1", port))
        .await
        .expect("connect");
    let request = format!("GET {path} HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\n\r\n");
    stream.write_all(request.as_bytes()).await.expect("write");
    let mut response = String::new();
    stream.read_to_string(&mut response).await.expect("read");
    response
}

async fn socket(
    port: u16,
    origin: Option<&str>,
) -> Result<
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
    tokio_tungstenite::tungstenite::Error,
> {
    let mut request = format!("ws://127.0.0.1:{port}/ws")
        .into_client_request()
        .expect("request");
    if let Some(origin) = origin {
        request
            .headers_mut()
            .insert("origin", origin.parse().expect("origin"));
    }
    connect_async(request).await.map(|(socket, _)| socket)
}

async fn next_json(
    socket: &mut tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
) -> serde_json::Value {
    let message = tokio::time::timeout(Duration::from_secs(2), socket.next())
        .await
        .expect("event timeout")
        .expect("event")
        .expect("event ok");
    match message {
        tokio_tungstenite::tungstenite::Message::Text(text) => {
            serde_json::from_str(text.as_str()).expect("json event")
        }
        other => panic!("unexpected message: {other:?}"),
    }
}

#[tokio::test]
async fn bridge_binds_loopback_with_a_random_token() {
    let bridge = start_bridge().await;
    assert_ne!(bridge.port, 0);
    assert_eq!(bridge.token.len(), 32);
    assert!(bridge.url().starts_with("http://127.0.0.1:"));
    assert!(bridge.url().contains("#token="));
}

#[tokio::test]
async fn asset_requests_require_the_loopback_host() {
    let bridge = start_bridge().await;
    let response = http_get(bridge.port, "example.com").await;
    assert!(response.starts_with("HTTP/1.1 403"), "{response}");
    let response = http_get(bridge.port, &format!("127.0.0.1:{}", bridge.port)).await;
    assert!(response.starts_with("HTTP/1.1 200"), "{response}");
    assert!(response.contains("<title>TiConnect</title>"), "{response}");
    assert!(!response.contains("bundle missing"), "{response}");

    let asset = response
        .split("src=\"")
        .nth(1)
        .and_then(|value| value.split('\"').next())
        .expect("script asset path");
    let response = http_get_path(bridge.port, &format!("127.0.0.1:{}", bridge.port), asset).await;
    assert!(response.starts_with("HTTP/1.1 200"), "{response}");
    assert!(!response.contains("not found"), "{response}");
}

#[tokio::test]
async fn websocket_rejects_bad_origin_and_bad_token() {
    let bridge = start_bridge().await;
    assert!(
        socket(bridge.port, Some("http://evil.example"))
            .await
            .is_err()
    );

    let origin = format!("http://127.0.0.1:{}", bridge.port);
    let mut socket = socket(bridge.port, Some(&origin)).await.expect("connect");
    socket
        .send(tokio_tungstenite::tungstenite::Message::Text(
            r#"{"type":"auth","token":"wrong"}"#.into(),
        ))
        .await
        .expect("send auth");
    let event = next_json(&mut socket).await;
    assert_eq!(event["type"], "error");
    assert_eq!(event["code"], "unauthorized");
}

#[tokio::test]
async fn websocket_dispatches_rpc_and_reports_daemon_unavailable() {
    let bridge = start_bridge().await;
    let origin = format!("http://127.0.0.1:{}", bridge.port);
    let mut socket = socket(bridge.port, Some(&origin)).await.expect("connect");
    let auth = format!(r#"{{"type":"auth","token":"{}"}}"#, bridge.token);
    socket
        .send(tokio_tungstenite::tungstenite::Message::Text(auth.into()))
        .await
        .expect("send auth");
    assert_eq!(next_json(&mut socket).await["type"], "ready");

    socket
        .send(tokio_tungstenite::tungstenite::Message::Text(
            r#"{"id":1,"method":"nope","params":{}}"#.into(),
        ))
        .await
        .expect("send unknown");
    let reply = next_json(&mut socket).await;
    assert_eq!(reply["ok"], false);
    assert_eq!(reply["error"]["code"], "unknown_method");

    socket
        .send(tokio_tungstenite::tungstenite::Message::Text(
            r#"{"id":2,"method":"identity","params":{}}"#.into(),
        ))
        .await
        .expect("send identity");
    let reply = next_json(&mut socket).await;
    assert_eq!(reply["ok"], false);
    assert_eq!(reply["error"]["code"], "daemon_unavailable");

    socket
        .send(tokio_tungstenite::tungstenite::Message::Text(
            r#"{"id":3,"method":"send_message","params":{}}"#.into(),
        ))
        .await
        .expect("send bad params");
    let reply = next_json(&mut socket).await;
    assert_eq!(reply["ok"], false);
    assert_eq!(reply["error"]["code"], "invalid_params");

    socket
        .send(tokio_tungstenite::tungstenite::Message::Text(
            r#"{"id":4,"method":"attachment_ticket","params":{"hash":"abc"}}"#.into(),
        ))
        .await
        .expect("send ticket");
    let reply = next_json(&mut socket).await;
    assert_eq!(reply["ok"], true);
    assert!(
        reply["result"]["url"]
            .as_str()
            .unwrap()
            .contains("/attachment/abc?ticket=")
    );

    socket
        .send(tokio_tungstenite::tungstenite::Message::Text(
            r#"{"id":5,"method":"clear_history","params":{"user_id":""}}"#.into(),
        ))
        .await
        .expect("send clear history");
    let reply = next_json(&mut socket).await;
    assert_eq!(reply["ok"], false);
    assert_eq!(reply["error"]["code"], "daemon_unavailable");

    socket
        .send(tokio_tungstenite::tungstenite::Message::Text(
            r#"{"id":6,"method":"storage_usage","params":{}}"#.into(),
        ))
        .await
        .expect("send storage usage");
    let reply = next_json(&mut socket).await;
    assert_eq!(reply["ok"], false);
    assert_eq!(reply["error"]["code"], "daemon_unavailable");
}
