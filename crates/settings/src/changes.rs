use crate::values::Values;

#[derive(Debug, Clone, Default)]
pub(crate) struct Changes {
    pub(crate) log_level: bool,
    pub(crate) log_file: bool,
    pub(crate) log_format: bool,
    pub(crate) data_dir: bool,
}

impl Changes {
    pub(crate) fn is_empty(&self) -> bool {
        !self.log_level && !self.log_file && !self.log_format && !self.data_dir
    }

    pub(crate) fn clear(&mut self) {
        *self = Self::default();
    }

    pub(crate) fn apply(&self, from: &Values, to: &mut Values) {
        if self.log_level {
            to.log_level = from.log_level.clone();
        }
        if self.log_file {
            to.log_file = from.log_file.clone();
        }
        if self.log_format {
            to.log_format = from.log_format;
        }
        if self.data_dir {
            to.data_dir = from.data_dir.clone();
        }
    }
}
