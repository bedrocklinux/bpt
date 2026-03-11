use crate::{constant::*, error::*, file::*, io::*, location::RootDir, metadata::*};
use camino::Utf8Path;
use std::{collections::HashMap, fs::File, io::ErrorKind};

/// Installed packages
pub struct InstalledPkgs {
    pkgs: HashMap<PkgId, InstPkg>,
    // Held for RAII lock guarding
    // Directory may not be available, in which case we have nothing to lock
    _lock: Option<File>,
}

impl InstalledPkgs {
    pub fn from_root_path_ro(root: &RootDir) -> Result<Self, Err> {
        Self::new(root, false)
    }

    pub fn from_root_path_rw(root: &RootDir) -> Result<Self, Err> {
        Self::new(root, true)
    }

    fn new(root: &RootDir, writable: bool) -> Result<Self, Err> {
        let dir = root.as_path().join(INSTPKG_DIR_PATH);
        let _lock = match if writable {
            dir.lock_rw("installed package information")
        } else {
            dir.lock_ro("installed package information")
        } {
            Ok(lock) => Some(lock),
            Err(Err::Open(_, e)) if e.kind() == ErrorKind::NotFound => {
                // If directory doesn't exist, treat as empty.
                return Ok(Self {
                    pkgs: HashMap::new(),
                    _lock: None,
                });
            }
            Err(e) => return Err(e),
        };

        let paths = match dir.readdir() {
            Ok(paths) => paths,
            Err(Err::ReadDir(_, e)) if e.kind() == ErrorKind::NotFound => {
                // If directory doesn't exist, treat as empty
                return Ok(Self {
                    pkgs: HashMap::new(),
                    _lock,
                });
            }
            Err(e) => return Err(e),
        };

        let mut pkgs = HashMap::new();
        for path in paths {
            if path.file_name() == Some(LOCK_FILE_NAME) {
                continue;
            }
            let instpkg = InstPkg::from_path(path)?;
            pkgs.insert(instpkg.pkgid().clone(), instpkg);
        }

        Ok(Self { pkgs, _lock })
    }

    pub fn get(&self, pkgid: &PkgId) -> Option<&InstPkg> {
        self.pkgs.get(pkgid)
    }

    pub fn pkgids(&self) -> impl Iterator<Item = &PkgId> + '_ {
        self.pkgs.keys()
    }

    pub fn pkgid_pkgdesc(&self) -> impl Iterator<Item = (&PkgId, &PkgDesc)> + '_ {
        self.pkgs
            .iter()
            .map(|(pkgid, instpkg)| (pkgid, &instpkg.pkginfo().pkgdesc))
    }

    pub fn pkgid_paths(&self) -> impl Iterator<Item = (&PkgId, Vec<&Utf8Path>)> + '_ {
        self.pkgs
            .iter()
            .map(|(pkgid, instpkg)| (pkgid, instpkg.paths().collect()))
    }

    pub fn as_map(&self) -> &HashMap<PkgId, InstPkg> {
        &self.pkgs
    }
    pub fn best_match(&self, partid: &PartId, default_archs: &[Arch]) -> Option<&InstPkg> {
        self.pkgs
            .iter()
            .filter(|(_, instpkg)| partid.matches(instpkg.pkgid()))
            .select_best_pkgid(default_archs)
    }

    pub fn best_provider(&self, depend: &Depend, default_archs: &[Arch]) -> Option<&InstPkg> {
        self.pkgs
            .iter()
            .filter(|(_, instpkg)| depend.provided_by(instpkg.pkgid()))
            .select_best_pkgid(default_archs)
    }
}
