macro_rules! settings_schema {
    (
        $(
            $(#[$field_attr:meta])*
            $field:ident : $ty:ty = $default:expr,
            get fn $get_name:ident (&self) $(-> $get_ret:ty)? as $get_values:ident $get_body:block,
            set fn $set_name:ident (&mut self $(, $arg_name:ident : $arg_ty:ty)*) as $set_values:ident $set_body:block;
        )*
    ) => {
        #[derive(Debug, Clone, PartialEq, serde::Deserialize, serde::Serialize)]
        #[serde(default)]
        pub(crate) struct Values {
            $(
                $(#[$field_attr])*
                pub(crate) $field: $ty,
            )*
        }

        impl Default for Values {
            fn default() -> Self {
                Self {
                    $(
                        $field: $default,
                    )*
                }
            }
        }

        #[derive(Debug, Clone, Default)]
        pub(crate) struct Changes {
            $(
                pub(crate) $field: bool,
            )*
        }

        impl Changes {
            pub(crate) fn is_empty(&self) -> bool {
                $( !self.$field && )* true
            }

            pub(crate) fn clear(&mut self) {
                *self = Self::default();
            }

            pub(crate) fn apply(&self, from: &Values, to: &mut Values) {
                $(
                    if self.$field {
                        to.$field = from.$field.clone();
                    }
                )*
            }
        }

        impl Settings {
            $(
                pub fn $get_name(&self) $(-> $get_ret)? {
                    let $get_values = &self.active;
                    $get_body
                }

                pub fn $set_name(&mut self $(, $arg_name: $arg_ty)*) {
                    self.changes.$field = true;
                    let $set_values = &mut self.active;
                    $set_body
                }
            )*
        }
    };
}
