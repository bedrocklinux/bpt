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
        let mut meta = vec![format!("from {}", self.from.color())];
        if let Some(world_change) = &self.world_change {
            meta.push(world_change.to_string());
        }
        if self.to.needs_build() {
            meta.push("build".to_string());
        }
        write!(
            f,
            " {}({}){}",
            Color::Deemphasize,
            meta.join("; "),
            Color::Default
        )?;
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
