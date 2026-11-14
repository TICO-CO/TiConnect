use std::os::fd::OwnedFd;

use reis::ei;

use super::{InputBackend, InputError, InputEvent};

pub struct LibeiBackend {
    context: Option<ei::Context>,
    connected: bool,
}

impl LibeiBackend {
    pub fn from_fd(fd: OwnedFd) -> Result<Self, InputError> {
        let context = ei::Context::new(std::os::unix::net::UnixStream::from(fd))
            .map_err(|error| InputError::Backend(error.to_string()))?;
        Ok(Self {
            context: Some(context),
            connected: false,
        })
    }

    pub fn from_environment() -> Result<Option<Self>, InputError> {
        match ei::Context::connect_to_env() {
            Ok(Some(context)) => Ok(Some(Self {
                context: Some(context),
                connected: false,
            })),
            Ok(None) => Ok(None),
            Err(error) => Err(InputError::Backend(error.to_string())),
        }
    }

    pub fn is_available(&self) -> bool {
        self.context.is_some()
    }

    fn flush(&self) -> Result<(), InputError> {
        if let Some(context) = &self.context {
            context
                .flush()
                .map_err(|error| InputError::Backend(error.to_string()))?;
        }
        Ok(())
    }
}

impl InputBackend for LibeiBackend {
    fn inject(&mut self, event: InputEvent) -> Result<(), InputError> {
        event.validate()?;
        if self.context.is_none() {
            return Err(InputError::Unavailable);
        }
        if !self.connected {
            return Err(InputError::Backend(
                "libei handshake has not completed".to_owned(),
            ));
        }
        self.flush()
    }

    fn release(&mut self) {
        self.connected = false;
        self.context.take();
    }
}
