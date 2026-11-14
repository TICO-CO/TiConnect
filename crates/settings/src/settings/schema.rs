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
    pub timeout_secs: u32,
}

impl Default for OverlaySettings {
    fn default() -> Self {
        Self {
            enabled: true,
            timeout_secs: DEFAULT_OVERLAY_TIMEOUT_SECS,
        }
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

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct AttachmentsSettings {
    pub max_size_mib: u32,
}

impl Default for AttachmentsSettings {
    fn default() -> Self {
        Self {
            max_size_mib: DEFAULT_ATTACHMENT_MAX_MIB,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct CallsSettings {
    pub timeout_secs: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_device: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_device: Option<String>,
}

impl Default for CallsSettings {
    fn default() -> Self {
        Self {
            timeout_secs: DEFAULT_CALL_TIMEOUT_SECS,
            input_device: None,
            output_device: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct RemoteSettings {
    pub session_timeout_secs: u32,
    pub pin_attempts: u32,
    pub pin_ttl_secs: u32,
}

impl Default for RemoteSettings {
    fn default() -> Self {
        Self {
            session_timeout_secs: 0,
            pin_attempts: DEFAULT_REMOTE_PIN_ATTEMPTS,
            pin_ttl_secs: DEFAULT_REMOTE_PIN_TTL_SECS,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct VideoSettings {
    pub bitrate_kbps: u32,
    pub fps: u32,
    pub keyframe_interval: u32,
}

impl Default for VideoSettings {
    fn default() -> Self {
        Self {
            bitrate_kbps: DEFAULT_VIDEO_BITRATE_KBPS,
            fps: DEFAULT_VIDEO_FPS,
            keyframe_interval: DEFAULT_VIDEO_KEYFRAME_INTERVAL,
        }
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

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Appearance {
    #[default]
    System,
    Light,
    Dark,
}

impl Appearance {
    pub const ALL: [Self; 3] = [Self::System, Self::Light, Self::Dark];

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "system" => Some(Self::System),
            "light" => Some(Self::Light),
            "dark" => Some(Self::Dark),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::Light => "light",
            Self::Dark => "dark",
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct AppearanceSettings {
    pub color_scheme: Appearance,
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
    "overlay.timeout_secs",
    "read_receipts.enabled",
    "outbox.ttl_days",
    "attachments.max_size_mib",
    "calls.timeout_secs",
    "calls.input_device",
    "calls.output_device",
    "remote.session_timeout_secs",
    "remote.pin_attempts",
    "remote.pin_ttl_secs",
    "video.bitrate_kbps",
    "video.fps",
    "video.keyframe_interval",
    "ringtone.path",
    "dnd.enabled",
    "appearance.color_scheme",
];

pub const BEACON_INTERVAL_MIN_SECS: u32 = 1;
pub const BEACON_INTERVAL_MAX_SECS: u32 = 3600;
pub const DEFAULT_TRANSPORT_PORT: u16 = 45871;

pub const OUTBOX_TTL_MIN_DAYS: u32 = 1;
pub const OUTBOX_TTL_MAX_DAYS: u32 = 365;

pub const DEFAULT_ATTACHMENT_MAX_MIB: u32 = 2048;
pub const ATTACHMENT_MAX_MIB_MIN: u32 = 1;
pub const ATTACHMENT_MAX_MIB_MAX: u32 = 102_400;
pub const DEFAULT_CALL_TIMEOUT_SECS: u32 = 30;
pub const CALL_TIMEOUT_MIN_SECS: u32 = 5;
pub const CALL_TIMEOUT_MAX_SECS: u32 = 300;
pub const DEFAULT_REMOTE_PIN_ATTEMPTS: u32 = 3;
pub const REMOTE_PIN_ATTEMPTS_MIN: u32 = 1;
pub const REMOTE_PIN_ATTEMPTS_MAX: u32 = 10;
pub const DEFAULT_REMOTE_PIN_TTL_SECS: u32 = 300;
pub const REMOTE_PIN_TTL_MIN_SECS: u32 = 30;
pub const REMOTE_PIN_TTL_MAX_SECS: u32 = 3600;
pub const DEFAULT_VIDEO_BITRATE_KBPS: u32 = 2500;
pub const VIDEO_BITRATE_MIN_KBPS: u32 = 256;
pub const VIDEO_BITRATE_MAX_KBPS: u32 = 20_000;
pub const DEFAULT_VIDEO_FPS: u32 = 30;
pub const VIDEO_FPS_MIN: u32 = 5;
pub const VIDEO_FPS_MAX: u32 = 60;
pub const DEFAULT_VIDEO_KEYFRAME_INTERVAL: u32 = 60;
pub const VIDEO_KEYFRAME_MIN: u32 = 1;
pub const VIDEO_KEYFRAME_MAX: u32 = 600;
pub const DEFAULT_OVERLAY_TIMEOUT_SECS: u32 = 5;
pub const OVERLAY_TIMEOUT_MIN_SECS: u32 = 1;
pub const OVERLAY_TIMEOUT_MAX_SECS: u32 = 60;

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
    attachments: AttachmentsSettings = AttachmentsSettings::default() => {
        get(&self) -> &AttachmentsSettings as active {
            &active.attachments
        };
        set(&mut self, value: AttachmentsSettings) -> set_attachments as active {
            active.attachments = value;
        };
    };

    #[serde(default)]
    calls: CallsSettings = CallsSettings::default() => {
        get(&self) -> &CallsSettings as active {
            &active.calls
        };
        set(&mut self, value: CallsSettings) -> set_calls as active {
            active.calls = value;
        };
    };

    #[serde(default)]
    remote: RemoteSettings = RemoteSettings::default() => {
        get(&self) -> &RemoteSettings as active {
            &active.remote
        };
        set(&mut self, value: RemoteSettings) -> set_remote as active {
            active.remote = value;
        };
    };

    #[serde(default)]
    video: VideoSettings = VideoSettings::default() => {
        get(&self) -> &VideoSettings as active {
            &active.video
        };
        set(&mut self, value: VideoSettings) -> set_video as active {
            active.video = value;
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

    #[serde(default, skip_serializing_if = "Option::is_none")]
    appearance: Option<AppearanceSettings> = None => {
        get(&self) -> Appearance as active {
            active
                .appearance
                .as_ref()
                .map_or(Appearance::System, |appearance| appearance.color_scheme)
        };
        set(&mut self, value: Appearance) -> set_appearance_color_scheme as active {
            active
                .appearance
                .get_or_insert_with(AppearanceSettings::default)
                .color_scheme = value;
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

    pub fn overlay_timeout_secs(&self) -> u32 {
        self.active.overlay.timeout_secs
    }

    pub fn set_overlay_timeout_secs(&mut self, secs: u32) {
        self.changes.overlay = true;
        self.active.overlay.timeout_secs = secs;
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

    pub fn attachment_max_size_mib(&self) -> u32 {
        self.active.attachments.max_size_mib
    }

    pub fn set_attachment_max_size_mib(&mut self, value: u32) {
        self.changes.attachments = true;
        self.active.attachments.max_size_mib = value;
    }

    pub fn call_timeout_secs(&self) -> u32 {
        self.active.calls.timeout_secs
    }

    pub fn set_call_timeout_secs(&mut self, value: u32) {
        self.changes.calls = true;
        self.active.calls.timeout_secs = value;
    }

    pub fn call_input_device(&self) -> Option<&str> {
        self.active.calls.input_device.as_deref()
    }

    pub fn call_output_device(&self) -> Option<&str> {
        self.active.calls.output_device.as_deref()
    }

    pub fn set_call_input_device(&mut self, value: Option<String>) {
        self.changes.calls = true;
        self.active.calls.input_device = value;
    }

    pub fn set_call_output_device(&mut self, value: Option<String>) {
        self.changes.calls = true;
        self.active.calls.output_device = value;
    }

    pub fn remote_session_timeout_secs(&self) -> u32 {
        self.active.remote.session_timeout_secs
    }

    pub fn set_remote_session_timeout_secs(&mut self, value: u32) {
        self.changes.remote = true;
        self.active.remote.session_timeout_secs = value;
    }

    pub fn remote_pin_attempts(&self) -> u32 {
        self.active.remote.pin_attempts
    }

    pub fn remote_pin_ttl_secs(&self) -> u32 {
        self.active.remote.pin_ttl_secs
    }

    pub fn set_remote_pin_attempts(&mut self, value: u32) {
        self.changes.remote = true;
        self.active.remote.pin_attempts = value;
    }

    pub fn set_remote_pin_ttl_secs(&mut self, value: u32) {
        self.changes.remote = true;
        self.active.remote.pin_ttl_secs = value;
    }

    pub fn video_bitrate_kbps(&self) -> u32 {
        self.active.video.bitrate_kbps
    }

    pub fn video_fps(&self) -> u32 {
        self.active.video.fps
    }

    pub fn video_keyframe_interval(&self) -> u32 {
        self.active.video.keyframe_interval
    }

    pub fn set_video_bitrate_kbps(&mut self, value: u32) {
        self.changes.video = true;
        self.active.video.bitrate_kbps = value;
    }

    pub fn set_video_fps(&mut self, value: u32) {
        self.changes.video = true;
        self.active.video.fps = value;
    }

    pub fn set_video_keyframe_interval(&mut self, value: u32) {
        self.changes.video = true;
        self.active.video.keyframe_interval = value;
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

    pub fn stored_appearance_color_scheme(&self) -> Option<Appearance> {
        self.active
            .appearance
            .as_ref()
            .map(|appearance| appearance.color_scheme)
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
            "overlay.timeout_secs" => Ok(SettingValue::Integer(self.overlay_timeout_secs())),
            "read_receipts.enabled" => Ok(SettingValue::Bool(self.read_receipts_enabled())),
            "outbox.ttl_days" => Ok(SettingValue::Integer(self.outbox_ttl_days())),
            "attachments.max_size_mib" => Ok(SettingValue::Integer(self.attachment_max_size_mib())),
            "calls.timeout_secs" => Ok(SettingValue::Integer(self.call_timeout_secs())),
            "calls.input_device" => Ok(SettingValue::String(
                self.call_input_device().unwrap_or_default().to_owned(),
            )),
            "calls.output_device" => Ok(SettingValue::String(
                self.call_output_device().unwrap_or_default().to_owned(),
            )),
            "remote.session_timeout_secs" => {
                Ok(SettingValue::Integer(self.remote_session_timeout_secs()))
            }
            "remote.pin_attempts" => Ok(SettingValue::Integer(self.remote_pin_attempts())),
            "remote.pin_ttl_secs" => Ok(SettingValue::Integer(self.remote_pin_ttl_secs())),
            "video.bitrate_kbps" => Ok(SettingValue::Integer(self.video_bitrate_kbps())),
            "video.fps" => Ok(SettingValue::Integer(self.video_fps())),
            "video.keyframe_interval" => Ok(SettingValue::Integer(self.video_keyframe_interval())),
            "ringtone.path" => Ok(SettingValue::Path(
                self.ringtone_path().map(Path::to_path_buf),
            )),
            "dnd.enabled" => Ok(SettingValue::Bool(self.dnd_enabled())),
            "appearance.color_scheme" => {
                Ok(SettingValue::String(self.appearance().as_str().to_owned()))
            }
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
            "overlay.timeout_secs" => {
                let secs = value.parse::<u32>().map_err(|_| Error::InvalidSetting {
                    key: key.to_owned(),
                    value: value.to_owned(),
                    reason: "expected an integer".to_owned(),
                })?;
                if !(OVERLAY_TIMEOUT_MIN_SECS..=OVERLAY_TIMEOUT_MAX_SECS).contains(&secs) {
                    return Err(Error::InvalidSetting {
                        key: key.to_owned(),
                        value: value.to_owned(),
                        reason: format!(
                            "expected a value between {OVERLAY_TIMEOUT_MIN_SECS} and {OVERLAY_TIMEOUT_MAX_SECS}"
                        ),
                    });
                }
                self.set_overlay_timeout_secs(secs);
            }
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
            "attachments.max_size_mib" => {
                let mib =
                    parse_bounded_u32(key, value, ATTACHMENT_MAX_MIB_MIN, ATTACHMENT_MAX_MIB_MAX)?;
                self.set_attachment_max_size_mib(mib);
            }
            "calls.timeout_secs" => {
                let secs =
                    parse_bounded_u32(key, value, CALL_TIMEOUT_MIN_SECS, CALL_TIMEOUT_MAX_SECS)?;
                self.set_call_timeout_secs(secs);
            }
            "calls.input_device" => self.set_call_input_device(parse_optional_string(value)),
            "calls.output_device" => self.set_call_output_device(parse_optional_string(value)),
            "remote.session_timeout_secs" => {
                let secs = parse_u32(key, value)?;
                self.set_remote_session_timeout_secs(secs);
            }
            "remote.pin_attempts" => {
                let attempts = parse_bounded_u32(
                    key,
                    value,
                    REMOTE_PIN_ATTEMPTS_MIN,
                    REMOTE_PIN_ATTEMPTS_MAX,
                )?;
                self.set_remote_pin_attempts(attempts);
            }
            "remote.pin_ttl_secs" => {
                let secs = parse_bounded_u32(
                    key,
                    value,
                    REMOTE_PIN_TTL_MIN_SECS,
                    REMOTE_PIN_TTL_MAX_SECS,
                )?;
                self.set_remote_pin_ttl_secs(secs);
            }
            "video.bitrate_kbps" => {
                let kbps =
                    parse_bounded_u32(key, value, VIDEO_BITRATE_MIN_KBPS, VIDEO_BITRATE_MAX_KBPS)?;
                self.set_video_bitrate_kbps(kbps);
            }
            "video.fps" => {
                let fps = parse_bounded_u32(key, value, VIDEO_FPS_MIN, VIDEO_FPS_MAX)?;
                self.set_video_fps(fps);
            }
            "video.keyframe_interval" => {
                let interval =
                    parse_bounded_u32(key, value, VIDEO_KEYFRAME_MIN, VIDEO_KEYFRAME_MAX)?;
                self.set_video_keyframe_interval(interval);
            }
            "ringtone.path" => self.set_ringtone_path(parse_optional_path(value)),
            "dnd.enabled" => self.set_dnd_enabled(parse_bool(key, value)?),
            "appearance.color_scheme" => {
                let appearance = Appearance::parse(value).ok_or_else(|| Error::InvalidSetting {
                    key: key.to_owned(),
                    value: value.to_owned(),
                    reason: "expected system, light, or dark".to_owned(),
                })?;
                self.set_appearance_color_scheme(appearance);
            }
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
        if !(OVERLAY_TIMEOUT_MIN_SECS..=OVERLAY_TIMEOUT_MAX_SECS)
            .contains(&self.overlay_timeout_secs())
        {
            return Err(Error::InvalidSetting {
                key: "overlay.timeout_secs".to_owned(),
                value: self.overlay_timeout_secs().to_string(),
                reason: format!(
                    "expected a value between {OVERLAY_TIMEOUT_MIN_SECS} and {OVERLAY_TIMEOUT_MAX_SECS}"
                ),
            });
        }
        if !(ATTACHMENT_MAX_MIB_MIN..=ATTACHMENT_MAX_MIB_MAX)
            .contains(&self.attachment_max_size_mib())
        {
            return Err(Error::InvalidSetting {
                key: "attachments.max_size_mib".to_owned(),
                value: self.attachment_max_size_mib().to_string(),
                reason: format!(
                    "expected a value between {ATTACHMENT_MAX_MIB_MIN} and {ATTACHMENT_MAX_MIB_MAX}"
                ),
            });
        }
        if !(CALL_TIMEOUT_MIN_SECS..=CALL_TIMEOUT_MAX_SECS).contains(&self.call_timeout_secs()) {
            return Err(Error::InvalidSetting {
                key: "calls.timeout_secs".to_owned(),
                value: self.call_timeout_secs().to_string(),
                reason: format!(
                    "expected a value between {CALL_TIMEOUT_MIN_SECS} and {CALL_TIMEOUT_MAX_SECS}"
                ),
            });
        }
        if !(REMOTE_PIN_ATTEMPTS_MIN..=REMOTE_PIN_ATTEMPTS_MAX)
            .contains(&self.remote_pin_attempts())
        {
            return Err(Error::InvalidSetting {
                key: "remote.pin_attempts".to_owned(),
                value: self.remote_pin_attempts().to_string(),
                reason: format!(
                    "expected a value between {REMOTE_PIN_ATTEMPTS_MIN} and {REMOTE_PIN_ATTEMPTS_MAX}"
                ),
            });
        }
        if !(REMOTE_PIN_TTL_MIN_SECS..=REMOTE_PIN_TTL_MAX_SECS)
            .contains(&self.remote_pin_ttl_secs())
        {
            return Err(Error::InvalidSetting {
                key: "remote.pin_ttl_secs".to_owned(),
                value: self.remote_pin_ttl_secs().to_string(),
                reason: format!(
                    "expected a value between {REMOTE_PIN_TTL_MIN_SECS} and {REMOTE_PIN_TTL_MAX_SECS}"
                ),
            });
        }
        if !(VIDEO_BITRATE_MIN_KBPS..=VIDEO_BITRATE_MAX_KBPS).contains(&self.video_bitrate_kbps()) {
            return Err(Error::InvalidSetting {
                key: "video.bitrate_kbps".to_owned(),
                value: self.video_bitrate_kbps().to_string(),
                reason: format!(
                    "expected a value between {VIDEO_BITRATE_MIN_KBPS} and {VIDEO_BITRATE_MAX_KBPS}"
                ),
            });
        }
        if !(VIDEO_FPS_MIN..=VIDEO_FPS_MAX).contains(&self.video_fps()) {
            return Err(Error::InvalidSetting {
                key: "video.fps".to_owned(),
                value: self.video_fps().to_string(),
                reason: format!("expected a value between {VIDEO_FPS_MIN} and {VIDEO_FPS_MAX}"),
            });
        }
        if !(VIDEO_KEYFRAME_MIN..=VIDEO_KEYFRAME_MAX).contains(&self.video_keyframe_interval()) {
            return Err(Error::InvalidSetting {
                key: "video.keyframe_interval".to_owned(),
                value: self.video_keyframe_interval().to_string(),
                reason: format!(
                    "expected a value between {VIDEO_KEYFRAME_MIN} and {VIDEO_KEYFRAME_MAX}"
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

fn parse_optional_string(value: &str) -> Option<String> {
    (!value.trim().is_empty()).then(|| value.trim().to_owned())
}

fn parse_u32(key: &str, value: &str) -> Result<u32> {
    value.parse::<u32>().map_err(|_| Error::InvalidSetting {
        key: key.to_owned(),
        value: value.to_owned(),
        reason: "expected an integer".to_owned(),
    })
}

fn parse_bounded_u32(key: &str, value: &str, min: u32, max: u32) -> Result<u32> {
    let value = parse_u32(key, value)?;
    if !(min..=max).contains(&value) {
        return Err(Error::InvalidSetting {
            key: key.to_owned(),
            value: value.to_string(),
            reason: format!("expected a value between {min} and {max}"),
        });
    }
    Ok(value)
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
