use crate::{make_field_list, metadata::*};

/// Install/runtime dependencies on some other package(s)
#[derive(Clone, Debug)]
pub struct Depends(Vec<Depend>);

make_field_list!(Depends, PkgKey, Depend);

impl Depends {
    /// Used to translate between bbuild and bpt pkgids.
    pub fn populate_depends_arch_if_missing(&self, arch: Arch) -> Self {
        Depends(
            self.0
                .iter()
                .map(|d| d.populate_depends_arch_if_missing(arch))
                .collect(),
        )
    }
}
