use crate::{constant::*, error::*, file::*, io::*, location::*, metadata::*};
use camino::{Utf8Path, Utf8PathBuf};
use std::{collections::HashMap, fs::File, io::ErrorKind};

/// Mapping of package ids to the files their packages contain across all known [FileIdx]s.
pub struct PkgFiles {
    pkgs: HashMap<PkgId, Vec<Utf8PathBuf>>,
    // Held for RAII lock guarding
    // Directory may not be available, in which case we have nothing to lock
    _lock: Option<File>,
}

impl PkgFiles {
    pub fn from_root_path(root: &RootDir, pubkeys: &PublicKeys) -> Result<Self, Err> {
        let dir = root.as_path().join(FILEIDX_DIR_PATH);
        let _lock = match dir.lock_ro("available file information") {
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

        let mut pkgs = HashMap::new();
        let paths = match root.as_path().join(FILEIDX_DIR_PATH).readdir() {
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
            let file = File::open_ro(&path)?;
            let new_pkgs = FileIdx::from_file(file, pubkeys).loc(&path)?.into_pkgs();
            pkgs.extend(new_pkgs);
        }

        Ok(Self { pkgs, _lock })
    }

    pub fn pkgid_paths(&self) -> impl Iterator<Item = (&PkgId, Vec<&Utf8Path>)> + '_ {
        self.pkgs
            .iter()
            .map(|(pkgid, paths)| (pkgid, paths.iter().map(|p| p.as_path()).collect()))
    }
}
