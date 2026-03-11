use crate::{
    color::Color, constant::*, error::*, make_display_color, make_field, marshalling::*,
    metadata::*,
};
use std::borrow::Cow;

/// A package dependency
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct Depend {
    pub pkgname: PkgName,
    pub version_req: Option<VersionReq>,
    pub arch: Option<Arch>,
}

make_field!(Depend, PkgKey);

make_display_color!(Depend, |s, f| {
    write!(f, "{}", s.pkgname.color())?;

    if let Some(version_req) = &s.version_req {
        write!(f, "{}", version_req.color())?;
    }

    if let Some(arch) = &s.arch {
        write!(f, "{}:{}", Color::Glue, arch.color())?;
    }

    Ok(())
});

impl std::fmt::Display for Depend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.pkgname)?;

        if let Some(version_req) = &self.version_req {
            write!(f, "{version_req}")?;
        }

        if let Some(arch) = &self.arch {
            write!(f, ":{arch}")?;
        }

        Ok(())
    }
}

impl std::fmt::Debug for Depend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self}")
    }
}

impl Depend {
    pub fn provided_by(&self, provider: &PkgId) -> bool {
        if provider.pkgname != self.pkgname {
            return false;
        }

        match &self.version_req {
            // Version constraints requested
            Some(ver_req) if !ver_req.provided_by(&provider.pkgver) => return false,
            _ => {}
        }

        match self.arch {
            // Specific arch requested
            Some(arch) if provider.arch != arch => return false,
            _ => {}
        }

        true
    }

    /// Used to translate between bbuild and bpt pkgids.
    pub fn populate_depends_arch_if_missing(&self, arch: Arch) -> Self {
        Self {
            pkgname: self.pkgname.clone(),
            version_req: self.version_req.clone(),
            arch: self.arch.or(Some(arch)),
        }
    }
}

impl FromFieldStr for Depend {
    fn from_field_str(value: FieldStr) -> Result<Self, AnonLocErr> {
        match value
            .as_str()
            .find(|c: char| SEMVER_CMP_CHARS.contains(&c))
            .map(|idx| value.split_at(idx))
        {
            None => match value.split_once(":") {
                // pkgname
                None => Ok(Depend {
                    pkgname: PkgName::from_field_str(value)?,
                    version_req: None,
                    arch: None,
                }),
                // pkgname:arch
                Some((pkgname, arch)) => Ok(Depend {
                    pkgname: PkgName::from_field_str(pkgname)?,
                    version_req: None,
                    arch: Some(Arch::from_field_str(arch)?),
                }),
            },
            Some((start, end)) => {
                match end.split_once(":") {
                    // pkgname=version_req
                    None => Ok(Depend {
                        pkgname: PkgName::from_field_str(start)?,
                        version_req: Some(
                            VersionReq::from_field_str(end).map_err(|e| e.field(Self::NAME))?,
                        ),
                        arch: None,
                    }),
                    // pkgname=version_req:arch
                    Some((version_req, arch)) => Ok(Depend {
                        pkgname: PkgName::from_field_str(start)?,
                        version_req: Some(
                            VersionReq::from_field_str(version_req)
                                .map_err(|e| e.field(Self::NAME))?,
                        ),
                        arch: Some(Arch::from_field_str(arch)?),
                    }),
                }
            }
        }
    }
}

impl AsBytes for Depend {
    fn as_bytes(&self) -> Cow<'_, [u8]> {
        Cow::Owned(format!("{self}").into_bytes())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn test_provided_by_pkgname() {
        for (depend, provider, expect) in &[
            ("htop", "htop@1.0.0:x86_64", true),
            ("htop", "vim@1.0.0:x86_64", false),
            ("vim", "vim@1.0.0:x86_64", true),
        ] {
            let depend = FieldStr::try_from(*depend)
                .map_err(|e| e.field(Depend::NAME))
                .and_then(Depend::from_field_str)
                .unwrap();
            let provider = PartId::from_str(provider).unwrap().as_pkgid().unwrap();
            assert!(depend.provided_by(&provider) == *expect);
        }
    }

    #[test]
    fn test_provided_by_arch() {
        for (depend_arch, provider_arch, expect) in &[
            (Some("aarch64"), "aarch64", true),
            (Some("aarch64"), "x86_64", false),
            (Some("aarch64"), "noarch", false),
            (Some("x86_64"), "aarch64", false),
            (Some("x86_64"), "x86_64", true),
            (Some("x86_64"), "noarch", false),
            (None, "aarch64", true),
            (None, "x86_64", true),
            (None, "noarch", true),
        ] {
            let provider = PkgId {
                pkgname: PkgName::from_str("htop").unwrap(),
                pkgver: PkgVer::from_str("1.0.0").unwrap(),
                arch: Arch::from_str(provider_arch).unwrap(),
            };
            let depend = Depend {
                pkgname: PkgName::from_str("htop").unwrap(),
                version_req: None,
                arch: depend_arch.map(|a| Arch::from_str(a).unwrap()),
            };
            assert!(
                depend.provided_by(&provider) == *expect,
                "depend: {:?}, provider: {}, expect: {}, actual: {}",
                depend_arch,
                provider_arch,
                expect,
                depend.provided_by(&provider)
            );
        }
    }

    #[test]
    fn test_provided_by_version() {
        for (depend, provider, expect) in &[
            ("vim=1.0.0", "vim@1.0.0:x86_64", true),
            ("vim>1.0.0", "vim@1.0.0:x86_64", false),
            ("vim>=1.0.0", "vim@1.0.0:x86_64", true),
            ("vim<1.0.0", "vim@1.0.0:x86_64", false),
            ("vim<=1.0.0", "vim@1.0.0:x86_64", true),
            ("vim~1.0.0", "vim@1.0.0:x86_64", true),
            ("vim^1.0.0", "vim@1.0.0:x86_64", true),
            ("vim=1.0.0", "vim@e1-1.0.0:x86_64", false),
            ("vim>1.0.0", "vim@e1-1.0.0:x86_64", true),
            ("vim>=1.0.0", "vim@e1-1.0.0:x86_64", true),
            ("vim<1.0.0", "vim@e1-1.0.0:x86_64", false),
            ("vim<=1.0.0", "vim@e1-1.0.0:x86_64", false),
            ("vim~1.0.0", "vim@e1-1.0.0:x86_64", false),
            ("vim^1.0.0", "vim@e1-1.0.0:x86_64", false),
            ("vim=e1-1.0.0", "vim@1.0.0:x86_64", false),
            ("vim>e1-1.0.0", "vim@1.0.0:x86_64", false),
            ("vim>=e1-1.0.0", "vim@1.0.0:x86_64", false),
            ("vim<e1-1.0.0", "vim@1.0.0:x86_64", true),
            ("vim<=e1-1.0.0", "vim@1.0.0:x86_64", true),
            ("vim~e1-1.0.0", "vim@1.0.0:x86_64", false),
            ("vim^e1-1.0.0", "vim@1.0.0:x86_64", false),
        ] {
            let depend = FieldStr::try_from(*depend)
                .map_err(|e| e.field(Depend::NAME))
                .and_then(Depend::from_field_str)
                .unwrap();
            let provider = PartId::from_str(provider).unwrap().as_pkgid().unwrap();
            assert!(
                depend.provided_by(&provider) == *expect,
                "{provider} provides {depend} -> expected {expect}",
            );
        }
    }
}
