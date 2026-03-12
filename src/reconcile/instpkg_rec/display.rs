//! User-facing plan rendering for installed-package reconciliation.

use crate::{
    color::Color,
    metadata::PartId,
    reconcile::instpkg_rec::{
        InstPkgPlan, InstallOp, PlanVerb, RemoveOp, ReplaceOp, RetainOp, WorldChange,
    },
};
use std::fmt;

impl fmt::Display for InstPkgPlan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for op in &self.remove {
            op.fmt_with_verb(PlanVerb::Remove, f)?;
        }
        for op in &self.install {
            op.fmt_with_verb(PlanVerb::Install, f)?;
        }
        for op in &self.upgrade {
            op.fmt_with_verb(PlanVerb::Upgrade, f)?;
        }
        for op in &self.downgrade {
            op.fmt_with_verb(PlanVerb::Downgrade, f)?;
        }
        for op in &self.retain {
            op.fmt_with_verb(PlanVerb::Retain, f)?;
        }
        Ok(())
    }
}

impl RemoveOp {
    fn fmt_with_verb(&self, verb: PlanVerb, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}{}{} {}",
            verb.color(),
            verb.as_str(),
            Color::Default,
            self.pkgid.color()
        )?;
        if let Some(world_change) = &self.world_change {
            write!(
                f,
                " {}({}){}",
                Color::Deemphasize,
                world_change,
                Color::Default
            )?;
        }
        writeln!(f)
    }
}

impl InstallOp {
    fn fmt_with_verb(&self, verb: PlanVerb, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}{}{} {}",
            verb.color(),
            verb.as_str(),
            Color::Default,
            self.source.pkgid().color()
        )?;
        let mut meta = Vec::new();
        if let Some(world_change) = &self.world_change {
            meta.push(world_change.to_string());
        }
        if self.source.needs_build() {
            meta.push("build".to_string());
        }
        if !meta.is_empty() {
            write!(
                f,
                " {}({}){}",
                Color::Deemphasize,
                meta.join("; "),
                Color::Default
            )?;
        }
        writeln!(f)
    }
}

impl ReplaceOp {
    fn fmt_with_verb(&self, verb: PlanVerb, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}{}{} {}",
            verb.color(),
            verb.as_str(),
            Color::Default,
            self.to.pkgid().color()
        )?;
        write!(
            f,
            " {}({}from {}",
            Color::Deemphasize,
            Color::Deemphasize,
            self.from.color()
        )?;
        write!(f, "{}", Color::Deemphasize)?;
        if let Some(world_change) = &self.world_change {
            write!(f, "; {world_change}")?;
        }
        if self.to.needs_build() {
            write!(f, "; build")?;
        }
        write!(f, "){}", Color::Default)?;
        writeln!(f)
    }
}

impl RetainOp {
    fn fmt_with_verb(&self, verb: PlanVerb, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}{}{} {}",
            verb.color(),
            verb.as_str(),
            Color::Default,
            self.pkgid.color()
        )?;
        if let Some(world_change) = &self.world_change {
            write!(
                f,
                " {}({}){}",
                Color::Deemphasize,
                world_change,
                Color::Default
            )?;
        }
        writeln!(f)
    }
}

impl PlanVerb {
    fn as_str(self) -> &'static str {
        match self {
            Self::Install => "Install",
            Self::Remove => "Remove",
            Self::Upgrade => "Upgrade",
            Self::Downgrade => "Downgrade",
            Self::Retain => "Retain",
        }
    }

    fn color(self) -> Color {
        match self {
            Self::Install => Color::Create,
            Self::Remove => Color::Remove,
            Self::Upgrade => Color::Upgrade,
            Self::Downgrade => Color::Downgrade,
            Self::Retain => Color::Deemphasize,
        }
    }
}

impl WorldChange {
    fn describe_entries(entries: &[PartId]) -> String {
        entries
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(", ")
    }
}

impl fmt::Display for WorldChange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Add(partid) => write!(f, "world add {partid}"),
            Self::Remove(from) => write!(f, "world remove {}", Self::describe_entries(from)),
            Self::Replace { from, to } => {
                write!(f, "world {} -> {to}", Self::describe_entries(from))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::marshalling::{FieldList, FieldStr, FromFieldStr};
    use crate::metadata::{
        Arch, Backup, Depends, Homepage, License, MakeArchs, MakeBins, MakeDepends, PkgDesc, PkgId,
        PkgInfo, PkgName, PkgVer, RepoPath,
    };
    use crate::reconcile::instpkg_rec::TargetSource;

    fn make_pkginfo(pkgname: &str, pkgver: &str, arch: Arch) -> PkgInfo {
        PkgInfo {
            pkgid: PkgId::new(
                PkgName::try_from(pkgname).unwrap(),
                PkgVer::try_from(pkgver).unwrap(),
                arch,
            ),
            pkgdesc: PkgDesc::from_field_str(FieldStr::try_from("test package").unwrap()).unwrap(),
            homepage: Homepage::from_field_str(FieldStr::try_from("https://example.com").unwrap())
                .unwrap(),
            license: License::from_field_str(FieldStr::try_from("MIT").unwrap()).unwrap(),
            backup: Backup::new(),
            depends: Depends::new(),
            makearchs: MakeArchs::new(),
            makebins: MakeBins::new(),
            makedepends: MakeDepends::new(),
            repopath: RepoPath::empty(),
        }
    }

    #[test]
    fn replace_op_reapplies_deemphasize_after_colored_from_pkgid() {
        let plan = InstPkgPlan {
            upgrade: vec![ReplaceOp {
                from: PkgId::new(
                    PkgName::try_from("old").unwrap(),
                    PkgVer::try_from("1.0.0").unwrap(),
                    Arch::noarch,
                ),
                to: TargetSource::Installed(make_pkginfo("new", "2.0.0", Arch::noarch)),
                world_change: Some(WorldChange::Add(PartId::new(
                    PkgName::try_from("new").unwrap(),
                    None,
                    Some(Arch::noarch),
                ))),
            }],
            ..Default::default()
        };

        let rendered = format!("{plan}");
        let expected = format!(
            "{}from {}{}; world add new:noarch){}",
            Color::Deemphasize,
            PkgId::new(
                PkgName::try_from("old").unwrap(),
                PkgVer::try_from("1.0.0").unwrap(),
                Arch::noarch,
            )
            .color(),
            Color::Deemphasize,
            Color::Default
        );

        assert!(rendered.contains(&expected), "{rendered:?}");
    }
}
