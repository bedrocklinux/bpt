use crate::{error::*, make_display_color, make_field, marshalling::*, metadata::*};
use std::borrow::Cow;

/// License of a package
///
/// The expectation is for this to be SPDX formatted.  However, bpt does not enforce this;
/// ultimately it's just a free-form string.
#[derive(Clone, Debug)]

pub struct License(FieldStr);

make_field!(License, PkgKey);

impl std::fmt::Display for License {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

// No special coloring for license
// Implement anyways for developer experience consistency with other metadata fields
make_display_color!(License, |s, f| { write!(f, "{}", s.0) });

impl FromFieldStr for License {
    fn from_field_str(value: FieldStr) -> Result<Self, AnonLocErr> {
        if value.is_empty() {
            Err(AnonLocErr::FieldEmpty(Self::NAME))
        } else {
            Ok(Self(value))
        }
    }
}

impl AsBytes for License {
    fn as_bytes(&self) -> Cow<'_, [u8]> {
        Cow::from(self.0.as_bytes())
    }
}
