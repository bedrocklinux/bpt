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

/// `bpt make-repo` [PkgIdx] file reconciler.  Handles creating, removing, and updating [PkgIdx]
/// files to align with the available set of [Bbuild] and [Bpt] files.
pub struct PkgIdxReconciler<'a> {
    current: HashMap<Arch, CurrentPkgIdx<'a>>,
    target: HashMap<Arch, TargetPkgIdx>,
}

pub struct CurrentPkgIdx<'a> {
    /// A currently-on-disk [PkgIdx]'s modified time.
    ///
    /// If one of the underlying [Bbuild]s has been updated since the current [PkgIdx] has been
    /// updated, the current [PkgIdx] may have outdated information and needs to be rebuilt.
    mtime: SystemTime,
    /// File path to the [PkgIdx].  This is used to delete it if it is not in the target set.
    path: &'a Utf8Path,
    /// The list of packages within the currently-on-disk [PkgIdx].
    ///
    /// If this does not one-to-one match the list target list of packages - packages have
    /// been added, removed, or updated - we need to rebuild the [PkgIdx].
    pkgids: HashSet<PkgId>,
}

pub struct TargetPkgIdx {
    /// The target [PkgIdx] modified time.
    ///
    /// This is the newest modified time of any underlying [Bbuild].  The corresponding [PkgIdx]
    /// should be at least as new; if it isn't, it needs to be rebuilt.
    mtime: SystemTime,
    /// The target list of packages within the [PkgIdx].
    ///
    /// If this does not one-to-one match the list target list of packages - packages have
    /// been added, removed, or updated - we need to rebuild the [PkgIdx].
    pkgids: HashSet<PkgId>,
}

pub struct PkgIdxRecArgs<'a> {
    /// Directory containing input/output files.
    pub dir: &'a Utf8Path,
    /// Key to sign created files
    pub privkey: &'a PrivKey,
    /// `bpt make-repo` creates [Reconciler]s and displays their [ReconcilePlan]s for
    /// confirmation before it executes any of the [ReconcilePlan]s.  This means the [Bpt]s needed
    /// to build [PkgIdx]s may not yet be available when [PkgIdxReconciler::new] is run.
    ///
    /// We need public keys to load the [Bpt]s during eventual execution of the plan.
    pub pubkeys: &'a PublicKeys,
}

impl<'a> PkgIdxReconciler<'a> {
    pub fn new(
        // `bpt make-repo` collects the system time, path, and type for every file in the
        // directory.  This is the most natural way for it to pass the information along to this
        // reconciler.
        bbuilds: &'a [(SystemTime, &Utf8Path, Bbuild)],
        pkgidxs: &'a [(SystemTime, &Utf8Path, PkgIdx)],
        conf: &BptConf,
    ) -> Result<Self, Err> {
        let mut current = HashMap::new();
        for (mtime, path, pkgidx) in pkgidxs {
            let arch = path
                .file_stem()
                .map(Arch::from_str)
                .ok_or_else(|| Err::FilenameStemArch(path.to_string()))?
                .map_err(|_| Err::FilenameStemArch(path.to_string()))?;
            let pkgids = pkgidx.pkgids().cloned().collect();
            current.insert(
                arch,
                CurrentPkgIdx {
                    mtime: *mtime,
                    path,
                    pkgids,
                },
            );
        }

        let mut target = HashMap::new();
        for arch in &conf.make_repo.archs {
            let mut pkgids = HashSet::new();
            let mut newest_mtime = SystemTime::UNIX_EPOCH;
            for (mtime, _, bbuild) in bbuilds.iter() {
                // We're currently collecting information about the PkgIdx for `arch`.  Only
                // consider bbuilds that target `arch`.
                //
                // Unless, that is, we're building the PkgIdx for bbuilds, in which case we want
                // all bbuilds.
                if !bbuild.pkginfo().makearchs.as_slice().contains(arch) && arch != &Arch::bbuild {
                    continue;
                }

                pkgids.insert(bbuild.pkgid().with_arch(*arch));
                if *mtime > newest_mtime {
                    newest_mtime = *mtime;
                }
            }
            target.insert(
                *arch,
                TargetPkgIdx {
                    mtime: newest_mtime,
                    pkgids,
                },
            );
        }

        Ok(Self { current, target })
    }
}

impl<'a> Reconciler<'a> for PkgIdxReconciler<'a> {
    type Key = Arch;
    type Current = CurrentPkgIdx<'a>;
    type Target = TargetPkgIdx;
    type ApplyArgs = PkgIdxRecArgs<'a>;

    fn cmp(_key: &Self::Key, current: &Self::Current, target: &Self::Target) -> std::cmp::Ordering {
        // We need to update the PkgIdx if:
        // - Any target package is newer than the index, as this could indicate new metadata.
        // - The target set of packages does not exactly match the contained set of packages.
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
        let pkginfos = collect_pkginfos(&target.pkgids, args.dir, args.pubkeys)?;
        let pkgidx = PkgIdx::from_pkginfos(&pkginfos, args.dir, args.privkey)?;
        pkgidx.link(&args.dir.join(format!("{arch}.pkgidx")))
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
        let pkginfos = collect_pkginfos(&target.pkgids, args.dir, args.pubkeys)?;
        let pkgidx = PkgIdx::from_pkginfos(&pkginfos, args.dir, args.privkey)?;
        // We may be rebuilding an out-of-date index, e.g. if source files updated.
        //
        // In such a case, we should first clear the old index that is blocking our output path
        // before linking the built package.
        if let Err(e) = remove_file(current.path) {
            if e.kind() != std::io::ErrorKind::NotFound {
                return Err(Err::Remove(current.path.to_string(), e));
            }
        }
        pkgidx.link(current.path)
    }

    fn create_desc(
        arch: &Self::Key,
        _target: &Self::Target,
        f: &mut fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        writeln!(
            f,
            "{}Create {}{}{}.{}pkgidx{}",
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
            "{}Remove {}{}{}.{}pkgidx{}",
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
            "{}Update {}{}{}.{}pkgidx{}",
            Color::Upgrade,
            Color::Arch,
            arch,
            Color::Glue,
            Color::File,
            Color::Default,
        )
    }
}

/// Collect the [PkgInfo]s needed to build a [PkgIdx].
///
/// This runs:
///
/// - After `bpt make-repo` creates [Reconciler]s
/// - After `bpt make-repo` displays their [ReconcilePlan]s for confirmation
/// - After `bpt make-repo` runs the [Bpt] [ReconcilePlan] to build the [Bpt]s
/// - As part of `bpt make-repo` running the [PkgIdx] [ReconcilePlan]
///
/// And thus the [Bpt]s should be available on disk at this point.
fn collect_pkginfos(
    pkgids: &HashSet<PkgId>,
    dir: &Utf8Path,
    pubkeys: &PublicKeys,
) -> Result<Vec<PkgInfo>, Err> {
    let mut pkginfos = Vec::new();

    for path in dir.readdir()? {
        match path.extension() {
            Some("bpt") => {
                let file = File::open_ro(&path)?;
                let bpt = Bpt::from_file(file, pubkeys).loc(&path)?;
                if pkgids.contains(bpt.pkgid()) {
                    let mut pkginfo = bpt.pkginfo().clone();
                    pkginfo.repopath = RepoPath::from_path(&path)?;
                    pkginfos.push(pkginfo);
                }
            }
            Some("bbuild") => {
                let file = File::open_ro(&path)?;
                let bbuild = Bbuild::from_file(file, pubkeys, None).loc(&path)?;
                if pkgids.contains(bbuild.pkgid()) {
                    let mut pkginfo = bbuild.pkginfo().clone();
                    pkginfo.repopath = RepoPath::from_path(&path)?;
                    pkginfos.push(pkginfo);
                }
            }
            _ => continue,
        }
    }

    pkginfos.sort_by_key(|pkginfo| pkginfo.pkgid.clone());
    Ok(pkginfos)
}
