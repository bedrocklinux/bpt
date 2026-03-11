use crate::{error::*, io::reject_unsafe_path, make_field, marshalling::*, metadata::*};
use camino::Utf8PathBuf;
use std::borrow::Cow;

/// A subdirectory in an installed package, relative to the preceding [Dir] or [Subdir] entry.
///
/// Paths are relative to the configured root directory (defaulting to `/`); absolute paths and
/// `..` components are rejected to prevent path traversal.
#[derive(Clone, Debug)]
pub struct Subdir(Utf8PathBuf);

make_field!(Subdir, InstPkgKey);

impl Subdir {
    pub fn from_pathbuf(path: Utf8PathBuf) -> Self {
        Self(path)
    }

    pub fn into_pathbuf(self) -> Utf8PathBuf {
        self.0
    }
}

impl FromFieldStr for Subdir {
    fn from_field_str(value: FieldStr) -> Result<Self, AnonLocErr> {
        if value.is_empty() {
            Err(AnonLocErr::FieldEmpty(Self::NAME))
        } else {
            reject_unsafe_path(value.as_str(), Self::NAME)?;
            Ok(Self(value.into_pathbuf()))
        }
    }
}

impl AsBytes for Subdir {
    fn as_bytes(&self) -> Cow<'_, [u8]> {
        Cow::from(self.0.as_str().as_bytes())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(s: &str) -> Result<Subdir, AnonLocErr> {
        FieldStr::try_from(s)
            .map_err(|e| e.field(Subdir::NAME))
            .and_then(Subdir::from_field_str)
    }

    #[test]
    fn roundtrip() {
        for path in &["bin", "lib/x86_64-linux-gnu", "share"] {
            let subdir = parse(path).unwrap();
            assert_eq!(std::str::from_utf8(&subdir.as_bytes()).unwrap(), *path);
        }
    }

    #[test]
    fn empty_rejected() {
        assert!(parse("").is_err());
    }

    #[test]
    fn absolute_path_rejected() {
        for path in &["/bin", "/usr/lib", "/"] {
            let err = parse(path).unwrap_err();
            let msg = format!("{err:?}");
            assert!(
                msg.contains("absolute"),
                "expected absolute path error for `{path}`, got: {msg}"
            );
        }
    }

    #[test]
    fn dotdot_rejected() {
        for path in &["..", "../lib", "lib/../../etc"] {
            let err = parse(path).unwrap_err();
            let msg = format!("{err:?}");
            assert!(
                msg.contains(".."),
                "expected `..` error for `{path}`, got: {msg}"
            );
        }
    }

    #[test]
    fn dot_component_accepted() {
        assert!(parse("./bin").is_ok());
        assert!(parse("lib/./plugins").is_ok());
    }
}
