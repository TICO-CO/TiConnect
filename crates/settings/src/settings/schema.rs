use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use super::Settings;
use crate::error::{Error, Result};

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct IdentitySettings {
    pub display_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct DiscoverySettings {
    pub enabled: bool,
    pub mdns: bool,
    pub broadcast: bool,
    pub beacon_interval_secs: u32,
}

impl Default for DiscoverySettings {
    fn default() -> Self {
        Self {
            enabled: true,
            mdns: true,
            broadcast: true,
            beacon_interval_secs: 5,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct TransportSettings {
    pub port: u16,
}

impl Default for TransportSettings {
    fn default() -> Self {
        Self {
            port: DEFAULT_TRANSPORT_PORT,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct OverlaySettings {
    pub enabled: bool,
}

impl Default for OverlaySettings {
    fn default() -> Self {
        Self { enabled: true }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct ReadReceiptsSettings {
    pub enabled: bool,
}

impl Default for ReadReceiptsSettings {
    fn default() -> Self {
        Self { enabled: true }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct OutboxSettings {
    pub ttl_days: u32,
}

impl Default for OutboxSettings {
    fn default() -> Self {
        Self { ttl_days: 7 }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct RingtoneSettings {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<PathBuf>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct DndSettings {
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SettingValue {
    String(String),
    Bool(bool),
    Integer(u32),
    Path(Option<PathBuf>),
}

impl SettingValue {
    pub fn render(&self) -> String {
        match self {
            Self::String(value) => value.clone(),
            Self::Bool(value) => value.to_string(),
            Self::Integer(value) => value.to_string(),
            Self::Path(Some(path)) => path.display().to_string(),
            Self::Path(None) => String::new(),
        }
    }
}

pub const SETTING_KEYS: &[&str] = &[
    "log_level",
    "log_file",
    "log_format",
    "data_dir",
    "identity.display_name",
    "discovery.enabled",
    "discovery.mdns",
    "discovery.broadcast",
    "discovery.beacon_interval_secs",
    "transport.port",
    "overlay.enabled",
    "read_receipts.enabled",
    "outbox.ttl_days",
    "ringtone.path",
    "dnd.enabled",
];

pub const BEACON_INTERVAL_MIN_SECS: u32 = 1;
pub const BEACON_INTERVAL_MAX_SECS: u32 = 3600;
pub const DEFAULT_TRANSPORT_PORT: u16 = 45871;

pub const OUTBOX_TTL_MIN_DAYS: u32 = 1;
pub const OUTBOX_TTL_MAX_DAYS: u32 = 365;

settings_schema! {
    log_level: String = "info".to_owned() => {
        get(&self) -> &str as active {
            active.log_level.as_str()
        };
        set(&mut self, level: impl Into<String>) -> set_log_level as active {
            active.log_level = level.into();
        };
    };

    #[serde(skip_serializing_if = "Option::is_none")]
    log_file: Option<PathBuf> = None => {
        get(&self) -> Option<&Path> as active {
            active.log_file.as_deref()
        };
        set(&mut self, file: Option<PathBuf>) -> set_log_file as active {
            active.log_file = file;
        };
    };

    log_format: String = "human".to_owned() => {
        get(&self) -> &str as active {
            active.log_format.as_str()
        };
        set(&mut self, format: impl Into<String>) -> set_log_format as active {
            active.log_format = format.into();
        };
    };

    #[serde(skip_serializing_if = "Option::is_none")]
    data_dir: Option<PathBuf> = None => {
        get(&self) -> Result<PathBuf> as active, storage {
            storage.data_dir(active.data_dir.as_deref())
        };
        set(&mut self, dir: Option<PathBuf>) -> set_data_dir as active {
            active.data_dir = dir;
        };
    };

    #[serde(default)]
    identity: IdentitySettings = IdentitySettings::default() => {
        get(&self) -> &IdentitySettings as active {
            &active.identity
        };
        set(&mut self, value: IdentitySettings) -> set_identity as active {
            active.identity = value;
        };
    };

    #[serde(default)]
    discovery: DiscoverySettings = DiscoverySettings::default() => {
        get(&self) -> &DiscoverySettings as active {
            &active.discovery
        };
        set(&mut self, value: DiscoverySettings) -> set_discovery as active {
            active.discovery = value;
        };
    };

    #[serde(default)]
    transport: TransportSettings = TransportSettings::default() => {
        get(&self) -> &TransportSettings as active {
            &active.transport
        };
        set(&mut self, value: TransportSettings) -> set_transport as active {
            active.transport = value;
        };
    };

    #[serde(default)]
    overlay: OverlaySettings = OverlaySettings::default() => {
        get(&self) -> &OverlaySettings as active {
            &active.overlay
        };
        set(&mut self, value: OverlaySettings) -> set_overlay as active {
            active.overlay = value;
        };
    };

    #[serde(default)]
    read_receipts: ReadReceiptsSettings = ReadReceiptsSettings::default() => {
        get(&self) -> &ReadReceiptsSettings as active {
            &active.read_receipts
        };
        set(&mut self, value: ReadReceiptsSettings) -> set_read_receipts as active {
            active.read_receipts = value;
        };
    };

    #[serde(default)]
    outbox: OutboxSettings = OutboxSettings::default() => {
        get(&self) -> &OutboxSettings as active {
            &active.outbox
        };
        set(&mut self, value: OutboxSettings) -> set_outbox as active {
            active.outbox = value;
        };
    };

    #[serde(default)]
    ringtone: RingtoneSettings = RingtoneSettings::default() => {
        get(&self) -> &RingtoneSettings as active {
            &active.ringtone
        };
        set(&mut self, value: RingtoneSettings) -> set_ringtone as active {
            active.ringtone = value;
        };
    };

    #[serde(default)]
    dnd: DndSettings = DndSettings::default() => {
        get(&self) -> &DndSettings as active {
            &active.dnd
        };
        set(&mut self, value: DndSettings) -> set_dnd as active {
            active.dnd = value;
        };
    };
}

impl Settings {
    pub fn identity_display_name(&self) -> &str {
        self.active.identity.display_name.as_str()
    }

    pub fn set_identity_display_name(&mut self, name: impl Into<String>) {
        self.changes.identity = true;
        self.active.identity.display_name = name.into();
    }

    pub fn discovery_enabled(&self) -> bool {
        self.active.discovery.enabled
    }

    pub fn set_discovery_enabled(&mut self, enabled: bool) {
        self.changes.discovery = true;
        self.active.discovery.enabled = enabled;
    }

    pub fn discovery_mdns(&self) -> bool {
        self.active.discovery.mdns
    }

    pub fn set_discovery_mdns(&mut self, enabled: bool) {
        self.changes.discovery = true;
        self.active.discovery.mdns = enabled;
    }

    pub fn discovery_broadcast(&self) -> bool {
        self.active.discovery.broadcast
    }

    pub fn set_discovery_broadcast(&mut self, enabled: bool) {
        self.changes.discovery = true;
        self.active.discovery.broadcast = enabled;
    }

    pub fn discovery_beacon_interval_secs(&self) -> u32 {
        self.active.discovery.beacon_interval_secs
    }

    pub fn set_discovery_beacon_interval_secs(&mut self, secs: u32) {
        self.changes.discovery = true;
        self.active.discovery.beacon_interval_secs = secs;
    }

    pub fn transport_port(&self) -> u16 {
        self.active.transport.port
    }

    pub fn set_transport_port(&mut self, port: u16) {
        self.changes.transport = true;
        self.active.transport.port = port;
    }

    pub fn overlay_enabled(&self) -> bool {
        self.active.overlay.enabled
    }

    pub fn set_overlay_enabled(&mut self, enabled: bool) {
        self.changes.overlay = true;
        self.active.overlay.enabled = enabled;
    }

    pub fn read_receipts_enabled(&self) -> bool {
        self.active.read_receipts.enabled
    }

    pub fn set_read_receipts_enabled(&mut self, enabled: bool) {
        self.changes.read_receipts = true;
        self.active.read_receipts.enabled = enabled;
    }

    pub fn outbox_ttl_days(&self) -> u32 {
        self.active.outbox.ttl_days
    }

    pub fn set_outbox_ttl_days(&mut self, days: u32) {
        self.changes.outbox = true;
        self.active.outbox.ttl_days = days;
    }

    pub fn ringtone_path(&self) -> Option<&Path> {
        self.active.ringtone.path.as_deref()
    }

    pub fn set_ringtone_path(&mut self, path: Option<PathBuf>) {
        self.changes.ringtone = true;
        self.active.ringtone.path = path;
    }

    pub fn dnd_enabled(&self) -> bool {
        self.active.dnd.enabled
    }

    pub fn set_dnd_enabled(&mut self, enabled: bool) {
        self.changes.dnd = true;
        self.active.dnd.enabled = enabled;
    }

    pub fn get_setting(&self, key: &str) -> Result<SettingValue> {
        match key {
            "log_level" => Ok(SettingValue::String(self.log_level().to_owned())),
            "log_file" => Ok(SettingValue::Path(self.log_file().map(Path::to_path_buf))),
            "log_format" => Ok(SettingValue::String(self.log_format().to_owned())),
            "data_dir" => Ok(SettingValue::Path(Some(self.data_dir()?))),
            "identity.display_name" => Ok(SettingValue::String(
                self.identity_display_name().to_owned(),
            )),
            "discovery.enabled" => Ok(SettingValue::Bool(self.discovery_enabled())),
            "discovery.mdns" => Ok(SettingValue::Bool(self.discovery_mdns())),
            "discovery.broadcast" => Ok(SettingValue::Bool(self.discovery_broadcast())),
            "discovery.beacon_interval_secs" => {
                Ok(SettingValue::Integer(self.discovery_beacon_interval_secs()))
            }
            "transport.port" => Ok(SettingValue::Integer(u32::from(self.transport_port()))),
            "overlay.enabled" => Ok(SettingValue::Bool(self.overlay_enabled())),
            "read_receipts.enabled" => Ok(SettingValue::Bool(self.read_receipts_enabled())),
            "outbox.ttl_days" => Ok(SettingValue::Integer(self.outbox_ttl_days())),
            "ringtone.path" => Ok(SettingValue::Path(
                self.ringtone_path().map(Path::to_path_buf),
            )),
            "dnd.enabled" => Ok(SettingValue::Bool(self.dnd_enabled())),
            other => Err(Error::UnknownSetting(other.to_owned())),
        }
    }

    pub fn set_setting(&mut self, key: &str, value: &str) -> Result<()> {
        match key {
            "log_level" => self.set_log_level(value),
            "log_file" => self.set_log_file(parse_optional_path(value)),
            "log_format" => self.set_log_format(value),
            "data_dir" => self.set_data_dir(parse_optional_path(value)),
            "identity.display_name" => self.set_identity_display_name(value),
            "discovery.enabled" => {
                self.set_discovery_enabled(parse_bool(key, value)?);
            }
            "discovery.mdns" => {
                self.set_discovery_mdns(parse_bool(key, value)?);
            }
            "discovery.broadcast" => {
                self.set_discovery_broadcast(parse_bool(key, value)?);
            }
            "discovery.beacon_interval_secs" => {
                let secs = value.parse::<u32>().map_err(|_| Error::InvalidSetting {
                    key: key.to_owned(),
                    value: value.to_owned(),
                    reason: "expected an integer".to_owned(),
                })?;
                if !(BEACON_INTERVAL_MIN_SECS..=BEACON_INTERVAL_MAX_SECS).contains(&secs) {
                    return Err(Error::InvalidSetting {
                        key: key.to_owned(),
                        value: value.to_owned(),
                        reason: format!(
                            "expected a value between {BEACON_INTERVAL_MIN_SECS} and {BEACON_INTERVAL_MAX_SECS}"
                        ),
                    });
                }
                self.set_discovery_beacon_interval_secs(secs);
            }
            "transport.port" => {
                let port = value.parse::<u16>().map_err(|_| Error::InvalidSetting {
                    key: key.to_owned(),
                    value: value.to_owned(),
                    reason: "expected an integer between 1 and 65535".to_owned(),
                })?;
                if port == 0 {
                    return Err(Error::InvalidSetting {
                        key: key.to_owned(),
                        value: value.to_owned(),
                        reason: "expected an integer between 1 and 65535".to_owned(),
                    });
                }
                self.set_transport_port(port);
            }
            "overlay.enabled" => self.set_overlay_enabled(parse_bool(key, value)?),
            "read_receipts.enabled" => {
                self.set_read_receipts_enabled(parse_bool(key, value)?);
            }
            "outbox.ttl_days" => {
                let days = value.parse::<u32>().map_err(|_| Error::InvalidSetting {
                    key: key.to_owned(),
                    value: value.to_owned(),
                    reason: "expected an integer".to_owned(),
                })?;
                if !(OUTBOX_TTL_MIN_DAYS..=OUTBOX_TTL_MAX_DAYS).contains(&days) {
                    return Err(Error::InvalidSetting {
                        key: key.to_owned(),
                        value: value.to_owned(),
                        reason: format!(
                            "expected a value between {OUTBOX_TTL_MIN_DAYS} and {OUTBOX_TTL_MAX_DAYS}"
                        ),
                    });
                }
                self.set_outbox_ttl_days(days);
            }
            "ringtone.path" => self.set_ringtone_path(parse_optional_path(value)),
            "dnd.enabled" => self.set_dnd_enabled(parse_bool(key, value)?),
            other => return Err(Error::UnknownSetting(other.to_owned())),
        }
        Ok(())
    }

    pub fn list_settings(&self) -> Result<BTreeMap<String, SettingValue>> {
        let mut values = BTreeMap::new();
        for key in SETTING_KEYS {
            values.insert((*key).to_owned(), self.get_setting(key)?);
        }
        Ok(values)
    }

    pub fn validate(&self) -> Result<()> {
        if !(OUTBOX_TTL_MIN_DAYS..=OUTBOX_TTL_MAX_DAYS).contains(&self.outbox_ttl_days()) {
            return Err(Error::InvalidSetting {
                key: "outbox.ttl_days".to_owned(),
                value: self.outbox_ttl_days().to_string(),
                reason: format!(
                    "expected a value between {OUTBOX_TTL_MIN_DAYS} and {OUTBOX_TTL_MAX_DAYS}"
                ),
            });
        }
        if !(BEACON_INTERVAL_MIN_SECS..=BEACON_INTERVAL_MAX_SECS)
            .contains(&self.discovery_beacon_interval_secs())
        {
            return Err(Error::InvalidSetting {
                key: "discovery.beacon_interval_secs".to_owned(),
                value: self.discovery_beacon_interval_secs().to_string(),
                reason: format!(
                    "expected a value between {BEACON_INTERVAL_MIN_SECS} and {BEACON_INTERVAL_MAX_SECS}"
                ),
            });
        }
        if self.transport_port() == 0 {
            return Err(Error::InvalidSetting {
                key: "transport.port".to_owned(),
                value: self.transport_port().to_string(),
                reason: "expected an integer between 1 and 65535".to_owned(),
            });
        }
        Ok(())
    }
}

fn parse_optional_path(value: &str) -> Option<PathBuf> {
    (!value.is_empty()).then(|| PathBuf::from(value))
}

fn parse_bool(key: &str, value: &str) -> Result<bool> {
    match value {
        "true" | "yes" | "on" | "1" => Ok(true),
        "false" | "no" | "off" | "0" => Ok(false),
        _ => Err(Error::InvalidSetting {
            key: key.to_owned(),
            value: value.to_owned(),
            reason: "expected true or false".to_owned(),
        }),
    }
}
