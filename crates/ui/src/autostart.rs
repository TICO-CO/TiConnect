use std::fs;
use std::path::PathBuf;

use crate::error::{Error, Result};

const TEMPLATE: &str = "[Desktop Entry]\nType=Application\nName=TiConnect\nComment=LAN-only messaging, calls, and remote desktop\nExec=ticonnectd\nHidden=false\nNoDisplay=true\nX-GNOME-Autostart-enabled=true\n";

pub fn path() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))?;
    Some(base.join("autostart").join("ticonnect.desktop"))
}

pub fn enabled() -> bool {
    let Some(path) = path() else {
        return false;
    };
    let Ok(contents) = fs::read_to_string(path) else {
        return false;
    };
    entry_enabled(&contents)
}

pub fn set_enabled(enabled: bool) -> Result<()> {
    let Some(path) = path() else {
        return Err(Error::Autostart(
            "cannot locate the XDG config directory".to_owned(),
        ));
    };
    let existing = fs::read_to_string(&path).unwrap_or_else(|_| TEMPLATE.to_owned());
    if !enabled && !path.exists() {
        return Ok(());
    }
    let contents = set_entry_enabled(&existing, enabled);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| Error::Autostart(format!("create autostart directory: {error}")))?;
    }
    fs::write(&path, contents)
        .map_err(|error| Error::Autostart(format!("write autostart entry: {error}")))?;
    set_mode(&path)
}

fn entry_enabled(contents: &str) -> bool {
    let hidden = key_value(contents, "Hidden");
    let enabled = key_value(contents, "X-GNOME-Autostart-enabled");
    hidden != Some("true") && enabled != Some("false")
}

pub fn set_entry_enabled(contents: &str, enabled: bool) -> String {
    let mut lines: Vec<String> = contents.lines().map(ToOwned::to_owned).collect();
    set_key(&mut lines, "Hidden", if enabled { "false" } else { "true" });
    set_key(
        &mut lines,
        "X-GNOME-Autostart-enabled",
        if enabled { "true" } else { "false" },
    );
    let mut output = lines.join("\n");
    output.push('\n');
    output
}

fn key_value<'a>(contents: &'a str, key: &str) -> Option<&'a str> {
    contents.lines().find_map(|line| {
        let (name, value) = line.split_once('=')?;
        (name.trim() == key).then_some(value.trim())
    })
}

fn set_key(lines: &mut Vec<String>, key: &str, value: &str) {
    if let Some(line) = lines.iter_mut().find(|line| {
        line.split_once('=')
            .is_some_and(|(name, _)| name.trim() == key)
    }) {
        *line = format!("{key}={value}");
    } else {
        lines.push(format!("{key}={value}"));
    }
}

#[cfg(unix)]
fn set_mode(path: &std::path::Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o644))
        .map_err(|error| Error::Autostart(format!("set autostart permissions: {error}")))
}

#[cfg(not(unix))]
fn set_mode(_path: &std::path::Path) -> Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_flags_mean_enabled() {
        assert!(entry_enabled("[Desktop Entry]\nName=TiConnect\n"));
    }

    #[test]
    fn hidden_entry_is_disabled() {
        assert!(!entry_enabled(
            "Hidden=true\nX-GNOME-Autostart-enabled=true\n"
        ));
        assert!(!entry_enabled(
            "Hidden=false\nX-GNOME-Autostart-enabled=false\n"
        ));
    }

    #[test]
    fn toggling_preserves_other_keys() {
        let input = "[Desktop Entry]\nName=TiConnect\nExec=/usr/bin/ticonnectd\n";
        let disabled = set_entry_enabled(input, false);
        assert!(disabled.contains("Exec=/usr/bin/ticonnectd"));
        assert!(disabled.contains("Hidden=true"));
        assert!(disabled.contains("X-GNOME-Autostart-enabled=false"));
        let enabled = set_entry_enabled(&disabled, true);
        assert!(enabled.contains("Hidden=false"));
        assert!(enabled.contains("X-GNOME-Autostart-enabled=true"));
    }
}
