use crate::{error::*, make_field, marshalling::*, metadata::*};
use camino::Utf8PathBuf;
use std::borrow::Cow;

/// A symlink target in an installed package.
///
/// In addition to indicating that this is a symlink this field contains the link's target.  The
/// symlink's filename is serialized/deserialized in a preceding [Filename] field rather than in this
/// field directly.
#[derive(Clone, Debug, PartialEq)]
pub struct Symlink(Utf8PathBuf);

make_field!(Symlink, InstPkgKey);

impl Symlink {
    pub fn from_pathbuf(path: Utf8PathBuf) -> Self {
        Self(path)
    }
}

impl FromFieldStr for Symlink {
    fn from_field_str(value: FieldStr) -> Result<Self, AnonLocErr> {
        // Linux disallows empty symlinks
        // https://lwn.net/Articles/551224/
        if value.is_empty() {
            Err(AnonLocErr::FieldEmpty(Self::NAME))
        } else {
            Ok(Self(value.into_pathbuf()))
        }
    }
}

impl AsBytes for Symlink {
    fn as_bytes(&self) -> Cow<'_, [u8]> {
        Cow::from(self.0.as_str().as_bytes())
    }
}

impl std::fmt::Display for Symlink {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(s: &str) -> Result<Symlink, AnonLocErr> {
        FieldStr::try_from(s)
            .map_err(|e| e.field(Symlink::NAME))
            .and_then(Symlink::from_field_str)
    }

    #[test]
    fn roundtrip() {
        for target in &[
            "/usr/lib/libfoo.so.1",
            "../lib/libfoo.so.1",
            "libfoo.so.1.2.3",
        ] {
            let symlink = parse(target).unwrap();
            assert_eq!(std::str::from_utf8(&symlink.as_bytes()).unwrap(), *target);
        }
    }

    #[test]
    fn empty_rejected() {
        assert!(parse("").is_err());
    }

    #[test]
    fn display() {
        let symlink = parse("../lib/libfoo.so").unwrap();
        assert_eq!(symlink.to_string(), "../lib/libfoo.so");
    }
}
