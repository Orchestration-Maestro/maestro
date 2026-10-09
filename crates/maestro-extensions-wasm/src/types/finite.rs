//! Checks authored numbers before JSON serialization can replace them with null.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use serde::Serialize;
use serde::ser::{self, Error as _};

/// Serialization traversal that retains no values or output bytes.
#[derive(Clone, Copy)]
pub(crate) struct Finite;

/// The result of traversing a serializable value.
type Checked = Result<(), serde_json::Error>;

/// Implement scalar operations that contain no floating-point value.
macro_rules! scalars {
    ($($method:ident($($name:ident: $ty:ty),*);)*) => {$(
        fn $method(self, $($name: $ty),*) -> Checked { Ok(()) }
    )*};
}

/// Implement compound starts without constructing a second representation.
macro_rules! compounds {
    ($($method:ident($($name:ident: $ty:ty),*);)*) => {$(
        fn $method(self, $($name: $ty),*) -> Result<Self, Self::Error> { Ok(self) }
    )*};
}

impl ser::Serializer for Finite {
    type Ok = ();
    type Error = serde_json::Error;
    type SerializeSeq = Self;
    type SerializeTuple = Self;
    type SerializeTupleStruct = Self;
    type SerializeTupleVariant = Self;
    type SerializeMap = Self;
    type SerializeStruct = Self;
    type SerializeStructVariant = Self;

    scalars! {
        serialize_bool(_v: bool);
        serialize_i8(_v: i8);
        serialize_i16(_v: i16);
        serialize_i32(_v: i32);
        serialize_i64(_v: i64);
        serialize_i128(_v: i128);
        serialize_u8(_v: u8);
        serialize_u16(_v: u16);
        serialize_u32(_v: u32);
        serialize_u64(_v: u64);
        serialize_u128(_v: u128);
        serialize_char(_v: char);
        serialize_str(_v: &str);
        serialize_bytes(_v: &[u8]);
        serialize_none();
        serialize_unit();
        serialize_unit_struct(_name: &'static str);
        serialize_unit_variant(_name: &'static str, _index: u32, _variant: &'static str);
    }

    fn serialize_f32(self, value: f32) -> Checked {
        self.serialize_f64(f64::from(value))
    }

    fn serialize_f64(self, value: f64) -> Checked {
        if value.is_finite() {
            Ok(())
        } else {
            Err(Self::Error::custom(
                "extension wrote a non-finite number (Infinity or NaN)",
            ))
        }
    }

    fn serialize_some<T: ?Sized + Serialize>(self, value: &T) -> Checked {
        value.serialize(self)
    }

    fn serialize_newtype_struct<T: ?Sized + Serialize>(
        self,
        _name: &'static str,
        value: &T,
    ) -> Checked {
        value.serialize(self)
    }

    fn serialize_newtype_variant<T: ?Sized + Serialize>(
        self,
        _name: &'static str,
        _index: u32,
        _variant: &'static str,
        value: &T,
    ) -> Checked {
        value.serialize(self)
    }

    compounds! {
        serialize_seq(_len: Option<usize>);
        serialize_tuple(_len: usize);
        serialize_tuple_struct(_name: &'static str, _len: usize);
        serialize_tuple_variant(_name: &'static str, _index: u32, _variant: &'static str, _len: usize);
        serialize_map(_len: Option<usize>);
        serialize_struct(_name: &'static str, _len: usize);
        serialize_struct_variant(_name: &'static str, _index: u32, _variant: &'static str, _len: usize);
    }
}

/// Visit each compound member through the same check.
macro_rules! members {
    ($trait:ident; $($method:ident($($name:ident: $ty:ty,)*);)*) => {
        impl ser::$trait for Finite {
            type Ok = ();
            type Error = serde_json::Error;
            $(fn $method<T: ?Sized + Serialize>(&mut self, $($name: $ty,)* value: &T) -> Checked {
                value.serialize(*self)
            })*
            fn end(self) -> Checked { Ok(()) }
        }
    };
}

members!(SerializeSeq; serialize_element(););
members!(SerializeTuple; serialize_element(););
members!(SerializeTupleStruct; serialize_field(););
members!(SerializeTupleVariant; serialize_field(););
members!(SerializeMap; serialize_key(); serialize_value(););
members!(SerializeStruct; serialize_field(_key: &'static str,););
members!(SerializeStructVariant; serialize_field(_key: &'static str,););
