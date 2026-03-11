//! Installed-package reconciler.
//!
//! Shared backend for install/remove/upgrade/downgrade planning and execution.

use crate::collection::{AvailableBpts, Cache, InstalledPkgs, RepositoryPkgs};
use crate::error::Err;
use crate::file::{Bbuild, Bpt, BptConfGeneral, BuildArgs, PublicKeys, World};
use crate::io::{NetUtil, QueryCredentials};
use crate::location::{PkgPathUrlRepo, RootDir};
use crate::marshalling::FieldList;
use crate::metadata::{Arch, Depend, PartId, PkgId, PkgInfo, PkgName};
use camino::Utf8Path;
use std::{cell::RefCell, collections::HashSet};

mod apply;
mod display;
mod planner;
mod resolve;
#[cfg(test)]
mod tests;

pub struct InstPkgReconciler<'a> {
    pub world: &'a World,
    pub installed: &'a InstalledPkgs,
    pub repository: &'a RepositoryPkgs,
    pub pubkeys: &'a PublicKeys,
    pub netutil: &'a RefCell<NetUtil<'a>>,
    pub pkgcache: &'a RefCell<Cache>,
    pub general: &'a BptConfGeneral,
    pub query_credentials: &'a QueryCredentials<'a>,
    pub command: CommandRequest<'a>,
}

#[derive(Default)]
pub enum CommandRequest<'a> {
    #[default]
    None,
    Install {
        pkgs: &'a [PkgPathUrlRepo],
        reinstall: bool,
    },
    Remove {
        pkgs: &'a [PartId],
    },
    Upgrade {
        pkgs: &'a [PkgPathUrlRepo],
    },
    Downgrade {
        pkgs: &'a [PkgPathUrlRepo],
    },
}

pub struct InstPkgApplyArgs<'a> {
    pub root: &'a RootDir,
    pub installed: &'a InstalledPkgs,
    pub world: &'a mut World,
    pub instpkg_dir: &'a Utf8Path,
    pub purge: bool,
    pub forget: bool,
    pub repository: &'a RepositoryPkgs,
    pub pubkeys: &'a PublicKeys,
    pub netutil: &'a RefCell<NetUtil<'a>>,
    pub pkgcache: &'a RefCell<Cache>,
    pub available_bpts: &'a RefCell<AvailableBpts>,
    pub buildargs: Option<&'a BuildArgs<'a>>,
}

#[derive(Default)]
pub struct InstPkgPlan {
    retain: Vec<RetainOp>,
    remove: Vec<RemoveOp>,
    install: Vec<InstallOp>,
    upgrade: Vec<ReplaceOp>,
    downgrade: Vec<ReplaceOp>,
    world_entries: HashSet<PartId>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum PlanVerb {
    Install,
    Remove,
    Upgrade,
    Downgrade,
    Retain,
}

struct RetainOp {
    pkgid: PkgId,
    world_change: Option<WorldChange>,
}

struct RemoveOp {
    pkgid: PkgId,
    world_change: Option<WorldChange>,
}

struct InstallOp {
    source: TargetSource,
    world_change: Option<WorldChange>,
}

struct ReplaceOp {
    from: PkgId,
    to: TargetSource,
    world_change: Option<WorldChange>,
}

struct PreparedInstallOp {
    pkgid: PkgId,
}

struct PreparedReplaceOp {
    from: PkgId,
    to: PkgId,
}

#[derive(Clone, PartialEq, Eq, Hash)]
struct NameArch {
    pkgname: PkgName,
    arch: Arch,
}

#[derive(Default)]
struct TargetPkgState {
    explicit_world_entries: HashSet<PartId>,
    source: Option<TargetSource>,
}

enum TargetSource {
    Installed(PkgInfo),
    Bpt(Bpt),
    Bbuild { bbuild: Bbuild, pkgid: PkgId },
}

#[derive(Clone)]
enum WorldChange {
    Add(PartId),
    Remove(Vec<PartId>),
    Replace { from: Vec<PartId>, to: PartId },
}

struct ResolvedCliPkg {
    source: TargetSource,
    world_entry: PartId,
}

enum ResolveMode {
    Install { reinstall: bool },
    Upgrade,
    Downgrade,
}

impl NameArch {
    fn from(pkgid: &PkgId) -> Self {
        Self {
            pkgname: pkgid.pkgname.clone(),
            arch: pkgid.arch,
        }
    }
}

impl TargetSource {
    fn pkgid(&self) -> &PkgId {
        match self {
            Self::Installed(pkginfo) => pkginfo.pkgid(),
            Self::Bpt(bpt) => bpt.pkgid(),
            Self::Bbuild { pkgid, .. } => pkgid,
        }
    }

    fn is_installed(&self) -> bool {
        matches!(self, Self::Installed(_))
    }

    fn needs_build(&self) -> bool {
        matches!(self, Self::Bbuild { .. })
    }

    fn depends(&self) -> Result<Vec<Depend>, Err> {
        match self {
            Self::Installed(pkginfo) => Self::validate_binary_depends(pkginfo),
            Self::Bpt(bpt) => Self::validate_binary_depends(bpt.pkginfo()),
            Self::Bbuild { bbuild, pkgid } => Ok(bbuild
                .pkginfo()
                .depends
                .populate_depends_arch_if_missing(pkgid.arch)
                .iter()
                .cloned()
                .collect()),
        }
    }

    fn validate_binary_depends(pkginfo: &PkgInfo) -> Result<Vec<Depend>, Err> {
        let mut depends = Vec::new();
        for depend in pkginfo.depends.iter() {
            if depend.arch.is_none() {
                return Err(Err::DependArchMissing(
                    Box::new(depend.clone()),
                    pkginfo.pkgid().clone(),
                ));
            }
            depends.push(depend.clone());
        }
        Ok(depends)
    }
}
