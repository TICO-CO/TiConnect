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
fn save_merges_only_explicit_changes_across_handles() {
    Jail::expect_with(|jail| {
        jail.clear_env();
        jail.create_file("config.toml", "log_level = \"debug\"\n")?;

        let path = jail.directory().join("config.toml");
        let mut first = Settings::load_from(&path).expect("valid settings");
        let mut second = Settings::load_from(&path).expect("valid settings");

        first.set_log_level("warn");
        second.set_log_format("json");
        first.save().expect("first save");
        second.save().expect("second save");

        let contents = fs::read_to_string(&path).expect("saved settings");
        let table: toml::Table = toml::from_str(&contents).expect("valid toml");
        assert_eq!(table["log_level"].as_str(), Some("warn"));
        assert_eq!(table["log_format"].as_str(), Some("json"));
        Ok(())
    });
}

#[test]
fn save_preserves_external_edit_to_untouched_field() {
    Jail::expect_with(|jail| {
        jail.clear_env();
        jail.create_file("config.toml", "log_level = \"debug\"\n")?;

        let path = jail.directory().join("config.toml");
        let mut settings = Settings::load_from(&path).expect("valid settings");
        fs::write(&path, "log_level = \"trace\"\n").expect("external edit");

        settings.set_log_format("json");
        settings.save().expect("save");

        let contents = fs::read_to_string(&path).expect("saved settings");
        let table: toml::Table = toml::from_str(&contents).expect("valid toml");
        assert_eq!(table["log_level"].as_str(), Some("trace"));
        assert_eq!(table["log_format"].as_str(), Some("json"));
        Ok(())
    });
}

#[test]
fn noop_save_does_not_create_directories() {
    Jail::expect_with(|jail| {
        jail.clear_env();
        let path = jail.directory().join("nested/deeper/config.toml");
        let mut settings = Settings::load_from(&path).expect("valid settings");

        settings.save().expect("no-op save");

        assert!(!path.parent().expect("parent").exists());
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

#[test]
fn nested_settings_save_as_toml_tables() {
    Jail::expect_with(|jail| {
        jail.clear_env();
        let path = jail.directory().join("config.toml");
        let mut settings = Settings::load_from(&path).expect("valid settings");

        settings.set_dnd_enabled(true);
        settings.set_outbox_ttl_days(14);
        settings.set_identity_display_name("Desk");
        settings.save().expect("save");

        let contents = fs::read_to_string(&path).expect("saved settings");
        let table: toml::Table = toml::from_str(&contents).expect("valid toml");
        assert_eq!(table["dnd"]["enabled"].as_bool(), Some(true));
        assert_eq!(table["outbox"]["ttl_days"].as_integer(), Some(14));
        assert_eq!(table["identity"]["display_name"].as_str(), Some("Desk"));
        Ok(())
    });
}

#[test]
fn nested_environment_override_uses_double_underscore() {
    Jail::expect_with(|jail| {
        jail.clear_env();
        jail.set_env("TICONNECT_DND__ENABLED", "true");
        jail.set_env("TICONNECT_OUTBOX__TTL_DAYS", "21");

        let path = jail.directory().join("missing.toml");
        let settings = Settings::load_from(&path).expect("valid settings");

        assert!(settings.dnd_enabled());
        assert_eq!(settings.outbox_ttl_days(), 21);
        Ok(())
    });
}

#[test]
fn invalid_outbox_ttl_is_rejected_on_load() {
    Jail::expect_with(|jail| {
        jail.clear_env();
        jail.create_file("config.toml", "[outbox]\nttl_days = 0\n")?;

        let path = jail.directory().join("config.toml");
        let error = Settings::load_from(&path).expect_err("invalid ttl");

        assert!(matches!(error, crate::Error::InvalidSetting { .. }));
        Ok(())
    });
}

#[test]
fn key_api_sets_and_reads_typed_values() {
    Jail::expect_with(|jail| {
        jail.clear_env();
        let path = jail.directory().join("config.toml");
        let mut settings = Settings::load_from(&path).expect("valid settings");

        settings.set_setting("dnd.enabled", "yes").expect("set dnd");
        settings
            .set_setting("ringtone.path", "/tmp/ring.ogg")
            .expect("set ringtone");
        settings
            .set_setting("outbox.ttl_days", "30")
            .expect("set ttl");
        settings.save().expect("save");

        let settings = Settings::load_from(&path).expect("reload");
        assert_eq!(
            settings.get_setting("dnd.enabled").expect("get dnd"),
            crate::SettingValue::Bool(true)
        );
        assert_eq!(
            settings.get_setting("outbox.ttl_days").expect("get ttl"),
            crate::SettingValue::Integer(30)
        );
        assert_eq!(
            settings
                .get_setting("ringtone.path")
                .expect("get ringtone")
                .render(),
            "/tmp/ring.ogg"
        );
        assert!(
            settings
                .list_settings()
                .expect("list")
                .contains_key("discovery.enabled")
        );
        Ok(())
    });
}

#[test]
fn key_api_rejects_unknown_and_invalid_values() {
    Jail::expect_with(|jail| {
        jail.clear_env();
        let path = jail.directory().join("config.toml");
        let mut settings = Settings::load_from(&path).expect("valid settings");

        assert!(matches!(
            settings.get_setting("missing.key"),
            Err(crate::Error::UnknownSetting(_))
        ));
        assert!(matches!(
            settings.set_setting("outbox.ttl_days", "999"),
            Err(crate::Error::InvalidSetting { .. })
        ));
        assert!(matches!(
            settings.set_setting("dnd.enabled", "maybe"),
            Err(crate::Error::InvalidSetting { .. })
        ));
        Ok(())
    });
}
