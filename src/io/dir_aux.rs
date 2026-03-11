//! Auxiliary helper methods for directories

use crate::{constant::*, error::*, io::FileAux, str::*};
use camino::{Utf8Path, Utf8PathBuf};
use std::{fs::File, vec::IntoIter};

pub trait DirAux {
    fn readdir(&self) -> Result<IntoIter<Utf8PathBuf>, Err>;

    /// Acquire an exclusive advisory lock on the directory.
    ///
    /// `lock_name` is a human-readable description shown to the user if another bpt instance
    /// holds the lock (e.g. "package database", "repository cache").
    ///
    /// The caller **must** hold the returned `File` for the duration of the locked operation;
    /// dropping it releases the lock.
    #[must_use = "dropping the returned File immediately releases the lock"]
    fn lock_rw(&self, lock_name: &str) -> Result<File, Err>;

    /// Acquire a shared advisory lock on the directory.
    ///
    /// `lock_name` is a human-readable description shown to the user if another bpt instance
    /// holds the lock (e.g. "package database", "repository cache").
    ///
    /// The caller **must** hold the returned `File` for the duration of the locked operation;
    /// dropping it releases the lock.
    #[must_use = "dropping the returned File immediately releases the lock"]
    fn lock_ro(&self, lock_name: &str) -> Result<File, Err>;
}

/// Rust's directory reading infrastructure has tedious error handling.  Abstract it away here.
impl DirAux for Utf8Path {
    fn readdir(&self) -> Result<IntoIter<Utf8PathBuf>, Err> {
        std::fs::read_dir(self)
            .map_err(|e| Err::ReadDir(self.to_string(), e))?
            .map(|entry| {
                entry
                    .map_err(|e| Err::ReadDir(self.to_string(), e))
                    .map(|entry| entry.path())
                    .and_then(|path| {
                        path.into_pathbuf()
                            .map_err(|e| Err::ReadDir(self.to_string(), e))
                    })
            })
            .collect::<Result<Vec<Utf8PathBuf>, Err>>()
            .map(|v| v.into_iter())
    }

    fn lock_rw(&self, lock_name: &str) -> Result<File, Err> {
        let lock_path = self.join(LOCK_FILE_NAME);

        let file = File::create_or_open_rw(&lock_path)?;
        file.lock_rw(lock_name).loc(lock_path)?;
        Ok(file)
    }

    fn lock_ro(&self, lock_name: &str) -> Result<File, Err> {
        let lock_path = self.join(LOCK_FILE_NAME);

        let file = File::create_or_open_ro(&lock_path)?;
        file.lock_ro(lock_name).loc(lock_path)?;
        Ok(file)
    }
}
