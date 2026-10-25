//! The macro that declares a port trait together with the facade methods forwarding to it.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

/// Declares a port trait and the facade methods that forward to it from one list.
///
/// Ordinary methods return `ExtensionResult<T>`; the optional infallible block returns directly.
/// An asynchronous block adds trait declarations with explicit facade implementations.
/// The facade reaches its port through the
/// field `$access`. An optional `errors` string describes fallible delegated operations.
macro_rules! port {
    (
        $(#[$port_meta:meta])*
        $port:ident for $facade:ident via $access:tt {
            errors $errors:literal;
            $(
                $(#[$meta:meta])*
                fn $method:ident($($arg:ident: $ty:ty),* $(,)?) -> $ret:ty;
            )*
        }
        $(asynchronous { $( $(#[$async_meta:meta])* fn $async_method:ident<$lifetime:lifetime>($($async_arg:ident: $async_ty:ty),*) -> $async_ret:ty; )* })?
        $(infallible {
            $( $(#[$infallible_meta:meta])* fn $infallible:ident() -> $value:ty; )*
        })?
    ) => {
        $(#[$port_meta])*
        pub trait $port {
            $($( $(#[$async_meta])* fn $async_method<$lifetime>(&$lifetime self $(, $async_arg: $async_ty)*) -> ExtensionFuture<$lifetime, $async_ret>; )*)?
            $($( $(#[$infallible_meta])* fn $infallible(&self) -> $value; )*)?
            $(
                $(#[$meta])*
                ///
                /// # Errors
                #[doc = $errors]
                fn $method(&self $(, $arg: $ty)*) -> ExtensionResult<$ret>;
            )*
        }

        impl $facade {
            $($( $(#[$infallible_meta])*
                #[must_use]
                pub fn $infallible(&self) -> $value { self.$access.$infallible() }
            )*)?
            $(
                $(#[$meta])*
                ///
                /// # Errors
                #[doc = $errors]
                pub fn $method(&self $(, $arg: $ty)*) -> ExtensionResult<$ret> {
                    self.$access.$method($($arg),*)
                }
            )*
        }
    };
    (
        $(#[$port_meta:meta])*
        $port:ident for $facade:ident via $access:tt { $($methods:tt)* }
        $($tail:tt)*
    ) => {
        port! {
            $(#[$port_meta])*
            $port for $facade via $access {
                errors "Returns the host's message when the host rejects the request.";
                $($methods)*
            }
            $($tail)*
        }
    };
}

/// Implements resource and function forwarding without duplicating a body per import.
#[cfg(target_arch = "wasm32")]
macro_rules! forward_resources {
    (functions { $($name:ident($($arg:ident: $ty:ty),* $(,)?) -> $ret:ty => $function:path;)* }) => {
        $(fn $name(&self $(, $arg: $ty)*) -> $ret {
            $function($($arg),*)
        })*
    };
    ($($name:ident($resource:ident: $resource_ty:ty $(, $arg:ident: $ty:ty)*) -> $ret:ty => $method:ident($($value:expr),* $(,)?);)*) => {
        $(fn $name(&self, $resource: $resource_ty $(, $arg: $ty)*) -> $ret {
            $resource.$method($($value),*)
        })*
    };
}
