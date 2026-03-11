//! A [PkgIdx] or [FileIdx] file.
use crate::{error::*, file::*, marshalling::*, metadata::*};
use camino::Utf8Path;
use std::fs::File;

pub enum Idx {
    PkgIdx(PkgIdx),
    FileIdx(FileIdx),
}

impl Idx {
    pub fn from_file(mut file: File, pubkeys: &PublicKeys) -> Result<Option<Self>, AnonLocErr> {
        if file.verify_magic::<PkgIdx>().is_ok() {
            PkgIdx::from_file(file, pubkeys).map(Idx::PkgIdx).map(Some)
        } else if file.verify_magic::<FileIdx>().is_ok() {
            FileIdx::from_file(file, pubkeys)
                .map(Idx::FileIdx)
                .map(Some)
        } else {
            Ok(None)
        }
    }

    pub fn timestamp(&self) -> &Timestamp {
        match self {
            Idx::PkgIdx(pkgidx) => pkgidx.timestamp(),
            Idx::FileIdx(fileidx) => fileidx.timestamp(),
        }
    }

    pub fn link(&self, path: &Utf8Path) -> Result<(), Err> {
        match self {
            Idx::PkgIdx(pkgidx) => pkgidx.link(path),
            Idx::FileIdx(fileidx) => fileidx.link(path),
        }
    }
}
