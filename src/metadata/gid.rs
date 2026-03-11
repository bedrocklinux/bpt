use crate::{error::*, make_field, marshalling::*, metadata::*};
use std::borrow::Cow;

/// Group ID (gid) of a file in an installed package
///
/// Serialized as a hexadecimal string.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Gid(u64);

make_field!(Gid, InstPkgKey);

impl Gid {
    pub fn from_u64(value: u64) -> Self {
        Self(value)
    }
}

impl FromFieldStr for Gid {
    fn from_field_str(value: FieldStr) -> Result<Self, AnonLocErr> {
        u64::from_str_radix(value.as_str(), 16)
            .map(Self)
            .map_err(|e| AnonLocErr::FieldInvalid(Self::NAME, e.to_string()))
    }
}

impl AsBytes for Gid {
    fn as_bytes(&self) -> Cow<'_, [u8]> {
        Cow::from(format!("{:x}", self.0).into_bytes())
    }
}

impl std::fmt::Display for Gid {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(s: &str) -> Result<Gid, AnonLocErr> {
        FieldStr::try_from(s)
            .map_err(|e| e.field(Gid::NAME))
            .and_then(Gid::from_field_str)
    }

    #[test]
    fn roundtrip() {
        for hex in &["0", "ff", "1a2b", "ffffffffffffffff"] {
            let gid = parse(hex).unwrap();
            assert_eq!(std::str::from_utf8(&gid.as_bytes()).unwrap(), *hex);
        }
    }

    #[test]
    fn invalid_hex() {
        assert!(parse("xyz").is_err());
        assert!(parse("").is_err());
    }

    #[test]
    fn display_decimal() {
        assert_eq!(Gid::from_u64(255).to_string(), "255");
        assert_eq!(Gid::from_u64(0).to_string(), "0");
        assert_eq!(Gid::from_u64(1000).to_string(), "1000");
    }
}
