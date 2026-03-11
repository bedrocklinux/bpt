use crate::{color::*, error::*, make_display_color, metadata::*};

/// A partial package identifier.
///
/// These are intended to be functionally one-to-one with [PkgId] once sane values for missing
/// fields are automatically determined from context.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PartId {
    pub pkgname: PkgName,
    pub pkgver: Option<PkgVer>,
    pub arch: Option<Arch>,
}
make_display_color!(PartId, |s, f| {
    write!(f, "{}{}", Color::PkgName, s.pkgname)?;
    if let Some(pkgver) = &s.pkgver {
        write!(f, "{}@{}", Color::Glue, pkgver.color())?;
    }
    if let Some(arch) = &s.arch {
        write!(f, "{}:{}", Color::Glue, arch.color())?;
    }
    write!(f, "{}", Color::Default)
});

impl std::fmt::Display for PartId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.pkgname)?;
        if let Some(pkgver) = &self.pkgver {
            write!(f, "@{pkgver}")?;
        }
        if let Some(arch) = &self.arch {
            write!(f, ":{arch}")?;
        }
        Ok(())
    }
}

impl PartId {
    pub fn new(pkgname: PkgName, pkgver: Option<PkgVer>, arch: Option<Arch>) -> Self {
        Self {
            pkgname,
            pkgver,
            arch,
        }
    }

    /// Used to translate between bbuild and bpt pkgids.
    pub fn with_arch(&self, arch: Arch) -> Self {
        Self {
            pkgname: self.pkgname.clone(),
            pkgver: self.pkgver.clone(),
            arch: Some(arch),
        }
    }

    /// Returns a PkgId if this PartId is complete.
    pub fn as_pkgid(&self) -> Option<PkgId> {
        Some(PkgId {
            pkgname: self.pkgname.clone(),
            pkgver: self.pkgver.as_ref()?.clone(),
            arch: *self.arch.as_ref()?,
        })
    }

    pub fn matches(&self, other: &PkgId) -> bool {
        self.pkgname == other.pkgname
            && self.pkgver.as_ref().is_none_or(|ver| *ver == other.pkgver)
            && self.arch.as_ref().is_none_or(|arch| *arch == other.arch)
    }
}

impl std::str::FromStr for PartId {
    type Err = Err;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.split_once('@') {
            None => match value.split_once(':') {
                // pkgname
                None => Ok(Self {
                    pkgname: PkgName::from_str(value)?,
                    pkgver: None,
                    arch: None,
                }),
                // pkgname:arch
                Some((pkgname, arch)) => Ok(Self {
                    pkgname: PkgName::from_str(pkgname)?,
                    pkgver: None,
                    arch: Some(Arch::from_str(arch)?),
                }),
            },
            Some((start, end)) => match (start.split_once(':'), end.split_once(':')) {
                // pkgname@ver
                (None, None) => Ok(Self {
                    pkgname: PkgName::from_str(start)?,
                    pkgver: Some(PkgVer::from_str(end)?),
                    arch: None,
                }),
                // pkgname:arch@ver
                (Some((pkgname, arch)), _) => Ok(Self {
                    pkgname: PkgName::from_str(pkgname)?,
                    pkgver: Some(PkgVer::from_str(end)?),
                    arch: Some(Arch::from_str(arch)?),
                }),
                // pkgname@ver:arch
                (_, Some((pkgver, arch))) => Ok(Self {
                    pkgname: PkgName::from_str(start)?,
                    pkgver: Some(PkgVer::from_str(pkgver)?),
                    arch: Some(Arch::from_str(arch)?),
                }),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::*;

    #[test]
    fn just_pkgname() {
        let str = "bpt";
        let pkgid = PartId::from_str(str).unwrap();
        assert_eq!(
            pkgid,
            PartId {
                pkgname: PkgName::try_from("bpt").unwrap(),
                pkgver: None,
                arch: None,
            }
        );
        assert_eq!(format!("{pkgid}"), str);
    }

    #[test]
    fn pkgname_pkgver() {
        let str = "bpt@1.0.0";
        let pkgid = PartId::from_str(str).unwrap();
        assert_eq!(
            pkgid,
            PartId {
                pkgname: PkgName::try_from("bpt").unwrap(),
                pkgver: Some(PkgVer::try_from("1.0.0").unwrap()),
                arch: None,
            }
        );
        assert_eq!(format!("{pkgid}"), str);
    }

    #[test]
    fn pkgname_arch() {
        let str = "bpt:aarch64";
        let pkgid = PartId::from_str(str).unwrap();
        assert_eq!(
            pkgid,
            PartId {
                pkgname: PkgName::try_from("bpt").unwrap(),
                pkgver: None,
                arch: Some(Arch::from_str("aarch64").unwrap()),
            }
        );
        assert_eq!(format!("{pkgid}"), str);
    }

    #[test]
    fn pkgname_pkgver_arch() {
        let str = "bpt@1.0.0:aarch64";
        let pkgid = PartId::from_str(str).unwrap();
        assert_eq!(
            pkgid,
            PartId {
                pkgname: PkgName::try_from("bpt").unwrap(),
                pkgver: Some(PkgVer::try_from("1.0.0").unwrap()),
                arch: Some(Arch::from_str("aarch64").unwrap()),
            }
        );
        assert_eq!(format!("{pkgid}"), str);
    }

    #[test]
    fn pkgname_arch_pkgver() {
        // Convention is version comes before arch.  Be flexible on interpretation, but rework on
        // output.
        let str = "bpt:aarch64@1.0.0";
        let pkgid = PartId::from_str(str).unwrap();
        assert_eq!(
            pkgid,
            PartId {
                pkgname: PkgName::try_from("bpt").unwrap(),
                pkgver: Some(PkgVer::try_from("1.0.0").unwrap()),
                arch: Some(Arch::from_str("aarch64").unwrap()),
            }
        );
        assert_eq!(format!("{pkgid}"), "bpt@1.0.0:aarch64");
    }

    #[test]
    fn bad_arch() {
        let str = "bpt:x86_65";
        let err = format!("{}", PartId::from_str(str).unwrap_err());
        assert_eq!(err, "Invalid Arch field: x86_65");

        let str = "bpt@1.0.0:aarch65";
        let err = format!("{}", PartId::from_str(str).unwrap_err());
        assert_eq!(err, "Invalid Arch field: aarch65");

        let str = "bpt:aarch65@1.0.0";
        let err = format!("{}", PartId::from_str(str).unwrap_err());
        assert_eq!(err, "Invalid Arch field: aarch65");
    }

    #[test]
    fn bad_ver() {
        let expect = "Invalid PkgVer field: foo";
        let str = "bpt@foo";
        let err = format!("{}", PartId::from_str(str).unwrap_err());
        assert_eq!(err, expect);

        let str = "bpt@foo:aarch64";
        let err = format!("{}", PartId::from_str(str).unwrap_err());
        assert_eq!(err, expect);

        let str = "bpt:aarch64@foo";
        let err = format!("{}", PartId::from_str(str).unwrap_err());
        assert_eq!(err, expect);
    }
}
