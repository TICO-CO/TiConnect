use tokio::sync::broadcast;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActivationEvent {
    Primary,
    Show(String),
    ShowUser(String),
    Duplicate,
}

#[derive(Clone)]
struct UiService {
    sender: broadcast::Sender<ActivationEvent>,
}

#[zbus::interface(name = "org.ticonnect.UI")]
impl UiService {
    async fn activate(&self, view: String) -> zbus::fdo::Result<()> {
        if let Some(user_id) = view.strip_prefix("chat:") {
            user_id
                .parse::<ticonnect_core::id::NodeId>()
                .map_err(|_| zbus::fdo::Error::InvalidArgs("invalid user id".to_owned()))?;
            self.sender
                .send(ActivationEvent::ShowUser(user_id.to_owned()))
                .map_err(|_| zbus::fdo::Error::Failed("UI is not available".to_owned()))?;
            return Ok(());
        }
        let view = crate::client::parse_activation_view(&view)
            .map_err(|error| zbus::fdo::Error::InvalidArgs(error.to_string()))?;
        self.sender
            .send(ActivationEvent::Show(view.to_owned()))
            .map_err(|_| zbus::fdo::Error::Failed("UI is not available".to_owned()))?;
        Ok(())
    }
}

pub async fn serve(sender: broadcast::Sender<ActivationEvent>) -> zbus::Result<()> {
    let registered = ticonnect_core::ipc::connection_builder()
        .allow_name_replacements(false)
        .replace_existing_names(false)
        .name(ticonnect_core::ipc::UI_INTERFACE)
        .serve_at(
            "/org/ticonnect/UI",
            UiService {
                sender: sender.clone(),
            },
        )
        .build()
        .await;

    match registered {
        Ok(_connection) => {
            let _ = sender.send(ActivationEvent::Primary);
            std::future::pending::<()>().await;
            Ok(())
        }
        Err(zbus::Error::NameTaken) => {
            let connection = ticonnect_core::ipc::connect().await?;
            let proxy = zbus::Proxy::new(
                &connection,
                ticonnect_core::ipc::UI_INTERFACE,
                "/org/ticonnect/UI",
                ticonnect_core::ipc::UI_INTERFACE,
            )
            .await?;
            let _: () = proxy.call("Activate", &("chat",)).await?;
            let _ = sender.send(ActivationEvent::Duplicate);
            Ok(())
        }
        Err(error) => {
            let _ = sender.send(ActivationEvent::Primary);
            Err(error)
        }
    }
}
