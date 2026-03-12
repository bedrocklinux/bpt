use crate::{color::Color, error::*, metadata::*};
use camino::{Utf8Path, Utf8PathBuf};
use std::{io::ErrorKind, os::unix::fs::MetadataExt};

/// A file in an installed package (`*.instpkg`).
///
/// Serializing/Deserializing depends on other [InstFile]s in the same package, and thus the
/// associated code is in [crate::file::InstPkg] rather than within [InstFile].
#[derive(Clone, Debug)]
pub struct InstFile {
    pub mode: Mode,
    pub uid: Uid,
    pub gid: Gid,
    pub path: Utf8PathBuf,
    pub entry_type: InstFileType,
}

#[derive(Clone, Debug)]
pub enum InstFileType {
    Directory,
    RegFile(RegFile),
    Symlink(Symlink),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InstFileCheckIssue {
    pub message: String,
    pub is_content_difference: bool,
}

impl InstFileType {
    pub fn is_dir(&self) -> bool {
        matches!(self, Self::Directory)
    }
}

impl InstFile {
    fn color_path(path: &Utf8Path) -> String {
        format!("{}{}{}", Color::File, path, Color::Default)
    }

    fn color_warn(label: &str) -> String {
        format!("{}{}{}", Color::Warn, label, Color::Default)
    }

    fn color_meta(label: &str) -> String {
        format!("{}{}{}", Color::Deemphasize, label, Color::Default)
    }

    fn can_keep_directory_remove_error(error: &std::io::Error) -> bool {
        matches!(
            error.kind(),
            ErrorKind::NotFound | ErrorKind::DirectoryNotEmpty | ErrorKind::ResourceBusy
        )
    }

    /// Check if the on-disk file content differs from the stored checksum/target.
    ///
    /// Returns `false` for directories, missing files, or files matching their stored content.
    pub fn is_content_modified(&self, root: &Utf8Path) -> Result<bool, Err> {
        let path = root.join(&self.path);

        match &self.entry_type {
            InstFileType::Directory => Ok(false),
            InstFileType::RegFile(expect) => {
                let mut file = match std::fs::File::open(&path) {
                    Ok(f) => f,
                    Err(e) if e.kind() == ErrorKind::NotFound => return Ok(false),
                    Err(e) => return Err(Err::Open(path.to_string(), e)),
                };
                let actual = RegFile::from_file(&mut file).loc(path)?;
                Ok(*expect != actual)
            }
            InstFileType::Symlink(expect) => {
                let actual = match path.read_link_utf8() {
                    Ok(target) => target,
                    Err(e) if e.kind() == ErrorKind::NotFound => return Ok(false),
                    Err(e) => return Err(Err::Read(path.to_string(), e)),
                };
                let actual = Symlink::from_pathbuf(actual);
                Ok(*expect != actual)
            }
        }
    }

    pub fn remove(&self, root: &Utf8Path) -> Result<(), Err> {
        let path = root.join(&self.path);
        match self.entry_type {
            InstFileType::Directory => {
                // Multiple packages can share a directory. Keep directories the kernel reports as
                // still in use, including Bedrock mount points such as `/etc`.
                match std::fs::remove_dir(&path) {
                    Ok(_) => Ok(()),
                    Result::Err(e) if Self::can_keep_directory_remove_error(&e) => Ok(()),
                    Result::Err(e) => Err(Err::Remove(path.to_string(), e)),
                }
            }
            _ => match std::fs::remove_file(&path) {
                Ok(_) => Ok(()),
                Result::Err(e) if e.kind() == ErrorKind::NotFound => Ok(()),
                Result::Err(e) => Err(Err::Remove(path.to_string(), e)),
            },
        }
    }

    pub fn check(&self, root: &Utf8Path) -> Result<Vec<InstFileCheckIssue>, Err> {
        let mut issues = Vec::new();
        let path = root.join(&self.path);

        let metadata = match path.symlink_metadata() {
            Ok(metadata) => metadata,
            Err(e) if e.kind() == ErrorKind::NotFound => {
                return Ok(vec![InstFileCheckIssue {
                    message: format!(
                        "{}: {}",
                        Self::color_warn("Missing"),
                        Self::color_path(&path)
                    ),
                    is_content_difference: false,
                }]);
            }
            Err(e) => return Err(Err::Stat(path, e)),
        };

        // Skip checking permissions on symlinks.  `man 7 symlink`:
        // > On Linux, the permissions of an ordinary symbolic link are not
        // > used in any operations; the permissions are always 0777 (read,
        // > write, and execute for all user categories), and can't be
        // > changed.
        if !matches!(self.entry_type, InstFileType::Symlink(_)) {
            // `& 0o7777` strips off the file type bits
            let found_mode = metadata.mode() & 0o7777;
            let found_mode = Mode::from_u32(found_mode);
            if found_mode != self.mode {
                issues.push(InstFileCheckIssue {
                    message: format!(
                        "{} mode: {} ({} {}; {} {})",
                        Self::color_warn("Incorrect"),
                        Self::color_path(&path),
                        Self::color_meta("expected"),
                        self.mode,
                        Self::color_meta("found"),
                        found_mode
                    ),
                    is_content_difference: false,
                });
            }
        }

        let found_uid = Uid::from_u64(metadata.uid() as u64);
        if found_uid != self.uid {
            issues.push(InstFileCheckIssue {
                message: format!(
                    "{} uid: {} ({} {}; {} {})",
                    Self::color_warn("Incorrect"),
                    Self::color_path(&path),
                    Self::color_meta("expected"),
                    self.uid,
                    Self::color_meta("found"),
                    found_uid
                ),
                is_content_difference: false,
            });
        }

        let found_gid = Gid::from_u64(metadata.gid() as u64);
        if found_gid != self.gid {
            issues.push(InstFileCheckIssue {
                message: format!(
                    "{} gid: {} ({} {}; {} {})",
                    Self::color_warn("Incorrect"),
                    Self::color_path(&path),
                    Self::color_meta("expected"),
                    self.gid,
                    Self::color_meta("found"),
                    found_gid
                ),
                is_content_difference: false,
            });
        }

        match &self.entry_type {
            InstFileType::Directory if !metadata.is_dir() => {
                issues.push(InstFileCheckIssue {
                    message: format!(
                        "{} a directory: {}",
                        Self::color_warn("Not"),
                        Self::color_path(&path)
                    ),
                    is_content_difference: false,
                });
            }
            InstFileType::Directory => {}
            InstFileType::RegFile(_expect) if !metadata.is_file() => {
                issues.push(InstFileCheckIssue {
                    message: format!(
                        "{} a regular file: {}",
                        Self::color_warn("Not"),
                        Self::color_path(&path)
                    ),
                    is_content_difference: false,
                });
            }
            InstFileType::RegFile(expect) => {
                let mut file =
                    std::fs::File::open(&path).map_err(|e| Err::Open(path.to_string(), e))?;
                let actual = RegFile::from_file(&mut file).loc(&path)?;
                if *expect != actual {
                    issues.push(InstFileCheckIssue {
                        message: format!(
                            "{} sha256: {} ({} {}; {} {})",
                            Self::color_warn("Incorrect"),
                            Self::color_path(&path),
                            Self::color_meta("expected"),
                            expect,
                            Self::color_meta("found"),
                            actual
                        ),
                        is_content_difference: true,
                    });
                }
            }
            InstFileType::Symlink(_expect) if !metadata.is_symlink() => {
                issues.push(InstFileCheckIssue {
                    message: format!(
                        "{} a symlink: {}",
                        Self::color_warn("Not"),
                        Self::color_path(&path)
                    ),
                    is_content_difference: false,
                });
            }
            InstFileType::Symlink(expect) => {
                let actual = path
                    .read_link_utf8()
                    .map_err(|e| Err::Read(path.to_string(), e))?;
                let actual = Symlink::from_pathbuf(actual);
                if *expect != actual {
                    issues.push(InstFileCheckIssue {
                        message: format!(
                            "{} symlink: {} ({} {}; {} {})",
                            Self::color_warn("Incorrect"),
                            Self::color_path(&path),
                            Self::color_meta("expected"),
                            expect,
                            Self::color_meta("found"),
                            actual
                        ),
                        is_content_difference: true,
                    });
                }
            }
        }

        Ok(issues)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::unit_test_tmp_dir;
    use sha2::{Digest, Sha256};

    fn test_root(name: &str) -> Utf8PathBuf {
        unit_test_tmp_dir("instfile", name)
    }

    fn make_instfile(path: &str, entry_type: InstFileType) -> InstFile {
        InstFile {
            mode: Mode::from_u32(0o644),
            uid: Uid::from_u64(0),
            gid: Gid::from_u64(0),
            path: Utf8PathBuf::from(path),
            entry_type,
        }
    }

    fn sha256_of(data: &[u8]) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(data);
        let mut out = [0u8; 32];
        out.copy_from_slice(&hasher.finalize());
        out
    }

    #[test]
    fn directory_not_modified() {
        let root = test_root("directory_not_modified");
        std::fs::create_dir_all(root.join("etc")).unwrap();

        let entry = make_instfile("etc", InstFileType::Directory);
        assert!(!entry.is_content_modified(root.as_path()).unwrap());
    }

    #[test]
    fn regfile_matching_content() {
        let root = test_root("regfile_matching_content");
        std::fs::create_dir_all(root.join("etc")).unwrap();

        let content = b"original content";
        std::fs::write(root.join("etc/config"), content).unwrap();

        let entry = make_instfile(
            "etc/config",
            InstFileType::RegFile(RegFile::from_sha256(sha256_of(content))),
        );
        assert!(!entry.is_content_modified(root.as_path()).unwrap());
    }

    #[test]
    fn regfile_modified_content() {
        let root = test_root("regfile_modified_content");
        std::fs::create_dir_all(root.join("etc")).unwrap();

        std::fs::write(root.join("etc/config"), b"modified content").unwrap();

        let entry = make_instfile(
            "etc/config",
            InstFileType::RegFile(RegFile::from_sha256(sha256_of(b"original content"))),
        );
        assert!(entry.is_content_modified(root.as_path()).unwrap());
    }

    #[test]
    fn regfile_missing() {
        let root = test_root("regfile_missing");

        let entry = make_instfile(
            "etc/config",
            InstFileType::RegFile(RegFile::from_sha256(sha256_of(b"anything"))),
        );
        assert!(!entry.is_content_modified(root.as_path()).unwrap());
    }

    #[test]
    fn symlink_matching_target() {
        let root = test_root("symlink_matching_target");
        std::fs::create_dir_all(root.join("etc")).unwrap();

        std::os::unix::fs::symlink("/usr/share/zoneinfo/UTC", root.join("etc/localtime")).unwrap();

        let entry = make_instfile(
            "etc/localtime",
            InstFileType::Symlink(Symlink::from_pathbuf("/usr/share/zoneinfo/UTC".into())),
        );
        assert!(!entry.is_content_modified(root.as_path()).unwrap());
    }

    #[test]
    fn symlink_modified_target() {
        let root = test_root("symlink_modified_target");
        std::fs::create_dir_all(root.join("etc")).unwrap();

        std::os::unix::fs::symlink("/usr/share/zoneinfo/US/Eastern", root.join("etc/localtime"))
            .unwrap();

        let entry = make_instfile(
            "etc/localtime",
            InstFileType::Symlink(Symlink::from_pathbuf("/usr/share/zoneinfo/UTC".into())),
        );
        assert!(entry.is_content_modified(root.as_path()).unwrap());
    }

    #[test]
    fn symlink_missing() {
        let root = test_root("symlink_missing");

        let entry = make_instfile(
            "etc/localtime",
            InstFileType::Symlink(Symlink::from_pathbuf("/usr/share/zoneinfo/UTC".into())),
        );
        assert!(!entry.is_content_modified(root.as_path()).unwrap());
    }

    #[test]
    fn remove_dangling_symlink() {
        let root = test_root("remove_dangling_symlink");
        std::fs::create_dir_all(root.join("etc")).unwrap();

        std::os::unix::fs::symlink("/nonexistent/target", root.join("etc/localtime")).unwrap();

        let entry = make_instfile(
            "etc/localtime",
            InstFileType::Symlink(Symlink::from_pathbuf("/nonexistent/target".into())),
        );
        entry.remove(root.as_path()).unwrap();
        assert!(!root.join("etc/localtime").exists());
    }

    #[test]
    fn remove_nonempty_directory_kept() {
        let root = test_root("remove_nonempty_directory_kept");
        std::fs::create_dir_all(root.join("etc")).unwrap();
        std::fs::write(root.join("etc/config"), b"x").unwrap();

        let entry = make_instfile("etc", InstFileType::Directory);
        entry.remove(root.as_path()).unwrap();
        assert!(root.join("etc").exists());
    }

    #[test]
    fn remove_missing_file_is_ok() {
        let root = test_root("remove_missing_file_is_ok");

        let entry = make_instfile(
            "etc/config",
            InstFileType::RegFile(RegFile::from_sha256(sha256_of(b"anything"))),
        );
        entry.remove(root.as_path()).unwrap();
    }

    #[test]
    fn keep_directory_remove_error_kinds_include_bedrock_mountpoints() {
        assert!(InstFile::can_keep_directory_remove_error(&std::io::Error::from(
            ErrorKind::NotFound
        )));
        assert!(InstFile::can_keep_directory_remove_error(&std::io::Error::from(
            ErrorKind::DirectoryNotEmpty
        )));
        assert!(InstFile::can_keep_directory_remove_error(&std::io::Error::from(
            ErrorKind::ResourceBusy
        )));
    }
}
