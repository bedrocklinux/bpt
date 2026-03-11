use crate::{error::*, marshalling::*};
use std::borrow::Cow;

/// A single-null-separated section of a file.
pub trait Field: FromFieldStr + AsBytes {
    /// Name of field.  Used in error messages.
    const NAME: &'static str;
    /// Fields are serialized as:
    ///
    ///     [key, one byte][actual data][terminating null, one byte]
    ///
    /// This is the leading key byte.
    const KEY: u8;
}

/// The field type name, NAME constant, and KEY byte enum variant are all the same symbol.
/// Use a macro to avoid the possibility of an error when repeating them.
#[macro_export]
macro_rules! make_field {
    ($type:ident, $key_enum:ty) => {
        impl $crate::marshalling::Field for $type {
            const NAME: &'static str = stringify!($type);
            const KEY: u8 = <$key_enum>::$type as u8;
        }
    };
}

/// Create the field from the serialized content body, without preceding key byte or trailing null.
///
/// The Deserialize trait wraps this to handle the key byte and trailing null.
pub trait FromFieldStr: Sized {
    fn from_field_str(value: FieldStr) -> Result<Self, AnonLocErr>;
}

/// Serialized version of content body, without preceding key byte or trailing null byte.
///
/// The Serialize trait wraps this to provide the full serialized field.
pub trait AsBytes {
    fn as_bytes(&self) -> Cow<'_, [u8]>;
}

/// Convenience function for writing a field which includes preceding key and trailing null
///
/// Empty fields are intentionally omitted.  The consumer of deserialization should treat a missing
/// expected field as an empty field.
pub trait Serialize<W: std::io::Write> {
    fn serialize(&self, w: &mut W) -> Result<(), AnonLocErr>;
}

impl<F: Field, W: std::io::Write> Serialize<W> for F {
    fn serialize(&self, w: &mut W) -> Result<(), AnonLocErr> {
        let bytes = self.as_bytes();

        // Empty fields can be omitted
        if bytes.is_empty() {
            return Ok(());
        }

        || -> std::io::Result<()> {
            w.write_all(&[F::KEY])?;
            w.write_all(bytes.as_ref())?;
            w.write_all(b"\0")
        }()
        .map_err(AnonLocErr::Write)
    }
}

/// Convenience function for reading a field which validates the key and FieldStr constraints.
///
/// Does not handle the trailing null byte.  Expected to be called on a slice split on null.
///
/// Empty fields are intentionally omitted.  The consumer of deserialization should treat a missing
/// expected field as an empty field.
pub trait Deserialize {
    fn deserialize(bytes: &[u8]) -> Result<Self, AnonLocErr>
    where
        Self: std::marker::Sized;
}

impl<F: Field> Deserialize for F {
    fn deserialize(bytes: &[u8]) -> Result<Self, AnonLocErr>
    where
        Self: std::marker::Sized,
    {
        // Validate key byte
        if bytes.is_empty() {
            return Err(AnonLocErr::FieldMissing(Self::NAME));
        }
        let (key, value) = bytes.split_at(1);
        if key != [Self::KEY] {
            return Err(AnonLocErr::FieldInvalid(
                Self::NAME,
                format!(
                    "Expected key '{}' but found key '{}'",
                    Self::KEY as char,
                    key.first().map(|&c| c as char).unwrap_or('?'),
                ),
            ));
        }

        // Deserialize
        FieldStr::try_from(value)
            .map_err(|e| e.field(Self::NAME))
            .and_then(Self::from_field_str)
    }
}
