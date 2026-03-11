use crate::{color::*, error::*, make_display_color, make_field, marshalling::*, metadata::*};
use std::borrow::Cow;

/// Upstream web address of a package's project, e.g. a homepage
#[derive(Clone, Debug)]
pub struct Homepage(FieldStr);

make_field!(Homepage, PkgKey);

impl std::fmt::Display for Homepage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

make_display_color!(Homepage, |s, f| {
    write!(f, "{}{}{}", Color::Url, s.0, Color::Default)
});

impl FromFieldStr for Homepage {
    fn from_field_str(value: FieldStr) -> Result<Self, AnonLocErr> {
        Ok(Self(value))
    }
}

impl AsBytes for Homepage {
    fn as_bytes(&self) -> Cow<'_, [u8]> {
        Cow::from(self.0.as_bytes())
    }
}
