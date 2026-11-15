//! Checks authored numbers while the text is written, before JSON serialization can replace
//! them with null.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use serde::Serialize;
use serde::ser::{self, Error as _};

/// A serializer that rejects a nonfinite number and otherwise hands every value on unchanged.
struct Check<S>(S);

/// A value serialized through [`Check`], so nested numbers are checked too.
struct Item<'a, T: ?Sized>(&'a T);

impl<T: ?Sized + Serialize> Serialize for Item<'_, T> {
    fn serialize<S: ser::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(Check(serializer))
    }
}

/// Forward scalar operations that contain no floating-point value.
macro_rules! scalars {
    ($($method:ident($($name:ident: $ty:ty),*);)*) => {$(
        fn $method(self, $($name: $ty),*) -> Result<S::Ok, S::Error> { self.0.$method($($name),*) }
    )*};
}

/// Forward compound starts, wrapping the compound so its members are checked.
macro_rules! compounds {
    ($($method:ident -> $kind:ident($($name:ident: $ty:ty),*);)*) => {$(
        fn $method(self, $($name: $ty),*) -> Result<Self::$kind, S::Error> {
            self.0.$method($($name),*).map(Check)
        }
    )*};
}

impl<S: ser::Serializer> ser::Serializer for Check<S> {
    type Ok = S::Ok;
    type Error = S::Error;
    type SerializeSeq = Check<S::SerializeSeq>;
    type SerializeTuple = Check<S::SerializeTuple>;
    type SerializeTupleStruct = Check<S::SerializeTupleStruct>;
    type SerializeTupleVariant = Check<S::SerializeTupleVariant>;
    type SerializeMap = Check<S::SerializeMap>;
    type SerializeStruct = Check<S::SerializeStruct>;
    type SerializeStructVariant = Check<S::SerializeStructVariant>;

    scalars! {
        serialize_bool(v: bool);
        serialize_i8(v: i8);
        serialize_i16(v: i16);
        serialize_i32(v: i32);
        serialize_i64(v: i64);
        serialize_i128(v: i128);
        serialize_u8(v: u8);
        serialize_u16(v: u16);
        serialize_u32(v: u32);
        serialize_u64(v: u64);
        serialize_u128(v: u128);
        serialize_char(v: char);
        serialize_str(v: &str);
        serialize_bytes(v: &[u8]);
        serialize_none();
        serialize_unit();
        serialize_unit_struct(name: &'static str);
        serialize_unit_variant(name: &'static str, index: u32, variant: &'static str);
    }

    fn serialize_f32(self, value: f32) -> Result<S::Ok, S::Error> {
        self.serialize_f64(f64::from(value))
    }

    fn serialize_f64(self, value: f64) -> Result<S::Ok, S::Error> {
        if value.is_finite() {
            self.0.serialize_f64(value)
        } else {
            Err(S::Error::custom(
                "extension wrote a non-finite number (Infinity or NaN)",
            ))
        }
    }

    fn serialize_some<T: ?Sized + Serialize>(self, value: &T) -> Result<S::Ok, S::Error> {
        self.0.serialize_some(&Item(value))
    }

    fn serialize_newtype_struct<T: ?Sized + Serialize>(
        self,
        name: &'static str,
        value: &T,
    ) -> Result<S::Ok, S::Error> {
        self.0.serialize_newtype_struct(name, &Item(value))
    }

    fn serialize_newtype_variant<T: ?Sized + Serialize>(
        self,
        name: &'static str,
        index: u32,
        variant: &'static str,
        value: &T,
    ) -> Result<S::Ok, S::Error> {
        self.0
            .serialize_newtype_variant(name, index, variant, &Item(value))
    }

    fn collect_str<T: ?Sized + std::fmt::Display>(self, value: &T) -> Result<S::Ok, S::Error> {
        self.0.collect_str(value)
    }

    compounds! {
        serialize_seq -> SerializeSeq(len: Option<usize>);
        serialize_tuple -> SerializeTuple(len: usize);
        serialize_tuple_struct -> SerializeTupleStruct(name: &'static str, len: usize);
        serialize_tuple_variant -> SerializeTupleVariant(
            name: &'static str, index: u32, variant: &'static str, len: usize
        );
        serialize_map -> SerializeMap(len: Option<usize>);
        serialize_struct -> SerializeStruct(name: &'static str, len: usize);
        serialize_struct_variant -> SerializeStructVariant(
            name: &'static str, index: u32, variant: &'static str, len: usize
        );
    }
}

/// Visit each compound member through the same check.
macro_rules! members {
    ($trait:ident; $($method:ident($($name:ident: $ty:ty,)*);)*) => {
        impl<C: ser::$trait> ser::$trait for Check<C> {
            type Ok = C::Ok;
            type Error = C::Error;
            $(fn $method<T: ?Sized + Serialize>(&mut self, $($name: $ty,)* value: &T)
                -> Result<(), C::Error> {
                self.0.$method($($name,)* &Item(value))
            })*
            fn end(self) -> Result<C::Ok, C::Error> { self.0.end() }
        }
    };
}

members!(SerializeSeq; serialize_element(););
members!(SerializeTuple; serialize_element(););
members!(SerializeTupleStruct; serialize_field(););
members!(SerializeTupleVariant; serialize_field(););
members!(SerializeMap; serialize_key(); serialize_value(););
members!(SerializeStruct; serialize_field(key: &'static str,););
members!(SerializeStructVariant; serialize_field(key: &'static str,););

/// Encodes authored data in one pass, rejecting nonfinite typed numbers.
pub(crate) fn encode_value<T: Serialize>(value: &T) -> Result<String, String> {
    let mut text = Vec::new();
    value
        .serialize(Check(&mut serde_json::Serializer::new(&mut text)))
        .map_err(|error| error.to_string())?;
    String::from_utf8(text).map_err(|error| error.to_string())
}
