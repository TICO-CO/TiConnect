use std::sync::Arc;

use ksni::TrayMethods;
use ksni::menu::{CheckmarkItem, MenuItem, RadioGroup, RadioItem, StandardItem, SubMenu};
use ticonnect_core::ipc;
use tokio::task::JoinHandle;

use crate::messaging::MessagingService;
use crate::runtime::Shutdown;
use crate::storage::UserPolicy;

const REFRESH_INTERVAL: std::time::Duration = std::time::Duration::from_secs(5);

#[derive(Debug, Clone)]
struct UserMenu {
    id: String,
    label: String,
    policy: UserPolicy,
}

struct Tray {
    messaging: MessagingService,
    shutdown: Arc<Shutdown>,
    connection: zbus::Connection,
    users: Vec<UserMenu>,
    dnd: bool,
}

pub fn spawn(
    connection: zbus::Connection,
    messaging: MessagingService,
    shutdown: Arc<Shutdown>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let tray = Tray {
            messaging: messaging.clone(),
            shutdown,
            connection,
            users: Vec::new(),
            dnd: messaging.dnd_enabled(),
        };
        let handle = match tray.assume_sni_available(true).spawn().await {
            Ok(handle) => handle,
            Err(error) => {
                tracing::info!(%error, "system tray unavailable; continuing without it");
                return;
            }
        };

        let mut interval = tokio::time::interval(REFRESH_INTERVAL);
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            interval.tick().await;
            let users = match messaging.database().list_users().await {
                Ok(users) => users
                    .into_iter()
                    .map(|user| UserMenu {
                        label: user
                            .display_name
                            .clone()
                            .filter(|name| !name.is_empty())
                            .unwrap_or_else(|| user.id.clone()),
                        id: user.id,
                        policy: user.policy,
                    })
                    .collect::<Vec<_>>(),
                Err(error) => {
                    tracing::debug!(%error, "failed to refresh tray users");
                    continue;
                }
            };
            let dnd = messaging.dnd_enabled();
            let _ = handle
                .update(move |tray: &mut Tray| {
                    tray.users = users;
                    tray.dnd = dnd;
                })
                .await;
        }
    })
}

impl ksni::Tray for Tray {
    fn id(&self) -> String {
        "ticonnect".to_owned()
    }

    fn title(&self) -> String {
        "TiConnect".to_owned()
    }

    fn icon_name(&self) -> String {
        "mail-message-new".to_owned()
    }

    fn menu(&self) -> Vec<MenuItem<Self>> {
        let mut items = vec![
            StandardItem {
                label: "Open TiConnect".to_owned(),
                icon_name: "mail-message-new".to_owned(),
                activate: Box::new(|tray: &mut Self| {
                    let connection = tray.connection.clone();
                    tokio::spawn(async move {
                        let _ = connection
                            .call_method(
                                Some("org.ticonnect.UI"),
                                "/org/ticonnect/UI",
                                Some("org.ticonnect.UI"),
                                "Activate",
                                &("chat",),
                            )
                            .await;
                    });
                }),
                ..Default::default()
            }
            .into(),
            CheckmarkItem {
                label: "Do not disturb".to_owned(),
                checked: self.dnd,
                activate: Box::new(|tray: &mut Self| {
                    tray.dnd = !tray.dnd;
                    let messaging = tray.messaging.clone();
                    let enabled = tray.dnd;
                    tokio::spawn(async move {
                        if let Err(error) = messaging.set_dnd(enabled).await {
                            tracing::warn!(%error, "failed to toggle DND from tray");
                        }
                    });
                }),
                ..Default::default()
            }
            .into(),
        ];

        if !self.users.is_empty() {
            items.push(MenuItem::Separator);
            items.push(
                SubMenu {
                    label: "User policy".to_owned(),
                    submenu: self.users.iter().map(user_menu).collect(),
                    ..Default::default()
                }
                .into(),
            );
        }

        items.push(MenuItem::Separator);
        items.push(
            StandardItem {
                label: "Quit".to_owned(),
                icon_name: "application-exit".to_owned(),
                activate: Box::new(|tray: &mut Self| {
                    tray.shutdown.request();
                }),
                ..Default::default()
            }
            .into(),
        );
        items
    }
}

fn user_menu(user: &UserMenu) -> MenuItem<Tray> {
    let id = user.id.clone();
    let selected = match user.policy {
        UserPolicy::Accept => 0,
        UserPolicy::Mute => 1,
        UserPolicy::Ignore => 2,
    };
    SubMenu {
        label: user.label.clone(),
        submenu: vec![
            RadioGroup {
                selected,
                select: Box::new(move |tray: &mut Tray, index| {
                    let policy = match index {
                        1 => UserPolicy::Mute,
                        2 => UserPolicy::Ignore,
                        _ => UserPolicy::Accept,
                    };
                    let messaging = tray.messaging.clone();
                    let user_id = id.clone();
                    tokio::spawn(async move {
                        if let Err(error) = messaging.set_user_policy(&user_id, policy).await {
                            tracing::warn!(%error, "failed to set user policy from tray");
                        }
                    });
                }),
                options: vec![
                    RadioItem {
                        label: "Accept".to_owned(),
                        ..Default::default()
                    },
                    RadioItem {
                        label: "Mute".to_owned(),
                        ..Default::default()
                    },
                    RadioItem {
                        label: "Ignore".to_owned(),
                        ..Default::default()
                    },
                ],
            }
            .into(),
        ],
        ..Default::default()
    }
    .into()
}

pub fn activate_interface() -> &'static str {
    ipc::UI_INTERFACE
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn policy_index_matches_tray_order() {
        for (policy, selected) in [
            (UserPolicy::Accept, 0),
            (UserPolicy::Mute, 1),
            (UserPolicy::Ignore, 2),
        ] {
            let user = UserMenu {
                id: "user".to_owned(),
                label: "User".to_owned(),
                policy,
            };
            let menu = user_menu(&user);
            let MenuItem::SubMenu(submenu) = menu else {
                panic!("expected submenu");
            };
            let MenuItem::RadioGroup(group) = &submenu.submenu[0] else {
                panic!("expected radio group");
            };
            assert_eq!(group.selected, selected);
        }
    }
}
