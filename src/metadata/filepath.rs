use crate::{color::*, error::*, make_display_color, make_field, marshalling::*, metadata::*};
use camino::{Utf8Path, Utf8PathBuf};
use std::{borrow::Cow, os::unix::prelude::OsStrExt};

/// Filepath
///
/// This is an entry in the [Backup] field, see it for details.
#[derive(Clone, Debug)]
pub struct Filepath(Utf8PathBuf);

make_field!(Filepath, PkgKey);

impl Filepath {
    pub fn as_path(&self) -> &Utf8Path {
        &self.0
    }
}

impl std::fmt::Display for Filepath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

make_display_color!(Filepath, |s, f| {
    write!(f, "{}{}{}", Color::File, s.0, Color::Default)
});

impl FromFieldStr for Filepath {
    fn from_field_str(value: FieldStr) -> Result<Self, AnonLocErr> {
        if value.is_empty() {
            Err(AnonLocErr::FieldEmpty(Self::NAME))
        } else {
            Ok(Self(value.into_pathbuf()))
        }
    }
}

impl AsBytes for Filepath {
    fn as_bytes(&self) -> Cow<'_, [u8]> {
        Cow::from(self.0.as_os_str().as_bytes())
    }
}
