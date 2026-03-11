//! File path to a local [Bpt] or [Bbuild] file.

use crate::{error::*, file::*, io::*, location::*};
use camino::{Utf8Path, Utf8PathBuf};
use std::fs::File;

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PkgPath(Utf8PathBuf);

impl std::fmt::Display for PkgPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl PkgPath {
    pub fn open(
        &self,
        pubkeys: &PublicKeys,
        dir: Option<&Utf8Path>,
        query_credentials: Option<&QueryCredentials>,
    ) -> Result<Pkg, Err> {
        let mut file = File::open_ro(self.as_path())?;

        // If `dir` is populated, ensure file is available to be linked into that directory.
        let file = match dir {
            None => file,
            Some(dir) => file.clone_anon_into(dir)?,
        };

        Pkg::from_file(file, pubkeys, query_credentials, self.as_str())?
            .ok_or_else(|| Err::InvalidPkgPath(self.to_string()))
    }

    pub fn from_str(s: &str) -> Result<Self, Err> {
        Ok(Self::from_path(Utf8Path::new(s)))
    }

    pub fn from_path(path: &Utf8Path) -> Self {
        Self(path.to_owned())
    }

    pub fn as_path(&self) -> &Utf8Path {
        &self.0
    }
}

impl std::ops::Deref for PkgPath {
    type Target = Utf8Path;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
