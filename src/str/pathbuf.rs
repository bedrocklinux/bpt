use crate::str::*;
use camino::{Utf8Path, Utf8PathBuf};
use std::{
    io::{Error, ErrorKind},
    os::unix::prelude::OsStrExt,
    path::{Path, PathBuf},
    str::FromStr,
};

pub trait IntoPathBuf {
    fn into_pathbuf(self) -> Result<Utf8PathBuf, std::io::Error>;
}

impl IntoPathBuf for PathBuf {
    fn into_pathbuf(self) -> Result<Utf8PathBuf, std::io::Error> {
        Utf8PathBuf::from_path_buf(self).map_err(|p| {
            let s = String::from_utf8_lossy(p.as_os_str().as_bytes());
            Error::new(
                ErrorKind::InvalidData,
                format!("`{s}` is not a valid UTF-8 filepath"),
            )
        })
    }
}

impl IntoPathBuf for &Path {
    fn into_pathbuf(self) -> Result<Utf8PathBuf, std::io::Error> {
        match Utf8PathBuf::from_path_buf(self.to_owned()) {
            Ok(p) => Ok(p),
            Err(p) => {
                let s = String::from_utf8_lossy(p.as_os_str().as_bytes());
                Err(Error::new(
                    ErrorKind::InvalidData,
                    format!("`{s}` is not a valid UTF-8 filepath"),
                ))
            }
        }
    }
}

// Under-the-hood, Utf8PathBuf sets `type Err = Infallible` and thus the error branch should be
// impossible.  However, it's safer to implement and have the compiler remove that branch than set
// it as `unreachable!()` and then later find Utf8PathBuf changes its implementation and we've
// introduced a very bad bug.
impl IntoPathBuf for &str {
    fn into_pathbuf(self) -> Result<Utf8PathBuf, std::io::Error> {
        match Utf8PathBuf::from_str(self) {
            Ok(p) => Ok(p),
            Err(p) => Err(Error::new(
                ErrorKind::InvalidData,
                format!("`{p}` is not a valid UTF-8 filepath"),
            )),
        }
    }
}

impl IntoPathBuf for &[u8] {
    fn into_pathbuf(self) -> Result<Utf8PathBuf, std::io::Error> {
        self.into_string()?.into_pathbuf()
    }
}

/// Return an absolute path by prepending the current working directory if needed.
pub fn absolute_path_from_cwd(path: &Utf8Path) -> Result<Utf8PathBuf, std::io::Error> {
    if path.is_absolute() {
        return Ok(path.to_owned());
    }

    let cwd = std::env::current_dir()?.into_pathbuf()?;
    Ok(cwd.join(path))
}

pub trait Normalize {
    fn normalize(&self) -> Result<Utf8PathBuf, std::io::Error>;
    /// Like [normalize](Normalize::normalize), but rejects paths containing `..` or non-leading
    /// `.` components.  Legitimate package entries should never contain these; their presence is
    /// either malicious (path traversal) or obfuscation.
    fn strict_normalize(&self) -> Result<Utf8PathBuf, std::io::Error>;
}

impl Normalize for Path {
    // As of 0.4.40, the tar crate does not consistently normalize paths.  While Rust does provide
    // a `.canonicalize()`, this dereferences symlinks, which is not what we want.
    //
    // Implement path normalization ourselves.
    fn normalize(&self) -> Result<Utf8PathBuf, std::io::Error> {
        let mut path = Utf8PathBuf::new();
        for component in self.components() {
            match component {
                // "Does not occur on Unix"
                std::path::Component::Prefix(_) => unreachable!(),
                std::path::Component::RootDir => path.push("/"),
                std::path::Component::CurDir => {}
                std::path::Component::ParentDir => {
                    if !path.pop() {
                        return Err(Error::new(
                            ErrorKind::InvalidData,
                            "path attempted to `..` past its base",
                        ));
                    }
                }
                std::path::Component::Normal(entry) => {
                    let entry = entry.to_str().ok_or_else(|| {
                        Error::new(ErrorKind::InvalidData, "path component is not valid UTF-8")
                    })?;
                    if entry.is_empty() || entry == "." {
                        continue;
                    }
                    path.push(entry);
                }
            }
        }
        Ok(path)
    }

    fn strict_normalize(&self) -> Result<Utf8PathBuf, std::io::Error> {
        use std::path::Component;

        let mut seen_normal = false;
        for component in self.components() {
            match component {
                Component::ParentDir => {
                    return Err(Error::new(
                        ErrorKind::InvalidData,
                        "path contains `..` component",
                    ));
                }
                // Leading `.` is a common tar convention (e.g. `./usr/bin/foo`); allow it.
                // Non-leading `.` (e.g. `usr/./bin`) is obfuscation; reject it.
                //
                // Note: as of Rust 1.x, `std::path::Component` strips non-leading `.` during
                // iteration, so this branch currently cannot trigger.  It's kept defensively in
                // case the path source (e.g. tar crate) changes behavior in the future.
                Component::CurDir if seen_normal => {
                    return Err(Error::new(
                        ErrorKind::InvalidData,
                        "path contains non-leading `.` component",
                    ));
                }
                Component::Normal(_) => seen_normal = true,
                _ => {}
            }
        }

        self.normalize()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use camino::Utf8PathBuf;
    use std::ffi::OsStr;
    use std::io::ErrorKind;
    use std::path::Path;

    #[test]
    fn test_path_normalize_happy_path() {
        for (input, expected) in &[
            ("/a/b/c", "/a/b/c"),
            ("/a/./b/./c", "/a/b/c"),
            ("/a/b/../c", "/a/c"),
            ("/a/b/c/../../d", "/a/d"),
            ("/a/b/c/", "/a/b/c/"),
        ] {
            let result = Path::new(input).normalize().unwrap();
            assert_eq!(result, expected.into_pathbuf().unwrap());
        }
    }

    #[test]
    fn test_path_normalize_error() {
        for input in &[
            OsStr::new("../a/b"),
            OsStr::new("/../../a/b"),
            OsStr::from_bytes(b"/a/\xFF/c"),
        ] {
            let result = Path::new(input).normalize();
            assert!(result.is_err());
            assert_eq!(result.unwrap_err().kind(), ErrorKind::InvalidData);
        }
    }

    #[test]
    fn test_strict_normalize_happy_path() {
        for (input, expected) in &[
            ("usr/bin/foo", "usr/bin/foo"),
            ("./usr/bin/foo", "usr/bin/foo"),
            ("/a/b/c", "/a/b/c"),
            ("/a/b/c/", "/a/b/c/"),
        ] {
            let result = Path::new(input).strict_normalize().unwrap();
            assert_eq!(result, expected.into_pathbuf().unwrap(), "input: {input}");
        }
    }

    #[test]
    fn test_strict_normalize_rejects_dotdot() {
        for input in &["usr/bin/../../etc/shadow", "../etc/shadow", "./usr/../bin"] {
            let result = Path::new(input).strict_normalize();
            assert!(result.is_err(), "expected error for: {input}");
            assert!(
                result.unwrap_err().to_string().contains(".."),
                "error should mention `..` for: {input}"
            );
        }
    }

    #[test]
    fn test_absolute_path_from_cwd_already_absolute() {
        let path = Utf8Path::new("/tmp");
        let result = absolute_path_from_cwd(path).unwrap();
        assert_eq!(result, Utf8PathBuf::from("/tmp"));
    }

    #[test]
    fn test_absolute_path_from_cwd_relative() {
        let cwd = std::env::current_dir().unwrap().into_pathbuf().unwrap();
        let path = Utf8Path::new("test-root");
        let result = absolute_path_from_cwd(path).unwrap();
        assert_eq!(result, cwd.join(path));
    }

    // Cannot test: as of Rust 1.x, `std::path::Component` strips non-leading `.` during
    // iteration, so it never surfaces as `CurDir` and the defensive check cannot trigger.
    // The check is kept in `strict_normalize` in case future path sources bypass `components()`.
    //
    // #[test]
    // fn test_strict_normalize_rejects_non_leading_dot() {
    //     for input in &["usr/./bin/foo", "usr/bin/./foo"] {
    //         let result = Path::new(input).strict_normalize();
    //         assert!(result.is_err(), "expected error for: {input}");
    //         assert!(
    //             result.unwrap_err().to_string().contains("."),
    //             "error should mention `.` for: {input}"
    //         );
    //     }
    // }
}
