use crate::{error::*, make_field, marshalling::*, metadata::*};
use std::borrow::Cow;

/// Mode (i.e. permissions) of an installed file
///
/// Serialized as a hexadecimal string.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Mode(u32);

make_field!(Mode, InstPkgKey);

impl Mode {
    pub fn from_u32(value: u32) -> Self {
        Self(value)
    }
}

impl FromFieldStr for Mode {
    fn from_field_str(value: FieldStr) -> Result<Self, AnonLocErr> {
        u32::from_str_radix(value.as_str(), 16)
            .map(Self)
            .map_err(|e| AnonLocErr::FieldInvalid(Self::NAME, e.to_string()))
    }
}

impl AsBytes for Mode {
    fn as_bytes(&self) -> Cow<'_, [u8]> {
        Cow::from(format!("{:x}", self.0).into_bytes())
    }
}

impl std::fmt::Display for Mode {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{:04o}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(s: &str) -> Result<Mode, AnonLocErr> {
        FieldStr::try_from(s)
            .map_err(|e| e.field(Mode::NAME))
            .and_then(Mode::from_field_str)
    }

    #[test]
    fn roundtrip() {
        for hex in &["0", "1ff", "1ed", "a4"] {
            let mode = parse(hex).unwrap();
            assert_eq!(std::str::from_utf8(&mode.as_bytes()).unwrap(), *hex);
        }
    }

    #[test]
    fn invalid_hex() {
        assert!(parse("xyz").is_err());
        assert!(parse("").is_err());
    }

    #[test]
    fn display_octal() {
        assert_eq!(Mode::from_u32(0o755).to_string(), "0755");
        assert_eq!(Mode::from_u32(0o644).to_string(), "0644");
        assert_eq!(Mode::from_u32(0o777).to_string(), "0777");
        assert_eq!(Mode::from_u32(0).to_string(), "0000");
    }
}
