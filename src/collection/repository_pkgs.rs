use crate::{
    constant::*, error::*, file::*, io::*, location::RootDir, marshalling::*, metadata::*, str::*,
};
use std::{collections::HashMap, fs::File, io::ErrorKind};

/// Available packages in the known repositories.
pub struct RepositoryPkgs {
    // Packages in the repository, including both prebuilt .bpts and could-be-built .bbuilds
    pkgs: HashMap<PkgId, PkgInfo>,
    // Binary packages we could get, either directly or by building a bbuild.
    //
    // PkgId's arch will also be a binary format (i.e. not bbuild)
    //
    // PkgInfo's arch will indicate the repository's arch; may be bbuild indicating you have to
    // build it to get PkgId.
    buildable: HashMap<PkgId, PkgInfo>,
    // Held for RAII lock guarding
    // Directory may not be available, in which case we have nothing to lock
    _lock: Option<File>,
}

impl RepositoryPkgs {
    pub fn from_root_path(root: &RootDir, pubkeys: &PublicKeys) -> Result<Self, Err> {
        let dir = root.as_path().join(PKGIDX_DIR_PATH);
        let _lock = match dir.lock_ro("repository package information") {
            Ok(lock) => Some(lock),
            Err(Err::Open(_, e)) if e.kind() == ErrorKind::NotFound => {
                // If directory doesn't exist, treat as empty.
                return Ok(Self {
                    pkgs: HashMap::new(),
                    buildable: HashMap::new(),
                    _lock: None,
                });
            }
            Err(e) => return Err(e),
        };

        let mut pkgs = HashMap::new();
        let paths = match dir.readdir() {
            Ok(paths) => paths,
            Err(Err::ReadDir(_, e)) if e.kind() == ErrorKind::NotFound => {
                // If directory doesn't exist, treat as empty
                vec![].into_iter()
            }
            Err(e) => return Err(e),
        };

        for path in paths {
            if path.file_name() == Some(LOCK_FILE_NAME) {
                continue;
            }

            // Within a pkgidx, package locations may be represented relative to the pkgidx such
            // that they're relocatable with it.  bpt stores the pkgidx remote location as the
            // (underscore-encoded) filename.
            let pkgidx_path = path
                .file_name()
                .ok_or_else(|| Err::PathLacksFileName(path.to_string()))?
                .underscore_decode()?;
            // We have the pkgidx's full URL/path.  We want its containing directory.
            let pkgidx_dir = FieldStr::try_from(pkgidx_path.as_ref().strip_filename())
                .field("pkgidx_dir")
                .loc(&path)?;

            let file = File::open_ro(&path)?;
            let new_pkgs = PkgIdx::from_file(file, pubkeys)
                .loc(&path)?
                .into_pkgs()
                .into_iter()
                .map(|mut pkginfo| {
                    pkginfo.repopath.absolutize(&pkgidx_dir);
                    (pkginfo.pkgid().clone(), pkginfo)
                });
            pkgs.extend(new_pkgs);
        }

        let mut buildable = HashMap::new();
        for (pkgid, pkginfo) in pkgs.iter() {
            if pkgid.arch != Arch::bbuild {
                // If we have both pre-built and buildable, favor pre-built.
                // Pre-built here, and so clobber.
                buildable.insert(pkgid.clone(), pkginfo.clone());
                continue;
            }
            for arch in pkginfo.makearchs.iter() {
                // If we have both pre-built and buildable, favor pre-built.
                // Build-able here, and so favor preexisting.
                let new = pkgid.with_arch(*arch);
                buildable.entry(new).or_insert(pkginfo.clone());
                if *arch == Arch::host() {
                    // if we can build local, we can build native
                    let new = pkgid.with_arch(Arch::native);
                    buildable.entry(new).or_insert(pkginfo.clone());
                }
            }
        }

        Ok(Self {
            pkgs,
            buildable,
            _lock,
        })
    }

    pub fn pkgids(&self) -> impl Iterator<Item = &PkgId> + '_ {
        self.pkgs.keys()
    }

    pub fn pkgid_pkgdesc(&self) -> impl Iterator<Item = (&PkgId, &PkgDesc)> + '_ {
        self.pkgs
            .iter()
            .map(|(pkgid, pkginfo)| (pkgid, &pkginfo.pkgdesc))
    }

    // Find the entry that best matches a given [PartId].
    //
    // Returns [PkgInfo] to either a [bpt] or [bbuild].
    pub fn best_pkg_match(&self, partid: &PartId, default_archs: &[Arch]) -> Option<&PkgInfo> {
        self.pkgs
            .iter()
            .filter(|(pkgid, _)| partid.matches(pkgid))
            .select_best_pkgid(default_archs)
    }

    // Find the entry that best matches or builds into something which matches a given [PartId].
    //
    // Returns [PkgInfo] to a [bpt], a [bbuild] file, or a [bbuild] that would build into the
    // requested [bpt].
    pub fn best_buildable_match(
        &self,
        partid: &PartId,
        default_archs: &[Arch],
    ) -> Option<&PkgInfo> {
        self.buildable
            .iter()
            .filter(|(pkgid, _)| partid.matches(pkgid))
            .select_best_pkgid(default_archs)
    }

    // Find the entry that best provides or builds into something which provides a given [Depend].
    //
    // Returns the [PkgInfo] of the [bpt], [bbuild], or a [bbuild] that could be built into
    // something which provides the given [Depend].
    pub fn best_buildable_provider(
        &self,
        depend: &Depend,
        default_archs: &[Arch],
    ) -> Option<&PkgInfo> {
        self.buildable
            .iter()
            .filter(|(pkgid, _)| depend.provided_by(pkgid))
            .select_best_pkgid(default_archs)
    }
}
