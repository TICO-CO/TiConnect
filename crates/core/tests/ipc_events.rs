use std::collections::HashMap;

use ticonnect_core::ipc;
use ticonnect_core::ipc::events::{self, MessageState, ParseError, UserPolicy};
use zbus::OwnedVariant;
use zbus::Str;

fn text(value: &str) -> OwnedVariant {
    OwnedVariant::from(Str::from(value.to_owned()))
}

fn message_values() -> HashMap<String, OwnedVariant> {
    HashMap::from([
        (ipc::MESSAGE_ID.to_owned(), text("message-id")),
        (
            ipc::MESSAGE_CONVERSATION_ID.to_owned(),
            text("conversation-id"),
        ),
        (ipc::MESSAGE_USER_ID.to_owned(), text("user-id")),
        (ipc::MESSAGE_SENDER_ID.to_owned(), text("user-id")),
        (ipc::MESSAGE_SEQ.to_owned(), OwnedVariant::from(7_u32)),
        (ipc::MESSAGE_SENT_AT.to_owned(), OwnedVariant::from(11_u64)),
        (
            ipc::MESSAGE_RECEIVED_AT.to_owned(),
            OwnedVariant::from(12_u64),
        ),
        (ipc::MESSAGE_BODY.to_owned(), text("hello")),
        (ipc::MESSAGE_STATE.to_owned(), text("sent")),
        (ipc::MESSAGE_NOTIFY.to_owned(), OwnedVariant::from(false)),
    ])
}

#[test]
fn parses_message_and_notification_flag() {
    let message = events::parse_message(&message_values(), Some("local")).expect("message");
    assert_eq!(message.id, "message-id");
    assert_eq!(message.user_id, "user-id");
    assert_eq!(message.body, "hello");
    assert_eq!(message.state, MessageState::Sent);
    assert!(!message.local_sender);
    assert!(!message.notify);
}

#[test]
fn missing_required_field_is_typed() {
    let mut values = message_values();
    values.remove(ipc::MESSAGE_ID);
    let error = events::parse_message(&values, None).expect_err("missing id");
    assert!(matches!(error, ParseError::MissingField(field) if field == ipc::MESSAGE_ID));
}

#[test]
fn optional_message_fields_default() {
    let mut values = message_values();
    values.remove(ipc::MESSAGE_USER_ID);
    values.remove(ipc::MESSAGE_SEQ);
    values.remove(ipc::MESSAGE_STATE);
    values.remove(ipc::MESSAGE_NOTIFY);
    let message = events::parse_message(&values, None).expect("message");
    assert!(message.user_id.is_empty());
    assert_eq!(message.seq, 0);
    assert_eq!(message.state, MessageState::Unknown);
    assert!(!message.notify);
}

#[test]
fn parses_user_and_policy() {
    let values = HashMap::from([
        (ipc::USER_ID.to_owned(), text("user-id")),
        (ipc::USER_FINGERPRINT.to_owned(), text("fingerprint")),
        (ipc::USER_DISPLAY_NAME.to_owned(), text("Ada")),
        (ipc::USER_POLICY.to_owned(), text("mute")),
        (ipc::USER_CONNECTED.to_owned(), OwnedVariant::from(true)),
    ]);
    let user = events::parse_user(&values).expect("user");
    assert_eq!(user.display_name, "Ada");
    assert_eq!(user.policy, UserPolicy::Mute);
    assert!(user.connected);
}

#[test]
fn settings_map_renders_values() {
    let values = HashMap::from([
        ("dnd.enabled".to_owned(), OwnedVariant::from(true)),
        ("overlay.timeout_secs".to_owned(), OwnedVariant::from(5_u32)),
        ("identity.display_name".to_owned(), text("Ada")),
    ]);
    let settings = events::settings_map(&values).expect("settings");
    assert_eq!(settings["dnd.enabled"], "true");
    assert_eq!(settings["overlay.timeout_secs"], "5");
    assert_eq!(settings["identity.display_name"], "Ada");
}

#[test]
fn policy_and_state_parse_stably() {
    assert_eq!(
        UserPolicy::parse("accept").expect("accept").as_str(),
        "accept"
    );
    assert_eq!(UserPolicy::parse("mute").expect("mute").as_str(), "mute");
    assert_eq!(
        UserPolicy::parse("ignore").expect("ignore").as_str(),
        "ignore"
    );
    assert!(UserPolicy::parse("unknown").is_err());
    assert_eq!(MessageState::parse("read").expect("read").as_str(), "read");
    assert_eq!(
        MessageState::parse("unknown").expect("unknown"),
        MessageState::Unknown
    );
}
