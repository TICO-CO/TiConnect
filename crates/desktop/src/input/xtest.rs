use std::collections::HashMap;

use x11rb::connection::Connection;
use x11rb::protocol::xproto::{
    BUTTON_PRESS_EVENT, BUTTON_RELEASE_EVENT, KEY_PRESS_EVENT, KEY_RELEASE_EVENT,
    MOTION_NOTIFY_EVENT,
};
use x11rb::protocol::xtest::ConnectionExt as _;
use x11rb::rust_connection::RustConnection;

use super::{InputBackend, InputError, InputEvent};

pub struct XtestBackend {
    connection: RustConnection,
    screen: usize,
    keycodes: HashMap<u32, u8>,
}

impl XtestBackend {
    pub fn connect() -> Result<Self, InputError> {
        let (connection, screen) =
            x11rb::connect(None).map_err(|error| InputError::Backend(error.to_string()))?;
        Ok(Self {
            connection,
            screen,
            keycodes: HashMap::new(),
        })
    }

    fn keycode(&mut self, keysym: u32) -> Result<u8, InputError> {
        if let Some(code) = self.keycodes.get(&keysym) {
            return Ok(*code);
        }
        let setup = self.connection.setup();
        let min = setup.min_keycode;
        let max = setup.max_keycode;
        let count = max - min + 1;
        let mapping = x11rb::protocol::xproto::get_keyboard_mapping(&self.connection, min, count)
            .map_err(|error| InputError::Backend(error.to_string()))?
            .reply()
            .map_err(|error| InputError::Backend(error.to_string()))?;
        let per = u16::from(mapping.keysyms_per_keycode);
        for (index, chunk) in mapping.keysyms.chunks(per as usize).enumerate() {
            if chunk.contains(&keysym) {
                let code = min + u8::try_from(index).unwrap_or(0);
                self.keycodes.insert(keysym, code);
                return Ok(code);
            }
        }
        Err(InputError::InvalidKey)
    }

    fn root(&self) -> Result<u32, InputError> {
        self.connection
            .setup()
            .roots
            .get(self.screen)
            .map(|screen| screen.root)
            .ok_or(InputError::Unavailable)
    }

    fn move_pointer(&self, x: i32, y: i32) -> Result<(), InputError> {
        let root = self.root()?;
        self.connection
            .xtest_fake_input(
                MOTION_NOTIFY_EVENT,
                0,
                0,
                root,
                i16::try_from(x).unwrap_or(i16::MAX),
                i16::try_from(y).unwrap_or(i16::MAX),
                0,
            )
            .map_err(|error| InputError::Backend(error.to_string()))?;
        Ok(())
    }
}

impl InputBackend for XtestBackend {
    fn inject(&mut self, event: InputEvent) -> Result<(), InputError> {
        event.validate()?;
        match event {
            InputEvent::PointerMotion { x, y } => self.move_pointer(x, y)?,
            InputEvent::PointerButton { button, pressed } => {
                let event_type = if pressed {
                    BUTTON_PRESS_EVENT
                } else {
                    BUTTON_RELEASE_EVENT
                };
                self.connection
                    .xtest_fake_input(event_type, button, 0, x11rb::NONE, 0, 0, 0)
                    .map_err(|error| InputError::Backend(error.to_string()))?;
            }
            InputEvent::PointerAxis { vertical, .. } => {
                let button = if vertical < 0 { 4 } else { 5 };
                for _ in 0..vertical.unsigned_abs().min(10) {
                    self.connection
                        .xtest_fake_input(BUTTON_PRESS_EVENT, button, 0, x11rb::NONE, 0, 0, 0)
                        .map_err(|error| InputError::Backend(error.to_string()))?;
                    self.connection
                        .xtest_fake_input(BUTTON_RELEASE_EVENT, button, 0, x11rb::NONE, 0, 0, 0)
                        .map_err(|error| InputError::Backend(error.to_string()))?;
                }
            }
            InputEvent::Key { keysym, pressed } => {
                let event_type = if pressed {
                    KEY_PRESS_EVENT
                } else {
                    KEY_RELEASE_EVENT
                };
                let keycode = self.keycode(keysym)?;
                self.connection
                    .xtest_fake_input(event_type, keycode, 0, x11rb::NONE, 0, 0, 0)
                    .map_err(|error| InputError::Backend(error.to_string()))?;
            }
        }
        self.connection
            .flush()
            .map_err(|error| InputError::Backend(error.to_string()))?;
        Ok(())
    }

    fn release(&mut self) {
        let _ = self.connection.flush();
    }
}

pub fn supported() -> bool {
    std::env::var("DISPLAY").is_ok()
}
