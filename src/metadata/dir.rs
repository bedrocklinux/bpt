use crate::{error::*, io::reject_unsafe_path, make_field, marshalling::*, metadata::*};
use camino::{Utf8Path, Utf8PathBuf};
use std::borrow::Cow;

/// A top-level directory in an installed package (`*.instpkg`)
///
/// Paths are relative to the configured root directory (defaulting to `/`); absolute paths and
/// `..` components are rejected to prevent path traversal.
///
/// Most paths stored in an instpkg are relative to the preceding directory entry.  This is the
/// exception which (re)sets the relative path of following items.
#[derive(Clone, Debug)]
pub struct Dir(Utf8PathBuf);

make_field!(Dir, InstPkgKey);

impl Dir {
    /// Initial empty directory context before any directory entry is encountered.
    pub fn empty() -> Self {
        Self(Utf8PathBuf::new())
    }

    pub fn from_pathbuf(path: Utf8PathBuf) -> Self {
        Self(path)
    }

    pub fn into_pathbuf(self) -> Utf8PathBuf {
        self.0
    }

    pub fn push(&mut self, subdir: &Utf8Path) {
        self.0.push(subdir);
    }

    pub fn as_path(&self) -> &Utf8Path {
        &self.0
    }
}

impl FromFieldStr for Dir {
    fn from_field_str(value: FieldStr) -> Result<Self, AnonLocErr> {
        if value.is_empty() {
            Err(AnonLocErr::FieldEmpty(Self::NAME))
        } else {
            reject_unsafe_path(value.as_str(), Self::NAME)?;
            Ok(Self(value.into_pathbuf()))
        }
    }
}

impl AsBytes for Dir {
    fn as_bytes(&self) -> Cow<'_, [u8]> {
        Cow::from(self.0.as_str().as_bytes())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(s: &str) -> Result<Dir, AnonLocErr> {
        FieldStr::try_from(s)
            .map_err(|e| e.field(Dir::NAME))
            .and_then(Dir::from_field_str)
    }

    #[test]
    fn roundtrip() {
        for path in &["usr", "usr/bin", "etc/bpt"] {
            let dir = parse(path).unwrap();
            assert_eq!(std::str::from_utf8(&dir.as_bytes()).unwrap(), *path);
        }
    }

    #[test]
    fn empty_rejected() {
        assert!(parse("").is_err());
    }

    #[test]
    fn absolute_path_rejected() {
        for path in &["/usr", "/usr/bin", "/etc/bpt", "/"] {
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
        for path in &["..", "../etc", "usr/../etc", "usr/bin/../../etc"] {
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
        // Leading `./` is a common tar convention and should be normalized away by callers,
        // but Dir stores the raw value.  The important thing is that `.` is not dangerous.
        assert!(parse("./usr").is_ok());
        assert!(parse("usr/./bin").is_ok());
    }

    #[test]
    fn push_subdir() {
        let mut dir = parse("usr").unwrap();
        dir.push("bin".into());
        assert_eq!(dir.as_path(), Utf8Path::new("usr/bin"));
    }
}
