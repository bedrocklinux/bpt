//! File path to a local [PkgIdx] or [FileIdx] file.

use crate::io::FileAux;
use crate::{color::*, error::*, file::*, location::*, make_display_color};
use camino::{Utf8Path, Utf8PathBuf};
use std::fs::File;

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct IdxPath(Utf8PathBuf);

make_display_color!(IdxPath, |s, f| {
    write!(f, "{}{}{}", Color::File, s.0, Color::Default)
});

impl std::fmt::Display for IdxPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl IdxPath {
    pub fn open(&self, pubkeys: &PublicKeys, dir: Option<&Utf8Path>) -> Result<Idx, Err> {
        let mut file = File::open_ro(self.as_path())?;

        // If `dir` is populated, ensure file is available to be linked into that directory.
        let file = match dir {
            None => file,
            Some(dir) => file.clone_anon_into(dir)?,
        };

        Idx::from_file(file, pubkeys)
            .loc(self)?
            .ok_or_else(|| Err::InvalidIdxPath(self.to_string()))
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

impl std::ops::Deref for IdxPath {
    type Target = Utf8Path;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
