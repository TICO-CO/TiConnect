use std::collections::HashMap;

use zbus::OwnedVariant;
use zbus::proxy;

#[proxy(
    default_service = "org.ticonnect.Daemon",
    default_path = "/org/ticonnect/Daemon",
    interface = "org.ticonnect.Daemon"
)]
pub trait Daemon {
    fn ping(&self) -> zbus::Result<String>;
    fn get_version(&self) -> zbus::Result<String>;
    fn get_status(&self) -> zbus::Result<HashMap<String, OwnedVariant>>;
    fn get_identity(&self) -> zbus::Result<HashMap<String, OwnedVariant>>;
    fn get_avatar(&self, fingerprint: &str) -> zbus::Result<Vec<u8>>;
    fn set_avatar(&self, avatar: &[u8]) -> zbus::Result<()>;
    fn clear_avatar(&self) -> zbus::Result<()>;
    fn get_setting(&self, key: &str) -> zbus::Result<OwnedVariant>;
    fn list_settings(&self) -> zbus::Result<HashMap<String, OwnedVariant>>;
    fn list_users(&self) -> zbus::Result<Vec<HashMap<String, OwnedVariant>>>;
    fn get_user(&self, user_id: &str) -> zbus::Result<HashMap<String, OwnedVariant>>;
    fn list_candidates(&self) -> zbus::Result<Vec<HashMap<String, OwnedVariant>>>;
    fn get_discovery_status(&self) -> zbus::Result<HashMap<String, OwnedVariant>>;
    fn get_network_status(&self) -> zbus::Result<HashMap<String, OwnedVariant>>;
    fn run_lan_check(&self) -> zbus::Result<HashMap<String, OwnedVariant>>;
    fn forget_user(&self, user_id: &str) -> zbus::Result<bool>;
    fn list_blocked_users(&self) -> zbus::Result<Vec<HashMap<String, OwnedVariant>>>;
    fn unblock_user(&self, fingerprint: &str) -> zbus::Result<bool>;
    fn send_message(&self, user_id: &str, body: &str, reply_to: &str) -> zbus::Result<String>;
    fn retry_message(&self, message_id: &str) -> zbus::Result<bool>;
    fn edit_message(&self, message_id: &str, body: &str) -> zbus::Result<()>;
    fn delete_message(&self, message_id: &str) -> zbus::Result<()>;
    fn list_messages(
        &self,
        conversation_id: &str,
        limit: u32,
        offset: u32,
    ) -> zbus::Result<Vec<HashMap<String, OwnedVariant>>>;
    fn list_conversation_summaries(&self) -> zbus::Result<Vec<HashMap<String, OwnedVariant>>>;
    fn get_message(&self, message_id: &str) -> zbus::Result<HashMap<String, OwnedVariant>>;
    fn count_messages(&self, conversation_id: &str) -> zbus::Result<u32>;
    fn clear_history(&self, user_id: &str) -> zbus::Result<u32>;
    fn get_storage_usage(&self) -> zbus::Result<HashMap<String, OwnedVariant>>;
    fn mark_read(&self, conversation_id: &str, message_ids: &[&str]) -> zbus::Result<()>;
    fn mark_conversation_read(&self, conversation_id: &str) -> zbus::Result<()>;
    fn set_typing(&self, user_id: &str, typing: bool) -> zbus::Result<()>;
    fn get_message_state(&self, message_id: &str) -> zbus::Result<String>;
    fn set_user_policy(&self, user_id: &str, policy: &str) -> zbus::Result<()>;
    fn set_dnd(&self, enabled: bool) -> zbus::Result<()>;
    fn send_attachment(
        &self,
        user_id: &str,
        path: &str,
        mime: &str,
        reply_to: &str,
    ) -> zbus::Result<String>;
    fn get_transfer_state(&self, hash: &str) -> zbus::Result<HashMap<String, OwnedVariant>>;
    fn cancel_transfer(&self, hash: &str) -> zbus::Result<()>;
    fn save_attachment(&self, hash: &str, destination: &str) -> zbus::Result<bool>;
    fn get_attachment_path(&self, hash: &str) -> zbus::Result<String>;
    fn start_call(&self, user_id: &str) -> zbus::Result<String>;
    fn accept_call(&self, call_id: &str) -> zbus::Result<()>;
    fn decline_call(&self, call_id: &str) -> zbus::Result<()>;
    fn hangup_call(&self, call_id: &str) -> zbus::Result<()>;
    fn set_call_muted(
        &self,
        call_id: &str,
        input_muted: bool,
        output_muted: bool,
    ) -> zbus::Result<()>;
    fn get_call_state(&self) -> zbus::Result<HashMap<String, OwnedVariant>>;
    fn list_call_logs(
        &self,
        user_id: &str,
        limit: u32,
    ) -> zbus::Result<Vec<HashMap<String, OwnedVariant>>>;
    fn list_audio_devices(&self) -> zbus::Result<Vec<HashMap<String, OwnedVariant>>>;
    fn get_call_metrics(&self) -> zbus::Result<HashMap<String, OwnedVariant>>;
    fn start_watch_party(
        &self,
        user_ids: &[&str],
        video_id: &str,
    ) -> zbus::Result<HashMap<String, OwnedVariant>>;
    fn invite_watch_party(
        &self,
        party_id: &str,
        user_ids: &[&str],
    ) -> zbus::Result<HashMap<String, OwnedVariant>>;
    fn accept_watch_party(&self, party_id: &str) -> zbus::Result<HashMap<String, OwnedVariant>>;
    fn decline_watch_party(&self, party_id: &str) -> zbus::Result<()>;
    fn leave_watch_party(&self, party_id: &str) -> zbus::Result<()>;
    fn publish_watch_party(
        &self,
        party_id: &str,
        position_ms: u64,
        playing: bool,
    ) -> zbus::Result<HashMap<String, OwnedVariant>>;
    fn end_watch_party(&self, party_id: &str) -> zbus::Result<()>;
    fn get_watch_party(&self) -> zbus::Result<HashMap<String, OwnedVariant>>;
    fn request_remote_session(&self, user_id: &str) -> zbus::Result<HashMap<String, OwnedVariant>>;
    fn accept_remote_session(&self, session_id: &str) -> zbus::Result<()>;
    fn deny_remote_session(&self, session_id: &str) -> zbus::Result<()>;
    fn end_remote_session(&self, session_id: &str) -> zbus::Result<()>;
    fn set_remote_input(&self, session_id: &str, allowed: bool) -> zbus::Result<()>;
    fn get_remote_session(&self) -> zbus::Result<HashMap<String, OwnedVariant>>;
    fn shutdown(&self) -> zbus::Result<()>;

    #[zbus(signal)]
    fn candidate_found(&self, candidate: HashMap<String, OwnedVariant>) -> zbus::Result<()>;

    #[zbus(signal)]
    fn candidate_lost(&self, fingerprint: &str) -> zbus::Result<()>;

    #[zbus(signal)]
    fn user_connected(&self, user: HashMap<String, OwnedVariant>) -> zbus::Result<()>;

    #[zbus(signal)]
    fn user_disconnected(&self, user: HashMap<String, OwnedVariant>) -> zbus::Result<()>;

    #[zbus(signal)]
    fn message_received(&self, message: HashMap<String, OwnedVariant>) -> zbus::Result<()>;

    #[zbus(signal)]
    fn message_state_changed(&self, message_id: &str, state: &str) -> zbus::Result<()>;

    #[zbus(signal)]
    fn message_edited(
        &self,
        message_id: &str,
        user_id: &str,
        body: &str,
        edited_at: u64,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    fn message_deleted(&self, message_id: &str, user_id: &str) -> zbus::Result<()>;

    #[zbus(signal)]
    fn conversation_changed(&self, user_id: &str) -> zbus::Result<()>;

    #[zbus(signal)]
    fn typing_changed(&self, user_id: &str, typing: bool) -> zbus::Result<()>;

    #[zbus(signal)]
    fn user_profile_changed(&self, fingerprint: &str) -> zbus::Result<()>;

    #[zbus(signal)]
    fn self_profile_changed(&self) -> zbus::Result<()>;

    #[zbus(signal)]
    fn sync_completed(&self, user_id: &str, count: u32) -> zbus::Result<()>;

    #[zbus(signal)]
    fn policy_changed(&self, user_id: &str, policy: &str) -> zbus::Result<()>;

    #[zbus(signal)]
    fn settings_changed(&self, key: &str) -> zbus::Result<()>;

    #[zbus(signal)]
    fn incoming_call(&self, call: HashMap<String, OwnedVariant>) -> zbus::Result<()>;

    #[zbus(signal)]
    fn call_state_changed(&self, call_id: &str, state: &str) -> zbus::Result<()>;

    #[zbus(signal)]
    fn call_mute_changed(
        &self,
        call_id: &str,
        input_muted: bool,
        output_muted: bool,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    fn call_ended(&self, call_id: &str, reason: &str) -> zbus::Result<()>;

    #[zbus(signal)]
    fn watch_party_requested(&self, party: HashMap<String, OwnedVariant>) -> zbus::Result<()>;

    #[zbus(signal)]
    fn watch_party_changed(&self, party: HashMap<String, OwnedVariant>) -> zbus::Result<()>;

    #[zbus(signal)]
    fn watch_party_ended(&self, party_id: &str, reason: &str) -> zbus::Result<()>;

    #[zbus(signal)]
    fn remote_session_requested(&self, session: HashMap<String, OwnedVariant>) -> zbus::Result<()>;

    #[zbus(signal)]
    fn remote_session_started(&self, session_id: &str) -> zbus::Result<()>;

    #[zbus(signal)]
    fn remote_session_ended(&self, session_id: &str, reason: &str) -> zbus::Result<()>;

    #[zbus(signal)]
    fn remote_indicator_changed(&self, session_id: &str, visible: bool) -> zbus::Result<()>;

    #[zbus(signal)]
    fn remote_input_changed(&self, session_id: &str, allowed: bool) -> zbus::Result<()>;

    #[zbus(signal)]
    fn transfer_progress(&self, hash: &str, bytes: u64, total: u64) -> zbus::Result<()>;

    #[zbus(signal)]
    fn transfer_done(&self, hash: &str) -> zbus::Result<()>;

    #[zbus(signal)]
    fn transfer_failed(&self, hash: &str, reason: &str) -> zbus::Result<()>;
}
