use crate::{make_field_list, metadata::*};

/// Build-time dependencies on some other package(s)
///
/// This should never be populated within a built package (`*.bpt`).
#[derive(Clone, Debug)]
pub struct MakeDepends(Vec<Depend>);

make_field_list!(MakeDepends, PkgKey, Depend);

impl MakeDepends {
    /// Used to translate between bbuild and bpt pkgids.
    pub fn populate_depends_arch_if_missing(&self, arch: Arch) -> Self {
        MakeDepends(
            self.0
                .iter()
                .map(|d| d.populate_depends_arch_if_missing(arch))
                .collect(),
        )
    }
}
