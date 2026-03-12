//! File path to a local [Bpt] file.

use crate::{error::*, file::*, io::*, marshalling::VerifyMagic};
use camino::{Utf8Path, Utf8PathBuf};
use std::fs::File;
use std::io::{Seek, SeekFrom};

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BptPath(Utf8PathBuf);

impl std::fmt::Display for BptPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl BptPath {
    pub fn open(&self, pubkeys: &PublicKeys, dir: Option<&Utf8Path>) -> Result<Bpt, Err> {
        let mut file = File::open_ro(self.as_path())?;
        if file.verify_magic::<Bpt>().is_err() {
            return Err(Err::InvalidBptPath(self.to_string()));
        }
        file.seek(SeekFrom::Start(0))
            .map_err(|e| Err::Seek(self.to_string(), e))?;

        let file = match dir {
            None => file,
            Some(dir) => file.clone_anon_into(dir)?,
        };

        Bpt::from_file(file, pubkeys).loc(self)
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

impl std::ops::Deref for BptPath {
    type Target = Utf8Path;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::file::PublicKeys;

    #[test]
    fn rejects_bbuild_paths() {
        let path = BptPath::from_path(Utf8Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/bbuilds/fakeblock@1.0.0.bbuild"
        )));

        match path.open(&PublicKeys::from_skipping_verification(), None) {
            Err(Err::InvalidBptPath(_)) => {}
            Err(err) => panic!("expected InvalidBptPath, got {err}"),
            Ok(_) => panic!("expected bbuild path to be rejected"),
        }
    }
}
