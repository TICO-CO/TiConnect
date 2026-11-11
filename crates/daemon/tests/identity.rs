use std::fs;
use std::path::PathBuf;

use ticonnect_daemon::identity::{Identity, IdentityError};

#[test]
fn first_load_creates_private_identity_files() {
    let dir = temp_dir();
    let identity = Identity::load_or_create(&dir).expect("create identity");
    let public = identity.public();

    assert!(dir.join("identity.key").is_file());
    assert!(dir.join("identity.toml").is_file());
    assert_eq!(public.fingerprint().to_hex().len(), 64);

    assert_eq!(mode(&dir), 0o700);
    assert_eq!(mode(&dir.join("identity.key")), 0o600);
    assert_eq!(mode(&dir.join("identity.toml")), 0o600);

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn reload_keeps_fingerprint_and_node_id() {
    let dir = temp_dir();
    let first = Identity::load_or_create(&dir)
        .expect("first identity")
        .public();
    let second = Identity::load_or_create(&dir)
        .expect("second identity")
        .public();

    assert_eq!(first.node_id(), second.node_id());
    assert_eq!(first.fingerprint(), second.fingerprint());

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn world_readable_key_is_rejected() {
    use std::os::unix::fs::PermissionsExt;

    let dir = temp_dir();
    let _ = Identity::load_or_create(&dir).expect("create identity");
    let key = dir.join("identity.key");
    fs::set_permissions(&key, fs::Permissions::from_mode(0o644)).expect("chmod");

    let error = Identity::load_or_create(&dir).expect_err("insecure key");
    assert!(matches!(
        error,
        IdentityError::InsecureKeyPermissions { .. }
    ));

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn missing_metadata_is_an_error_when_key_exists() {
    let dir = temp_dir();
    let _ = Identity::load_or_create(&dir).expect("create identity");
    fs::remove_file(dir.join("identity.toml")).expect("remove metadata");

    let error = Identity::load_or_create(&dir).expect_err("missing metadata");
    assert!(matches!(error, IdentityError::MissingMetadata));

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn deleting_key_creates_a_new_node() {
    let dir = temp_dir();
    let first = Identity::load_or_create(&dir)
        .expect("first identity")
        .public();
    fs::remove_file(dir.join("identity.key")).expect("remove key");

    let second = Identity::load_or_create(&dir)
        .expect("second identity")
        .public();
    assert_ne!(first.node_id(), second.node_id());
    assert_ne!(first.fingerprint(), second.fingerprint());

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn short_key_is_rejected() {
    let dir = temp_dir();
    fs::create_dir_all(&dir).expect("create dir");
    use std::os::unix::fs::PermissionsExt;

    let key = dir.join("identity.key");
    fs::write(&key, [1_u8; 16]).expect("write short key");
    fs::set_permissions(&key, fs::Permissions::from_mode(0o600)).expect("chmod");

    let error = Identity::load_or_create(&dir).expect_err("short key");
    assert!(matches!(error, IdentityError::InvalidKeyLength { .. }));

    let _ = fs::remove_dir_all(dir);
}

fn mode(path: &std::path::Path) -> u32 {
    use std::os::unix::fs::PermissionsExt;

    fs::metadata(path).expect("metadata").permissions().mode() & 0o777
}

fn temp_dir() -> PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT: AtomicU64 = AtomicU64::new(0);
    let id = NEXT.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!("ticonnect-identity-{}-{id}", std::process::id()))
}
