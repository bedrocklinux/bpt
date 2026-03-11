use crate::{error::*, io::reject_unsafe_path, make_field, marshalling::*, metadata::*};
use camino::{Utf8Path, Utf8PathBuf};
use std::borrow::Cow;

/// The filename of some non-directory file type in an installed package.
///
/// Paths are relative to the configured root directory (defaulting to `/`); absolute paths and
/// `..` components are rejected to prevent path traversal.
///
/// This is relative to the last [Dir]/[Subdir] field in the [crate::file::InstPkg].  Since that is
/// usually the containing directory, we can use this to lessen the size of the serialized data.
///
/// Various file types in [crate::file::InstPkg]s require both a filename/path and some additional
/// data such as the file's checksum or symlink target.  Rather than try to combine these into a
/// single field, this [Filename] type is serialized just before the file type that uses it.
///
/// Since directories are functionally just paths and require no additional data, their serialized
/// fields do not utilize this [Filename] type, but instead directly contain the target path.
#[derive(Clone, Debug)]
pub struct Filename(Utf8PathBuf);

make_field!(Filename, InstPkgKey);

impl Filename {
    pub fn from_path(path: &Utf8Path) -> Self {
        Self(path.to_path_buf())
    }

    pub fn into_pathbuf(self) -> Utf8PathBuf {
        self.0
    }
}

impl FromFieldStr for Filename {
    fn from_field_str(value: FieldStr) -> Result<Self, AnonLocErr> {
        if value.is_empty() {
            Err(AnonLocErr::FieldEmpty(Self::NAME))
        } else {
            reject_unsafe_path(value.as_str(), Self::NAME)?;
            Ok(Self(value.into_pathbuf()))
        }
    }
}

impl AsBytes for Filename {
    fn as_bytes(&self) -> Cow<'_, [u8]> {
        Cow::from(self.0.as_str().as_bytes())
    }
}

impl std::fmt::Display for Filename {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(s: &str) -> Result<Filename, AnonLocErr> {
        FieldStr::try_from(s)
            .map_err(|e| e.field(Filename::NAME))
            .and_then(Filename::from_field_str)
    }

    #[test]
    fn roundtrip() {
        for name in &["libfoo.so", "config.toml", "a.out"] {
            let filename = parse(name).unwrap();
            assert_eq!(std::str::from_utf8(&filename.as_bytes()).unwrap(), *name);
        }
    }

    #[test]
    fn empty_rejected() {
        assert!(parse("").is_err());
    }

    #[test]
    fn absolute_path_rejected() {
        for path in &["/libfoo.so", "/etc/passwd"] {
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
        for path in &["..", "../passwd", "sub/../other"] {
            let err = parse(path).unwrap_err();
            let msg = format!("{err:?}");
            assert!(
                msg.contains(".."),
                "expected `..` error for `{path}`, got: {msg}"
            );
        }
    }

    #[test]
    fn display() {
        let filename = parse("hello.txt").unwrap();
        assert_eq!(filename.to_string(), "hello.txt");
    }
}
