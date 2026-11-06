use std::fs;

use figment::Jail;

use crate::Settings;

#[test]
fn environment_overrides_file_at_runtime_only() {
    Jail::expect_with(|jail| {
        jail.clear_env();
        jail.create_file("user.toml", "log_level = \"debug\"\n")?;
        jail.set_env("TICONNECT_LOG_LEVEL", "trace");

        let user = jail.directory().join("user.toml");
        let settings = Settings::load_from(&user).expect("valid settings");

        assert_eq!(settings.log_level(), "trace");
        Ok(())
    });
}

#[test]
fn save_does_not_persist_untouched_environment() {
    Jail::expect_with(|jail| {
        jail.clear_env();
        jail.create_file("config.toml", "log_level = \"debug\"\n")?;
        jail.set_env("TICONNECT_LOG_LEVEL", "trace");

        let path = jail.directory().join("config.toml");
        let mut settings = Settings::load_from(&path).expect("valid settings");
        assert_eq!(settings.log_level(), "trace");

        settings.save().expect("save");

        let contents = fs::read_to_string(&path).expect("saved settings");
        assert!(contents.contains("log_level = \"debug\""));
        assert!(!contents.contains("trace"));
        Ok(())
    });
}

#[test]
fn set_changes_effective_value_and_clears_env_shadow() {
    Jail::expect_with(|jail| {
        jail.clear_env();
        jail.create_file("config.toml", "log_level = \"debug\"\n")?;
        jail.set_env("TICONNECT_LOG_LEVEL", "trace");

        let path = jail.directory().join("config.toml");
        let mut settings = Settings::load_from(&path).expect("valid settings");
        assert_eq!(settings.log_level(), "trace");

        settings.set_log_level("error");

        assert_eq!(settings.log_level(), "error");
        Ok(())
    });
}

#[test]
fn save_persists_explicit_changes() {
    Jail::expect_with(|jail| {
        jail.clear_env();
        jail.create_file("config.toml", "log_level = \"debug\"\n")?;

        let path = jail.directory().join("config.toml");
        let mut settings = Settings::load_from(&path).expect("valid settings");
        settings.set_log_level("error");
        settings.save().expect("save");

        assert_eq!(settings.log_level(), "error");

        let contents = fs::read_to_string(&path).expect("saved settings");
        assert!(contents.contains("log_level = \"error\""));
        Ok(())
    });
}

#[test]
fn explicit_set_is_not_written_until_save() {
    Jail::expect_with(|jail| {
        jail.clear_env();
        jail.create_file("config.toml", "log_level = \"debug\"\n")?;

        let path = jail.directory().join("config.toml");
        let mut settings = Settings::load_from(&path).expect("valid settings");
        settings.set_log_level("error");

        let contents = fs::read_to_string(&path).expect("settings file");
        assert!(contents.contains("log_level = \"debug\""));

        settings.save().expect("save");

        let contents = fs::read_to_string(&path).expect("settings file");
        assert!(contents.contains("log_level = \"error\""));
        Ok(())
    });
}

#[test]
fn save_clears_optional_path() {
    Jail::expect_with(|jail| {
        jail.clear_env();
        jail.create_file("config.toml", "log_file = \"/tmp/ticonnect.log\"\n")?;

        let path = jail.directory().join("config.toml");
        let mut settings = Settings::load_from(&path).expect("valid settings");
        assert_eq!(
            settings.log_file().map(|path| path.to_path_buf()),
            Some("/tmp/ticonnect.log".into())
        );

        settings.set_log_file(None);
        settings.save().expect("save");

        assert_eq!(settings.log_file(), None);
        let contents = fs::read_to_string(&path).expect("saved settings");
        assert!(!contents.contains("log_file"));
        Ok(())
    });
}

#[test]
fn reset_restores_defaults_but_keeps_environment() {
    Jail::expect_with(|jail| {
        jail.clear_env();
        jail.create_file("config.toml", "log_level = \"debug\"\n")?;
        jail.set_env("TICONNECT_LOG_LEVEL", "trace");

        let path = jail.directory().join("config.toml");
        let mut settings = Settings::load_from(&path).expect("valid settings");
        assert_eq!(settings.log_level(), "trace");

        settings.reset().expect("reset");

        assert!(!path.exists());
        assert_eq!(settings.log_level(), "trace");
        assert_eq!(settings.log_format(), "human");
        Ok(())
    });
}

#[test]
fn reset_restores_environment_after_explicit_set() {
    Jail::expect_with(|jail| {
        jail.clear_env();
        jail.create_file("config.toml", "log_level = \"debug\"\n")?;
        jail.set_env("TICONNECT_LOG_LEVEL", "trace");

        let path = jail.directory().join("config.toml");
        let mut settings = Settings::load_from(&path).expect("valid settings");
        settings.set_log_level("error");
        assert_eq!(settings.log_level(), "error");

        settings.reset().expect("reset");

        assert_eq!(settings.log_level(), "trace");
        Ok(())
    });
}

#[test]
fn missing_files_yield_default_settings() {
    Jail::expect_with(|jail| {
        jail.clear_env();

        let missing = jail.directory().join("missing.toml");
        let settings = Settings::load_from(&missing).expect("valid settings");

        assert_eq!(settings.log_level(), "info");
        assert_eq!(settings.log_file(), None);
        assert_eq!(settings.log_format(), "human");
        Ok(())
    });
}

#[test]
fn environment_data_dir_override_is_not_persisted() {
    Jail::expect_with(|jail| {
        jail.clear_env();
        let file_data = jail.create_dir("file-data")?;
        let env_data = jail.create_dir("env-data")?;
        jail.create_file(
            "config.toml",
            &format!("data_dir = \"{}\"\n", file_data.display()),
        )?;
        jail.set_env("TICONNECT_DATA_DIR", env_data.display());

        let path = jail.directory().join("config.toml");
        let mut settings = Settings::load_from(&path).expect("valid settings");

        assert_eq!(settings.data_dir().expect("valid data dir"), env_data);
        settings.save().expect("save");

        let contents = fs::read_to_string(&path).expect("saved settings");
        assert!(contents.contains(&file_data.display().to_string()));
        assert!(!contents.contains(&env_data.display().to_string()));
        Ok(())
    });
}

#[test]
fn settings_preserves_unknown_toml_keys() {
    Jail::expect_with(|jail| {
        jail.clear_env();
        jail.create_file(
            "config.toml",
            "log_level = \"debug\"\nfuture_key = \"keep\"\n",
        )?;

        let path = jail.directory().join("config.toml");
        let mut settings = Settings::load_from(&path).expect("valid settings");
        settings.set_log_level("warn");
        settings.save().expect("save");

        let contents = fs::read_to_string(&path).expect("saved settings");
        let table: toml::Table = toml::from_str(&contents).expect("valid toml");
        assert_eq!(table["log_level"].as_str(), Some("warn"));
        assert_eq!(table["future_key"].as_str(), Some("keep"));
        Ok(())
    });
}

#[test]
fn environment_config_dir_selects_default_settings_file() {
    Jail::expect_with(|jail| {
        jail.clear_env();
        let dir = jail.create_dir("custom")?;
        jail.create_file("custom/config.toml", "log_level = \"debug\"\n")?;
        jail.set_env("TICONNECT_CONFIG_DIR", dir.display());

        let settings = Settings::load().expect("valid settings");

        assert_eq!(settings.log_level(), "debug");
        Ok(())
    });
}
