use crate::{error::*, make_display_color, make_field, marshalling::*, metadata::*};
use std::borrow::Cow;

/// A terse, one-liner description of a package.
#[derive(Clone, Debug, PartialEq)]
pub struct PkgDesc(FieldStr);

make_field!(PkgDesc, PkgKey);

make_display_color!(PkgDesc, |s, f| write!(f, "{}", s.0));

impl std::fmt::Display for PkgDesc {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromFieldStr for PkgDesc {
    fn from_field_str(value: FieldStr) -> Result<Self, AnonLocErr> {
        if value.is_empty() {
            Err(AnonLocErr::FieldEmpty(Self::NAME))
        } else if value.as_str().contains('\n') {
            Err(AnonLocErr::FieldIllegalChar(
                Self::NAME,
                "newline".to_owned(),
            ))
        } else {
            Ok(Self(value))
        }
    }
}

impl AsBytes for PkgDesc {
    fn as_bytes(&self) -> Cow<'_, [u8]> {
        Cow::from(self.0.as_bytes())
    }
}

impl PkgDesc {
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}
