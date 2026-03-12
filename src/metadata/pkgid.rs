use std::borrow::Borrow;

use crate::{color::*, error::*, make_display_color, marshalling::*, metadata::*};
use camino::Utf8PathBuf;

/// A package identifier.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PkgId {
    pub pkgname: PkgName,
    pub pkgver: PkgVer,
    pub arch: Arch,
}
make_display_color!(PkgId, |s, f| {
    write!(
        f,
        "{}{}{}@{}{}{}:{}{}{}",
        Color::PkgName,
        s.pkgname,
        Color::Glue,
        Color::PkgVer,
        s.pkgver,
        Color::Glue,
        Color::Arch,
        s.arch,
        Color::Default
    )
});

impl std::fmt::Display for PkgId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}@{}:{}", self.pkgname, self.pkgver, self.arch,)
    }
}

impl PkgId {
    pub fn new(pkgname: PkgName, pkgver: PkgVer, arch: Arch) -> Self {
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
            arch,
        }
    }

    pub fn canonical_filename(&self) -> Utf8PathBuf {
        if self.arch == Arch::bbuild {
            format!("{}@{}.bbuild", self.pkgname, self.pkgver).into()
        } else {
            format!("{}@{}:{}.bpt", self.pkgname, self.pkgver, self.arch).into()
        }
    }

    pub fn to_pkgidpart(&self) -> PartId {
        PartId {
            pkgname: self.pkgname.clone(),
            pkgver: Some(self.pkgver.clone()),
            arch: Some(self.arch),
        }
    }

    /// Check if `self` is preferable to another [PkgId] based on:
    ///
    /// - How early the [Arch] shows up in the default_archs list
    /// - If the [Arch] is the same, compare the [PkgVer]
    ///
    /// This is useful if multiple [PkgId]s match a given requirement such as matching a [PartId]
    /// or fulfilling a [Depend].
    ///
    /// We're not implementing `Ord` due to the need for `default_archs`.
    pub fn better_match_than(&self, other: &Self, default_archs: &[Arch]) -> std::cmp::Ordering {
        use std::cmp::Ordering::*;

        // First, compare arch fields.  Earlier in the list is better.
        let self_arch_index = default_archs.iter().position(|a| a == &self.arch);
        let other_arch_index = default_archs.iter().position(|a| a == &other.arch);

        match (self_arch_index, other_arch_index) {
            (None, None) => self.pkgver.cmp(&other.pkgver),
            (None, Some(_)) => Less,
            (Some(_), None) => Greater,
            (Some(a), Some(b)) if a == b => self.pkgver.cmp(&other.pkgver),
            (Some(a), Some(b)) => b.cmp(&a),
        }
    }
}

impl<W: std::io::Write> Serialize<W> for PkgId {
    fn serialize(&self, w: &mut W) -> Result<(), AnonLocErr> {
        self.pkgname.serialize(w)?;
        self.pkgver.serialize(w)?;
        self.arch.serialize(w)?;
        Ok(())
    }
}

impl ColorPkgId<'_> {
    pub fn canonical_filename(&self) -> String {
        if self.0.arch == Arch::bbuild {
            format!(
                "{}{}{}@{}{}{}.{}bbuild{}",
                Color::PkgName,
                self.0.pkgname,
                Color::Glue,
                Color::PkgVer,
                self.0.pkgver,
                Color::Glue,
                Color::File,
                Color::Default
            )
        } else {
            format!(
                "{}{}{}@{}{}{}:{}{}{}.{}bpt{}",
                Color::PkgName,
                self.0.pkgname,
                Color::Glue,
                Color::PkgVer,
                self.0.pkgver,
                Color::Glue,
                Color::Arch,
                self.0.arch,
                Color::Glue,
                Color::File,
                Color::Default
            )
        }
    }
}

// The code base has multiple collections of `(PkgId, _)` pairs (such as maps) which can be filtered
// down (e.g. by those that provide a dependency).  After filtering, this is used to find the best
// [PkgId] of potentially multiple which pass the filter.
pub trait BestPkgId<'a, K, V>
where
    K: Borrow<PkgId>,
{
    fn select_best_pkgid(self, default_archs: &[Arch]) -> Option<&'a V>;
}

impl<'a, K, V: 'a, I> BestPkgId<'a, K, V> for I
where
    I: Iterator<Item = (K, &'a V)>,
    K: Borrow<PkgId>,
{
    fn select_best_pkgid(self, default_archs: &[Arch]) -> Option<&'a V> {
        self.fold(None, |best, cur| match best {
            None => Some(cur),
            Some(best) => match best
                .0
                .borrow()
                .better_match_than(cur.0.borrow(), default_archs)
            {
                std::cmp::Ordering::Greater => Some(best),
                std::cmp::Ordering::Equal => Some(best),
                std::cmp::Ordering::Less => Some(cur),
            },
        })
        .map(|(_, v)| v)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cmp::Ordering::*;
    use std::str::FromStr;

    fn mkpkgid(name: &str, ver: &str, arch: &str) -> PkgId {
        PkgId {
            pkgname: PkgName::try_from(name).unwrap(),
            pkgver: PkgVer::try_from(ver).unwrap(),
            arch: Arch::from_str(arch).unwrap(),
        }
    }

    // default_archs = [noarch, x86_64, aarch64] for most tests
    fn default_archs() -> Vec<Arch> {
        vec![
            Arch::from_str("noarch").unwrap(),
            Arch::from_str("x86_64").unwrap(),
            Arch::from_str("aarch64").unwrap(),
        ]
    }

    #[test]
    fn preferred_arch_wins_over_higher_version() {
        let archs = default_archs();
        // noarch (index 0) vs x86_64 (index 1): noarch is preferred even with lower version
        let a = mkpkgid("foo", "1.0.0", "noarch");
        let b = mkpkgid("foo", "2.0.0", "x86_64");
        assert_eq!(a.better_match_than(&b, &archs), Greater);
        assert_eq!(b.better_match_than(&a, &archs), Less);
    }

    #[test]
    fn same_arch_tiebreaks_on_version() {
        let archs = default_archs();
        let a = mkpkgid("foo", "2.0.0", "x86_64");
        let b = mkpkgid("foo", "1.0.0", "x86_64");
        assert_eq!(a.better_match_than(&b, &archs), Greater);
        assert_eq!(b.better_match_than(&a, &archs), Less);
    }

    #[test]
    fn same_arch_same_version_is_equal() {
        let archs = default_archs();
        let a = mkpkgid("foo", "1.0.0", "x86_64");
        let b = mkpkgid("foo", "1.0.0", "x86_64");
        assert_eq!(a.better_match_than(&b, &archs), Equal);
    }

    #[test]
    fn in_list_beats_not_in_list() {
        let archs = default_archs(); // noarch, x86_64, aarch64
        let a = mkpkgid("foo", "1.0.0", "aarch64"); // in list (index 2)
        let b = mkpkgid("foo", "9.0.0", "riscv64gc"); // not in list
        assert_eq!(a.better_match_than(&b, &archs), Greater);
        assert_eq!(b.better_match_than(&a, &archs), Less);
    }

    #[test]
    fn neither_in_list_tiebreaks_on_version() {
        let archs = default_archs();
        let a = mkpkgid("foo", "2.0.0", "riscv64gc");
        let b = mkpkgid("foo", "1.0.0", "s390x");
        assert_eq!(a.better_match_than(&b, &archs), Greater);
        assert_eq!(b.better_match_than(&a, &archs), Less);
    }

    #[test]
    fn neither_in_list_same_version_is_equal() {
        let archs = default_archs();
        let a = mkpkgid("foo", "1.0.0", "riscv64gc");
        let b = mkpkgid("foo", "1.0.0", "s390x");
        assert_eq!(a.better_match_than(&b, &archs), Equal);
    }

    #[test]
    fn arch_ordering_matches_list_position() {
        let archs = default_archs(); // noarch=0, x86_64=1, aarch64=2
        let noarch = mkpkgid("foo", "1.0.0", "noarch");
        let x86 = mkpkgid("foo", "1.0.0", "x86_64");
        let aarch = mkpkgid("foo", "1.0.0", "aarch64");

        // noarch (0) > x86_64 (1) > aarch64 (2)
        assert_eq!(noarch.better_match_than(&x86, &archs), Greater);
        assert_eq!(noarch.better_match_than(&aarch, &archs), Greater);
        assert_eq!(x86.better_match_than(&aarch, &archs), Greater);
        assert_eq!(aarch.better_match_than(&x86, &archs), Less);
        assert_eq!(aarch.better_match_than(&noarch, &archs), Less);
        assert_eq!(x86.better_match_than(&noarch, &archs), Less);
    }

    #[test]
    fn canonical_filename_uses_pkgver_for_bbuilds() {
        let pkgid = mkpkgid("foo", "1.2.3", "bbuild");
        assert_eq!(pkgid.canonical_filename(), "foo@1.2.3.bbuild");
    }

    #[test]
    fn same_arch_version_with_epoch() {
        let archs = default_archs();
        let a = mkpkgid("foo", "e1-1.0.0", "x86_64");
        let b = mkpkgid("foo", "2.0.0", "x86_64");
        // epoch 1 > no epoch, regardless of semver
        assert_eq!(a.better_match_than(&b, &archs), Greater);
        assert_eq!(b.better_match_than(&a, &archs), Less);
    }

    #[test]
    fn same_arch_version_with_pkgrel() {
        let archs = default_archs();
        let a = mkpkgid("foo", "1.0.0-r2", "x86_64");
        let b = mkpkgid("foo", "1.0.0-r1", "x86_64");
        assert_eq!(a.better_match_than(&b, &archs), Greater);
        assert_eq!(b.better_match_than(&a, &archs), Less);
    }

    #[test]
    fn empty_default_archs() {
        let archs: Vec<Arch> = vec![];
        let a = mkpkgid("foo", "2.0.0", "x86_64");
        let b = mkpkgid("foo", "1.0.0", "aarch64");
        // Neither arch in list, falls back to version comparison
        assert_eq!(a.better_match_than(&b, &archs), Greater);
        assert_eq!(b.better_match_than(&a, &archs), Less);
    }

    #[test]
    fn antisymmetry() {
        // For all pairs, a > b implies b < a
        let archs = default_archs();
        let pkgs = vec![
            mkpkgid("foo", "1.0.0", "noarch"),
            mkpkgid("foo", "2.0.0", "noarch"),
            mkpkgid("foo", "1.0.0", "x86_64"),
            mkpkgid("foo", "2.0.0", "x86_64"),
            mkpkgid("foo", "1.0.0", "aarch64"),
            mkpkgid("foo", "1.0.0", "riscv64gc"),
        ];

        for a in &pkgs {
            for b in &pkgs {
                let ab = a.better_match_than(b, &archs);
                let ba = b.better_match_than(a, &archs);
                match ab {
                    Greater => assert_eq!(ba, Less, "{a} > {b} but {b} !< {a}"),
                    Less => assert_eq!(ba, Greater, "{a} < {b} but {b} !> {a}"),
                    Equal => assert_eq!(ba, Equal, "{a} == {b} but {b} != {a}"),
                }
            }
        }
    }
}
