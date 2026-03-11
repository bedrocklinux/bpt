use crate::metadata::*;
use crate::{color::*, error::*, file::*, io::*, location::*, reconcile::*, str::*};
use camino::{Utf8Path, Utf8PathBuf};
use std::{
    collections::HashMap,
    fmt,
    fs::{File, FileTimes, remove_file},
    ops::Deref,
    str::FromStr,
    time::{Duration, SystemTime},
};

const SYNC_FRESHNESS_WINDOW: Duration = Duration::from_secs(60 * 60);

/// `bpt sync` index file reconciler.  Handles updating the index databases.
pub struct IdxReconciler {
    current: HashMap<IdxPathUrl, Utf8PathBuf>,
    target: HashMap<IdxPathUrl, ()>,
}

pub struct IdxReconcilerArgs<'a> {
    pub pkgidxdir: &'a Utf8Path,
    pub fileidxdir: &'a Utf8Path,
    pub force: bool,
    pub pubkeys: &'a PublicKeys,
    pub netutil: &'a NetUtil<'a>,
}

impl IdxReconciler {
    /// `bpt sync` may take specific index locations, in which case we're just adding them to the
    /// database, and we want to retain preexisting values.
    ///
    /// If `bpt sync` is run without any arguments, it is syncing the entire set of indexes to
    /// the configured index locations.  In this case, we want to remove any on-disk values that do
    /// not match configuration.
    ///
    /// When `remove_missing` is false, current entries not in the target set are filtered out so
    /// they are invisible to the reconciler and remain on disk untouched.
    pub fn new(
        pkgidxdir: &Utf8Path,
        fileidxdir: &Utf8Path,
        args: &[IdxPathUrl],
        remove_missing: bool,
    ) -> Result<Self, Err> {
        let mut current = HashMap::new();

        let mut paths = Vec::new();
        if pkgidxdir.exists() {
            paths.extend(pkgidxdir.readdir()?);
        }
        if fileidxdir.exists() {
            paths.extend(fileidxdir.readdir()?);
        }

        for path in paths {
            let str = path
                .file_name()
                .ok_or_else(|| Err::PathLacksFileName(path.to_string()))?
                .underscore_decode()?;
            if str == ".lock" {
                continue;
            }
            let source = IdxPathUrl::from_str(str.as_ref())?;
            current.insert(source, path);
        }

        let target: HashMap<_, _> = args.iter().map(|r| (r.clone(), ())).collect();

        if !remove_missing {
            current.retain(|key, _| target.contains_key(key));
        }

        Ok(Self { current, target })
    }
}

impl<'a> Reconciler<'a> for IdxReconciler {
    type Key = IdxPathUrl;
    type Current = Utf8PathBuf;
    type Target = ();
    type ApplyArgs = IdxReconcilerArgs<'a>;

    fn cmp(
        _key: &Self::Key,
        _current: &Self::Current,
        _target: &Self::Target,
    ) -> std::cmp::Ordering {
        // We always need to download the latest version to check for updates.
        std::cmp::Ordering::Less
    }

    fn current(&self) -> &HashMap<Self::Key, Self::Current> {
        &self.current
    }

    fn target(&self) -> &HashMap<Self::Key, Self::Target> {
        &self.target
    }

    fn create(
        idx_path_url: &Self::Key,
        _target: &Self::Target,
        args: &Self::ApplyArgs,
    ) -> Result<(), Err> {
        println!("{}Initializing {}", Color::Create, idx_path_url.color());

        // We don't know if the file is a PkgIdx or FileIdx until after we download it.  We're
        // assuming PkgIdxDb and FileIdxDb are on the same mount point such that we can use either
        // to downloaded and link anonymous files.  PkgIdxDb was selected arbitrarily.
        let dir = Some(args.pkgidxdir);
        let filename = idx_path_url.as_str().underscore_encode();

        match idx_path_url.open(args.pubkeys, args.netutil, dir)? {
            Idx::PkgIdx(pkgidx) => pkgidx.link(&args.pkgidxdir.join(filename.deref())),
            Idx::FileIdx(fileidx) => fileidx.link(&args.fileidxdir.join(filename.deref())),
        }
    }

    fn remove(
        idx_path_url: &Self::Key,
        current: &Self::Current,
        _args: &Self::ApplyArgs,
    ) -> Result<(), Err> {
        println!("{}Removing {}", Color::Remove, idx_path_url.color());
        remove_file(current).map_err(|e| Err::Remove(current.to_string(), e))
    }

    fn upgrade(
        idx_path_url: &Self::Key,
        filepath: &Self::Current,
        _target: &Self::Target,
        args: &Self::ApplyArgs,
    ) -> Result<(), Err> {
        if !args.force
            && let Some(mtime) = fresh_idx_mtime(filepath)?
        {
            println!("{}Skipping {}", Color::Deemphasize, idx_path_url.color());
            println!("{}Still fresh as of {}", Color::Deemphasize, mtime.color());
            return Ok(());
        }

        println!("{}Checking {}", Color::Deemphasize, idx_path_url.color());

        let current_idx = IdxPath::from_path(filepath).open(args.pubkeys, None)?;
        let current_timestamp = current_idx.timestamp();

        // We don't know if the file is a PkgIdx or FileIdx until after we download it.  We're
        // assuming PkgIdxDb and FileIdxDb are on the same mount point such that we can use either
        // to downloaded and link anonymous files.  PkgIdxDb was selected arbitrarily.
        let dir = Some(args.pkgidxdir);
        let target_idx = idx_path_url.open(args.pubkeys, args.netutil, dir)?;
        let target_timestamp = target_idx.timestamp();

        use std::cmp::Ordering::*;
        match target_timestamp.cmp(current_timestamp) {
            Less => return Err(Err::IndexTimestampOld(target_timestamp.to_string())),
            Equal => {
                println!(
                    "{}No update since {}",
                    Color::Deemphasize,
                    current_timestamp.color(),
                );

                touch_idx(filepath)?;
            }
            Greater => {
                println!(
                    "{}Updating {} {}-> {}",
                    Color::Upgrade,
                    current_timestamp.color(),
                    Color::Glue,
                    target_timestamp.color(),
                );

                remove_file(filepath).map_err(|e| Err::Remove(filepath.to_string(), e))?;
                target_idx.link(filepath)?;
            }
        }

        Ok(())
    }

    fn create_desc(
        idx_path_url: &Self::Key,
        _target: &Self::Target,
        f: &mut fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        writeln!(
            f,
            "{}Initialize {}{}",
            Color::Create,
            idx_path_url,
            Color::Default
        )
    }

    fn remove_desc(
        idx_path_url: &Self::Key,
        _current: &Self::Current,
        f: &mut fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        writeln!(
            f,
            "{}Remove {}{}",
            Color::Remove,
            idx_path_url,
            Color::Default
        )
    }

    fn upgrade_desc(
        idx_path_url: &Self::Key,
        _current: &Self::Current,
        _target: &Self::Target,
        f: &mut fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        writeln!(
            f,
            "{}Check {}{}",
            Color::Deemphasize,
            idx_path_url,
            Color::Default
        )
    }
}

fn fresh_idx_mtime(path: &Utf8Path) -> Result<Option<Timestamp>, Err> {
    let mtime = path
        .metadata()
        .and_then(|m| m.modified())
        .map_err(|e| Err::Stat(path.to_path_buf(), e))?;
    let age = SystemTime::now().duration_since(mtime).unwrap_or_default();
    if age < SYNC_FRESHNESS_WINDOW {
        return Timestamp::from_system_time(mtime)
            .map(Some)
            .map_err(Err::GetTime);
    }
    Ok(None)
}

fn touch_idx(path: &Utf8Path) -> Result<(), Err> {
    let file = File::open(path).map_err(|e| Err::Open(path.to_string(), e))?;
    file.set_times(FileTimes::new().set_modified(SystemTime::now()))
        .map_err(|e| Err::Open(path.to_string(), e))
}

#[cfg(test)]
mod tests {
    use crate::testutil::unit_test_tmp_dir;
    use camino::Utf8PathBuf;
    use std::fs::{self, File, FileTimes};

    use super::*;

    fn test_dir(name: &str) -> Utf8PathBuf {
        unit_test_tmp_dir("idx_rec", name)
    }

    #[test]
    fn fresh_idx_mtime_returns_timestamp_for_recent_file() {
        let dir = test_dir("recent");
        let path = dir.join("recent.pkgidx");
        fs::write(&path, b"test").unwrap();

        assert!(fresh_idx_mtime(path.as_path()).unwrap().is_some());
    }

    #[test]
    fn fresh_idx_mtime_returns_none_for_old_file() {
        let dir = test_dir("old");
        let path = dir.join("old.pkgidx");
        let file = File::create(&path).unwrap();
        let old_time = SystemTime::now() - SYNC_FRESHNESS_WINDOW - Duration::from_secs(1);
        file.set_times(FileTimes::new().set_modified(old_time))
            .unwrap();

        assert!(fresh_idx_mtime(path.as_path()).unwrap().is_none());
    }

    #[test]
    fn touch_idx_updates_mtime() {
        let dir = test_dir("touch");
        let path = dir.join("touch.pkgidx");
        let file = File::create(&path).unwrap();
        let old_time = SystemTime::now() - SYNC_FRESHNESS_WINDOW - Duration::from_secs(10);
        file.set_times(FileTimes::new().set_modified(old_time))
            .unwrap();
        drop(file);

        touch_idx(path.as_path()).unwrap();

        let mtime = path.metadata().unwrap().modified().unwrap();
        let age = SystemTime::now().duration_since(mtime).unwrap_or_default();
        assert!(age < Duration::from_secs(5));
    }
}
