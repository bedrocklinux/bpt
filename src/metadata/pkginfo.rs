use crate::{error::*, make_display_color, marshalling::*, metadata::*};
use std::collections::HashMap;

/// Common set of package metadata
///
/// This is directly contains a [PkgId] instead of separate [PkgName], [PkgVer], and [Arch] fields
/// so we can cheaply return a reference to a [PkgId] without transmute hacks.
#[derive(Clone, Debug)]
pub struct PkgInfo {
    pub pkgid: PkgId,
    pub pkgdesc: PkgDesc,
    pub homepage: Homepage,
    pub license: License,
    pub backup: Backup,
    pub depends: Depends,
    // Only populated in bbuilds
    pub makearchs: MakeArchs,
    pub makebins: MakeBins,
    pub makedepends: MakeDepends,
    // Only populated in pkgidx
    pub repopath: RepoPath,
}

make_display_color!(PkgInfo, |s, f| {
    use crate::color::Color::*;
    writeln!(
        f,
        "{Field}Name{Glue}:{Default}         {}",
        s.pkgid.pkgname.color()
    )?;
    writeln!(
        f,
        "{Field}Version{Glue}:{Default}      {}",
        s.pkgid.pkgver.color()
    )?;
    writeln!(
        f,
        "{Field}Architecture{Glue}:{Default} {}",
        s.pkgid.arch.color()
    )?;
    writeln!(
        f,
        "{Field}Description{Glue}:{Default}  {}",
        s.pkgdesc.color()
    )?;
    writeln!(
        f,
        "{Field}Homepage{Glue}:{Default}     {}",
        s.homepage.color()
    )?;
    writeln!(
        f,
        "{Field}License{Glue}:{Default}      {}",
        s.license.color()
    )?;
    writeln!(
        f,
        "{Field}Backup{Glue}:{Default}       {}",
        s.backup.color()
    )?;
    writeln!(
        f,
        "{Field}Depends{Glue}:{Default}      {}",
        s.depends.color()
    )?;
    if s.pkgid.arch == crate::metadata::Arch::bbuild {
        writeln!(
            f,
            "{Field}MakeBins{Glue}:{Default}     {}",
            s.makebins.color()
        )?;
        writeln!(
            f,
            "{Field}MakeDepends{Glue}:{Default}  {}",
            s.makedepends.color()
        )?;
        writeln!(
            f,
            "{Field}MakeArchs{Glue}:{Default}    {}",
            s.makearchs.color()
        )?;
    }
    if !s.repopath.is_empty() {
        writeln!(
            f,
            "{Field}RepoPath{Glue}:{Default}     {}",
            s.repopath.color()
        )?;
    }
    Ok(())
});

impl PkgInfo {
    pub fn pkgid(&self) -> &PkgId {
        &self.pkgid
    }

    pub fn select_make_arch(&self, archs: &[Arch]) -> Result<Arch, Err> {
        if self.pkgid.arch != Arch::bbuild {
            return Ok(self.pkgid.arch);
        }

        archs
            .iter()
            .find(|&&arch| self.makearchs.can_build(arch))
            .copied()
            .ok_or_else(|| Err::NoDefaultArchForBbuild(self.pkgid().clone()))
    }
}

impl<W: std::io::Write> Serialize<W> for PkgInfo {
    fn serialize(&self, w: &mut W) -> Result<(), AnonLocErr> {
        let PkgInfo {
            pkgid:
                PkgId {
                    pkgname,
                    pkgver,
                    arch,
                },
            pkgdesc,
            homepage,
            license,
            backup,
            depends,
            makearchs,
            makebins,
            makedepends,
            repopath,
        } = self;

        pkgname.serialize(w)?;
        pkgver.serialize(w)?;
        arch.serialize(w)?;
        pkgdesc.serialize(w)?;
        homepage.serialize(w)?;
        license.serialize(w)?;
        backup.serialize(w)?;
        depends.serialize(w)?;
        makearchs.serialize(w)?;
        makebins.serialize(w)?;
        makedepends.serialize(w)?;
        repopath.serialize(w)?;

        Ok(())
    }
}

macro_rules! extract {
    ($iter:expr, $field:ty) => {
        <$field>::deserialize(
            $iter
                .remove(&<$field>::KEY as &u8)
                // Missing fields are treated as empty fields
                .unwrap_or(&[<$field>::KEY as u8]),
        )
    };
}

impl Deserialize for PkgInfo {
    /// Considered a block.  Call on [crate::marshalling::BlockIter] output.
    fn deserialize(bytes: &[u8]) -> Result<Self, AnonLocErr> {
        let mut fields: HashMap<u8, &[u8]> = HashMap::new();
        for field in bytes.split(|&b| b == b'\0').filter(|f| !f.is_empty()) {
            let Some(key) = field.iter().next() else {
                continue;
            };
            if fields.insert(*key, field).is_some() {
                return Err(AnonLocErr::FieldDuplicated((*key as char).to_string()));
            }
        }

        let pkginfo = Self {
            pkgid: PkgId {
                pkgname: extract!(fields, PkgName)?,
                pkgver: extract!(fields, PkgVer)?,
                arch: extract!(fields, Arch)?,
            },
            pkgdesc: extract!(fields, PkgDesc)?,
            homepage: extract!(fields, Homepage)?,
            license: extract!(fields, License)?,
            backup: extract!(fields, Backup)?,
            depends: extract!(fields, Depends)?,
            makearchs: extract!(fields, MakeArchs)?,
            makebins: extract!(fields, MakeBins)?,
            makedepends: extract!(fields, MakeDepends)?,
            repopath: extract!(fields, RepoPath)?,
        };

        if !fields.is_empty() {
            return Err(AnonLocErr::UnexpectedData);
        }

        Ok(pkginfo)
    }
}
