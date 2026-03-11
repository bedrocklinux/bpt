use crate::{error::*, file::*};
use camino::{Utf8Path, Utf8PathBuf};
use std::fs::{File, create_dir_all, remove_dir_all};
use std::path::Component;

fn reject_traversal(subpath: &Utf8Path) -> Result<(), Err> {
    if subpath.as_std_path().is_absolute()
        || subpath
            .as_std_path()
            .components()
            .any(|c| c == Component::ParentDir)
    {
        return Result::Err(Err::PathTraversal(subpath.to_string()));
    }
    Ok(())
}

/// Temporary directory handling
pub struct TmpDir {
    path: Utf8PathBuf,
    /// Normally we want to clean up temporary files, but in rare occasions - such as debugging a
    /// build failure - they should be retained.
    retain_on_close: bool,
}

/// Temporary file handling
///
/// This is never constructed directly, but instead as a subpath on TmpDir.  This helps keep the
/// temporary file/directory structure organized.
pub struct TmpFile(File);

impl TmpDir {
    pub fn new(bpt_conf: &BptConf) -> Result<Self, Err> {
        // This type may be created but never used.  Delay actually creating the directory until
        // a subdir/subfile is requested.
        Ok(Self {
            path: bpt_conf
                .build
                .tmp
                .join(format!("bpt-{}", std::process::id())),
            retain_on_close: false,
        })
    }

    pub fn retain_on_close(&mut self) {
        self.retain_on_close = true;
    }

    pub fn as_path(&self) -> &Utf8Path {
        &self.path
    }

    pub fn as_str(&self) -> &str {
        self.path.as_str()
    }

    pub fn subdir(&self, subpath: &Utf8Path) -> Result<TmpDir, Err> {
        reject_traversal(subpath)?;
        let path = self.as_path().join(subpath);
        create_dir_all(&path).map_err(|e| Err::CreateDir(path.to_string(), e))?;

        Ok(TmpDir {
            path,
            // The root tmpdir will delete itself and all children if retain_on_close is false.
            // No need to propagate that behavior to child subdirs.
            retain_on_close: true,
        })
    }

    pub fn subfile(&self, subpath: &Utf8Path) -> Result<TmpFile, Err> {
        reject_traversal(subpath)?;
        let path = self.as_path().join(subpath);
        if let Some(parent) = path.parent() {
            create_dir_all(parent).map_err(|e| Err::CreateDir(parent.to_string(), e))?;
        }
        let file = File::create(&path).map_err(|e| Err::CreateFile(path.to_string(), e))?;
        Ok(TmpFile(file))
    }
}

impl TmpFile {
    pub fn into_file(self) -> File {
        self.0
    }
}

impl Drop for TmpDir {
    fn drop(&mut self) {
        if !self.retain_on_close {
            let _ = remove_dir_all(&self.path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::unit_test_tmp_dir;

    fn make_tmpdir(name: &str) -> TmpDir {
        let path = unit_test_tmp_dir("tmpdir", name);
        TmpDir {
            path,
            retain_on_close: true, // tests clean up via unit_test_tmp_dir
        }
    }

    // reject_traversal tests

    #[test]
    fn reject_traversal_allows_simple_relative() {
        assert!(reject_traversal(Utf8Path::new("foo/bar")).is_ok());
    }

    #[test]
    fn reject_traversal_rejects_parent_dir() {
        assert!(reject_traversal(Utf8Path::new("foo/../bar")).is_err());
    }

    #[test]
    fn reject_traversal_rejects_leading_parent_dir() {
        assert!(reject_traversal(Utf8Path::new("../escape")).is_err());
    }

    #[test]
    fn reject_traversal_rejects_absolute_path() {
        assert!(reject_traversal(Utf8Path::new("/etc/passwd")).is_err());
    }

    // subdir tests

    #[test]
    fn subdir_creates_directory() {
        let tmp = make_tmpdir("subdir_creates");
        let sub = tmp.subdir(Utf8Path::new("child")).unwrap();
        assert!(sub.as_path().is_dir());
    }

    #[test]
    fn subdir_rejects_absolute_path() {
        let tmp = make_tmpdir("subdir_rejects_abs");
        assert!(tmp.subdir(Utf8Path::new("/etc")).is_err());
    }

    #[test]
    fn subdir_rejects_parent_traversal() {
        let tmp = make_tmpdir("subdir_rejects_parent");
        assert!(tmp.subdir(Utf8Path::new("a/../../escape")).is_err());
    }

    // subfile tests

    #[test]
    fn subfile_creates_file_and_parents() {
        let tmp = make_tmpdir("subfile_creates");
        let _f = tmp.subfile(Utf8Path::new("a/b/c.txt")).unwrap();
        assert!(tmp.as_path().join("a/b/c.txt").exists());
    }

    #[test]
    fn subfile_rejects_absolute_path() {
        let tmp = make_tmpdir("subfile_rejects_abs");
        assert!(tmp.subfile(Utf8Path::new("/tmp/evil")).is_err());
    }

    #[test]
    fn subfile_rejects_parent_traversal() {
        let tmp = make_tmpdir("subfile_rejects_parent");
        assert!(tmp.subfile(Utf8Path::new("../escape.txt")).is_err());
    }

    // drop tests

    #[test]
    fn drop_removes_directory_when_not_retained() {
        let path;
        {
            let tmp = TmpDir {
                path: unit_test_tmp_dir("tmpdir", "drop_removes"),
                retain_on_close: false,
            };
            let _f = tmp.subfile(Utf8Path::new("file.txt")).unwrap();
            path = tmp.as_path().to_owned();
            assert!(path.exists());
        }
        assert!(!path.exists());
    }

    #[test]
    fn drop_retains_directory_when_retained() {
        let path;
        {
            let mut tmp = TmpDir {
                path: unit_test_tmp_dir("tmpdir", "drop_retains"),
                retain_on_close: false,
            };
            let _f = tmp.subfile(Utf8Path::new("file.txt")).unwrap();
            tmp.retain_on_close();
            path = tmp.as_path().to_owned();
        }
        assert!(path.exists());
        // Clean up manually
        let _ = remove_dir_all(&path);
    }
}
