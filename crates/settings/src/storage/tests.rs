use std::fs;

use figment::Jail;

use super::store::Storage;
use crate::error::Error;
use crate::settings::schema::SettingsValues;

#[test]
fn defaults_are_correct() {
    let config = SettingsValues::default();

    assert_eq!(config.log_level, "info");
    assert_eq!(config.log_file, None);
    assert_eq!(config.log_format, "human");
    assert_eq!(config.data_dir, None);
}

#[test]
fn user_file_overrides_defaults() {
    Jail::expect_with(|jail| {
        jail.clear_env();
        jail.create_file("user.toml", "log_level = \"debug\"\n")?;

        let user = jail.directory().join("user.toml");
        let storage = Storage::at(&user).expect("valid settings");

        assert_eq!(storage.saved().log_level, "debug");
        Ok(())
    });
}

#[test]
fn invalid_config_returns_typed_error() {
    Jail::expect_with(|jail| {
        jail.clear_env();
        jail.create_file("invalid.toml", "log_level = [\n")?;

        let invalid = jail.directory().join("invalid.toml");
        let error = Storage::at(&invalid).expect_err("invalid settings must fail");

        assert!(matches!(error, Error::Settings(_)));
        Ok(())
    });
}

#[test]
fn missing_files_are_skipped() {
    Jail::expect_with(|jail| {
        jail.clear_env();

        let missing = jail.directory().join("missing.toml");
        let storage = Storage::at(&missing).expect("missing files must not fail");

        assert_eq!(storage.saved(), &SettingsValues::default());
        Ok(())
    });
}

#[test]
fn environment_config_dir_controls_user_file() {
    Jail::expect_with(|jail| {
        jail.clear_env();
        let dir = jail.create_dir("custom")?;
        jail.create_file("custom/config.toml", "log_level = \"debug\"\n")?;
        jail.set_env("TICONNECT_CONFIG_DIR", dir.display());

        let storage = Storage::open_default().expect("valid storage");

        assert_eq!(
            storage.location().file().to_path_buf(),
            dir.join("config.toml")
        );
        assert_eq!(storage.location().dir().to_path_buf(), dir);
        assert_eq!(storage.saved().log_level, "debug");
        Ok(())
    });
}

#[test]
fn save_writes_changed_fields_and_preserves_unknown_keys() {
    Jail::expect_with(|jail| {
        jail.clear_env();
        jail.create_file(
            "config.toml",
            "log_level = \"debug\"\nlog_format = \"json\"\nfuture_key = \"keep\"\n",
        )?;

        let path = jail.directory().join("config.toml");
        let mut storage = Storage::at(&path).expect("valid settings");
        let mut config = storage.saved().clone();
        config.log_level = "warn".to_owned();
        config.log_format = "human".to_owned();
        storage.save(&config).expect("save");

        let contents = fs::read_to_string(&path).expect("saved settings");
        let table: toml::Table = toml::from_str(&contents).expect("valid toml");

        assert_eq!(table["log_level"].as_str(), Some("warn"));
        assert_eq!(table["log_format"].as_str(), Some("human"));
        assert_eq!(table["future_key"].as_str(), Some("keep"));
        Ok(())
    });
}

#[test]
fn save_clears_optional_path() {
    Jail::expect_with(|jail| {
        jail.clear_env();
        jail.create_file("config.toml", "log_file = \"/tmp/ticonnect.log\"\n")?;

        let path = jail.directory().join("config.toml");
        let mut storage = Storage::at(&path).expect("valid settings");
        let mut config = storage.saved().clone();
        config.log_file = None;
        storage.save(&config).expect("save");

        let contents = fs::read_to_string(&path).expect("saved settings");
        assert!(!contents.contains("log_file"));

        let reloaded = Storage::at(&path).expect("valid settings");
        assert_eq!(reloaded.saved().log_file, None);
        Ok(())
    });
}

#[test]
fn save_creates_parent_directories() {
    Jail::expect_with(|jail| {
        jail.clear_env();
        let path = jail.directory().join("nested/deeper/config.toml");
        let config = SettingsValues {
            log_level: "warn".to_owned(),
            ..SettingsValues::default()
        };

        Storage::at(&path)
            .expect("valid storage")
            .save(&config)
            .expect("save");

        assert!(path.exists());
        let loaded = Storage::at(&path).expect("valid settings");
        assert_eq!(loaded.saved().log_level, "warn");
        Ok(())
    });
}

#[test]
fn save_without_changes_does_not_create_file() {
    Jail::expect_with(|jail| {
        jail.clear_env();
        let path = jail.directory().join("missing.toml");

        Storage::at(&path)
            .expect("valid storage")
            .save(&SettingsValues::default())
            .expect("save");

        assert!(!path.exists());
        Ok(())
    });
}

#[test]
fn concurrent_saves_leave_valid_toml() {
    Jail::expect_with(|jail| {
        jail.clear_env();
        let path = jail.directory().join("config.toml");

        std::thread::scope(|scope| {
            for level in ["debug", "trace", "warn", "error"] {
                let path = path.clone();
                scope.spawn(move || {
                    for _ in 0..20 {
                        let config = SettingsValues {
                            log_level: level.to_owned(),
                            ..SettingsValues::default()
                        };
                        Storage::at(&path)
                            .expect("valid storage")
                            .save(&config)
                            .expect("save");
                    }
                });
            }
        });

        let contents = fs::read_to_string(&path).expect("saved settings");
        let table: toml::Table = toml::from_str(&contents).expect("valid toml");
        assert!(matches!(
            table["log_level"].as_str(),
            Some("debug" | "trace" | "warn" | "error")
        ));
        Ok(())
    });
}

#[test]
fn save_removes_legacy_runtime_config_dir() {
    Jail::expect_with(|jail| {
        jail.clear_env();
        jail.create_file(
            "config.toml",
            "log_level = \"debug\"\nconfig_dir = \"/tmp/legacy\"\n",
        )?;

        let path = jail.directory().join("config.toml");
        let mut storage = Storage::at(&path).expect("valid settings");
        let config = storage.saved().clone();
        assert_eq!(config.log_level, "debug");

        storage.save(&config).expect("save");

        let contents = fs::read_to_string(&path).expect("saved settings");
        assert!(contents.contains("log_level = \"debug\""));
        assert!(!contents.contains("config_dir"));
        Ok(())
    });
}

#[test]
fn reset_removes_explicit_file() {
    Jail::expect_with(|jail| {
        jail.clear_env();
        let dir = jail.create_dir("custom")?;
        let _ = jail.create_file("custom/config.toml", "log_level = \"warn\"\n")?;
        let path = dir.join("config.toml");

        Storage::at(&path)
            .expect("valid storage")
            .reset()
            .expect("reset");

        assert!(!path.exists());
        Ok(())
    });
}

#[test]
fn reset_missing_file_is_ok() {
    Jail::expect_with(|jail| {
        jail.clear_env();
        let path = jail.directory().join("missing.toml");

        Storage::at(&path)
            .expect("valid storage")
            .reset()
            .expect("missing file must not fail");
        Ok(())
    });
}

#[test]
fn reset_does_not_create_parent_directories() {
    Jail::expect_with(|jail| {
        jail.clear_env();
        let path = jail.directory().join("nested/deeper/config.toml");

        Storage::at(&path)
            .expect("valid storage")
            .reset()
            .expect("reset");

        assert!(!path.parent().expect("parent").exists());
        Ok(())
    });
}

#[test]
fn reset_uses_environment_config_dir() {
    Jail::expect_with(|jail| {
        jail.clear_env();
        let dir = jail.create_dir("custom")?;
        let _ = jail.create_file("custom/config.toml", "log_level = \"warn\"\n")?;
        jail.set_env("TICONNECT_CONFIG_DIR", dir.display());
        let path = dir.join("config.toml");

        Storage::open_default()
            .expect("valid storage")
            .reset()
            .expect("reset");

        assert!(!path.exists());
        Ok(())
    });
}
