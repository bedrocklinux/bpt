//! Plan application for installed-package reconciliation.

use crate::{
    collection::{AvailableBpts, Cache, InstalledPkgs, RepositoryPkgs},
    error::Err,
    file::{Bpt, PublicKeys},
    io::NetUtil,
    location::Pkg,
    marshalling::MagicNumber,
    metadata::{Arch, PartId, PkgId},
    reconcile::instpkg_rec::{
        InstPkgApplyArgs, InstPkgPlan, PreparedInstallOp, PreparedReplaceOp, RemoveOp, TargetSource,
    },
    reconcile::{BuildTarget, sort_build_targets},
};
use camino::Utf8PathBuf;
use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
};

impl InstPkgPlan {
    pub fn is_empty(&self) -> bool {
        self.retain.is_empty()
            && self.remove.is_empty()
            && self.install.is_empty()
            && self.upgrade.is_empty()
            && self.downgrade.is_empty()
    }

    pub fn needs_builds(&self) -> bool {
        self.install.iter().any(|op| op.source.needs_build())
            || self.upgrade.iter().any(|op| op.to.needs_build())
            || self.downgrade.iter().any(|op| op.to.needs_build())
    }

    pub fn summary(&self) -> String {
        let mut parts = Vec::new();
        if !self.install.is_empty() {
            parts.push(format!("{} install", self.install.len()));
        }
        if !self.remove.is_empty() {
            parts.push(format!("{} remove", self.remove.len()));
        }
        if !self.upgrade.is_empty() {
            parts.push(format!("{} upgrade", self.upgrade.len()));
        }
        if !self.downgrade.is_empty() {
            parts.push(format!("{} downgrade", self.downgrade.len()));
        }
        if !self.retain.is_empty() {
            parts.push(format!("{} retain", self.retain.len()));
        }
        if parts.is_empty() {
            "No changes needed".to_string()
        } else {
            format!("Updated installed package set ({})", parts.join(", "))
        }
    }

    pub fn apply(self, args: InstPkgApplyArgs<'_>) -> Result<Vec<Utf8PathBuf>, Err> {
        let InstPkgPlan {
            retain: _retain,
            remove,
            install,
            upgrade,
            downgrade,
            world_entries,
        } = self;
        let available_bpts = args.available_bpts;
        let mut build_sources = Vec::<(PkgId, crate::file::Bbuild, Arch)>::new();
        let mut prepared_install = Vec::new();
        let mut prepared_upgrade = Vec::new();
        let mut prepared_downgrade = Vec::new();

        for op in install {
            match op.source {
                TargetSource::Installed(_) => {
                    panic!("install operation cannot retain installed source")
                }
                TargetSource::Bpt(bpt) => {
                    let pkgid = bpt.pkgid().clone();
                    available_bpts.borrow_mut().add(bpt);
                    prepared_install.push(PreparedInstallOp { pkgid });
                }
                TargetSource::Bbuild { bbuild, pkgid } => {
                    build_sources.push((pkgid.clone(), bbuild, pkgid.arch));
                    prepared_install.push(PreparedInstallOp { pkgid });
                }
            }
        }
        for op in upgrade {
            match op.to {
                TargetSource::Installed(_) => {
                    panic!("upgrade operation cannot retain installed source")
                }
                TargetSource::Bpt(bpt) => {
                    let pkgid = bpt.pkgid().clone();
                    available_bpts.borrow_mut().add(bpt);
                    prepared_upgrade.push(PreparedReplaceOp {
                        from: op.from,
                        to: pkgid,
                    });
                }
                TargetSource::Bbuild { bbuild, pkgid } => {
                    build_sources.push((pkgid.clone(), bbuild, pkgid.arch));
                    prepared_upgrade.push(PreparedReplaceOp {
                        from: op.from,
                        to: pkgid,
                    });
                }
            }
        }
        for op in downgrade {
            match op.to {
                TargetSource::Installed(_) => {
                    panic!("downgrade operation cannot retain installed source")
                }
                TargetSource::Bpt(bpt) => {
                    let pkgid = bpt.pkgid().clone();
                    available_bpts.borrow_mut().add(bpt);
                    prepared_downgrade.push(PreparedReplaceOp {
                        from: op.from,
                        to: pkgid,
                    });
                }
                TargetSource::Bbuild { bbuild, pkgid } => {
                    build_sources.push((pkgid.clone(), bbuild, pkgid.arch));
                    prepared_downgrade.push(PreparedReplaceOp {
                        from: op.from,
                        to: pkgid,
                    });
                }
            }
        }

        if !build_sources.is_empty() {
            Self::load_repository_binaries(
                args.repository,
                args.pubkeys,
                args.netutil,
                args.pkgcache,
                &mut available_bpts.borrow_mut(),
            )?;
            let Some(buildargs) = args.buildargs else {
                return Err(Err::InputFieldInvalid(
                    "build arguments",
                    "missing build arguments for required build phase".to_string(),
                ));
            };
            let build_targets = build_sources
                .iter()
                .map(|(pkgid, bbuild, arch)| BuildTarget {
                    pkgid: pkgid.clone(),
                    bbuild,
                    arch: *arch,
                })
                .collect::<Vec<_>>();
            let sorted = sort_build_targets(build_targets)?;
            for target in sorted {
                let bpt = target.bbuild.build(buildargs, target.arch)?;
                available_bpts.borrow_mut().add(bpt);
            }
        }

        Self::conflict_check(
            args.installed,
            &remove,
            &prepared_install,
            &prepared_upgrade,
            &prepared_downgrade,
            &available_bpts.borrow(),
        )?;

        let mut bptnew = Vec::new();
        for op in remove {
            let instpkg = args
                .installed
                .get(&op.pkgid)
                .expect("remove package missing from installed set");
            instpkg.uninstall(args.root, args.purge, args.forget)?;
        }
        for op in prepared_install {
            let mut bpt = available_bpts
                .borrow_mut()
                .remove(&op.pkgid)
                .expect("install bpt missing from build phase storage");
            bpt.install(args.root, args.instpkg_dir, &mut bptnew)?;
        }
        for op in prepared_upgrade {
            let old = args
                .installed
                .get(&op.from)
                .expect("upgrade source missing from installed set");
            let mut bpt = available_bpts
                .borrow_mut()
                .remove(&op.to)
                .expect("upgrade bpt missing from build phase storage");
            bpt.upgrade(old, args.root, args.instpkg_dir, &mut bptnew)?;
        }
        for op in prepared_downgrade {
            let old = args
                .installed
                .get(&op.from)
                .expect("downgrade source missing from installed set");
            let mut bpt = available_bpts
                .borrow_mut()
                .remove(&op.to)
                .expect("downgrade bpt missing from build phase storage");
            bpt.upgrade(old, args.root, args.instpkg_dir, &mut bptnew)?;
        }

        args.world.replace_entries(world_entries);
        args.world.save()?;
        Ok(bptnew)
    }

    pub(super) fn conflict_check(
        installed: &InstalledPkgs,
        remove: &[RemoveOp],
        install: &[PreparedInstallOp],
        upgrade: &[PreparedReplaceOp],
        downgrade: &[PreparedReplaceOp],
        available_bpts: &AvailableBpts,
    ) -> Result<(), Err> {
        let removed = remove
            .iter()
            .map(|op| op.pkgid.clone())
            .chain(upgrade.iter().map(|op| op.from.clone()))
            .chain(downgrade.iter().map(|op| op.from.clone()))
            .collect::<HashSet<_>>();
        let mut new_paths = HashMap::<Utf8PathBuf, (PkgId, bool)>::new();
        for pkgid in install
            .iter()
            .map(|op| &op.pkgid)
            .chain(upgrade.iter().map(|op| &op.to))
            .chain(downgrade.iter().map(|op| &op.to))
        {
            let bpt = available_bpts
                .get(pkgid)
                .expect("conflict check bpt missing from build phase storage");
            for entry in bpt.instfiles() {
                let is_dir = entry.entry_type.is_dir();
                if let Some((other_pkgid, other_is_dir)) = new_paths.get(&entry.path)
                    && !(*other_is_dir && is_dir)
                {
                    return Err(Err::InstallConflict(
                        entry.path.clone(),
                        Box::new(other_pkgid.clone()),
                        Box::new(pkgid.clone()),
                    ));
                }
                new_paths
                    .entry(entry.path.clone())
                    .or_insert_with(|| (pkgid.clone(), is_dir));
            }
        }
        for (pkgid, instpkg) in installed.as_map() {
            if removed.contains(pkgid) {
                continue;
            }
            for entry in instpkg.entries() {
                if let Some((new_pkgid, new_is_dir)) = new_paths.get(&entry.path)
                    && !(entry.entry_type.is_dir() && *new_is_dir)
                {
                    return Err(Err::InstallConflict(
                        entry.path.clone(),
                        Box::new(new_pkgid.clone()),
                        Box::new(pkgid.clone()),
                    ));
                }
            }
        }
        Ok(())
    }

    fn load_repository_binaries(
        repository: &RepositoryPkgs,
        pubkeys: &PublicKeys,
        netutil: &RefCell<NetUtil<'_>>,
        pkgcache: &RefCell<Cache>,
        available_bpts: &mut AvailableBpts,
    ) -> Result<(), Err> {
        for pkgid in repository.pkgids() {
            if pkgid.arch == Arch::bbuild || available_bpts.get(pkgid).is_some() {
                continue;
            }
            let partid = pkgid.to_pkgidpart();
            let pkginfo = repository
                .best_pkg_match(&partid, &[pkgid.arch])
                .ok_or_else(|| Err::UnableToLocateAvailablePkg(partid.clone()))?;
            let path_url = pkginfo.repopath.as_pkg_path_url()?;
            let pkg = {
                let mut pkgcache = pkgcache.borrow_mut();
                let netutil = netutil.borrow();
                path_url.open(&netutil, &mut pkgcache, pubkeys, None, None)?
            };
            let bpt = match pkg {
                Pkg::Bpt(bpt) => bpt,
                Pkg::Bbuild(_) => {
                    return Err(Err::InvalidMagicNumber(
                        path_url.to_string(),
                        Bpt::DESCRIPTION,
                    ));
                }
            };
            available_bpts.add(bpt);
        }
        Ok(())
    }

    pub(super) fn world_entries_for_pkgid(&self, pkgid: &PkgId) -> HashSet<PartId> {
        self.world_entries
            .iter()
            .filter(|entry| entry.matches(pkgid))
            .cloned()
            .collect()
    }
}
