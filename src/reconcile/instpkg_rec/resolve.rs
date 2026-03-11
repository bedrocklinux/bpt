//! Package source resolution for installed-package reconciliation.

use crate::{
    error::Err,
    file::Bbuild,
    location::{Pkg, PkgPathUrlRepo},
    marshalling::FieldList,
    metadata::{Arch, Depend, PartId, PkgInfo, PkgName, PkgVer},
    reconcile::instpkg_rec::{InstPkgReconciler, ResolveMode, ResolvedCliPkg, TargetSource},
};

impl<'a> InstPkgReconciler<'a> {
    pub(super) fn resolve_cli_pkg(
        &self,
        pkg: &PkgPathUrlRepo,
        mode: ResolveMode,
    ) -> Result<ResolvedCliPkg, Err> {
        match pkg {
            PkgPathUrlRepo::Path(path) => self.resolve_opened_pkg(
                path.open(self.pubkeys, None, Some(self.query_credentials))?,
                mode,
            ),
            PkgPathUrlRepo::Url(url) => {
                let mut pkgcache = self.pkgcache.borrow_mut();
                let netutil = self.netutil.borrow();
                let pkg = url.download(
                    &netutil,
                    &mut pkgcache,
                    self.pubkeys,
                    None,
                    Some(self.query_credentials),
                )?;
                drop(netutil);
                drop(pkgcache);
                self.resolve_opened_pkg(pkg, mode)
            }
            PkgPathUrlRepo::Repo(partid) => self.resolve_repo_cli_pkg(partid, mode),
        }
    }

    fn resolve_opened_pkg(&self, pkg: Pkg, mode: ResolveMode) -> Result<ResolvedCliPkg, Err> {
        match pkg {
            Pkg::Bpt(bpt) => {
                let world_entry = self.world_entry_from_loaded_pkg(bpt.pkgid());
                Ok(ResolvedCliPkg {
                    source: TargetSource::Bpt(bpt),
                    world_entry,
                })
            }
            Pkg::Bbuild(bbuild) => {
                let preferred =
                    self.preferred_installed_arch_for_pkgname(&bbuild.pkgid().pkgname, &mode);
                let arch = self.select_bbuild_arch(&bbuild, preferred)?;
                let pkgid = bbuild.pkgid().with_arch(arch);
                let world_entry = self.world_entry_from_loaded_pkg(&pkgid);
                Ok(ResolvedCliPkg {
                    source: TargetSource::Bbuild { bbuild, pkgid },
                    world_entry,
                })
            }
        }
    }

    fn resolve_repo_cli_pkg(
        &self,
        partid: &PartId,
        mode: ResolveMode,
    ) -> Result<ResolvedCliPkg, Err> {
        let source = match mode {
            ResolveMode::Install { reinstall: false } => {
                if let Some(instpkg) = self
                    .installed
                    .best_match(partid, &self.general.default_archs)
                {
                    TargetSource::Installed(instpkg.pkginfo().clone())
                } else {
                    self.resolve_repo_partid_available(partid, &self.general.default_archs)?
                }
            }
            ResolveMode::Install { reinstall: true } | ResolveMode::Upgrade => {
                let (lookup, archs) = self.repo_lookup_for_upgrade(partid);
                self.resolve_repo_partid_available(&lookup, &archs)?
            }
            ResolveMode::Downgrade => {
                if partid.pkgver.is_none() {
                    return Err(Err::InputFieldInvalid(
                        "downgrade package",
                        format!("missing version in `{partid}`"),
                    ));
                }
                let (lookup, archs) = self.repo_lookup_for_upgrade(partid);
                self.resolve_repo_partid_available(&lookup, &archs)?
            }
        };
        Ok(ResolvedCliPkg {
            world_entry: partid.clone(),
            source,
        })
    }

    fn repo_lookup_for_upgrade(&self, partid: &PartId) -> (PartId, Vec<Arch>) {
        let mut lookup = partid.clone();
        if lookup.arch.is_none()
            && let Some(instpkg) = self
                .installed
                .best_match(partid, &self.general.default_archs)
        {
            lookup.arch = Some(instpkg.pkgid().arch);
            return (lookup, vec![instpkg.pkgid().arch]);
        }
        (lookup, self.general.default_archs.to_vec())
    }

    pub(super) fn resolve_partid_default(&self, partid: &PartId) -> Result<TargetSource, Err> {
        if let Some(instpkg) = self
            .installed
            .best_match(partid, &self.general.default_archs)
        {
            Ok(TargetSource::Installed(instpkg.pkginfo().clone()))
        } else {
            self.resolve_repo_partid_available(partid, &self.general.default_archs)
        }
    }

    pub(super) fn resolve_repo_provider(&self, depend: &Depend) -> Result<TargetSource, Err> {
        let pkginfo = self
            .repository
            .best_buildable_provider(depend, &self.general.default_archs)
            .ok_or_else(|| {
                Err::UnableToLocateDependency(
                    Box::new(depend.clone()),
                    crate::metadata::PkgId::new(
                        depend.pkgname.clone(),
                        PkgVer::try_from("0.0.0").expect("valid synthetic version"),
                        depend.arch.unwrap_or(Arch::host()),
                    ),
                )
            })?;
        self.open_repo_pkginfo(pkginfo, depend.arch)
    }

    pub(super) fn resolve_repo_partid_available(
        &self,
        partid: &PartId,
        archs: &[Arch],
    ) -> Result<TargetSource, Err> {
        let pkginfo = self
            .repository
            .best_buildable_match(partid, archs)
            .ok_or_else(|| Err::UnableToLocateRepositoryPkg(partid.clone()))?;
        self.open_repo_pkginfo(pkginfo, partid.arch)
    }

    fn open_repo_pkginfo(
        &self,
        pkginfo: &PkgInfo,
        requested_arch: Option<Arch>,
    ) -> Result<TargetSource, Err> {
        let path_url = pkginfo.repopath.as_pkg_path_url()?;
        let pkg = {
            let mut pkgcache = self.pkgcache.borrow_mut();
            let netutil = self.netutil.borrow();
            path_url.open(
                &netutil,
                &mut pkgcache,
                self.pubkeys,
                None,
                Some(self.query_credentials),
            )?
        };
        match pkg {
            Pkg::Bpt(bpt) => Ok(TargetSource::Bpt(bpt)),
            Pkg::Bbuild(bbuild) => {
                let arch = self.select_bbuild_arch(&bbuild, requested_arch)?;
                let pkgid = bbuild.pkgid().with_arch(arch);
                Ok(TargetSource::Bbuild { bbuild, pkgid })
            }
        }
    }

    fn preferred_installed_arch_for_pkgname(
        &self,
        pkgname: &PkgName,
        mode: &ResolveMode,
    ) -> Option<Arch> {
        match mode {
            ResolveMode::Install { reinstall: true }
            | ResolveMode::Upgrade
            | ResolveMode::Downgrade => self
                .installed
                .pkgids()
                .find(|pkgid| &pkgid.pkgname == pkgname)
                .map(|pkgid| pkgid.arch),
            ResolveMode::Install { reinstall: false } => None,
        }
    }

    fn select_bbuild_arch(&self, bbuild: &Bbuild, preferred: Option<Arch>) -> Result<Arch, Err> {
        if bbuild
            .pkginfo()
            .makearchs
            .as_slice()
            .contains(&Arch::noarch)
        {
            return Ok(Arch::noarch);
        }
        if let Some(preferred) = preferred
            && bbuild.pkginfo().makearchs.can_build(preferred)
        {
            return Ok(preferred);
        }
        bbuild.select_make_arch(&self.general.default_archs)
    }

    fn world_entry_from_loaded_pkg(&self, pkgid: &crate::metadata::PkgId) -> PartId {
        PartId::new(
            pkgid.pkgname.clone(),
            self.general
                .pin_direct_pkgver
                .then_some(pkgid.pkgver.clone()),
            Some(pkgid.arch),
        )
    }
}
