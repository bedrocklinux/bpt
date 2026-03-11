use super::FileAux;
use crate::error::*;
use camino::{Utf8Path, Utf8PathBuf};
use std::fs::File;
use std::os::unix::prelude::PermissionsExt;

/// Validate that a path is relative and contains no `..` components.
///
/// All instpkg paths are relative to a configurable root directory (defaulting to `/`).  Absolute
/// paths and `..` components are rejected to prevent path traversal attacks from crafted instpkg
/// files.
pub fn reject_unsafe_path(path: &str, field_name: &'static str) -> Result<(), AnonLocErr> {
    if path.starts_with('/') {
        return Err(AnonLocErr::FieldInvalid(
            field_name,
            format!("absolute paths are not allowed: `{path}`"),
        ));
    }
    for component in std::path::Path::new(path).components() {
        if matches!(component, std::path::Component::ParentDir) {
            return Err(AnonLocErr::FieldInvalid(
                field_name,
                format!("`..` components are not allowed: `{path}`"),
            ));
        }
    }
    Ok(())
}

#[allow(dead_code)]
pub fn is_executable_in_path(exe: &str) -> bool {
    is_executable_in_paths(exe, &path_env())
}

pub fn path_env() -> Vec<Utf8PathBuf> {
    std::env::var("PATH")
        .unwrap_or_default()
        .split(':')
        .map(Utf8PathBuf::from)
        .collect()
}

pub fn is_executable_in_paths(exe: &str, paths: &[Utf8PathBuf]) -> bool {
    for path in paths {
        let mut path = path.clone();
        path.push(exe);

        let metadata = match std::fs::metadata(&path) {
            Ok(md) => md,
            Err(_) => continue,
        };

        if metadata.is_file() && metadata.permissions().mode() & 0o111 != 0 {
            return true;
        }
    }

    false
}

pub trait PathAux {
    /// Read the entirety of a file we expect to be small, as a String.
    ///
    /// If the file is larger than SMALL_FILE_MAX_SIZE, error.
    fn read_small_file_string(&self) -> Result<String, Err>;

    /// Read the entirety of a file we expect to be small, as raw bytes.
    ///
    /// If the file is larger than SMALL_FILE_MAX_SIZE, error.
    #[cfg(test)]
    fn read_small_file_bytes(&self) -> Result<Vec<u8>, Err>;
}

impl PathAux for Utf8Path {
    fn read_small_file_string(&self) -> Result<String, Err> {
        File::open_ro(self)?.read_small_file_string().loc(self)
    }

    #[cfg(test)]
    fn read_small_file_bytes(&self) -> Result<Vec<u8>, Err> {
        File::open_ro(self)?.read_small_file_bytes().loc(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constant::SMALL_FILE_MAX_SIZE;
    use crate::testutil::unit_test_tmp_dir;
    use camino::Utf8PathBuf;

    fn test_dir(name: &str) -> Utf8PathBuf {
        unit_test_tmp_dir("path_aux", name)
    }

    #[test]
    fn read_small_file_string_reads_contents() {
        let dir = test_dir("read_small_file_string_reads_contents");
        let path = dir.join("small.txt");
        std::fs::write(&path, "hello from path aux").unwrap();

        let out = path.as_path().read_small_file_string().unwrap();
        assert_eq!(out, "hello from path aux");
    }

    #[test]
    fn read_small_file_bytes_reads_contents() {
        let dir = test_dir("read_small_file_bytes_reads_contents");
        let path = dir.join("small.bin");
        let input = vec![0_u8, 1, 2, 3, 255];
        std::fs::write(&path, &input).unwrap();

        let out = path.as_path().read_small_file_bytes().unwrap();
        assert_eq!(out, input);
    }

    #[test]
    fn read_small_file_string_rejects_oversized_file() {
        let dir = test_dir("read_small_file_string_rejects_oversized_file");
        let path = dir.join("too-large.txt");
        std::fs::write(&path, vec![b'a'; SMALL_FILE_MAX_SIZE + 1]).unwrap();

        let err = path.as_path().read_small_file_string().unwrap_err();
        assert!(matches!(err, Err::FileTooLarge(_, SMALL_FILE_MAX_SIZE)));
    }

    #[test]
    fn read_small_file_bytes_missing_path_returns_open_error() {
        let dir = test_dir("read_small_file_bytes_missing_path_returns_open_error");
        let path = dir.join("missing.bin");

        let err = path.as_path().read_small_file_bytes().unwrap_err();
        assert!(matches!(err, Err::Open(_, _)));
    }

    #[test]
    fn reject_unsafe_path_accepts_relative() {
        for path in &["usr", "usr/bin", "etc/bpt", "a", "./usr", "usr/./bin"] {
            assert!(
                reject_unsafe_path(path, "test").is_ok(),
                "should accept `{path}`"
            );
        }
    }

    #[test]
    fn reject_unsafe_path_rejects_absolute() {
        for path in &["/", "/usr", "/usr/bin", "/etc/bpt/keys"] {
            let err = reject_unsafe_path(path, "test").unwrap_err();
            let msg = format!("{err:?}");
            assert!(
                msg.contains("absolute"),
                "expected absolute error for `{path}`, got: {msg}"
            );
        }
    }

    #[test]
    fn reject_unsafe_path_rejects_dotdot() {
        for path in &["..", "../etc", "usr/../etc", "a/b/../../c", "usr/bin/../.."] {
            let err = reject_unsafe_path(path, "test").unwrap_err();
            let msg = format!("{err:?}");
            assert!(
                msg.contains(".."),
                "expected `..` error for `{path}`, got: {msg}"
            );
        }
    }

    #[test]
    fn reject_unsafe_path_absolute_takes_precedence_over_dotdot() {
        // When both absolute and `..` are present, absolute is caught first
        let err = reject_unsafe_path("/../../etc", "test").unwrap_err();
        let msg = format!("{err:?}");
        assert!(
            msg.contains("absolute"),
            "expected absolute error, got: {msg}"
        );
    }
}
