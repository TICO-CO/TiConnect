use std::fs;

use figment::Jail;

use super::{Config, ConfigStore};
use crate::error::Error;
use crate::log::LogFormat;

#[test]
fn defaults_are_correct() {
    let config = Config::default();

    assert_eq!(config.log_level, "info");
    assert_eq!(config.log_file, None);
    assert_eq!(config.log_format, LogFormat::Human);
    assert_eq!(config.data_dir, None);
}

#[test]
fn user_file_overrides_defaults() {
    Jail::expect_with(|jail| {
        jail.clear_env();
        jail.create_file("user.toml", "log_level = \"debug\"\n")?;

        let user = jail.directory().join("user.toml");
        let config = ConfigStore::at(&user).load().expect("valid config");

        assert_eq!(config.log_level, "debug");
        Ok(())
    });
}

#[test]
fn invalid_config_returns_typed_error() {
    Jail::expect_with(|jail| {
        jail.clear_env();
        jail.create_file("invalid.toml", "log_level = [\n")?;

        let invalid = jail.directory().join("invalid.toml");
        let error = ConfigStore::at(&invalid)
            .load()
            .expect_err("invalid config must fail");

        assert!(matches!(error, Error::Config(_)));
        Ok(())
    });
}

#[test]
fn missing_files_are_skipped() {
    Jail::expect_with(|jail| {
        jail.clear_env();

        let missing = jail.directory().join("missing.toml");
        let config = ConfigStore::at(&missing)
            .load()
            .expect("missing files must not fail");

        assert_eq!(config, Config::default());
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

        let store = ConfigStore::open_default().expect("valid store");
        let config = store.load().expect("valid config");

        assert_eq!(
            store.location().file().to_path_buf(),
            dir.join("config.toml")
        );
        assert_eq!(store.location().dir().to_path_buf(), dir);
        assert_eq!(config.log_level, "debug");
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
        let store = ConfigStore::at(&path);
        let mut config = store.load().expect("valid config");
        config.log_level = "warn".to_owned();
        config.log_format = LogFormat::Human;
        store.save(&config).expect("save");

        let contents = fs::read_to_string(&path).expect("saved config");
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
        let store = ConfigStore::at(&path);
        let mut config = store.load().expect("valid config");
        config.log_file = None;
        store.save(&config).expect("save");

        let contents = fs::read_to_string(&path).expect("saved config");
        assert!(!contents.contains("log_file"));

        let reloaded = store.load().expect("valid config");
        assert_eq!(reloaded.log_file, None);
        Ok(())
    });
}

#[test]
fn save_creates_parent_directories() {
    Jail::expect_with(|jail| {
        jail.clear_env();
        let path = jail.directory().join("nested/deeper/config.toml");
        let config = Config {
            log_level: "warn".to_owned(),
            ..Config::default()
        };

        ConfigStore::at(&path).save(&config).expect("save");

        assert!(path.exists());
        let loaded = ConfigStore::at(&path).load().expect("valid config");
        assert_eq!(loaded.log_level, "warn");
        Ok(())
    });
}

#[test]
fn save_without_changes_does_not_create_file() {
    Jail::expect_with(|jail| {
        jail.clear_env();
        let path = jail.directory().join("missing.toml");

        ConfigStore::at(&path)
            .save(&Config::default())
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
                        let config = Config {
                            log_level: level.to_owned(),
                            ..Config::default()
                        };
                        ConfigStore::at(&path).save(&config).expect("save");
                    }
                });
            }
        });

        let contents = fs::read_to_string(&path).expect("saved config");
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
        let store = ConfigStore::at(&path);
        let config = store.load().expect("valid config");
        assert_eq!(config.log_level, "debug");

        store.save(&config).expect("save");

        let contents = fs::read_to_string(&path).expect("saved config");
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

        ConfigStore::at(&path).reset().expect("reset");

        assert!(!path.exists());
        Ok(())
    });
}

#[test]
fn reset_missing_file_is_ok() {
    Jail::expect_with(|jail| {
        jail.clear_env();
        let path = jail.directory().join("missing.toml");

        ConfigStore::at(&path)
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

        ConfigStore::at(&path).reset().expect("reset");

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

        ConfigStore::open_default()
            .expect("valid store")
            .reset()
            .expect("reset");

        assert!(!path.exists());
        Ok(())
    });
}
