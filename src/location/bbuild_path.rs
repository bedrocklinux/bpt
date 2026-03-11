//! File path to a local [Bbuild] file.

use crate::{error::*, file::*, io::*};
use camino::{Utf8Path, Utf8PathBuf};
use std::fs::File;

#[derive(Clone)]
pub struct BbuildPath(Utf8PathBuf);

impl BbuildPath {
    pub fn from_str(s: &str) -> Result<Self, Err> {
        Ok(Self::from_path(Utf8Path::new(s)))
    }

    pub fn from_path(path: &Utf8Path) -> Self {
        Self(path.to_owned())
    }
    pub fn open(
        &self,
        pubkeys: &PublicKeys,
        query_credentials: Option<&ProcessCredentials>,
    ) -> Result<Bbuild, Err> {
        let file = File::open_ro(self)?;
        Bbuild::from_file(file, pubkeys, query_credentials).loc(self.to_string())
    }
}

impl std::ops::Deref for BbuildPath {
    type Target = Utf8Path;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::fmt::Display for BbuildPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0.as_str())
    }
}
