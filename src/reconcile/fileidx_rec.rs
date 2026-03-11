use crate::{color::*, error::*, file::*, io::*, marshalling::*, metadata::*, reconcile::*};
use camino::Utf8Path;
use std::{
    collections::{HashMap, HashSet},
    fmt,
    fs::File,
    fs::remove_file,
    str::FromStr,
    time::SystemTime,
};

/// `bpt make-repo` [FileIdx] file reconciler.  Handles creating, removing, and updating [FileIdx]
/// files to align with the available set of [Bbuild] and [Bpt] files.
pub struct FileIdxReconciler<'a> {
    current: HashMap<Arch, CurrentFileIdx<'a>>,
    target: HashMap<Arch, TargetFileIdx>,
}

pub struct CurrentFileIdx<'a> {
    /// A currently-on-disk [FileIdx]'s modified time.
    ///
    /// If one of the underlying [Bbuild]s has been updated since the current [FileIdx] has been
    /// updated, the current [FileIdx] may have outdated information and needs to be rebuilt.
    mtime: SystemTime,
    /// File path to the [FileIdx].  This is used to delete it if it is not in the target set.
    path: &'a Utf8Path,
    /// The list of packages within the currently-on-disk [FileIdx].
    ///
    /// If this does not one-to-one match the list target list of packages - packages have
    /// been added, removed, or updated - we need to rebuild the [FileIdx].
    pkgids: HashSet<PkgId>,
}

pub struct TargetFileIdx {
    /// The target [FileIdx] modified time.
    ///
    /// This is the newest modified time of any underlying [Bbuild].  The corresponding [FileIdx]
    /// should be at least as new; if it isn't, it needs to be rebuilt.
    mtime: SystemTime,
    /// The target list of packages within the [FileIdx].
    ///
    /// If this does not one-to-one match the list target list of packages - packages have
    /// been added, removed, or updated - we need to rebuild the [FileIdx].
    pkgids: HashSet<PkgId>,
}

pub struct FileIdxRecArgs<'a> {
    /// Directory containing input/output files.
    pub dir: &'a Utf8Path,
    /// Key to sign created files
    pub privkey: &'a PrivKey,
    /// `bpt make-repo` creates [Reconciler]s and displays their [ReconcilePlan]s for
    /// confirmation before it executes any of the [ReconcilePlan]s.  This means the [Bpt]s needed
    /// to build [FileIdx]s may not yet be available when [FileIdxReconciler::new] is run.
    ///
    /// We need public keys to load the [Bpt]s during eventual execution of the plan.
    pub pubkeys: &'a PublicKeys,
}

impl<'a> FileIdxReconciler<'a> {
    pub fn new(
        // `bpt make-repo` collects the system time, path, and type for every file in the
        // directory.  This is the most natural way for it to pass the information along to this
        // reconciler.
        bbuilds: &'a [(SystemTime, &Utf8Path, Bbuild)],
        fileidxs: &'a [(SystemTime, &Utf8Path, FileIdx)],
        conf: &BptConf,
    ) -> Result<Self, Err> {
        let mut current = HashMap::new();
        for (mtime, path, fileidx) in fileidxs {
            let arch: Arch = path
                .file_stem()
                .map(Arch::from_str)
                .ok_or_else(|| Err::FilenameStemArch(path.to_string()))?
                .map_err(|_| Err::FilenameStemArch(path.to_string()))?;
            let pkgids = fileidx.pkgids().cloned().collect::<HashSet<PkgId>>();
            current.insert(
                arch,
                CurrentFileIdx {
                    mtime: *mtime,
                    path,
                    pkgids,
                },
            );
        }

        let mut target = HashMap::new();
        for arch in conf.make_repo.archs.iter().filter(|&&a| a != Arch::bbuild) {
            let mut target_pkgids = HashSet::new();
            let mut target_mtime = SystemTime::UNIX_EPOCH;
            for (mtime, _, bbuild) in bbuilds.iter() {
                if !bbuild.pkginfo().makearchs.as_slice().contains(arch) {
                    continue;
                }

                target_pkgids.insert(bbuild.pkgid().with_arch(*arch));
                if *mtime > target_mtime {
                    target_mtime = *mtime;
                }
            }
            target.insert(
                *arch,
                TargetFileIdx {
                    mtime: target_mtime,
                    pkgids: target_pkgids,
                },
            );
        }

        Ok(Self { current, target })
    }
}

impl<'a> Reconciler<'a> for FileIdxReconciler<'a> {
    type Key = Arch;
    type Current = CurrentFileIdx<'a>;
    type Target = TargetFileIdx;
    type ApplyArgs = FileIdxRecArgs<'a>;

    fn cmp(
        _arch: &Self::Key,
        current: &Self::Current,
        target: &Self::Target,
    ) -> std::cmp::Ordering {
        // We need to update the fileidx if:
        // - Any target package is newer than the index, as this could indicate new paths
        // - The target set of packages does not match the contained set of packages.
        if current.mtime < target.mtime || current.pkgids != target.pkgids {
            std::cmp::Ordering::Less
        } else {
            std::cmp::Ordering::Equal
        }
    }

    fn current(&self) -> &HashMap<Self::Key, Self::Current> {
        &self.current
    }

    fn target(&self) -> &HashMap<Self::Key, Self::Target> {
        &self.target
    }

    fn create(arch: &Self::Key, target: &Self::Target, args: &Self::ApplyArgs) -> Result<(), Err> {
        let bpts = collect_bpts(&target.pkgids, args.dir, args.pubkeys)?;
        let fileidx = FileIdx::from_bpts(&bpts, args.dir, args.privkey)?;
        fileidx.link(&args.dir.join(format!("{arch}.fileidx")))
    }

    fn remove(
        _arch: &Self::Key,
        current: &Self::Current,
        _args: &Self::ApplyArgs,
    ) -> Result<(), Err> {
        remove_file(current.path).map_err(|e| Err::Remove(current.path.to_string(), e))
    }

    fn upgrade(
        _arch: &Self::Key,
        current: &Self::Current,
        target: &Self::Target,
        args: &Self::ApplyArgs,
    ) -> Result<(), Err> {
        let bpts = collect_bpts(&target.pkgids, args.dir, args.pubkeys)?;
        let fileidx = FileIdx::from_bpts(&bpts, args.dir, args.privkey)?;
        // We may be rebuilding an out-of-date index, e.g. if source files updated.
        //
        // In such a case, we should first clear the old index that is blocking our output path
        // before linking the built package.
        if let Err(e) = remove_file(current.path) {
            if e.kind() != std::io::ErrorKind::NotFound {
                return Err(Err::Remove(current.path.to_string(), e));
            }
        }
        fileidx.link(current.path)
    }

    fn create_desc(
        arch: &Self::Key,
        _target: &Self::Target,
        f: &mut fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        writeln!(
            f,
            "{}Create {}{}{}.{}fileidx{}",
            Color::Create,
            Color::Arch,
            arch,
            Color::Glue,
            Color::File,
            Color::Default,
        )
    }

    fn remove_desc(
        arch: &Self::Key,
        _current: &Self::Current,
        f: &mut fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        writeln!(
            f,
            "{}Remove {}{}{}.{}fileidx{}",
            Color::Remove,
            Color::Arch,
            arch,
            Color::Glue,
            Color::File,
            Color::Default,
        )
    }

    fn upgrade_desc(
        arch: &Self::Key,
        _current: &Self::Current,
        _target: &Self::Target,
        f: &mut fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        writeln!(
            f,
            "{}Update {}{}{}.{}fileidx{}",
            Color::Upgrade,
            Color::Arch,
            arch,
            Color::Glue,
            Color::File,
            Color::Default,
        )
    }
}

fn collect_bpts(
    pkgids: &HashSet<PkgId>,
    dir: &Utf8Path,
    pubkeys: &PublicKeys,
) -> Result<Vec<Bpt>, Err> {
    let mut bpts = Vec::new();

    for path in dir.readdir()? {
        match path.extension() {
            Some("bpt") => {
                let file = File::open_ro(&path)?;
                let bpt = Bpt::from_file(file, pubkeys).loc(path)?;
                if pkgids.contains(bpt.pkgid()) {
                    bpts.push(bpt);
                }
            }
            _ => continue,
        }
    }

    bpts.sort_by_key(|a| a.pkgid().to_string());
    Ok(bpts)
}
