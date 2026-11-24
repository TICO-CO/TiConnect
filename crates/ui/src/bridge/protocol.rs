use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::{Value, json};

use crate::bridge::BridgeState;
use crate::error::Error;

#[derive(Debug, Deserialize)]
pub struct Request {
    #[serde(default)]
    pub id: Value,
    pub method: String,
    #[serde(default)]
    pub params: Value,
}

#[derive(Debug, Serialize)]
pub struct Reply {
    pub id: Value,
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<RpcError>,
}

impl Reply {
    pub fn result(id: Value, result: Value) -> Self {
        Self {
            id,
            ok: true,
            result: Some(result),
            error: None,
        }
    }

    pub fn error(id: Value, error: RpcError) -> Self {
        Self {
            id,
            ok: false,
            result: None,
            error: Some(error),
        }
    }

    pub fn encode(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| {
            r#"{"id":null,"ok":false,"error":{"code":"serialize","message":"serialize reply"}}"#
                .to_owned()
        })
    }
}

#[derive(Debug, Serialize)]
pub struct RpcError {
    pub code: String,
    pub message: String,
}

impl RpcError {
    pub fn new(code: &str, message: impl Into<String>) -> Self {
        Self {
            code: code.to_owned(),
            message: message.into(),
        }
    }

    pub fn invalid_params(message: impl Into<String>) -> Self {
        Self::new("invalid_params", message)
    }

    pub fn unknown_method(method: &str) -> Self {
        Self::new("unknown_method", format!("unknown method: {method}"))
    }
}

impl From<Error> for RpcError {
    fn from(error: Error) -> Self {
        let code = match &error {
            Error::DaemonUnavailable(_) => "daemon_unavailable",
            Error::InvalidResponse(_) => "invalid_response",
            Error::Settings(_) => "settings_error",
            Error::Autostart(_) => "autostart_error",
        };
        Self::new(code, error.to_string())
    }
}

fn params<T: DeserializeOwned>(value: Value) -> Result<T, RpcError> {
    serde_json::from_value(value).map_err(|error| RpcError::invalid_params(error.to_string()))
}

pub async fn dispatch(state: &BridgeState, request: Request) -> Reply {
    let id = request.id.clone();
    match dispatch_inner(state, &request.method, request.params).await {
        Ok(result) => Reply::result(id, result),
        Err(error) => Reply::error(id, error),
    }
}

async fn dispatch_inner(
    state: &BridgeState,
    method: &str,
    value: Value,
) -> Result<Value, RpcError> {
    let link = &state.link;
    match method {
        "bootstrap" => {
            let identity = link.identity().await?;
            let status = link.status().await?;
            let network = link.network_status().await?;
            let settings = link.list_settings().await?;
            let autostart = crate::autostart::enabled();
            Ok(json!({
                "identity": identity,
                "status": status,
                "network": network,
                "settings": settings.values(),
                "appearance": settings.get("appearance.color_scheme").unwrap_or("system"),
                "autostart": autostart,
                "version": env!("CARGO_PKG_VERSION"),
                "online": state.online(),
            }))
        }
        "identity" => Ok(json!(link.identity().await?)),
        "status" => Ok(json!(link.status().await?)),
        "network_status" => Ok(json!(link.network_status().await?)),
        "run_lan_check" => Ok(json!(link.run_lan_check().await?)),
        "list_users" => Ok(json!(link.list_users().await?)),
        "get_user" => {
            let input: UserId = params(value)?;
            Ok(json!(link.get_user(input.user_id).await?))
        }
        "list_candidates" => Ok(json!(link.list_candidates().await?)),
        "list_conversation_summaries" => Ok(json!(link.list_conversation_summaries().await?)),
        "list_blocked_users" => Ok(json!(link.list_blocked_users().await?)),
        "list_settings" => {
            let settings = link.list_settings().await?;
            Ok(json!(settings.values()))
        }
        "settings_save" => {
            let input: KeyValue = params(value)?;
            save_setting(&input.key, &input.value)?;
            Ok(Value::Null)
        }
        "autostart_get" => Ok(json!(crate::autostart::enabled())),
        "autostart_set" => {
            let input: Enabled = params(value)?;
            crate::autostart::set_enabled(input.enabled)?;
            Ok(Value::Null)
        }
        "get_avatar" => Err(RpcError::new(
            "use_http",
            "avatars are served by GET /avatar/{fingerprint}?ticket=...",
        )),
        "set_avatar" => Err(RpcError::new(
            "use_http",
            "avatars are uploaded by POST /avatar",
        )),
        "clear_avatar" => {
            link.clear_avatar().await?;
            Ok(Value::Null)
        }
        "block_user" => {
            let input: UserId = params(value)?;
            link.block_user(input.user_id).await?;
            Ok(Value::Null)
        }
        "unblock_user" => {
            let input: Fingerprint = params(value)?;
            link.unblock_user(input.fingerprint).await?;
            Ok(Value::Null)
        }
        "get_message" => {
            let input: MessageId = params(value)?;
            Ok(json!(link.get_message(input.message_id).await?))
        }
        "list_messages" => {
            let input: ListMessages = params(value)?;
            link.list_messages(input.conversation_id, input.limit, input.offset)
                .await
                .map(|messages| json!(messages))
                .map_err(Into::into)
        }
        "count_messages" => {
            let input: ConversationId = params(value)?;
            Ok(json!(link.count_messages(input.conversation_id).await?))
        }
        "clear_history" => {
            let input: UserId = params(value)?;
            Ok(json!(link.clear_history(input.user_id).await?))
        }
        "storage_usage" => Ok(json!(link.storage_usage().await?)),
        "send_message" => {
            let input: SendMessage = params(value)?;
            if input.reply_to.is_empty() {
                Ok(json!(link.send_message(input.user_id, input.body).await?))
            } else {
                Ok(json!(
                    link.send_message_reply(input.user_id, input.body, input.reply_to)
                        .await?
                ))
            }
        }
        "retry_message" => {
            let input: MessageId = params(value)?;
            Ok(json!(link.retry_message(input.message_id).await?))
        }
        "edit_message" => {
            let input: EditMessage = params(value)?;
            link.edit_message(input.message_id, input.body).await?;
            Ok(Value::Null)
        }
        "delete_message" => {
            let input: MessageId = params(value)?;
            link.delete_message(input.message_id).await?;
            Ok(Value::Null)
        }
        "send_attachment" => Err(RpcError::new(
            "use_http",
            "attachments are uploaded by POST /attachment?user_id=...",
        )),
        "attachment_path" | "save_attachment" => Err(RpcError::new(
            "use_http",
            "attachments are downloaded with a ticket from attachment_ticket",
        )),
        "attachment_ticket" => {
            let input: Hash = params(value)?;
            let ticket = state
                .mint_ticket(crate::bridge::TicketKind::Attachment, input.hash)
                .map_err(|error| RpcError::new("internal", error.to_string()))?;
            Ok(json!({
                "url": state.ticket_url(&ticket),
                "expires_at": ticket.expires_at,
            }))
        }
        "avatar_ticket" => {
            let input: Fingerprint = params(value)?;
            let ticket = state
                .mint_ticket(crate::bridge::TicketKind::Avatar, input.fingerprint)
                .map_err(|error| RpcError::new("internal", error.to_string()))?;
            Ok(json!({
                "url": state.ticket_url(&ticket),
                "expires_at": ticket.expires_at,
            }))
        }
        "mark_read" => {
            let input: MarkRead = params(value)?;
            link.mark_read(input.conversation_id, input.message_ids)
                .await?;
            Ok(Value::Null)
        }
        "mark_conversation_read" => {
            let input: ConversationId = params(value)?;
            link.mark_conversation_read(input.conversation_id).await?;
            Ok(Value::Null)
        }
        "set_user_policy" => {
            let input: SetPolicy = params(value)?;
            link.set_user_policy(input.user_id, input.policy).await?;
            Ok(Value::Null)
        }
        "set_typing" => {
            let input: SetTyping = params(value)?;
            link.set_typing(input.user_id, input.typing).await?;
            Ok(Value::Null)
        }
        "set_dnd" => {
            let input: Enabled = params(value)?;
            link.set_dnd(input.enabled).await?;
            Ok(Value::Null)
        }
        "start_call" => {
            let input: UserId = params(value)?;
            Ok(json!(link.start_call(input.user_id).await?))
        }
        "accept_call" => {
            let input: CallId = params(value)?;
            link.accept_call(input.call_id).await?;
            Ok(Value::Null)
        }
        "decline_call" => {
            let input: CallId = params(value)?;
            link.decline_call(input.call_id).await?;
            Ok(Value::Null)
        }
        "hangup_call" => {
            let input: CallId = params(value)?;
            link.hangup_call(input.call_id).await?;
            Ok(Value::Null)
        }
        "set_call_muted" => {
            let input: SetCallMuted = params(value)?;
            link.set_call_muted(input.call_id, input.input_muted, input.output_muted)
                .await?;
            Ok(Value::Null)
        }
        "call_state" => Ok(json!(link.call_state().await?)),
        "call_metrics" => Ok(json!(link.call_metrics().await?)),
        "list_call_logs" => {
            let input: ListCallLogs = params(value)?;
            Ok(json!(
                link.list_call_logs(input.user_id, input.limit).await?
            ))
        }
        "list_audio_devices" => Ok(json!(link.list_audio_devices().await?)),
        "start_watch_party" => {
            let input: StartWatchParty = params(value)?;
            Ok(json!(
                link.start_watch_party(input.user_ids, input.video_id)
                    .await?
            ))
        }
        "youtube_preview" => {
            let input: VideoId = params(value)?;
            crate::bridge::youtube::preview(&input.video_id)
                .await
                .map(|preview| json!(preview))
                .map_err(|error| RpcError::new("youtube_unavailable", error.to_string()))
        }
        "youtube_stream" => {
            let input: VideoId = params(value)?;
            let path = crate::bridge::youtube::download(&input.video_id)
                .await
                .map_err(|error| RpcError::new("youtube_unavailable", error.to_string()))?;
            let ticket = state
                .mint_stream_ticket(path.to_string_lossy().into_owned())
                .map_err(|error| RpcError::new("internal", error.to_string()))?;
            Ok(json!({"video_id": input.video_id, "url": state.ticket_url(&ticket)}))
        }
        "invite_watch_party" => {
            let input: InviteWatchParty = params(value)?;
            Ok(json!(
                link.invite_watch_party(input.party_id, input.user_ids)
                    .await?
            ))
        }
        "accept_watch_party" => {
            let input: PartyId = params(value)?;
            Ok(json!(link.accept_watch_party(input.party_id).await?))
        }
        "decline_watch_party" => {
            let input: PartyId = params(value)?;
            link.decline_watch_party(input.party_id).await?;
            Ok(Value::Null)
        }
        "leave_watch_party" => {
            let input: PartyId = params(value)?;
            link.leave_watch_party(input.party_id).await?;
            Ok(Value::Null)
        }
        "publish_watch_party" => {
            let input: PublishWatchParty = params(value)?;
            Ok(json!(
                link.publish_watch_party(input.party_id, input.position_ms, input.playing)
                    .await?
            ))
        }
        "end_watch_party" => {
            let input: PartyId = params(value)?;
            link.end_watch_party(input.party_id).await?;
            Ok(Value::Null)
        }
        "watch_party" => Ok(json!(link.watch_party().await?)),
        "request_remote_session" => {
            let input: UserId = params(value)?;
            Ok(json!(link.request_remote_session(input.user_id).await?))
        }
        "accept_remote_session" => {
            let input: SessionId = params(value)?;
            link.accept_remote_session(input.session_id).await?;
            Ok(Value::Null)
        }
        "deny_remote_session" => {
            let input: SessionId = params(value)?;
            link.deny_remote_session(input.session_id).await?;
            Ok(Value::Null)
        }
        "end_remote_session" => {
            let input: SessionId = params(value)?;
            link.end_remote_session(input.session_id).await?;
            Ok(Value::Null)
        }
        "set_remote_input" => {
            let input: SetRemoteInput = params(value)?;
            link.set_remote_input(input.session_id, input.allowed)
                .await?;
            Ok(Value::Null)
        }
        "remote_session" => Ok(json!(link.remote_session().await?)),
        other => Err(RpcError::unknown_method(other)),
    }
}

fn save_setting(key: &str, value: &str) -> Result<(), RpcError> {
    let mut settings = ticonnect_settings::Settings::load()
        .map_err(|error| RpcError::new("settings_error", error.to_string()))?;
    settings
        .set_setting(key, value)
        .map_err(|error| RpcError::new("settings_error", error.to_string()))?;
    settings
        .save()
        .map_err(|error| RpcError::new("settings_error", error.to_string()))
}

#[derive(Deserialize)]
struct KeyValue {
    key: String,
    value: String,
}

#[derive(Deserialize)]
struct Enabled {
    enabled: bool,
}

#[derive(Deserialize)]
struct UserId {
    user_id: String,
}

#[derive(Deserialize)]
struct Fingerprint {
    fingerprint: String,
}

#[derive(Deserialize)]
struct Hash {
    hash: String,
}

#[derive(Deserialize)]
struct ConversationId {
    conversation_id: String,
}

#[derive(Deserialize)]
struct MessageId {
    message_id: String,
}

#[derive(Deserialize)]
struct ListMessages {
    conversation_id: String,
    limit: u32,
    offset: u32,
}

#[derive(Deserialize)]
struct SendMessage {
    user_id: String,
    body: String,
    #[serde(default)]
    reply_to: String,
}

#[derive(Deserialize)]
struct EditMessage {
    message_id: String,
    body: String,
}

#[derive(Deserialize)]
struct MarkRead {
    conversation_id: String,
    message_ids: Vec<String>,
}

#[derive(Deserialize)]
struct SetPolicy {
    user_id: String,
    policy: ticonnect_core::ipc::events::UserPolicy,
}

#[derive(Deserialize)]
struct SetTyping {
    user_id: String,
    typing: bool,
}

#[derive(Deserialize)]
struct CallId {
    call_id: String,
}

#[derive(Deserialize)]
struct SetCallMuted {
    call_id: String,
    input_muted: bool,
    output_muted: bool,
}

#[derive(Deserialize)]
struct ListCallLogs {
    #[serde(default)]
    user_id: String,
    #[serde(default = "default_call_log_limit")]
    limit: u32,
}

fn default_call_log_limit() -> u32 {
    100
}

#[derive(Deserialize)]
struct SetRemoteInput {
    session_id: String,
    allowed: bool,
}

#[derive(Deserialize)]
struct SessionId {
    session_id: String,
}

#[derive(Deserialize)]
struct StartWatchParty {
    user_ids: Vec<String>,
    video_id: String,
}

#[derive(Deserialize)]
struct VideoId {
    video_id: String,
}

#[derive(Deserialize)]
struct InviteWatchParty {
    party_id: String,
    user_ids: Vec<String>,
}

#[derive(Deserialize)]
struct PartyId {
    party_id: String,
}

#[derive(Deserialize)]
struct PublishWatchParty {
    party_id: String,
    position_ms: u64,
    playing: bool,
}
