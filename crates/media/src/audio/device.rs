use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioDevice {
    pub id: String,
    pub name: String,
    pub is_input: bool,
}

#[derive(Debug, Clone, Default)]
pub struct AudioDevices {
    pub inputs: Vec<AudioDevice>,
    pub outputs: Vec<AudioDevice>,
}

impl AudioDevices {
    pub fn discover() -> Result<Self, DeviceError> {
        use cpal::traits::{DeviceTrait, HostTrait};
        let host = cpal::default_host();
        let mut inputs = Vec::new();
        let mut outputs = Vec::new();
        for device in host.input_devices().map_err(device_error)? {
            let name = device.description().map_or_else(
                |_| "unknown input".to_owned(),
                |description| description.name().to_owned(),
            );
            inputs.push(AudioDevice {
                id: name.clone(),
                name,
                is_input: true,
            });
        }
        for device in host.output_devices().map_err(device_error)? {
            let name = device.description().map_or_else(
                |_| "unknown output".to_owned(),
                |description| description.name().to_owned(),
            );
            outputs.push(AudioDevice {
                id: name.clone(),
                name,
                is_input: false,
            });
        }
        Ok(Self { inputs, outputs })
    }

    pub fn input(&self, id: Option<&str>) -> Option<&AudioDevice> {
        match id {
            Some(id) => self.inputs.iter().find(|device| device.id == id),
            None => self.inputs.first(),
        }
    }

    pub fn output(&self, id: Option<&str>) -> Option<&AudioDevice> {
        match id {
            Some(id) => self.outputs.iter().find(|device| device.id == id),
            None => self.outputs.first(),
        }
    }
}

#[derive(Debug, Error)]
pub enum DeviceError {
    #[error("audio host is unavailable: {0}")]
    Host(String),

    #[error("audio device is unavailable")]
    Missing,
}

fn device_error(error: impl std::fmt::Display) -> DeviceError {
    DeviceError::Host(error.to_string())
}
