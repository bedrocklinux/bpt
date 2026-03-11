use crate::{constant::*, error::*, file::*, io::*, marshalling::*, metadata::*};
use camino::Utf8Path;
use std::{
    fs::File,
    io::{Read, Write},
};

/// List of available binary packages and/or package build definitions within a repository
pub struct PkgIdx {
    timestamp: Timestamp,
    pkgs: Vec<PkgInfo>,
    file: File,
}

impl MagicNumber for PkgIdx {
    const DESCRIPTION: &'static str = "bpt package index";
    const MAGIC: &'static [u8] = PKGIDX_MAGIC;
}

impl PkgIdx {
    pub fn from_file(file: File, pubkeys: &PublicKeys) -> Result<Self, AnonLocErr> {
        let (mut file, timestamp) = file
            .verify_and_strip_sig(pubkeys)?
            .verify_and_strip_magic::<Self>()?
            .strip_timestamp()?;

        let mut buf = Vec::new();
        CompressionDecoder::new(&mut file)?
            .read_to_end(&mut buf)
            .map_err(AnonLocErr::Read)?;

        // After the magic and timestamp (stripped above), PkgIdx is a series of PkgInfo blocks.
        let pkgs = buf
            .as_block_iter()
            .map(PkgInfo::deserialize)
            .collect::<Result<Vec<_>, _>>()?;

        let file = file.into_inner();

        Ok(Self {
            timestamp,
            pkgs,
            file,
        })
    }

    pub fn from_pkginfos(
        pkginfos: &[PkgInfo],
        out_dir: &Utf8Path,
        privkey: &PrivKey,
    ) -> Result<Self, Err> {
        let mut file = File::create_anon(out_dir)?;

        || -> Result<Self, AnonLocErr> {
            file.write_all(Self::MAGIC).map_err(AnonLocErr::Write)?;

            Timestamp::now()
                .map_err(AnonLocErr::GetTime)?
                .serialize(&mut file)?;

            let mut file = CompressionEncoder::new(file)?;
            for pkginfo in pkginfos {
                pkginfo.serialize(&mut file)?;
                // blocks are separated by an "empty" field only containing null field separator.
                file.write_all(b"\0").map_err(AnonLocErr::Write)?;
            }
            let mut file = file.finish()?;
            file.sign(privkey)?;

            Self::from_file(file, &PublicKeys::from_skipping_verification())
        }()
        .loc(out_dir.join("<anon-pkgidx>"))
    }

    pub fn link(&self, path: &Utf8Path) -> Result<(), Err> {
        self.file.link(path)
    }

    pub fn timestamp(&self) -> &Timestamp {
        &self.timestamp
    }

    pub fn into_pkgs(self) -> Vec<PkgInfo> {
        self.pkgs
    }

    pub fn pkgids(&self) -> impl Iterator<Item = &PkgId> + '_ {
        self.pkgs.iter().map(|info| info.pkgid())
    }
}
