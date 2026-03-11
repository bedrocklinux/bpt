use crate::{constant::*, error::*, file::*, io::*, marshalling::*, metadata::*, str::*};
use camino::{Utf8Path, Utf8PathBuf};
use std::{
    collections::HashMap,
    fs::File,
    io::{Read, Write},
};

/// Mapping of packages to the files they provide
pub struct FileIdx {
    timestamp: Timestamp,
    pkgs: HashMap<PkgId, Vec<Utf8PathBuf>>,
    file: File,
}

impl MagicNumber for FileIdx {
    const DESCRIPTION: &'static str = "bpt file index";
    const MAGIC: &'static [u8] = FILEIDX_MAGIC;
}

macro_rules! extract {
    ($fields:ident, $ty:ty) => {{
        let buf = $fields
            .next()
            .ok_or_else(|| AnonLocErr::FieldMissing(stringify!($ty)))?;
        <$ty>::deserialize(buf)
    }};
}

impl FileIdx {
    pub fn from_file(file: File, pubkeys: &PublicKeys) -> Result<Self, AnonLocErr> {
        let (mut file, timestamp) = file
            .verify_and_strip_sig(pubkeys)?
            .verify_and_strip_magic::<Self>()?
            .strip_timestamp()?;

        let mut buf = Vec::new();
        CompressionDecoder::new(&mut file)?
            .read_to_end(&mut buf)
            .map_err(AnonLocErr::Read)?;

        // After the magic and timestamp (stripped above), FileIdx is a series of blocks starting
        // with a PkgId followed by the package's file paths.
        let mut pkgs = HashMap::new();
        for block in buf.as_block_iter() {
            let mut fields = block.split(|&x| x == b'\0');

            let pkgname = extract!(fields, PkgName)?;
            let pkgver = extract!(fields, PkgVer)?;
            let arch = extract!(fields, Arch)?;
            let pkgid = PkgId::new(pkgname, pkgver, arch);

            let paths = fields
                .filter(|f| !f.is_empty())
                .map(|path| {
                    path.into_pathbuf().map_err(|_| {
                        AnonLocErr::FieldIllegalChar("filepath", "non-utf8".to_string())
                    })
                })
                .collect::<Result<Vec<_>, _>>()?;
            if pkgs.insert(pkgid.clone(), paths).is_some() {
                return Err(AnonLocErr::FieldDuplicated(pkgid.to_string()));
            }
        }

        let file = file.into_inner();

        Ok(Self {
            timestamp,
            pkgs,
            file,
        })
    }

    pub fn from_bpts(bpts: &[Bpt], out_dir: &Utf8Path, privkey: &PrivKey) -> Result<Self, Err> {
        let mut file = File::create_anon(out_dir)?;

        || -> Result<Self, AnonLocErr> {
            file.write_all(Self::MAGIC).map_err(AnonLocErr::Write)?;

            Timestamp::now()
                .map_err(AnonLocErr::GetTime)?
                .serialize(&mut file)?;

            let mut file = CompressionEncoder::new(file)?;
            for bpt in bpts {
                bpt.pkgid().serialize(&mut file)?;

                let paths = bpt.filepaths();
                for path in paths {
                    file.write_all(path.as_str().as_bytes())
                        .map_err(AnonLocErr::Write)?;
                    file.write_all(b"\0").map_err(AnonLocErr::Write)?;
                }

                // blocks are separated by "empty" field only containing null field separator.
                file.write_all(b"\0").map_err(AnonLocErr::Write)?;
            }
            let mut file = file.finish()?;
            file.sign(privkey)?;

            Self::from_file(file, &PublicKeys::from_skipping_verification())
        }()
        .loc(out_dir.join("<anon-fileidx>"))
    }

    pub fn link(&self, path: &Utf8Path) -> Result<(), Err> {
        self.file.link(path)
    }

    pub fn timestamp(&self) -> &Timestamp {
        &self.timestamp
    }

    pub fn into_pkgs(self) -> HashMap<PkgId, Vec<Utf8PathBuf>> {
        self.pkgs
    }

    pub fn pkgids(&self) -> impl Iterator<Item = &PkgId> + '_ {
        self.pkgs.keys()
    }
}
