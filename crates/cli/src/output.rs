use std::collections::HashMap;

use serde_json::{Map, Value, json};
use ticonnect_core::ipc;
use zbus::OwnedVariant;

pub fn status_json(version: &str, status: &HashMap<String, OwnedVariant>) -> Value {
    let mut output = Map::new();
    output.insert("version".to_owned(), json!(version));
    for (key, value) in status {
        let value = if let Ok(value) = u64::try_from(value) {
            json!(value)
        } else if let Ok(value) = u32::try_from(value) {
            json!(value)
        } else if let Ok(value) = <&str>::try_from(value) {
            json!(value)
        } else {
            json!(display(value))
        };
        output.insert(key.clone(), value);
    }
    Value::Object(output)
}

pub fn status_human(version: &str, status: &HashMap<String, OwnedVariant>) -> String {
    format!(
        "version: {version}\nuptime_seconds: {}\npeer_count: {}\noutbox_depth: {}\ntransport_state: {}\nactive_sessions: {}\n",
        status_value(status, ipc::STATUS_UPTIME_SECONDS),
        status_value(status, ipc::STATUS_PEER_COUNT),
        status_value(status, ipc::STATUS_OUTBOX_DEPTH),
        status_value(status, ipc::STATUS_TRANSPORT_STATE),
        status_value(status, ipc::STATUS_ACTIVE_SESSIONS),
    )
}

fn status_value(status: &HashMap<String, OwnedVariant>, key: &str) -> String {
    status
        .get(key)
        .map(display)
        .unwrap_or_else(|| "unknown".to_owned())
}

pub fn candidate_line(candidate: &HashMap<String, OwnedVariant>) -> String {
    format!(
        "{} {} {} {} {}",
        map_value(candidate, ipc::CANDIDATE_NODE_ID),
        map_value(candidate, ipc::CANDIDATE_FINGERPRINT),
        map_value(candidate, ipc::CANDIDATE_SOURCE),
        map_value(candidate, ipc::CANDIDATE_ADDRESSES),
        map_value(candidate, ipc::CANDIDATE_DISPLAY_NAME),
    )
}

fn map_value(map: &HashMap<String, OwnedVariant>, key: &str) -> String {
    map.get(key)
        .map(display)
        .unwrap_or_else(|| "unknown".to_owned())
}

pub fn variant_text(value: &OwnedVariant) -> String {
    display(value)
}

fn display(value: &OwnedVariant) -> String {
    if let Ok(value) = u64::try_from(value) {
        value.to_string()
    } else if let Ok(value) = u32::try_from(value) {
        value.to_string()
    } else if let Ok(value) = <&str>::try_from(value) {
        value.to_owned()
    } else {
        format!("{value:?}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_human_status() {
        let status = fixture();
        assert_eq!(
            status_human("0.1.0", &status),
            "version: 0.1.0\nuptime_seconds: 7\npeer_count: 0\noutbox_depth: 0\ntransport_state: offline\nactive_sessions: 0\n"
        );
    }

    #[test]
    fn renders_json_status_with_typed_values() {
        let json = status_json("0.1.0", &fixture());
        assert_eq!(json["version"], "0.1.0");
        assert_eq!(json["uptime_seconds"], 7);
        assert_eq!(json["peer_count"], 0);
        assert_eq!(json["outbox_depth"], 0);
        assert_eq!(json["transport_state"], "offline");
    }

    #[test]
    fn status_json_serializes_and_parses_with_expected_keys() {
        let text = serde_json::to_string(&status_json("0.1.0", &fixture())).expect("serialize");
        let parsed: Value = serde_json::from_str(&text).expect("parse");

        assert_eq!(parsed["version"], "0.1.0");
        assert_eq!(parsed["uptime_seconds"], 7);
        assert_eq!(parsed["peer_count"], 0);
        assert_eq!(parsed["outbox_depth"], 0);
        assert_eq!(parsed["transport_state"], "offline");
    }

    fn fixture() -> HashMap<String, OwnedVariant> {
        HashMap::from([
            (
                ipc::STATUS_UPTIME_SECONDS.to_owned(),
                OwnedVariant::from(7_u64),
            ),
            (ipc::STATUS_PEER_COUNT.to_owned(), OwnedVariant::from(0_u32)),
            (
                ipc::STATUS_OUTBOX_DEPTH.to_owned(),
                OwnedVariant::from(0_u32),
            ),
            (
                ipc::STATUS_TRANSPORT_STATE.to_owned(),
                OwnedVariant::from(zbus::Str::from(ipc::TRANSPORT_OFFLINE)),
            ),
            (
                ipc::STATUS_ACTIVE_SESSIONS.to_owned(),
                OwnedVariant::from(0_u32),
            ),
        ])
    }
}
