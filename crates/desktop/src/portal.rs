use std::os::fd::OwnedFd;

use ashpd::desktop::remote_desktop::{DeviceType, RemoteDesktop, SelectDevicesOptions};
use ashpd::desktop::screencast::{CursorMode, Screencast, SelectSourcesOptions, SourceType};
use ashpd::desktop::{PersistMode, Session};
use thiserror::Error;

pub struct PortalSession {
    remote: RemoteDesktop,
    screencast: Screencast,
    session: Session<RemoteDesktop>,
    eis: Option<OwnedFd>,
    pipewire: Option<OwnedFd>,
    width: u32,
    height: u32,
}

impl PortalSession {
    pub async fn open() -> Result<Self, PortalError> {
        let remote = RemoteDesktop::new().await.map_err(portal_error)?;
        let screencast = Screencast::new().await.map_err(portal_error)?;
        let session = remote
            .create_session(Default::default())
            .await
            .map_err(portal_error)?;
        remote
            .select_devices(
                &session,
                SelectDevicesOptions::default()
                    .set_devices(DeviceType::KEYBOARD | DeviceType::POINTER),
            )
            .await
            .map_err(portal_error)?;
        screencast
            .select_sources(
                &session,
                SelectSourcesOptions::default()
                    .set_cursor_mode(CursorMode::METADATA)
                    .set_sources(SourceType::MONITOR)
                    .set_multiple(false)
                    .set_persist_mode(PersistMode::DoNot),
            )
            .await
            .map_err(portal_error)?;
        let response = remote
            .start(&session, None, Default::default())
            .await
            .map_err(portal_error)?
            .response()
            .map_err(portal_error)?;
        let (width, height) = response
            .streams()
            .first()
            .and_then(|stream| stream.size())
            .map_or((1920, 1080), |size| {
                (
                    u32::try_from(size.0).unwrap_or(1920),
                    u32::try_from(size.1).unwrap_or(1080),
                )
            });
        let eis = remote
            .connect_to_eis(&session, Default::default())
            .await
            .ok();
        let pipewire = screencast
            .open_pipe_wire_remote(&session, Default::default())
            .await
            .ok();
        Ok(Self {
            remote,
            screencast,
            session,
            eis,
            pipewire,
            width,
            height,
        })
    }

    pub fn size(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    pub fn take_eis(&mut self) -> Option<OwnedFd> {
        self.eis.take()
    }

    pub fn take_pipewire(&mut self) -> Option<OwnedFd> {
        self.pipewire.take()
    }

    pub async fn close(self) -> Result<(), PortalError> {
        self.session.close().await.map_err(portal_error)?;
        let _ = (&self.remote, &self.screencast);
        Ok(())
    }
}

#[derive(Debug, Error)]
pub enum PortalError {
    #[error("desktop portal failed: {0}")]
    Portal(String),

    #[error("desktop portal denied the request")]
    Denied,

    #[error("desktop portal did not provide a capture stream")]
    NoStream,
}

fn portal_error(error: ashpd::Error) -> PortalError {
    PortalError::Portal(error.to_string())
}
