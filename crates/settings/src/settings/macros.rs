macro_rules! settings_schema {
    (
        $(
            $(#[$field_attr:meta])*
            $field:ident : $ty:ty = $default:expr => {
                get (&self) $(-> $get_ret:ty)? as $get_active:ident $(, $get_storage:ident)? $get_body:block;
                set (&mut self $(, $arg_name:ident : $arg_ty:ty)*) -> $setter:ident as $set_active:ident $set_body:block;
            };
        )*
    ) => {
        #[derive(Debug, Clone, PartialEq, serde::Deserialize, serde::Serialize)]
        #[serde(default)]
        pub struct SettingsValues {
            $(
                $(#[$field_attr])*
                pub $field: $ty,
            )*
        }

        impl Default for SettingsValues {
            fn default() -> Self {
                Self {
                    $(
                        $field: $default,
                    )*
                }
            }
        }

        #[derive(Debug, Clone, Default)]
        pub struct SettingsChanges {
            $(
                $field: bool,
            )*
        }

        impl SettingsChanges {
            pub fn is_empty(&self) -> bool {
                $( !self.$field && )* true
            }

            pub fn clear(&mut self) {
                *self = Self::default();
            }

            pub fn apply(&self, from: &SettingsValues, to: &mut SettingsValues) {
                $(
                    if self.$field {
                        to.$field = from.$field.clone();
                    }
                )*
            }
        }

        impl Settings {
            $(
                pub fn $field(&self) $(-> $get_ret)? {
                    let $get_active = &self.active;
                    $(let $get_storage = &self.storage;)?
                    $get_body
                }

                pub fn $setter(&mut self $(, $arg_name: $arg_ty)*) {
                    self.changes.$field = true;
                    let $set_active = &mut self.active;
                    $set_body
                }
            )*
        }
    };
}
