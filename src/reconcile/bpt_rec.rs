use crate::{color::*, error::*, file::*, marshalling::*, metadata::*, reconcile::*};
use camino::Utf8Path;
use std::{collections::HashMap, fmt, fs::remove_file, time::SystemTime};

/// `bpt make-repo` [Bpt] file reconciler.  Handles creating, removing, and updating binary [Bpt]
/// files to align with the available set of [Bbuild] files.
pub struct BptReconciler<'a> {
    current: HashMap<PkgId, CurrentBpt<'a>>,
    target: HashMap<PkgId, TargetBpt<'a>>,
}

pub struct CurrentBpt<'a> {
    /// A currently-on-disk [Bpt]'s modified time.
    ///
    /// If the underlying [Bbuild]s has been updated since the current [Bpt] has been
    /// created, the current [Bpt] may have outdated information and needs to be rebuilt.
    mtime: SystemTime,
    /// File path to the [Bpt].  This is used to delete it if it is not in the target set.
    path: &'a Utf8Path,
}

pub struct TargetBpt<'a> {
    /// The target [Bpt] modified time.
    ///
    /// This is the modified time the underlying [Bbuild].  The corresponding [Bpt]
    /// should be at least as new; if it isn't, it needs to be rebuilt.
    mtime: SystemTime,
    /// The [Bbuild] defining how to build the target [Bpt]
    bbuild: &'a Bbuild,
}

impl<'a> BptReconciler<'a> {
    pub fn new(
        // `bpt make-repo` collects the system time, path, and type for every file in the
        // directory.  This is the most natural way for it to pass the information along to this
        // reconciler.
        bbuilds: &'a [(SystemTime, &Utf8Path, Bbuild)],
        bpts: &'a [(SystemTime, &Utf8Path, Bpt)],
        conf: &BptConf,
    ) -> Self {
        let current = bpts
            .iter()
            .map(|(mtime, path, bpt)| {
                (
                    bpt.pkgid().clone(),
                    CurrentBpt {
                        mtime: *mtime,
                        path,
                    },
                )
            })
            .collect::<HashMap<_, _>>();

        let mut target = HashMap::new();
        for arch in &conf.make_repo.archs {
            for (mtime, _, bbuild) in bbuilds.iter() {
                if bbuild.pkginfo().makearchs.as_slice().contains(arch) {
                    target.insert(
                        bbuild.pkgid().with_arch(*arch),
                        TargetBpt {
                            mtime: *mtime,
                            bbuild,
                        },
                    );
                }
            }
        }

        Self { current, target }
    }
}

impl<'a> Reconciler<'a> for BptReconciler<'a> {
    type Key = PkgId;
    type Current = CurrentBpt<'a>;
    type Target = TargetBpt<'a>;
    type ApplyArgs = BuildArgs<'a>;

    fn cmp(_key: &Self::Key, current: &Self::Current, target: &Self::Target) -> std::cmp::Ordering {
        // We need to upgrade if the existing bpt is older (timestamp is less) than the bbuild, as
        // that indicates the bbuild has been updated since the bpt was created.
        use std::cmp::Ordering::*;
        match current.mtime.cmp(&target.mtime) {
            Less => Less,
            Equal | Greater => Equal,
        }
    }

    fn current(&self) -> &HashMap<Self::Key, Self::Current> {
        &self.current
    }

    fn target(&self) -> &HashMap<Self::Key, Self::Target> {
        &self.target
    }

    fn create(key: &Self::Key, target: &Self::Target, args: &Self::ApplyArgs) -> Result<(), Err> {
        let bpt = target.bbuild.build(args, key.arch)?;
        let path = args.out_dir.join(key.canonical_filename());
        bpt.link(&path)
    }

    fn remove(
        _key: &Self::Key,
        current: &Self::Current,
        _args: &Self::ApplyArgs,
    ) -> Result<(), Err> {
        remove_file(current.path).map_err(|e| Err::Remove(current.path.to_string(), e))
    }

    fn upgrade(
        key: &Self::Key,
        current: &Self::Current,
        target: &Self::Target,
        args: &Self::ApplyArgs,
    ) -> Result<(), Err> {
        let bpt = target.bbuild.build(args, key.arch)?;
        // We may be rebuilding an out-of-date package, e.g. if its bbuild was updated and has a
        // newer timestamp.
        //
        // In such a case, we should first clear the old package that is blocking our output path
        // before linking the built package.
        if let Err(e) = remove_file(current.path) {
            if e.kind() != std::io::ErrorKind::NotFound {
                return Err(Err::Remove(current.path.to_string(), e));
            }
        }
        bpt.link(current.path)
    }

    fn apply_plan(plan: &ReconcilePlan<'a, Self>, args: &Self::ApplyArgs) -> Result<(), Err> {
        let mut create = HashMap::new();
        for (key, target) in &plan.create {
            create.insert((*key).clone(), *target);
        }

        let mut upgrade = HashMap::new();
        for (key, current, target) in &plan.upgrade {
            upgrade.insert((*key).clone(), (*current, *target));
        }

        let sorted_targets = sort_build_targets(
            create
                .iter()
                .map(|(pkgid, target)| BuildTarget {
                    pkgid: pkgid.clone(),
                    bbuild: target.bbuild,
                    arch: pkgid.arch,
                })
                .chain(upgrade.iter().map(|(pkgid, (_, target))| BuildTarget {
                    pkgid: pkgid.clone(),
                    bbuild: target.bbuild,
                    arch: pkgid.arch,
                }))
                .collect(),
        )?;

        // Build everything first.  This can fail due to dependency cycles, missing deps, or build
        // errors.  Avoid mutating on-disk repo files until this phase succeeds.
        let mut built = Vec::new();
        for target in sorted_targets {
            let bpt = target.bbuild.build(args, target.arch)?;
            args.available_bpts.borrow_mut().add(bpt);
            built.push(target.pkgid);
        }

        // With builds complete, apply output mutations.
        for pkgid in built {
            let out_path = if let Some((current, _)) = upgrade.get(&pkgid) {
                if let Err(e) = remove_file(current.path) {
                    if e.kind() != std::io::ErrorKind::NotFound {
                        return Err(Err::Remove(current.path.to_string(), e));
                    }
                }
                current.path.to_owned()
            } else {
                args.out_dir.join(pkgid.canonical_filename())
            };

            args.available_bpts
                .borrow()
                .get(&pkgid)
                .ok_or_else(|| Err::UnableToLocateRepositoryPkg(pkgid.to_pkgidpart()))?
                .link(&out_path)?;
        }

        // Remove stale entries last, since failure here leaves extra files around but doesn't
        // jeopardize newly built/updated outputs.
        for (key, current) in &plan.remove {
            Self::remove(key, current, args)?;
            args.available_bpts.borrow_mut().remove(key);
        }

        Ok(())
    }

    fn create_desc(
        key: &Self::Key,
        _target: &Self::Target,
        f: &mut fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        writeln!(
            f,
            "{}Create{} {}",
            Color::Create,
            Color::Default,
            key.color().canonical_filename()
        )
    }

    fn remove_desc(
        key: &Self::Key,
        _current: &Self::Current,
        f: &mut fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        writeln!(
            f,
            "{}Remove{} {}",
            Color::Remove,
            Color::Default,
            key.color().canonical_filename()
        )
    }

    fn upgrade_desc(
        key: &Self::Key,
        _current: &Self::Current,
        _target: &Self::Target,
        f: &mut fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        writeln!(
            f,
            "{}Update{} {}",
            Color::Upgrade,
            Color::Default,
            key.color().canonical_filename()
        )
    }
}
