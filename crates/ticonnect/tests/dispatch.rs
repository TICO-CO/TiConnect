use std::ffi::OsString;
use std::process::Command;

use ticonnect::{Action, parse};

fn parse_args(args: &[&str]) -> Action {
    parse(args.iter().map(OsString::from)).expect("parse action")
}

#[test]
fn parses_launch_and_help_actions() {
    assert_eq!(parse_args(&[]), Action::App);
    assert_eq!(parse_args(&["--help"]), Action::Help);
    assert_eq!(parse_args(&["--version"]), Action::Version);
    assert_eq!(parse_args(&["version"]), Action::Version);
}

#[test]
fn parses_internal_process_entries() {
    assert_eq!(parse_args(&["daemon"]), Action::Daemon);
    assert_eq!(parse_args(&["overlay"]), Action::Overlay);
    assert_eq!(parse_args(&["ui"]), Action::Ui);
}

#[test]
fn parses_cli_commands() {
    assert_eq!(
        parse_args(&["cli", "status"]),
        Action::Cli(vec![OsString::from("status")])
    );
    assert_eq!(
        parse_args(&["status"]),
        Action::Cli(vec![OsString::from("status")])
    );
    assert_eq!(
        parse_args(&["pair", "start"]),
        Action::Cli(vec![OsString::from("pair"), OsString::from("start")])
    );
    assert_eq!(
        parse_args(&["status", "--help"]),
        Action::Cli(vec![OsString::from("status"), OsString::from("--help")])
    );
}

#[test]
fn parses_install_actions() {
    assert_eq!(
        parse_args(&["--install-user"]),
        Action::InstallUser { force: false }
    );
    assert_eq!(
        parse_args(&["--uninstall-user", "--purge"]),
        Action::UninstallUser {
            purge: true,
            force: false
        }
    );
}

#[test]
fn rejects_unknown_leading_options() {
    let error = parse(["--bogus"]).expect_err("unknown option");
    assert!(error.to_string().contains("--bogus"));
}

#[test]
fn help_hides_internal_entry_points() {
    let output = Command::new(env!("CARGO_BIN_EXE_ticonnect"))
        .arg("--help")
        .output()
        .expect("run ticonnect");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Launch TiConnect"));
    let words: Vec<&str> = stdout
        .split_whitespace()
        .map(|word| word.trim_matches(|c: char| !c.is_alphanumeric()))
        .collect();
    for hidden in ["daemon", "overlay", "cli"] {
        assert!(!words.contains(&hidden), "help mentions {hidden}");
    }
}

#[test]
fn version_prints_workspace_version() {
    let output = Command::new(env!("CARGO_BIN_EXE_ticonnect"))
        .arg("--version")
        .output()
        .expect("run ticonnect");
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        format!("ticonnect {}", env!("CARGO_PKG_VERSION"))
    );
}

#[test]
fn unknown_option_exits_two_with_help() {
    let output = Command::new(env!("CARGO_BIN_EXE_ticonnect"))
        .arg("--bogus")
        .output()
        .expect("run ticonnect");
    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("Usage:"));
}
