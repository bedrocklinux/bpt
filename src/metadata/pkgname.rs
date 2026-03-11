use crate::{
    color::*, constant::*, error::*, make_display_color, make_field, marshalling::*, metadata::*,
};
use std::borrow::Cow;

/// Name of a package
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PkgName(FieldStr);

make_field!(PkgName, PkgKey);

make_display_color!(PkgName, |s, f| {
    write!(f, "{}{}{}", Color::PkgName, s.0, Color::Default)
});

impl std::fmt::Display for PkgName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl PkgName {
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl FromFieldStr for PkgName {
    fn from_field_str(value: FieldStr) -> Result<Self, AnonLocErr> {
        if value.is_empty() {
            return Err(AnonLocErr::FieldEmpty(Self::NAME));
        }

        if let Some(c) = value.as_str().chars().find(|&c| !is_pkgname_char(c)) {
            return Err(AnonLocErr::FieldIllegalChar(Self::NAME, c.to_string()));
        }

        Ok(Self(value))
    }
}

impl AsBytes for PkgName {
    fn as_bytes(&self) -> Cow<'_, [u8]> {
        Cow::from(self.0.as_str().as_bytes())
    }
}

impl TryFrom<&str> for PkgName {
    type Error = AnonLocErr;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        FieldStr::try_from(value)
            .field(Self::NAME)
            .and_then(Self::from_field_str)
    }
}

impl std::str::FromStr for PkgName {
    type Err = Err;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::try_from(s).map_err(|_| Err::InputFieldInvalid(Self::NAME, s.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_pkgname() {
        let name = PkgName::try_from("abc012+-.").expect("failed to parse valid package name");
        assert_eq!(name.as_str(), "abc012+-.");
        assert_eq!(format!("{name}"), "abc012+-.");
        assert_eq!(name.as_bytes().as_ref(), b"abc012+-.");
    }

    #[test]
    fn test_empty_pkgname_rejected() {
        let err = PkgName::from_field_str(FieldStr::empty()).expect_err("empty pkgname parsed");
        assert!(
            matches!(err, AnonLocErr::FieldEmpty(PkgName::NAME)),
            "expected FieldEmpty, got {err:?}"
        );
    }

    #[test]
    fn test_invalid_pkgname_chars_rejected() {
        for bad in [
            "hello_world",
            "Hello",
            "with space",
            "with/slash",
            "with$sign",
        ] {
            let err = PkgName::try_from(bad).expect_err("invalid package name unexpectedly parsed");
            assert!(
                matches!(err, AnonLocErr::FieldIllegalChar(PkgName::NAME, _)),
                "expected FieldIllegalChar for `{bad}`, got {err:?}"
            );
        }
    }

    #[test]
    fn test_from_str_maps_to_cli_error() {
        let err = "Invalid-Name"
            .parse::<PkgName>()
            .expect_err("expected CLI parse failure");
        assert!(
            matches!(err, Err::InputFieldInvalid(PkgName::NAME, _)),
            "expected InputFieldInvalid, got {err:?}"
        );
    }
}
