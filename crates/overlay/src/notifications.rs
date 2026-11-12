use notify_rust::{Notification, Timeout};
use tokio::sync::mpsc::UnboundedSender;

use crate::card::Card;
use crate::client::Command;

pub fn show(card: Card, timeout_ms: u64, commands: UnboundedSender<Command>) {
    std::thread::spawn(move || {
        let mut notification = Notification::new();
        notification
            .summary(&card.sender)
            .body(&card.preview)
            .icon("dialog-information")
            .timeout(Timeout::Milliseconds(
                u32::try_from(timeout_ms).unwrap_or(u32::MAX),
            ))
            .action("default", "Open");
        match notification.show() {
            Ok(handle) => handle.wait_for_action(move |action| {
                if action == "default" {
                    let _ = commands.send(Command::Activate("chat".to_owned()));
                }
            }),
            Err(error) => tracing::warn!(%error, "system notification failed"),
        }
    });
}
