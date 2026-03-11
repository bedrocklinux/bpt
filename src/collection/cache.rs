use crate::{constant::*, error::*, io::*, location::*, str::*};
use camino::Utf8PathBuf;
use std::{fs::File, io::ErrorKind, time::SystemTime};

/// Generic local cache.
///
/// Used to avoid redundant network requests for files.
///
/// A [Cache] is often created preemptively, in case it is needed, but may never actually be used.
/// The lack of a cache may be acceptable, and so errors are delayed until the [Cache] is actually
/// needed.
pub struct Cache {
    dir: Utf8PathBuf,
    lock: Option<File>,
    label: &'static str,
}

pub enum CacheResult {
    Found(File),
    NewEntry(File),
}

impl Cache {
    fn parent_cache_dir(
        root: &RootDir,
        uid: u32,
        user_cache_dir: Option<Utf8PathBuf>,
        label: &'static str,
    ) -> Result<Utf8PathBuf, Err> {
        if uid == 0 {
            Ok(root.as_path().join(ROOT_CACHE_DIR))
        } else if let Some(dir) = user_cache_dir {
            Ok(dir)
        } else {
            // User might have permission at this path despite not being root, e.g. weird ownership
            // structure
            let _ = label;
            Ok(root.as_path().join(ROOT_CACHE_DIR))
        }
    }

    pub fn from_root_path(
        root: &RootDir,
        subdir: &'static str,
        label: &'static str,
    ) -> Result<Self, Err> {
        let user_cache_dir = match dirs::cache_dir() {
            Some(dir) => Some(
                dir.into_pathbuf()
                    .map_err(|e| Err::Open(label.to_string(), e))?,
            ),
            None => None,
        };
        let parent_cache_dir =
            Self::parent_cache_dir(root, nix::unistd::getuid().as_raw(), user_cache_dir, label)?;

        let dir = parent_cache_dir.join(subdir);

        Ok(Self {
            dir,
            lock: None,
            label,
        })
    }

    fn acquire_lock(&mut self) -> Result<(), Err> {
        if self.lock.is_some() {
            return Ok(());
        }

        std::fs::create_dir_all(&self.dir).map_err(|e| Err::CreateDir(self.dir.to_string(), e))?;

        self.lock = Some(self.dir.lock_rw(self.label)?);

        Ok(())
    }

    pub fn get(&mut self, url: &Url) -> Result<CacheResult, Err> {
        self.acquire_lock()?;

        let path = self.cache_path(url);
        match File::open_ro(&path) {
            Ok(file) => return Ok(CacheResult::Found(file)),
            Err(Err::Open(_, e)) if e.kind() == ErrorKind::NotFound => {}
            Err(e) => return Err(e),
        }

        File::create_anon(&self.dir).map(CacheResult::NewEntry)
    }

    pub fn cache_path(&self, url: &Url) -> Utf8PathBuf {
        self.dir.join(url.as_str().underscore_encode().as_ref())
    }

    /// Remove cache entries older than `max_age_days`.
    ///
    /// If `max_age_days` is `None`, retain all entries (no eviction).
    pub fn evict(&mut self, max_age_days: Option<u32>) -> Result<usize, Err> {
        self.evict_internal(max_age_days, true)
    }

    /// Count cache entries that would be evicted by [Self::evict].
    pub fn count_evictable(&mut self, max_age_days: Option<u32>) -> Result<usize, Err> {
        self.evict_internal(max_age_days, false)
    }

    fn evict_internal(&mut self, max_age_days: Option<u32>, remove: bool) -> Result<usize, Err> {
        let Some(max_days) = max_age_days else {
            return Ok(0);
        };

        self.acquire_lock()?;

        let max_age = std::time::Duration::from_secs(u64::from(max_days) * 24 * 60 * 60);
        let now = SystemTime::now();
        let mut count = 0;

        for path in self.dir.as_path().readdir()? {
            if path.file_name() == Some(LOCK_FILE_NAME) {
                continue;
            }

            let mtime = path
                .metadata()
                .and_then(|m| m.modified())
                .map_err(|e| Err::Stat(path.clone(), e))?;

            let age = now.duration_since(mtime).unwrap_or_default();

            if age > max_age {
                count += 1;
                if remove {
                    std::fs::remove_file(&path).map_err(|e| Err::Remove(path.to_string(), e))?;
                }
            }
        }

        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::unit_test_tmp_dir;
    use camino::Utf8Path;
    use std::fs::FileTimes;
    use std::time::Duration;

    fn test_dir(name: &str) -> Utf8PathBuf {
        unit_test_tmp_dir("cache", name)
    }

    fn make_cache(dir: &Utf8Path) -> Cache {
        Cache {
            dir: dir.to_owned(),
            lock: None,
            label: "test cache",
        }
    }

    #[test]
    fn test_cache_path_encoding() {
        let dir = test_dir("encoding");
        let cache = make_cache(&dir);
        // Use FromStr to construct a Url
        let url: Url = "https://example.com/path/file.bpt".parse().unwrap();
        let path = cache.cache_path(&url);
        assert!(path.starts_with(&dir));
        let filename = path.file_name().unwrap();
        assert!(!filename.contains('/'));
    }

    #[test]
    fn test_evict_removes_old_entries() {
        let dir = test_dir("evict-old");

        // Create a lock file (should be preserved)
        let lock_path = dir.join(LOCK_FILE_NAME);
        std::fs::write(&lock_path, b"").unwrap();

        // Create an "old" file (91 days ago)
        let old_file = dir.join("old-entry");
        let f = std::fs::File::create(&old_file).unwrap();
        let old_time = SystemTime::now() - Duration::from_secs(91 * 24 * 60 * 60);
        f.set_times(FileTimes::new().set_modified(old_time))
            .unwrap();
        drop(f);

        // Create a "new" file (1 day ago)
        let new_file = dir.join("new-entry");
        let f = std::fs::File::create(&new_file).unwrap();
        let new_time = SystemTime::now() - Duration::from_secs(24 * 60 * 60);
        f.set_times(FileTimes::new().set_modified(new_time))
            .unwrap();
        drop(f);

        let mut cache = make_cache(&dir);
        assert_eq!(cache.evict(Some(90)).unwrap(), 1);

        assert!(!old_file.exists(), "old entry should be removed");
        assert!(new_file.exists(), "new entry should be preserved");
        assert!(lock_path.exists(), "lock file should be preserved");
    }

    #[test]
    fn test_evict_none_is_noop() {
        let dir = test_dir("evict-none");

        let old_file = dir.join("should-remain");
        let f = std::fs::File::create(&old_file).unwrap();
        let old_time = SystemTime::now() - Duration::from_secs(365 * 24 * 60 * 60);
        f.set_times(FileTimes::new().set_modified(old_time))
            .unwrap();
        drop(f);

        let mut cache = make_cache(&dir);
        assert_eq!(cache.evict(None).unwrap(), 0);

        assert!(
            old_file.exists(),
            "entry should remain when max_age is None"
        );
    }

    #[test]
    fn test_evict_boundary() {
        let dir = test_dir("evict-boundary");

        // Create a file just inside the boundary (should be preserved)
        // Use 89 days + 23 hours to avoid race between set_times and evict's SystemTime::now()
        let boundary_file = dir.join("boundary-entry");
        let f = std::fs::File::create(&boundary_file).unwrap();
        let boundary_time =
            SystemTime::now() - Duration::from_secs(89 * 24 * 60 * 60 + 23 * 60 * 60);
        f.set_times(FileTimes::new().set_modified(boundary_time))
            .unwrap();
        drop(f);

        // Create a file just past the boundary (should be removed)
        let past_file = dir.join("past-entry");
        let f = std::fs::File::create(&past_file).unwrap();
        let past_time = SystemTime::now() - Duration::from_secs(91 * 24 * 60 * 60);
        f.set_times(FileTimes::new().set_modified(past_time))
            .unwrap();
        drop(f);

        let mut cache = make_cache(&dir);
        assert_eq!(cache.evict(Some(90)).unwrap(), 1);

        assert!(
            boundary_file.exists(),
            "entry at boundary should be preserved"
        );
        assert!(!past_file.exists(), "entry past boundary should be removed");
    }

    #[test]
    fn test_evict_empty_dir() {
        let dir = test_dir("evict-empty");

        let mut cache = make_cache(&dir);
        // Should not error on empty directory (only .lock will exist after acquire_lock)
        assert_eq!(cache.evict(Some(90)).unwrap(), 0);
    }

    #[test]
    fn test_parent_cache_dir_uses_root_cache_for_root_uid() {
        let dir = test_dir("parent-cache-root");
        let root = RootDir::from_path(&dir);

        let parent = Cache::parent_cache_dir(
            &root,
            0,
            Some(Utf8PathBuf::from("/tmp/ignored-user-cache")),
            "test cache",
        )
        .unwrap();

        assert_eq!(parent, dir.join(ROOT_CACHE_DIR));
    }

    #[test]
    fn test_parent_cache_dir_uses_user_cache_for_non_root_uid() {
        let dir = test_dir("parent-cache-user");
        let root = RootDir::from_path(&dir);
        let user_cache = Utf8PathBuf::from("/tmp/bpt-user-cache");

        let parent =
            Cache::parent_cache_dir(&root, 1000, Some(user_cache.clone()), "test cache").unwrap();

        assert_eq!(parent, user_cache);
    }

    #[test]
    fn test_parent_cache_dir_falls_back_to_root_cache_without_user_cache_dir() {
        let dir = test_dir("parent-cache-fallback");
        let root = RootDir::from_path(&dir);

        let parent = Cache::parent_cache_dir(&root, 1000, None, "test cache").unwrap();

        assert_eq!(parent, dir.join(ROOT_CACHE_DIR));
    }
}
