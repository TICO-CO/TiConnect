use ticonnect_core::ipc;

#[test]
fn dbus_names_are_stable() {
    assert_eq!(ipc::DAEMON_BUS_NAME, "org.ticonnect.Daemon");
    assert_eq!(ipc::DAEMON_PATH, "/org/ticonnect/Daemon");
    assert_eq!(ipc::DAEMON_INTERFACE, "org.ticonnect.Daemon");
    assert_eq!(ipc::UI_INTERFACE, "org.ticonnect.UI");
}

#[test]
fn methods_and_signals_match_the_documented_contract() {
    assert_eq!(ipc::methods::PING, "Ping");
    assert_eq!(ipc::methods::GET_VERSION, "GetVersion");
    assert_eq!(ipc::methods::GET_STATUS, "GetStatus");
    assert_eq!(ipc::methods::GET_IDENTITY, "GetIdentity");
    assert_eq!(ipc::methods::GET_SETTING, "GetSetting");
    assert_eq!(ipc::methods::LIST_SETTINGS, "ListSettings");
    assert_eq!(ipc::methods::LIST_PEERS, "ListPeers");
    assert_eq!(ipc::methods::SHUTDOWN, "Shutdown");

    assert_eq!(ipc::signals::DAEMON_STARTED, "DaemonStarted");
    assert_eq!(ipc::signals::DAEMON_STOPPING, "DaemonStopping");
    assert_eq!(ipc::signals::SETTINGS_CHANGED, "SettingsChanged");
}

#[test]
fn error_names_use_the_stable_prefix() {
    assert_eq!(ipc::errors::FAILED, "org.ticonnect.Daemon.Error.Failed");
    assert_eq!(
        ipc::errors::NOT_FOUND,
        "org.ticonnect.Daemon.Error.NotFound"
    );
}

#[test]
fn status_and_identity_keys_are_stable() {
    assert_eq!(ipc::STATUS_UPTIME_SECONDS, "uptime_seconds");
    assert_eq!(ipc::STATUS_PEER_COUNT, "peer_count");
    assert_eq!(ipc::STATUS_OUTBOX_DEPTH, "outbox_depth");
    assert_eq!(ipc::STATUS_TRANSPORT_STATE, "transport_state");
    assert_eq!(ipc::IDENTITY_NODE_ID, "node_id");
    assert_eq!(ipc::IDENTITY_FINGERPRINT, "fingerprint");
    assert_eq!(ipc::IDENTITY_DISPLAY_NAME, "display_name");
}
