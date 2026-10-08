//! The macro that declares a port trait together with the facade methods forwarding to it.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

/// Declares a port trait and the facade methods that forward to it from one list.
///
/// Each listed method returns `ExtensionResult<T>`; the facade reaches its port through the
/// field `$access`. The optional `extra` block adds trait methods the facade implements by
/// hand, and the optional supertrait is the port a wider port extends.
macro_rules! port {
    (
        $(#[$port_meta:meta])*
        $port:ident $(: $super:ident)? for $facade:ident via $access:tt {
            $(
                $(#[$meta:meta])*
                fn $method:ident($($arg:ident: $ty:ty),* $(,)?) -> $ret:ty;
            )*
        }
        $(extra { $($extra:tt)* })?
    ) => {
        $(#[$port_meta])*
        pub trait $port $(: $super)? {
            $(
                $(#[$meta])*
                ///
                /// # Errors
                /// Returns the host's message when the host rejects the request.
                fn $method(&self $(, $arg: $ty)*) -> ExtensionResult<$ret>;
            )*
            $($($extra)*)?
        }

        impl $facade {
            $(
                $(#[$meta])*
                ///
                /// # Errors
                /// Returns the host's message when the host rejects the request.
                pub fn $method(&self $(, $arg: $ty)*) -> ExtensionResult<$ret> {
                    self.$access.$method($($arg),*)
                }
            )*
        }
    };
}
